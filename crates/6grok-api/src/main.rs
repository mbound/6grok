//! 6grok aggregation/API service.

mod gsmtap;

use anyhow::{Context, Result};
use axum::{
    extract::{
        ws::{Message, WebSocket},
        Query, State, WebSocketUpgrade,
    },
    response::IntoResponse,
    routing::get,
    Json, Router,
};
use clap::Parser;
use gsmtap::{GsmtapSink, GSMTAP_UDP_PORT};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use sixgrok_core::{decode_wire_frame, CaptureFrame, Vendor};
use std::{
    collections::{BTreeMap, VecDeque},
    net::SocketAddr,
    sync::Arc,
    time::{SystemTime, UNIX_EPOCH},
};
use tokio::{
    io::AsyncReadExt,
    net::{TcpListener, TcpStream},
    sync::{broadcast, RwLock},
};

const MAX_WIRE_FRAME: usize = 8 * 1024 * 1024;

#[derive(Debug, Parser)]
#[command(name = "6grok-api")]
#[command(about = "6grok agent ingest, REST API and live WebSocket service")]
struct Cli {
    /// HTTP/REST/WebSocket listen address.
    #[arg(long, default_value = "0.0.0.0:8080")]
    http: SocketAddr,
    /// Agent TCP/MessagePack ingest listen address.
    #[arg(long, default_value = "0.0.0.0:5566")]
    ingest: SocketAddr,
    /// Number of most recent decoded packets retained in memory.
    #[arg(long, default_value_t = 5000)]
    history: usize,
    /// Mirror Qualcomm DIAG frames to Wireshark using GSMTAP type QC_DIAG.
    /// Example: --gsmtap 127.0.0.1:4729
    #[arg(long)]
    gsmtap: Option<String>,
}

#[derive(Clone)]
struct AppState {
    store: Arc<RwLock<Store>>,
    live: broadcast::Sender<String>,
    history_limit: usize,
    gsmtap: Option<Arc<GsmtapSink>>,
}

struct Store {
    started_at_ms: i64,
    received: u64,
    fully_decoded: u64,
    by_vendor: BTreeMap<String, u64>,
    by_rat: BTreeMap<String, u64>,
    by_layer: BTreeMap<String, u64>,
    history: VecDeque<Value>,
}

impl Store {
    fn new() -> Self {
        Self {
            started_at_ms: unix_ms(),
            received: 0,
            fully_decoded: 0,
            by_vendor: BTreeMap::new(),
            by_rat: BTreeMap::new(),
            by_layer: BTreeMap::new(),
            history: VecDeque::new(),
        }
    }
}

#[derive(Debug, Serialize)]
struct Stats {
    started_at_ms: i64,
    uptime_ms: i64,
    received: u64,
    fully_decoded: u64,
    decode_ratio: f64,
    live_subscribers: usize,
    gsmtap_enabled: bool,
    by_vendor: BTreeMap<String, u64>,
    by_rat: BTreeMap<String, u64>,
    by_layer: BTreeMap<String, u64>,
    history_len: usize,
}

#[derive(Debug, Deserialize)]
struct PacketQuery {
    limit: Option<usize>,
}

#[tokio::main]
async fn main() -> Result<()> {
    let cli = Cli::parse();
    let history_limit = cli.history.max(1);
    let (live, _) = broadcast::channel(2048);
    let gsmtap = cli
        .gsmtap
        .as_deref()
        .map(GsmtapSink::connect)
        .transpose()
        .context("creating GSMTAP UDP sink")?
        .map(Arc::new);
    if let Some(destination) = &cli.gsmtap {
        eprintln!(
            "6grok-api: mirroring Qualcomm DIAG to GSMTAP {destination} (canonical port is {GSMTAP_UDP_PORT})"
        );
    }

    let state = AppState {
        store: Arc::new(RwLock::new(Store::new())),
        live,
        history_limit,
        gsmtap,
    };

    let ingest_state = state.clone();
    let ingest_addr = cli.ingest;
    tokio::spawn(async move {
        if let Err(err) = run_ingest(ingest_addr, ingest_state).await {
            eprintln!("6grok-api: ingest listener failed: {err:#}");
        }
    });

    let app = Router::new()
        .route("/health", get(health))
        .route("/api/v1/stats", get(stats))
        .route("/api/v1/packets", get(packets))
        .route("/ws", get(ws_upgrade))
        .with_state(state);

    let listener = TcpListener::bind(cli.http)
        .await
        .with_context(|| format!("binding HTTP listener {}", cli.http))?;
    eprintln!("6grok-api: HTTP/WebSocket listening on {}", cli.http);
    eprintln!("6grok-api: agent ingest listening on {}", cli.ingest);

    axum::serve(listener, app)
        .with_graceful_shutdown(shutdown_signal())
        .await
        .context("serving HTTP API")?;
    Ok(())
}

async fn health() -> Json<Value> {
    Json(json!({"status":"ok"}))
}

async fn stats(State(state): State<AppState>) -> Json<Stats> {
    let store = state.store.read().await;
    let now = unix_ms();
    let ratio = if store.received == 0 {
        0.0
    } else {
        store.fully_decoded as f64 / store.received as f64
    };
    Json(Stats {
        started_at_ms: store.started_at_ms,
        uptime_ms: now.saturating_sub(store.started_at_ms),
        received: store.received,
        fully_decoded: store.fully_decoded,
        decode_ratio: ratio,
        live_subscribers: state.live.receiver_count(),
        gsmtap_enabled: state.gsmtap.is_some(),
        by_vendor: store.by_vendor.clone(),
        by_rat: store.by_rat.clone(),
        by_layer: store.by_layer.clone(),
        history_len: store.history.len(),
    })
}

async fn packets(
    State(state): State<AppState>,
    Query(query): Query<PacketQuery>,
) -> Json<Vec<Value>> {
    let limit = query.limit.unwrap_or(100).clamp(1, 1000);
    let store = state.store.read().await;
    let start = store.history.len().saturating_sub(limit);
    Json(store.history.iter().skip(start).cloned().collect())
}

async fn ws_upgrade(ws: WebSocketUpgrade, State(state): State<AppState>) -> impl IntoResponse {
    ws.on_upgrade(move |socket| websocket(socket, state))
}

async fn websocket(mut socket: WebSocket, state: AppState) {
    let mut rx = state.live.subscribe();
    loop {
        match rx.recv().await {
            Ok(text) => {
                if socket.send(Message::Text(text.into())).await.is_err() {
                    break;
                }
            }
            Err(broadcast::error::RecvError::Lagged(skipped)) => {
                let notice = json!({"type":"lagged","skipped":skipped}).to_string();
                if socket.send(Message::Text(notice.into())).await.is_err() {
                    break;
                }
            }
            Err(broadcast::error::RecvError::Closed) => break,
        }
    }
}

async fn run_ingest(addr: SocketAddr, state: AppState) -> Result<()> {
    let listener = TcpListener::bind(addr)
        .await
        .with_context(|| format!("binding agent ingest listener {addr}"))?;
    loop {
        let (stream, peer) = listener.accept().await.context("accepting agent")?;
        let state = state.clone();
        tokio::spawn(async move {
            if let Err(err) = ingest_client(stream, state).await {
                eprintln!("6grok-api: agent {peer} disconnected with error: {err:#}");
            }
        });
    }
}

async fn ingest_client(mut stream: TcpStream, state: AppState) -> Result<()> {
    loop {
        let mut len_buf = [0_u8; 4];
        match stream.read_exact(&mut len_buf).await {
            Ok(_) => {}
            Err(err) if err.kind() == std::io::ErrorKind::UnexpectedEof => return Ok(()),
            Err(err) => return Err(err).context("reading agent frame length"),
        }
        let len = u32::from_be_bytes(len_buf) as usize;
        if len == 0 || len > MAX_WIRE_FRAME {
            anyhow::bail!("invalid agent frame length {len}");
        }
        let mut payload = vec![0_u8; len];
        stream
            .read_exact(&mut payload)
            .await
            .context("reading agent MessagePack frame")?;
        let frame = decode_wire_frame(&payload).context("decoding agent MessagePack frame")?;
        process_frame(frame, &state).await?;
    }
}

async fn process_frame(frame: CaptureFrame, state: &AppState) -> Result<()> {
    if frame.vendor == Vendor::Qualcomm {
        if let Some(sink) = &state.gsmtap {
            if let Some(raw_diag) = frame.payload.get(2..) {
                sink.send_qc_diag(raw_diag)
                    .context("sending Qualcomm DIAG frame over GSMTAP")?;
            }
        }
    }

    let decoded = frame.decode();
    let value = serde_json::to_value(&decoded).context("serializing decoded packet")?;
    let text = serde_json::to_string(&decoded).context("encoding live packet JSON")?;

    {
        let mut store = state.store.write().await;
        store.received += 1;
        if decoded.fully_decoded {
            store.fully_decoded += 1;
        }
        let vendor = if decoded.vendor.is_empty() {
            "Qualcomm".to_owned()
        } else {
            decoded.vendor.clone()
        };
        *store.by_vendor.entry(vendor).or_default() += 1;
        *store.by_rat.entry(decoded.rat.clone()).or_default() += 1;
        *store.by_layer.entry(decoded.layer.clone()).or_default() += 1;
        store.history.push_back(value);
        while store.history.len() > state.history_limit {
            store.history.pop_front();
        }
    }

    let _ = state.live.send(text);
    Ok(())
}

async fn shutdown_signal() {
    let _ = tokio::signal::ctrl_c().await;
}

fn unix_ms() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as i64
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn process_frame_updates_store() {
        let (live, _) = broadcast::channel(8);
        let state = AppState {
            store: Arc::new(RwLock::new(Store::new())),
            live,
            history_limit: 2,
            gsmtap: None,
        };
        let frame = CaptureFrame {
            sequence: 1,
            timestamp_wall: 1,
            timestamp_mono: 1,
            vendor: Vendor::Qualcomm,
            log_code: 0x0098,
            payload: vec![0x98, 0x00, 0x10, 0, 0, 0, 0, 0, 0x98, 0x00],
        };
        process_frame(frame, &state).await.unwrap();
        let store = state.store.read().await;
        assert_eq!(store.received, 1);
        assert_eq!(store.history.len(), 1);
    }
}
