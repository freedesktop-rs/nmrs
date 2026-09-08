//! Global DNS override exposed as NetworkManager.GlobalDnsConfiguration.

use std::collections::HashMap;
use zvariant::{OwnedValue, Value};

/// One domain entry under `domains`.
#[non_exhaustive]
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct GlobalDnsDomain {
    pub servers: Vec<String>,
    pub options: Vec<String>,
}

impl GlobalDnsDomain {
    /// Empty domain entry.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Sets this domain's nameservers.
    #[must_use]
    pub fn with_servers(mut self, servers: impl Into<Vec<String>>) -> Self {
        self.servers = servers.into();
        self
    }

    /// Sets this domain's resolver options.
    #[must_use]
    pub fn with_options(mut self, options: impl Into<Vec<String>>) -> Self {
        self.options = options.into();
        self
    }
}

/// Typed form of NetworkManager's `GlobalDnsConfiguration` property.
///
/// An empty value (no searches, options, or domain servers) serializes to an
/// empty dict and clears the global override.
#[non_exhaustive]
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct GlobalDnsConfiguration {
    pub searches: Vec<String>,
    pub options: Vec<String>,
    pub domains: HashMap<String, GlobalDnsDomain>,
}

impl GlobalDnsConfiguration {
    /// Empty configuration; writing it clears the global override.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Default-domain nameservers only (`domains["*"].servers`).
    #[must_use]
    pub fn from_servers(servers: impl Into<Vec<String>>) -> Self {
        Self::new().with_default_servers(servers)
    }

    /// Sets the global search domains.
    #[must_use]
    pub fn with_searches(mut self, searches: impl Into<Vec<String>>) -> Self {
        self.searches = searches.into();
        self
    }

    /// Sets the global resolver options.
    #[must_use]
    pub fn with_options(mut self, options: impl Into<Vec<String>>) -> Self {
        self.options = options.into();
        self
    }

    /// Inserts or replaces one domain entry.
    #[must_use]
    pub fn with_domain(mut self, name: impl Into<String>, domain: GlobalDnsDomain) -> Self {
        self.domains.insert(name.into(), domain);
        self
    }

    /// Sets nameservers for the default `"*"` domain.
    #[must_use]
    pub fn with_default_servers(self, servers: impl Into<Vec<String>>) -> Self {
        self.with_domain("*", GlobalDnsDomain::new().with_servers(servers))
    }

    /// `true` when writing this value should send an empty dict.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.searches.is_empty()
            && self.options.is_empty()
            && self
                .domains
                .values()
                .all(|domain| domain.servers.is_empty() && domain.options.is_empty())
    }

    /// Nameservers configured on the default `"*"` domain.
    #[must_use]
    pub fn default_servers(&self) -> &[String] {
        self.domains
            .get("*")
            .map(|domain| domain.servers.as_slice())
            .unwrap_or(&[])
    }

    pub(crate) fn to_dbus(&self) -> HashMap<&'static str, Value<'static>> {
        if self.is_empty() {
            return HashMap::new();
        }

        let mut map = HashMap::new();
        if !self.searches.is_empty() {
            map.insert("searches", Value::from(self.searches.clone()));
        }
        if !self.options.is_empty() {
            map.insert("options", Value::from(self.options.clone()));
        }
        if !self.domains.is_empty() {
            let mut domains = HashMap::new();
            for (name, domain) in &self.domains {
                domains.insert(name.clone(), domain_to_dbus(domain));
            }
            map.insert("domains", Value::from(domains));
        }
        map
    }

    pub(crate) fn from_dbus(map: &HashMap<String, OwnedValue>) -> Self {
        let searches = take_strings(map, "searches");
        let options = take_strings(map, "options");
        let mut domains = HashMap::new();

        if let Some(value) = map.get("domains")
            && let Ok(raw_domains) = HashMap::<String, OwnedValue>::try_from(value.clone())
        {
            for (name, raw_domain) in raw_domains {
                let inner = HashMap::<String, OwnedValue>::try_from(raw_domain).unwrap_or_default();
                domains.insert(
                    name,
                    GlobalDnsDomain {
                        servers: take_strings(&inner, "servers"),
                        options: take_strings(&inner, "options"),
                    },
                );
            }
        }

        Self {
            searches,
            options,
            domains,
        }
    }
}

fn domain_to_dbus(domain: &GlobalDnsDomain) -> Value<'static> {
    let mut inner: HashMap<&str, Value<'static>> = HashMap::new();
    if !domain.servers.is_empty() {
        inner.insert("servers", Value::from(domain.servers.clone()));
    }
    if !domain.options.is_empty() {
        inner.insert("options", Value::from(domain.options.clone()));
    }
    Value::from(inner)
}

fn take_strings(map: &HashMap<String, OwnedValue>, key: &str) -> Vec<String> {
    map.get(key)
        .and_then(|value| Vec::<String>::try_from(value.clone()).ok())
        .unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;
    use zvariant::Str;

    #[test]
    fn empty_encodes_as_empty_dict() {
        let encoded = GlobalDnsConfiguration::default().to_dbus();
        assert!(encoded.is_empty());
    }

    #[test]
    fn empty_dict_decodes_as_empty_config() {
        let decoded = GlobalDnsConfiguration::from_dbus(&HashMap::new());
        assert!(decoded.is_empty());
        assert!(decoded.default_servers().is_empty());
    }

    #[test]
    fn default_servers_round_trip() {
        let original =
            GlobalDnsConfiguration::from_servers(vec!["1.1.1.1".into(), "8.8.8.8".into()]);

        let encoded = original.to_dbus();
        assert!(!encoded.contains_key("searches"));
        assert!(encoded.contains_key("domains"));

        let owned: HashMap<String, OwnedValue> = encoded
            .into_iter()
            .map(|(k, v)| (k.to_string(), OwnedValue::try_from(v).expect("owned value")))
            .collect();
        let decoded = GlobalDnsConfiguration::from_dbus(&owned);

        assert_eq!(decoded.default_servers(), &["1.1.1.1", "8.8.8.8"]);
        assert!(decoded.searches.is_empty());
        assert!(decoded.options.is_empty());
    }

    #[test]
    fn searches_options_and_split_domain_round_trip() {
        let original = GlobalDnsConfiguration::new()
            .with_searches(vec!["example.test".into()])
            .with_options(vec!["timeout:2".into()])
            .with_default_servers(vec!["9.9.9.9".into()])
            .with_domain(
                "corp.example",
                GlobalDnsDomain::new().with_servers(vec!["10.0.0.1".into()]),
            );

        let owned: HashMap<String, OwnedValue> = original
            .to_dbus()
            .into_iter()
            .map(|(k, v)| (k.to_string(), OwnedValue::try_from(v).expect("owned value")))
            .collect();
        let decoded = GlobalDnsConfiguration::from_dbus(&owned);

        assert_eq!(decoded, original);
    }

    #[test]
    fn missing_keys_decode_as_empty() {
        let decoded = GlobalDnsConfiguration::from_dbus(&HashMap::from([(
            "unrelated".into(),
            OwnedValue::from(Str::from("x")),
        )]));
        assert!(decoded.is_empty());
    }
}
