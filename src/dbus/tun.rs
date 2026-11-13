//! NetworkManager TUN/TAP Device Proxy

use zbus::Result;
use zbus::proxy;

/// Proxy for TUN/TAP devices.
#[proxy(
    interface = "org.freedesktop.NetworkManager.Device.Tun",
    default_service = "org.freedesktop.NetworkManager"
)]
pub trait NMTun {
    /// Tunnel mode, either `"tun"` or `"tap"`.
    #[zbus(property)]
    fn mode(&self) -> Result<String>;
}
