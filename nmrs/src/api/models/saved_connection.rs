//! Saved NetworkManager connection profiles with decoded settings summaries.
//!
//! Use [`crate::NetworkManager::list_saved_connections`] to enumerate every
//! profile NM knows about (Wi-Fi, Ethernet, VPN, WireGuard, mobile, Bluetooth).
//! Secrets (PSK, EAP passwords, VPN tokens) are **not** included in
//! [`SavedConnection`] — NetworkManager only returns them via
//! [`GetSecrets`](https://networkmanager.dev/docs/api/latest/gdbus-org.freedesktop.NetworkManager.Settings.Connection.html#gdbus-method-org-freedesktop-NetworkManager-Settings-Connection.GetSecrets)
//! when a [secret agent](crate::agent) is registered. See feature `01-secret-agent`.

use std::{
    collections::HashMap,
    net::{Ipv4Addr, Ipv6Addr},
};

use zvariant::{OwnedObjectPath, OwnedValue};

use crate::{builders::Route, models::IpAddress};

/// Full saved profile with a structured [`SettingsSummary`].
#[non_exhaustive]
#[derive(Debug, Clone)]
pub struct SavedConnection {
    /// D-Bus object path of the settings connection.
    pub path: OwnedObjectPath,
    /// Connection UUID (`connection.uuid`).
    pub uuid: String,
    /// Human-visible name (`connection.id`).
    pub id: String,
    /// NM connection type string (`connection.type`), e.g. `802-11-wireless`.
    pub connection_type: String,
    /// Bound interface, if any (`connection.interface-name`).
    pub interface_name: Option<String>,
    /// Whether NM may auto-activate this profile (`connection.autoconnect`).
    pub autoconnect: bool,
    /// Autoconnect priority (`connection.autoconnect-priority`).
    pub autoconnect_priority: i32,
    /// Last activation time as Unix seconds (`connection.timestamp`), or `0` if never.
    pub timestamp_unix: u64,
    /// `connection.permissions` user strings, if present.
    pub permissions: Vec<String>,
    /// In-memory-only profile not yet written to disk.
    pub unsaved: bool,
    /// On-disk keyfile path when saved.
    pub filename: Option<String>,
    /// Decoded type-specific fields (no secrets).
    pub summary: SettingsSummary,
    /// IPv4 specific settings.
    pub ipv4: Option<IpSettings<Ipv4Addr>>,
    /// IPv6 specific settings.
    pub ipv6: Option<IpSettings<Ipv6Addr>>,
}

/// Cheap listing: path plus `connection` identity fields only (still one `GetSettings` per profile).
#[non_exhaustive]
#[derive(Debug, Clone)]
pub struct SavedConnectionBrief {
    /// D-Bus object path.
    pub path: OwnedObjectPath,
    /// `connection.uuid`.
    pub uuid: String,
    /// `connection.id`.
    pub id: String,
    /// `connection.type`.
    pub connection_type: String,
}

/// Partial update merged via [`crate::NetworkManager::update_saved_connection`].
#[non_exhaustive]
#[derive(Debug, Default, Clone)]
pub struct SettingsPatch {
    /// When `Some`, sets `connection.autoconnect`.
    pub autoconnect: Option<bool>,
    /// When `Some`, sets `connection.autoconnect-priority`.
    pub autoconnect_priority: Option<i32>,
    /// When `Some`, sets `connection.id`.
    pub id: Option<String>,
    /// `Some(Some(name))` sets `interface-name`; `Some(None)` clears it (best-effort empty string).
    pub interface_name: Option<Option<String>>,
    /// Merged after the fields above; section → key → value. Overwrites keys present.
    pub raw_overlay: Option<HashMap<String, HashMap<String, OwnedValue>>>,
}

/// NM `password-flags` / `psk-flags` style bitmask (subset used for summaries).
#[non_exhaustive]
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash)]
pub struct VpnSecretFlags(pub u32);

impl VpnSecretFlags {
    /// `NM_SETTING_SECRET_FLAG_AGENT_OWNED`.
    pub const AGENT_OWNED: u32 = 0x1;

    /// True if the secret is expected to be provided by an agent.
    #[must_use]
    pub fn agent_owned(self) -> bool {
        self.0 & Self::AGENT_OWNED != 0
    }
}

/// Wi-Fi key management style from `802-11-wireless-security.key-mgmt`.
#[non_exhaustive]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum WifiKeyMgmt {
    /// Open or no key management string.
    None,
    /// WEP (legacy).
    Wep,
    /// WPA-PSK (`wpa-psk`, `wpa-none`, …).
    WpaPsk,
    /// WPA-EAP / 802.1X.
    WpaEap,
    /// SAE (WPA3-Personal).
    Sae,
    /// OWE.
    Owe,
    /// OWE transition mode.
    OweTransitionMode,
}

/// Non-secret Wi-Fi security hints for UI / filtering.
#[non_exhaustive]
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WifiSecuritySummary {
    /// Derived key management style.
    pub key_mgmt: WifiKeyMgmt,
    /// `psk` key exists in non-secret settings.
    pub has_psk_field: bool,
    /// `psk-flags` has [`VpnSecretFlags::AGENT_OWNED`].
    pub psk_agent_owned: bool,
    /// EAP method names from `802-1x.eap`.
    pub eap_methods: Vec<String>,
}

/// Decoded summary for the connection `type` (and related sections).
#[non_exhaustive]
#[derive(Debug, Clone)]
pub enum SettingsSummary {
    /// `802-11-wireless` — SSID and security hints (no PSK / EAP secrets).
    Wifi {
        /// Decoded SSID (hidden networks may be empty).
        ssid: String,
        /// `mode` string from settings: `infrastructure`, `ap`, `adhoc`, …
        mode: Option<String>,
        /// Present when a security block exists (`802-11-wireless-security` / `802-1x`).
        security: Option<WifiSecuritySummary>,
        /// `band` if set (`a` / `bg`).
        band: Option<String>,
        /// `channel` if set.
        channel: Option<u32>,
        /// `bssid` MAC string if set.
        bssid: Option<String>,
        /// `hidden` property.
        hidden: bool,
        /// `mac-address-randomization` if set.
        mac_randomization: Option<String>,
    },
    /// `802-3-ethernet`.
    Ethernet {
        /// `mac-address` string if set.
        mac_address: Option<String>,
        /// `auto-negotiate`.
        auto_negotiate: Option<bool>,
        /// `speed` in Mbps.
        speed_mbps: Option<u32>,
        /// `mtu`.
        mtu: Option<u32>,
    },
    /// Generic `vpn` connection (non-WireGuard service types).
    Vpn {
        /// `vpn.service-type` (e.g. OpenVPN plugin name).
        service_type: String,
        /// `vpn.user-name`.
        user_name: Option<String>,
        /// `vpn.password-flags`.
        password_flags: VpnSecretFlags,
        /// Keys present in `vpn.data` (values omitted).
        data_keys: Vec<String>,
        /// `vpn.persistent` when present.
        persistent: bool,
    },
    /// Native WireGuard or VPN plugin pointing at WireGuard.
    WireGuard {
        /// `listen-port`.
        listen_port: Option<u16>,
        /// `mtu`.
        mtu: Option<u32>,
        /// `fwmark`.
        fwmark: Option<u32>,
        /// Number of peer dicts under `wireguard.peers`.
        peer_count: usize,
        /// `endpoint` of the first peer, if any.
        first_peer_endpoint: Option<String>,
    },
    /// `gsm` mobile broadband.
    Gsm {
        /// `apn`.
        apn: Option<String>,
        /// `username`.
        user_name: Option<String>,
        /// `password-flags`.
        password_flags: u32,
        /// `pin-flags`.
        pin_flags: u32,
    },
    /// `cdma` mobile broadband.
    Cdma {
        /// `number`.
        number: Option<String>,
        /// `username`.
        user_name: Option<String>,
        /// `password-flags`.
        password_flags: u32,
    },
    /// `bluetooth`.
    Bluetooth {
        /// Bluetooth MAC / bdaddr.
        bdaddr: String,
        /// `type` (`panu`, `dun`, …).
        bt_type: String,
    },
    /// Any other `connection.type` — lists settings section names only.
    Other {
        /// Keys from the top-level settings dict (`connection`, `ipv4`, …).
        sections: Vec<String>,
    },
}

/// Settings from the ipv4/6 section.
#[derive(Debug, Clone)]
#[non_exhaustive]
pub struct IpSettings<A> {
    /// The method by which the connection obtains its IP address for this protocol.
    pub method: IpMethod,
    /// The list of IP addresses.
    pub address_data: Vec<IpAddress<A>>,
    /// The gateway associated with this protocol configuration.
    pub gateway: Option<A>,
    /// The list of DNS search domains.
    pub dns_search: Vec<String>,
    /// The list of IP routes.
    pub route_data: Vec<Route>,
    /// If true, this connection will never be assigned the default route.
    pub never_default: bool,
    /// Ignore the automatically configured DNS servers, and only use the saved configuration.
    pub ignore_auto_dns: bool,
}

/// How the connection obtains its IP address.
#[derive(Debug, Clone)]
pub enum IpMethod {
    /// IP configuration is automatically defined according to the hardware interface.
    Auto,
    /// IP configuration is manually specified in the connection settings.
    Manual,
    /// The connection does not use or require an IP address of this type.
    Disabled,
    /// The connection should only be configured for link-local operation.
    LinkLocal,
    /// Allows other devices to connect through this device to the default network.
    Shared,
    /// IP configuration is ignored for this protocol
    Ignore,
    /// Unknown method
    Other(String),
}

impl From<String> for IpMethod {
    fn from(value: String) -> Self {
        match value.as_str() {
            "auto" => Self::Auto,
            "manual" => Self::Manual,
            "disabled" => Self::Disabled,
            "link-local" => Self::LinkLocal,
            "shared" => Self::Shared,
            "ignore" => Self::Ignore,
            _ => Self::Other(value),
        }
    }
}
