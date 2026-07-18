//! Input validation utilities for NetworkManager operations.
//!
//! This module provides validation functions for various inputs to ensure
//! they meet NetworkManager's requirements before attempting D-Bus operations.

#![allow(deprecated)]

use crate::api::models::{
    ConnectionError, OpenVpnAuthType, OpenVpnConfig, OpenVpnProxy, VpnCredentials, WifiSecurity,
    WireGuardPeer,
};
use crate::{EapMethod, EapOptions};

/// Maximum SSID length in bytes (802.11 standard).
const MAX_SSID_BYTES: usize = 32;

/// WireGuard key length in bytes (before base64 encoding).
const WIREGUARD_KEY_BYTES: usize = 32;

/// WireGuard key length in base64 characters (with padding).
const WIREGUARD_KEY_BASE64_LEN: usize = 44;

/// Minimum WPA-PSK password length (WPA standard).
const MIN_WPA_PSK_LENGTH: usize = 8;

/// Maximum WPA-PSK password length (WPA standard).
const MAX_WPA_PSK_LENGTH: usize = 63;

/// Validates an SSID or connection name string.
///
/// # Rules
/// - Must not be empty (unless explicitly allowed for hidden networks)
/// - Must not exceed 32 bytes when encoded as UTF-8
/// - Should not contain only whitespace
///
/// # Errors
/// Returns `ConnectionError::InvalidAddress` if the SSID is invalid.
pub fn validate_ssid(ssid: &str) -> Result<(), ConnectionError> {
    // Check if empty
    if ssid.is_empty() {
        return Err(ConnectionError::InvalidAddress(
            "SSID cannot be empty".to_string(),
        ));
    }

    // Check if only whitespace
    if ssid.trim().is_empty() {
        return Err(ConnectionError::InvalidAddress(
            "SSID cannot be only whitespace".to_string(),
        ));
    }

    // Check byte length (802.11 standard allows up to 32 bytes)
    if ssid.len() > MAX_SSID_BYTES {
        return Err(ConnectionError::InvalidAddress(format!(
            "SSID too long: {} bytes (max {} bytes)",
            ssid.len(),
            MAX_SSID_BYTES
        )));
    }

    Ok(())
}

/// Validates a connection name (for VPN, etc.).
///
/// Similar to SSID validation but allows slightly more flexibility.
/// Used for VPN connection names and other non-WiFi connection names.
///
/// # Rules
/// - Must not be empty
/// - Should not contain only whitespace
/// - Must not exceed 255 bytes (reasonable limit for connection names)
///
/// # Errors
/// Returns `ConnectionError::InvalidAddress` if the name is invalid.
pub fn validate_connection_name(name: &str) -> Result<(), ConnectionError> {
    // Check if empty
    if name.is_empty() {
        return Err(ConnectionError::InvalidAddress(
            "Connection name cannot be empty".to_string(),
        ));
    }

    // Check if only whitespace
    if name.trim().is_empty() {
        return Err(ConnectionError::InvalidAddress(
            "Connection name cannot be only whitespace".to_string(),
        ));
    }

    // Check byte length (reasonable limit for connection names)
    if name.len() > 255 {
        return Err(ConnectionError::InvalidAddress(format!(
            "Connection name too long: {} bytes (max 255 bytes)",
            name.len()
        )));
    }

    Ok(())
}

/// Validates WiFi security credentials.
///
/// # Rules
/// - WPA-PSK: Password must be 8-63 characters (WPA standard)
/// - WPA-EAP: Identity and password must not be empty
/// - Open: No validation needed
///
/// # Errors
/// Returns appropriate `ConnectionError` if credentials are invalid.
pub fn validate_wifi_security(security: &WifiSecurity) -> Result<(), ConnectionError> {
    match security {
        WifiSecurity::Open => Ok(()),

        WifiSecurity::WpaPsk { psk } => {
            // Allow empty PSK only if user wants to use saved credentials
            if psk.is_empty() {
                return Ok(());
            }

            let psk_len = psk.len();

            if psk_len < MIN_WPA_PSK_LENGTH {
                return Err(ConnectionError::InvalidAddress(format!(
                    "WPA-PSK password too short: {} characters (minimum {} characters)",
                    psk_len, MIN_WPA_PSK_LENGTH
                )));
            }

            if psk_len > MAX_WPA_PSK_LENGTH {
                return Err(ConnectionError::InvalidAddress(format!(
                    "WPA-PSK password too long: {} characters (maximum {} characters)",
                    psk_len, MAX_WPA_PSK_LENGTH
                )));
            }

            Ok(())
        }

        WifiSecurity::WpaEap { opts } => {
            validate_wifi_eap(opts)?;

            Ok(())
        }

        WifiSecurity::Wpa3Eap192bit { opts } => {
            if opts.method != EapMethod::Tls {
                return Err(ConnectionError::InvalidAddress(
                    "WPA3-EAP 192bit requires authentication method TLS".to_string(),
                ));
            }

            validate_wifi_eap(opts)?;

            Ok(())
        }
    }
}

fn validate_wifi_eap(opts: &EapOptions) -> Result<(), ConnectionError> {
    // Validate identity
    if opts.identity.trim().is_empty() {
        return Err(ConnectionError::InvalidAddress(
            "EAP identity cannot be empty".to_string(),
        ));
    }

    match opts.method {
        EapMethod::Peap | EapMethod::Ttls => {
            // Validate password
            if opts.password.is_empty() {
                return Err(ConnectionError::InvalidAddress(
                    "EAP password cannot be empty".to_string(),
                ));
            }

            // Validate anonymous identity if provided
            if let Some(ref anon_id) = opts.anonymous_identity
                && anon_id.trim().is_empty()
            {
                return Err(ConnectionError::InvalidAddress(
                    "EAP anonymous identity cannot be empty if provided".to_string(),
                ));
            }

            // Validate domain suffix match if provided
            if let Some(ref domain) = opts.domain_suffix_match
                && domain.trim().is_empty()
            {
                return Err(ConnectionError::InvalidAddress(
                    "EAP domain suffix match cannot be empty if provided".to_string(),
                ));
            }
        }
        EapMethod::Tls => {
            if !validate_path_or_blob(
                "EAP private key",
                &opts.private_key_path,
                &opts.private_key_blob,
            )? {
                return Err(ConnectionError::InvalidAddress(
                    "EAP private key must be provided".to_string(),
                ));
            }

            if !validate_path_or_blob(
                "EAP client certificate",
                &opts.client_cert_path,
                &opts.client_cert_blob,
            )? {
                return Err(ConnectionError::InvalidAddress(
                    "EAP client certificate must be provided".to_string(),
                ));
            }
        }
    }

    validate_path_or_blob("EAP CA certificate", &opts.ca_cert_path, &opts.ca_cert_blob)?;

    Ok(())
}

fn validate_path_or_blob(
    field: &str,
    path: &Option<String>,
    blob: &Option<Vec<u8>>,
) -> Result<bool, ConnectionError> {
    // Validate CA cert path if provided
    match (path, blob) {
        (None, None) => Ok(false),
        (Some(path), None) => {
            if path.trim().is_empty() {
                return Err(ConnectionError::InvalidAddress(format!(
                    "{field} path cannot be empty if provided"
                )));
            }
            // Check if it starts with file:// as required by NetworkManager
            if !path.starts_with("file://") {
                return Err(ConnectionError::InvalidAddress(format!(
                    "{field} path must start with 'file://'"
                )));
            }
            Ok(true)
        }
        (None, Some(_)) => Ok(true),
        (Some(_), Some(_)) => Err(ConnectionError::InvalidAddress(format!(
            "{field} path and blob cannot be provided at the same time"
        ))),
    }
}

/// Validates a WireGuard private or public key.
///
/// # Rules
/// - Must be valid base64
/// - Must decode to exactly 32 bytes
/// - Must be 44 characters long (base64 with padding)
///
/// # Errors
/// Returns `ConnectionError::InvalidPrivateKey` or `InvalidPublicKey` if invalid.
fn validate_wireguard_key(key: &str, key_type: &str) -> Result<(), ConnectionError> {
    if key.is_empty() {
        return Err(invalid_wireguard_key(
            key_type,
            format!("{} cannot be empty", key_type),
        ));
    }

    // Check length (base64 encoded 32 bytes = 44 chars with padding)
    if key.len() != WIREGUARD_KEY_BASE64_LEN {
        return Err(invalid_wireguard_key(
            key_type,
            format!(
                "{} must be {} characters (base64 encoded), got {}",
                key_type,
                WIREGUARD_KEY_BASE64_LEN,
                key.len()
            ),
        ));
    }

    // Validate base64 and length
    match base64::Engine::decode(&base64::engine::general_purpose::STANDARD, key) {
        Ok(decoded) => {
            if decoded.len() != WIREGUARD_KEY_BYTES {
                return Err(invalid_wireguard_key(
                    key_type,
                    format!(
                        "{} must decode to {} bytes, got {}",
                        key_type,
                        WIREGUARD_KEY_BYTES,
                        decoded.len()
                    ),
                ));
            }
            Ok(())
        }
        Err(e) => Err(invalid_wireguard_key(
            key_type,
            format!("{} is not valid base64: {}", key_type, e),
        )),
    }
}

fn invalid_wireguard_key(key_type: &str, message: String) -> ConnectionError {
    if key_type.to_ascii_lowercase().contains("public key") {
        ConnectionError::InvalidPublicKey(message)
    } else {
        ConnectionError::InvalidPrivateKey(message)
    }
}

/// Validates a WireGuard peer configuration.
///
/// # Rules
/// - Public key must be valid base64 and 32 bytes
/// - Gateway must be in "host:port" format
/// - Allowed IPs must be valid CIDR notation
/// - Preshared key (if provided) must be valid base64 and 32 bytes
///
/// # Errors
/// Returns appropriate `ConnectionError` if peer configuration is invalid.
fn validate_wireguard_peer(peer: &WireGuardPeer) -> Result<(), ConnectionError> {
    // Validate public key
    validate_wireguard_key(&peer.public_key, "Peer public key")?;

    // Validate gateway (should be host:port)
    validate_wireguard_gateway(&peer.gateway, "Peer")?;

    // Validate allowed IPs
    if peer.allowed_ips.is_empty() {
        return Err(ConnectionError::InvalidPeers(
            "Peer must have at least one allowed IP range".to_string(),
        ));
    }

    for allowed_ip in &peer.allowed_ips {
        validate_cidr(allowed_ip)?;
    }

    // Validate preshared key if provided
    if let Some(ref psk) = peer.preshared_key {
        validate_wireguard_key(psk, "Peer preshared key")?;
    }

    // Validate persistent keepalive if provided
    if let Some(keepalive) = peer.persistent_keepalive {
        if keepalive == 0 {
            return Err(ConnectionError::InvalidPeers(
                "Persistent keepalive must be greater than 0 if specified".to_string(),
            ));
        }
        if keepalive > 65535 {
            return Err(ConnectionError::InvalidPeers(format!(
                "Persistent keepalive too large: {} (max 65535)",
                keepalive
            )));
        }
    }

    Ok(())
}

fn validate_wireguard_gateway(gateway: &str, label: &str) -> Result<(), ConnectionError> {
    if gateway.trim().is_empty() {
        return Err(ConnectionError::InvalidGateway(format!(
            "{label} gateway cannot be empty"
        )));
    }

    let (host, port_str) = gateway.rsplit_once(':').ok_or_else(|| {
        ConnectionError::InvalidGateway(format!(
            "{label} gateway must be in 'host:port' format, got '{gateway}'"
        ))
    })?;
    if host.trim().is_empty() {
        return Err(ConnectionError::InvalidGateway(format!(
            "{label} gateway host cannot be empty"
        )));
    }
    if host.contains(':')
        && !(host.starts_with('[')
            && host.ends_with(']')
            && host[1..host.len() - 1]
                .parse::<std::net::Ipv6Addr>()
                .is_ok())
    {
        return Err(ConnectionError::InvalidGateway(format!(
            "{label} IPv6 gateway must use '[address]:port' format, got '{gateway}'"
        )));
    }

    let port = port_str.parse::<u16>().map_err(|_| {
        ConnectionError::InvalidGateway(format!("Invalid port number in gateway '{gateway}'"))
    })?;
    if port == 0 {
        return Err(ConnectionError::InvalidGateway(format!(
            "Port number in gateway '{gateway}' cannot be 0"
        )));
    }

    Ok(())
}

/// Validates CIDR notation (e.g., "10.0.0.0/24" or "2001:db8::/32").
///
/// # Errors
/// Returns `ConnectionError::InvalidAddress` if CIDR is invalid.
fn validate_cidr(cidr: &str) -> Result<(), ConnectionError> {
    if cidr.is_empty() {
        return Err(ConnectionError::InvalidAddress(
            "CIDR notation cannot be empty".to_string(),
        ));
    }

    let parts: Vec<&str> = cidr.split('/').collect();
    if parts.len() != 2 {
        return Err(ConnectionError::InvalidAddress(format!(
            "Invalid CIDR notation '{}' (must be 'address/prefix')",
            cidr
        )));
    }

    let address = parts[0];
    let prefix = parts[1];

    let prefix_num = prefix.parse::<u8>().map_err(|_| {
        ConnectionError::InvalidAddress(format!(
            "Invalid prefix length '{}' in CIDR '{}'",
            prefix, cidr
        ))
    })?;

    if address.contains(':') {
        // IPv6
        if prefix_num > 128 {
            return Err(ConnectionError::InvalidAddress(format!(
                "IPv6 prefix length {} is too large (max 128)",
                prefix_num
            )));
        }
        if address.parse::<std::net::Ipv6Addr>().is_err() {
            return Err(ConnectionError::InvalidAddress(format!(
                "Invalid IPv6 address '{}'",
                address
            )));
        }
    } else {
        // IPv4
        if prefix_num > 32 {
            return Err(ConnectionError::InvalidAddress(format!(
                "IPv4 prefix length {} is too large (max 32)",
                prefix_num
            )));
        }
        // Validate IPv4 format
        let octets: Vec<&str> = address.split('.').collect();
        if octets.len() != 4 {
            return Err(ConnectionError::InvalidAddress(format!(
                "Invalid IPv4 address '{}' (must have 4 octets)",
                address
            )));
        }
        for octet in octets {
            let num = octet.parse::<u16>().map_err(|_| {
                ConnectionError::InvalidAddress(format!("Invalid IPv4 octet '{}'", octet))
            })?;
            if num > 255 {
                return Err(ConnectionError::InvalidAddress(format!(
                    "IPv4 octet {} is too large (max 255)",
                    num
                )));
            }
        }
    }

    Ok(())
}

/// Validates VPN credentials.
///
/// # Rules
/// - Name must not be empty
/// - Gateway must be in "host:port" format
/// - Private key must be valid base64 and 32 bytes
/// - Address must be valid CIDR notation
/// - At least one peer must be configured
/// - All peers must be valid
/// - DNS servers (if provided) must be valid IP addresses
/// - MTU (if provided) must be reasonable (576-9000)
///
/// # Errors
/// Returns appropriate `ConnectionError` if credentials are invalid.
pub fn validate_vpn_credentials(creds: &VpnCredentials) -> Result<(), ConnectionError> {
    // Validate name
    validate_connection_name(&creds.name)?;

    // Validate gateway
    validate_wireguard_gateway(&creds.gateway, "VPN")?;

    // Validate private key
    validate_wireguard_key(&creds.private_key, "Private key")?;

    // Validate address (must be CIDR notation)
    validate_cidr(&creds.address)?;

    // Validate peers
    if creds.peers.is_empty() {
        return Err(ConnectionError::InvalidPeers(
            "VPN must have at least one peer configured".to_string(),
        ));
    }

    for (i, peer) in creds.peers.iter().enumerate() {
        validate_wireguard_peer(peer).map_err(|e| match e {
            ConnectionError::InvalidPeers(msg) => {
                ConnectionError::InvalidPeers(format!("Peer {}: {}", i, msg))
            }
            ConnectionError::InvalidGateway(msg) => {
                ConnectionError::InvalidGateway(format!("Peer {}: {}", i, msg))
            }
            ConnectionError::InvalidPublicKey(msg) => {
                ConnectionError::InvalidPublicKey(format!("Peer {}: {}", i, msg))
            }
            other => other,
        })?;
    }

    // Validate DNS servers if provided
    if let Some(ref dns_servers) = creds.dns {
        if dns_servers.is_empty() {
            return Err(ConnectionError::InvalidAddress(
                "DNS server list cannot be empty if provided".to_string(),
            ));
        }

        for dns in dns_servers {
            validate_ip_address(dns)?;
        }
    }

    validate_mtu(creds.mtu)?;

    Ok(())
}

/// Validates an IP address (IPv4 or IPv6).
///
/// # Errors
/// Returns `ConnectionError::InvalidAddress` if the IP address is invalid.
fn validate_ip_address(ip: &str) -> Result<(), ConnectionError> {
    if ip.is_empty() {
        return Err(ConnectionError::InvalidAddress(
            "IP address cannot be empty".to_string(),
        ));
    }

    if ip.contains(':') {
        if ip.parse::<std::net::Ipv6Addr>().is_err() {
            return Err(ConnectionError::InvalidAddress(format!(
                "Invalid IPv6 address '{}'",
                ip
            )));
        }
    } else {
        let octets: Vec<&str> = ip.split('.').collect();
        if octets.len() != 4 {
            return Err(ConnectionError::InvalidAddress(format!(
                "Invalid IPv4 address '{}' (must have 4 octets)",
                ip
            )));
        }
        for octet in octets {
            let num = octet.parse::<u16>().map_err(|_| {
                ConnectionError::InvalidAddress(format!(
                    "Invalid IPv4 octet '{}' in address '{}'",
                    octet, ip
                ))
            })?;
            if num > 255 {
                return Err(ConnectionError::InvalidAddress(format!(
                    "IPv4 octet {} is too large (max 255) in address '{}'",
                    num, ip
                )));
            }
        }
    }

    Ok(())
}

/// Validates an OpenVPN configuration.
///
/// # Rules
/// - Connection name must be valid (via [`validate_connection_name`])
/// - Remote server must not be empty
/// - Port is validated at the type level (`u16`), no extra check needed
/// - Auth-type-specific required fields:
///   - `Password`: username must be set
///   - `Tls`: CA cert, client cert, and client key must be set
///   - `PasswordTls`: username plus all TLS cert paths must be set
///   - `StaticKey`: no additional fields required
/// - Cert paths (if set) must be non-empty strings
/// - DNS servers (if provided) must be valid IP addresses
/// - MTU (if provided) must be in 576–9000
/// - Proxy server (if provided) must not be empty
///
/// # Errors
/// Returns appropriate `ConnectionError` if the configuration is invalid.
pub fn validate_openvpn_config(config: &OpenVpnConfig) -> Result<(), ConnectionError> {
    validate_connection_name(&config.name)?;

    if config.remote.trim().is_empty() {
        return Err(ConnectionError::InvalidGateway(
            "OpenVPN remote server cannot be empty".to_string(),
        ));
    }

    if let Some(ref auth_type) = config.auth_type {
        match auth_type {
            OpenVpnAuthType::Password => {
                if config.username.as_deref().unwrap_or("").trim().is_empty() {
                    return Err(ConnectionError::InvalidAddress(
                        "Username is required for password authentication".to_string(),
                    ));
                }
            }
            OpenVpnAuthType::Tls => {
                validate_openvpn_cert_paths(config)?;
            }
            OpenVpnAuthType::PasswordTls => {
                if config.username.as_deref().unwrap_or("").trim().is_empty() {
                    return Err(ConnectionError::InvalidAddress(
                        "Username is required for password+TLS authentication".to_string(),
                    ));
                }
                validate_openvpn_cert_paths(config)?;
            }
            OpenVpnAuthType::StaticKey => {}
        }
    }

    validate_optional_cert_path(&config.ca_cert, "CA certificate")?;
    validate_optional_cert_path(&config.client_cert, "Client certificate")?;
    validate_optional_cert_path(&config.client_key, "Client key")?;

    if let Some(ref dns_servers) = config.dns {
        if dns_servers.is_empty() {
            return Err(ConnectionError::InvalidAddress(
                "DNS server list cannot be empty if provided".to_string(),
            ));
        }
        for dns in dns_servers {
            validate_ip_address(dns)?;
        }
    }

    validate_mtu(config.mtu)?;

    if let Some(ref proxy) = config.proxy {
        match proxy {
            OpenVpnProxy::Http { server, .. } | OpenVpnProxy::Socks { server, .. } => {
                if server.trim().is_empty() {
                    return Err(ConnectionError::InvalidAddress(
                        "Proxy server address cannot be empty".to_string(),
                    ));
                }
            }
        }
    }

    for route in &config.routes {
        if route.dest.trim().is_empty() {
            return Err(ConnectionError::InvalidAddress(
                "OpenVPN route destination cannot be empty".to_string(),
            ));
        }
        if route.dest.parse::<std::net::Ipv4Addr>().is_err() {
            return Err(ConnectionError::InvalidAddress(format!(
                "Invalid OpenVPN route destination '{}'",
                route.dest
            )));
        }
        if route.prefix > 32 {
            return Err(ConnectionError::InvalidAddress(format!(
                "OpenVPN route prefix must be at most 32, got {}",
                route.prefix
            )));
        }
        if let Some(ref nh) = route.next_hop {
            validate_ip_address(nh)?;
        }
    }

    for (label, val) in [
        ("ping", config.ping),
        ("ping-exit", config.ping_exit),
        ("ping-restart", config.ping_restart),
        ("reneg-sec", config.reneg_seconds),
        ("connect-timeout", config.connect_timeout),
    ] {
        if let Some(v) = val
            && v == 0
        {
            return Err(ConnectionError::InvalidAddress(format!(
                "{label} must be greater than 0 if set"
            )));
        }
    }

    Ok(())
}

/// Validates that TLS cert paths required for certificate authentication are present.
fn validate_openvpn_cert_paths(config: &OpenVpnConfig) -> Result<(), ConnectionError> {
    if config.ca_cert.as_deref().unwrap_or("").is_empty() {
        return Err(ConnectionError::InvalidAddress(
            "CA certificate path is required for TLS authentication".to_string(),
        ));
    }
    if config.client_cert.as_deref().unwrap_or("").is_empty() {
        return Err(ConnectionError::InvalidAddress(
            "Client certificate path is required for TLS authentication".to_string(),
        ));
    }
    if config.client_key.as_deref().unwrap_or("").is_empty() {
        return Err(ConnectionError::InvalidAddress(
            "Client key path is required for TLS authentication".to_string(),
        ));
    }
    Ok(())
}

/// Validates that an optional certificate path, if provided, is non-empty.
fn validate_optional_cert_path(path: &Option<String>, label: &str) -> Result<(), ConnectionError> {
    if let Some(p) = path
        && p.trim().is_empty()
    {
        return Err(ConnectionError::InvalidAddress(format!(
            "{label} path cannot be empty if provided"
        )));
    }
    Ok(())
}

/// Validates an MTU value (576–9000).
fn validate_mtu(mtu: Option<u32>) -> Result<(), ConnectionError> {
    if let Some(mtu) = mtu {
        if mtu < 576 {
            return Err(ConnectionError::InvalidAddress(format!(
                "MTU too small: {mtu} (minimum 576)"
            )));
        }
        if mtu > 9000 {
            return Err(ConnectionError::InvalidAddress(format!(
                "MTU too large: {mtu} (maximum 9000)"
            )));
        }
    }
    Ok(())
}

/// Validates a Bluetooth address against the EUI-48 format (using colons).
///
/// # Errors
/// Returns `ConnectionError::InvalidAddress` if the Bluetooth address is invalid.
pub fn validate_bluetooth_address(bdaddr: &str) -> Result<(), ConnectionError> {
    let parts: Vec<&str> = bdaddr.split(':').collect();

    if parts.len() != 6 {
        return Err(ConnectionError::InvalidAddress(format!(
            "Invalid Bluetooth Address '{}' (must have 6 segments)",
            bdaddr,
        )));
    }

    for part in parts {
        if part.len() != 2 {
            return Err(ConnectionError::InvalidAddress(format!(
                "Invalid segment '{}' in Bluetooth Address '{}' (must be 2 characters)",
                part, bdaddr
            )));
        }

        if !part.chars().all(|c| c.is_ascii_hexdigit()) {
            return Err(ConnectionError::InvalidAddress(format!(
                "Invalid segment '{}' in Bluetooth Address '{}' (must be hex digits)",
                part, bdaddr
            )));
        }
    }

    Ok(())
}

/// Validates a BSSID (MAC address) in `XX:XX:XX:XX:XX:XX` format.
///
/// Both uppercase and lowercase hex digits are accepted.
///
/// # Errors
///
/// Returns [`ConnectionError::InvalidBssid`] if the format is invalid.
pub fn validate_bssid(bssid: &str) -> Result<(), ConnectionError> {
    let parts: Vec<&str> = bssid.split(':').collect();

    if parts.len() != 6 {
        return Err(ConnectionError::InvalidBssid(bssid.to_string()));
    }

    for part in parts {
        if part.len() != 2 || !part.chars().all(|c| c.is_ascii_hexdigit()) {
            return Err(ConnectionError::InvalidBssid(bssid.to_string()));
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::api::models::{EapMethod, EapOptions, Phase2, VpnKind, VpnRoute};

    macro_rules! assert_error_message {
        ($result:expr, $variant:ident, $expected:expr) => {
            match $result {
                Err(ConnectionError::$variant(message)) => assert_eq!(message, $expected),
                other => panic!(
                    "expected {}::{}, got {other:?}",
                    stringify!(ConnectionError),
                    stringify!($variant)
                ),
            }
        };
    }

    const VALID_WIREGUARD_KEY: &str = "YBk6X3pP8KjKz7+HFWzVHNqL3qTZq8hX9VxFQJ4zVmM=";

    fn base_vpn_credentials() -> VpnCredentials {
        VpnCredentials::new(
            VpnKind::WireGuard,
            "WireGuard",
            "vpn.example.com:51820",
            VALID_WIREGUARD_KEY,
            "10.0.0.2/24",
            vec![WireGuardPeer::new(
                VALID_WIREGUARD_KEY,
                "vpn.example.com:51820",
                vec!["0.0.0.0/0".into()],
            )],
        )
    }

    #[test]
    fn test_validate_ssid_valid() {
        assert!(validate_ssid("MyNetwork").is_ok());
        assert!(validate_ssid("Test-Network_123").is_ok());
        assert!(validate_ssid("A").is_ok());
        assert!(validate_ssid("12345678901234567890123456789012").is_ok()); // 32 bytes
    }

    #[test]
    fn test_validate_ssid_empty() {
        assert_error_message!(validate_ssid(""), InvalidAddress, "SSID cannot be empty");
        assert_error_message!(
            validate_ssid("   "),
            InvalidAddress,
            "SSID cannot be only whitespace"
        );
    }

    #[test]
    fn test_validate_ssid_too_long() {
        let long_ssid = "123456789012345678901234567890123"; // 33 bytes
        assert_error_message!(
            validate_ssid(long_ssid),
            InvalidAddress,
            "SSID too long: 33 bytes (max 32 bytes)"
        );
    }

    #[test]
    fn test_validate_ssid_uses_utf8_byte_boundary() {
        let max_multibyte_ssid = "é".repeat(16);
        assert_eq!(max_multibyte_ssid.len(), 32);
        assert!(validate_ssid(&max_multibyte_ssid).is_ok());

        let too_long_multibyte_ssid = "é".repeat(17);
        assert_eq!(too_long_multibyte_ssid.len(), 34);
        assert_error_message!(
            validate_ssid(&too_long_multibyte_ssid),
            InvalidAddress,
            "SSID too long: 34 bytes (max 32 bytes)"
        );
    }

    #[test]
    fn test_validate_connection_name_valid() {
        assert!(validate_connection_name("MyVPN").is_ok());
        assert!(validate_connection_name("Test-VPN_123").is_ok());
        assert!(validate_connection_name("A").is_ok());
        // Connection names can be longer than SSIDs
        assert!(validate_connection_name(&"a".repeat(255)).is_ok());
    }

    #[test]
    fn test_validate_connection_name_too_long() {
        let long_name = "a".repeat(256);
        assert_error_message!(
            validate_connection_name(&long_name),
            InvalidAddress,
            "Connection name too long: 256 bytes (max 255 bytes)"
        );
    }

    #[test]
    fn test_validate_connection_name_uses_utf8_byte_boundary() {
        let max_multibyte_name = format!("a{}", "é".repeat(127));
        assert_eq!(max_multibyte_name.len(), 255);
        assert!(validate_connection_name(&max_multibyte_name).is_ok());

        let too_long_multibyte_name = "é".repeat(128);
        assert_eq!(too_long_multibyte_name.len(), 256);
        assert_error_message!(
            validate_connection_name(&too_long_multibyte_name),
            InvalidAddress,
            "Connection name too long: 256 bytes (max 255 bytes)"
        );
    }

    #[test]
    fn test_validate_wifi_security_open() {
        assert!(validate_wifi_security(&WifiSecurity::Open).is_ok());
    }

    #[test]
    fn test_validate_wifi_security_psk_valid() {
        let psk = WifiSecurity::WpaPsk {
            psk: "password123".to_string(),
        };
        assert!(validate_wifi_security(&psk).is_ok());
    }

    #[test]
    fn test_validate_wifi_security_psk_empty() {
        let psk = WifiSecurity::WpaPsk {
            psk: "".to_string(),
        };
        // Empty PSK is allowed (for saved credentials)
        assert!(validate_wifi_security(&psk).is_ok());
    }

    #[test]
    fn test_validate_wifi_security_psk_too_short() {
        let psk = WifiSecurity::WpaPsk {
            psk: "short".to_string(),
        };
        assert_error_message!(
            validate_wifi_security(&psk),
            InvalidAddress,
            "WPA-PSK password too short: 5 characters (minimum 8 characters)"
        );
    }

    #[test]
    fn test_validate_wifi_security_psk_too_long() {
        let psk = WifiSecurity::WpaPsk {
            psk: "a".repeat(64),
        };
        assert_error_message!(
            validate_wifi_security(&psk),
            InvalidAddress,
            "WPA-PSK password too long: 64 characters (maximum 63 characters)"
        );
    }

    #[test]
    fn test_validate_wifi_security_eap_valid() {
        let eap = WifiSecurity::WpaEap {
            opts: EapOptions {
                identity: "user@example.com".to_string(),
                password: "password".to_string(),
                anonymous_identity: None,
                domain_suffix_match: Some("example.com".to_string()),
                ca_cert_path: Some("file:///etc/ssl/cert.pem".to_string()),
                ca_cert_blob: None,
                system_ca_certs: false,
                method: EapMethod::Peap,
                phase2: Phase2::Mschapv2,
                private_key_path: None,
                private_key_blob: None,
                private_key_password: None,
                client_cert_path: None,
                client_cert_blob: None,
            },
        };
        assert!(validate_wifi_security(&eap).is_ok());
    }

    #[test]
    fn test_validate_wifi_security_eap_empty_identity() {
        let eap = WifiSecurity::WpaEap {
            opts: EapOptions {
                identity: "".to_string(),
                password: "password".to_string(),
                anonymous_identity: None,
                domain_suffix_match: None,
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
            },
        };
        assert_error_message!(
            validate_wifi_security(&eap),
            InvalidAddress,
            "EAP identity cannot be empty"
        );
    }

    #[test]
    fn test_validate_wifi_security_eap_invalid_ca_cert() {
        let eap = WifiSecurity::WpaEap {
            opts: EapOptions {
                identity: "user@example.com".to_string(),
                password: "password".to_string(),
                anonymous_identity: None,
                domain_suffix_match: None,
                ca_cert_path: Some("/etc/ssl/cert.pem".to_string()), // Missing file://
                ca_cert_blob: None,
                system_ca_certs: false,
                method: EapMethod::Peap,
                phase2: Phase2::Mschapv2,
                private_key_path: None,
                private_key_blob: None,
                private_key_password: None,
                client_cert_path: None,
                client_cert_blob: None,
            },
        };
        assert_error_message!(
            validate_wifi_security(&eap),
            InvalidAddress,
            "EAP CA certificate path must start with 'file://'"
        );
    }

    #[test]
    fn test_validate_wifi_security_eap_192bit_not_tls() {
        let eap = WifiSecurity::Wpa3Eap192bit {
            opts: EapOptions {
                identity: "".to_string(),
                password: "password".to_string(),
                anonymous_identity: None,
                domain_suffix_match: None,
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
            },
        };
        assert_error_message!(
            validate_wifi_security(&eap),
            InvalidAddress,
            "WPA3-EAP 192bit requires authentication method TLS"
        );
    }

    #[test]
    fn test_validate_wifi_security_eap_192bit_valid_path() {
        let eap = WifiSecurity::Wpa3Eap192bit {
            opts: EapOptions {
                identity: "user@example.com".to_string(),
                password: String::new(),
                anonymous_identity: None,
                domain_suffix_match: Some("example.com".to_string()),
                ca_cert_path: Some("file:///etc/ssl/certs/ca.pem".to_string()),
                ca_cert_blob: None,
                system_ca_certs: false,
                method: EapMethod::Tls,
                phase2: Phase2::Mschapv2,
                private_key_path: Some("file:///etc/ssl/private/client.pem".to_string()),
                private_key_blob: None,
                private_key_password: None,
                client_cert_path: Some("file:///etc/ssl/certs/client.pem".to_string()),
                client_cert_blob: None,
            },
        };
        assert!(validate_wifi_security(&eap).is_ok());
    }

    #[test]
    fn test_validate_wifi_security_eap_192bit_valid_blob() {
        let eap = WifiSecurity::Wpa3Eap192bit {
            opts: EapOptions {
                identity: "user@example.com".to_string(),
                password: String::new(),
                anonymous_identity: None,
                domain_suffix_match: Some("example.com".to_string()),
                ca_cert_path: None,
                ca_cert_blob: Some(b"ca_cert_blob".to_vec()),
                system_ca_certs: false,
                method: EapMethod::Tls,
                phase2: Phase2::Mschapv2,
                private_key_path: None,
                private_key_blob: Some(b"private_key_blob".to_vec()),
                private_key_password: None,
                client_cert_path: None,
                client_cert_blob: Some(b"client_cert_blob".to_vec()),
            },
        };
        assert!(validate_wifi_security(&eap).is_ok());
    }

    #[test]
    fn test_validate_wifi_security_eap_192bit_path_blob() {
        let eap = WifiSecurity::Wpa3Eap192bit {
            opts: EapOptions {
                identity: "user@example.com".to_string(),
                password: String::new(),
                anonymous_identity: None,
                domain_suffix_match: Some("example.com".to_string()),
                ca_cert_path: Some("file:///etc/ssl/certs/ca.pem".to_string()),
                ca_cert_blob: Some(b"ca_cert_blob".to_vec()),
                system_ca_certs: false,
                method: EapMethod::Tls,
                phase2: Phase2::Mschapv2,
                private_key_path: Some("file:///etc/ssl/private/client.pem".to_string()),
                private_key_blob: Some(b"private_key_blob".to_vec()),
                private_key_password: None,
                client_cert_path: Some("file:///etc/ssl/certs/client.pem".to_string()),
                client_cert_blob: Some(b"client_cert_blob".to_vec()),
            },
        };
        assert_error_message!(
            validate_wifi_security(&eap),
            InvalidAddress,
            "EAP private key path and blob cannot be provided at the same time"
        );
    }

    #[test]
    fn test_validate_wifi_security_eap_tls_invalid_private_key() {
        let eap = WifiSecurity::WpaEap {
            opts: EapOptions {
                identity: "user@example.com".to_string(),
                password: String::new(),
                anonymous_identity: None,
                domain_suffix_match: Some("example.com".to_string()),
                ca_cert_path: Some("file:///etc/ssl/certs/ca.pem".to_string()),
                ca_cert_blob: None,
                system_ca_certs: false,
                method: EapMethod::Tls,
                phase2: Phase2::Mschapv2,
                private_key_path: Some("/etc/ssl/private/client.pem".to_string()),
                private_key_blob: None,
                private_key_password: None,
                client_cert_path: Some("file:///etc/ssl/certs/client.pem".to_string()),
                client_cert_blob: None,
            },
        };
        assert_error_message!(
            validate_wifi_security(&eap),
            InvalidAddress,
            "EAP private key path must start with 'file://'"
        );
    }

    #[test]
    fn test_validate_wifi_security_eap_tls_invalid_client_cert() {
        let eap = WifiSecurity::WpaEap {
            opts: EapOptions {
                identity: "user@example.com".to_string(),
                password: String::new(),
                anonymous_identity: None,
                domain_suffix_match: Some("example.com".to_string()),
                ca_cert_path: Some("file:///etc/ssl/certs/ca.pem".to_string()),
                ca_cert_blob: None,
                system_ca_certs: false,
                method: EapMethod::Tls,
                phase2: Phase2::Mschapv2,
                private_key_path: Some("file:///etc/ssl/private/client.pem".to_string()),
                private_key_blob: None,
                private_key_password: None,
                client_cert_path: Some("/etc/ssl/certs/client.pem".to_string()),
                client_cert_blob: None,
            },
        };
        assert_error_message!(
            validate_wifi_security(&eap),
            InvalidAddress,
            "EAP client certificate path must start with 'file://'"
        );
    }

    #[test]
    fn test_validate_wifi_security_eap_tls_missing_private_key() {
        let eap = WifiSecurity::WpaEap {
            opts: EapOptions {
                identity: "user@example.com".to_string(),
                password: String::new(),
                anonymous_identity: None,
                domain_suffix_match: Some("example.com".to_string()),
                ca_cert_path: Some("file:///etc/ssl/certs/ca.pem".to_string()),
                ca_cert_blob: None,
                system_ca_certs: false,
                method: EapMethod::Tls,
                phase2: Phase2::Mschapv2,
                private_key_path: None,
                private_key_blob: None,
                private_key_password: None,
                client_cert_path: Some("file:///etc/ssl/certs/client.pem".to_string()),
                client_cert_blob: None,
            },
        };
        assert_error_message!(
            validate_wifi_security(&eap),
            InvalidAddress,
            "EAP private key must be provided"
        );
    }

    #[test]
    fn test_validate_wifi_security_eap_tls_missing_client_cert() {
        let eap = WifiSecurity::WpaEap {
            opts: EapOptions {
                identity: "user@example.com".to_string(),
                password: String::new(),
                anonymous_identity: None,
                domain_suffix_match: Some("example.com".to_string()),
                ca_cert_path: Some("file:///etc/ssl/certs/ca.pem".to_string()),
                ca_cert_blob: None,
                system_ca_certs: false,
                method: EapMethod::Tls,
                phase2: Phase2::Mschapv2,
                private_key_path: Some("file:///etc/ssl/private/client.pem".to_string()),
                private_key_blob: None,
                private_key_password: None,
                client_cert_path: None,
                client_cert_blob: None,
            },
        };
        assert_error_message!(
            validate_wifi_security(&eap),
            InvalidAddress,
            "EAP client certificate must be provided"
        );
    }

    #[test]
    fn eap_password_and_optional_identity_fields_have_exact_errors() {
        let base = EapOptions::new("user@example.com", "password").with_system_ca_certs(true);

        let mut empty_password = base.clone();
        empty_password.password.clear();
        assert_error_message!(
            validate_wifi_security(&WifiSecurity::WpaEap {
                opts: empty_password
            }),
            InvalidAddress,
            "EAP password cannot be empty"
        );

        let mut empty_anonymous = base.clone();
        empty_anonymous.anonymous_identity = Some("   ".into());
        assert_error_message!(
            validate_wifi_security(&WifiSecurity::WpaEap {
                opts: empty_anonymous
            }),
            InvalidAddress,
            "EAP anonymous identity cannot be empty if provided"
        );

        let mut empty_domain = base;
        empty_domain.domain_suffix_match = Some("\t".into());
        assert_error_message!(
            validate_wifi_security(&WifiSecurity::WpaEap { opts: empty_domain }),
            InvalidAddress,
            "EAP domain suffix match cannot be empty if provided"
        );
    }

    #[test]
    fn test_validate_cidr_ipv4_valid() {
        assert!(validate_cidr("10.0.0.0/24").is_ok());
        assert!(validate_cidr("192.168.1.0/16").is_ok());
        assert!(validate_cidr("0.0.0.0/0").is_ok());
    }

    #[test]
    fn test_validate_cidr_ipv6_valid() {
        assert!(validate_cidr("2001:db8::/32").is_ok());
        assert!(validate_cidr("::/0").is_ok());
    }

    #[test]
    fn test_validate_cidr_invalid() {
        for (cidr, expected) in [
            (
                "10.0.0.0",
                "Invalid CIDR notation '10.0.0.0' (must be 'address/prefix')",
            ),
            ("10.0.0.0/33", "IPv4 prefix length 33 is too large (max 32)"),
            ("256.0.0.0/24", "IPv4 octet 256 is too large (max 255)"),
            (
                "10.0.0/24",
                "Invalid IPv4 address '10.0.0' (must have 4 octets)",
            ),
        ] {
            assert_error_message!(validate_cidr(cidr), InvalidAddress, expected);
        }
    }

    #[test]
    fn malformed_ipv6_cidr_is_rejected() {
        assert_error_message!(
            validate_cidr("2001:::1/64"),
            InvalidAddress,
            "Invalid IPv6 address '2001:::1'"
        );
        assert_error_message!(
            validate_cidr("2001:db8::/129"),
            InvalidAddress,
            "IPv6 prefix length 129 is too large (max 128)"
        );
    }

    #[test]
    fn test_validate_ip_address_ipv4_valid() {
        assert!(validate_ip_address("192.168.1.1").is_ok());
        assert!(validate_ip_address("8.8.8.8").is_ok());
        assert!(validate_ip_address("0.0.0.0").is_ok());
    }

    #[test]
    fn test_validate_ip_address_ipv4_invalid() {
        for (address, expected) in [
            (
                "256.1.1.1",
                "IPv4 octet 256 is too large (max 255) in address '256.1.1.1'",
            ),
            (
                "192.168.1",
                "Invalid IPv4 address '192.168.1' (must have 4 octets)",
            ),
            (
                "192.168.1.1.1",
                "Invalid IPv4 address '192.168.1.1.1' (must have 4 octets)",
            ),
        ] {
            assert_error_message!(validate_ip_address(address), InvalidAddress, expected);
        }
    }

    #[test]
    fn malformed_ipv6_addresses_are_rejected() {
        for address in [":::1", "2001:db8:::1", "gggg::1"] {
            assert_error_message!(
                validate_ip_address(address),
                InvalidAddress,
                format!("Invalid IPv6 address '{address}'")
            );
        }
    }

    #[test]
    fn test_validate_wireguard_key_valid() {
        // Valid 32-byte base64 key
        let key = "YBk6X3pP8KjKz7+HFWzVHNqL3qTZq8hX9VxFQJ4zVmM=";
        assert!(validate_wireguard_key(key, "Test key").is_ok());
    }

    #[test]
    fn test_validate_wireguard_key_invalid_length() {
        let key = "tooshort";
        assert_error_message!(
            validate_wireguard_key(key, "Test key"),
            InvalidPrivateKey,
            "Test key must be 44 characters (base64 encoded), got 8"
        );
    }

    #[test]
    fn test_validate_wireguard_key_invalid_base64() {
        let key = "!".repeat(WIREGUARD_KEY_BASE64_LEN);
        match validate_wireguard_key(&key, "Test key") {
            Err(ConnectionError::InvalidPrivateKey(message)) => {
                assert!(message.starts_with("Test key is not valid base64:"));
            }
            other => panic!("expected InvalidPrivateKey base64 error, got {other:?}"),
        }
    }

    #[test]
    fn test_validate_wireguard_key_invalid_decoded_length() {
        let key = "A".repeat(WIREGUARD_KEY_BASE64_LEN);
        assert_error_message!(
            validate_wireguard_key(&key, "Private key"),
            InvalidPrivateKey,
            "Private key must decode to 32 bytes, got 33"
        );
    }

    #[test]
    fn public_key_errors_use_public_key_variant() {
        assert_error_message!(
            validate_wireguard_key("short", "Peer public key"),
            InvalidPublicKey,
            "Peer public key must be 44 characters (base64 encoded), got 5"
        );
    }

    #[test]
    fn vpn_credentials_valid_happy_path() {
        assert!(validate_vpn_credentials(&base_vpn_credentials()).is_ok());
    }

    #[test]
    fn vpn_credentials_validate_gateway_before_key_material() {
        let mut credentials = base_vpn_credentials();
        credentials.gateway.clear();
        assert_error_message!(
            validate_vpn_credentials(&credentials),
            InvalidGateway,
            "VPN gateway cannot be empty"
        );

        credentials.gateway = "vpn.example.com".into();
        assert_error_message!(
            validate_vpn_credentials(&credentials),
            InvalidGateway,
            "VPN gateway must be in 'host:port' format, got 'vpn.example.com'"
        );

        credentials.gateway = "vpn.example.com:not-a-port".into();
        assert_error_message!(
            validate_vpn_credentials(&credentials),
            InvalidGateway,
            "Invalid port number in gateway 'vpn.example.com:not-a-port'"
        );

        credentials.gateway = "vpn.example.com:0".into();
        assert_error_message!(
            validate_vpn_credentials(&credentials),
            InvalidGateway,
            "Port number in gateway 'vpn.example.com:0' cannot be 0"
        );

        credentials.gateway = ":51820".into();
        assert_error_message!(
            validate_vpn_credentials(&credentials),
            InvalidGateway,
            "VPN gateway host cannot be empty"
        );

        credentials.gateway = "2001:db8::1:51820".into();
        assert_error_message!(
            validate_vpn_credentials(&credentials),
            InvalidGateway,
            "VPN IPv6 gateway must use '[address]:port' format, got '2001:db8::1:51820'"
        );

        credentials.gateway = "[2001:db8::1]:51820".into();
        assert!(validate_vpn_credentials(&credentials).is_ok());
    }

    #[test]
    fn vpn_credentials_validate_private_key_address_and_peer_presence() {
        let mut credentials = base_vpn_credentials();
        credentials.private_key = "short".into();
        assert_error_message!(
            validate_vpn_credentials(&credentials),
            InvalidPrivateKey,
            "Private key must be 44 characters (base64 encoded), got 5"
        );

        credentials.private_key = VALID_WIREGUARD_KEY.into();
        credentials.address = "10.0.0.2".into();
        assert_error_message!(
            validate_vpn_credentials(&credentials),
            InvalidAddress,
            "Invalid CIDR notation '10.0.0.2' (must be 'address/prefix')"
        );

        credentials.address = "10.0.0.2/24".into();
        credentials.peers.clear();
        assert_error_message!(
            validate_vpn_credentials(&credentials),
            InvalidPeers,
            "VPN must have at least one peer configured"
        );
    }

    #[test]
    fn vpn_credentials_prefix_peer_errors_with_index() {
        let mut credentials = base_vpn_credentials();
        credentials.peers[0].public_key = "short".into();
        assert_error_message!(
            validate_vpn_credentials(&credentials),
            InvalidPublicKey,
            "Peer 0: Peer public key must be 44 characters (base64 encoded), got 5"
        );

        credentials.peers[0].public_key = VALID_WIREGUARD_KEY.into();
        credentials.peers[0].gateway.clear();
        assert_error_message!(
            validate_vpn_credentials(&credentials),
            InvalidGateway,
            "Peer 0: Peer gateway cannot be empty"
        );

        credentials.peers[0].gateway = "vpn.example.com:0".into();
        assert_error_message!(
            validate_vpn_credentials(&credentials),
            InvalidGateway,
            "Peer 0: Port number in gateway 'vpn.example.com:0' cannot be 0"
        );

        credentials.peers[0].gateway = "2001:db8::1:51820".into();
        assert_error_message!(
            validate_vpn_credentials(&credentials),
            InvalidGateway,
            "Peer 0: Peer IPv6 gateway must use '[address]:port' format, got '2001:db8::1:51820'"
        );

        credentials.peers[0].gateway = "[2001:db8::1]:51820".into();
        credentials.peers[0].allowed_ips.clear();
        assert_error_message!(
            validate_vpn_credentials(&credentials),
            InvalidPeers,
            "Peer 0: Peer must have at least one allowed IP range"
        );

        credentials.peers[0].allowed_ips = vec!["0.0.0.0/0".into()];
        credentials.peers[0].persistent_keepalive = Some(0);
        assert_error_message!(
            validate_vpn_credentials(&credentials),
            InvalidPeers,
            "Peer 0: Persistent keepalive must be greater than 0 if specified"
        );

        credentials.peers[0].persistent_keepalive = Some(65_536);
        assert_error_message!(
            validate_vpn_credentials(&credentials),
            InvalidPeers,
            "Peer 0: Persistent keepalive too large: 65536 (max 65535)"
        );
    }

    #[test]
    fn vpn_credentials_validate_optional_dns_and_mtu() {
        let mut credentials = base_vpn_credentials();
        credentials.dns = Some(Vec::new());
        assert_error_message!(
            validate_vpn_credentials(&credentials),
            InvalidAddress,
            "DNS server list cannot be empty if provided"
        );

        credentials.dns = Some(vec!["not-an-ip".into()]);
        assert_error_message!(
            validate_vpn_credentials(&credentials),
            InvalidAddress,
            "Invalid IPv4 address 'not-an-ip' (must have 4 octets)"
        );

        credentials.dns = Some(vec!["1.1.1.1".into(), "2001:4860:4860::8888".into()]);
        credentials.mtu = Some(575);
        assert_error_message!(
            validate_vpn_credentials(&credentials),
            InvalidAddress,
            "MTU too small: 575 (minimum 576)"
        );

        credentials.mtu = Some(9001);
        assert_error_message!(
            validate_vpn_credentials(&credentials),
            InvalidAddress,
            "MTU too large: 9001 (maximum 9000)"
        );
    }

    #[test]
    fn test_validate_bluetooth_address_valid() {
        assert!(validate_bluetooth_address("00:1A:7D:DA:71:13").is_ok());
        assert!(validate_bluetooth_address("00:1a:7d:da:71:13").is_ok());
        assert!(validate_bluetooth_address("aA:bB:cC:dD:eE:fF").is_ok());
    }

    #[test]
    fn test_validate_bluetooth_address_invalid_format() {
        for address in ["00-1A-7D-DA-71-13", "001A7DDA7113"] {
            assert_error_message!(
                validate_bluetooth_address(address),
                InvalidAddress,
                format!("Invalid Bluetooth Address '{address}' (must have 6 segments)")
            );
        }
        assert_error_message!(
            validate_bluetooth_address("00:1A:7D:DA:711:3"),
            InvalidAddress,
            "Invalid segment '711' in Bluetooth Address '00:1A:7D:DA:711:3' (must be 2 characters)"
        );
    }

    #[test]
    fn test_validate_bluetooth_address_invalid_char() {
        for segment in ["GG", "!!"] {
            let address = format!("00:1A:7D:DA:71:{segment}");
            assert_error_message!(
                validate_bluetooth_address(&address),
                InvalidAddress,
                format!(
                    "Invalid segment '{segment}' in Bluetooth Address '{address}' (must be hex digits)"
                )
            );
        }
    }

    #[test]
    fn test_validate_bluetooth_address_invalid_length() {
        for address in ["00:1A:7D", "00:1A:7D:DA:71:13:FF", ""] {
            assert_error_message!(
                validate_bluetooth_address(address),
                InvalidAddress,
                format!("Invalid Bluetooth Address '{address}' (must have 6 segments)")
            );
        }
    }

    fn base_openvpn_config() -> OpenVpnConfig {
        OpenVpnConfig::new("MyVPN", "vpn.example.com", 1194, false)
    }

    #[test]
    fn test_validate_openvpn_valid_minimal() {
        assert!(validate_openvpn_config(&base_openvpn_config()).is_ok());
    }

    #[test]
    fn test_validate_openvpn_empty_name() {
        let config = OpenVpnConfig::new("", "vpn.example.com", 1194, false);
        assert_error_message!(
            validate_openvpn_config(&config),
            InvalidAddress,
            "Connection name cannot be empty"
        );
    }

    #[test]
    fn test_validate_openvpn_whitespace_name() {
        let config = OpenVpnConfig::new("   ", "vpn.example.com", 1194, false);
        assert_error_message!(
            validate_openvpn_config(&config),
            InvalidAddress,
            "Connection name cannot be only whitespace"
        );
    }

    #[test]
    fn test_validate_openvpn_empty_remote() {
        let config = OpenVpnConfig::new("MyVPN", "", 1194, false);
        assert_error_message!(
            validate_openvpn_config(&config),
            InvalidGateway,
            "OpenVPN remote server cannot be empty"
        );
    }

    #[test]
    fn test_validate_openvpn_whitespace_remote() {
        let config = OpenVpnConfig::new("MyVPN", "   ", 1194, false);
        assert_error_message!(
            validate_openvpn_config(&config),
            InvalidGateway,
            "OpenVPN remote server cannot be empty"
        );
    }

    #[test]
    fn test_validate_openvpn_password_auth_missing_username() {
        let config = base_openvpn_config().with_auth_type(OpenVpnAuthType::Password);
        assert_error_message!(
            validate_openvpn_config(&config),
            InvalidAddress,
            "Username is required for password authentication"
        );
    }

    #[test]
    fn test_validate_openvpn_password_auth_rejects_whitespace_username() {
        let config = base_openvpn_config()
            .with_auth_type(OpenVpnAuthType::Password)
            .with_username("   ");
        assert_error_message!(
            validate_openvpn_config(&config),
            InvalidAddress,
            "Username is required for password authentication"
        );
    }

    #[test]
    fn test_validate_openvpn_password_auth_with_username() {
        let config = base_openvpn_config()
            .with_auth_type(OpenVpnAuthType::Password)
            .with_username("user");
        assert!(validate_openvpn_config(&config).is_ok());
    }

    #[test]
    fn test_validate_openvpn_tls_auth_missing_certs() {
        let config = base_openvpn_config().with_auth_type(OpenVpnAuthType::Tls);
        assert_error_message!(
            validate_openvpn_config(&config),
            InvalidAddress,
            "CA certificate path is required for TLS authentication"
        );
    }

    #[test]
    fn test_validate_openvpn_tls_auth_partial_certs() {
        let config = base_openvpn_config()
            .with_auth_type(OpenVpnAuthType::Tls)
            .with_ca_cert("/path/to/ca.crt");
        assert_error_message!(
            validate_openvpn_config(&config),
            InvalidAddress,
            "Client certificate path is required for TLS authentication"
        );
    }

    #[test]
    fn test_validate_openvpn_tls_auth_with_all_certs() {
        let config = base_openvpn_config()
            .with_auth_type(OpenVpnAuthType::Tls)
            .with_ca_cert("/path/to/ca.crt")
            .with_client_cert("/path/to/client.crt")
            .with_client_key("/path/to/client.key");
        assert!(validate_openvpn_config(&config).is_ok());
    }

    #[test]
    fn test_validate_openvpn_password_tls_missing_username() {
        let config = base_openvpn_config()
            .with_auth_type(OpenVpnAuthType::PasswordTls)
            .with_ca_cert("/path/to/ca.crt")
            .with_client_cert("/path/to/client.crt")
            .with_client_key("/path/to/client.key");
        assert_error_message!(
            validate_openvpn_config(&config),
            InvalidAddress,
            "Username is required for password+TLS authentication"
        );
    }

    #[test]
    fn test_validate_openvpn_password_tls_missing_certs() {
        let config = base_openvpn_config()
            .with_auth_type(OpenVpnAuthType::PasswordTls)
            .with_username("user");
        assert_error_message!(
            validate_openvpn_config(&config),
            InvalidAddress,
            "CA certificate path is required for TLS authentication"
        );
    }

    #[test]
    fn test_validate_openvpn_password_tls_complete() {
        let config = base_openvpn_config()
            .with_auth_type(OpenVpnAuthType::PasswordTls)
            .with_username("user")
            .with_ca_cert("/path/to/ca.crt")
            .with_client_cert("/path/to/client.crt")
            .with_client_key("/path/to/client.key");
        assert!(validate_openvpn_config(&config).is_ok());
    }

    #[test]
    fn test_validate_openvpn_static_key_minimal() {
        let config = base_openvpn_config().with_auth_type(OpenVpnAuthType::StaticKey);
        assert!(validate_openvpn_config(&config).is_ok());
    }

    #[test]
    fn test_validate_openvpn_empty_cert_path_provided() {
        let config = base_openvpn_config().with_ca_cert("");
        assert_error_message!(
            validate_openvpn_config(&config),
            InvalidAddress,
            "CA certificate path cannot be empty if provided"
        );
    }

    #[test]
    fn test_validate_openvpn_whitespace_cert_path() {
        let config = base_openvpn_config().with_client_cert("   ");
        assert_error_message!(
            validate_openvpn_config(&config),
            InvalidAddress,
            "Client certificate path cannot be empty if provided"
        );
    }

    #[test]
    fn test_validate_openvpn_valid_dns() {
        let config = base_openvpn_config().with_dns(vec!["1.1.1.1".into(), "8.8.8.8".into()]);
        assert!(validate_openvpn_config(&config).is_ok());
    }

    #[test]
    fn test_validate_openvpn_empty_dns_list() {
        let config = base_openvpn_config().with_dns(vec![]);
        assert_error_message!(
            validate_openvpn_config(&config),
            InvalidAddress,
            "DNS server list cannot be empty if provided"
        );
    }

    #[test]
    fn test_validate_openvpn_invalid_dns() {
        let config = base_openvpn_config().with_dns(vec!["not-an-ip".into()]);
        assert_error_message!(
            validate_openvpn_config(&config),
            InvalidAddress,
            "Invalid IPv4 address 'not-an-ip' (must have 4 octets)"
        );
    }

    #[test]
    fn test_validate_openvpn_mtu_too_small() {
        let config = base_openvpn_config().with_mtu(100);
        assert_error_message!(
            validate_openvpn_config(&config),
            InvalidAddress,
            "MTU too small: 100 (minimum 576)"
        );
    }

    #[test]
    fn test_validate_openvpn_mtu_too_large() {
        let config = base_openvpn_config().with_mtu(10000);
        assert_error_message!(
            validate_openvpn_config(&config),
            InvalidAddress,
            "MTU too large: 10000 (maximum 9000)"
        );
    }

    #[test]
    fn test_validate_openvpn_mtu_valid() {
        let config = base_openvpn_config().with_mtu(1500);
        assert!(validate_openvpn_config(&config).is_ok());
    }

    #[test]
    fn test_validate_openvpn_mtu_boundary_min() {
        let config = base_openvpn_config().with_mtu(576);
        assert!(validate_openvpn_config(&config).is_ok());
    }

    #[test]
    fn test_validate_openvpn_mtu_boundary_max() {
        let config = base_openvpn_config().with_mtu(9000);
        assert!(validate_openvpn_config(&config).is_ok());
    }

    #[test]
    fn test_validate_openvpn_empty_proxy_server() {
        let config = base_openvpn_config().with_proxy(OpenVpnProxy::Http {
            server: "".into(),
            port: 8080,
            username: None,
            password: None,
            retry: false,
        });
        assert_error_message!(
            validate_openvpn_config(&config),
            InvalidAddress,
            "Proxy server address cannot be empty"
        );
    }

    #[test]
    fn test_validate_openvpn_valid_http_proxy() {
        let config = base_openvpn_config().with_proxy(OpenVpnProxy::Http {
            server: "proxy.example.com".into(),
            port: 8080,
            username: None,
            password: None,
            retry: false,
        });
        assert!(validate_openvpn_config(&config).is_ok());
    }

    #[test]
    fn test_validate_openvpn_empty_socks_proxy_server() {
        let config = base_openvpn_config().with_proxy(OpenVpnProxy::Socks {
            server: "  ".into(),
            port: 1080,
            retry: false,
        });
        assert_error_message!(
            validate_openvpn_config(&config),
            InvalidAddress,
            "Proxy server address cannot be empty"
        );
    }

    #[test]
    fn openvpn_routes_validate_destination_prefix_and_next_hop() {
        let empty = base_openvpn_config().with_routes(vec![VpnRoute::new("", 24)]);
        assert_error_message!(
            validate_openvpn_config(&empty),
            InvalidAddress,
            "OpenVPN route destination cannot be empty"
        );

        let malformed = base_openvpn_config().with_routes(vec![VpnRoute::new("not-an-ip", 24)]);
        assert_error_message!(
            validate_openvpn_config(&malformed),
            InvalidAddress,
            "Invalid OpenVPN route destination 'not-an-ip'"
        );

        let prefix = base_openvpn_config().with_routes(vec![VpnRoute::new("10.0.0.0", 33)]);
        assert_error_message!(
            validate_openvpn_config(&prefix),
            InvalidAddress,
            "OpenVPN route prefix must be at most 32, got 33"
        );

        let next_hop = base_openvpn_config()
            .with_routes(vec![VpnRoute::new("10.0.0.0", 24).next_hop("bad-gateway")]);
        assert_error_message!(
            validate_openvpn_config(&next_hop),
            InvalidAddress,
            "Invalid IPv4 address 'bad-gateway' (must have 4 octets)"
        );

        let valid = base_openvpn_config().with_routes(vec![
            VpnRoute::new("10.0.0.0", 24)
                .next_hop("192.168.1.1")
                .metric(10),
        ]);
        assert!(validate_openvpn_config(&valid).is_ok());
    }

    #[test]
    fn openvpn_timers_reject_zero_with_directive_specific_error() {
        let cases = [
            ("ping", base_openvpn_config().with_ping(0)),
            ("ping-exit", base_openvpn_config().with_ping_exit(0)),
            ("ping-restart", base_openvpn_config().with_ping_restart(0)),
            ("reneg-sec", base_openvpn_config().with_reneg_seconds(0)),
            (
                "connect-timeout",
                base_openvpn_config().with_connect_timeout(0),
            ),
        ];

        for (label, config) in cases {
            assert_error_message!(
                validate_openvpn_config(&config),
                InvalidAddress,
                format!("{label} must be greater than 0 if set")
            );
        }

        let valid = base_openvpn_config()
            .with_ping(1)
            .with_ping_exit(1)
            .with_ping_restart(1)
            .with_reneg_seconds(1)
            .with_connect_timeout(1);
        assert!(validate_openvpn_config(&valid).is_ok());
    }

    #[test]
    fn test_validate_openvpn_valid_socks_proxy() {
        let config = base_openvpn_config().with_proxy(OpenVpnProxy::Socks {
            server: "socks.example.com".into(),
            port: 1080,
            retry: false,
        });
        assert!(validate_openvpn_config(&config).is_ok());
    }

    #[test]
    fn test_validate_openvpn_no_auth_type_is_valid() {
        let config = base_openvpn_config();
        assert!(config.auth_type.is_none());
        assert!(validate_openvpn_config(&config).is_ok());
    }

    #[test]
    fn test_validate_bssid_valid_uppercase() {
        assert!(validate_bssid("AA:BB:CC:DD:EE:FF").is_ok());
    }

    #[test]
    fn test_validate_bssid_valid_lowercase() {
        assert!(validate_bssid("aa:bb:cc:dd:ee:ff").is_ok());
    }

    #[test]
    fn test_validate_bssid_valid_mixed() {
        assert!(validate_bssid("aA:Bb:cC:Dd:eE:fF").is_ok());
    }

    #[test]
    fn test_validate_bssid_too_short() {
        assert_error_message!(
            validate_bssid("AA:BB:CC:DD:EE"),
            InvalidBssid,
            "AA:BB:CC:DD:EE"
        );
    }

    #[test]
    fn test_validate_bssid_empty() {
        assert_error_message!(validate_bssid(""), InvalidBssid, "");
    }

    #[test]
    fn test_validate_bssid_unicode() {
        assert_error_message!(
            validate_bssid("AA:BB:CC:DD:EE:ÀÀ"),
            InvalidBssid,
            "AA:BB:CC:DD:EE:ÀÀ"
        );
    }

    #[test]
    fn test_validate_bssid_invalid_segment() {
        assert_error_message!(
            validate_bssid("GG:BB:CC:DD:EE:FF"),
            InvalidBssid,
            "GG:BB:CC:DD:EE:FF"
        );
    }
}
