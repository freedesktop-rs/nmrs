#![allow(deprecated)]

use super::vpn::{VpnConfig, VpnKind};
use crate::api::models::error::ConnectionError;
use std::convert::TryFrom;
use std::net::Ipv4Addr;
use uuid::Uuid;

/// A static IPv4 route for OpenVPN split tunneling.
///
/// Serialized to NetworkManager `ipv4.route-data`.
#[non_exhaustive]
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VpnRoute {
    /// Destination network (e.g. `10.0.0.0`).
    pub dest: String,
    /// CIDR prefix length (0–32).
    pub prefix: u32,
    /// Optional gateway (`next-hop` in NM).
    pub next_hop: Option<String>,
    /// Optional route metric.
    pub metric: Option<u32>,
}

impl VpnRoute {
    /// Creates a route to `dest`/`prefix`.
    #[must_use]
    pub fn new(dest: impl Into<String>, prefix: u32) -> Self {
        Self {
            dest: dest.into(),
            prefix,
            next_hop: None,
            metric: None,
        }
    }

    /// Sets the gateway for this route.
    #[must_use]
    pub fn next_hop(mut self, gateway: impl Into<String>) -> Self {
        self.next_hop = Some(gateway.into());
        self
    }

    /// Sets the route metric.
    #[must_use]
    pub fn metric(mut self, metric: u32) -> Self {
        self.metric = Some(metric);
        self
    }
}

/// OpenVPN authentication type.
///
/// Specifies how the client authenticates with the OpenVPN server.
#[non_exhaustive]
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum OpenVpnAuthType {
    /// Username/password authentication only.
    Password,
    /// TLS certificate authentication only.
    Tls,
    /// Both password and TLS certificate authentication.
    PasswordTls,
    /// Static key authentication (pre-shared key).
    StaticKey,
}

/// OpenVPN connection configuration.
///
/// Stores the necessary information to configure and connect to an OpenVPN server.
///
/// # Example
///
/// ```rust
/// use nmrs::{OpenVpnConfig, OpenVpnAuthType};
///
/// let config = OpenVpnConfig::new("MyVPN", "vpn.example.com", 1194, false)
///     .with_auth_type(OpenVpnAuthType::PasswordTls)
///     .with_username("user")
///     .with_password("secret")
///     .with_ca_cert("/path/to/ca.crt")
///     .with_dns(vec!["1.1.1.1".into()]);
/// ```
#[non_exhaustive]
#[derive(Debug, Clone)]
pub struct OpenVpnConfig {
    /// Connection name.
    pub name: String,
    /// Remote server hostname or IP.
    pub remote: String,
    /// Remote server port (default: 1194).
    pub port: u16,
    /// Use TCP instead of UDP.
    pub tcp: bool,
    /// Authentication type.
    pub auth_type: Option<OpenVpnAuthType>,
    /// HMAC digest algorithm (e.g., "SHA256").
    pub auth: Option<String>,
    /// Data channel cipher (e.g., "AES-256-GCM").
    pub cipher: Option<String>,
    /// DNS servers to use when connected.
    pub dns: Option<Vec<String>>,
    /// MTU size.
    pub mtu: Option<u32>,
    /// Connection UUID.
    pub uuid: Option<Uuid>,
    /// Path to CA certificate.
    pub ca_cert: Option<String>,
    /// Path to client certificate.
    pub client_cert: Option<String>,
    /// Path to client private key.
    pub client_key: Option<String>,
    /// Password for encrypted private key.
    pub key_password: Option<String>,
    /// Username for password authentication.
    pub username: Option<String>,
    /// Password for password authentication.
    pub password: Option<String>,
    /// Compression algorithm. See [`OpenVpnCompression`] for security considerations.
    pub compression: Option<OpenVpnCompression>,
    /// Proxy configuration.
    pub proxy: Option<OpenVpnProxy>,
    /// Path to TLS authentication (HMAC firewall) key file.
    pub tls_auth_key: Option<String>,
    /// TLS auth direction (`0` or `1`). Only meaningful when `tls_auth_key` is set.
    pub tls_auth_direction: Option<u8>,
    /// Path to TLS-Crypt key file (encrypt+authenticate control channel).
    pub tls_crypt: Option<String>,
    /// Path to TLS-Crypt-v2 key file (per-client TLS-Crypt).
    pub tls_crypt_v2: Option<String>,
    /// Minimum TLS version (e.g. "1.2").
    pub tls_version_min: Option<String>,
    /// Maximum TLS version (e.g. "1.3").
    pub tls_version_max: Option<String>,
    /// Control channel TLS cipher suites (e.g. "TLS-ECDHE-RSA-WITH-AES-256-GCM-SHA384").
    pub tls_cipher: Option<String>,
    /// Require remote certificate to be of a specific type ("server" or "client").
    pub remote_cert_tls: Option<String>,
    /// X.509 name verification: `(name, type)` where type is e.g. "name", "subject",
    /// or "name-prefix".
    pub verify_x509_name: Option<(String, String)>,
    /// Path to a Certificate Revocation List file.
    pub crl_verify: Option<String>,
    /// When true, this profile may become the default route (full tunnel / `redirect-gateway`).
    ///
    /// Maps to `ipv4.never-default = false` in NetworkManager when set.
    pub redirect_gateway: bool,
    /// Static IPv4 routes for split tunneling (`ipv4.route-data`).
    pub routes: Vec<VpnRoute>,
    /// OpenVPN `ping` interval in seconds.
    pub ping: Option<u32>,
    /// OpenVPN `ping-exit` seconds.
    pub ping_exit: Option<u32>,
    /// OpenVPN `ping-restart` seconds.
    pub ping_restart: Option<u32>,
    /// TLS renegotiation period (`reneg-sec`).
    pub reneg_seconds: Option<u32>,
    /// Initial connection timeout in seconds (`connect-timeout`).
    pub connect_timeout: Option<u32>,
    /// Negotiable data ciphers list (`data-ciphers`), colon-separated.
    pub data_ciphers: Option<String>,
    /// Fallback data cipher (`data-ciphers-fallback`).
    pub data_ciphers_fallback: Option<String>,
    /// When true, disables NCP (`ncp-disable`).
    pub ncp_disable: bool,
}

impl OpenVpnConfig {
    /// Creates a new `OpenVpnConfig` with required fields.
    pub fn new(name: impl Into<String>, remote: impl Into<String>, port: u16, tcp: bool) -> Self {
        Self {
            name: name.into(),
            remote: remote.into(),
            port,
            tcp,
            auth_type: None,
            auth: None,
            cipher: None,
            dns: None,
            mtu: None,
            uuid: None,
            ca_cert: None,
            client_cert: None,
            client_key: None,
            key_password: None,
            username: None,
            password: None,
            compression: None,
            proxy: None,
            tls_auth_key: None,
            tls_auth_direction: None,
            tls_crypt: None,
            tls_crypt_v2: None,
            tls_version_min: None,
            tls_version_max: None,
            tls_cipher: None,
            remote_cert_tls: None,
            verify_x509_name: None,
            crl_verify: None,
            redirect_gateway: false,
            routes: Vec::new(),
            ping: None,
            ping_exit: None,
            ping_restart: None,
            reneg_seconds: None,
            connect_timeout: None,
            data_ciphers: None,
            data_ciphers_fallback: None,
            ncp_disable: false,
        }
    }

    /// Sets the authentication type.
    #[must_use]
    pub fn with_auth_type(mut self, auth_type: OpenVpnAuthType) -> Self {
        self.auth_type = Some(auth_type);
        self
    }

    /// Sets the HMAC digest algorithm.
    #[must_use]
    pub fn with_auth(mut self, auth: impl Into<String>) -> Self {
        self.auth = Some(auth.into());
        self
    }

    /// Sets the data channel cipher.
    #[must_use]
    pub fn with_cipher(mut self, cipher: impl Into<String>) -> Self {
        self.cipher = Some(cipher.into());
        self
    }

    /// Sets the DNS servers to use when connected.
    #[must_use]
    pub fn with_dns(mut self, dns: Vec<String>) -> Self {
        self.dns = Some(dns);
        self
    }

    /// Sets the MTU (Maximum Transmission Unit) size.
    #[must_use]
    pub fn with_mtu(mut self, mtu: u32) -> Self {
        self.mtu = Some(mtu);
        self
    }

    /// Sets the UUID for the connection.
    #[must_use]
    pub fn with_uuid(mut self, uuid: Uuid) -> Self {
        self.uuid = Some(uuid);
        self
    }

    /// Sets the CA certificate path.
    #[must_use]
    pub fn with_ca_cert(mut self, path: impl Into<String>) -> Self {
        self.ca_cert = Some(path.into());
        self
    }

    /// Sets the client certificate path.
    #[must_use]
    pub fn with_client_cert(mut self, path: impl Into<String>) -> Self {
        self.client_cert = Some(path.into());
        self
    }

    /// Sets the client private key path.
    #[must_use]
    pub fn with_client_key(mut self, path: impl Into<String>) -> Self {
        self.client_key = Some(path.into());
        self
    }

    /// Sets the password for an encrypted private key.
    #[must_use]
    pub fn with_key_password(mut self, password: impl Into<String>) -> Self {
        self.key_password = Some(password.into());
        self
    }

    /// Sets the username for password authentication.
    #[must_use]
    pub fn with_username(mut self, username: impl Into<String>) -> Self {
        self.username = Some(username.into());
        self
    }

    /// Sets the password for password authentication.
    #[must_use]
    pub fn with_password(mut self, password: impl Into<String>) -> Self {
        self.password = Some(password.into());
        self
    }

    /// Sets the compression algorithm.
    ///
    /// # Security Warning
    ///
    /// Some compression modes are subject to the VORACLE vulnerability.
    /// See [`OpenVpnCompression`] for details and recommendations.
    #[must_use]
    pub fn with_compression(mut self, compression: OpenVpnCompression) -> Self {
        self.compression = Some(compression);
        self
    }

    /// Sets the proxy configuration.
    #[must_use]
    pub fn with_proxy(mut self, proxy: OpenVpnProxy) -> Self {
        self.proxy = Some(proxy);
        self
    }

    /// Sets the TLS authentication key path and optional direction.
    ///
    /// The `--tls-auth` option adds an HMAC firewall to the control channel,
    /// providing an additional layer of DoS protection.
    #[must_use]
    pub fn with_tls_auth(mut self, key_path: impl Into<String>, direction: Option<u8>) -> Self {
        self.tls_auth_key = Some(key_path.into());
        self.tls_auth_direction = direction;
        self
    }

    /// Sets the TLS-Crypt key path.
    ///
    /// Encrypts and authenticates the control channel with a pre-shared key,
    /// providing stronger protection than `--tls-auth`.
    #[must_use]
    pub fn with_tls_crypt(mut self, key_path: impl Into<String>) -> Self {
        self.tls_crypt = Some(key_path.into());
        self
    }

    /// Sets the TLS-Crypt-v2 key path (per-client key wrapping).
    #[must_use]
    pub fn with_tls_crypt_v2(mut self, key_path: impl Into<String>) -> Self {
        self.tls_crypt_v2 = Some(key_path.into());
        self
    }

    /// Sets the minimum TLS protocol version (e.g. "1.2").
    #[must_use]
    pub fn with_tls_version_min(mut self, version: impl Into<String>) -> Self {
        self.tls_version_min = Some(version.into());
        self
    }

    /// Sets the maximum TLS protocol version (e.g. "1.3").
    #[must_use]
    pub fn with_tls_version_max(mut self, version: impl Into<String>) -> Self {
        self.tls_version_max = Some(version.into());
        self
    }

    /// Sets the allowed control channel TLS cipher suites.
    #[must_use]
    pub fn with_tls_cipher(mut self, cipher: impl Into<String>) -> Self {
        self.tls_cipher = Some(cipher.into());
        self
    }

    /// Requires the remote certificate to be of a specific type ("server" or "client").
    #[must_use]
    pub fn with_remote_cert_tls(mut self, cert_type: impl Into<String>) -> Self {
        self.remote_cert_tls = Some(cert_type.into());
        self
    }

    /// Sets X.509 name verification for the remote certificate.
    ///
    /// `name_type` is one of "name", "subject", or "name-prefix".
    #[must_use]
    pub fn with_verify_x509_name(
        mut self,
        name: impl Into<String>,
        name_type: impl Into<String>,
    ) -> Self {
        self.verify_x509_name = Some((name.into(), name_type.into()));
        self
    }

    /// Sets the path to a Certificate Revocation List for peer verification.
    #[must_use]
    pub fn with_crl_verify(mut self, path: impl Into<String>) -> Self {
        self.crl_verify = Some(path.into());
        self
    }

    /// When true, the connection may become the default IPv4 route (full tunnel).
    #[must_use]
    pub fn with_redirect_gateway(mut self, redirect: bool) -> Self {
        self.redirect_gateway = redirect;
        self
    }

    /// Replaces static IPv4 routes for split tunneling.
    #[must_use]
    pub fn with_routes(mut self, routes: Vec<VpnRoute>) -> Self {
        self.routes = routes;
        self
    }

    /// Sets the OpenVPN `ping` interval (seconds).
    #[must_use]
    pub fn with_ping(mut self, seconds: u32) -> Self {
        self.ping = Some(seconds);
        self
    }

    /// Sets OpenVPN `ping-exit` (seconds).
    #[must_use]
    pub fn with_ping_exit(mut self, seconds: u32) -> Self {
        self.ping_exit = Some(seconds);
        self
    }

    /// Sets OpenVPN `ping-restart` (seconds).
    #[must_use]
    pub fn with_ping_restart(mut self, seconds: u32) -> Self {
        self.ping_restart = Some(seconds);
        self
    }

    /// Sets TLS renegotiation period (`reneg-sec`, seconds).
    #[must_use]
    pub fn with_reneg_seconds(mut self, seconds: u32) -> Self {
        self.reneg_seconds = Some(seconds);
        self
    }

    /// Sets initial connection timeout (`connect-timeout`, seconds).
    #[must_use]
    pub fn with_connect_timeout(mut self, seconds: u32) -> Self {
        self.connect_timeout = Some(seconds);
        self
    }

    /// Sets negotiable data ciphers (colon-separated, e.g. `AES-256-GCM:AES-128-GCM`).
    #[must_use]
    pub fn with_data_ciphers(mut self, ciphers: impl Into<String>) -> Self {
        self.data_ciphers = Some(ciphers.into());
        self
    }

    /// Sets the fallback data cipher (`data-ciphers-fallback`).
    #[must_use]
    pub fn with_data_ciphers_fallback(mut self, cipher: impl Into<String>) -> Self {
        self.data_ciphers_fallback = Some(cipher.into());
        self
    }

    /// When true, disables NCP cipher negotiation (`ncp-disable`).
    #[must_use]
    pub fn with_ncp_disable(mut self, disable: bool) -> Self {
        self.ncp_disable = disable;
        self
    }
}

fn ipv4_netmask_to_prefix(netmask: Ipv4Addr) -> u32 {
    let mut prefix = 0u32;
    for byte in netmask.octets() {
        if byte == 0xff {
            prefix += 8;
        } else if byte == 0 {
            break;
        } else {
            let mut b = byte;
            while b & 0x80 != 0 {
                prefix += 1;
                b <<= 1;
            }
            break;
        }
    }
    prefix
}

pub(crate) fn vpn_route_from_parser(
    r: crate::core::ovpn_parser::parser::Route,
) -> Result<VpnRoute, ConnectionError> {
    let dest = r.network.to_string();
    let prefix = r.netmask.map(ipv4_netmask_to_prefix).unwrap_or(32);
    if prefix > 32 {
        return Err(ConnectionError::InvalidAddress(format!(
            "invalid route netmask for destination {dest}"
        )));
    }
    let next_hop = r.gateway.map(|g| g.to_string());
    Ok(VpnRoute {
        dest,
        prefix,
        next_hop,
        metric: None,
    })
}

impl TryFrom<crate::core::ovpn_parser::parser::OvpnFile> for OpenVpnConfig {
    type Error = ConnectionError;

    fn try_from(f: crate::core::ovpn_parser::parser::OvpnFile) -> Result<Self, Self::Error> {
        use crate::core::ovpn_parser::parser::{AllowCompress, CertSource, Compress};

        let first_remote = f
            .remotes
            .into_iter()
            .next()
            .ok_or_else(|| ConnectionError::InvalidGateway("no remote in .ovpn file".into()))?;

        let tcp = first_remote
            .proto
            .as_deref()
            .map(|p: &str| p.starts_with("tcp"))
            .unwrap_or_else(|| {
                f.proto
                    .as_deref()
                    .map(|p: &str| p.starts_with("tcp"))
                    .unwrap_or(false)
            });

        let compression = match (f.compress, f.allow_compress) {
            (Some(Compress::Algorithm(ref s)), _) => Some(match s.as_str() {
                "lz4" => OpenVpnCompression::Lz4,
                "lz4-v2" => OpenVpnCompression::Lz4V2,
                _ => OpenVpnCompression::Yes,
            }),
            (Some(Compress::Stub | Compress::StubV2), _) => Some(OpenVpnCompression::No),
            (None, Some(AllowCompress::No)) => Some(OpenVpnCompression::No),
            _ => None,
        };

        // Client certificate auth needs both cert and key; one alone is incomplete.
        let has_client_cert_pair = f.cert.is_some() && f.key.is_some();
        let auth_type = match (f.auth_user_pass, has_client_cert_pair) {
            (true, true) => Some(OpenVpnAuthType::PasswordTls),
            (true, false) => Some(OpenVpnAuthType::Password),
            (false, true) => Some(OpenVpnAuthType::Tls),
            (false, false) => None,
        };

        let cert_path = |src: CertSource, field: &str| -> Result<String, ConnectionError> {
            match src {
                CertSource::File(p) => Ok(p),
                CertSource::Inline(_) => Err(ConnectionError::VpnFailed(format!(
                    "inline <{field}> blocks require OpenVpnBuilder::from_ovpn_file() \
                         or from_ovpn_str() which persists them via the cert store; \
                         TryFrom<OvpnFile> cannot handle inline certs"
                ))),
            }
        };

        let routes: Vec<VpnRoute> = f
            .routes
            .into_iter()
            .map(vpn_route_from_parser)
            .collect::<Result<_, _>>()?;

        let redirect_gateway = f.redirect_gateway.is_some();

        let data_ciphers = if f.data_ciphers.is_empty() {
            None
        } else {
            Some(f.data_ciphers.join(":"))
        };

        Ok(OpenVpnConfig {
            name: String::new(),
            remote: first_remote.host,
            port: first_remote.port.unwrap_or(1194),
            tcp,
            auth_type,
            auth: f.auth,
            cipher: f.cipher,
            dns: None,
            mtu: None,
            uuid: None,
            ca_cert: f.ca.map(|s| cert_path(s, "ca")).transpose()?,
            client_cert: f.cert.map(|s| cert_path(s, "cert")).transpose()?,
            client_key: f.key.map(|s| cert_path(s, "key")).transpose()?,
            key_password: None,
            username: None,
            password: None,
            compression,
            proxy: None,
            tls_auth_key: None,
            tls_auth_direction: None,
            tls_crypt: None,
            tls_crypt_v2: None,
            tls_version_min: None,
            tls_version_max: None,
            tls_cipher: None,
            remote_cert_tls: None,
            verify_x509_name: None,
            crl_verify: None,
            redirect_gateway,
            routes,
            ping: None,
            ping_exit: None,
            ping_restart: None,
            reneg_seconds: None,
            connect_timeout: None,
            data_ciphers,
            data_ciphers_fallback: None,
            ncp_disable: false,
        })
    }
}

impl super::vpn::sealed::Sealed for OpenVpnConfig {}

impl VpnConfig for OpenVpnConfig {
    fn vpn_kind(&self) -> VpnKind {
        VpnKind::Plugin
    }

    fn name(&self) -> &str {
        &self.name
    }

    fn dns(&self) -> Option<&[String]> {
        self.dns.as_deref()
    }

    fn mtu(&self) -> Option<u32> {
        self.mtu
    }

    fn uuid(&self) -> Option<Uuid> {
        self.uuid
    }
}

/// Compression algorithm for OpenVPN connections.
///
/// Maps to the NM `compress` and `comp-lzo` keys in the `vpn.data` dict.
///
/// # Security Warning
///
/// Compression is generally discouraged due to the VORACLE vulnerability,
/// where compression oracles can be exploited to recover plaintext from
/// encrypted tunnels. OpenVPN 2.5+ defaults to `--allow-compression no`.
/// Prefer [`No`](OpenVpnCompression::No) unless you have a specific need
/// and understand the risk. See <https://community.openvpn.net/openvpn/wiki/VORACLE>.
#[non_exhaustive]
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum OpenVpnCompression {
    /// Disable compression explicitly. Recommended default.
    ///
    /// Maps to `compress no` in the NM `vpn.data` dict.
    No,

    /// LZO compression.
    ///
    /// Maps to `comp-lzo yes` in the NM `vpn.data` dict.
    ///
    /// # Security Warning
    ///
    /// Subject to the VORACLE vulnerability. See [`OpenVpnCompression`] docs.
    ///
    /// # Deprecation
    ///
    /// `comp-lzo` is deprecated upstream in OpenVPN in favour of the newer
    /// `compress` directive. Use [`Lz4V2`](OpenVpnCompression::Lz4V2) if
    /// you need compression, or [`No`](OpenVpnCompression::No) to disable it.
    #[deprecated(note = "comp-lzo is deprecated upstream. Use Lz4V2 or No instead.")]
    Lzo,

    /// LZ4 compression.
    ///
    /// Maps to `compress lz4` in the NM `vpn.data` dict.
    ///
    /// # Security Warning
    ///
    /// Subject to the VORACLE vulnerability. See [`OpenVpnCompression`] docs.
    Lz4,

    /// LZ4 v2 compression.
    ///
    /// Maps to `compress lz4-v2` in the NM `vpn.data` dict.
    ///
    /// # Security Warning
    ///
    /// Subject to the VORACLE vulnerability. See [`OpenVpnCompression`] docs.
    Lz4V2,

    /// Adaptive compression — algorithm negotiated at runtime.
    ///
    /// Maps to `compress yes` in the NM `vpn.data` dict.
    ///
    /// # Security Warning
    ///
    /// Subject to the VORACLE vulnerability. See [`OpenVpnCompression`] docs.
    Yes,
}

/// Proxy configuration for OpenVPN connections.
///
/// Maps to the NM `proxy-type`, `proxy-server`, `proxy-port`,
/// `proxy-retry`, `http-proxy-username`, and `http-proxy-password` keys.
#[non_exhaustive]
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum OpenVpnProxy {
    /// HTTP proxy.
    Http {
        server: String,
        port: u16,
        username: Option<String>,
        password: Option<String>,
        retry: bool,
    },
    /// SOCKS proxy.
    Socks {
        server: String,
        port: u16,
        retry: bool,
    },
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::ovpn_parser::parser::parse_ovpn;

    fn ovpn_with_remote(extra: &str) -> String {
        format!("remote vpn.example.com 1194 udp\n{extra}")
    }

    #[test]
    fn try_from_auth_user_pass_with_file_certs_infers_password_tls() {
        let input = ovpn_with_remote(
            "auth-user-pass\ncert /etc/openvpn/client.crt\nkey /etc/openvpn/client.key",
        );
        let ovpn = parse_ovpn(&input).unwrap();
        let config = OpenVpnConfig::try_from(ovpn).unwrap();
        assert_eq!(config.auth_type, Some(OpenVpnAuthType::PasswordTls));
    }

    #[test]
    fn try_from_auth_user_pass_without_certs_infers_password() {
        let input = ovpn_with_remote("auth-user-pass");
        let ovpn = parse_ovpn(&input).unwrap();
        let config = OpenVpnConfig::try_from(ovpn).unwrap();
        assert_eq!(config.auth_type, Some(OpenVpnAuthType::Password));
    }

    #[test]
    fn try_from_no_auth_user_pass_with_file_certs_infers_tls() {
        let input = ovpn_with_remote("cert /etc/openvpn/client.crt\nkey /etc/openvpn/client.key");
        let ovpn = parse_ovpn(&input).unwrap();
        let config = OpenVpnConfig::try_from(ovpn).unwrap();
        assert_eq!(config.auth_type, Some(OpenVpnAuthType::Tls));
    }

    #[test]
    fn try_from_no_auth_user_pass_no_certs_infers_none() {
        let input = ovpn_with_remote("");
        let ovpn = parse_ovpn(&input).unwrap();
        let config = OpenVpnConfig::try_from(ovpn).unwrap();
        assert_eq!(config.auth_type, None);
    }

    #[test]
    fn try_from_inline_cert_returns_error() {
        let input = ovpn_with_remote("<cert>\nCERTPEM\n</cert>\n<key>\nKEYPEM\n</key>");
        let ovpn = parse_ovpn(&input).unwrap();
        let error = OpenVpnConfig::try_from(ovpn).unwrap_err();
        assert!(matches!(
            error,
            ConnectionError::VpnFailed(message)
                if message.contains("inline <cert> blocks")
                    && message.contains("TryFrom<OvpnFile> cannot handle inline certs")
        ));
    }

    #[test]
    fn try_from_cert_only_without_auth_user_pass_does_not_infer_tls() {
        let input = ovpn_with_remote("cert /etc/openvpn/client.crt");
        let ovpn = parse_ovpn(&input).unwrap();
        let config = OpenVpnConfig::try_from(ovpn).unwrap();
        assert_eq!(config.auth_type, None);
    }

    #[test]
    fn try_from_cert_only_with_auth_user_pass_infers_password_not_password_tls() {
        let input = ovpn_with_remote("auth-user-pass\ncert /etc/openvpn/client.crt");
        let ovpn = parse_ovpn(&input).unwrap();
        let config = OpenVpnConfig::try_from(ovpn).unwrap();
        assert_eq!(config.auth_type, Some(OpenVpnAuthType::Password));
    }
}
