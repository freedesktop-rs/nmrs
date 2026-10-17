//! VPN connection types and configuration traits.
//!
//! `nmrs` treats both NM plugin-based VPNs (`connection.type = "vpn"`) and
//! kernel-level WireGuard tunnels (`connection.type = "wireguard"`) as VPN
//! connections. [`VpnKind`] distinguishes the two, while [`VpnType`] carries
//! protocol-specific metadata decoded from NM settings.

use std::{collections::HashMap, fmt};

use super::device::DeviceState;
use super::openvpn::OpenVpnConfig;
use super::saved_connection::VpnSecretFlags;
use super::wireguard::WireGuardConfig;
use super::{Redacted, redact_option};
use uuid::Uuid;

pub(crate) mod sealed {
    pub trait Sealed {}
}

/// Whether a VPN connection is a NM-plugin VPN or kernel WireGuard.
#[non_exhaustive]
#[derive(Debug, Clone, Copy, Eq, PartialEq, Hash)]
pub enum VpnKind {
    /// NM VPN plugin (OpenVPN, strongSwan, OpenConnect, PPTP, L2TP, …).
    Plugin,
    /// Kernel-level WireGuard tunnel.
    WireGuard,
}

/// Saved VPN profile summary for applet lists.
#[non_exhaustive]
#[derive(Debug, Clone)]
pub struct SavedVpnSummary {
    /// Connection UUID.
    pub uuid: String,
    /// Human-visible connection id.
    pub id: String,
    /// VPN implementation kind, when it can be inferred from saved settings.
    pub kind: Option<VpnKind>,
    /// `true` when an active VPN connection has the same UUID.
    pub active: bool,
}

/// OpenVPN authentication/connection type.
#[non_exhaustive]
#[derive(Debug, Clone, Copy, Eq, PartialEq, Hash)]
pub enum OpenVpnConnectionType {
    /// Pure TLS certificate authentication.
    Tls,
    /// Static pre-shared key.
    StaticKey,
    /// Username/password only.
    Password,
    /// Username/password + TLS certificate.
    PasswordTls,
}

impl OpenVpnConnectionType {
    /// Parse from NM's `data.connection-type` string.
    #[must_use]
    pub fn from_nm_str(s: &str) -> Option<Self> {
        match s {
            "tls" => Some(Self::Tls),
            "static-key" => Some(Self::StaticKey),
            "password" => Some(Self::Password),
            "password-tls" => Some(Self::PasswordTls),
            _ => None,
        }
    }
}

/// Protocol-specific VPN metadata decoded from NM saved settings.
///
/// Returned by [`VpnConnection::vpn_type`] to describe a saved VPN profile.
/// Each variant carries the fields an applet typically needs to render a VPN
/// list entry.
#[non_exhaustive]
#[derive(Clone, PartialEq)]
pub enum VpnType {
    /// Kernel WireGuard tunnel.
    WireGuard {
        /// Interface private key (often agent-owned and absent).
        private_key: Option<String>,
        /// First peer's public key.
        peer_public_key: Option<String>,
        /// First peer's `endpoint` (e.g. `"vpn.example.com:51820"`).
        endpoint: Option<String>,
        /// First peer's allowed-ips list.
        allowed_ips: Vec<String>,
        /// First peer's persistent keepalive (seconds).
        persistent_keepalive: Option<u32>,
    },
    /// OpenVPN (NM plugin `org.freedesktop.NetworkManager.openvpn`).
    OpenVpn {
        /// Remote server address.
        remote: Option<String>,
        /// Authentication/connection type.
        connection_type: Option<OpenVpnConnectionType>,
        /// VPN-level user name.
        user_name: Option<String>,
        /// CA certificate path.
        ca: Option<String>,
        /// Client certificate path.
        cert: Option<String>,
        /// Client key path.
        key: Option<String>,
        /// TLS-auth key path.
        ta: Option<String>,
        /// Password secret flags.
        password_flags: VpnSecretFlags,
    },
    /// OpenConnect (Cisco AnyConnect / Juniper / GlobalProtect / Pulse).
    OpenConnect {
        /// Gateway hostname.
        gateway: Option<String>,
        /// VPN-level user name.
        user_name: Option<String>,
        /// Protocol variant (`"anyconnect"`, `"nc"`, `"gp"`, `"pulse"`).
        protocol: Option<String>,
        /// Password secret flags.
        password_flags: VpnSecretFlags,
    },
    /// strongSwan (IPSec/IKEv2).
    StrongSwan {
        /// Gateway address.
        address: Option<String>,
        /// Auth method (`"eap"`, `"key"`, `"agent"`, `"smartcard"`).
        method: Option<String>,
        /// VPN-level user name.
        user_name: Option<String>,
        /// Certificate path.
        certificate: Option<String>,
        /// Password secret flags.
        password_flags: VpnSecretFlags,
    },
    /// PPTP VPN.
    Pptp {
        /// Gateway hostname.
        gateway: Option<String>,
        /// VPN-level user name.
        user_name: Option<String>,
        /// Password secret flags.
        password_flags: VpnSecretFlags,
    },
    /// L2TP VPN.
    L2tp {
        /// Gateway hostname.
        gateway: Option<String>,
        /// VPN-level user name.
        user_name: Option<String>,
        /// Password secret flags.
        password_flags: VpnSecretFlags,
        /// Whether IPSec encapsulation is enabled.
        ipsec_enabled: bool,
    },
    /// Catch-all for VPN plugins nmrs doesn't model first-class.
    Generic {
        /// NM VPN plugin D-Bus service name.
        service_type: String,
        /// Raw `vpn.data` key-value pairs.
        data: HashMap<String, String>,
        /// Raw `vpn.secrets` key-value pairs (often empty without agent).
        secrets: HashMap<String, String>,
        /// VPN-level user name.
        user_name: Option<String>,
        /// Password secret flags.
        password_flags: VpnSecretFlags,
    },
}

impl fmt::Debug for VpnType {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::WireGuard {
                private_key,
                peer_public_key,
                endpoint,
                allowed_ips,
                persistent_keepalive,
            } => formatter
                .debug_struct("WireGuard")
                .field("private_key", &redact_option(private_key))
                .field("peer_public_key", peer_public_key)
                .field("endpoint", endpoint)
                .field("allowed_ips", allowed_ips)
                .field("persistent_keepalive", persistent_keepalive)
                .finish(),
            Self::OpenVpn {
                remote,
                connection_type,
                user_name,
                ca,
                cert,
                key,
                ta,
                password_flags,
            } => formatter
                .debug_struct("OpenVpn")
                .field("remote", remote)
                .field("connection_type", connection_type)
                .field("user_name", user_name)
                .field("ca", ca)
                .field("cert", cert)
                .field("key", key)
                .field("ta", ta)
                .field("password_flags", password_flags)
                .finish(),
            Self::OpenConnect {
                gateway,
                user_name,
                protocol,
                password_flags,
            } => formatter
                .debug_struct("OpenConnect")
                .field("gateway", gateway)
                .field("user_name", user_name)
                .field("protocol", protocol)
                .field("password_flags", password_flags)
                .finish(),
            Self::StrongSwan {
                address,
                method,
                user_name,
                certificate,
                password_flags,
            } => formatter
                .debug_struct("StrongSwan")
                .field("address", address)
                .field("method", method)
                .field("user_name", user_name)
                .field("certificate", certificate)
                .field("password_flags", password_flags)
                .finish(),
            Self::Pptp {
                gateway,
                user_name,
                password_flags,
            } => formatter
                .debug_struct("Pptp")
                .field("gateway", gateway)
                .field("user_name", user_name)
                .field("password_flags", password_flags)
                .finish(),
            Self::L2tp {
                gateway,
                user_name,
                password_flags,
                ipsec_enabled,
            } => formatter
                .debug_struct("L2tp")
                .field("gateway", gateway)
                .field("user_name", user_name)
                .field("password_flags", password_flags)
                .field("ipsec_enabled", ipsec_enabled)
                .finish(),
            Self::Generic {
                service_type,
                user_name,
                password_flags,
                ..
            } => formatter
                .debug_struct("Generic")
                .field("service_type", service_type)
                .field("data", &Redacted)
                .field("secrets", &Redacted)
                .field("user_name", user_name)
                .field("password_flags", password_flags)
                .finish(),
        }
    }
}

/// VPN connection configuration
///
/// Type-safe wrapper for VPN configurations that enables protocol dispatch.
#[non_exhaustive]
#[derive(Debug, Clone)]
pub enum VpnConfiguration {
    /// WireGuard VPN configuration.
    WireGuard(WireGuardConfig),
    /// OpenVPN configuration
    OpenVpn(Box<OpenVpnConfig>),
}

impl From<WireGuardConfig> for VpnConfiguration {
    fn from(config: WireGuardConfig) -> Self {
        Self::WireGuard(config)
    }
}

impl From<OpenVpnConfig> for VpnConfiguration {
    fn from(config: OpenVpnConfig) -> Self {
        Self::OpenVpn(Box::new(config))
    }
}

impl sealed::Sealed for VpnConfiguration {}

impl VpnConfig for VpnConfiguration {
    fn vpn_kind(&self) -> VpnKind {
        match self {
            Self::WireGuard(_) => VpnKind::WireGuard,
            Self::OpenVpn(_) => VpnKind::Plugin,
        }
    }

    fn name(&self) -> &str {
        match self {
            Self::WireGuard(c) => &c.name,
            Self::OpenVpn(c) => &c.name,
        }
    }

    fn dns(&self) -> Option<&[String]> {
        match self {
            Self::WireGuard(c) => c.dns.as_deref(),
            Self::OpenVpn(c) => c.dns.as_deref(),
        }
    }

    fn mtu(&self) -> Option<u32> {
        match self {
            Self::WireGuard(c) => c.mtu,
            Self::OpenVpn(c) => c.mtu,
        }
    }

    fn uuid(&self) -> Option<Uuid> {
        match self {
            Self::WireGuard(c) => c.uuid,
            Self::OpenVpn(c) => c.uuid,
        }
    }
}

/// Common metadata shared by VPN connection configurations.
///
/// This trait is sealed and cannot be implemented outside of this crate.
/// Use [`WireGuardConfig`], [`OpenVpnConfig`], or [`VpnConfiguration`] instead.
pub trait VpnConfig: sealed::Sealed + Send + Sync + std::fmt::Debug {
    /// Returns whether this is a plugin VPN or kernel WireGuard.
    fn vpn_kind(&self) -> VpnKind;

    /// Returns the connection name.
    fn name(&self) -> &str;

    /// Returns the configured DNS servers, if any.
    fn dns(&self) -> Option<&[String]>;

    /// Returns the configured MTU, if any.
    fn mtu(&self) -> Option<u32>;

    /// Returns the configured UUID, if any.
    fn uuid(&self) -> Option<Uuid>;
}

/// A saved or active VPN connection with rich metadata.
///
/// Returned by [`crate::NetworkManager::list_vpn_connections`].
///
/// # Example
///
/// ```no_run
/// # use nmrs::{VpnConnection, VpnKind};
/// # let vpn: VpnConnection = todo!();
/// println!("{} ({:?}) active={}", vpn.id, vpn.kind, vpn.active);
/// ```
#[non_exhaustive]
#[derive(Debug, Clone)]
pub struct VpnConnection {
    /// NM connection UUID.
    pub uuid: String,
    /// Connection display name (`connection.id`).
    pub id: String,
    /// Alias for `id` (backward compat).
    pub name: String,
    /// Protocol-specific decoded settings.
    pub vpn_type: VpnType,
    /// Current device/active-connection state.
    pub state: DeviceState,
    /// Network interface name when active.
    pub interface: Option<String>,
    /// Whether this VPN is currently activated.
    pub active: bool,
    /// VPN-level user name (from `vpn.user-name`).
    pub user_name: Option<String>,
    /// Password secret flags.
    pub password_flags: VpnSecretFlags,
    /// Raw NM `vpn.service-type` string (empty for WireGuard).
    pub service_type: String,
    /// Plugin-based vs kernel WireGuard.
    pub kind: VpnKind,
}

/// Protocol-specific details for an active VPN connection.
///
/// Provides configuration details extracted from the NetworkManager connection
/// profile, varying by VPN type.
#[non_exhaustive]
#[derive(Debug, Clone)]
pub enum VpnDetails {
    /// WireGuard-specific connection details.
    WireGuard {
        /// The local interface's public key.
        public_key: Option<String>,
        /// The peer endpoint (e.g. "vpn.example.com:51820").
        endpoint: Option<String>,
    },
    /// OpenVPN-specific connection details.
    OpenVpn {
        /// Remote server address (e.g. "vpn.example.com:1194").
        remote: String,
        /// Remote server port.
        port: u16,
        /// Transport protocol ("udp" or "tcp").
        protocol: String,
        /// Data channel cipher (e.g. "AES-256-GCM").
        cipher: Option<String>,
        /// HMAC digest algorithm (e.g. "SHA256").
        auth: Option<String>,
        /// Compression mode if enabled (e.g. "lz4-v2").
        compression: Option<String>,
    },
}

/// Detailed VPN connection information and statistics.
///
/// Provides comprehensive information about an active VPN connection,
/// including IP configuration and connection details.
///
/// # Example
///
/// ```no_run
/// # use nmrs::{VpnConnectionInfo, VpnKind, DeviceState};
/// # let info: VpnConnectionInfo = todo!();
/// if let Some(ip) = &info.ip4_address {
///     println!("VPN IPv4: {}", ip);
/// }
/// ```
#[non_exhaustive]
#[derive(Debug, Clone)]
pub struct VpnConnectionInfo {
    /// The connection name/identifier.
    pub name: String,
    /// Plugin vs WireGuard.
    pub vpn_kind: VpnKind,
    /// Current connection state.
    pub state: DeviceState,
    /// Network interface name when active (e.g., "wg0").
    pub interface: Option<String>,
    /// VPN gateway endpoint address.
    pub gateway: Option<String>,
    /// Assigned IPv4 address with CIDR notation.
    pub ip4_address: Option<String>,
    /// Assigned IPv6 address with CIDR notation.
    pub ip6_address: Option<String>,
    /// DNS servers configured for this VPN.
    pub dns_servers: Vec<String>,
    /// Protocol-specific connection details, if available.
    pub details: Option<VpnDetails>,
}
