//! NetworkManager Wireless Device proxy.

use std::collections::HashMap;
use zbus::{Result, proxy};
use zvariant::OwnedObjectPath;

/// Proxy for wireless device interface.
///
/// Extends the base device interface with Wi-Fi specific functionality
/// like scanning and access point enumeration.
#[proxy(
    interface = "org.freedesktop.NetworkManager.Device.Wireless",
    default_service = "org.freedesktop.NetworkManager"
)]
pub trait NMWireless {
    /// Requests a Wi-Fi scan. Options are usually empty.
    fn request_scan(&self, options: HashMap<String, zvariant::Value<'_>>) -> Result<()>;

    /// Signal emitted when a new access point is discovered.
    #[zbus(signal)]
    fn access_point_added(&self, path: OwnedObjectPath);

    /// Signal emitted when an access point is no longer visible.
    #[zbus(signal)]
    fn access_point_removed(&self, path: OwnedObjectPath);

    /// The operating mode of the wireless device
    #[zbus(property)]
    fn mode(&self) -> Result<u32>;

    /// Permanent hardware (MAC) address of the device.
    #[zbus(property, name = "PermHwAddress")]
    fn perm_hw_address(&self) -> Result<String>;

    /// Current connection bitrate in Kbit/s.
    #[zbus(property)]
    fn bitrate(&self) -> Result<u32>;

    /// List of object paths of access point visible to this wireless device.
    #[zbus(property)]
    fn access_points(&self) -> Result<Vec<OwnedObjectPath>>;

    /// Path to the currently connected access point ("/" if none).
    #[zbus(property)]
    fn active_access_point(&self) -> Result<OwnedObjectPath>;

    /// The capabilities of the wireless device.
    #[zbus(property)]
    fn wireless_capabilities(&self) -> Result<u32>;

    /// The timestamp (in CLOCK_BOOTTIME milliseconds) for the last finished network scan.
    /// A value of -1 means the device never scanned for access points.
    #[zbus(property)]
    fn last_scan(&self) -> Result<i64>;
}
