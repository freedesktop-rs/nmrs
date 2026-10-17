//! Main NetworkManager proxy.

use std::collections::HashMap;
use zbus::proxy;
use zvariant::OwnedObjectPath;

/// Proxy for the main NetworkManager interface.
///
/// Provides methods for listing devices, managing connections,
/// and controlling Wi-Fi state.
#[proxy(
    interface = "org.freedesktop.NetworkManager",
    default_service = "org.freedesktop.NetworkManager",
    default_path = "/org/freedesktop/NetworkManager"
)]
pub trait NM {
    /// Returns paths to all network devices.
    fn get_devices(&self) -> zbus::Result<Vec<OwnedObjectPath>>;

    /// Whether Wi-Fi is globally enabled.
    #[zbus(property)]
    fn wireless_enabled(&self) -> zbus::Result<bool>;

    /// Enable or disable Wi-Fi globally.
    #[zbus(property)]
    fn set_wireless_enabled(&self, value: bool) -> zbus::Result<()>;

    /// Whether wireless hardware is enabled.
    #[zbus(property)]
    fn wireless_hardware_enabled(&self) -> zbus::Result<bool>;

    /// Paths to all active connections.
    #[zbus(property)]
    fn active_connections(&self) -> zbus::Result<Vec<OwnedObjectPath>>;

    /// Creates a new connection and activates it simultaneously.
    ///
    /// Returns paths to both the new connection settings and active connection.
    fn add_and_activate_connection(
        &self,
        connection: HashMap<&str, HashMap<&str, zvariant::Value<'_>>>,
        device: OwnedObjectPath,
        specific_object: OwnedObjectPath,
    ) -> zbus::Result<(OwnedObjectPath, OwnedObjectPath)>;

    /// Activates an existing saved connection.
    fn activate_connection(
        &self,
        connection: OwnedObjectPath,
        device: OwnedObjectPath,
        specific_object: OwnedObjectPath,
    ) -> zbus::Result<OwnedObjectPath>;

    /// Deactivates an active connection.
    fn deactivate_connection(&self, active_connection: OwnedObjectPath) -> zbus::Result<()>;

    /// Signal emitted when a device is added to NetworkManager.
    #[zbus(signal, name = "DeviceAdded")]
    fn device_added(&self, device: OwnedObjectPath);

    /// Signal emitted when a device is removed from NetworkManager.
    #[zbus(signal, name = "DeviceRemoved")]
    fn device_removed(&self, device: OwnedObjectPath);

    /// Whether WWAN (mobile broadband) is globally enabled.
    #[zbus(property)]
    fn wwan_enabled(&self) -> zbus::Result<bool>;

    /// Enable or disable WWAN globally.
    #[zbus(property)]
    fn set_wwan_enabled(&self, value: bool) -> zbus::Result<()>;

    /// Whether WWAN hardware is enabled (rfkill state).
    #[zbus(property)]
    fn wwan_hardware_enabled(&self) -> zbus::Result<bool>;

    /// Signal emitted when any device changes state.
    #[zbus(signal, name = "StateChanged")]
    fn state_changed(&self, state: u32);

    /// Current connectivity state (`0`–`4`).
    #[zbus(property)]
    fn connectivity(&self) -> zbus::Result<u32>;

    /// Whether NM is allowed to probe for connectivity.
    #[zbus(property)]
    fn connectivity_check_enabled(&self) -> zbus::Result<bool>;

    /// URL NM probes when checking connectivity.
    #[zbus(property)]
    fn connectivity_check_uri(&self) -> zbus::Result<String>;

    /// Primary active connection path (`/` when none).
    #[zbus(property)]
    fn primary_connection(&self) -> zbus::Result<OwnedObjectPath>;

    /// Forces a fresh connectivity check; blocks until done.
    fn check_connectivity(&self) -> zbus::Result<u32>;

    /// Global DNS override (`a{sv}`). Empty dict clears it.
    #[zbus(property)]
    fn global_dns_configuration(
        &self,
    ) -> zbus::Result<std::collections::HashMap<String, zvariant::OwnedValue>>;

    /// Write the global DNS override.
    #[zbus(property)]
    fn set_global_dns_configuration(
        &self,
        value: std::collections::HashMap<&str, zvariant::Value<'_>>,
    ) -> zbus::Result<()>;
}
