use std::{collections::HashMap, str::FromStr};

use zbus::zvariant::{OwnedValue, Value};

use crate::api::settings::error::SettingsError as ConnectionError;

use super::super::common::BoolWithDefault;

/// Editable NetworkManager Wi-Fi settings.
#[derive(Debug, Clone)]
pub struct WirelessSection {
    /// The NetworkManager `802-11-wireless.ssid` property.
    pub ssid: String,
    /// The NetworkManager `802-11-wireless.mode` property.
    pub mode: WirelessMode,
    /// The NetworkManager `802-11-wireless.band` property.
    pub band: Option<WirelessBand>,
    /// The NetworkManager `802-11-wireless.bssid` property.
    pub bssid: Option<String>,
    /// The NetworkManager `802-11-wireless.channel` property.
    pub channel: u32,
    /// The NetworkManager `802-11-wireless.cloned-mac-address` property.
    pub cloned_mac_address: Option<String>,
    /// The NetworkManager `802-11-wireless.assigned-mac-address` property.
    pub assigned_mac_address: Option<String>,
    /// The NetworkManager `802-11-wireless.generate-mac-address-mask` property.
    pub generate_mac_address_mask: Option<String>,
    /// The NetworkManager `802-11-wireless.hidden` property.
    pub hidden: bool,
    /// The NetworkManager `802-11-wireless.mac-address` property.
    pub mac_address: Option<String>,
    /// The NetworkManager `802-11-wireless.mac-address-blacklist` property.
    pub mac_address_blacklist: Vec<String>,
    /// The NetworkManager `802-11-wireless.mac-address-randomization` property.
    pub mac_address_randomization: MacAddressRandomization,
    /// The NetworkManager `802-11-wireless.mtu` property.
    pub mtu: u32,
    /// The NetworkManager `802-11-wireless.powersave` property.
    pub powersave: WirelessPowersave,
    /// The NetworkManager `802-11-wireless.tx-power` property.
    pub tx_power: u32,
    /// The NetworkManager `802-11-wireless.wake-on-wlan` property.
    pub wake_on_wlan: u32,
    /// The NetworkManager `802-11-wireless.ap-isolation` property.
    pub ap_isolation: BoolWithDefault,
}

impl WirelessSection {
    pub(crate) const fn get_name() -> &'static str {
        "802-11-wireless"
    }
}

impl From<WirelessSection> for HashMap<&str, Value<'_>> {
    fn from(value: WirelessSection) -> Self {
        let mut wifi = HashMap::new();
        insert_value!(wifi, "ssid", value.ssid.into_bytes());
        insert_value!(wifi, "mode", value.mode.to_string());
        insert_opt!(wifi, "band", value.band.map(|band| band.to_string()));
        insert_opt!(wifi, "bssid", value.bssid.map(String::into_bytes));
        insert_value!(wifi, "channel", value.channel);
        insert_opt!(
            wifi,
            "cloned-mac-address",
            value.cloned_mac_address.map(String::into_bytes)
        );
        insert_opt!(wifi, "assigned-mac-address", value.assigned_mac_address);
        insert_opt!(
            wifi,
            "generate-mac-address-mask",
            value.generate_mac_address_mask
        );
        insert_value!(wifi, "hidden", value.hidden);
        insert_opt!(
            wifi,
            "mac-address",
            value.mac_address.map(String::into_bytes)
        );
        if !value.mac_address_blacklist.is_empty() {
            insert_value!(wifi, "mac-address-blacklist", value.mac_address_blacklist);
        }
        insert_value!(
            wifi,
            "mac-address-randomization",
            u32::from(value.mac_address_randomization)
        );
        insert_value!(wifi, "mtu", value.mtu);
        insert_value!(wifi, "powersave", u32::from(value.powersave));
        insert_value!(wifi, "tx-power", value.tx_power);
        insert_value!(wifi, "wake-on-wlan", value.wake_on_wlan);
        insert_value!(wifi, "ap-isolation", i32::from(value.ap_isolation));
        wifi
    }
}

impl TryFrom<&HashMap<String, OwnedValue>> for WirelessSection {
    type Error = ConnectionError;
    fn try_from(value: &HashMap<String, OwnedValue>) -> Result<Self, Self::Error> {
        Ok(WirelessSection {
            ssid: str::from_utf8(&extract_vec_field!(value, "ssid", u8)?)?.to_string(),
            mode: WirelessMode::from_str(&extract_field!(value, "mode", String)?)?,
            band: extract_optional_field!(value, "band", String)?
                .map(|value| WirelessBand::from_str(&value))
                .transpose()?,
            bssid: extract_optional_vec_field!(value, "bssid", u8)?
                .map(|v| str::from_utf8(&v).map(|s| s.to_string()))
                .transpose()?,
            channel: extract_field!(value, "channel", u32).unwrap_or(0),
            cloned_mac_address: extract_optional_vec_field!(value, "cloned-mac-address", u8)?
                .map(|v| str::from_utf8(&v).map(|s| s.to_string()))
                .transpose()?,
            assigned_mac_address: extract_optional_field!(value, "assigned-mac-address", String)?,
            generate_mac_address_mask: extract_optional_field!(
                value,
                "generate-mac-address-mask",
                String
            )?,
            hidden: extract_field!(value, "hidden", bool).unwrap_or(false),
            mac_address: extract_optional_vec_field!(value, "mac-address", u8)?
                .map(|v| str::from_utf8(&v).map(|s| s.to_string()))
                .transpose()?,
            mac_address_blacklist: extract_vec_field_or_default!(
                value,
                "mac-address-blacklist",
                String
            )?,
            mac_address_randomization: MacAddressRandomization::from(
                extract_field!(value, "mac-address-randomization", u32).unwrap_or(0),
            ),
            mtu: extract_field!(value, "mtu", u32).unwrap_or(0),
            powersave: WirelessPowersave::from(
                extract_field!(value, "powersave", u32).unwrap_or(0),
            ),
            tx_power: extract_field!(value, "tx-power", u32).unwrap_or(0),
            wake_on_wlan: extract_field!(value, "wake-on-wlan", u32).unwrap_or(1),
            ap_isolation: BoolWithDefault::from(
                extract_field!(value, "ap-isolation", i32).unwrap_or(-1),
            ),
        })
    }
}

string_enum!(WirelessMode {
    Infrastructure => "infrastructure",
    Mesh => "mesh",
    Adhoc => "adhoc",
    Ap => "ap",
});

string_enum!(WirelessBand {
    Ghz24 => "bg",
    Ghz5 => "a",
    Ghz6 => "6GHz",
});

int_enum!(MacAddressRandomization : u32 {
    Default = 0,
    Never = 1,
    Always = 2,
} default Default);

int_enum!(WirelessPowersave : u32 {
    Default = 0,
    Ignore = 1,
    Disable = 2,
    Enable = 3,
} default Default);
