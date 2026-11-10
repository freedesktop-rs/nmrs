use std::collections::HashMap;

use zbus::zvariant::{Array, Dict, OwnedValue, Value};

use crate::api::settings::error::SettingsError as ConnectionError;

use super::common::{BoolWithDefault, ConnectionSection, IpV4Section, IpV6Section, ProxySection};

/// One peer in a NetworkManager WireGuard profile.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WireGuardPeer {
    /// The `public-key` value in a `wireguard.peers` entry.
    pub public_key: String,
    /// The `endpoint` value in a `wireguard.peers` entry.
    pub endpoint: Option<String>,
    /// The `allowed-ips` value in a `wireguard.peers` entry.
    pub allowed_ips: Vec<String>,
    /// The `persistent-keepalive` value in a `wireguard.peers` entry.
    pub persistent_keepalive: Option<u32>,
    /// The `preshared-key` value in a `wireguard.peers` entry.
    pub preshared_key: Option<String>,
}

impl WireGuardPeer {
    fn from_dict(dict: &Dict<'_, '_>) -> Result<Self, ConnectionError> {
        let public_key = dict
            .get::<_, String>(&"public-key")?
            .ok_or(ConnectionError::MissingField("wireguard peer public-key"))?;
        let allowed_ips = match dict.get::<_, Array<'_>>(&"allowed-ips")? {
            Some(array) => array
                .iter()
                .map(|value| {
                    value
                        .downcast_ref::<String>()
                        .map_err(|_| ConnectionError::FailedDowncast(("allowed-ips", "String")))
                })
                .collect::<Result<Vec<_>, _>>()?,
            None => Vec::new(),
        };
        Ok(Self {
            public_key,
            endpoint: dict.get::<_, String>(&"endpoint")?,
            allowed_ips,
            persistent_keepalive: dict.get::<_, u32>(&"persistent-keepalive")?,
            preshared_key: dict.get::<_, String>(&"preshared-key")?,
        })
    }
}

impl From<WireGuardPeer> for HashMap<&'static str, Value<'static>> {
    fn from(peer: WireGuardPeer) -> Self {
        let mut result = HashMap::new();
        insert_value!(result, "public-key", peer.public_key);
        insert_value!(result, "allowed-ips", peer.allowed_ips);
        insert_opt!(result, "endpoint", peer.endpoint);
        insert_opt!(result, "persistent-keepalive", peer.persistent_keepalive);
        insert_opt!(result, "preshared-key", peer.preshared_key);
        result
    }
}

/// Typed NetworkManager `wireguard` settings.
#[derive(Debug, Clone)]
pub struct WireGuardSection {
    /// The NetworkManager `wireguard.listen-port` property.
    pub listen_port: u32,
    /// The NetworkManager `wireguard.mtu` property.
    pub mtu: u32,
    /// The NetworkManager `wireguard.fwmark` property.
    pub fwmark: u32,
    /// The NetworkManager `wireguard.peer-routes` property.
    pub peer_routes: bool,
    /// The NetworkManager `wireguard.ip4-auto-default-route` property.
    pub ip4_auto_default_route: BoolWithDefault,
    /// The NetworkManager `wireguard.ip6-auto-default-route` property.
    pub ip6_auto_default_route: BoolWithDefault,
    /// The NetworkManager `wireguard.private-key` property.
    pub private_key: Option<String>,
    /// The NetworkManager `wireguard.private-key-flags` property.
    pub private_key_flags: u32,
    /// The NetworkManager `wireguard.peers` property.
    pub peers: Vec<WireGuardPeer>,
}

impl TryFrom<&HashMap<String, OwnedValue>> for WireGuardSection {
    type Error = ConnectionError;

    fn try_from(value: &HashMap<String, OwnedValue>) -> Result<Self, Self::Error> {
        let peers = match value.get("peers") {
            Some(value) => {
                let array = value
                    .downcast_ref::<Array<'_>>()
                    .map_err(|_| ConnectionError::FailedDowncast(("peers", "Array")))?;
                array
                    .iter()
                    .map(|value| {
                        let dict = value
                            .downcast_ref::<Dict<'_, '_>>()
                            .map_err(|_| ConnectionError::FailedDowncast(("peers", "Dict")))?;
                        WireGuardPeer::from_dict(&dict)
                    })
                    .collect::<Result<Vec<_>, _>>()?
            }
            None => Vec::new(),
        };
        Ok(Self {
            listen_port: extract_field!(value, "listen-port", u32).unwrap_or(0),
            mtu: extract_field!(value, "mtu", u32).unwrap_or(0),
            fwmark: extract_field!(value, "fwmark", u32).unwrap_or(0),
            peer_routes: extract_field!(value, "peer-routes", bool).unwrap_or(true),
            ip4_auto_default_route: BoolWithDefault::from(
                extract_field!(value, "ip4-auto-default-route", i32).unwrap_or(-1),
            ),
            ip6_auto_default_route: BoolWithDefault::from(
                extract_field!(value, "ip6-auto-default-route", i32).unwrap_or(-1),
            ),
            private_key: extract_optional_field!(value, "private-key", String)?,
            private_key_flags: extract_field!(value, "private-key-flags", u32).unwrap_or(0),
            peers,
        })
    }
}

impl From<WireGuardSection> for HashMap<&'static str, Value<'static>> {
    fn from(section: WireGuardSection) -> Self {
        let mut result = HashMap::new();
        insert_value!(result, "listen-port", section.listen_port);
        insert_value!(result, "mtu", section.mtu);
        insert_value!(result, "fwmark", section.fwmark);
        insert_value!(result, "peer-routes", section.peer_routes);
        insert_value!(
            result,
            "ip4-auto-default-route",
            i32::from(section.ip4_auto_default_route)
        );
        insert_value!(
            result,
            "ip6-auto-default-route",
            i32::from(section.ip6_auto_default_route)
        );
        insert_opt!(result, "private-key", section.private_key);
        insert_value!(result, "private-key-flags", section.private_key_flags);
        let peers: Vec<HashMap<&str, Value<'_>>> =
            section.peers.into_iter().map(Into::into).collect();
        insert_value!(result, "peers", peers);
        result
    }
}

/// Typed editable representation of a saved WireGuard connection profile.
#[derive(Debug, Clone)]
pub struct WireGuardSettings {
    /// Shared connection metadata.
    pub connection: ConnectionSection,
    /// WireGuard settings.
    pub wireguard: WireGuardSection,
    /// Optional proxy settings.
    pub proxy: Option<ProxySection>,
    /// IPv4 settings.
    pub ipv4: IpV4Section,
    /// IPv6 settings.
    pub ipv6: IpV6Section,
}

impl WireGuardSettings {
    /// Parse the raw NetworkManager settings dictionary into WireGuard settings.
    ///
    /// # Errors
    /// Returns an error when a required section or value is absent or invalid.
    pub fn from_raw_dict(
        value: HashMap<String, HashMap<String, OwnedValue>>,
    ) -> Result<Self, ConnectionError> {
        let connection = ConnectionSection::try_from(
            value
                .get("connection")
                .ok_or(ConnectionError::MissingField("connection"))?,
        )?;
        let wireguard = WireGuardSection::try_from(
            value
                .get("wireguard")
                .ok_or(ConnectionError::MissingField("wireguard"))?,
        )?;
        let proxy = value.get("proxy").map(ProxySection::try_from).transpose()?;
        let ipv4 = IpV4Section::try_from(
            value
                .get("ipv4")
                .ok_or(ConnectionError::MissingField("ipv4"))?,
        )?;
        let ipv6 = IpV6Section::try_from(
            value
                .get("ipv6")
                .ok_or(ConnectionError::MissingField("ipv6"))?,
        )?;
        Ok(Self {
            connection,
            wireguard,
            proxy,
            ipv4,
            ipv6,
        })
    }

    /// Serialize these settings into NetworkManager's owned settings dictionary.
    ///
    /// # Errors
    /// Returns an error if an IP section is invalid or a value cannot be serialized.
    pub fn to_owned_dict(
        &self,
    ) -> Result<HashMap<String, HashMap<String, OwnedValue>>, ConnectionError> {
        self.ipv4.validate()?;
        self.ipv6.validate()?;
        let mut result = HashMap::new();
        insert_section(&mut result, "connection", self.connection.clone())?;
        insert_section(&mut result, "wireguard", self.wireguard.clone())?;
        insert_section(&mut result, "ipv4", self.ipv4.clone())?;
        insert_section(&mut result, "ipv6", self.ipv6.clone())?;
        if let Some(proxy) = &self.proxy {
            insert_section(&mut result, "proxy", proxy.clone())?;
        }
        Ok(result)
    }

    /// Build a merge patch containing these settings.
    ///
    /// # Errors
    /// Returns an error if these settings cannot be serialized.
    pub fn to_patch(&self) -> Result<crate::SettingsPatch, crate::ConnectionError> {
        Ok(crate::SettingsPatch {
            raw_overlay: Some(self.to_owned_dict()?),
            ..Default::default()
        })
    }
}

fn insert_section<T>(
    output: &mut HashMap<String, HashMap<String, OwnedValue>>,
    name: &str,
    section: T,
) -> Result<(), ConnectionError>
where
    T: Into<HashMap<&'static str, Value<'static>>>,
{
    let fields = section
        .into()
        .into_iter()
        .map(|(key, value)| Ok((key.to_string(), value.try_into_owned()?)))
        .collect::<Result<HashMap<_, _>, ConnectionError>>()?;
    output.insert(name.to_string(), fields);
    Ok(())
}
