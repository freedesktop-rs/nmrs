//! NetworkManager Team Device Proxy

use zbus::Result;
use zbus::proxy;

/// Proxy for team devices.
///
/// Port devices come from `Device.Ports` on `NMDeviceProxy`.
#[proxy(
    interface = "org.freedesktop.NetworkManager.Device.Team",
    default_service = "org.freedesktop.NetworkManager"
)]
pub trait NMTeam {
    /// JSON configuration currently applied on the device.
    #[zbus(property)]
    fn config(&self) -> Result<String>;
}
