//! WiFi connection builder with type-safe API.
//!
//! Provides a fluent builder interface for constructing WiFi connection settings
//! with support for different security modes (Open, WPA-PSK, WPA-EAP).

use std::collections::HashMap;
use zvariant::Value;

use super::connection_builder::ConnectionBuilder;
use crate::api::models::{self, ConnectionOptions, EapMethod};

/// WiFi band selection.
#[non_exhaustive]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WifiBand {
    /// 2.4 GHz band
    Bg,
    /// 5 GHz band
    A,
}

/// WiFi operating mode.
///
/// Determines whether the device acts as a client connecting to an existing
/// network or creates its own network for other devices to join.
#[non_exhaustive]
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub enum WifiMode {
    /// Standard client mode — connects to an existing access point.
    #[default]
    Infrastructure,
    /// Access point mode — the device acts as a WiFi hotspot.
    ///
    /// Typically paired with `.ipv4_shared()` so NetworkManager sets up
    /// DHCP and NAT for connected clients.
    Ap,
    /// Ad-hoc (IBSS) mode — peer-to-peer networking without an access point.
    Adhoc,
}

impl WifiMode {
    fn as_nm_str(self) -> &'static str {
        match self {
            Self::Infrastructure => "infrastructure",
            Self::Ap => "ap",
            Self::Adhoc => "adhoc",
        }
    }
}

/// Builder for WiFi (802.11) connections.
///
/// This builder provides a type-safe, ergonomic API for creating WiFi connection
/// settings. It wraps `ConnectionBuilder` and adds WiFi-specific configuration.
///
/// # Examples
///
/// ## Open Network
///
/// ```rust
/// use nmrs::builders::WifiConnectionBuilder;
///
/// let settings = WifiConnectionBuilder::new("CoffeeShop-WiFi")
///     .open()
///     .autoconnect(true)
///     .build();
/// ```
///
/// ## WPA-PSK (Personal)
///
/// ```rust
/// use nmrs::builders::WifiConnectionBuilder;
///
/// let settings = WifiConnectionBuilder::new("HomeNetwork")
///     .wpa_psk("my_secure_password")
///     .autoconnect(true)
///     .autoconnect_priority(10)
///     .build();
/// ```
///
/// ## WPA-EAP (Enterprise)
///
/// ```rust
/// use nmrs::builders::WifiConnectionBuilder;
/// use nmrs::{EapOptions, EapMethod, Phase2};
///
/// let eap_opts = EapOptions::new("user@company.com", "password")
///     .with_domain_suffix_match("company.com")
///     .with_system_ca_certs(true)
///     .with_method(EapMethod::Peap)
///     .with_phase2(Phase2::Mschapv2);
///
/// let settings = WifiConnectionBuilder::new("CorpNetwork")
///     .wpa_eap(eap_opts)
///     .autoconnect(false)
///     .build();
/// ```
///
/// ## Access Point (Hotspot)
///
/// ```rust
/// use nmrs::builders::{WifiConnectionBuilder, WifiMode};
///
/// let settings = WifiConnectionBuilder::new("MyHotspot")
///     .mode(WifiMode::Ap)
///     .wpa_psk("hotspot_password")
///     .ipv4_shared()
///     .ipv6_ignore()
///     .build();
/// ```
pub struct WifiConnectionBuilder {
    inner: ConnectionBuilder,
    ssid: String,
    mode: WifiMode,
    security_configured: bool,
    hidden: Option<bool>,
    band: Option<WifiBand>,
    bssid: Option<String>,
}

impl WifiConnectionBuilder {
    /// Creates a new WiFi connection builder for the specified SSID.
    ///
    /// By default, the connection is configured as an open network. Use
    /// `.wpa_psk()` or `.wpa_eap()` to add security.
    #[must_use]
    pub fn new(ssid: impl Into<String>) -> Self {
        let ssid = ssid.into();
        let inner = ConnectionBuilder::new("802-11-wireless", &ssid);

        Self {
            inner,
            ssid,
            mode: WifiMode::default(),
            security_configured: false,
            hidden: None,
            band: None,
            bssid: None,
        }
    }

    /// Configures this as an open (unsecured) network.
    ///
    /// This is the default, but can be called explicitly for clarity.
    #[must_use]
    pub fn open(mut self) -> Self {
        self.inner = self
            .inner
            .without_section("802-11-wireless-security")
            .without_section("802-1x");
        self.security_configured = false;
        self
    }

    /// Configures WPA-PSK (Personal) security with the given passphrase.
    ///
    /// Lets NetworkManager negotiate the best protocol (WPA/WPA2/WPA3)
    /// and cipher (TKIP/CCMP) with the access point, supporting mixed-mode
    /// routers that advertise both WPA and WPA2.
    #[must_use]
    pub fn wpa_psk(mut self, psk: impl Into<String>) -> Self {
        let mut security = HashMap::new();
        security.insert("key-mgmt", Value::from("wpa-psk"));
        security.insert("psk", Value::from(psk.into()));
        security.insert("psk-flags", Value::from(0u32));
        security.insert("auth-alg", Value::from("open"));

        self.inner = self
            .inner
            .without_section("802-1x")
            .with_section("802-11-wireless-security", security);
        self.security_configured = true;
        self
    }

    /// Configures WPA-EAP (Enterprise) security with 802.1X authentication.
    ///
    /// Supports PEAP, TTLS, and TLS methods with various inner authentication protocols.
    #[must_use]
    pub fn wpa_eap(self, opts: models::EapOptions) -> Self {
        self.wpa_eap_shared("wpa-eap", opts)
    }

    /// Configures WPA3-EAP (Enterprise) with 192bit security with 802.1X authentication.
    ///
    /// Supports only EAP-TLS.
    #[must_use]
    pub fn wpa3_eap_192_bit(self, opts: models::EapOptions) -> Self {
        self.wpa_eap_shared("wpa-eap-suite-b-192", opts)
    }

    #[must_use]
    fn wpa_eap_shared(mut self, key_mgmt: &'static str, opts: models::EapOptions) -> Self {
        let mut security = HashMap::new();
        security.insert("key-mgmt", Value::from(key_mgmt));
        security.insert("auth-alg", Value::from("open"));

        self.inner = self
            .inner
            .with_section("802-11-wireless-security", security);

        // Build 802.1x section
        let mut e1x = HashMap::new();

        let eap_str = match opts.method {
            EapMethod::Peap => "peap",
            EapMethod::Ttls => "ttls",
            EapMethod::Tls => "tls",
        };
        e1x.insert("eap", Self::string_array(&[eap_str]));
        e1x.insert("identity", Value::from(opts.identity));

        match opts.method {
            EapMethod::Peap | EapMethod::Ttls => {
                e1x.insert("password", Value::from(opts.password));

                if let Some(ai) = opts.anonymous_identity {
                    e1x.insert("anonymous-identity", Value::from(ai));
                }

                let p2 = match opts.phase2 {
                    models::Phase2::Mschapv2 => "mschapv2",
                    models::Phase2::Pap => "pap",
                };
                e1x.insert("phase2-auth", Value::from(p2));
            }
            EapMethod::Tls => {
                if let Some(cert) =
                    Self::path_or_blob("private_key", opts.private_key_path, opts.private_key_blob)
                {
                    e1x.insert("private-key", cert);
                }

                if let Some(password) = opts.private_key_password {
                    e1x.insert("private-key-password", Value::from(password));
                }

                if let Some(cert) =
                    Self::path_or_blob("client_cert", opts.client_cert_path, opts.client_cert_blob)
                {
                    e1x.insert("client-cert", cert);
                }
            }
        }

        if opts.system_ca_certs {
            e1x.insert("system-ca-certs", Value::from(true));
        }
        if let Some(cert) = Self::path_or_blob("ca_cert", opts.ca_cert_path, opts.ca_cert_blob) {
            e1x.insert("ca-cert", cert);
        }
        if let Some(dom) = opts.domain_suffix_match {
            e1x.insert("domain-suffix-match", Value::from(dom));
        }

        self.inner = self.inner.with_section("802-1x", e1x);
        self.security_configured = true;
        self
    }

    /// Marks this network as hidden (doesn't broadcast SSID).
    #[must_use]
    pub fn hidden(mut self, hidden: bool) -> Self {
        self.hidden = Some(hidden);
        self
    }

    /// Restricts connection to a specific WiFi band.
    #[must_use]
    pub fn band(mut self, band: WifiBand) -> Self {
        self.band = Some(band);
        self
    }

    /// Restricts connection to a specific access point by BSSID (MAC address).
    ///
    /// Format: "00:11:22:33:44:55"
    #[must_use]
    pub fn bssid(mut self, bssid: impl Into<String>) -> Self {
        self.bssid = Some(bssid.into());
        self
    }

    /// Sets the WiFi operating mode.
    ///
    /// Defaults to [`WifiMode::Infrastructure`] (standard client mode).
    ///
    /// # Example: Access Point
    ///
    /// ```rust
    /// use nmrs::builders::{WifiConnectionBuilder, WifiMode};
    ///
    /// let settings = WifiConnectionBuilder::new("MyHotspot")
    ///     .mode(WifiMode::Ap)
    ///     .wpa_psk("hotspot_password")
    ///     .ipv4_shared()
    ///     .ipv6_ignore()
    ///     .build();
    /// ```
    #[must_use]
    pub fn mode(mut self, mode: WifiMode) -> Self {
        self.mode = mode;
        self
    }

    // Delegation methods to inner ConnectionBuilder

    /// Applies connection options (autoconnect settings).
    #[must_use]
    pub fn options(mut self, opts: &ConnectionOptions) -> Self {
        self.inner = self.inner.options(opts);
        self
    }

    /// Enables or disables automatic connection.
    #[must_use]
    pub fn autoconnect(mut self, enabled: bool) -> Self {
        self.inner = self.inner.autoconnect(enabled);
        self
    }

    /// Sets autoconnect priority (higher values preferred).
    #[must_use]
    pub fn autoconnect_priority(mut self, priority: i32) -> Self {
        self.inner = self.inner.autoconnect_priority(priority);
        self
    }

    /// Sets autoconnect retry limit.
    #[must_use]
    pub fn autoconnect_retries(mut self, retries: i32) -> Self {
        self.inner = self.inner.autoconnect_retries(retries);
        self
    }

    /// Configures IPv4 to use DHCP.
    #[must_use]
    pub fn ipv4_auto(mut self) -> Self {
        self.inner = self.inner.ipv4_auto();
        self
    }

    /// Configures IPv4 for internet connection sharing (DHCP + NAT).
    ///
    /// This is the typical IPv4 setting for [`WifiMode::Ap`] connections,
    /// where the device provides network access to connected clients.
    #[must_use]
    pub fn ipv4_shared(mut self) -> Self {
        self.inner = self.inner.ipv4_shared();
        self
    }

    /// Configures IPv6 to use SLAAC/DHCPv6.
    #[must_use]
    pub fn ipv6_auto(mut self) -> Self {
        self.inner = self.inner.ipv6_auto();
        self
    }

    /// Disables IPv6.
    #[must_use]
    pub fn ipv6_ignore(mut self) -> Self {
        self.inner = self.inner.ipv6_ignore();
        self
    }

    /// Builds the final connection settings dictionary.
    ///
    /// This method adds the WiFi-specific "802-11-wireless" section and links
    /// it to the security section if configured.
    #[must_use]
    pub fn build(mut self) -> HashMap<&'static str, HashMap<&'static str, Value<'static>>> {
        // Build the 802-11-wireless section
        let mut wireless = HashMap::new();
        wireless.insert("ssid", Value::from(self.ssid.as_bytes().to_vec()));
        wireless.insert("mode", Value::from(self.mode.as_nm_str()));

        // Add optional WiFi settings
        if let Some(hidden) = self.hidden {
            wireless.insert("hidden", Value::from(hidden));
        }

        if let Some(band) = self.band {
            let band_str = match band {
                WifiBand::Bg => "bg",
                WifiBand::A => "a",
            };
            wireless.insert("band", Value::from(band_str));
        }

        if let Some(bssid) = self.bssid {
            wireless.insert("bssid", Value::from(bssid));
        }

        if self.security_configured {
            wireless.insert("security", Value::from("802-11-wireless-security"));
        }

        self.inner = self.inner.with_section("802-11-wireless", wireless);

        self.inner.build()
    }

    // Helper functions

    fn string_array(xs: &[&str]) -> Value<'static> {
        let vals: Vec<String> = xs.iter().map(|s| s.to_string()).collect();
        Value::from(vals)
    }

    fn path_or_blob(
        attribute: &str,
        path: Option<String>,
        blob: Option<Vec<u8>>,
    ) -> Option<Value<'static>> {
        match (path, blob) {
            (None, None) => None,
            (Some(path), None) => Some(Self::path(path)),
            (None, Some(blob)) => Some(Self::blob(blob)),
            (Some(_), Some(_)) => {
                panic!("Cannot specify both {attribute}_path and {attribute}_blob.");
            }
        }
    }

    fn path(mut value: String) -> Value<'static> {
        value.push('\0');
        Value::from(value.into_bytes())
    }

    fn blob(value: Vec<u8>) -> Value<'static> {
        Value::from(value)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::{EapOptions, Phase2};

    #[test]
    fn builds_open_wifi() {
        let settings = WifiConnectionBuilder::new("OpenNetwork")
            .open()
            .autoconnect(true)
            .ipv4_auto()
            .ipv6_auto()
            .build();

        assert!(settings.contains_key("connection"));
        assert!(settings.contains_key("802-11-wireless"));
        assert!(settings.contains_key("ipv4"));
        assert!(settings.contains_key("ipv6"));
        assert!(!settings.contains_key("802-11-wireless-security"));
        assert!(!settings.contains_key("802-1x"));

        let wireless = settings.get("802-11-wireless").unwrap();
        assert_eq!(
            wireless.get("ssid"),
            Some(&Value::from(b"OpenNetwork".to_vec()))
        );
        assert_eq!(wireless.get("mode"), Some(&Value::from("infrastructure")));
        assert!(
            !wireless.contains_key("security"),
            "open Wi-Fi must not reference a security section"
        );
    }

    #[test]
    fn open_overrides_previously_configured_security() {
        let settings = WifiConnectionBuilder::new("OpenNetwork")
            .wpa_psk("password123")
            .open()
            .build();

        assert!(!settings.contains_key("802-11-wireless-security"));
        assert!(!settings.contains_key("802-1x"));
        assert!(
            !settings["802-11-wireless"].contains_key("security"),
            "open() must remove the security link as well as its sections"
        );
    }

    #[test]
    fn builds_wpa_psk_wifi() {
        let settings = WifiConnectionBuilder::new("SecureNet")
            .wpa_psk("password123")
            .ipv4_auto()
            .ipv6_auto()
            .build();

        assert!(settings.contains_key("802-11-wireless-security"));

        let security = settings.get("802-11-wireless-security").unwrap();
        assert_eq!(security.get("key-mgmt"), Some(&Value::from("wpa-psk")));
        assert_eq!(
            security.get("psk"),
            Some(&Value::from("password123".to_string()))
        );

        // proto/pairwise/group must not be set so NetworkManager can
        // negotiate with mixed-mode (WPA1+WPA2) access points.
        assert!(security.get("proto").is_none());
        assert!(security.get("pairwise").is_none());
        assert!(security.get("group").is_none());

        let wireless = settings.get("802-11-wireless").unwrap();
        assert_eq!(
            wireless.get("security"),
            Some(&Value::from("802-11-wireless-security"))
        );
    }

    #[test]
    fn builds_wpa_eap_wifi() {
        let eap_opts = EapOptions {
            identity: "user@example.com".into(),
            password: "secret".into(),
            anonymous_identity: Some("anon@example.com".into()),
            domain_suffix_match: Some("example.com".into()),
            ca_cert_path: None,
            ca_cert_blob: None,
            system_ca_certs: true,
            method: EapMethod::Peap,
            phase2: Phase2::Mschapv2,
            private_key_path: None,
            private_key_blob: None,
            private_key_password: None,
            client_cert_path: None,
            client_cert_blob: None,
        };

        let settings = WifiConnectionBuilder::new("Enterprise")
            .wpa_eap(eap_opts)
            .autoconnect(false)
            .ipv4_auto()
            .ipv6_auto()
            .build();

        assert!(settings.contains_key("802-11-wireless-security"));
        assert!(settings.contains_key("802-1x"));

        let security = settings.get("802-11-wireless-security").unwrap();
        assert_eq!(security.get("key-mgmt"), Some(&Value::from("wpa-eap")));

        let e1x = settings.get("802-1x").unwrap();
        assert_eq!(
            e1x.get("identity"),
            Some(&Value::from("user@example.com".to_string()))
        );
        assert_eq!(e1x.get("phase2-auth"), Some(&Value::from("mschapv2")));
    }

    #[test]
    fn configures_hidden_network() {
        let settings = WifiConnectionBuilder::new("HiddenSSID")
            .open()
            .hidden(true)
            .ipv4_auto()
            .build();

        let wireless = settings.get("802-11-wireless").unwrap();
        assert_eq!(wireless.get("hidden"), Some(&Value::from(true)));
    }

    #[test]
    fn configures_specific_band() {
        let settings = WifiConnectionBuilder::new("5GHz-Only")
            .open()
            .band(WifiBand::A)
            .ipv4_auto()
            .build();

        let wireless = settings.get("802-11-wireless").unwrap();
        assert_eq!(wireless.get("band"), Some(&Value::from("a")));
    }

    #[test]
    fn configures_bssid() {
        let settings = WifiConnectionBuilder::new("SpecificAP")
            .open()
            .bssid("00:11:22:33:44:55")
            .ipv4_auto()
            .build();

        let wireless = settings.get("802-11-wireless").unwrap();
        assert_eq!(
            wireless.get("bssid"),
            Some(&Value::from("00:11:22:33:44:55"))
        );
    }

    #[test]
    fn defaults_to_infrastructure_mode() {
        let settings = WifiConnectionBuilder::new("DefaultMode")
            .open()
            .ipv4_auto()
            .build();

        let wireless = settings.get("802-11-wireless").unwrap();
        assert_eq!(wireless.get("mode"), Some(&Value::from("infrastructure")));
    }

    #[test]
    fn builds_ap_mode_hotspot() {
        let settings = WifiConnectionBuilder::new("MyHotspot")
            .mode(WifiMode::Ap)
            .wpa_psk("hotspot_pass")
            .ipv4_shared()
            .ipv6_ignore()
            .build();

        let wireless = settings.get("802-11-wireless").unwrap();
        assert_eq!(wireless.get("mode"), Some(&Value::from("ap")));

        let ipv4 = settings.get("ipv4").unwrap();
        assert_eq!(ipv4.get("method"), Some(&Value::from("shared")));

        let ipv6 = settings.get("ipv6").unwrap();
        assert_eq!(ipv6.get("method"), Some(&Value::from("ignore")));
    }

    #[test]
    fn builds_adhoc_mode() {
        let settings = WifiConnectionBuilder::new("PeerNet")
            .mode(WifiMode::Adhoc)
            .open()
            .ipv4_auto()
            .build();

        let wireless = settings.get("802-11-wireless").unwrap();
        assert_eq!(wireless.get("mode"), Some(&Value::from("adhoc")));
    }

    #[test]
    fn applies_connection_options() {
        let opts = ConnectionOptions {
            autoconnect: false,
            autoconnect_priority: Some(5),
            autoconnect_retries: Some(3),
        };

        let settings = WifiConnectionBuilder::new("TestNet")
            .open()
            .options(&opts)
            .ipv4_auto()
            .build();

        let conn = settings.get("connection").unwrap();
        assert_eq!(conn.get("autoconnect"), Some(&Value::from(false)));
        assert_eq!(conn.get("autoconnect-priority"), Some(&Value::from(5i32)));
    }
}
