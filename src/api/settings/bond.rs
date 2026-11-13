use std::collections::HashMap;

use zbus::zvariant::{Dict, OwnedValue, Value};

use crate::api::settings::error::SettingsError as ConnectionError;

use super::common::{ConnectionSection, IpV4Section, IpV6Section, ProxySection};

/// Typed NetworkManager `bond` settings.
#[derive(Debug, Clone)]
pub struct BondSection {
    /// Bond option names and their string values.
    pub options: HashMap<String, String>,
}

impl TryFrom<&HashMap<String, OwnedValue>> for BondSection {
    type Error = ConnectionError;

    fn try_from(value: &HashMap<String, OwnedValue>) -> Result<Self, Self::Error> {
        let options = match value.get("options") {
            Some(value) => {
                let dict = value
                    .downcast_ref::<Dict<'_, '_>>()
                    .map_err(|_| ConnectionError::FailedDowncast(("options", "Dict")))?;
                let mut result = HashMap::new();
                for (key, value) in dict.iter() {
                    let key = match key {
                        Value::Str(value) => value.to_string(),
                        _ => {
                            return Err(ConnectionError::FailedDowncast(("options", "string key")));
                        }
                    };
                    let value = match value {
                        Value::Str(value) => value.to_string(),
                        _ => {
                            return Err(ConnectionError::FailedDowncast((
                                "options",
                                "string value",
                            )));
                        }
                    };
                    result.insert(key, value);
                }
                result
            }
            None => HashMap::new(),
        };
        Ok(Self { options })
    }
}

impl From<BondSection> for HashMap<&'static str, Value<'static>> {
    fn from(value: BondSection) -> Self {
        HashMap::from([("options", Value::from(value.options))])
    }
}

/// Typed editable representation of a saved bond connection profile.
#[derive(Debug, Clone)]
pub struct BondSettings {
    /// Shared connection metadata.
    pub connection: ConnectionSection,
    /// Bond settings.
    pub bond: BondSection,
    /// Optional proxy settings.
    pub proxy: Option<ProxySection>,
    /// IPv4 settings.
    pub ipv4: IpV4Section,
    /// IPv6 settings.
    pub ipv6: IpV6Section,
}

impl BondSettings {
    /// Parse the raw NetworkManager settings dictionary into bond settings.
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
        let bond = BondSection::try_from(
            value
                .get("bond")
                .ok_or(ConnectionError::MissingField("bond"))?,
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
            bond,
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
        insert_section(&mut result, "bond", self.bond.clone())?;
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
    output.insert(
        name.to_string(),
        section
            .into()
            .into_iter()
            .map(|(key, value)| Ok((key.to_string(), value.try_into_owned()?)))
            .collect::<Result<_, ConnectionError>>()?,
    );
    Ok(())
}
