use std::collections::HashMap;

use zbus::zvariant::{OwnedValue, Value};

use crate::api::settings::error::SettingsError as ConnectionError;

use super::common::{ConnectionSection, IpV4Section, IpV6Section, ProxySection};
use super::profile::{parse_common_profile, sections_to_owned};

mod ieee_auth;
mod wireless;
mod wireless_security;

pub use ieee_auth::{IeeeAuthEap, IeeeAuthPhaseTwoAuth, IeeeAuthSection};
pub use wireless::{
    MacAddressRandomization, WirelessBand, WirelessMode, WirelessPowersave, WirelessSection,
};
pub use wireless_security::{
    FilsMode, GroupCipher, PairwiseCipher, PmfMode, WepKeyType, WirelessAuthAlg, WirelessProtocol,
    WirelessSecurityKeyMgmt, WirelessSecuritySection, WpsMethod,
};

/// Typed editable representation of a saved Wi-Fi connection profile.
#[derive(Debug, Clone)]
pub struct WiFiSettings {
    /// Shared connection metadata.
    pub connection: ConnectionSection,
    /// Wireless settings.
    pub wifi: WirelessSection,
    /// Optional wireless-security settings.
    pub wifi_security: Option<WirelessSecuritySection>,
    /// Optional IEEE 802.1X authentication settings.
    pub ieee_auth: Option<IeeeAuthSection>,
    /// Optional proxy settings.
    pub proxy: Option<ProxySection>,
    /// IPv4 settings.
    pub ipv4: IpV4Section,
    /// IPv6 settings.
    pub ipv6: IpV6Section,
}

impl WiFiSettings {
    /// Parse NetworkManager's raw `a{sa{sv}}` settings dictionary.
    pub fn from_raw_dict(
        value: HashMap<String, HashMap<String, OwnedValue>>,
    ) -> Result<Self, ConnectionError> {
        let common = parse_common_profile(&value)?;

        let wifi = WirelessSection::try_from(extract_map!(value, WirelessSection::get_name())?)?;

        let wifi_security = match value.get(WirelessSecuritySection::get_name()) {
            Some(wifi_sec_map) => Some(WirelessSecuritySection::try_from(wifi_sec_map)?),
            None => None,
        };

        let ieee_auth = match value.get(IeeeAuthSection::get_name()) {
            Some(ieee_auth_map) => Some(IeeeAuthSection::try_from(ieee_auth_map)?),
            None => None,
        };

        Ok(WiFiSettings {
            connection: common.connection,
            wifi,
            wifi_security,
            ieee_auth,
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

impl From<WiFiSettings> for HashMap<&str, HashMap<&str, Value<'_>>> {
    fn from(value: WiFiSettings) -> Self {
        let mut map = HashMap::new();

        map.insert(ConnectionSection::get_name(), value.connection.into());
        map.insert(WirelessSection::get_name(), value.wifi.into());

        if let Some(wifi_security) = value.wifi_security {
            map.insert(WirelessSecuritySection::get_name(), wifi_security.into());
        }

        if let Some(ieee_auth) = value.ieee_auth {
            map.insert(IeeeAuthSection::get_name(), ieee_auth.into());
        }

        if let Some(proxy) = value.proxy {
            map.insert(ProxySection::get_name(), proxy.into());
        }

        map.insert(IpV4Section::get_name(), value.ipv4.into());
        map.insert(IpV6Section::get_name(), value.ipv6.into());

        map
    }
}
