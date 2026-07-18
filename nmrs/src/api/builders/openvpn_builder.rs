//! OpenVPN connection builder with validation.
//!
//! Provides a type-safe builder API for constructing [`OpenVpnConfig`] with
//! validation of required fields and auth-type-specific requirements at
//! build time.
//!
//! Unlike [`super::vpn::build_wireguard_connection`] which returns NM-ready
//! D-Bus settings directly, this builder produces an [`OpenVpnConfig`] domain
//! struct. Use [`super::vpn::build_openvpn_connection`] to convert it into
//! NetworkManager connection settings.

use std::path::Path;

use uuid::Uuid;

use crate::api::models::{
    ConnectionError, OpenVpnAuthType, OpenVpnCompression, OpenVpnConfig, OpenVpnProxy, VpnRoute,
    vpn_route_from_parser,
};
use crate::core::ovpn_parser::parser::{self, CertSource, OvpnFile};
use crate::util::cert_store::store_inline_cert;
use crate::util::validation::validate_connection_name;

/// Builder for OpenVPN connections.
///
/// Validates at build time:
/// - `remote` must be set and non-empty
/// - `auth_type` must be set
/// - `Password` or `PasswordTls`: `username` required
/// - `Tls` or `PasswordTls`: `ca_cert`, `client_cert`, `client_key` required
/// - port must be 1–65535
///
/// # Example
///
/// ```rust
/// use nmrs::builders::OpenVpnBuilder;
/// use nmrs::OpenVpnAuthType;
///
/// let config = OpenVpnBuilder::new("CorpVPN")
///     .remote("vpn.example.com")
///     .port(1194)
///     .auth_type(OpenVpnAuthType::Tls)
///     .ca_cert("/etc/openvpn/ca.crt")
///     .client_cert("/etc/openvpn/client.crt")
///     .client_key("/etc/openvpn/client.key")
///     .build()
///     .expect("Failed to build OpenVPN config");
/// ```
#[non_exhaustive]
#[derive(Debug)]
pub struct OpenVpnBuilder {
    name: String,
    remote: Option<String>,
    port: Option<u16>,
    tcp: bool,
    auth_type: Option<OpenVpnAuthType>,
    auth: Option<String>,
    cipher: Option<String>,
    dns: Option<Vec<String>>,
    mtu: Option<u32>,
    uuid: Option<Uuid>,
    ca_cert: Option<String>,
    client_cert: Option<String>,
    client_key: Option<String>,
    key_password: Option<String>,
    username: Option<String>,
    password: Option<String>,
    compression: Option<OpenVpnCompression>,
    proxy: Option<OpenVpnProxy>,
    tls_auth_key: Option<String>,
    tls_auth_direction: Option<u8>,
    tls_crypt: Option<String>,
    tls_crypt_v2: Option<String>,
    tls_version_min: Option<String>,
    tls_version_max: Option<String>,
    tls_cipher: Option<String>,
    remote_cert_tls: Option<String>,
    verify_x509_name: Option<(String, String)>,
    crl_verify: Option<String>,
    redirect_gateway: bool,
    routes: Vec<VpnRoute>,
    ping: Option<u32>,
    ping_exit: Option<u32>,
    ping_restart: Option<u32>,
    reneg_seconds: Option<u32>,
    connect_timeout: Option<u32>,
    data_ciphers: Option<String>,
    data_ciphers_fallback: Option<String>,
    ncp_disable: bool,
}

impl OpenVpnBuilder {
    /// Creates a new OpenVPN connection builder.
    #[must_use]
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            remote: None,
            port: None,
            tcp: false,
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

    /// Creates a builder pre-populated from a `.ovpn` file on disk.
    ///
    /// Reads the file, parses it, extracts inline certificates (persisting them
    /// via the cert store), and pre-populates the builder. The connection name
    /// defaults to the file stem (e.g. `"corp"` for `corp.ovpn`).
    ///
    /// The caller can override any field before calling [`build()`](Self::build).
    ///
    /// # Errors
    ///
    /// - `ConnectionError::VpnFailed` if the file cannot be read
    /// - `ConnectionError::ParseError` if the `.ovpn` content is malformed
    /// - `ConnectionError::InvalidGateway` if no `remote` directive is found
    pub fn from_ovpn_file(path: impl AsRef<Path>) -> Result<Self, ConnectionError> {
        let path = path.as_ref();
        let content = std::fs::read_to_string(path).map_err(|e| {
            ConnectionError::VpnFailed(format!("failed to read {}: {e}", path.display()))
        })?;
        let name = path
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("openvpn")
            .to_string();
        Self::from_ovpn_str(&content, name)
    }

    /// Creates a builder pre-populated from `.ovpn` file content.
    ///
    /// Parses the content, extracts inline certificates (persisting them via
    /// the cert store under `name`), and pre-populates the builder.
    ///
    /// The caller can override any field before calling [`build()`](Self::build).
    ///
    /// # Errors
    ///
    /// - `ConnectionError::ParseError` if the content is malformed
    /// - `ConnectionError::InvalidGateway` if no `remote` directive is found
    /// - `ConnectionError::VpnFailed` if inline cert storage fails
    pub fn from_ovpn_str(content: &str, name: impl Into<String>) -> Result<Self, ConnectionError> {
        let name = name.into();
        let ovpn = parser::parse_ovpn(content)?;
        Self::from_parsed(ovpn, name)
    }

    /// Populates a builder from a parsed `OvpnFile`, resolving inline certs.
    fn from_parsed(f: OvpnFile, name: String) -> Result<Self, ConnectionError> {
        use crate::core::ovpn_parser::parser::{AllowCompress, Compress};

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

        let resolve_cert =
            |src: CertSource, cert_type: &str, conn: &str| -> Result<String, ConnectionError> {
                match src {
                    CertSource::File(p) => Ok(p),
                    CertSource::Inline(pem) => {
                        let path = store_inline_cert(conn, cert_type, &pem)?;
                        Ok(path.to_string_lossy().into_owned())
                    }
                }
            };

        let ca_cert = f.ca.map(|s| resolve_cert(s, "ca", &name)).transpose()?;
        let client_cert = f.cert.map(|s| resolve_cert(s, "cert", &name)).transpose()?;
        let client_key = f.key.map(|s| resolve_cert(s, "key", &name)).transpose()?;

        let has_client_cert_pair = client_cert.is_some() && client_key.is_some();
        let auth_type = match (f.auth_user_pass, has_client_cert_pair) {
            (true, true) => Some(OpenVpnAuthType::PasswordTls),
            (true, false) => Some(OpenVpnAuthType::Password),
            (false, true) => Some(OpenVpnAuthType::Tls),
            (false, false) => None,
        };

        let (tls_auth_key, tls_auth_direction) = match f.tls_auth {
            Some(ta) => {
                let path = resolve_cert(ta.source, "ta", &name)?;
                (Some(path), ta.key_direction)
            }
            None => (None, None),
        };

        let tls_crypt = f
            .tls_crypt
            .map(|s| resolve_cert(s, "tls-crypt", &name))
            .transpose()?;

        Ok(Self {
            name,
            remote: Some(first_remote.host),
            port: first_remote.port,
            tcp,
            auth_type,
            auth: f.auth,
            cipher: f.cipher,
            dns: None,
            mtu: None,
            uuid: None,
            ca_cert,
            client_cert,
            client_key,
            key_password: None,
            username: None,
            password: None,
            compression,
            proxy: None,
            tls_auth_key,
            tls_auth_direction,
            tls_crypt,
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

    /// Sets the remote server hostname or IP address.
    #[must_use]
    pub fn remote(mut self, remote: impl Into<String>) -> Self {
        self.remote = Some(remote.into());
        self
    }

    /// Sets the remote server port (1–65535).
    #[must_use]
    pub fn port(mut self, port: u16) -> Self {
        self.port = Some(port);
        self
    }

    /// Use TCP instead of UDP.
    #[must_use]
    pub fn tcp(mut self, tcp: bool) -> Self {
        self.tcp = tcp;
        self
    }

    /// Sets the authentication type.
    #[must_use]
    pub fn auth_type(mut self, auth_type: OpenVpnAuthType) -> Self {
        self.auth_type = Some(auth_type);
        self
    }

    /// Sets the HMAC digest algorithm (e.g. "SHA256").
    #[must_use]
    pub fn auth(mut self, auth: impl Into<String>) -> Self {
        self.auth = Some(auth.into());
        self
    }

    /// Sets the data channel cipher (e.g. "AES-256-GCM").
    #[must_use]
    pub fn cipher(mut self, cipher: impl Into<String>) -> Self {
        self.cipher = Some(cipher.into());
        self
    }

    /// Sets DNS servers for the connection.
    #[must_use]
    pub fn dns(mut self, servers: Vec<String>) -> Self {
        self.dns = Some(servers);
        self
    }

    /// Sets the MTU size.
    #[must_use]
    pub fn mtu(mut self, mtu: u32) -> Self {
        self.mtu = Some(mtu);
        self
    }

    /// Sets a specific UUID for the connection.
    #[must_use]
    pub fn uuid(mut self, uuid: Uuid) -> Self {
        self.uuid = Some(uuid);
        self
    }

    /// Sets the CA certificate path.
    #[must_use]
    pub fn ca_cert(mut self, path: impl Into<String>) -> Self {
        self.ca_cert = Some(path.into());
        self
    }

    /// Sets the client certificate path.
    #[must_use]
    pub fn client_cert(mut self, path: impl Into<String>) -> Self {
        self.client_cert = Some(path.into());
        self
    }

    /// Sets the client private key path.
    #[must_use]
    pub fn client_key(mut self, path: impl Into<String>) -> Self {
        self.client_key = Some(path.into());
        self
    }

    /// Sets the password for an encrypted private key.
    #[must_use]
    pub fn key_password(mut self, password: impl Into<String>) -> Self {
        self.key_password = Some(password.into());
        self
    }

    /// Sets the username for password authentication.
    #[must_use]
    pub fn username(mut self, username: impl Into<String>) -> Self {
        self.username = Some(username.into());
        self
    }

    /// Sets the password for password authentication.
    #[must_use]
    pub fn password(mut self, password: impl Into<String>) -> Self {
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
    pub fn compression(mut self, compression: OpenVpnCompression) -> Self {
        self.compression = Some(compression);
        self
    }

    /// Sets the proxy configuration.
    #[must_use]
    pub fn proxy(mut self, proxy: OpenVpnProxy) -> Self {
        self.proxy = Some(proxy);
        self
    }

    /// Sets the TLS authentication key path and optional direction.
    #[must_use]
    pub fn tls_auth(mut self, key_path: impl Into<String>, direction: Option<u8>) -> Self {
        self.tls_auth_key = Some(key_path.into());
        self.tls_auth_direction = direction;
        self
    }

    /// Sets the TLS-Crypt key path.
    #[must_use]
    pub fn tls_crypt(mut self, key_path: impl Into<String>) -> Self {
        self.tls_crypt = Some(key_path.into());
        self
    }

    /// Sets the TLS-Crypt-v2 key path.
    #[must_use]
    pub fn tls_crypt_v2(mut self, key_path: impl Into<String>) -> Self {
        self.tls_crypt_v2 = Some(key_path.into());
        self
    }

    /// Sets the minimum TLS protocol version.
    #[must_use]
    pub fn tls_version_min(mut self, version: impl Into<String>) -> Self {
        self.tls_version_min = Some(version.into());
        self
    }

    /// Sets the maximum TLS protocol version.
    #[must_use]
    pub fn tls_version_max(mut self, version: impl Into<String>) -> Self {
        self.tls_version_max = Some(version.into());
        self
    }

    /// Sets the control channel TLS cipher suites.
    #[must_use]
    pub fn tls_cipher(mut self, cipher: impl Into<String>) -> Self {
        self.tls_cipher = Some(cipher.into());
        self
    }

    /// Requires the remote certificate to be of a specific type.
    #[must_use]
    pub fn remote_cert_tls(mut self, cert_type: impl Into<String>) -> Self {
        self.remote_cert_tls = Some(cert_type.into());
        self
    }

    /// Sets X.509 name verification for the remote certificate.
    #[must_use]
    pub fn verify_x509_name(
        mut self,
        name: impl Into<String>,
        name_type: impl Into<String>,
    ) -> Self {
        self.verify_x509_name = Some((name.into(), name_type.into()));
        self
    }

    /// Sets the path to a Certificate Revocation List.
    #[must_use]
    pub fn crl_verify(mut self, path: impl Into<String>) -> Self {
        self.crl_verify = Some(path.into());
        self
    }

    /// When true, the profile may become the default IPv4 route.
    #[must_use]
    pub fn redirect_gateway(mut self, redirect: bool) -> Self {
        self.redirect_gateway = redirect;
        self
    }

    /// Sets static IPv4 routes for split tunneling.
    #[must_use]
    pub fn routes(mut self, routes: Vec<VpnRoute>) -> Self {
        self.routes = routes;
        self
    }

    /// Sets OpenVPN `ping` (seconds).
    #[must_use]
    pub fn ping(mut self, seconds: u32) -> Self {
        self.ping = Some(seconds);
        self
    }

    /// Sets OpenVPN `ping-exit` (seconds).
    #[must_use]
    pub fn ping_exit(mut self, seconds: u32) -> Self {
        self.ping_exit = Some(seconds);
        self
    }

    /// Sets OpenVPN `ping-restart` (seconds).
    #[must_use]
    pub fn ping_restart(mut self, seconds: u32) -> Self {
        self.ping_restart = Some(seconds);
        self
    }

    /// Sets TLS renegotiation period (`reneg-sec`, seconds).
    #[must_use]
    pub fn reneg_seconds(mut self, seconds: u32) -> Self {
        self.reneg_seconds = Some(seconds);
        self
    }

    /// Sets initial connection timeout (`connect-timeout`, seconds).
    #[must_use]
    pub fn connect_timeout(mut self, seconds: u32) -> Self {
        self.connect_timeout = Some(seconds);
        self
    }

    /// Sets negotiable data ciphers (colon-separated).
    #[must_use]
    pub fn data_ciphers(mut self, ciphers: impl Into<String>) -> Self {
        self.data_ciphers = Some(ciphers.into());
        self
    }

    /// Sets `data-ciphers-fallback`.
    #[must_use]
    pub fn data_ciphers_fallback(mut self, cipher: impl Into<String>) -> Self {
        self.data_ciphers_fallback = Some(cipher.into());
        self
    }

    /// When true, disables NCP (`ncp-disable`).
    #[must_use]
    pub fn ncp_disable(mut self, disable: bool) -> Self {
        self.ncp_disable = disable;
        self
    }

    /// Builds and validates the `OpenVpnConfig`.
    ///
    /// # Errors
    ///
    /// - `ConnectionError::InvalidGateway` if `remote` is not set or empty
    /// - `ConnectionError::InvalidGateway` if `port` is 0
    /// - `ConnectionError::VpnFailed` if `auth_type` is not set
    /// - `ConnectionError::VpnFailed` if `username` is required but missing
    /// - `ConnectionError::VpnFailed` if TLS certs are required but missing
    #[must_use = "the validated OpenVPN config should be used to build connection settings"]
    pub fn build(self) -> Result<OpenVpnConfig, ConnectionError> {
        validate_connection_name(&self.name)?;

        let remote = self
            .remote
            .ok_or_else(|| ConnectionError::InvalidGateway("remote must be set".into()))?;
        if remote.trim().is_empty() {
            return Err(ConnectionError::InvalidGateway(
                "remote must not be empty".into(),
            ));
        }

        // Validate port
        let port = self.port.unwrap_or(1194);
        if port == 0 {
            return Err(ConnectionError::InvalidGateway(
                "port must be between 1 and 65535".into(),
            ));
        }

        // Validate auth_type
        let auth_type = self
            .auth_type
            .ok_or_else(|| ConnectionError::VpnFailed("auth_type must be set".into()))?;

        // auth_type-specific validation
        match &auth_type {
            OpenVpnAuthType::Password | OpenVpnAuthType::PasswordTls if self.username.is_none() => {
                return Err(ConnectionError::VpnFailed(
                    "username is required for Password and PasswordTls auth".into(),
                ));
            }
            _ => {}
        }

        if matches!(auth_type, OpenVpnAuthType::StaticKey) {
            return Err(ConnectionError::VpnFailed(
                "StaticKey auth validation is not yet implemented".into(),
            ));
        }

        match &auth_type {
            OpenVpnAuthType::Tls | OpenVpnAuthType::PasswordTls => {
                if self.ca_cert.is_none() {
                    return Err(ConnectionError::VpnFailed(
                        "ca_cert is required for Tls and PasswordTls auth".into(),
                    ));
                }
                if self.client_cert.is_none() {
                    return Err(ConnectionError::VpnFailed(
                        "client_cert is required for Tls and PasswordTls auth".into(),
                    ));
                }
                if self.client_key.is_none() {
                    return Err(ConnectionError::VpnFailed(
                        "client_key is required for Tls and PasswordTls auth".into(),
                    ));
                }
            }
            _ => {}
        }

        Ok(OpenVpnConfig {
            name: self.name,
            remote,
            port,
            tcp: self.tcp,
            auth_type: Some(auth_type),
            auth: self.auth,
            cipher: self.cipher,
            dns: self.dns,
            mtu: self.mtu,
            uuid: self.uuid,
            ca_cert: self.ca_cert,
            client_cert: self.client_cert,
            client_key: self.client_key,
            key_password: self.key_password,
            username: self.username,
            password: self.password,
            compression: self.compression,
            proxy: self.proxy,
            tls_auth_key: self.tls_auth_key,
            tls_auth_direction: self.tls_auth_direction,
            tls_crypt: self.tls_crypt,
            tls_crypt_v2: self.tls_crypt_v2,
            tls_version_min: self.tls_version_min,
            tls_version_max: self.tls_version_max,
            tls_cipher: self.tls_cipher,
            remote_cert_tls: self.remote_cert_tls,
            verify_x509_name: self.verify_x509_name,
            crl_verify: self.crl_verify,
            redirect_gateway: self.redirect_gateway,
            routes: self.routes,
            ping: self.ping,
            ping_exit: self.ping_exit,
            ping_restart: self.ping_restart,
            reneg_seconds: self.reneg_seconds,
            connect_timeout: self.connect_timeout,
            data_ciphers: self.data_ciphers,
            data_ciphers_fallback: self.data_ciphers_fallback,
            ncp_disable: self.ncp_disable,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tls_builder() -> OpenVpnBuilder {
        OpenVpnBuilder::new("TestVPN")
            .remote("vpn.example.com")
            .port(1194)
            .auth_type(OpenVpnAuthType::Tls)
            .ca_cert("/etc/openvpn/ca.crt")
            .client_cert("/etc/openvpn/client.crt")
            .client_key("/etc/openvpn/client.key")
    }

    fn password_builder() -> OpenVpnBuilder {
        OpenVpnBuilder::new("TestVPN")
            .remote("vpn.example.com")
            .port(1194)
            .auth_type(OpenVpnAuthType::Password)
            .username("user")
    }

    fn assert_stored_material(
        path: Option<String>,
        connection_name: &str,
        filename: &str,
        expected: &str,
    ) {
        let path = std::path::PathBuf::from(path.expect("stored material path"));
        assert!(path.is_absolute());
        assert_eq!(
            path.file_name().and_then(|name| name.to_str()),
            Some(filename)
        );
        assert_eq!(
            path.parent()
                .and_then(std::path::Path::file_name)
                .and_then(|name| name.to_str()),
            Some(connection_name)
        );
        assert_eq!(std::fs::read_to_string(path).unwrap(), expected);
    }

    #[test]
    fn builds_tls_connection() {
        let config = tls_builder().build().unwrap();
        assert_eq!(config.name, "TestVPN");
        assert_eq!(config.remote, "vpn.example.com");
        assert_eq!(config.port, 1194);
        assert!(!config.tcp);
        assert_eq!(config.auth_type, Some(OpenVpnAuthType::Tls));
        assert_eq!(config.ca_cert.as_deref(), Some("/etc/openvpn/ca.crt"));
        assert_eq!(
            config.client_cert.as_deref(),
            Some("/etc/openvpn/client.crt")
        );
        assert_eq!(
            config.client_key.as_deref(),
            Some("/etc/openvpn/client.key")
        );
    }

    #[test]
    fn builds_password_connection() {
        let config = password_builder().build().unwrap();
        assert_eq!(config.auth_type, Some(OpenVpnAuthType::Password));
        assert_eq!(config.username.as_deref(), Some("user"));
        assert!(config.ca_cert.is_none());
        assert!(config.client_cert.is_none());
        assert!(config.client_key.is_none());
    }

    #[test]
    fn builds_password_tls_connection() {
        let config = OpenVpnBuilder::new("TestVPN")
            .remote("vpn.example.com")
            .auth_type(OpenVpnAuthType::PasswordTls)
            .username("user")
            .ca_cert("/etc/openvpn/ca.crt")
            .client_cert("/etc/openvpn/client.crt")
            .client_key("/etc/openvpn/client.key")
            .build()
            .unwrap();
        assert_eq!(config.auth_type, Some(OpenVpnAuthType::PasswordTls));
        assert_eq!(config.username.as_deref(), Some("user"));
        assert_eq!(config.ca_cert.as_deref(), Some("/etc/openvpn/ca.crt"));
        assert_eq!(
            config.client_cert.as_deref(),
            Some("/etc/openvpn/client.crt")
        );
        assert_eq!(
            config.client_key.as_deref(),
            Some("/etc/openvpn/client.key")
        );
    }

    #[test]
    fn rejects_static_key_unimplemented() {
        let result = OpenVpnBuilder::new("TestVPN")
            .remote("vpn.example.com")
            .auth_type(OpenVpnAuthType::StaticKey)
            .build();
        assert!(matches!(
            result.unwrap_err(),
            ConnectionError::VpnFailed(message)
                if message == "StaticKey auth validation is not yet implemented"
        ));
    }

    #[test]
    fn defaults_port_to_1194() {
        let config = tls_builder().build().unwrap();
        assert_eq!(config.port, 1194);
    }

    #[test]
    fn sets_tcp_flag() {
        let config = tls_builder().tcp(true).build().unwrap();
        assert!(config.tcp);
    }

    #[test]
    fn sets_optional_fields() {
        let config = tls_builder()
            .auth("SHA256")
            .cipher("AES-256-GCM")
            .mtu(1400)
            .dns(vec!["1.1.1.1".into()])
            .build()
            .unwrap();
        assert_eq!(config.auth, Some("SHA256".into()));
        assert_eq!(config.cipher, Some("AES-256-GCM".into()));
        assert_eq!(config.mtu, Some(1400));
        assert_eq!(config.dns, Some(vec!["1.1.1.1".to_string()]));
    }

    #[test]
    fn sets_compression() {
        let config = tls_builder()
            .compression(OpenVpnCompression::Lz4V2)
            .build()
            .unwrap();
        assert_eq!(config.compression, Some(OpenVpnCompression::Lz4V2));
    }

    #[test]
    fn sets_proxy() {
        let config = tls_builder()
            .proxy(OpenVpnProxy::Http {
                server: "proxy.example.com".into(),
                port: 8080,
                username: None,
                password: None,
                retry: false,
            })
            .build()
            .unwrap();
        assert_eq!(
            config.proxy,
            Some(OpenVpnProxy::Http {
                server: "proxy.example.com".into(),
                port: 8080,
                username: None,
                password: None,
                retry: false,
            })
        );
    }

    #[test]
    fn rejects_empty_name() {
        let result = OpenVpnBuilder::new("")
            .remote("vpn.example.com")
            .auth_type(OpenVpnAuthType::Tls)
            .ca_cert("/etc/openvpn/ca.crt")
            .client_cert("/etc/openvpn/client.crt")
            .client_key("/etc/openvpn/client.key")
            .build();
        assert!(matches!(
            result.unwrap_err(),
            ConnectionError::InvalidAddress(message)
                if message == "Connection name cannot be empty"
        ));
    }

    #[test]
    fn requires_remote() {
        let result = OpenVpnBuilder::new("TestVPN")
            .auth_type(OpenVpnAuthType::Tls)
            .ca_cert("/etc/openvpn/ca.crt")
            .client_cert("/etc/openvpn/client.crt")
            .client_key("/etc/openvpn/client.key")
            .build();
        assert!(matches!(
            result.unwrap_err(),
            ConnectionError::InvalidGateway(message) if message == "remote must be set"
        ));
    }

    #[test]
    fn rejects_empty_remote() {
        let result = OpenVpnBuilder::new("TestVPN")
            .remote("")
            .auth_type(OpenVpnAuthType::Tls)
            .ca_cert("/etc/openvpn/ca.crt")
            .client_cert("/etc/openvpn/client.crt")
            .client_key("/etc/openvpn/client.key")
            .build();
        assert!(matches!(
            result.unwrap_err(),
            ConnectionError::InvalidGateway(message) if message == "remote must not be empty"
        ));
    }

    #[test]
    fn rejects_zero_port() {
        let result = tls_builder().port(0).build();
        assert!(matches!(
            result.unwrap_err(),
            ConnectionError::InvalidGateway(message)
                if message == "port must be between 1 and 65535"
        ));
    }

    #[test]
    fn requires_auth_type() {
        let result = OpenVpnBuilder::new("TestVPN")
            .remote("vpn.example.com")
            .build();
        assert!(matches!(
            result.unwrap_err(),
            ConnectionError::VpnFailed(message) if message == "auth_type must be set"
        ));
    }

    #[test]
    fn requires_username_for_password_auth() {
        let result = OpenVpnBuilder::new("TestVPN")
            .remote("vpn.example.com")
            .auth_type(OpenVpnAuthType::Password)
            .build();
        assert!(matches!(
            result.unwrap_err(),
            ConnectionError::VpnFailed(message)
                if message == "username is required for Password and PasswordTls auth"
        ));
    }

    #[test]
    fn requires_username_for_password_tls_auth() {
        let result = OpenVpnBuilder::new("TestVPN")
            .remote("vpn.example.com")
            .auth_type(OpenVpnAuthType::PasswordTls)
            .ca_cert("/etc/openvpn/ca.crt")
            .client_cert("/etc/openvpn/client.crt")
            .client_key("/etc/openvpn/client.key")
            .build();
        assert!(matches!(
            result.unwrap_err(),
            ConnectionError::VpnFailed(message)
                if message == "username is required for Password and PasswordTls auth"
        ));
    }

    #[test]
    fn requires_ca_cert_for_tls_auth() {
        let result = OpenVpnBuilder::new("TestVPN")
            .remote("vpn.example.com")
            .auth_type(OpenVpnAuthType::Tls)
            .client_cert("/etc/openvpn/client.crt")
            .client_key("/etc/openvpn/client.key")
            .build();
        assert!(matches!(
            result.unwrap_err(),
            ConnectionError::VpnFailed(message)
                if message == "ca_cert is required for Tls and PasswordTls auth"
        ));
    }

    #[test]
    fn requires_client_cert_for_tls_auth() {
        let result = OpenVpnBuilder::new("TestVPN")
            .remote("vpn.example.com")
            .auth_type(OpenVpnAuthType::Tls)
            .ca_cert("/etc/openvpn/ca.crt")
            .client_key("/etc/openvpn/client.key")
            .build();
        assert!(matches!(
            result.unwrap_err(),
            ConnectionError::VpnFailed(message)
                if message == "client_cert is required for Tls and PasswordTls auth"
        ));
    }

    #[test]
    fn requires_client_key_for_tls_auth() {
        let result = OpenVpnBuilder::new("TestVPN")
            .remote("vpn.example.com")
            .auth_type(OpenVpnAuthType::Tls)
            .ca_cert("/etc/openvpn/ca.crt")
            .client_cert("/etc/openvpn/client.crt")
            .build();
        assert!(matches!(
            result.unwrap_err(),
            ConnectionError::VpnFailed(message)
                if message == "client_key is required for Tls and PasswordTls auth"
        ));
    }

    // --- from_ovpn_str tests ---

    use crate::util::test_utils::with_fake_xdg;

    #[test]
    fn from_ovpn_str_basic_tls_file_certs() {
        let ovpn = "\
remote vpn.example.com 1194 udp
ca /etc/openvpn/ca.crt
cert /etc/openvpn/client.crt
key /etc/openvpn/client.key
";
        let builder = OpenVpnBuilder::from_ovpn_str(ovpn, "test-tls").unwrap();
        let config = builder.build().unwrap();
        assert_eq!(config.remote, "vpn.example.com");
        assert_eq!(config.port, 1194);
        assert!(!config.tcp);
        assert_eq!(config.auth_type, Some(OpenVpnAuthType::Tls));
        assert_eq!(config.ca_cert, Some("/etc/openvpn/ca.crt".into()));
        assert_eq!(config.client_cert, Some("/etc/openvpn/client.crt".into()));
        assert_eq!(config.client_key, Some("/etc/openvpn/client.key".into()));
    }

    #[test]
    fn from_ovpn_str_password_auth() {
        let ovpn = "remote vpn.example.com 443 tcp\nauth-user-pass\n";
        let builder = OpenVpnBuilder::from_ovpn_str(ovpn, "test-pw")
            .unwrap()
            .username("user");
        let config = builder.build().unwrap();
        assert_eq!(config.auth_type, Some(OpenVpnAuthType::Password));
        assert!(config.tcp);
        assert_eq!(config.port, 443);
    }

    #[test]
    fn from_ovpn_str_inline_certs_stored() {
        with_fake_xdg(|| {
            let ovpn = "\
remote vpn.example.com 1194
<ca>
-----BEGIN CERTIFICATE-----
FAKECA
-----END CERTIFICATE-----
</ca>
<cert>
-----BEGIN CERTIFICATE-----
FAKECERT
-----END CERTIFICATE-----
</cert>
<key>
-----BEGIN PRIVATE KEY-----
FAKEKEY
-----END PRIVATE KEY-----
</key>
";
            let builder = OpenVpnBuilder::from_ovpn_str(ovpn, "inline-test").unwrap();
            let config = builder.build().unwrap();
            assert_eq!(config.auth_type, Some(OpenVpnAuthType::Tls));
            assert_stored_material(
                config.ca_cert,
                "inline-test",
                "ca.pem",
                "-----BEGIN CERTIFICATE-----\nFAKECA\n-----END CERTIFICATE-----\n",
            );
            assert_stored_material(
                config.client_cert,
                "inline-test",
                "cert.pem",
                "-----BEGIN CERTIFICATE-----\nFAKECERT\n-----END CERTIFICATE-----\n",
            );
            assert_stored_material(
                config.client_key,
                "inline-test",
                "key.pem",
                "-----BEGIN PRIVATE KEY-----\nFAKEKEY\n-----END PRIVATE KEY-----\n",
            );
        });
    }

    #[test]
    fn from_ovpn_str_tls_auth_with_direction() {
        let ovpn = "\
remote vpn.example.com 1194
ca /etc/openvpn/ca.crt
cert /etc/openvpn/client.crt
key /etc/openvpn/client.key
tls-auth /etc/openvpn/ta.key 1
";
        let builder = OpenVpnBuilder::from_ovpn_str(ovpn, "test-ta").unwrap();
        let config = builder.build().unwrap();
        assert_eq!(config.tls_auth_key, Some("/etc/openvpn/ta.key".into()));
        assert_eq!(config.tls_auth_direction, Some(1));
    }

    #[test]
    fn from_ovpn_str_inline_tls_auth() {
        with_fake_xdg(|| {
            let ovpn = "\
remote vpn.example.com 1194
ca /etc/openvpn/ca.crt
cert /etc/openvpn/client.crt
key /etc/openvpn/client.key
key-direction 0
<tls-auth>
-----BEGIN OpenVPN Static key V1-----
FAKEKEY
-----END OpenVPN Static key V1-----
</tls-auth>
";
            let builder = OpenVpnBuilder::from_ovpn_str(ovpn, "inline-ta").unwrap();
            let config = builder.build().unwrap();
            assert_stored_material(
                config.tls_auth_key,
                "inline-ta",
                "ta.key",
                "-----BEGIN OpenVPN Static key V1-----\nFAKEKEY\n-----END OpenVPN Static key V1-----\n",
            );
            assert_eq!(config.tls_auth_direction, Some(0));
        });
    }

    #[test]
    fn from_ovpn_str_compression_lz4() {
        let ovpn = "\
remote vpn.example.com 1194
ca /etc/openvpn/ca.crt
cert /etc/openvpn/client.crt
key /etc/openvpn/client.key
compress lz4-v2
";
        let builder = OpenVpnBuilder::from_ovpn_str(ovpn, "test-comp").unwrap();
        let config = builder.build().unwrap();
        assert_eq!(config.compression, Some(OpenVpnCompression::Lz4V2));
    }

    #[test]
    fn from_ovpn_str_cipher_and_auth() {
        let ovpn = "\
remote vpn.example.com 1194
ca /etc/openvpn/ca.crt
cert /etc/openvpn/client.crt
key /etc/openvpn/client.key
cipher AES-256-GCM
auth SHA256
";
        let builder = OpenVpnBuilder::from_ovpn_str(ovpn, "test-cipher").unwrap();
        let config = builder.build().unwrap();
        assert_eq!(config.cipher, Some("AES-256-GCM".into()));
        assert_eq!(config.auth, Some("SHA256".into()));
    }

    #[test]
    fn from_ovpn_str_caller_can_override() {
        let ovpn = "\
remote vpn.example.com 1194
ca /etc/openvpn/ca.crt
cert /etc/openvpn/client.crt
key /etc/openvpn/client.key
";
        let config = OpenVpnBuilder::from_ovpn_str(ovpn, "test-override")
            .unwrap()
            .port(443)
            .tcp(true)
            .dns(vec!["1.1.1.1".into()])
            .build()
            .unwrap();
        assert_eq!(config.port, 443);
        assert!(config.tcp);
        assert_eq!(config.dns, Some(vec!["1.1.1.1".to_string()]));
    }

    #[test]
    fn from_ovpn_str_no_remote_fails() {
        let ovpn = "cipher AES-256-GCM\n";
        let result = OpenVpnBuilder::from_ovpn_str(ovpn, "test-fail");
        assert!(matches!(
            result.unwrap_err(),
            ConnectionError::InvalidGateway(message) if message == "no remote in .ovpn file"
        ));
    }

    #[test]
    fn from_ovpn_file_reads_and_parses() {
        let dir = std::env::temp_dir().join(format!("nmrs-ovpn-file-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("corp.ovpn");
        std::fs::write(&path, "remote vpn.corp.com 1194\nauth-user-pass\n").unwrap();

        let builder = OpenVpnBuilder::from_ovpn_file(&path).unwrap();
        assert_eq!(builder.name, "corp");
        let config = builder.username("user").build().unwrap();
        assert_eq!(config.remote, "vpn.corp.com");
        assert_eq!(config.auth_type, Some(OpenVpnAuthType::Password));

        let _ = std::fs::remove_dir_all(&dir);
    }
}
