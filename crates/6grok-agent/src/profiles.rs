//! Curated Qualcomm capture profiles built from log codes already documented
//! by the MIT-licensed fivegrok-parser metadata table.

use clap::ValueEnum;

#[derive(Debug, Clone, Copy, ValueEnum)]
pub enum CaptureProfile {
    /// RRC and NAS signalling with modest log volume.
    Signaling,
    /// Serving/neighbor radio measurements, PHY and scheduler/MAC visibility.
    Radio,
    /// Union of signalling and radio profiles.
    Full,
}

const SIGNALING: &[u16] = &[
    // NR RRC
    0xB821, 0xB822, 0xB823, 0xB825, 0xB826,
    // NR NAS / 5GMM / 5GSM
    0xB0C0, 0xB0C1, 0xB0C2, 0xB0C3, 0xB0C4, 0xB0C5, 0xB0C6, 0xB0CD, 0xB0CF,
    // LTE NAS
    0xB0E0, 0xB0E2, 0xB0EA, 0xB0EB, 0xB0EC,
    // Legacy LTE/NR RRC forms supported by the parser
    0x11EB, 0x184C, 0x1849, 0x1D0B, 0x1850,
];

const RADIO: &[u16] = &[
    // NR PHY
    0xB800, 0xB801, 0xB802, 0xB803, 0xB804, 0xB805,
    // NR ML1 / measurements / beam management
    0xB880, 0xB884, 0xB886, 0xB887, 0xB88A, 0xB8DA,
    // NR MAC / scheduler
    0xB890, 0xB891, 0xB893, 0xB894, 0xB895,
    // NR PDCP/RLC statistics
    0xB840, 0xB841, 0xB850, 0xB851,
    // LTE ML1
    0xB110, 0xB113, 0xB17F, 0xB193, 0xB197,
    // LTE MAC
    0xB060, 0xB063, 0xB064,
    // LTE PDCP/RLC
    0xB080, 0xB082, 0xB083, 0xB091, 0xB092,
    // Legacy radio forms
    0x1874, 0x18F7, 0x14D8, 0x12E8, 0x1C6E, 0x1C6F, 0x1C70, 0x1C71, 0x1C72,
];

impl CaptureProfile {
    pub fn codes(self) -> Vec<u16> {
        match self {
            Self::Signaling => SIGNALING.to_vec(),
            Self::Radio => RADIO.to_vec(),
            Self::Full => {
                let mut codes = SIGNALING.to_vec();
                codes.extend_from_slice(RADIO);
                codes.sort_unstable();
                codes.dedup();
                codes
            }
        }
    }
}

pub fn merge_profiles(explicit: &[u16], profiles: &[CaptureProfile]) -> Vec<u16> {
    let mut codes = explicit.to_vec();
    for profile in profiles {
        codes.extend(profile.codes());
    }
    codes.sort_unstable();
    codes.dedup();
    codes
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn full_is_union_without_duplicates() {
        let full = CaptureProfile::Full.codes();
        assert!(full.contains(&0xB821));
        assert!(full.contains(&0xB887));
        assert_eq!(full.iter().copied().collect::<std::collections::BTreeSet<_>>().len(), full.len());
    }

    #[test]
    fn explicit_codes_merge_with_profiles() {
        let merged = merge_profiles(&[0x0098, 0xB821], &[CaptureProfile::Signaling]);
        assert!(merged.contains(&0x0098));
        assert_eq!(merged.iter().filter(|&&v| v == 0xB821).count(), 1);
    }
}
