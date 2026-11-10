use std::collections::HashMap;

use zbus::zvariant::{OwnedValue, Value};

use crate::api::settings::error::SettingsError as ConnectionError;

/// NetworkManager proxy settings.
#[derive(Debug, Clone)]
pub struct ProxySection {
    /// The NetworkManager `proxy.browser-only` property.
    pub browser_only: bool,
    /// The NetworkManager `proxy.method` property.
    pub method: i32,
    /// The NetworkManager `proxy.pac-script` property.
    pub pac_script: Option<String>,
    /// The NetworkManager `proxy.pac-url` property.
    pub pac_url: Option<String>,
}

impl ProxySection {
    pub(crate) const fn get_name() -> &'static str {
        "proxy"
    }
}

impl From<ProxySection> for HashMap<&str, Value<'_>> {
    fn from(value: ProxySection) -> Self {
        let mut proxy = HashMap::new();
        insert_value!(proxy, "browser-only", value.browser_only);
        insert_value!(proxy, "method", value.method);
        insert_opt!(proxy, "pac-script", value.pac_script);
        insert_opt!(proxy, "pac-url", value.pac_url);
        proxy
    }
}

impl TryFrom<&HashMap<String, OwnedValue>> for ProxySection {
    type Error = ConnectionError;
    fn try_from(value: &HashMap<String, OwnedValue>) -> Result<Self, Self::Error> {
        Ok(ProxySection {
            browser_only: extract_field!(value, "browser-only", bool).unwrap_or(false),
            method: extract_field!(value, "method", i32).unwrap_or(0),
            pac_script: extract_optional_field!(value, "pac-script", String)?,
            pac_url: extract_optional_field!(value, "pac-url", String)?,
        })
    }
}
