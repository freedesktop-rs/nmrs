use std::{collections::HashMap, str::FromStr};

use zbus::zvariant::{OwnedValue, Value};

use crate::api::settings::error::SettingsError as ConnectionError;

use super::common::{ConnectionSection, IpV4Section, IpV6Section, ProxySection};

string_enum!(
    #[doc = "Bluetooth network role used by NetworkManager."]
    BluetoothType {
    PanU => "panu",
    Dun => "dun",
    Nap => "nap",
    }
);

/// Typed NetworkManager `bluetooth` settings.
#[derive(Debug, Clone)]
pub struct BluetoothSection {
    /// The NetworkManager `bluetooth.bdaddr` property.
    pub bdaddr: String,
    /// The Bluetooth network role (`bluetooth.type`).
    pub bt_type: BluetoothType,
}

impl TryFrom<&HashMap<String, OwnedValue>> for BluetoothSection {
    type Error = ConnectionError;

    fn try_from(value: &HashMap<String, OwnedValue>) -> Result<Self, Self::Error> {
        Ok(Self {
            bdaddr: extract_field!(value, "bdaddr", String)?,
            bt_type: BluetoothType::from_str(&extract_field!(value, "type", String)?)?,
        })
    }
}

impl From<BluetoothSection> for HashMap<&'static str, Value<'static>> {
    fn from(value: BluetoothSection) -> Self {
        HashMap::from([
            ("bdaddr", Value::from(value.bdaddr)),
            ("type", Value::from(value.bt_type.to_string())),
        ])
    }
}

/// Typed editable representation of a saved Bluetooth connection profile.
#[derive(Debug, Clone)]
pub struct BluetoothSettings {
    /// Shared connection metadata.
    pub connection: ConnectionSection,
    /// Bluetooth settings.
    pub bluetooth: BluetoothSection,
    /// Optional proxy settings.
    pub proxy: Option<ProxySection>,
    /// IPv4 settings.
    pub ipv4: IpV4Section,
    /// IPv6 settings.
    pub ipv6: IpV6Section,
}

impl BluetoothSettings {
    /// Parse the raw NetworkManager settings dictionary into Bluetooth settings.
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
        let bluetooth = BluetoothSection::try_from(
            value
                .get("bluetooth")
                .ok_or(ConnectionError::MissingField("bluetooth"))?,
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
            bluetooth,
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
        insert_section(&mut result, "bluetooth", self.bluetooth.clone())?;
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
