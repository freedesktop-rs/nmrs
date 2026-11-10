use std::collections::HashMap;

use zbus::zvariant::{Dict, OwnedValue, Value};

use crate::api::settings::error::SettingsError as ConnectionError;

use super::common::{ConnectionSection, IpV4Section, IpV6Section, ProxySection};

/// Typed NetworkManager `vpn` settings.
#[derive(Debug, Clone)]
pub struct VpnSection {
    /// The NetworkManager `vpn.service-type` property.
    pub service_type: String,
    /// The NetworkManager `vpn.user-name` property.
    pub user_name: Option<String>,
    /// The NetworkManager `vpn.persistent` property.
    pub persistent: bool,
    /// The NetworkManager `vpn.timeout` property.
    pub timeout: u32,
    /// VPN plugin-specific string key/value options.
    pub data: HashMap<String, String>,
    /// VPN plugin-specific secret key/value options.
    pub secrets: HashMap<String, String>,
}

/// Common NetworkManager OpenVPN plugin options decoded from `vpn.data`.
///
/// The original string map remains available on [`VpnSection::data`] because
/// OpenVPN plugins add options across NetworkManager versions.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct OpenVpnOptions {
    /// OpenVPN remote host or address.
    pub remote: Option<String>,
    /// OpenVPN transport type (`tun` or `tap`).
    pub connection_type: Option<String>,
    /// Whether OpenVPN uses TCP instead of UDP.
    pub proto_tcp: Option<bool>,
    /// The OpenVPN `username` option.
    pub username: Option<String>,
    /// The OpenVPN `auth` option.
    pub auth: Option<String>,
    /// The OpenVPN `cipher` option.
    pub cipher: Option<String>,
    /// The OpenVPN `ca` option.
    pub ca: Option<String>,
    /// The OpenVPN `cert` option.
    pub cert: Option<String>,
    /// The OpenVPN `key` option.
    pub key: Option<String>,
    /// The OpenVPN `tunnel-mtu` option.
    pub tunnel_mtu: Option<u32>,
    /// The OpenVPN `compress` option.
    pub compress: Option<String>,
    /// Expected remote certificate role, if configured.
    pub remote_cert_tls: Option<bool>,
}

impl OpenVpnOptions {
    /// Parse known OpenVPN options from a VPN `data` map.
    pub fn from_data(data: &HashMap<String, String>) -> Self {
        let bool_value = |key: &str| {
            data.get(key).and_then(|value| match value.as_str() {
                "yes" | "true" | "1" => Some(true),
                "no" | "false" | "0" => Some(false),
                _ => None,
            })
        };
        Self {
            remote: data.get("remote").cloned(),
            connection_type: data.get("connection-type").cloned(),
            proto_tcp: bool_value("proto-tcp"),
            username: data.get("username").cloned(),
            auth: data.get("auth").cloned(),
            cipher: data.get("cipher").cloned(),
            ca: data.get("ca").cloned(),
            cert: data.get("cert").cloned(),
            key: data.get("key").cloned(),
            tunnel_mtu: data.get("tunnel-mtu").and_then(|value| value.parse().ok()),
            compress: data
                .get("compress")
                .or_else(|| data.get("comp-lzo"))
                .cloned(),
            remote_cert_tls: bool_value("remote-cert-tls"),
        }
    }

    /// Apply these OpenVPN options to a VPN `data` map, preserving other keys.
    pub fn apply_to_data(&self, data: &mut HashMap<String, String>) {
        macro_rules! set {
            ($key:literal, $value:expr) => {
                if let Some(value) = $value {
                    data.insert($key.into(), value);
                }
            };
        }
        set!("remote", self.remote.clone());
        set!("connection-type", self.connection_type.clone());
        if let Some(value) = self.proto_tcp {
            data.insert("proto-tcp".into(), value.to_string());
        }
        set!("username", self.username.clone());
        set!("auth", self.auth.clone());
        set!("cipher", self.cipher.clone());
        set!("ca", self.ca.clone());
        set!("cert", self.cert.clone());
        set!("key", self.key.clone());
        if let Some(value) = self.tunnel_mtu {
            data.insert("tunnel-mtu".into(), value.to_string());
        }
        set!("compress", self.compress.clone());
        if let Some(value) = self.remote_cert_tls {
            data.insert("remote-cert-tls".into(), value.to_string());
        }
    }
}

impl TryFrom<&HashMap<String, OwnedValue>> for VpnSection {
    type Error = ConnectionError;

    fn try_from(value: &HashMap<String, OwnedValue>) -> Result<Self, Self::Error> {
        Ok(Self {
            service_type: extract_field!(value, "service-type", String)?,
            user_name: extract_optional_field!(value, "user-name", String)?,
            persistent: extract_field!(value, "persistent", bool).unwrap_or(false),
            timeout: extract_field!(value, "timeout", u32).unwrap_or(0),
            data: parse_string_map(value, "data")?,
            secrets: parse_string_map(value, "secrets")?,
        })
    }
}

fn parse_string_map(
    value: &HashMap<String, OwnedValue>,
    key: &'static str,
) -> Result<HashMap<String, String>, ConnectionError> {
    let Some(raw) = value.get(key) else {
        return Ok(HashMap::new());
    };
    let dict = raw
        .downcast_ref::<Dict<'_, '_>>()
        .map_err(|_| ConnectionError::FailedDowncast((key, "Dict")))?;
    let mut result = HashMap::new();
    for (raw_key, raw_value) in dict.iter() {
        let map_key = match raw_key {
            Value::Str(value) => value.to_string(),
            _ => return Err(ConnectionError::FailedDowncast((key, "string key"))),
        };
        let map_value = match raw_value {
            Value::Str(value) => value.to_string(),
            _ => return Err(ConnectionError::FailedDowncast((key, "string value"))),
        };
        result.insert(map_key, map_value);
    }
    Ok(result)
}

impl From<VpnSection> for HashMap<&'static str, Value<'static>> {
    fn from(value: VpnSection) -> Self {
        let mut result = HashMap::new();
        insert_value!(result, "service-type", value.service_type);
        insert_opt!(result, "user-name", value.user_name);
        insert_value!(result, "persistent", value.persistent);
        insert_value!(result, "timeout", value.timeout);
        insert_value!(result, "data", value.data);
        if !value.secrets.is_empty() {
            insert_value!(result, "secrets", value.secrets);
        }
        result
    }
}

/// Typed editable representation of a saved VPN connection profile.
#[derive(Debug, Clone)]
pub struct VpnSettings {
    /// Shared connection metadata.
    pub connection: ConnectionSection,
    /// VPN settings.
    pub vpn: VpnSection,
    /// Optional proxy settings.
    pub proxy: Option<ProxySection>,
    /// IPv4 settings.
    pub ipv4: IpV4Section,
    /// IPv6 settings.
    pub ipv6: IpV6Section,
}

impl VpnSettings {
    /// Return typed OpenVPN options when this profile uses the OpenVPN plugin.
    pub fn openvpn_options(&self) -> Option<OpenVpnOptions> {
        self.vpn
            .service_type
            .contains("openvpn")
            .then(|| OpenVpnOptions::from_data(&self.vpn.data))
    }

    /// Parse the raw NetworkManager settings dictionary into VPN settings.
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
        let vpn = VpnSection::try_from(
            value
                .get("vpn")
                .ok_or(ConnectionError::MissingField("vpn"))?,
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
            vpn,
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
        insert_section(&mut result, "vpn", self.vpn.clone())?;
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

/// OpenVPN uses the standard VPN section with plugin-specific string data.
pub type OpenVpnSettings = VpnSettings;

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
