//! Typed active connection models.

use super::ActiveConnectionState;

/// A typed active NetworkManager connection.
#[non_exhaustive]
#[derive(Debug, Clone)]
pub enum ActiveConnection {
    /// Active wired Ethernet connection.
    Wired(ActiveWiredConnection),
    /// Active Wi-Fi connection.
    Wifi(ActiveWifiConnection),
    /// Active VPN connection.
    Vpn(ActiveVpnConnection),
    /// Active connection type not modeled by the higher-level variants.
    Other(ActiveOtherConnection),
}

/// An active wired Ethernet connection.
#[non_exhaustive]
#[derive(Debug, Clone)]
pub struct ActiveWiredConnection {
    /// Human-visible connection id.
    pub id: String,
    /// Connection UUID.
    pub uuid: String,
    /// Interface name, if NetworkManager exposes the device.
    pub interface: Option<String>,
    /// Current hardware address, if exposed.
    pub hw_address: Option<String>,
    /// Raw Ethernet link speed in megabits per second, if exposed.
    pub speed_mbps: Option<u32>,
    /// Assigned IPv4 address with CIDR notation, if connected.
    pub ip4_address: Option<String>,
    /// Assigned IPv6 address with CIDR notation, if connected.
    pub ip6_address: Option<String>,
    /// Active connection state.
    pub state: ActiveConnectionState,
}

/// An active Wi-Fi connection.
#[non_exhaustive]
#[derive(Debug, Clone)]
pub struct ActiveWifiConnection {
    /// Human-visible connection id.
    pub id: String,
    /// Connection UUID.
    pub uuid: String,
    /// SSID of the active access point, or the connection id when unavailable.
    pub ssid: String,
    /// Interface name, if NetworkManager exposes the device.
    pub interface: Option<String>,
    /// BSSID of the active access point, if known.
    pub bssid: Option<String>,
    /// Signal strength percentage, if known.
    pub strength: Option<u8>,
    /// Assigned IPv4 address with CIDR notation, if connected.
    pub ip4_address: Option<String>,
    /// Assigned IPv6 address with CIDR notation, if connected.
    pub ip6_address: Option<String>,
    /// Active connection state.
    pub state: ActiveConnectionState,
}

/// An active VPN connection.
#[non_exhaustive]
#[derive(Debug, Clone)]
pub struct ActiveVpnConnection {
    /// Human-visible connection id.
    pub id: String,
    /// Connection UUID.
    pub uuid: String,
    /// Interface name, if NetworkManager exposes a VPN device.
    pub interface: Option<String>,
    /// Assigned IPv4 address with CIDR notation, if connected.
    pub ip4_address: Option<String>,
    /// Assigned IPv6 address with CIDR notation, if connected.
    pub ip6_address: Option<String>,
    /// Active connection state.
    pub state: ActiveConnectionState,
}

/// An active connection not covered by the typed variants.
#[non_exhaustive]
#[derive(Debug, Clone)]
pub struct ActiveOtherConnection {
    /// Human-visible connection id.
    pub id: String,
    /// Connection UUID.
    pub uuid: String,
    /// NM connection type string, if it could be resolved from settings.
    pub connection_type: Option<String>,
    /// Interface name, if NetworkManager exposes an associated device.
    pub interface: Option<String>,
    /// Assigned IPv4 address with CIDR notation, if connected.
    pub ip4_address: Option<String>,
    /// Assigned IPv6 address with CIDR notation, if connected.
    pub ip6_address: Option<String>,
    /// Active connection state.
    pub state: ActiveConnectionState,
}
