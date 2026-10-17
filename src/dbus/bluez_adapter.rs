//! BlueZ Adapter1 proxy for reading and controlling Bluetooth radio power.

use zbus::proxy;

/// Proxy for `org.bluez.Adapter1` on a specific adapter path (e.g. `/org/bluez/hci0`).
///
/// Used to read and toggle the adapter's `Powered` property, which controls
/// whether the Bluetooth radio is software-enabled.
#[proxy(interface = "org.bluez.Adapter1", default_service = "org.bluez")]
pub trait BluezAdapter {
    /// Whether the adapter is currently powered on (software-enabled).
    #[zbus(property)]
    fn powered(&self) -> zbus::Result<bool>;

    /// Enable or disable the adapter.
    #[zbus(property)]
    fn set_powered(&self, value: bool) -> zbus::Result<()>;
}
