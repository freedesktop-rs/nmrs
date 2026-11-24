//! NetworkManager Bond Device Proxy

use zbus::Result;
use zbus::proxy;
use zvariant::OwnedObjectPath;

/// Proxy for bond devices.
///
/// Port devices also come from `Device.Ports` on `NMDeviceProxy`.
#[proxy(
    interface = "org.freedesktop.NetworkManager.Device.Bond",
    default_service = "org.freedesktop.NetworkManager"
)]
pub trait NMBond {
    /// Hardware address of the bond device.
    #[zbus(property)]
    fn hw_address(&self) -> Result<String>;

    /// Whether the bond has carrier.
    #[zbus(property)]
    fn carrier(&self) -> Result<bool>;

    /// Object paths of the bond's port devices (`Slaves`).
    #[zbus(property)]
    fn slaves(&self) -> Result<Vec<OwnedObjectPath>>;
}
