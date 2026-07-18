//! VPN connection settings builders.
//!
//! This module provides functions to build NetworkManager settings dictionaries
//! for VPN connections. Supports:
//!
//! - **WireGuard** — Modern, high-performance VPN protocol
//! - **OpenVPN** — Widely-used open-source VPN protocol (via NM plugin)
//!
//! # Usage
//!
//! Most users should call [`NetworkManager::connect_vpn`][crate::NetworkManager::connect_vpn]
//! instead of using these builders directly. This module is intended for
//! advanced use cases where you need low-level control over the connection settings.
//!
//! # Connection Builder API
//!
//! Consider using the fluent builder API added in 1.3.0:
//!
//! ```rust
//! use nmrs::builders::WireGuardBuilder;
//! use nmrs::WireGuardPeer;
//!
//! let peer = WireGuardPeer::new(
//!     "HIgo9xNzJMWLKAShlKl6/bUT1VI9Q0SDBXGtLXkPFXc=",
//!     "vpn.example.com:51820",
//!     vec!["0.0.0.0/0".into()],
//! ).with_persistent_keepalive(25);
//!
//! let settings = WireGuardBuilder::new("MyVPN")
//!     .private_key("YBk6X3pP8KjKz7+HFWzVHNqL3qTZq8hX9VxFQJ4zVmM=")
//!     .address("10.0.0.2/24")
//!     .add_peer(peer)
//!     .dns(vec!["1.1.1.1".into()])
//!     .build()
//!     .expect("Failed to build WireGuard connection");
//! ```
//!
//! # Legacy Function API
//!
//! The `build_wireguard_connection` function is maintained for backward compatibility:
//!
//! ```rust
//! use nmrs::builders::build_wireguard_connection;
//! use nmrs::{VpnCredentials, VpnKind, WireGuardPeer, ConnectionOptions};
//!
//! let peer = WireGuardPeer::new(
//!     "HIgo9xNzJMWLKAShlKl6/bUT1VI9Q0SDBXGtLXkPFXc=",
//!     "vpn.example.com:51820",
//!     vec!["0.0.0.0/0".into()],
//! ).with_persistent_keepalive(25);
//!
//! let creds = VpnCredentials::new(
//!     VpnKind::WireGuard,
//!     "MyVPN",
//!     "vpn.example.com:51820",
//!     "YBk6X3pP8KjKz7+HFWzVHNqL3qTZq8hX9VxFQJ4zVmM=",
//!     "10.0.0.2/24",
//!     vec![peer],
//! ).with_dns(vec!["1.1.1.1".into()]);
//!
//! let opts = ConnectionOptions::new(false);
//!
//! let settings = build_wireguard_connection(&creds, &opts).unwrap();
//! // Pass settings to NetworkManager's AddAndActivateConnection
//! ```
#![allow(deprecated)]

use std::collections::HashMap;
use zvariant::{Dict, Value, signature};

use super::wireguard_builder::WireGuardBuilder;
use crate::api::models::{
    ConnectionError, ConnectionOptions, OpenVpnAuthType, OpenVpnCompression, OpenVpnConfig,
    OpenVpnProxy, VpnCredentials,
};

/// Builds WireGuard VPN connection settings.
///
/// Returns a complete NetworkManager settings dictionary suitable for
/// `AddAndActivateConnection`.
///
/// # Errors
///
/// - `ConnectionError::InvalidPeers` if no peers are provided
/// - `ConnectionError::InvalidAddress` if the address is missing or malformed
///
/// # Note
///
/// This function is maintained for backward compatibility. For new code,
/// consider using `WireGuardBuilder` for a more ergonomic API.
#[must_use = "the connection settings must be passed to NetworkManager"]
pub fn build_wireguard_connection(
    creds: &VpnCredentials,
    opts: &ConnectionOptions,
) -> Result<HashMap<&'static str, HashMap<&'static str, Value<'static>>>, ConnectionError> {
    let mut builder = WireGuardBuilder::new(&creds.name)
        .private_key(&creds.private_key)
        .address(&creds.address)
        .add_peers(creds.peers.iter().cloned())
        .options(opts);

    if let Some(uuid) = creds.uuid {
        builder = builder.uuid(uuid);
    }

    if let Some(dns) = &creds.dns {
        builder = builder.dns(dns.clone());
    }

    if let Some(mtu) = creds.mtu {
        builder = builder.mtu(mtu);
    }

    builder.build()
}

/// Converts a list of string key-value pairs into a `zvariant::Dict` with
/// D-Bus signature `a{ss}`, which NetworkManager requires for `vpn.data`
/// and `vpn.secrets`.
fn string_pairs_to_dict(
    pairs: Vec<(String, String)>,
) -> Result<Dict<'static, 'static>, ConnectionError> {
    let sig = signature!("s");
    let mut dict = Dict::new(&sig, &sig);
    for (k, v) in pairs {
        dict.append(Value::from(k), Value::from(v)).map_err(|e| {
            ConnectionError::VpnFailed(format!("failed to append VPN setting: {e}"))
        })?;
    }
    Ok(dict)
}

/// Pushes `(key, value.clone())` onto `out` if `value` is `Some`.
///
/// Convenience wrapper around the common pattern of mapping an
/// `Option<String>` field on an [`OpenVpnConfig`] to an entry in the flat
/// `vpn.data` dict.
fn push_opt_str(out: &mut Vec<(String, String)>, key: &str, value: Option<&String>) {
    if let Some(v) = value {
        out.push((key.to_string(), v.clone()));
    }
}

/// Pushes `(key, value.to_string())` onto `out` if `value` is `Some`.
///
/// Used for numeric/boolean OpenVPN options that NetworkManager stores as
/// strings (`tunnel-mtu`, `ping`, `connect-timeout`, …).
fn push_opt_display<T: std::fmt::Display>(
    out: &mut Vec<(String, String)>,
    key: &str,
    value: Option<T>,
) {
    if let Some(v) = value {
        out.push((key.to_string(), v.to_string()));
    }
}

/// Builds OpenVPN connection settings for NetworkManager.
///
/// Returns a settings dictionary suitable for `AddAndActivateConnection`.
/// OpenVPN uses the NM VPN plugin model: `connection.type = "vpn"` with
/// `vpn.service-type = "org.freedesktop.NetworkManager.openvpn"`.
/// All config lives in the flat `vpn.data` dict.
///
/// # Errors
///
/// - `ConnectionError::InvalidGateway` if `remote` is empty
/// - `ConnectionError::InvalidAddress` if a proxy port is zero
#[must_use = "the connection settings must be passed to NetworkManager"]
pub fn build_openvpn_connection(
    config: &OpenVpnConfig,
    opts: &ConnectionOptions,
) -> Result<HashMap<&'static str, HashMap<&'static str, Value<'static>>>, ConnectionError> {
    if config.remote.is_empty() {
        return Err(ConnectionError::InvalidGateway(
            "OpenVPN remote must not be empty".into(),
        ));
    }

    let uuid = config.uuid.unwrap_or_else(uuid::Uuid::new_v4).to_string();

    let mut connection: HashMap<&'static str, Value<'static>> = HashMap::new();
    connection.insert("type", Value::from("vpn"));
    connection.insert("id", Value::from(config.name.clone()));
    connection.insert("uuid", Value::from(uuid));
    connection.insert("autoconnect", Value::from(opts.autoconnect));
    if let Some(p) = opts.autoconnect_priority {
        connection.insert("autoconnect-priority", Value::from(p));
    }
    if let Some(retries) = opts.autoconnect_retries {
        connection.insert("autoconnect-retries", Value::from(retries));
    }

    let mut vpn_data: Vec<(String, String)> = Vec::new();

    let remote = format!("{}:{}", config.remote, config.port);

    vpn_data.push(("remote".into(), remote));

    let connection_type = match config.auth_type {
        Some(OpenVpnAuthType::Password) => "password",
        Some(OpenVpnAuthType::Tls) => "tls",
        Some(OpenVpnAuthType::PasswordTls) => "password-tls",
        Some(OpenVpnAuthType::StaticKey) => "static-key",
        None => "tls",
    };
    vpn_data.push(("connection-type".into(), connection_type.into()));

    if config.tcp {
        vpn_data.push(("proto-tcp".into(), "yes".into()));
    }

    push_opt_str(&mut vpn_data, "username", config.username.as_ref());
    push_opt_str(&mut vpn_data, "auth", config.auth.as_ref());
    push_opt_str(&mut vpn_data, "cipher", config.cipher.as_ref());
    push_opt_display(&mut vpn_data, "tunnel-mtu", config.mtu);

    // certs
    push_opt_str(&mut vpn_data, "ca", config.ca_cert.as_ref());
    push_opt_str(&mut vpn_data, "cert", config.client_cert.as_ref());
    push_opt_str(&mut vpn_data, "key", config.client_key.as_ref());

    if let Some(ref compression) = config.compression {
        #[allow(deprecated)]
        let (key, value) = match compression {
            OpenVpnCompression::No => ("compress", "no"),
            OpenVpnCompression::Lzo => ("comp-lzo", "yes"),
            OpenVpnCompression::Lz4 => ("compress", "lz4"),
            OpenVpnCompression::Lz4V2 => ("compress", "lz4-v2"),
            OpenVpnCompression::Yes => ("compress", "yes"),
        };
        vpn_data.push((key.into(), value.into()));
    }

    // TLS hardening options
    if let Some(ref key) = config.tls_auth_key {
        vpn_data.push(("tls-auth".into(), key.clone()));
        push_opt_display(&mut vpn_data, "ta-dir", config.tls_auth_direction);
    }
    push_opt_str(&mut vpn_data, "tls-crypt", config.tls_crypt.as_ref());
    push_opt_str(&mut vpn_data, "tls-crypt-v2", config.tls_crypt_v2.as_ref());
    push_opt_str(
        &mut vpn_data,
        "tls-version-min",
        config.tls_version_min.as_ref(),
    );
    push_opt_str(
        &mut vpn_data,
        "tls-version-max",
        config.tls_version_max.as_ref(),
    );
    push_opt_str(&mut vpn_data, "tls-cipher", config.tls_cipher.as_ref());
    push_opt_str(
        &mut vpn_data,
        "remote-cert-tls",
        config.remote_cert_tls.as_ref(),
    );
    if let Some((ref name, ref name_type)) = config.verify_x509_name {
        vpn_data.push(("verify-x509-name".into(), name.clone()));
        vpn_data.push(("verify-x509-type".into(), name_type.clone()));
    }
    push_opt_str(&mut vpn_data, "crl-verify", config.crl_verify.as_ref());

    push_opt_display(&mut vpn_data, "ping", config.ping);
    push_opt_display(&mut vpn_data, "ping-exit", config.ping_exit);
    push_opt_display(&mut vpn_data, "ping-restart", config.ping_restart);
    push_opt_display(&mut vpn_data, "reneg-sec", config.reneg_seconds);
    push_opt_display(&mut vpn_data, "connect-timeout", config.connect_timeout);
    push_opt_str(&mut vpn_data, "data-ciphers", config.data_ciphers.as_ref());
    push_opt_str(
        &mut vpn_data,
        "data-ciphers-fallback",
        config.data_ciphers_fallback.as_ref(),
    );
    if config.ncp_disable {
        vpn_data.push(("ncp-disable".into(), "yes".into()));
    }

    if let Some(ref proxy) = config.proxy {
        match proxy {
            OpenVpnProxy::Http {
                server,
                port,
                username,
                password,
                retry,
            } => {
                if *port == 0 {
                    return Err(ConnectionError::InvalidAddress(
                        "proxy port must not be zero".into(),
                    ));
                }
                vpn_data.push(("proxy-type".into(), "http".into()));
                vpn_data.push(("proxy-server".into(), server.clone()));
                vpn_data.push(("proxy-port".into(), port.to_string()));
                vpn_data.push((
                    "proxy-retry".into(),
                    if *retry { "yes" } else { "no" }.into(),
                ));
                if let Some(u) = username {
                    vpn_data.push(("http-proxy-username".into(), u.clone()));
                }
                if let Some(p) = password {
                    vpn_data.push(("http-proxy-password".into(), p.clone()));
                }
            }
            OpenVpnProxy::Socks {
                server,
                port,
                retry,
            } => {
                if *port == 0 {
                    return Err(ConnectionError::InvalidAddress(
                        "proxy port must not be zero".into(),
                    ));
                }
                vpn_data.push(("proxy-type".into(), "socks".into()));
                vpn_data.push(("proxy-server".into(), server.clone()));
                vpn_data.push(("proxy-port".into(), port.to_string()));
                vpn_data.push((
                    "proxy-retry".into(),
                    if *retry { "yes" } else { "no" }.into(),
                ));
            }
        }
    }

    let data_dict = string_pairs_to_dict(vpn_data)?;

    let mut vpn_secrets: Vec<(String, String)> = Vec::new();
    push_opt_str(&mut vpn_secrets, "password", config.password.as_ref());
    push_opt_str(&mut vpn_secrets, "cert-pass", config.key_password.as_ref());

    let mut vpn: HashMap<&'static str, Value<'static>> = HashMap::new();
    vpn.insert(
        "service-type",
        Value::from("org.freedesktop.NetworkManager.openvpn"),
    );
    vpn.insert("data", Value::from(data_dict));
    if !vpn_secrets.is_empty() {
        vpn.insert("secrets", Value::from(string_pairs_to_dict(vpn_secrets)?));
    }

    let mut ipv4: HashMap<&'static str, Value<'static>> = HashMap::new();
    ipv4.insert("method", Value::from("auto"));
    if config.redirect_gateway {
        ipv4.insert("never-default", Value::from(false));
    }
    if !config.routes.is_empty() {
        let route_data: Vec<HashMap<String, Value<'static>>> = config
            .routes
            .iter()
            .map(|route| {
                let mut route_dict = HashMap::new();
                route_dict.insert("dest".to_string(), Value::from(route.dest.clone()));
                route_dict.insert("prefix".to_string(), Value::from(route.prefix));
                if let Some(ref nh) = route.next_hop {
                    route_dict.insert("next-hop".to_string(), Value::from(nh.clone()));
                }
                if let Some(m) = route.metric {
                    route_dict.insert("metric".to_string(), Value::from(m));
                }
                route_dict
            })
            .collect();
        ipv4.insert("route-data", Value::from(route_data));
    }
    if let Some(dns) = &config.dns {
        ipv4.insert("dns-data", Value::from(dns.clone()));
    }

    let mut ipv6: HashMap<&'static str, Value<'static>> = HashMap::new();
    ipv6.insert("method", Value::from("ignore"));

    let mut settings = HashMap::new();
    settings.insert("connection", connection);
    settings.insert("vpn", vpn);
    settings.insert("ipv4", ipv4);
    settings.insert("ipv6", ipv6);

    Ok(settings)
}
#[cfg(test)]
mod tests {
    use super::*;
    use crate::api::models::{
        OpenVpnCompression, OpenVpnConfig, OpenVpnProxy, VpnKind, WireGuardPeer,
    };

    fn create_test_credentials() -> VpnCredentials {
        let peer = WireGuardPeer::new(
            "HIgo9xNzJMWLKAShlKl6/bUT1VI9Q0SDBXGtLXkPFXc=",
            "vpn.example.com:51820",
            vec!["0.0.0.0/0".into()],
        )
        .with_persistent_keepalive(25);

        VpnCredentials::new(
            VpnKind::WireGuard,
            "TestVPN",
            "vpn.example.com:51820",
            "YBk6X3pP8KjKz7+HFWzVHNqL3qTZq8hX9VxFQJ4zVmM=",
            "10.0.0.2/24",
            vec![peer],
        )
        .with_dns(vec!["1.1.1.1".into(), "8.8.8.8".into()])
        .with_mtu(1420)
    }

    fn create_test_options() -> ConnectionOptions {
        ConnectionOptions::new(false)
    }

    fn address_data(address: &str, prefix: u32) -> Value<'static> {
        let mut entry = HashMap::new();
        entry.insert("address".to_string(), Value::from(address.to_string()));
        entry.insert("prefix".to_string(), Value::from(prefix));
        Value::from(vec![entry])
    }

    fn peer_string_array(peer: &Dict<'_, '_>, key: &str) -> Vec<String> {
        let value = peer
            .iter()
            .find_map(|(candidate, value)| {
                matches!(candidate, Value::Str(candidate) if candidate.as_str() == key)
                    .then_some(value)
            })
            .unwrap_or_else(|| panic!("missing peer property {key}"));
        let Value::Value(value) = value else {
            panic!("peer property {key} must be stored as a variant");
        };
        let Value::Array(values) = value.as_ref() else {
            panic!("peer property {key} must be an array");
        };
        values
            .iter()
            .map(|value| match value {
                Value::Str(value) => value.as_str().to_string(),
                _ => panic!("peer property {key} entries must be strings"),
            })
            .collect()
    }

    #[test]
    fn connection_section_has_correct_type() {
        let creds = create_test_credentials();
        let opts = create_test_options();

        let settings = build_wireguard_connection(&creds, &opts).unwrap();
        let connection = settings.get("connection").unwrap();

        let conn_type = connection.get("type").unwrap();
        assert_eq!(conn_type, &Value::from("wireguard"));

        let id = connection.get("id").unwrap();
        assert_eq!(id, &Value::from("TestVPN"));
    }

    #[test]
    fn wireguard_section_has_no_service_type() {
        let creds = create_test_credentials();
        let opts = create_test_options();

        let settings = build_wireguard_connection(&creds, &opts).unwrap();
        let wg = settings.get("wireguard").unwrap();

        assert!(
            wg.get("service-type").is_none(),
            "kernel WireGuard connections must not have a service-type property"
        );
    }

    #[test]
    fn ipv4_section_is_manual() {
        let creds = create_test_credentials();
        let opts = create_test_options();

        let settings = build_wireguard_connection(&creds, &opts).unwrap();
        let ipv4 = settings.get("ipv4").unwrap();

        let method = ipv4.get("method").unwrap();
        assert_eq!(method, &Value::from("manual"));
    }

    #[test]
    fn ipv6_section_is_ignored() {
        let creds = create_test_credentials();
        let opts = create_test_options();

        let settings = build_wireguard_connection(&creds, &opts).unwrap();
        let ipv6 = settings.get("ipv6").unwrap();

        let method = ipv6.get("method").unwrap();
        assert_eq!(method, &Value::from("ignore"));
    }

    #[test]
    fn accepts_ipv6_address() {
        let mut creds = create_test_credentials();
        creds.address = "fd00::2/64".into();
        let opts = create_test_options();

        let settings = build_wireguard_connection(&creds, &opts).unwrap();
        assert_eq!(
            settings["ipv4"].get("method"),
            Some(&Value::from("disabled"))
        );
        assert!(!settings["ipv4"].contains_key("address-data"));
        assert_eq!(settings["ipv6"].get("method"), Some(&Value::from("manual")));
        assert_eq!(
            settings["ipv6"].get("address-data"),
            Some(&address_data("fd00::2", 64))
        );
    }

    #[test]
    fn handles_multiple_peers() {
        let mut creds = create_test_credentials();
        let extra_peer = WireGuardPeer::new(
            "xScVkH3fUGUVRvGLFcjkx+GGD7cf5eBVyN3Gh4FLjmI=",
            "peer2.example.com:51821",
            vec!["192.168.0.0/16".into()],
        )
        .with_preshared_key("PSKABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklm=");

        creds.peers.push(extra_peer);
        let opts = create_test_options();

        let settings = build_wireguard_connection(&creds, &opts).unwrap();
        let peers = settings["wireguard"].get("peers").unwrap();
        assert_eq!(peers.value_signature().to_string(), "aa{sv}");
        let Value::Array(peers) = peers else {
            panic!("wireguard.peers must be an array");
        };
        let peers = peers.iter().collect::<Vec<_>>();
        assert_eq!(peers.len(), 2);
        assert_eq!(peers[0].value_signature().to_string(), "a{sv}");
        assert_eq!(peers[1].value_signature().to_string(), "a{sv}");

        let Value::Dict(first) = peers[0] else {
            panic!("first wireguard peer must be a dictionary");
        };
        assert_eq!(
            first
                .get::<Value, String>(&Value::from("public-key"))
                .unwrap()
                .as_deref(),
            Some("HIgo9xNzJMWLKAShlKl6/bUT1VI9Q0SDBXGtLXkPFXc=")
        );
        assert_eq!(
            first
                .get::<Value, String>(&Value::from("endpoint"))
                .unwrap()
                .as_deref(),
            Some("vpn.example.com:51820")
        );
        assert_eq!(
            peer_string_array(first, "allowed-ips"),
            vec!["0.0.0.0/0".to_string()]
        );
        assert!(
            first
                .get::<Value, String>(&Value::from("preshared-key"))
                .unwrap()
                .is_none()
        );
        assert_eq!(
            first
                .get::<Value, u32>(&Value::from("persistent-keepalive"))
                .unwrap(),
            Some(25)
        );

        let Value::Dict(second) = peers[1] else {
            panic!("second wireguard peer must be a dictionary");
        };
        assert_eq!(
            second
                .get::<Value, String>(&Value::from("public-key"))
                .unwrap()
                .as_deref(),
            Some("xScVkH3fUGUVRvGLFcjkx+GGD7cf5eBVyN3Gh4FLjmI=")
        );
        assert_eq!(
            second
                .get::<Value, String>(&Value::from("endpoint"))
                .unwrap()
                .as_deref(),
            Some("peer2.example.com:51821")
        );
        assert_eq!(
            peer_string_array(second, "allowed-ips"),
            vec!["192.168.0.0/16".to_string()]
        );
        assert_eq!(
            second
                .get::<Value, String>(&Value::from("preshared-key"))
                .unwrap()
                .as_deref(),
            Some("PSKABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklm=")
        );
        assert!(
            second
                .get::<Value, u32>(&Value::from("persistent-keepalive"))
                .unwrap()
                .is_none()
        );
    }

    #[test]
    fn handles_optional_dns() {
        let mut creds = create_test_credentials();
        creds.dns = None;
        let opts = create_test_options();

        let settings = build_wireguard_connection(&creds, &opts).unwrap();
        assert!(!settings["ipv4"].contains_key("dns"));
        assert!(!settings["ipv6"].contains_key("dns"));
    }

    #[test]
    fn handles_optional_mtu() {
        let mut creds = create_test_credentials();
        creds.mtu = None;
        let opts = create_test_options();

        let settings = build_wireguard_connection(&creds, &opts).unwrap();
        assert!(!settings["wireguard"].contains_key("mtu"));
    }

    #[test]
    fn includes_dns_when_provided() {
        let creds = create_test_credentials();
        let opts = create_test_options();

        let settings = build_wireguard_connection(&creds, &opts).unwrap();
        let ipv4 = settings.get("ipv4").unwrap();

        assert_eq!(
            ipv4.get("dns"),
            Some(&Value::from(vec![
                u32::from(std::net::Ipv4Addr::new(1, 1, 1, 1)),
                u32::from(std::net::Ipv4Addr::new(8, 8, 8, 8)),
            ]))
        );
        assert_eq!(ipv4["dns"].value_signature().to_string(), "au");
    }

    #[test]
    fn includes_mtu_when_provided() {
        let creds = create_test_credentials();
        let opts = create_test_options();

        let settings = build_wireguard_connection(&creds, &opts).unwrap();
        assert_eq!(
            settings["wireguard"].get("mtu"),
            Some(&Value::from(1420u32))
        );
        assert!(!settings["ipv4"].contains_key("mtu"));
    }

    #[test]
    fn respects_autoconnect_option() {
        let creds = create_test_credentials();
        let mut opts = create_test_options();
        opts.autoconnect = true;

        let settings = build_wireguard_connection(&creds, &opts).unwrap();
        let connection = settings.get("connection").unwrap();

        let autoconnect = connection.get("autoconnect").unwrap();
        assert_eq!(autoconnect, &Value::from(true));
    }

    #[test]
    fn includes_autoconnect_priority_when_provided() {
        let creds = create_test_credentials();
        let mut opts = create_test_options();
        opts.autoconnect_priority = Some(10);

        let settings = build_wireguard_connection(&creds, &opts).unwrap();
        let connection = settings.get("connection").unwrap();

        assert_eq!(
            connection.get("autoconnect-priority"),
            Some(&Value::from(10i32))
        );
    }

    #[test]
    fn generates_uuid_when_not_provided() {
        let creds = create_test_credentials();
        let opts = create_test_options();

        let settings = build_wireguard_connection(&creds, &opts).unwrap();
        let connection = settings.get("connection").unwrap();

        assert!(connection.contains_key("uuid"));
    }

    #[test]
    fn uses_provided_uuid() {
        let mut creds = create_test_credentials();
        let test_uuid = uuid::Uuid::new_v4();
        creds.uuid = Some(test_uuid);
        let opts = create_test_options();

        let settings = build_wireguard_connection(&creds, &opts).unwrap();
        let connection = settings.get("connection").unwrap();

        let uuid = connection.get("uuid").unwrap();
        assert_eq!(uuid, &Value::from(test_uuid.to_string()));
    }

    #[test]
    fn peer_with_preshared_key() {
        let mut creds = create_test_credentials();
        creds.peers[0].preshared_key = Some("PSKABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklm=".into());
        let opts = create_test_options();

        let settings = build_wireguard_connection(&creds, &opts).unwrap();
        let Value::Array(peers) = &settings["wireguard"]["peers"] else {
            panic!("wireguard.peers must be an array");
        };
        let Value::Dict(peer) = peers.iter().next().unwrap() else {
            panic!("wireguard peer must be a dictionary");
        };
        assert_eq!(
            peer.get::<Value, String>(&Value::from("preshared-key"))
                .unwrap()
                .as_deref(),
            Some("PSKABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklm=")
        );
    }

    #[test]
    fn peer_without_keepalive() {
        let mut creds = create_test_credentials();
        creds.peers[0].persistent_keepalive = None;
        let opts = create_test_options();

        let settings = build_wireguard_connection(&creds, &opts).unwrap();
        let Value::Array(peers) = &settings["wireguard"]["peers"] else {
            panic!("wireguard.peers must be an array");
        };
        let Value::Dict(peer) = peers.iter().next().unwrap() else {
            panic!("wireguard peer must be a dictionary");
        };
        assert!(
            peer.get::<Value, u32>(&Value::from("persistent-keepalive"))
                .unwrap()
                .is_none()
        );
    }

    #[test]
    fn multiple_allowed_ips_for_peer() {
        let mut creds = create_test_credentials();
        creds.peers[0].allowed_ips =
            vec!["0.0.0.0/0".into(), "::/0".into(), "192.168.1.0/24".into()];
        let opts = create_test_options();

        let settings = build_wireguard_connection(&creds, &opts).unwrap();
        let Value::Array(peers) = &settings["wireguard"]["peers"] else {
            panic!("wireguard.peers must be an array");
        };
        let Value::Dict(peer) = peers.iter().next().unwrap() else {
            panic!("wireguard peer must be a dictionary");
        };
        let allowed_ips = peer
            .iter()
            .find_map(|(key, value)| {
                matches!(key, Value::Str(key) if key.as_str() == "allowed-ips").then_some(value)
            })
            .expect("allowed-ips peer property");
        let Value::Value(allowed_ips) = allowed_ips else {
            panic!("allowed-ips must be stored as a variant");
        };
        let Value::Array(allowed_ips) = allowed_ips.as_ref() else {
            panic!("allowed-ips must be an array");
        };
        let allowed_ips = allowed_ips
            .iter()
            .map(|value| match value {
                Value::Str(value) => value.as_str(),
                _ => panic!("allowed-ips entries must be strings"),
            })
            .collect::<Vec<_>>();
        assert_eq!(allowed_ips, vec!["0.0.0.0/0", "::/0", "192.168.1.0/24"]);
    }

    #[test]
    fn legacy_builder_propagates_wireguard_validation_errors() {
        let mut creds = create_test_credentials();
        creds.peers[0].public_key = "!".repeat(44);
        let opts = create_test_options();

        let result = build_wireguard_connection(&creds, &opts);
        assert!(matches!(
            result.unwrap_err(),
            ConnectionError::InvalidPublicKey(message)
                if message == "Peer 0 public key contains invalid base64 characters"
        ));
    }

    #[test]
    fn accepts_valid_ipv4_addresses() {
        let test_cases = vec![
            "10.0.0.2/24",
            "192.168.1.100/32",
            "172.16.0.1/16",
            "1.1.1.1/8",
        ];

        for address in test_cases {
            let mut creds = create_test_credentials();
            creds.address = address.into();
            let opts = create_test_options();

            let settings = build_wireguard_connection(&creds, &opts)
                .unwrap_or_else(|error| panic!("valid address {address} failed: {error}"));
            let (ip, prefix) = address.split_once('/').unwrap();
            assert_eq!(
                settings["ipv4"].get("address-data"),
                Some(&address_data(ip, prefix.parse().unwrap()))
            );
        }
    }

    #[test]
    fn accepts_standard_wireguard_ports() {
        let test_cases = vec![
            "vpn.example.com:51820",
            "192.168.1.1:51821",
            "test.local:12345",
        ];

        for gateway in test_cases {
            let mut creds = create_test_credentials();
            creds.peers[0].gateway = gateway.into();
            let opts = create_test_options();

            let settings = build_wireguard_connection(&creds, &opts)
                .unwrap_or_else(|error| panic!("valid gateway {gateway} failed: {error}"));
            let Value::Array(peers) = &settings["wireguard"]["peers"] else {
                panic!("wireguard.peers must be an array");
            };
            let Value::Dict(peer) = peers.iter().next().unwrap() else {
                panic!("wireguard peer must be a dictionary");
            };
            assert_eq!(
                peer.get::<Value, String>(&Value::from("endpoint"))
                    .unwrap()
                    .as_deref(),
                Some(gateway)
            );
        }
    }

    // --- OpenVPN tests ---
    fn create_openvpn_config() -> OpenVpnConfig {
        OpenVpnConfig::new("TestOpenVPN", "vpn.example.com", 1194, false)
            .with_ca_cert("/etc/openvpn/ca.crt")
            .with_client_cert("/etc/openvpn/client.crt")
            .with_client_key("/etc/openvpn/client.key")
    }

    #[test]
    fn openvpn_connection_type_is_vpn() {
        let config = create_openvpn_config();
        let opts = create_test_options();
        let settings = build_openvpn_connection(&config, &opts).unwrap();
        let conn = settings.get("connection").unwrap();
        assert_eq!(conn.get("type").unwrap(), &Value::from("vpn"));
    }

    #[test]
    fn openvpn_service_type_is_correct() {
        let config = create_openvpn_config();
        let opts = create_test_options();
        let settings = build_openvpn_connection(&config, &opts).unwrap();
        let vpn = settings.get("vpn").unwrap();
        assert_eq!(
            vpn.get("service-type").unwrap(),
            &Value::from("org.freedesktop.NetworkManager.openvpn")
        );
    }

    #[test]
    fn openvpn_rejects_empty_remote() {
        let mut config = create_openvpn_config();
        config.remote = "".into();
        let opts = create_test_options();
        let result = build_openvpn_connection(&config, &opts);
        assert!(matches!(
            result.unwrap_err(),
            ConnectionError::InvalidGateway(message)
                if message == "OpenVPN remote must not be empty"
        ));
    }

    #[allow(deprecated)]
    #[test]
    fn openvpn_serializes_each_compression_mode() {
        let cases = [
            (OpenVpnCompression::No, "compress", "no"),
            (OpenVpnCompression::Lzo, "comp-lzo", "yes"),
            (OpenVpnCompression::Lz4, "compress", "lz4"),
            (OpenVpnCompression::Lz4V2, "compress", "lz4-v2"),
            (OpenVpnCompression::Yes, "compress", "yes"),
        ];

        for (compression, key, expected) in cases {
            let config = create_openvpn_config().with_compression(compression.clone());
            let settings = build_openvpn_connection(&config, &create_test_options()).unwrap();
            assert_eq!(
                get_vpn_data_value(&settings, key).as_deref(),
                Some(expected),
                "wrong serialized value for {compression:?}"
            );
        }
    }

    #[test]
    fn openvpn_http_proxy() {
        let config = create_openvpn_config().with_proxy(OpenVpnProxy::Http {
            server: "proxy.example.com".into(),
            port: 8080,
            username: Some("user".into()),
            password: Some("pass".into()),
            retry: true,
        });
        let settings = build_openvpn_connection(&config, &create_test_options()).unwrap();

        assert_eq!(
            get_vpn_data_value(&settings, "proxy-type").as_deref(),
            Some("http")
        );
        assert_eq!(
            get_vpn_data_value(&settings, "proxy-server").as_deref(),
            Some("proxy.example.com")
        );
        assert_eq!(
            get_vpn_data_value(&settings, "proxy-port").as_deref(),
            Some("8080")
        );
        assert_eq!(
            get_vpn_data_value(&settings, "proxy-retry").as_deref(),
            Some("yes")
        );
        assert_eq!(
            get_vpn_data_value(&settings, "http-proxy-username").as_deref(),
            Some("user")
        );
        assert_eq!(
            get_vpn_data_value(&settings, "http-proxy-password").as_deref(),
            Some("pass")
        );
    }

    #[test]
    fn openvpn_http_proxy_no_credentials() {
        let config = create_openvpn_config().with_proxy(OpenVpnProxy::Http {
            server: "proxy.example.com".into(),
            port: 3128,
            username: None,
            password: None,
            retry: false,
        });
        let settings = build_openvpn_connection(&config, &create_test_options()).unwrap();

        assert_eq!(
            get_vpn_data_value(&settings, "proxy-type").as_deref(),
            Some("http")
        );
        assert_eq!(
            get_vpn_data_value(&settings, "proxy-port").as_deref(),
            Some("3128")
        );
        assert_eq!(
            get_vpn_data_value(&settings, "proxy-retry").as_deref(),
            Some("no")
        );
        assert!(get_vpn_data_value(&settings, "http-proxy-username").is_none());
        assert!(get_vpn_data_value(&settings, "http-proxy-password").is_none());
    }

    #[test]
    fn openvpn_socks_proxy() {
        let config = create_openvpn_config().with_proxy(OpenVpnProxy::Socks {
            server: "socks.example.com".into(),
            port: 1080,
            retry: false,
        });
        let settings = build_openvpn_connection(&config, &create_test_options()).unwrap();

        assert_eq!(
            get_vpn_data_value(&settings, "proxy-type").as_deref(),
            Some("socks")
        );
        assert_eq!(
            get_vpn_data_value(&settings, "proxy-server").as_deref(),
            Some("socks.example.com")
        );
        assert_eq!(
            get_vpn_data_value(&settings, "proxy-port").as_deref(),
            Some("1080")
        );
        assert_eq!(
            get_vpn_data_value(&settings, "proxy-retry").as_deref(),
            Some("no")
        );
    }

    #[test]
    fn openvpn_proxy_rejects_zero_port_http() {
        let config = create_openvpn_config().with_proxy(OpenVpnProxy::Http {
            server: "proxy.example.com".into(),
            port: 0,
            username: None,
            password: None,
            retry: false,
        });
        let opts = create_test_options();
        assert!(matches!(
            build_openvpn_connection(&config, &opts).unwrap_err(),
            ConnectionError::InvalidAddress(message)
                if message == "proxy port must not be zero"
        ));
    }

    #[test]
    fn openvpn_proxy_rejects_zero_port_socks() {
        let config = create_openvpn_config().with_proxy(OpenVpnProxy::Socks {
            server: "socks.example.com".into(),
            port: 0,
            retry: false,
        });
        let opts = create_test_options();
        assert!(matches!(
            build_openvpn_connection(&config, &opts).unwrap_err(),
            ConnectionError::InvalidAddress(message)
                if message == "proxy port must not be zero"
        ));
    }

    #[test]
    fn openvpn_with_dns() {
        let config = create_openvpn_config().with_dns(vec!["1.1.1.1".into(), "8.8.8.8".into()]);
        let opts = create_test_options();
        let settings = build_openvpn_connection(&config, &opts).unwrap();
        let ipv4 = settings.get("ipv4").unwrap();
        let dns = ipv4.get("dns-data").unwrap();
        assert_eq!(dns.value_signature().to_string(), "as");
        assert_eq!(
            dns,
            &Value::from(vec!["1.1.1.1".to_string(), "8.8.8.8".to_string()])
        );
    }

    #[test]
    fn openvpn_tcp_emits_proto_tcp() {
        let config = OpenVpnConfig::new("TcpVPN", "vpn.example.com", 443, true);
        let opts = create_test_options();
        let settings = build_openvpn_connection(&config, &opts).unwrap();
        assert_eq!(
            get_vpn_data_value(&settings, "proto-tcp").as_deref(),
            Some("yes")
        );
    }

    #[test]
    fn openvpn_serializes_all_autoconnect_options() {
        let opts = ConnectionOptions::new(true)
            .with_priority(12)
            .with_retries(7);
        let settings = build_openvpn_connection(&create_openvpn_config(), &opts).unwrap();
        let connection = settings.get("connection").unwrap();

        assert_eq!(connection.get("autoconnect"), Some(&Value::from(true)));
        assert_eq!(
            connection.get("autoconnect-priority"),
            Some(&Value::from(12i32))
        );
        assert_eq!(
            connection.get("autoconnect-retries"),
            Some(&Value::from(7i32))
        );
    }

    #[test]
    fn openvpn_vpn_data_has_dict_signature() {
        let config = create_openvpn_config();
        let opts = create_test_options();
        let settings = build_openvpn_connection(&config, &opts).unwrap();
        let vpn = settings.get("vpn").unwrap();
        let data = vpn.get("data").unwrap();
        assert_eq!(
            data.value_signature().to_string(),
            "a{ss}",
            "vpn.data must be a{{ss}} for NetworkManager"
        );
    }

    fn get_vpn_data_value(
        settings: &HashMap<&str, HashMap<&str, Value>>,
        key: &str,
    ) -> Option<String> {
        let vpn = settings.get("vpn")?;
        let data = vpn.get("data")?;
        if let Value::Dict(dict) = data {
            let val: String = dict.get::<Value, String>(&Value::from(key)).ok()??;
            return Some(val);
        }
        None
    }

    #[test]
    fn openvpn_vpn_secrets_has_dict_signature() {
        let config = create_openvpn_config()
            .with_auth_type(OpenVpnAuthType::Password)
            .with_username("user")
            .with_password("secret");
        let opts = create_test_options();
        let settings = build_openvpn_connection(&config, &opts).unwrap();
        let vpn = settings.get("vpn").unwrap();
        let secrets = vpn.get("secrets").unwrap();
        assert_eq!(
            secrets.value_signature().to_string(),
            "a{ss}",
            "vpn.secrets must be a{{ss}} for NetworkManager"
        );
    }

    #[test]
    fn openvpn_tls_auth_key_and_direction() {
        let config = create_openvpn_config().with_tls_auth("/etc/openvpn/ta.key", Some(1));
        let opts = create_test_options();
        let settings = build_openvpn_connection(&config, &opts).unwrap();
        assert_eq!(
            get_vpn_data_value(&settings, "tls-auth").as_deref(),
            Some("/etc/openvpn/ta.key")
        );
        assert_eq!(
            get_vpn_data_value(&settings, "ta-dir").as_deref(),
            Some("1")
        );
    }

    #[test]
    fn openvpn_tls_auth_key_without_direction() {
        let config = create_openvpn_config().with_tls_auth("/etc/openvpn/ta.key", None);
        let opts = create_test_options();
        let settings = build_openvpn_connection(&config, &opts).unwrap();
        assert_eq!(
            get_vpn_data_value(&settings, "tls-auth").as_deref(),
            Some("/etc/openvpn/ta.key")
        );
        assert!(get_vpn_data_value(&settings, "ta-dir").is_none());
    }

    #[test]
    fn openvpn_tls_crypt() {
        let config = create_openvpn_config().with_tls_crypt("/etc/openvpn/tls-crypt.key");
        let opts = create_test_options();
        let settings = build_openvpn_connection(&config, &opts).unwrap();
        assert_eq!(
            get_vpn_data_value(&settings, "tls-crypt").as_deref(),
            Some("/etc/openvpn/tls-crypt.key")
        );
    }

    #[test]
    fn openvpn_tls_crypt_v2() {
        let config = create_openvpn_config().with_tls_crypt_v2("/etc/openvpn/tls-crypt-v2.key");
        let opts = create_test_options();
        let settings = build_openvpn_connection(&config, &opts).unwrap();
        assert_eq!(
            get_vpn_data_value(&settings, "tls-crypt-v2").as_deref(),
            Some("/etc/openvpn/tls-crypt-v2.key")
        );
    }

    #[test]
    fn openvpn_tls_version_min() {
        let config = create_openvpn_config().with_tls_version_min("1.2");
        let opts = create_test_options();
        let settings = build_openvpn_connection(&config, &opts).unwrap();
        assert_eq!(
            get_vpn_data_value(&settings, "tls-version-min").as_deref(),
            Some("1.2")
        );
    }

    #[test]
    fn openvpn_tls_version_max() {
        let config = create_openvpn_config().with_tls_version_max("1.3");
        let opts = create_test_options();
        let settings = build_openvpn_connection(&config, &opts).unwrap();
        assert_eq!(
            get_vpn_data_value(&settings, "tls-version-max").as_deref(),
            Some("1.3")
        );
    }

    #[test]
    fn openvpn_tls_cipher() {
        let config =
            create_openvpn_config().with_tls_cipher("TLS-ECDHE-RSA-WITH-AES-256-GCM-SHA384");
        let opts = create_test_options();
        let settings = build_openvpn_connection(&config, &opts).unwrap();
        assert_eq!(
            get_vpn_data_value(&settings, "tls-cipher").as_deref(),
            Some("TLS-ECDHE-RSA-WITH-AES-256-GCM-SHA384")
        );
    }

    #[test]
    fn openvpn_remote_cert_tls() {
        let config = create_openvpn_config().with_remote_cert_tls("server");
        let opts = create_test_options();
        let settings = build_openvpn_connection(&config, &opts).unwrap();
        assert_eq!(
            get_vpn_data_value(&settings, "remote-cert-tls").as_deref(),
            Some("server")
        );
    }

    #[test]
    fn openvpn_verify_x509_name() {
        let config = create_openvpn_config().with_verify_x509_name("vpn.example.com", "name");
        let opts = create_test_options();
        let settings = build_openvpn_connection(&config, &opts).unwrap();
        assert_eq!(
            get_vpn_data_value(&settings, "verify-x509-name").as_deref(),
            Some("vpn.example.com")
        );
        assert_eq!(
            get_vpn_data_value(&settings, "verify-x509-type").as_deref(),
            Some("name")
        );
    }

    #[test]
    fn openvpn_crl_verify() {
        let config = create_openvpn_config().with_crl_verify("/etc/openvpn/crl.pem");
        let opts = create_test_options();
        let settings = build_openvpn_connection(&config, &opts).unwrap();
        assert_eq!(
            get_vpn_data_value(&settings, "crl-verify").as_deref(),
            Some("/etc/openvpn/crl.pem")
        );
    }

    #[test]
    fn openvpn_tls_options_absent_by_default() {
        let config = create_openvpn_config();
        let opts = create_test_options();
        let settings = build_openvpn_connection(&config, &opts).unwrap();
        assert!(get_vpn_data_value(&settings, "tls-auth").is_none());
        assert!(get_vpn_data_value(&settings, "ta-dir").is_none());
        assert!(get_vpn_data_value(&settings, "tls-crypt").is_none());
        assert!(get_vpn_data_value(&settings, "tls-crypt-v2").is_none());
        assert!(get_vpn_data_value(&settings, "tls-version-min").is_none());
        assert!(get_vpn_data_value(&settings, "tls-version-max").is_none());
        assert!(get_vpn_data_value(&settings, "tls-cipher").is_none());
        assert!(get_vpn_data_value(&settings, "remote-cert-tls").is_none());
        assert!(get_vpn_data_value(&settings, "verify-x509-name").is_none());
        assert!(get_vpn_data_value(&settings, "crl-verify").is_none());
    }

    #[test]
    fn openvpn_resilience_keys_in_vpn_data() {
        let config = create_openvpn_config()
            .with_ping(10)
            .with_ping_exit(60)
            .with_ping_restart(120)
            .with_reneg_seconds(3600)
            .with_connect_timeout(30);
        let opts = create_test_options();
        let settings = build_openvpn_connection(&config, &opts).unwrap();
        assert_eq!(get_vpn_data_value(&settings, "ping").as_deref(), Some("10"));
        assert_eq!(
            get_vpn_data_value(&settings, "ping-exit").as_deref(),
            Some("60")
        );
        assert_eq!(
            get_vpn_data_value(&settings, "ping-restart").as_deref(),
            Some("120")
        );
        assert_eq!(
            get_vpn_data_value(&settings, "reneg-sec").as_deref(),
            Some("3600")
        );
        assert_eq!(
            get_vpn_data_value(&settings, "connect-timeout").as_deref(),
            Some("30")
        );
    }

    #[test]
    fn openvpn_data_ciphers_and_ncp_disable() {
        let config = create_openvpn_config()
            .with_data_ciphers("AES-256-GCM:AES-128-GCM")
            .with_data_ciphers_fallback("AES-256-GCM")
            .with_ncp_disable(true);
        let opts = create_test_options();
        let settings = build_openvpn_connection(&config, &opts).unwrap();
        assert_eq!(
            get_vpn_data_value(&settings, "data-ciphers").as_deref(),
            Some("AES-256-GCM:AES-128-GCM")
        );
        assert_eq!(
            get_vpn_data_value(&settings, "data-ciphers-fallback").as_deref(),
            Some("AES-256-GCM")
        );
        assert_eq!(
            get_vpn_data_value(&settings, "ncp-disable").as_deref(),
            Some("yes")
        );
    }

    #[test]
    fn openvpn_ipv4_route_data() {
        use crate::api::models::VpnRoute;
        let config = create_openvpn_config().with_routes(vec![
            VpnRoute::new("10.0.0.0", 24)
                .next_hop("192.168.1.1")
                .metric(75),
        ]);
        let opts = create_test_options();
        let settings = build_openvpn_connection(&config, &opts).unwrap();
        let ipv4 = settings.get("ipv4").unwrap();
        let rd = ipv4.get("route-data").unwrap();
        assert_eq!(rd.value_signature().to_string(), "aa{sv}");
        let mut expected = HashMap::new();
        expected.insert("dest".to_string(), Value::from("10.0.0.0".to_string()));
        expected.insert("prefix".to_string(), Value::from(24u32));
        expected.insert(
            "next-hop".to_string(),
            Value::from("192.168.1.1".to_string()),
        );
        expected.insert("metric".to_string(), Value::from(75u32));
        assert_eq!(rd, &Value::from(vec![expected]));
    }

    #[test]
    fn openvpn_redirect_gateway_sets_never_default() {
        let config = create_openvpn_config().with_redirect_gateway(true);
        let opts = create_test_options();
        let settings = build_openvpn_connection(&config, &opts).unwrap();
        let ipv4 = settings.get("ipv4").unwrap();
        assert_eq!(ipv4.get("never-default"), Some(&Value::from(false)));
    }
}
