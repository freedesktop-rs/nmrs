//! NetworkManager Wired (Ethernet) Device Proxy

use zbus::Result;
use zbus::proxy;

/// Proxy for wired devices (Ethernet).
///
/// Provides access to wired-specific properties like carrier status.
#[proxy(
    interface = "org.freedesktop.NetworkManager.Device.Wired",
    default_service = "org.freedesktop.NetworkManager"
)]
pub trait NMWired {
    /// Design speed of the device, in megabits/second (Mb/s).
    #[zbus(property)]
    fn speed(&self) -> Result<u32>;

    /// Permanent hardware (MAC) address of the device.
    #[zbus(property, name = "PermHwAddress")]
    fn perm_hw_address(&self) -> Result<String>;
}
