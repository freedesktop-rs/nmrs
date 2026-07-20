use serde::{Deserialize, Serialize};

use super::access_point::{AccessPoint, SecurityFeatures};
use super::error::ConnectionError;
use super::saved_connection::SavedConnectionBrief;

/// Visible Wi-Fi access points grouped by interface and SSID for applet UIs.
///
/// A group preserves every BSSID seen for one `(interface, ssid)` pair while
/// exposing the strongest AP as the representative row.
#[non_exhaustive]
#[derive(Debug, Clone)]
pub struct WifiNetworkGroup {
    /// SSID shared by every access point in this group.
    pub ssid: String,
    /// Wi-Fi interface that sees this group.
    pub interface: String,
    /// Strongest AP in the group, preferring the active BSSID on ties.
    pub strongest: AccessPoint,
    /// All APs in the group, sorted strongest first.
    pub access_points: Vec<AccessPoint>,
    /// Saved Wi-Fi profiles that match this visible group.
    pub saved_profiles: Vec<SavedConnectionBrief>,
    /// `true` when a typed active Wi-Fi connection matches this group.
    pub active: bool,
    /// `true` when at least one saved profile matches this visible group.
    pub known: bool,
}

/// Represents a Wi-Fi network discovered during a scan.
///
/// This struct contains information about a WiFi network that was discovered
/// by NetworkManager during a scan operation.
///
/// # Examples
///
/// ```no_run
/// use nmrs::NetworkManager;
///
/// # async fn example() -> nmrs::Result<()> {
/// let nm = NetworkManager::new().await?;
///
/// // Scan for networks (None = all Wi-Fi devices)
/// nm.scan_networks(None).await?;
/// let networks = nm.list_networks(None).await?;
///
/// for net in networks {
///     println!("SSID: {}", net.ssid);
///     println!("  Signal: {}%", net.strength.unwrap_or(0));
///     println!("  Secured: {}", net.secured);
///     
///     if let Some(freq) = net.frequency {
///         let band = if freq > 5000 { "5GHz" } else { "2.4GHz" };
///         println!("  Band: {}", band);
///     }
/// }
/// # Ok(())
/// # }
/// ```
#[non_exhaustive]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Network {
    /// Device interface name (e.g., "wlan0")
    pub device: String,
    /// Network SSID (name)
    pub ssid: String,
    /// Access point MAC address (BSSID)
    pub bssid: Option<String>,
    /// Signal strength (0-100)
    pub strength: Option<u8>,
    /// Frequency in MHz (e.g., 2437 for channel 6)
    pub frequency: Option<u32>,
    /// Whether the network requires authentication
    pub secured: bool,
    /// Whether the network uses WPA-PSK authentication
    pub is_psk: bool,
    /// Whether the network uses WPA-EAP (Enterprise) authentication
    pub is_eap: bool,
    /// Whether the access point is operating in AP (hotspot) mode
    pub is_hotspot: bool,
    /// Assigned IPv4 address with CIDR notation (only present when connected)
    pub ip4_address: Option<String>,
    /// Assigned IPv6 address with CIDR notation (only present when connected)
    pub ip6_address: Option<String>,
    /// BSSID of the strongest AP for this SSID.
    #[serde(default)]
    pub best_bssid: String,
    /// All known BSSIDs for this SSID, strongest first.
    #[serde(default)]
    pub bssids: Vec<String>,
    /// `true` if this network is currently active (connected).
    #[serde(default)]
    pub is_active: bool,
    /// `true` if a saved connection profile exists for this SSID.
    #[serde(default)]
    pub known: bool,
    /// Decoded security capabilities from NM flag triplet.
    #[serde(default)]
    pub security_features: SecurityFeatures,
}

/// Detailed information about a Wi-Fi network.
///
/// Contains comprehensive information about a WiFi network, including
/// connection status, signal quality, and technical details.
///
/// # Examples
///
/// ```no_run
/// use nmrs::NetworkManager;
///
/// # async fn example() -> nmrs::Result<()> {
/// let nm = NetworkManager::new().await?;
/// let networks = nm.list_networks(None).await?;
///
/// if let Some(network) = networks.first() {
///     let info = nm.show_details(network).await?;
///     
///     println!("Network: {}", info.ssid);
///     println!("Signal: {} {}", info.strength, info.bars);
///     println!("Security: {}", info.security);
///     println!("Status: {}", info.status);
///     
///     if let Some(rate) = info.rate_mbps {
///         println!("Speed: {} Mbps", rate);
///     }
/// }
/// # Ok(())
/// # }
/// ```
#[non_exhaustive]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NetworkInfo {
    /// Network SSID (name)
    pub ssid: String,
    /// Access point MAC address (BSSID)
    pub bssid: String,
    /// Signal strength (0-100)
    pub strength: u8,
    /// Frequency in MHz
    pub freq: Option<u32>,
    /// WiFi channel number
    pub channel: Option<u16>,
    /// Operating mode (e.g., "infrastructure")
    pub mode: String,
    /// Connection speed in Mbps
    pub rate_mbps: Option<u32>,
    /// Visual signal strength representation (e.g., "▂▄▆█")
    pub bars: String,
    /// Security type description
    pub security: String,
    /// Connection status
    pub status: String,
    /// Assigned IPv4 address with CIDR notation (only present when connected)
    pub ip4_address: Option<String>,
    /// Assigned IPv6 address with CIDR notation (only present when connected)
    pub ip6_address: Option<String>,
}

/// EAP (Extensible Authentication Protocol) method for WPA-Enterprise Wi-Fi.
///
/// These are the outer authentication methods used in 802.1X authentication.
#[non_exhaustive]
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EapMethod {
    /// Protected EAP (PEAPv0) - tunnels inner authentication in TLS.
    /// Most commonly used with MSCHAPv2 inner authentication.
    Peap,
    /// Tunneled TLS (EAP-TTLS) - similar to PEAP but more flexible.
    /// Can use various inner authentication methods like PAP or MSCHAPv2.
    Ttls,
    /// TLS (EAP-TLS) - uses certificates for client authentication.
    Tls,
}

/// Phase 2 (inner) authentication methods for EAP connections.
///
/// These methods run inside the TLS tunnel established by the outer
/// EAP method (PEAP or TTLS).
#[non_exhaustive]
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Phase2 {
    /// Microsoft Challenge Handshake Authentication Protocol v2.
    /// More secure than PAP, commonly used with PEAP.
    Mschapv2,
    /// Password Authentication Protocol.
    /// Simple plaintext password (protected by TLS tunnel).
    /// Often used with TTLS.
    Pap,
}

/// EAP options for WPA-EAP (Enterprise) Wi-Fi connections.
///
/// Configuration for 802.1X authentication, commonly used in corporate
/// and educational networks.
///
/// # Examples
///
/// ## PEAP with MSCHAPv2 (Common Corporate Setup)
///
/// ```rust
/// use nmrs::{EapOptions, EapMethod, Phase2};
///
/// let opts = EapOptions::new("employee@company.com", "my_password")
///     .with_anonymous_identity("anonymous@company.com")
///     .with_domain_suffix_match("company.com")
///     .with_system_ca_certs(true)  // Use system certificate store
///     .with_method(EapMethod::Peap)
///     .with_phase2(Phase2::Mschapv2);
/// ```
///
/// ## TTLS with PAP (Alternative Setup)
///
/// ```rust
/// use nmrs::{EapOptions, EapMethod, Phase2};
///
/// let opts = EapOptions::new("student@university.edu", "password")
///     .with_ca_cert_path("file:///etc/ssl/certs/university-ca.pem")
///     .with_method(EapMethod::Ttls)
///     .with_phase2(Phase2::Pap);
/// ```
#[non_exhaustive]
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EapOptions {
    /// User identity (usually email or username)
    pub identity: String,
    /// PEAP/TTLS: Password for authentication
    pub password: String,
    /// PEAP/TTLS: Anonymous outer identity (for privacy)
    pub anonymous_identity: Option<String>,
    /// Domain to match against server certificate
    pub domain_suffix_match: Option<String>,
    /// Path to CA certificate file (file:// URL), mutually exclusive with `ca_cert_blob`
    pub ca_cert_path: Option<String>,
    /// CA certificate encoded as DER, mutually exclusive with `ca_cert_path`
    pub ca_cert_blob: Option<Vec<u8>>,
    /// Use system CA certificate store
    pub system_ca_certs: bool,
    /// EAP method (PEAP or TTLS)
    pub method: EapMethod,
    /// PEAP/TTLS: Phase 2 inner authentication method
    pub phase2: Phase2,
    /// TLS: Path to the private key file of the client certificate (file:// URL), mutually exclusive with `private_key_blob`
    pub private_key_path: Option<String>,
    /// TLS: Private key of the client certificate encoded as PEM or PKCS#12, mutually exclusive with `private_key_path`
    pub private_key_blob: Option<Vec<u8>>,
    /// TLS: Password for the private key file
    pub private_key_password: Option<String>,
    /// TLS: Path to the client certificate file (file:// URL), mutually exclusive with `client_cert_blob`
    pub client_cert_path: Option<String>,
    /// TLS: Client certificate encoded as DER or PKCS#12, mutually exclusive with `client_cert_path`
    pub client_cert_blob: Option<Vec<u8>>,
}

impl Default for EapOptions {
    fn default() -> Self {
        Self {
            identity: String::new(),
            password: String::new(),
            anonymous_identity: None,
            domain_suffix_match: None,
            ca_cert_path: None,
            ca_cert_blob: None,
            system_ca_certs: false,
            method: EapMethod::Peap,
            phase2: Phase2::Mschapv2,
            private_key_path: None,
            private_key_blob: None,
            private_key_password: None,
            client_cert_path: None,
            client_cert_blob: None,
        }
    }
}

impl EapOptions {
    /// Creates a new `EapOptions` with the minimum required fields.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use nmrs::{EapOptions, EapMethod, Phase2};
    ///
    /// let opts = EapOptions::new("user@example.com", "password")
    ///     .with_method(EapMethod::Peap)
    ///     .with_phase2(Phase2::Mschapv2);
    /// ```
    pub fn new(identity: impl Into<String>, password: impl Into<String>) -> Self {
        Self {
            identity: identity.into(),
            password: password.into(),
            ..Default::default()
        }
    }

    /// Creates a new `EapOptions` with the minimum required fields for EAP-TLS.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use nmrs::{EapOptions, EapMethod};
    ///
    /// let opts = EapOptions::new_tls_path("user@example.com", "file:///etc/ssl/private/client.key", "file:///etc/ssl/certs/client.crt")
    ///     .with_private_key_password("password")
    ///     .with_ca_cert_path("file:///etc/ssl/certs/ca.pem");
    /// ```
    pub fn new_tls_path(
        identity: impl Into<String>,
        private_key_path: impl Into<String>,
        client_cert_path: impl Into<String>,
    ) -> Self {
        Self {
            identity: identity.into(),
            method: EapMethod::Tls,
            private_key_path: Some(private_key_path.into()),
            client_cert_path: Some(client_cert_path.into()),
            ..Default::default()
        }
    }

    /// Creates a new `EapOptions` with the minimum required fields for EAP-TLS.
    ///
    /// Private key must be in PEM or PKCS#12 format.
    /// Certificate must be in DER or PKCS#12 format.
    /// CA certificate must be in DER format.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use nmrs::{EapOptions, EapMethod};
    ///
    /// let opts = EapOptions::new_tls_blob("user@example.com", vec![], vec![])
    ///     .with_private_key_password("password")
    ///     .with_ca_cert_blob(vec![]);
    /// ```
    pub fn new_tls_blob(
        identity: impl Into<String>,
        private_key_blob: impl Into<Vec<u8>>,
        client_cert_blob: impl Into<Vec<u8>>,
    ) -> Self {
        Self {
            identity: identity.into(),
            method: EapMethod::Tls,
            private_key_blob: Some(private_key_blob.into()),
            client_cert_blob: Some(client_cert_blob.into()),
            ..Default::default()
        }
    }

    /// Creates a new `EapOptions` builder.
    ///
    /// This provides an alternative way to construct EAP options with a fluent API,
    /// making it clearer what each configuration option does.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use nmrs::{EapOptions, EapMethod, Phase2};
    ///
    /// let opts = EapOptions::builder()
    ///     .identity("user@company.com")
    ///     .password("my_password")
    ///     .method(EapMethod::Peap)
    ///     .phase2(Phase2::Mschapv2)
    ///     .domain_suffix_match("company.com")
    ///     .system_ca_certs(true)
    ///     .build()
    ///     .expect("all required fields set");
    /// ```
    #[must_use]
    pub fn builder() -> EapOptionsBuilder {
        EapOptionsBuilder::default()
    }

    /// Sets the anonymous identity for privacy.
    #[must_use]
    pub fn with_anonymous_identity(mut self, anonymous_identity: impl Into<String>) -> Self {
        self.anonymous_identity = Some(anonymous_identity.into());
        self
    }

    /// Sets the domain suffix to match against the server certificate.
    #[must_use]
    pub fn with_domain_suffix_match(mut self, domain: impl Into<String>) -> Self {
        self.domain_suffix_match = Some(domain.into());
        self
    }

    /// Sets the path to the CA certificate file (must start with `file://`).
    ///
    /// Clears `ca_cert_blob` because they are mutually exclusive.
    #[must_use]
    pub fn with_ca_cert_path(mut self, path: impl Into<String>) -> Self {
        self.ca_cert_blob = None;
        self.ca_cert_path = Some(path.into());
        self
    }

    /// Sets the CA certificate encoded as DER.
    ///
    /// Clears `ca_cert_path` because they are mutually exclusive.
    #[must_use]
    pub fn with_ca_cert_blob(mut self, data: impl Into<Vec<u8>>) -> Self {
        self.ca_cert_path = None;
        self.ca_cert_blob = Some(data.into());
        self
    }

    /// Sets whether to use the system CA certificate store.
    #[must_use]
    pub fn with_system_ca_certs(mut self, use_system: bool) -> Self {
        self.system_ca_certs = use_system;
        self
    }

    /// Sets the EAP method (PEAP or TTLS).
    #[must_use]
    pub fn with_method(mut self, method: EapMethod) -> Self {
        self.method = method;
        self
    }

    /// Sets the Phase 2 authentication method.
    #[must_use]
    pub fn with_phase2(mut self, phase2: Phase2) -> Self {
        self.phase2 = phase2;
        self
    }

    /// Sets the password for the private key file.
    #[must_use]
    pub fn with_private_key_password(mut self, password: impl Into<String>) -> Self {
        self.private_key_password = Some(password.into());
        self
    }
}

/// Builder for constructing `EapOptions` with a fluent API.
///
/// This builder provides an ergonomic way to create EAP (Enterprise WiFi)
/// authentication options, making the configuration more explicit and readable.
///
/// # Examples
///
/// ## PEAP with MSCHAPv2 (Common Corporate Setup)
///
/// ```rust
/// use nmrs::{EapOptions, EapMethod, Phase2};
///
/// let opts = EapOptions::builder()
///     .identity("employee@company.com")
///     .password("my_password")
///     .method(EapMethod::Peap)
///     .phase2(Phase2::Mschapv2)
///     .anonymous_identity("anonymous@company.com")
///     .domain_suffix_match("company.com")
///     .system_ca_certs(true)
///     .build()
///     .expect("all required fields set");
/// ```
///
/// ## TTLS with PAP
///
/// ```rust
/// use nmrs::{EapOptions, EapMethod, Phase2};
///
/// let opts = EapOptions::builder()
///     .identity("student@university.edu")
///     .password("password")
///     .method(EapMethod::Ttls)
///     .phase2(Phase2::Pap)
///     .ca_cert_path("file:///etc/ssl/certs/university-ca.pem")
///     .build()
///     .expect("all required fields set");
/// ```
///
/// ## TLS
///
/// ```rust
/// use nmrs::{EapOptions, EapMethod};
///
/// let opts = EapOptions::builder()
///     .identity("student@university.edu")
///     .method(EapMethod::Tls)
///     .private_key_path("file:///etc/ssl/private/student.key")
///     .private_key_password("password")
///     .client_cert_path("file:///etc/ssl/certs/student.crt")
///     .ca_cert_path("file:///etc/ssl/certs/university-ca.pem")
///     .build()
///     .expect("all required fields set");
/// ```
#[derive(Debug, Default)]
pub struct EapOptionsBuilder {
    identity: Option<String>,
    password: Option<String>,
    anonymous_identity: Option<String>,
    domain_suffix_match: Option<String>,
    ca_cert_path: Option<String>,
    ca_cert_blob: Option<Vec<u8>>,
    system_ca_certs: bool,
    method: Option<EapMethod>,
    phase2: Option<Phase2>,
    private_key_path: Option<String>,
    private_key_blob: Option<Vec<u8>>,
    private_key_password: Option<String>,
    client_cert_path: Option<String>,
    client_cert_blob: Option<Vec<u8>>,
}

impl EapOptionsBuilder {
    /// Sets the user identity (usually email or username).
    ///
    /// This is a required field.
    #[must_use]
    pub fn identity(mut self, identity: impl Into<String>) -> Self {
        self.identity = Some(identity.into());
        self
    }

    /// Sets the password for authentication.
    ///
    /// This is a required field.
    #[must_use]
    pub fn password(mut self, password: impl Into<String>) -> Self {
        self.password = Some(password.into());
        self
    }

    /// Sets the anonymous outer identity for privacy.
    ///
    /// This identity is sent in the clear during the initial handshake,
    /// while the real identity is protected inside the TLS tunnel.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use nmrs::EapOptions;
    ///
    /// let builder = EapOptions::builder()
    ///     .anonymous_identity("anonymous@company.com");
    /// ```
    #[must_use]
    pub fn anonymous_identity(mut self, anonymous_identity: impl Into<String>) -> Self {
        self.anonymous_identity = Some(anonymous_identity.into());
        self
    }

    /// Sets the domain suffix to match against the server certificate.
    ///
    /// This provides additional security by verifying the server's certificate
    /// matches the expected domain.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use nmrs::EapOptions;
    ///
    /// let builder = EapOptions::builder()
    ///     .domain_suffix_match("company.com");
    /// ```
    #[must_use]
    pub fn domain_suffix_match(mut self, domain: impl Into<String>) -> Self {
        self.domain_suffix_match = Some(domain.into());
        self
    }

    /// Sets the path to the CA certificate file.
    ///
    /// The path must start with `file://` (e.g., "file:///etc/ssl/certs/ca.pem").
    ///
    /// Clears `ca_cert_blob` because they are mutually exclusive.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use nmrs::EapOptions;
    ///
    /// let builder = EapOptions::builder()
    ///     .ca_cert_path("file:///etc/ssl/certs/company-ca.pem");
    /// ```
    #[must_use]
    pub fn ca_cert_path(mut self, path: impl Into<String>) -> Self {
        self.ca_cert_blob = None;
        self.ca_cert_path = Some(path.into());
        self
    }

    /// Sets the CA certificate encoded as DER.
    ///
    /// Clears `ca_cert_path` because they are mutually exclusive.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use nmrs::EapOptions;
    ///
    /// let builder = EapOptions::builder()
    ///     .ca_cert_blob(vec![]);
    /// ```
    #[must_use]
    pub fn ca_cert_blob(mut self, data: impl Into<Vec<u8>>) -> Self {
        self.ca_cert_path = None;
        self.ca_cert_blob = Some(data.into());
        self
    }

    /// Sets whether to use the system CA certificate store.
    ///
    /// When enabled, the system's trusted CA certificates will be used
    /// to validate the server certificate.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use nmrs::EapOptions;
    ///
    /// let builder = EapOptions::builder()
    ///     .system_ca_certs(true);
    /// ```
    #[must_use]
    pub fn system_ca_certs(mut self, use_system: bool) -> Self {
        self.system_ca_certs = use_system;
        self
    }

    /// Sets the EAP method (PEAP or TTLS).
    ///
    /// This is a required field. PEAP is more common in corporate environments,
    /// while TTLS offers more flexibility in inner authentication methods.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use nmrs::{EapOptions, EapMethod};
    ///
    /// let builder = EapOptions::builder()
    ///     .method(EapMethod::Peap);
    /// ```
    #[must_use]
    pub fn method(mut self, method: EapMethod) -> Self {
        self.method = Some(method);
        self
    }

    /// Sets the Phase 2 (inner) authentication method.
    ///
    /// This is a required field. MSCHAPv2 is commonly used with PEAP,
    /// while PAP is often used with TTLS.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use nmrs::{EapOptions, Phase2};
    ///
    /// let builder = EapOptions::builder()
    ///     .phase2(Phase2::Mschapv2);
    /// ```
    #[must_use]
    pub fn phase2(mut self, phase2: Phase2) -> Self {
        self.phase2 = Some(phase2);
        self
    }

    /// Sets the path to the private key file of the client certificate.
    ///
    /// The path must start with `file://` (e.g., "file:///etc/ssl/private/client.key").
    ///
    /// Clears `private_key_blob` because they are mutually exclusive.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use nmrs::EapOptions;
    ///
    /// let builder = EapOptions::builder()
    ///     .private_key_path("file:///etc/ssl/private/client.key");
    /// ```
    #[must_use]
    pub fn private_key_path(mut self, path: impl Into<String>) -> Self {
        self.private_key_blob = None;
        self.private_key_path = Some(path.into());
        self
    }

    /// Sets the private key of the client certificate encoded as PEM or PKCS#12.
    ///
    /// Clears `private_key_path` because they are mutually exclusive.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use nmrs::EapOptions;
    ///
    /// let builder = EapOptions::builder()
    ///     .private_key_blob(vec![]);
    /// ```
    #[must_use]
    pub fn private_key_blob(mut self, data: impl Into<Vec<u8>>) -> Self {
        self.private_key_path = None;
        self.private_key_blob = Some(data.into());
        self
    }

    /// Sets the password for the private key file.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use nmrs::EapOptions;
    ///
    /// let builder = EapOptions::builder()
    ///     .private_key_password("password");
    /// ```
    #[must_use]
    pub fn private_key_password(mut self, password: impl Into<String>) -> Self {
        self.private_key_password = Some(password.into());
        self
    }

    /// Sets the path to the client certificate file.
    ///
    /// The path must start with `file://` (e.g., "file:///etc/ssl/certs/client.crt").
    ///
    /// Clears `client_cert_blob` because they are mutually exclusive.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use nmrs::EapOptions;
    ///
    /// let builder = EapOptions::builder()
    ///     .client_cert_path("file:///etc/ssl/certs/client.crt");
    /// ```
    #[must_use]
    pub fn client_cert_path(mut self, path: impl Into<String>) -> Self {
        self.client_cert_blob = None;
        self.client_cert_path = Some(path.into());
        self
    }

    /// Sets the client certificate encoded as DER or PKCS#12.
    ///
    /// Clears `client_cert_path` because they are mutually exclusive.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use nmrs::EapOptions;
    ///
    /// let builder = EapOptions::builder()
    ///     .client_cert_blob(vec![]);
    /// ```
    #[must_use]
    pub fn client_cert_blob(mut self, data: impl Into<Vec<u8>>) -> Self {
        self.client_cert_path = None;
        self.client_cert_blob = Some(data.into());
        self
    }

    /// Builds the `EapOptions` from the configured values.
    ///
    /// # Errors
    ///
    /// Returns [`ConnectionError::IncompleteBuilder`](crate::ConnectionError::IncompleteBuilder)
    /// if any required field is missing.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use nmrs::{EapOptions, EapMethod, Phase2};
    ///
    /// let opts = EapOptions::builder()
    ///     .identity("user@example.com")
    ///     .password("password")
    ///     .method(EapMethod::Peap)
    ///     .phase2(Phase2::Mschapv2)
    ///     .build()
    ///     .expect("all required fields set");
    /// ```
    #[must_use = "use the EAP options with WifiSecurity::WpaEap or handle the error"]
    pub fn build(self) -> Result<EapOptions, ConnectionError> {
        let is_peap_or_ttls =
            self.method == Some(EapMethod::Peap) || self.method == Some(EapMethod::Ttls);

        if self.ca_cert_path.is_some() && self.ca_cert_blob.is_some() {
            return Err(ConnectionError::IncompleteBuilder(
                "EAP CA certificate cannot be specified both as a path and blob".into(),
            ));
        }
        if self.private_key_path.is_some() && self.private_key_blob.is_some() {
            return Err(ConnectionError::IncompleteBuilder(
                "EAP private key cannot be specified both as a path and blob".into(),
            ));
        }
        if self.client_cert_path.is_some() && self.client_cert_blob.is_some() {
            return Err(ConnectionError::IncompleteBuilder(
                "EAP client certificate cannot be specified both as a path and blob".into(),
            ));
        }
        if self.method == Some(EapMethod::Tls) {
            if self.private_key_path.is_none() && self.private_key_blob.is_none() {
                return Err(ConnectionError::IncompleteBuilder(
                    "EAP private key is required for TLS (use .private_key_path() or .private_key_blob())".into(),
                ));
            }
            if self.client_cert_path.is_none() && self.client_cert_blob.is_none() {
                return Err(ConnectionError::IncompleteBuilder(
                    "EAP client certificate is required for TLS (use .client_cert_path() or .client_cert_blob())".into(),
                ));
            }
        }

        Ok(EapOptions {
            identity: self.identity.ok_or_else(|| {
                ConnectionError::IncompleteBuilder(
                    "EAP identity is required (use .identity())".into(),
                )
            })?,
            password: if is_peap_or_ttls {
                self.password.ok_or_else(|| {
                    ConnectionError::IncompleteBuilder(
                        "EAP password is required (use .password())".into(),
                    )
                })?
            } else {
                String::new()
            },
            anonymous_identity: self.anonymous_identity,
            domain_suffix_match: self.domain_suffix_match,
            ca_cert_path: self.ca_cert_path,
            ca_cert_blob: self.ca_cert_blob,
            system_ca_certs: self.system_ca_certs,
            method: self.method.ok_or_else(|| {
                ConnectionError::IncompleteBuilder("EAP method is required (use .method())".into())
            })?,
            phase2: if is_peap_or_ttls {
                self.phase2.ok_or_else(|| {
                    ConnectionError::IncompleteBuilder(
                        "EAP phase 2 method is required (use .phase2())".into(),
                    )
                })?
            } else {
                Phase2::Mschapv2
            },
            private_key_path: self.private_key_path,
            private_key_blob: self.private_key_blob,
            private_key_password: self.private_key_password,
            client_cert_path: self.client_cert_path,
            client_cert_blob: self.client_cert_blob,
        })
    }
}

/// Wi-Fi connection security types.
///
/// Represents the authentication method for connecting to a WiFi network.
///
/// # Variants
///
/// - [`Open`](WifiSecurity::Open) - No authentication required (open network)
/// - [`WpaPsk`](WifiSecurity::WpaPsk) - WPA/WPA2/WPA3 Personal (password-based)
/// - [`WpaEap`](WifiSecurity::WpaEap) - WPA/WPA2 Enterprise (802.1X authentication)
///
/// # Examples
///
/// ## Open Network
///
/// ```rust
/// use nmrs::WifiSecurity;
///
/// let security = WifiSecurity::Open;
/// ```
///
/// ## Password-Protected Network
///
/// ```no_run
/// use nmrs::{NetworkManager, WifiSecurity};
///
/// # async fn example() -> nmrs::Result<()> {
/// let nm = NetworkManager::new().await?;
///
/// nm.connect("HomeWiFi", None, WifiSecurity::WpaPsk {
///     psk: "my_secure_password".into()
/// }).await?;
/// # Ok(())
/// # }
/// ```
///
/// ## Enterprise Network (WPA-EAP)
///
/// ```no_run
/// use nmrs::{NetworkManager, WifiSecurity, EapOptions, EapMethod, Phase2};
///
/// # async fn example() -> nmrs::Result<()> {
/// let nm = NetworkManager::new().await?;
///
/// let eap_opts = EapOptions::new("user@company.com", "password")
///     .with_domain_suffix_match("company.com")
///     .with_system_ca_certs(true)
///     .with_method(EapMethod::Peap)
///     .with_phase2(Phase2::Mschapv2);
///
/// nm.connect("CorpWiFi", None, WifiSecurity::WpaEap {
///     opts: eap_opts
/// }).await?;
/// # Ok(())
/// # }
/// ```
#[non_exhaustive]
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum WifiSecurity {
    /// Open network (no authentication)
    Open,
    /// WPA-PSK (password-based authentication)
    WpaPsk {
        /// Pre-shared key (password)
        psk: String,
    },
    /// WPA-EAP (Enterprise authentication via 802.1X)
    WpaEap {
        /// EAP configuration options
        opts: EapOptions,
    },
    /// WPA3-EAP 192-bit mode (Enterprise authentication via 802.1X)
    /// Only EAP-TLS is allowed as authentication method.
    Wpa3Eap192bit {
        /// EAP configuration options
        opts: EapOptions,
    },
}

impl WifiSecurity {
    /// Returns `true` if this security type requires authentication.
    #[must_use]
    pub fn secured(&self) -> bool {
        !matches!(self, WifiSecurity::Open)
    }

    /// Returns `true` if this is a WPA-PSK (password-based) security type.
    #[must_use]
    pub fn is_psk(&self) -> bool {
        matches!(self, WifiSecurity::WpaPsk { .. })
    }

    /// Returns `true` if this is a WPA-EAP (Enterprise/802.1X) security type.
    #[must_use]
    pub fn is_eap(&self) -> bool {
        matches!(
            self,
            WifiSecurity::WpaEap { .. } | WifiSecurity::Wpa3Eap192bit { .. }
        )
    }
}

impl Network {
    /// Merges another access point's information into this network.
    ///
    /// When multiple access points share the same SSID (e.g., mesh networks),
    /// this method keeps the strongest signal and combines security flags.
    /// Used internally during network scanning to deduplicate results.
    pub fn merge_ap(&mut self, other: &Network) {
        let mut bssids = self.bssids.clone();
        if let Some(bssid) = &self.bssid {
            push_unique_bssid(&mut bssids, bssid);
        }
        for bssid in &other.bssids {
            push_unique_bssid(&mut bssids, bssid);
        }
        if let Some(bssid) = &other.bssid {
            push_unique_bssid(&mut bssids, bssid);
        }

        if other.strength.unwrap_or(0) > self.strength.unwrap_or(0) {
            self.strength = other.strength;
            self.frequency = other.frequency;
            self.bssid = other.bssid.clone();
            self.best_bssid = other.best_bssid.clone();
        }

        if let Some(best_bssid) = &self.bssid {
            bssids.retain(|bssid| !bssid.eq_ignore_ascii_case(best_bssid));
            bssids.insert(0, best_bssid.clone());
        }
        self.bssids = bssids;

        merge_security_features(&mut self.security_features, other.security_features);

        self.secured |= other.secured;
        self.is_psk |= other.is_psk;
        self.is_eap |= other.is_eap;
        self.is_hotspot |= other.is_hotspot;
        self.is_active |= other.is_active;
        self.known |= other.known;

        if self.ip4_address.is_none() {
            self.ip4_address.clone_from(&other.ip4_address);
        }
        if self.ip6_address.is_none() {
            self.ip6_address.clone_from(&other.ip6_address);
        }
        if self.device.is_empty() {
            self.device.clone_from(&other.device);
        }
    }
}

fn push_unique_bssid(bssids: &mut Vec<String>, candidate: &str) {
    if !bssids
        .iter()
        .any(|bssid| bssid.eq_ignore_ascii_case(candidate))
    {
        bssids.push(candidate.to_string());
    }
}

fn merge_security_features(current: &mut SecurityFeatures, other: SecurityFeatures) {
    current.privacy |= other.privacy;
    current.wps |= other.wps;
    current.psk |= other.psk;
    current.eap |= other.eap;
    current.sae |= other.sae;
    current.owe |= other.owe;
    current.owe_transition_mode |= other.owe_transition_mode;
    current.eap_suite_b_192 |= other.eap_suite_b_192;
    current.wep40 |= other.wep40;
    current.wep104 |= other.wep104;
    current.tkip |= other.tkip;
    current.ccmp |= other.ccmp;
}

#[cfg(test)]
mod network_merge_tests {
    use super::{Network, SecurityFeatures};

    fn network(bssid: &str, strength: u8) -> Network {
        Network {
            device: "wlan0".into(),
            ssid: "net".into(),
            bssid: Some(bssid.into()),
            strength: Some(strength),
            frequency: Some(2412),
            secured: false,
            is_psk: false,
            is_eap: false,
            is_hotspot: false,
            ip4_address: None,
            ip6_address: None,
            best_bssid: bssid.into(),
            bssids: vec![bssid.into()],
            is_active: false,
            known: false,
            security_features: SecurityFeatures::default(),
        }
    }

    #[test]
    fn merge_ap_keeps_ip_and_device_when_stronger_ap_has_none() {
        let mut weaker_connected = Network {
            device: "wlan0".into(),
            ssid: "net".into(),
            bssid: Some("aa:aa:aa:aa:aa:aa".into()),
            strength: Some(20),
            frequency: Some(5200),
            secured: true,
            is_psk: true,
            is_eap: false,
            is_hotspot: false,
            ip4_address: Some("192.168.1.5/24".into()),
            ip6_address: Some("fe80::1/64".into()),
            best_bssid: "aa:aa:aa:aa:aa:aa".into(),
            bssids: vec!["aa:aa:aa:aa:aa:aa".into()],
            is_active: true,
            known: false,
            security_features: Default::default(),
        };
        weaker_connected.security_features.psk = true;
        weaker_connected.security_features.ccmp = true;
        let mut stronger = Network {
            device: String::new(),
            ssid: "net".into(),
            bssid: Some("bb:bb:bb:bb:bb:bb".into()),
            strength: Some(90),
            frequency: Some(5200),
            secured: true,
            is_psk: true,
            is_eap: false,
            is_hotspot: false,
            ip4_address: None,
            ip6_address: None,
            best_bssid: "bb:bb:bb:bb:bb:bb".into(),
            bssids: vec!["bb:bb:bb:bb:bb:bb".into()],
            is_active: false,
            known: false,
            security_features: Default::default(),
        };
        stronger.security_features.eap = true;
        stronger.security_features.sae = true;
        weaker_connected.merge_ap(&stronger);
        assert_eq!(weaker_connected.strength, Some(90));
        assert_eq!(weaker_connected.bssid, Some("bb:bb:bb:bb:bb:bb".into()));
        assert_eq!(weaker_connected.best_bssid, "bb:bb:bb:bb:bb:bb");
        assert_eq!(weaker_connected.ip4_address, Some("192.168.1.5/24".into()));
        assert_eq!(weaker_connected.ip6_address, Some("fe80::1/64".into()));
        assert_eq!(weaker_connected.device, "wlan0");
        assert!(weaker_connected.is_active);
        assert!(weaker_connected.security_features.psk);
        assert!(weaker_connected.security_features.ccmp);
        assert!(weaker_connected.security_features.eap);
        assert!(weaker_connected.security_features.sae);
        assert_eq!(
            weaker_connected.bssids,
            vec![
                "bb:bb:bb:bb:bb:bb".to_string(),
                "aa:aa:aa:aa:aa:aa".to_string()
            ]
        );
    }

    #[test]
    fn merge_ap_combines_flags_security_and_all_unique_bssids() {
        let mut strongest = network("AA:AA:AA:AA:AA:01", 90);
        strongest.frequency = Some(5180);
        strongest.secured = true;
        strongest.is_psk = true;
        strongest.security_features.psk = true;
        strongest.security_features.ccmp = true;

        let mut weaker = network("BB:BB:BB:BB:BB:01", 30);
        weaker.frequency = Some(2412);
        weaker.is_eap = true;
        weaker.is_hotspot = true;
        weaker.known = true;
        weaker.bssids = vec![
            "BB:BB:BB:BB:BB:01".into(),
            "CC:CC:CC:CC:CC:01".into(),
            "aa:aa:aa:aa:aa:01".into(),
        ];
        weaker.security_features.eap = true;
        weaker.security_features.sae = true;
        weaker.security_features.wps = true;

        strongest.merge_ap(&weaker);

        assert_eq!(strongest.strength, Some(90));
        assert_eq!(strongest.frequency, Some(5180));
        assert_eq!(strongest.bssid.as_deref(), Some("AA:AA:AA:AA:AA:01"));
        assert_eq!(
            strongest.bssids,
            vec![
                "AA:AA:AA:AA:AA:01".to_string(),
                "BB:BB:BB:BB:BB:01".to_string(),
                "CC:CC:CC:CC:CC:01".to_string(),
            ]
        );
        assert!(strongest.secured);
        assert!(strongest.is_psk);
        assert!(strongest.is_eap);
        assert!(strongest.is_hotspot);
        assert!(strongest.known);
        assert!(strongest.security_features.psk);
        assert!(strongest.security_features.eap);
        assert!(strongest.security_features.sae);
        assert!(strongest.security_features.wps);
        assert!(strongest.security_features.ccmp);
    }

    #[test]
    fn merge_ap_fills_missing_connection_context() {
        let mut network_without_context = network("AA:AA:AA:AA:AA:01", 80);
        network_without_context.device.clear();
        let mut connected = network("BB:BB:BB:BB:BB:01", 40);
        connected.device = "wlan1".into();
        connected.ip4_address = Some("192.168.50.5/24".into());
        connected.ip6_address = Some("2001:db8::5/64".into());
        connected.is_active = true;

        network_without_context.merge_ap(&connected);

        assert_eq!(network_without_context.device, "wlan1");
        assert_eq!(
            network_without_context.ip4_address.as_deref(),
            Some("192.168.50.5/24")
        );
        assert_eq!(
            network_without_context.ip6_address.as_deref(),
            Some("2001:db8::5/64")
        );
        assert!(network_without_context.is_active);
    }
}
