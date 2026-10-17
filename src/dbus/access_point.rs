//! NetworkManager Access Point proxy.

use zbus::{Result, proxy};

/// Proxy for access point interface.
///
/// Provides information about a visible Wi-Fi network including
/// SSID, signal strength, security capabilities, and frequency.
#[proxy(
    interface = "org.freedesktop.NetworkManager.AccessPoint",
    default_service = "org.freedesktop.NetworkManager"
)]
pub trait NMAccessPoint {
    /// SSID as raw bytes (may not be valid UTF-8).
    #[zbus(property)]
    fn ssid(&self) -> Result<Vec<u8>>;

    /// Signal strength as percentage (0-100).
    #[zbus(property)]
    fn strength(&self) -> Result<u8>;

    /// BSSID (MAC address) of the access point.
    #[zbus(property)]
    fn hw_address(&self) -> Result<String>;

    /// General capability flags (bit 0 = privacy/WEP).
    #[zbus(property)]
    fn flags(&self) -> Result<u32>;

    /// WPA security flags (PSK, EAP, etc.).
    #[zbus(property)]
    fn wpa_flags(&self) -> Result<u32>;

    /// RSN/WPA2 security flags.
    #[zbus(property)]
    fn rsn_flags(&self) -> Result<u32>;

    /// Operating frequency in MHz.
    #[zbus(property)]
    fn frequency(&self) -> Result<u32>;

    /// Maximum supported bitrate in Kbit/s.
    #[zbus(property)]
    fn max_bitrate(&self) -> Result<u32>;

    /// Wi-Fi mode (1 = adhoc, 2 = infrastructure, 3 = AP, 4 = mesh).
    #[zbus(property)]
    fn mode(&self) -> Result<u32>;

    /// Monotonic seconds since boot when this AP was last seen, or -1 if never.
    #[zbus(property)]
    fn last_seen(&self) -> Result<i32>;
}
