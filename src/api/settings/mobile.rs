use crate::api::settings::error::SettingsError as ConnectionError;
use std::collections::HashMap;
use zbus::zvariant::{OwnedValue, Value};

/// Editable NetworkManager GSM settings.
#[derive(Debug, Clone)]
pub struct GsmSection {
    /// The NetworkManager `gsm.apn` property.
    pub apn: Option<String>,
    /// The NetworkManager `gsm.device-id` property.
    pub device_id: Option<String>,
    /// The NetworkManager `gsm.device-uid` property.
    pub device_uid: Option<String>,
    /// The NetworkManager `gsm.network-id` property.
    pub network_id: Option<String>,
    /// The NetworkManager `gsm.number` property.
    pub number: Option<String>,
    /// The NetworkManager `gsm.username` property.
    pub username: Option<String>,
    /// The NetworkManager `gsm.password` property.
    pub password: Option<String>,
    /// The NetworkManager `gsm.pin` property.
    pub pin: Option<String>,
    /// The NetworkManager `gsm.sim-id` property.
    pub sim_id: Option<String>,
    /// The NetworkManager `gsm.sim-operator-id` property.
    pub sim_operator_id: Option<String>,
    /// The NetworkManager `gsm.initial-eps-bearer-apn` property.
    pub initial_eps_bearer_apn: Option<String>,
    /// The NetworkManager `gsm.initial-eps-bearer-password` property.
    pub initial_eps_bearer_password: Option<String>,
    /// The NetworkManager `gsm.initial-eps-bearer-username` property.
    pub initial_eps_bearer_username: Option<String>,
    /// The NetworkManager `gsm.auto-config` property.
    pub auto_config: bool,
    /// The NetworkManager `gsm.home-only` property.
    pub home_only: bool,
    /// The NetworkManager `gsm.initial-eps-bearer-configure` property.
    pub initial_eps_bearer_configure: bool,
    /// The NetworkManager `gsm.initial-eps-bearer-noauth` property.
    pub initial_eps_bearer_noauth: bool,
    /// The NetworkManager `gsm.initial-eps-bearer-refuse-chap` property.
    pub initial_eps_bearer_refuse_chap: bool,
    /// The NetworkManager `gsm.initial-eps-bearer-refuse-eap` property.
    pub initial_eps_bearer_refuse_eap: bool,
    /// The NetworkManager `gsm.initial-eps-bearer-refuse-mschap` property.
    pub initial_eps_bearer_refuse_mschap: bool,
    /// The NetworkManager `gsm.initial-eps-bearer-refuse-mschapv2` property.
    pub initial_eps_bearer_refuse_mschapv2: bool,
    /// The NetworkManager `gsm.initial-eps-bearer-refuse-pap` property.
    pub initial_eps_bearer_refuse_pap: bool,
    /// The NetworkManager `gsm.mtu` property.
    pub mtu: u32,
    /// The NetworkManager `gsm.password-flags` property.
    pub password_flags: u32,
    /// The NetworkManager `gsm.pin-flags` property.
    pub pin_flags: u32,
    /// The NetworkManager `gsm.initial-eps-bearer-password-flags` property.
    pub initial_eps_bearer_password_flags: u32,
}
/// Editable NetworkManager CDMA settings.
#[derive(Debug, Clone)]
pub struct CdmaSection {
    /// The NetworkManager `cdma.number` property.
    pub number: Option<String>,
    /// The NetworkManager `cdma.username` property.
    pub username: Option<String>,
    /// The NetworkManager `cdma.password` property.
    pub password: Option<String>,
    /// The NetworkManager `cdma.mtu` property.
    pub mtu: u32,
    /// The NetworkManager `cdma.password-flags` property.
    pub password_flags: u32,
}

macro_rules! opt {
    ($m:expr, $k:literal) => {
        extract_optional_field!($m, $k, String)?
    };
}
impl TryFrom<&HashMap<String, OwnedValue>> for GsmSection {
    type Error = ConnectionError;
    fn try_from(v: &HashMap<String, OwnedValue>) -> Result<Self, Self::Error> {
        Ok(Self {
            apn: opt!(v, "apn"),
            device_id: opt!(v, "device-id"),
            device_uid: opt!(v, "device-uid"),
            network_id: opt!(v, "network-id"),
            number: opt!(v, "number"),
            username: opt!(v, "username"),
            password: opt!(v, "password"),
            pin: opt!(v, "pin"),
            sim_id: opt!(v, "sim-id"),
            sim_operator_id: opt!(v, "sim-operator-id"),
            initial_eps_bearer_apn: opt!(v, "initial-eps-bearer-apn"),
            initial_eps_bearer_password: opt!(v, "initial-eps-bearer-password"),
            initial_eps_bearer_username: opt!(v, "initial-eps-bearer-username"),
            auto_config: extract_field!(v, "auto-config", bool).unwrap_or(true),
            home_only: extract_field!(v, "home-only", bool).unwrap_or(false),
            initial_eps_bearer_configure: extract_field!(v, "initial-eps-bearer-configure", bool)
                .unwrap_or(false),
            initial_eps_bearer_noauth: extract_field!(v, "initial-eps-bearer-noauth", bool)
                .unwrap_or(false),
            initial_eps_bearer_refuse_chap: extract_field!(
                v,
                "initial-eps-bearer-refuse-chap",
                bool
            )
            .unwrap_or(false),
            initial_eps_bearer_refuse_eap: extract_field!(v, "initial-eps-bearer-refuse-eap", bool)
                .unwrap_or(false),
            initial_eps_bearer_refuse_mschap: extract_field!(
                v,
                "initial-eps-bearer-refuse-mschap",
                bool
            )
            .unwrap_or(false),
            initial_eps_bearer_refuse_mschapv2: extract_field!(
                v,
                "initial-eps-bearer-refuse-mschapv2",
                bool
            )
            .unwrap_or(false),
            initial_eps_bearer_refuse_pap: extract_field!(v, "initial-eps-bearer-refuse-pap", bool)
                .unwrap_or(false),
            mtu: extract_field!(v, "mtu", u32).unwrap_or(0),
            password_flags: extract_field!(v, "password-flags", u32).unwrap_or(0),
            pin_flags: extract_field!(v, "pin-flags", u32).unwrap_or(0),
            initial_eps_bearer_password_flags: extract_field!(
                v,
                "initial-eps-bearer-password-flags",
                u32
            )
            .unwrap_or(0),
        })
    }
}
impl TryFrom<&HashMap<String, OwnedValue>> for CdmaSection {
    type Error = ConnectionError;
    fn try_from(v: &HashMap<String, OwnedValue>) -> Result<Self, Self::Error> {
        Ok(Self {
            number: opt!(v, "number"),
            username: opt!(v, "username"),
            password: opt!(v, "password"),
            mtu: extract_field!(v, "mtu", u32).unwrap_or(0),
            password_flags: extract_field!(v, "password-flags", u32).unwrap_or(0),
        })
    }
}
impl From<GsmSection> for HashMap<&'static str, Value<'static>> {
    fn from(v: GsmSection) -> Self {
        let mut m = HashMap::new();
        insert_opt!(m, "apn", v.apn);
        insert_opt!(m, "device-id", v.device_id);
        insert_opt!(m, "device-uid", v.device_uid);
        insert_opt!(m, "network-id", v.network_id);
        insert_opt!(m, "number", v.number);
        insert_opt!(m, "username", v.username);
        insert_opt!(m, "password", v.password);
        insert_opt!(m, "pin", v.pin);
        insert_opt!(m, "sim-id", v.sim_id);
        insert_opt!(m, "sim-operator-id", v.sim_operator_id);
        insert_opt!(m, "initial-eps-bearer-apn", v.initial_eps_bearer_apn);
        insert_opt!(
            m,
            "initial-eps-bearer-password",
            v.initial_eps_bearer_password
        );
        insert_opt!(
            m,
            "initial-eps-bearer-username",
            v.initial_eps_bearer_username
        );
        insert_value!(m, "auto-config", v.auto_config);
        insert_value!(m, "home-only", v.home_only);
        insert_value!(
            m,
            "initial-eps-bearer-configure",
            v.initial_eps_bearer_configure
        );
        insert_value!(m, "initial-eps-bearer-noauth", v.initial_eps_bearer_noauth);
        insert_value!(
            m,
            "initial-eps-bearer-refuse-chap",
            v.initial_eps_bearer_refuse_chap
        );
        insert_value!(
            m,
            "initial-eps-bearer-refuse-eap",
            v.initial_eps_bearer_refuse_eap
        );
        insert_value!(
            m,
            "initial-eps-bearer-refuse-mschap",
            v.initial_eps_bearer_refuse_mschap
        );
        insert_value!(
            m,
            "initial-eps-bearer-refuse-mschapv2",
            v.initial_eps_bearer_refuse_mschapv2
        );
        insert_value!(
            m,
            "initial-eps-bearer-refuse-pap",
            v.initial_eps_bearer_refuse_pap
        );
        insert_value!(m, "mtu", v.mtu);
        insert_value!(m, "password-flags", v.password_flags);
        insert_value!(m, "pin-flags", v.pin_flags);
        insert_value!(
            m,
            "initial-eps-bearer-password-flags",
            v.initial_eps_bearer_password_flags
        );
        m
    }
}
impl From<CdmaSection> for HashMap<&'static str, Value<'static>> {
    fn from(v: CdmaSection) -> Self {
        let mut m = HashMap::new();
        insert_opt!(m, "number", v.number);
        insert_opt!(m, "username", v.username);
        insert_opt!(m, "password", v.password);
        insert_value!(m, "mtu", v.mtu);
        insert_value!(m, "password-flags", v.password_flags);
        m
    }
}
define_typed_profile!(GsmSettings, GsmSection, gsm, "gsm");
define_typed_profile!(CdmaSettings, CdmaSection, cdma, "cdma");
