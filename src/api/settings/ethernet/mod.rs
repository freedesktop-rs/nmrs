use std::collections::HashMap;

use zbus::zvariant::{OwnedValue, Value};

use crate::api::settings::error::SettingsError as ConnectionError;

use super::common::{ConnectionSection, IpV4Section, IpV6Section, ProxySection};
use super::profile::{parse_common_profile, sections_to_owned};

mod section;

pub use section::{Duplex, EthernetSection, PortType, S390NettypeType};

/// Typed editable representation of a saved Ethernet connection profile.
#[derive(Debug, Clone)]
pub struct EthernetSettings {
    /// Shared connection metadata.
    pub connection: ConnectionSection,
    /// Optional Ethernet settings.
    pub ethernet: EthernetSection,
    /// Optional proxy settings.
    pub proxy: Option<ProxySection>,
    /// IPv4 settings.
    pub ipv4: IpV4Section,
    /// IPv6 settings.
    pub ipv6: IpV6Section,
}

impl EthernetSettings {
    /// Parse NetworkManager's raw `a{sa{sv}}` settings dictionary.
    pub fn from_raw_dict(
        value: HashMap<String, HashMap<String, OwnedValue>>,
    ) -> Result<Self, ConnectionError> {
        let common = parse_common_profile(&value)?;
        let ethernet =
            EthernetSection::try_from(extract_map!(value, EthernetSection::get_name())?)?;

        Ok(EthernetSettings {
            connection: common.connection,
            ethernet,
            proxy: common.proxy,
            ipv4: common.ipv4,
            ipv6: common.ipv6,
        })
    }

    /// Serialize the profile into NetworkManager's owned settings dictionary.
    pub fn to_owned_dict(
        &self,
    ) -> Result<HashMap<String, HashMap<String, OwnedValue>>, ConnectionError> {
        sections_to_owned(self.clone().into())
    }

    /// Build a merge patch for `nm.update_saved_connection`.
    ///
    /// Keys and sections not present in the patch remain in the saved profile.
    pub fn to_patch(&self) -> Result<crate::SettingsPatch, crate::ConnectionError> {
        Ok(crate::SettingsPatch {
            raw_overlay: Some(self.to_owned_dict()?),
            ..Default::default()
        })
    }
}

impl From<EthernetSettings> for HashMap<&str, HashMap<&str, Value<'_>>> {
    fn from(value: EthernetSettings) -> Self {
        let mut map = HashMap::new();
        map.insert(ConnectionSection::get_name(), value.connection.into());
        map.insert(EthernetSection::get_name(), value.ethernet.into());
        if let Some(proxy) = value.proxy {
            map.insert(ProxySection::get_name(), proxy.into());
        }
        map.insert(IpV4Section::get_name(), value.ipv4.into());
        map.insert(IpV6Section::get_name(), value.ipv6.into());
        map
    }
}
