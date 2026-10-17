//! Refresh-oriented network event models.

use std::pin::Pin;

use futures::Stream;
use zvariant::OwnedObjectPath;

/// Stream of high-level network refresh events.
pub type NetworkEventStream =
    Pin<Box<dyn Stream<Item = crate::Result<NetworkEvent>> + Send + 'static>>;

/// Stream of saved-connection settings changes.
pub type SettingsEventStream =
    Pin<Box<dyn Stream<Item = crate::Result<SettingsChange>> + Send + 'static>>;

/// A high-level network event suitable for GUI refresh loops.
///
/// This enum is intentionally lossy: each variant is a signal that some part
/// of NetworkManager state changed and the caller should refresh its snapshot.
/// It is not an audit log and does not guarantee every transient event is
/// delivered.
#[non_exhaustive]
#[derive(Debug, Clone)]
pub enum NetworkEvent {
    /// Visible access points or their signal strengths changed.
    AccessPointsChanged,
    /// A device was added, removed, or changed state.
    DeviceChanged {
        /// Interface name when it was available at subscription time.
        interface: Option<String>,
    },
    /// NetworkManager's active connection list changed.
    ActiveConnectionsChanged,
    /// Wi-Fi enabled or Wi-Fi hardware-enabled state changed.
    WirelessEnabledChanged,
    /// Saved connection settings changed.
    SettingsChanged(SettingsChange),
    /// NetworkManager's connectivity state changed.
    ConnectivityChanged,
    /// The `org.freedesktop.NetworkManager` D-Bus owner changed.
    NetworkManagerRestarted,
}

/// A saved-connection settings change.
#[non_exhaustive]
#[derive(Debug, Clone)]
pub enum SettingsChange {
    /// A saved connection was added.
    Added {
        /// D-Bus path of the settings connection.
        path: OwnedObjectPath,
    },
    /// A saved connection was removed.
    Removed {
        /// D-Bus path of the settings connection.
        path: OwnedObjectPath,
    },
    /// A saved connection was updated.
    Updated {
        /// D-Bus path of the settings connection.
        path: OwnedObjectPath,
    },
    /// Saved connections were reloaded.
    Reloaded,
    /// A settings signal was received but could not be classified.
    Unknown,
}
