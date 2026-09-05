// SPDX-License-Identifier: GPL-3.0-or-later
//
// Log-code selections adapted from P1sec/QCSuper:
//   upstream commit: aa555b4f7f25f7a8bf4e5afd4dcb884edf2f6735
//   upstream path: src/qcsuper/modules/_enable_log_mixin.py
// QCSuper declares GPL-3.0+ / GPL-3.0-or-later. The pinned upstream file has
// no per-file copyright header; repository/project attribution is preserved
// here and in THIRD_PARTY.md.
// Modified/translated for 6grok on 2026-09-05.

use clap::ValueEnum;

#[derive(Debug, Clone, Copy, ValueEnum)]
pub enum QcsuperProfile {
    /// 2G/3G/4G/5G Layer-2/Layer-3/NAS logs used by QCSuper raw packet capture.
    Signaling,
    /// Qualcomm Data Protocol Logging (DPL) IP traffic records.
    Ip,
    /// Union of signaling and IP records.
    Full,
}

/// QCSuper `TYPES_FOR_RAW_PACKET_LOGGING` at the pinned revision.
const SIGNALING: &[u16] = &[
    0x5226, // GPRS MAC signalling
    0x512f, // GSM RR signalling
    0x412f, // WCDMA signalling
    0xb0c0, // LTE RRC OTA
    0xb821, // NR RRC OTA
    0x713a, // UMTS NAS OTA
    0xb0e2, // LTE NAS ESM incoming
    0xb0e3, // LTE NAS ESM outgoing
    0xb0ec, // LTE NAS EMM incoming
    0xb0ed, // LTE NAS EMM outgoing
];

/// QCSuper `TYPES_FOR_IP_TRAFFIC_LOGGING` at the pinned revision.
const IP: &[u16] = &[
    0x11eb, // Data Protocol Logging
    0x1574, // Network IP RM TX full
    0x1575, // Network IP RM RX full
];

impl QcsuperProfile {
    pub fn codes(self) -> Vec<u16> {
        match self {
            Self::Signaling => SIGNALING.to_vec(),
            Self::Ip => IP.to_vec(),
            Self::Full => {
                let mut values = SIGNALING.to_vec();
                values.extend_from_slice(IP);
                values.sort_unstable();
                values.dedup();
                values
            }
        }
    }
}

pub fn merge_profiles(explicit: &[u16], profiles: &[QcsuperProfile]) -> Vec<u16> {
    let mut values = explicit.to_vec();
    for profile in profiles {
        values.extend(profile.codes());
    }
    values.sort_unstable();
    values.dedup();
    values
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn signaling_contains_qcsuper_lte_and_nr_rrc() {
        let profile = QcsuperProfile::Signaling.codes();
        assert!(profile.contains(&0xb0c0));
        assert!(profile.contains(&0xb821));
        assert!(profile.contains(&0x412f));
    }

    #[test]
    fn ip_profile_contains_dpl_trigger_codes() {
        assert_eq!(QcsuperProfile::Ip.codes(), vec![0x11eb, 0x1574, 0x1575]);
    }
}
