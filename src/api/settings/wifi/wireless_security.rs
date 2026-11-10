use std::{collections::HashMap, str::FromStr};

use zbus::zvariant::{OwnedValue, Value};

use crate::api::settings::error::SettingsError as ConnectionError;

string_enum!(WirelessAuthAlg {
    Open => "open",
    Shared => "shared",
    Leap => "leap",
});

string_enum!(GroupCipher {
    Wep40 => "wep40",
    Wep104 => "wep104",
    Tkip => "tkip",
    Ccmp => "ccmp",
});

string_enum!(PairwiseCipher {
    Tkip => "tkip",
    Ccmp => "ccmp",
});

string_enum!(WirelessProtocol {
    Wpa => "wpa",
    Rsn => "rsn",
});

int_enum!(FilsMode: i32 {
    Default = 0,
    Disable = 1,
    Optional = 2,
    Required = 3
} default Default);

int_enum!(PmfMode: i32 {
    Default = 0,
    Disable = 1,
    Optional = 2,
    Required = 3
} default Default);

int_enum!(WepKeyType: u32 {
    NotSpecified = 0,
    Key = 1,
    Passphrase = 2
} default NotSpecified);

int_enum!(WpsMethod: u32 {
    Default = 0,
    Disabled = 1
} default Default);

/// Editable Wi-Fi security settings.
#[derive(Debug, Clone)]
pub struct WirelessSecuritySection {
    /// The NetworkManager `802-11-wireless-security.key-mgmt` property.
    pub key_mgmt: WirelessSecurityKeyMgmt,
    /// The NetworkManager `802-11-wireless-security.psk` property.
    pub psk: Option<String>,
    /// The NetworkManager `802-11-wireless-security.auth-alg` property.
    pub auth_alg: Option<WirelessAuthAlg>,
    /// The NetworkManager `802-11-wireless-security.fils` property.
    pub fils: FilsMode,
    /// The NetworkManager `802-11-wireless-security.group` property.
    pub group: Vec<GroupCipher>,
    /// The NetworkManager `802-11-wireless-security.leap-password` property.
    pub leap_password: Option<String>,
    /// The NetworkManager `802-11-wireless-security.leap-password-flags` property.
    pub leap_password_flags: u32,
    /// The NetworkManager `802-11-wireless-security.leap-username` property.
    pub leap_username: Option<String>,
    /// The NetworkManager `802-11-wireless-security.pairwise` property.
    pub pairwise: Vec<PairwiseCipher>,
    /// The NetworkManager `802-11-wireless-security.pmf` property.
    pub pmf: PmfMode,
    /// The NetworkManager `802-11-wireless-security.proto` property.
    pub proto: Vec<WirelessProtocol>,
    /// The NetworkManager `802-11-wireless-security.psk-flags` property.
    pub psk_flags: u32,
    /// The NetworkManager `802-11-wireless-security.wep-key-type` property.
    pub wep_key_type: WepKeyType,
    /// The NetworkManager `802-11-wireless-security.wep-key0` property.
    pub wep_key0: Option<String>,
    /// The NetworkManager `802-11-wireless-security.wep-key1` property.
    pub wep_key1: Option<String>,
    /// The NetworkManager `802-11-wireless-security.wep-key2` property.
    pub wep_key2: Option<String>,
    /// The NetworkManager `802-11-wireless-security.wep-key3` property.
    pub wep_key3: Option<String>,
    /// The NetworkManager `802-11-wireless-security.wep-key-flags` property.
    pub wep_key_flags: u32,
    /// The NetworkManager `802-11-wireless-security.wep-tx-keyidx` property.
    pub wep_tx_keyidx: u32,
    /// The NetworkManager `802-11-wireless-security.wps-method` property.
    pub wps_method: WpsMethod,
}

impl WirelessSecuritySection {
    pub(crate) const fn get_name() -> &'static str {
        "802-11-wireless-security"
    }
}

impl From<WirelessSecuritySection> for HashMap<&str, Value<'_>> {
    fn from(value: WirelessSecuritySection) -> Self {
        let mut wifi_security = HashMap::new();
        insert_value!(wifi_security, "key-mgmt", value.key_mgmt.to_string());
        insert_opt!(wifi_security, "psk", value.psk);
        insert_opt!(
            wifi_security,
            "auth-alg",
            value.auth_alg.map(|v| v.to_string())
        );
        insert_value!(wifi_security, "fils", i32::from(value.fils));
        if !value.group.is_empty() {
            insert_value!(
                wifi_security,
                "group",
                value
                    .group
                    .iter()
                    .map(ToString::to_string)
                    .collect::<Vec<_>>()
            );
        }
        insert_opt!(wifi_security, "leap-password", value.leap_password);
        insert_value!(
            wifi_security,
            "leap-password-flags",
            value.leap_password_flags
        );
        insert_opt!(wifi_security, "leap-username", value.leap_username);
        if !value.pairwise.is_empty() {
            insert_value!(
                wifi_security,
                "pairwise",
                value
                    .pairwise
                    .iter()
                    .map(ToString::to_string)
                    .collect::<Vec<_>>()
            );
        }
        insert_value!(wifi_security, "pmf", i32::from(value.pmf));
        if !value.proto.is_empty() {
            insert_value!(
                wifi_security,
                "proto",
                value
                    .proto
                    .iter()
                    .map(ToString::to_string)
                    .collect::<Vec<_>>()
            );
        }
        insert_value!(wifi_security, "psk-flags", value.psk_flags);
        insert_value!(wifi_security, "wep-key-type", u32::from(value.wep_key_type));
        insert_opt!(wifi_security, "wep-key0", value.wep_key0);
        insert_opt!(wifi_security, "wep-key1", value.wep_key1);
        insert_opt!(wifi_security, "wep-key2", value.wep_key2);
        insert_opt!(wifi_security, "wep-key3", value.wep_key3);
        insert_value!(wifi_security, "wep-key-flags", value.wep_key_flags);
        insert_value!(wifi_security, "wep-tx-keyidx", value.wep_tx_keyidx);
        insert_value!(wifi_security, "wps-method", u32::from(value.wps_method));
        wifi_security
    }
}

impl TryFrom<&HashMap<String, OwnedValue>> for WirelessSecuritySection {
    type Error = ConnectionError;
    fn try_from(value: &HashMap<String, OwnedValue>) -> Result<Self, Self::Error> {
        Ok(WirelessSecuritySection {
            key_mgmt: WirelessSecurityKeyMgmt::from_str(&extract_field!(
                value, "key-mgmt", String
            )?)?,
            psk: extract_optional_field!(value, "psk", String)?,
            auth_alg: extract_optional_field!(value, "auth-alg", String)?
                .map(|value| WirelessAuthAlg::from_str(&value))
                .transpose()?,
            fils: FilsMode::from(extract_field!(value, "fils", i32).unwrap_or(0)),
            group: extract_vec_field_or_default!(value, "group", String)?
                .iter()
                .map(|value| GroupCipher::from_str(value))
                .collect::<Result<_, _>>()?,
            leap_password: extract_optional_field!(value, "leap-password", String)?,
            leap_password_flags: extract_field!(value, "leap-password-flags", u32).unwrap_or(0),
            leap_username: extract_optional_field!(value, "leap-username", String)?,
            pairwise: extract_vec_field_or_default!(value, "pairwise", String)?
                .iter()
                .map(|value| PairwiseCipher::from_str(value))
                .collect::<Result<_, _>>()?,
            pmf: PmfMode::from(extract_field!(value, "pmf", i32).unwrap_or(0)),
            proto: extract_vec_field_or_default!(value, "proto", String)?
                .iter()
                .map(|value| WirelessProtocol::from_str(value))
                .collect::<Result<_, _>>()?,
            psk_flags: extract_field!(value, "psk-flags", u32).unwrap_or(0),
            wep_key_type: WepKeyType::from(extract_field!(value, "wep-key-type", u32).unwrap_or(0)),
            wep_key0: extract_optional_field!(value, "wep-key0", String)?,
            wep_key1: extract_optional_field!(value, "wep-key1", String)?,
            wep_key2: extract_optional_field!(value, "wep-key2", String)?,
            wep_key3: extract_optional_field!(value, "wep-key3", String)?,
            wep_key_flags: extract_field!(value, "wep-key-flags", u32).unwrap_or(0),
            wep_tx_keyidx: extract_field!(value, "wep-tx-keyidx", u32).unwrap_or(0),
            wps_method: WpsMethod::from(extract_field!(value, "wps-method", u32).unwrap_or(0)),
        })
    }
}

string_enum!(WirelessSecurityKeyMgmt {
    None => "none",
    Ieee8021x => "ieee8021x",
    Owe => "owe",
    Sae => "sae",
    WpaPsk => "wpa-psk",
    WpaEap => "wpa-eap",
    WpaEapSuiteB192 => "wpa-eap-suite-b-192",
});
