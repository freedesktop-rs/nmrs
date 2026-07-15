//! Per-AP model preserving BSSID and per-device state.
//!
//! [`AccessPoint`] represents a single Wi-Fi access point seen by a specific
//! wireless device, preserving the BSSID and all NM-reported properties.
//! Use [`list_access_points`](crate::NetworkManager::list_access_points) to
//! enumerate them; use [`list_networks`](crate::NetworkManager::list_networks)
//! for the deduplicated SSID-grouped view.

use std::fmt;

use serde::{Deserialize, Serialize};
use zvariant::OwnedObjectPath;

use super::DeviceState;

/// A single Wi-Fi access point reported by NetworkManager.
///
/// Unlike [`Network`](super::Network), which groups APs sharing an SSID,
/// each `AccessPoint` corresponds to one BSSID and carries per-device state.
#[non_exhaustive]
#[derive(Debug, Clone, PartialEq)]
pub struct AccessPoint {
    /// D-Bus path of this access point object.
    pub path: OwnedObjectPath,
    /// D-Bus path of the wireless device that sees this AP.
    pub device_path: OwnedObjectPath,
    /// Interface name of the device (e.g. `"wlan0"`).
    pub interface: String,
    /// SSID decoded as UTF-8, or `"<hidden>"` for hidden networks.
    pub ssid: String,
    /// Raw SSID bytes for non-UTF-8 SSIDs.
    pub ssid_bytes: Vec<u8>,
    /// BSSID in `"XX:XX:XX:XX:XX:XX"` format.
    pub bssid: String,
    /// Operating frequency in MHz.
    pub frequency_mhz: u32,
    /// Maximum supported bitrate in Kbit/s.
    pub max_bitrate_kbps: u32,
    /// Signal strength percentage (0–100).
    pub strength: u8,
    /// AP operating mode.
    pub mode: ApMode,
    /// Decoded security capabilities.
    pub security: SecurityFeatures,
    /// Monotonic seconds since boot when last seen, or `None` if never.
    pub last_seen_secs: Option<i64>,
    /// `true` if this AP is the active connection on `device_path`.
    pub is_active: bool,
    /// State of the wireless device at enumeration time (not live).
    pub device_state: DeviceState,
}

impl AccessPoint {
    pub fn is_hidden(&self) -> bool {
        self.ssid_bytes.is_empty()
    }
}

/// Wi-Fi access point operating mode.
#[non_exhaustive]
#[derive(Debug, Clone, Copy, Eq, PartialEq, Serialize, Deserialize)]
pub enum ApMode {
    /// Ad-hoc (IBSS) network.
    Adhoc,
    /// Infrastructure (managed) mode — the most common.
    Infrastructure,
    /// Access point (hotspot) mode.
    Ap,
    /// Mesh mode.
    Mesh,
    /// Unknown or unrecognised NM mode value.
    Unknown(u32),
}

impl From<u32> for ApMode {
    fn from(value: u32) -> Self {
        match value {
            1 => Self::Adhoc,
            2 => Self::Infrastructure,
            3 => Self::Ap,
            4 => Self::Mesh,
            other => Self::Unknown(other),
        }
    }
}

impl fmt::Display for ApMode {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Adhoc => write!(f, "Ad-Hoc"),
            Self::Infrastructure => write!(f, "Infrastructure"),
            Self::Ap => write!(f, "AP"),
            Self::Mesh => write!(f, "Mesh"),
            Self::Unknown(v) => write!(f, "Unknown({v})"),
        }
    }
}

/// Decoded security capabilities of an access point.
///
/// Derived from NetworkManager's `Flags`, `WpaFlags`, and `RsnFlags` properties
/// using the `NM80211ApFlags` and `NM80211ApSecurityFlags` bitmask values:
///
/// | Flag constant | Value | Field(s) |
/// |---|---|---|
/// | `NM_802_11_AP_FLAGS_PRIVACY` | `0x1` | `privacy` |
/// | `NM_802_11_AP_FLAGS_WPS` | `0x2` | `wps` |
/// | `PAIR_WEP40` | `0x1` | `wep40` |
/// | `PAIR_WEP104` | `0x2` | `wep104` |
/// | `PAIR_TKIP` | `0x4` | `tkip` |
/// | `PAIR_CCMP` | `0x8` | `ccmp` |
/// | `KEY_MGMT_PSK` | `0x100` | `psk` |
/// | `KEY_MGMT_802_1X` | `0x200` | `eap` |
/// | `KEY_MGMT_SAE` | `0x400` | `sae` |
/// | `KEY_MGMT_OWE` | `0x800` | `owe` |
/// | `KEY_MGMT_OWE_TM` | `0x1000` | `owe_transition_mode` |
/// | `KEY_MGMT_EAP_SUITE_B_192` | `0x2000` | `eap_suite_b_192` |
#[non_exhaustive]
#[derive(Debug, Clone, Copy, Eq, PartialEq, Default, Serialize, Deserialize)]
pub struct SecurityFeatures {
    /// AP advertises privacy (WEP or higher).
    pub privacy: bool,
    /// WPS (Wi-Fi Protected Setup) is available.
    pub wps: bool,

    /// Pre-shared key authentication (WPA/WPA2-Personal).
    pub psk: bool,
    /// 802.1X / EAP authentication (WPA/WPA2-Enterprise).
    pub eap: bool,
    /// Simultaneous Authentication of Equals (WPA3-Personal).
    pub sae: bool,
    /// Opportunistic Wireless Encryption.
    pub owe: bool,
    /// OWE transition mode (mixed open + OWE).
    pub owe_transition_mode: bool,
    /// EAP Suite B 192-bit (WPA3-Enterprise 192-bit).
    pub eap_suite_b_192: bool,

    /// Pairwise WEP-40 cipher.
    pub wep40: bool,
    /// Pairwise WEP-104 cipher.
    pub wep104: bool,
    /// TKIP cipher.
    pub tkip: bool,
    /// CCMP (AES) cipher.
    pub ccmp: bool,
}

impl SecurityFeatures {
    /// Returns `true` if no security mechanism is advertised.
    #[must_use]
    pub fn is_open(&self) -> bool {
        !self.privacy
            && !self.psk
            && !self.eap
            && !self.sae
            && !self.owe
            && !self.owe_transition_mode
            && !self.eap_suite_b_192
            && !self.wep40
            && !self.wep104
    }

    /// Returns `true` if enterprise authentication is available.
    #[must_use]
    pub fn is_enterprise(&self) -> bool {
        self.eap || self.eap_suite_b_192
    }

    /// Returns `true` if WPA3 (SAE or OWE) is available.
    #[must_use]
    pub fn is_wpa3(&self) -> bool {
        self.sae || self.owe
    }

    /// Returns the preferred connection type for this security profile.
    #[must_use]
    pub fn preferred_connect_type(&self) -> ConnectType {
        if self.eap || self.eap_suite_b_192 {
            ConnectType::Eap
        } else if self.sae {
            ConnectType::Sae
        } else if self.owe || self.owe_transition_mode {
            ConnectType::Owe
        } else if self.psk {
            ConnectType::Psk
        } else {
            ConnectType::Open
        }
    }
}

/// Preferred connection type derived from [`SecurityFeatures`].
#[non_exhaustive]
#[derive(Debug, Clone, Copy, Eq, PartialEq, Serialize, Deserialize)]
pub enum ConnectType {
    /// Open (no authentication).
    Open,
    /// Pre-shared key (WPA/WPA2-Personal).
    Psk,
    /// SAE (WPA3-Personal).
    Sae,
    /// 802.1X / EAP (Enterprise).
    Eap,
    /// Opportunistic Wireless Encryption.
    Owe,
}

impl fmt::Display for ConnectType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Open => write!(f, "Open"),
            Self::Psk => write!(f, "PSK"),
            Self::Sae => write!(f, "SAE"),
            Self::Eap => write!(f, "EAP"),
            Self::Owe => write!(f, "OWE"),
        }
    }
}

// NM80211ApFlags
const AP_FLAGS_PRIVACY: u32 = 0x1;
const AP_FLAGS_WPS: u32 = 0x2;

// NM80211ApSecurityFlags (applied to both WpaFlags and RsnFlags)
const SEC_PAIR_WEP40: u32 = 0x1;
const SEC_PAIR_WEP104: u32 = 0x2;
const SEC_PAIR_TKIP: u32 = 0x4;
const SEC_PAIR_CCMP: u32 = 0x8;
const SEC_KEY_MGMT_PSK: u32 = 0x100;
const SEC_KEY_MGMT_802_1X: u32 = 0x200;
const SEC_KEY_MGMT_SAE: u32 = 0x400;
const SEC_KEY_MGMT_OWE: u32 = 0x800;
const SEC_KEY_MGMT_OWE_TM: u32 = 0x1000;
const SEC_KEY_MGMT_EAP_SUITE_B_192: u32 = 0x2000;

/// Decodes NM's AP flag triplet into a [`SecurityFeatures`].
///
/// `flags` is `NM80211ApFlags`, `wpa` and `rsn` are `NM80211ApSecurityFlags`
/// from the `WpaFlags` and `RsnFlags` AP properties respectively.
pub(crate) fn decode_security(flags: u32, wpa: u32, rsn: u32) -> SecurityFeatures {
    let combined = wpa | rsn;
    SecurityFeatures {
        privacy: (flags & AP_FLAGS_PRIVACY) != 0,
        wps: (flags & AP_FLAGS_WPS) != 0,
        psk: (combined & SEC_KEY_MGMT_PSK) != 0,
        eap: (combined & SEC_KEY_MGMT_802_1X) != 0,
        sae: (combined & SEC_KEY_MGMT_SAE) != 0,
        owe: (combined & SEC_KEY_MGMT_OWE) != 0,
        owe_transition_mode: (combined & SEC_KEY_MGMT_OWE_TM) != 0,
        eap_suite_b_192: (combined & SEC_KEY_MGMT_EAP_SUITE_B_192) != 0,
        wep40: (combined & SEC_PAIR_WEP40) != 0,
        wep104: (combined & SEC_PAIR_WEP104) != 0,
        tkip: (combined & SEC_PAIR_TKIP) != 0,
        ccmp: (combined & SEC_PAIR_CCMP) != 0,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn decode_open_network() {
        let sec = decode_security(0, 0, 0);
        assert!(sec.is_open());
        assert!(!sec.is_enterprise());
        assert!(!sec.is_wpa3());
        assert_eq!(sec.preferred_connect_type(), ConnectType::Open);
    }

    #[test]
    fn decode_wep40() {
        let sec = decode_security(AP_FLAGS_PRIVACY, SEC_PAIR_WEP40, 0);
        assert!(!sec.is_open());
        assert!(sec.privacy);
        assert!(sec.wep40);
        assert_eq!(sec.preferred_connect_type(), ConnectType::Open);
    }

    #[test]
    fn decode_wep104() {
        let sec = decode_security(AP_FLAGS_PRIVACY, SEC_PAIR_WEP104, 0);
        assert!(sec.wep104);
        assert!(sec.privacy);
    }

    #[test]
    fn decode_wpa_tkip_psk() {
        let sec = decode_security(AP_FLAGS_PRIVACY, SEC_PAIR_TKIP | SEC_KEY_MGMT_PSK, 0);
        assert!(sec.psk);
        assert!(sec.tkip);
        assert!(!sec.ccmp);
        assert_eq!(sec.preferred_connect_type(), ConnectType::Psk);
    }

    #[test]
    fn decode_wpa2_ccmp_psk() {
        let sec = decode_security(AP_FLAGS_PRIVACY, 0, SEC_PAIR_CCMP | SEC_KEY_MGMT_PSK);
        assert!(sec.psk);
        assert!(sec.ccmp);
        assert!(!sec.tkip);
        assert_eq!(sec.preferred_connect_type(), ConnectType::Psk);
    }

    #[test]
    fn decode_wpa2_enterprise() {
        let sec = decode_security(AP_FLAGS_PRIVACY, 0, SEC_PAIR_CCMP | SEC_KEY_MGMT_802_1X);
        assert!(sec.eap);
        assert!(sec.ccmp);
        assert!(sec.is_enterprise());
        assert_eq!(sec.preferred_connect_type(), ConnectType::Eap);
    }

    #[test]
    fn decode_wpa3_sae() {
        let sec = decode_security(AP_FLAGS_PRIVACY, 0, SEC_PAIR_CCMP | SEC_KEY_MGMT_SAE);
        assert!(sec.sae);
        assert!(sec.ccmp);
        assert!(sec.is_wpa3());
        assert_eq!(sec.preferred_connect_type(), ConnectType::Sae);
    }

    #[test]
    fn decode_owe() {
        let sec = decode_security(0, 0, SEC_PAIR_CCMP | SEC_KEY_MGMT_OWE);
        assert!(sec.owe);
        assert!(sec.is_wpa3());
        assert_eq!(sec.preferred_connect_type(), ConnectType::Owe);
    }

    #[test]
    fn decode_owe_transition() {
        let sec = decode_security(0, 0, SEC_KEY_MGMT_OWE_TM);
        assert!(sec.owe_transition_mode);
        assert_eq!(sec.preferred_connect_type(), ConnectType::Owe);
    }

    #[test]
    fn decode_eap_suite_b_192() {
        let sec = decode_security(
            AP_FLAGS_PRIVACY,
            0,
            SEC_PAIR_CCMP | SEC_KEY_MGMT_EAP_SUITE_B_192,
        );
        assert!(sec.eap_suite_b_192);
        assert!(sec.is_enterprise());
        assert_eq!(sec.preferred_connect_type(), ConnectType::Eap);
    }

    #[test]
    fn decode_wps_flag() {
        let sec = decode_security(AP_FLAGS_WPS, 0, 0);
        assert!(sec.wps);
    }

    #[test]
    fn decode_mixed_wpa_wpa2() {
        let sec = decode_security(
            AP_FLAGS_PRIVACY,
            SEC_PAIR_TKIP | SEC_KEY_MGMT_PSK,
            SEC_PAIR_CCMP | SEC_KEY_MGMT_PSK,
        );
        assert!(sec.psk);
        assert!(sec.tkip);
        assert!(sec.ccmp);
    }

    #[test]
    fn ap_mode_from_u32() {
        assert_eq!(ApMode::from(1), ApMode::Adhoc);
        assert_eq!(ApMode::from(2), ApMode::Infrastructure);
        assert_eq!(ApMode::from(3), ApMode::Ap);
        assert_eq!(ApMode::from(4), ApMode::Mesh);
        assert_eq!(ApMode::from(99), ApMode::Unknown(99));
    }

    #[test]
    fn ap_mode_display() {
        assert_eq!(ApMode::Adhoc.to_string(), "Ad-Hoc");
        assert_eq!(ApMode::Infrastructure.to_string(), "Infrastructure");
        assert_eq!(ApMode::Ap.to_string(), "AP");
        assert_eq!(ApMode::Mesh.to_string(), "Mesh");
        assert_eq!(ApMode::Unknown(42).to_string(), "Unknown(42)");
    }

    #[test]
    fn connect_type_display() {
        assert_eq!(ConnectType::Open.to_string(), "Open");
        assert_eq!(ConnectType::Psk.to_string(), "PSK");
        assert_eq!(ConnectType::Sae.to_string(), "SAE");
        assert_eq!(ConnectType::Eap.to_string(), "EAP");
        assert_eq!(ConnectType::Owe.to_string(), "OWE");
    }

    #[test]
    fn security_features_default_is_open() {
        let sec = SecurityFeatures::default();
        assert!(sec.is_open());
    }

    #[test]
    fn eap_prioritized_over_psk() {
        let sec = SecurityFeatures {
            psk: true,
            eap: true,
            ccmp: true,
            privacy: true,
            ..Default::default()
        };
        assert_eq!(sec.preferred_connect_type(), ConnectType::Eap);
    }

    #[test]
    fn sae_prioritized_over_psk() {
        let sec = SecurityFeatures {
            psk: true,
            sae: true,
            ccmp: true,
            privacy: true,
            ..Default::default()
        };
        assert_eq!(sec.preferred_connect_type(), ConnectType::Sae);
    }
}
