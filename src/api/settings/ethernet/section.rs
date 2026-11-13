use std::{collections::HashMap, str::FromStr};

use zbus::zvariant::{Dict, OwnedValue, Value};

use crate::api::settings::error::SettingsError as ConnectionError;

use super::super::common::BoolWithDefault;

string_enum!(Duplex { Half => "half", Full => "full" } default Full);

string_enum!(
    PortType {
        TwistedPair => "tp",
        AttachmentUnitInterface => "aui",
        ThinEthernet => "bnc",
        MediaIndependentInterface => "mii",
    } default TwistedPair
);

string_enum!(S390NettypeType {
    Qeth => "qeth",
    Lcs => "lcs",
    Ctc => "ctc",
});

/// Editable NetworkManager Ethernet settings.
#[derive(Debug, Default, Clone)]
pub struct EthernetSection {
    /// The NetworkManager `802-3-ethernet.accept-all-mac-addresses` property.
    pub accept_all_mac_addresses: BoolWithDefault,
    /// The NetworkManager `802-3-ethernet.assigned-mac-address` property.
    pub assigned_mac_address: Option<String>,
    /// The NetworkManager `802-3-ethernet.cloned-mac-address` property.
    pub cloned_mac_address: Option<String>,
    /// The NetworkManager `802-3-ethernet.auto-negotiate` property.
    pub auto_negotiate: bool,
    /// The NetworkManager `802-3-ethernet.duplex` property.
    pub duplex: Option<Duplex>,
    /// The NetworkManager `802-3-ethernet.generate-mac-address-mask` property.
    pub generate_mac_address_mask: Option<String>,
    /// The NetworkManager `802-3-ethernet.mac-address` property.
    pub mac_address: Option<String>,
    /// The NetworkManager `802-3-ethernet.mac-address-blacklist` property.
    pub mac_address_blacklist: Vec<String>,
    /// The NetworkManager `802-3-ethernet.mac-address-denylist` property.
    pub mac_address_denylist: Vec<String>,
    /// The NetworkManager `802-3-ethernet.mtu` property.
    pub mtu: u32,
    /// The NetworkManager `802-3-ethernet.port` property.
    pub port: Option<PortType>,
    /// The NetworkManager `802-3-ethernet.s390-nettype` property.
    pub s390_nettype: Option<S390NettypeType>,
    /// The NetworkManager `802-3-ethernet.s390-options` property.
    pub s390_options: HashMap<String, String>,
    /// The NetworkManager `802-3-ethernet.s390-subchannels` property.
    pub s390_subchannels: Vec<String>,
    /// The NetworkManager `802-3-ethernet.speed` property.
    pub speed: u32,
    /// The NetworkManager `802-3-ethernet.wake-on-lan` property.
    pub wake_on_lan: u32,
    /// The NetworkManager `802-3-ethernet.wake-on-lan-password` property.
    pub wake_on_lan_password: Option<String>,
}

impl EthernetSection {
    pub(crate) const fn get_name() -> &'static str {
        "802-3-ethernet"
    }
}

impl From<EthernetSection> for HashMap<&str, Value<'_>> {
    fn from(value: EthernetSection) -> Self {
        let mut ethernet = HashMap::new();

        if value.accept_all_mac_addresses != BoolWithDefault::Default {
            insert_value!(
                ethernet,
                "accept-all-mac-addresses",
                i32::from(value.accept_all_mac_addresses)
            );
        }

        insert_opt!(ethernet, "assigned-mac-address", value.assigned_mac_address);

        insert_value!(ethernet, "auto-negotiate", value.auto_negotiate);

        insert_opt!(
            ethernet,
            "cloned-mac-address",
            value.cloned_mac_address.map(String::into_bytes)
        );

        insert_opt!(
            ethernet,
            "duplex",
            value.duplex.map(|duplex| duplex.to_string())
        );

        insert_opt!(
            ethernet,
            "generate-mac-address-mask",
            value.generate_mac_address_mask
        );

        insert_opt!(
            ethernet,
            "mac-address",
            value.mac_address.map(String::into_bytes)
        );

        if !value.mac_address_blacklist.is_empty() {
            insert_value!(
                ethernet,
                "mac-address-blacklist",
                value.mac_address_blacklist
            );
        }

        if !value.mac_address_denylist.is_empty() {
            insert_value!(ethernet, "mac-address-denylist", value.mac_address_denylist);
        }

        if value.mtu != 0 {
            insert_value!(ethernet, "mtu", value.mtu);
        }

        insert_opt!(ethernet, "port", value.port.map(|port| port.to_string()));

        insert_opt!(
            ethernet,
            "s390-nettype",
            value.s390_nettype.map(|nettype| nettype.to_string())
        );

        if !value.s390_options.is_empty() {
            insert_value!(ethernet, "s390-options", value.s390_options.clone());
        }

        if !value.s390_subchannels.is_empty() {
            insert_value!(ethernet, "s390-subchannels", value.s390_subchannels);
        }

        if value.speed != 0 {
            insert_value!(ethernet, "speed", value.speed);
        }

        insert_value!(ethernet, "wake-on-lan", value.wake_on_lan);

        insert_opt!(ethernet, "wake-on-lan-password", value.wake_on_lan_password);

        ethernet
    }
}

impl TryFrom<&HashMap<String, OwnedValue>> for EthernetSection {
    type Error = ConnectionError;
    fn try_from(value: &HashMap<String, OwnedValue>) -> Result<Self, Self::Error> {
        let mut cloned_mac_address = None;
        if let Some(cloned_mac_address_u8) =
            extract_optional_vec_field!(value, "cloned-mac-address", u8)?
        {
            cloned_mac_address = Some(str::from_utf8(&cloned_mac_address_u8)?.into());
        }

        let mut mac_address = None;
        if let Some(mac_address_u8) = extract_optional_vec_field!(value, "mac-address", u8)? {
            mac_address = Some(str::from_utf8(&mac_address_u8)?.into());
        }

        Ok(EthernetSection {
            accept_all_mac_addresses: BoolWithDefault::from(
                extract_field!(value, "accept-all-mac-addresses", i32).unwrap_or(-1),
            ),
            assigned_mac_address: extract_optional_field!(value, "assigned-mac-address", String)?,
            cloned_mac_address,
            auto_negotiate: extract_field!(value, "auto-negotiate", bool).unwrap_or(false),
            duplex: extract_optional_field!(value, "duplex", String)?
                .map(|s| Duplex::from_str(&s))
                .transpose()?,
            generate_mac_address_mask: extract_optional_field!(
                value,
                "generate-mac-address-mask",
                String
            )?,
            mac_address,
            mac_address_blacklist: extract_vec_field_or_default!(
                value,
                "mac-address-blacklist",
                String
            )?,
            mac_address_denylist: extract_vec_field_or_default!(
                value,
                "mac-address-denylist",
                String
            )?,
            mtu: extract_field!(value, "mtu", u32).unwrap_or(0),
            port: extract_optional_field!(value, "port", String)?
                .map(|s| PortType::from_str(&s))
                .transpose()?,
            s390_nettype: extract_optional_field!(value, "s390-nettype", String)?
                .map(|s| S390NettypeType::from_str(&s))
                .transpose()?,
            s390_options: match value.get("s390-options") {
                Some(v) => {
                    let dict = v.downcast_ref::<Dict<'_, '_>>()?;
                    let mut map = HashMap::new();
                    for (k, val) in dict.iter() {
                        let key = match k {
                            Value::Str(s) => s.to_string(),
                            _ => {
                                return Err(ConnectionError::FailedDowncast((
                                    "s390-options",
                                    "string key",
                                )));
                            }
                        };
                        let val_str = match val {
                            Value::Str(s) => s.to_string(),
                            _ => {
                                return Err(ConnectionError::FailedDowncast((
                                    "s390-options",
                                    "string value",
                                )));
                            }
                        };
                        map.insert(key, val_str);
                    }
                    map
                }
                None => HashMap::new(),
            },
            s390_subchannels: extract_vec_field_or_default!(value, "s390-subchannels", String)?,
            speed: extract_field!(value, "speed", u32).unwrap_or(0),
            wake_on_lan: extract_field!(value, "wake-on-lan", u32).unwrap_or(1),
            wake_on_lan_password: extract_optional_field!(value, "wake-on-lan-password", String)?,
        })
    }
}
