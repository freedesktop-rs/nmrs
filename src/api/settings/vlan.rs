use std::{collections::HashMap, str::FromStr};

use zbus::zvariant::{OwnedValue, Value};

use crate::api::settings::error::SettingsError as ConnectionError;

use super::common::{ConnectionSection, IpV4Section, IpV6Section, ProxySection};
use super::ethernet::EthernetSection;

string_enum!(
    #[doc = "VLAN encapsulation protocol."]
    VlanProtocol {
    Dot1Q => "802.1Q",
    Dot1Ad => "802.1ad",
    }
);

/// Typed NetworkManager `vlan` settings.
#[derive(Debug, Clone)]
pub struct VlanSection {
    /// The NetworkManager `vlan.parent` property.
    pub parent: String,
    /// The VLAN identifier, in the range 1 through 4094.
    pub id: u16,
    /// The NetworkManager `vlan.flags` property.
    pub flags: Option<u32>,
    /// The NetworkManager `vlan.ingress-priority-map` property.
    pub ingress_priority_map: Option<Vec<String>>,
    /// The NetworkManager `vlan.egress-priority-map` property.
    pub egress_priority_map: Option<Vec<String>>,
    /// The NetworkManager `vlan.interface-name` property.
    pub interface_name: Option<String>,
    /// The NetworkManager `vlan.protocol` property.
    pub protocol: Option<VlanProtocol>,
}

impl TryFrom<&HashMap<String, OwnedValue>> for VlanSection {
    type Error = ConnectionError;

    fn try_from(value: &HashMap<String, OwnedValue>) -> Result<Self, Self::Error> {
        let id = extract_field!(value, "id", u32)?;
        if !(1..=4094).contains(&id) {
            return Err(ConnectionError::FromStrError(
                id.to_string(),
                "VLAN ID (1..=4094)",
            ));
        }
        let protocol = extract_optional_field!(value, "protocol", String)?
            .map(|value| VlanProtocol::from_str(&value))
            .transpose()?;
        Ok(Self {
            parent: extract_field!(value, "parent", String)?,
            id: id as u16,
            flags: extract_optional_field!(value, "flags", u32)?,
            ingress_priority_map: extract_optional_vec_field!(
                value,
                "ingress-priority-map",
                String
            )?,
            egress_priority_map: extract_optional_vec_field!(value, "egress-priority-map", String)?,
            interface_name: extract_optional_field!(value, "interface-name", String)?,
            protocol,
        })
    }
}

impl From<VlanSection> for HashMap<&'static str, Value<'static>> {
    fn from(value: VlanSection) -> Self {
        let mut result = HashMap::new();
        insert_value!(result, "parent", value.parent);
        insert_value!(result, "id", u32::from(value.id));
        insert_opt!(result, "flags", value.flags);
        insert_opt!(result, "ingress-priority-map", value.ingress_priority_map);
        insert_opt!(result, "egress-priority-map", value.egress_priority_map);
        insert_opt!(result, "interface-name", value.interface_name);
        insert_opt!(
            result,
            "protocol",
            value.protocol.map(|protocol| protocol.to_string())
        );
        result
    }
}

/// Typed editable representation of a saved VLAN connection profile.
#[derive(Debug, Clone)]
pub struct VlanSettings {
    /// Shared connection metadata.
    pub connection: ConnectionSection,
    /// VLAN settings.
    pub vlan: VlanSection,
    /// Optional Ethernet settings.
    pub ethernet: Option<EthernetSection>,
    /// Optional proxy settings.
    pub proxy: Option<ProxySection>,
    /// IPv4 settings.
    pub ipv4: IpV4Section,
    /// IPv6 settings.
    pub ipv6: IpV6Section,
}

impl VlanSettings {
    /// Parse the raw NetworkManager settings dictionary into VLAN settings.
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
        let vlan = VlanSection::try_from(
            value
                .get("vlan")
                .ok_or(ConnectionError::MissingField("vlan"))?,
        )?;
        let ethernet = value
            .get("802-3-ethernet")
            .map(EthernetSection::try_from)
            .transpose()?;
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
            vlan,
            ethernet,
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
        insert_section(&mut result, "vlan", self.vlan.clone())?;
        insert_section(&mut result, "ipv4", self.ipv4.clone())?;
        insert_section(&mut result, "ipv6", self.ipv6.clone())?;
        if let Some(ethernet) = &self.ethernet {
            insert_section(&mut result, "802-3-ethernet", ethernet.clone())?;
        }
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
