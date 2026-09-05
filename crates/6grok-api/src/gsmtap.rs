//! Minimal, original GSMTAP v2 UDP exporter.
//!
//! The wire format and numeric type assignments are public protocol facts from
//! the canonical Osmocom/Wireshark GSMTAP definition. No libosmocore or
//! Wireshark implementation source is incorporated into this MIT module.

use std::io;
use std::net::{SocketAddr, ToSocketAddrs, UdpSocket};

pub const GSMTAP_UDP_PORT: u16 = 4729;
pub const GSMTAP_VERSION: u8 = 2;
pub const GSMTAP_HEADER_WORDS: u8 = 4;
pub const GSMTAP_TYPE_QC_DIAG: u8 = 0x11;
const GSMTAP_HEADER_LEN: usize = 16;

#[derive(Debug)]
pub struct GsmtapSink {
    socket: UdpSocket,
    destination: SocketAddr,
}

impl GsmtapSink {
    pub fn connect(destination: &str) -> io::Result<Self> {
        let destination = destination
            .to_socket_addrs()?
            .next()
            .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidInput, "GSMTAP destination resolved to no address"))?;
        let socket = match destination {
            SocketAddr::V4(_) => UdpSocket::bind("0.0.0.0:0")?,
            SocketAddr::V6(_) => UdpSocket::bind("[::]:0")?,
        };
        Ok(Self { socket, destination })
    }

    /// Send one de-framed Qualcomm DIAG packet.
    ///
    /// `packet` begins with the DIAG command byte. HDLC escaping, CRC and the
    /// `0x7e` delimiter are not included in the GSMTAP payload.
    pub fn send_qc_diag(&self, packet: &[u8]) -> io::Result<usize> {
        let datagram = qc_diag_datagram(packet);
        self.socket.send_to(&datagram, self.destination)
    }
}

pub fn qc_diag_datagram(packet: &[u8]) -> Vec<u8> {
    let mut out = Vec::with_capacity(GSMTAP_HEADER_LEN + packet.len());
    out.extend_from_slice(&gsmtap_header(GSMTAP_TYPE_QC_DIAG, 0));
    out.extend_from_slice(packet);
    out
}

/// Serialize the canonical 16-byte GSMTAP v2 header.
pub fn gsmtap_header(packet_type: u8, subtype: u8) -> [u8; GSMTAP_HEADER_LEN] {
    let mut header = [0_u8; GSMTAP_HEADER_LEN];
    header[0] = GSMTAP_VERSION;
    header[1] = GSMTAP_HEADER_WORDS;
    header[2] = packet_type;
    header[3] = 0; // timeslot
    header[4..6].copy_from_slice(&0_u16.to_be_bytes()); // ARFCN
    header[6] = 0; // signal dBm
    header[7] = 0; // SNR dB
    header[8..12].copy_from_slice(&0_u32.to_be_bytes()); // frame number
    header[12] = subtype;
    header[13] = 0; // antenna
    header[14] = 0; // sub-slot
    header[15] = 0; // reserved
    header
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn qc_diag_header_matches_canonical_type() {
        let header = gsmtap_header(GSMTAP_TYPE_QC_DIAG, 0);
        assert_eq!(header.len(), 16);
        assert_eq!(&header[..4], &[2, 4, 0x11, 0]);
        assert!(header[4..].iter().all(|&b| b == 0));
    }

    #[test]
    fn datagram_preserves_diag_packet_verbatim() {
        let packet = [0x10, 0x00, 0x34, 0x12, 0x7e, 0x7d];
        let datagram = qc_diag_datagram(&packet);
        assert_eq!(&datagram[..16], &gsmtap_header(0x11, 0));
        assert_eq!(&datagram[16..], &packet);
    }
}
