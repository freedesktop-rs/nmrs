//! NetworkManager Settings.Connection D-Bus proxy.

use std::collections::HashMap;
use zbus::proxy;
use zvariant::OwnedValue;

/// Proxy for `org.freedesktop.NetworkManager.Settings.Connection` instances.
#[proxy(
    interface = "org.freedesktop.NetworkManager.Settings.Connection",
    default_service = "org.freedesktop.NetworkManager"
)]
pub trait NMSettingsConnection {
    /// Full connection settings (`a{sa{sv}}`), excluding secrets.
    fn get_settings(&self) -> zbus::Result<HashMap<String, HashMap<String, OwnedValue>>>;

    /// Replaces this profile with the provided complete settings map.
    fn update(&self, settings: HashMap<String, HashMap<String, OwnedValue>>) -> zbus::Result<()>;

    /// Like [`update`](Self::update) for in-memory (unsaved) profiles.
    #[zbus(name = "UpdateUnsaved")]
    fn update_unsaved(
        &self,
        settings: HashMap<String, HashMap<String, OwnedValue>>,
    ) -> zbus::Result<()>;

    /// Deletes this saved connection.
    fn delete(&self) -> zbus::Result<()>;

    /// `true` if the profile exists only in memory.
    #[zbus(property)]
    fn unsaved(&self) -> zbus::Result<bool>;

    /// On-disk path, or `""` if none.
    #[zbus(property)]
    fn filename(&self) -> zbus::Result<String>;

    /// Connection flags bitmask.
    #[zbus(property)]
    fn flags(&self) -> zbus::Result<u32>;
}
