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
    fmt,
    net::{Ipv4Addr, Ipv6Addr},
    sync::Arc,
};

use zvariant::{OwnedObjectPath, OwnedValue};

use crate::models::{FromSetting, IpAddress, IpRoute, Property};

/// Raw `GetSettings` output: section name to key to value.
pub(crate) type RawSettings = HashMap<String, HashMap<String, OwnedValue>>;

/// Full saved profile with a structured [`SettingsSummary`].
///
/// Common sections are decoded into typed fields. Any other key is reachable
/// through [`get_property`](Self::get_property).
#[non_exhaustive]
#[derive(Clone)]
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
    /// Decoded `ipv4` section, if the profile has one.
    pub ipv4: Option<IpSettings<Ipv4Addr>>,
    /// Decoded `ipv6` section, if the profile has one.
    pub ipv6: Option<IpSettings<Ipv6Addr>>,
    /// Everything `GetSettings` returned, shared between clones.
    pub(crate) settings: Arc<RawSettings>,
}

impl SavedConnection {
    /// Reads one `section.key` from the profile, decoded as `T`.
    ///
    /// Returns `None` when the section or key is absent, which is how
    /// NetworkManager reports a key left at its default, and when the stored
    /// value is not a `T`. Secrets are never present: `GetSettings` omits them.
    ///
    /// ```rust,no_run
    /// use nmrs::NetworkManager;
    /// use nmrs::models::{Property, properties};
    ///
    /// # async fn run() -> nmrs::Result<()> {
    /// let nm = NetworkManager::new().await?;
    /// for profile in nm.list_saved_connections().await? {
    ///     let zone = profile.get_property(properties::connection::ZONE);
    ///     let mtu = profile.get_property(Property::<u32>::new("802-3-ethernet", "mtu"));
    ///     println!("{}: zone={zone:?} mtu={mtu:?}", profile.id);
    /// }
    /// # Ok(())
    /// # }
    /// ```
    #[must_use]
    pub fn get_property<T: FromSetting>(&self, property: Property<T>) -> Option<T> {
        self.settings
            .get(property.section())?
            .get(property.key())
            .and_then(T::from_setting)
    }

    /// Names of the settings sections present in the profile
    /// (`connection`, `ipv4`, `802-11-wireless`, …), sorted.
    #[must_use]
    pub fn sections(&self) -> Vec<&str> {
        let mut sections: Vec<&str> = self.settings.keys().map(String::as_str).collect();
        sections.sort_unstable();
        sections
    }

    /// Whether the profile has a `section`.
    #[must_use]
    pub fn has_section(&self, section: &str) -> bool {
        self.settings.contains_key(section)
    }
}

impl fmt::Debug for SavedConnection {
    /// Lists section names in place of the raw map, which would otherwise
    /// dump every key of every profile into logs.
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("SavedConnection")
            .field("path", &self.path)
            .field("uuid", &self.uuid)
            .field("id", &self.id)
            .field("connection_type", &self.connection_type)
            .field("interface_name", &self.interface_name)
            .field("autoconnect", &self.autoconnect)
            .field("autoconnect_priority", &self.autoconnect_priority)
            .field("timestamp_unix", &self.timestamp_unix)
            .field("permissions", &self.permissions)
            .field("unsaved", &self.unsaved)
            .field("filename", &self.filename)
            .field("summary", &self.summary)
            .field("ipv4", &self.ipv4)
            .field("ipv6", &self.ipv6)
            .field("sections", &self.sections())
            .finish()
    }
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

/// IP configuration decoded from a profile's `ipv4` or `ipv6` section.
///
/// `A` is [`Ipv4Addr`] for [`SavedConnection::ipv4`] and [`Ipv6Addr`] for
/// [`SavedConnection::ipv6`], so addresses, the gateway, name servers, and
/// routes are typed for that family. Keys NetworkManager omits decode to its
/// documented defaults (`Auto`, empty lists, `false`), the same way the
/// `connection` section does. Entries stored in a form that does not parse
/// as an address are dropped with a warning instead of failing the profile.
#[non_exhaustive]
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IpSettings<A> {
    /// How the connection obtains its address for this family (`method`).
    pub method: IpMethod,
    /// Static addresses with prefix lengths (`address-data`).
    pub addresses: Vec<IpAddress<A>>,
    /// Default gateway for this family (`gateway`).
    pub gateway: Option<A>,
    /// Name servers (`dns-data`, or the legacy `dns` array on daemons that
    /// do not send `dns-data`). Only the address is kept: a DNS-over-TLS
    /// server name (`1.1.1.1#one.one.one.one`) or the port of a `dns+tls://`
    /// URI is stripped.
    pub dns: Vec<A>,
    /// DNS search domains (`dns-search`).
    pub dns_search: Vec<String>,
    /// Static routes (`route-data`).
    pub routes: Vec<IpRoute<A>>,
    /// The connection is never assigned the default route (`never-default`).
    pub never_default: bool,
    /// Automatically obtained name servers are ignored (`ignore-auto-dns`).
    pub ignore_auto_dns: bool,
}

/// How a connection obtains its address for one IP family (`ipv4.method` or
/// `ipv6.method`).
#[non_exhaustive]
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum IpMethod {
    /// Automatic configuration: DHCP for IPv4, router advertisements and
    /// DHCPv6 for IPv6.
    Auto,
    /// Addresses come from a DHCPv6 server only (IPv6 only).
    Dhcp,
    /// Addresses are set manually in the profile.
    Manual,
    /// This family is not configured on the connection.
    Disabled,
    /// Only a link-local address is configured.
    LinkLocal,
    /// Other devices connect through this one to the default network.
    Shared,
    /// IP configuration for this family is left untouched (IPv6 only).
    Ignore,
    /// A method this crate does not know; carries NetworkManager's raw string.
    Other(String),
}

impl From<String> for IpMethod {
    fn from(value: String) -> Self {
        match value.as_str() {
            "auto" => Self::Auto,
            "dhcp" => Self::Dhcp,
            "manual" => Self::Manual,
            "disabled" => Self::Disabled,
            "link-local" => Self::LinkLocal,
            "shared" => Self::Shared,
            "ignore" => Self::Ignore,
            _ => Self::Other(value),
        }
    }
}
