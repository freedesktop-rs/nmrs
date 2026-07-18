//! Core VPN connection management logic.
//!
//! Supports:
//! - WireGuard connections (`connection.type == "wireguard"`)
//! - NM plugin VPNs (`connection.type == "vpn"`) — OpenVPN, OpenConnect,
//!   strongSwan, PPTP, L2TP, and any other installed plugin.
#![allow(deprecated)]

use log::{debug, info, trace, warn};
use std::collections::HashMap;
use zbus::Connection;
use zvariant::OwnedObjectPath;

use crate::Result;
use crate::api::models::{
    ConnectionError, ConnectionOptions, DeviceState, OpenVpnConnectionType, TimeoutConfig,
    VpnConfig, VpnConnection, VpnConnectionInfo, VpnCredentials, VpnDetails, VpnKind,
    VpnSecretFlags, VpnType,
};
use crate::builders::{build_openvpn_connection, build_wireguard_connection};
use crate::core::state_wait::wait_for_connection_activation;
use crate::dbus::{NMActiveConnectionProxy, NMProxy};
use crate::models::VpnConfiguration;
use crate::util::utils::{extract_connection_state_reason, nm_proxy, settings_proxy};
use crate::util::validation::{
    validate_connection_name, validate_openvpn_config, validate_vpn_credentials,
};

/// Detects whether a saved connection is a VPN and what kind.
fn detect_vpn_kind(
    settings: &HashMap<String, HashMap<String, zvariant::Value<'_>>>,
) -> Option<VpnKind> {
    let conn = settings.get("connection")?;
    let conn_type = match conn.get("type")? {
        zvariant::Value::Str(s) => s.as_str(),
        _ => return None,
    };

    match conn_type {
        "wireguard" => Some(VpnKind::WireGuard),
        "vpn" => Some(VpnKind::Plugin),
        _ => None,
    }
}

/// Extracts a string from a `Dict` (vpn.data / vpn.secrets) by key.
fn dict_str(dict: &zvariant::Dict<'_, '_>, key: &str) -> Option<String> {
    dict.iter().find_map(|(k, v)| match (k, v) {
        (zvariant::Value::Str(k_str), value) if k_str.as_str() == key => {
            match unbox_variant(value) {
                zvariant::Value::Str(v_str) => Some(v_str.to_string()),
                _ => None,
            }
        }
        _ => None,
    })
}

fn unbox_variant<'a, 'v>(mut value: &'a zvariant::Value<'v>) -> &'a zvariant::Value<'v> {
    while let zvariant::Value::Value(inner) = value {
        value = inner;
    }
    value
}

fn dict_value<'a, 'v>(
    dict: &'a zvariant::Dict<'v, 'v>,
    key: &str,
) -> Option<&'a zvariant::Value<'v>> {
    dict.iter().find_map(|(k, v)| {
        matches!(k, zvariant::Value::Str(k_str) if k_str.as_str() == key).then(|| unbox_variant(v))
    })
}

/// Converts a full `Dict` to `HashMap<String, String>`.
fn dict_to_map(dict: &zvariant::Dict<'_, '_>) -> HashMap<String, String> {
    dict.iter()
        .filter_map(|(k, v)| match (k, unbox_variant(v)) {
            (zvariant::Value::Str(k_str), zvariant::Value::Str(v_str)) => {
                Some((k_str.to_string(), v_str.to_string()))
            }
            _ => None,
        })
        .collect()
}

/// Decodes a [`VpnType`] from raw NM settings dictionaries.
///
/// Pure function — no D-Bus calls.
pub(crate) fn vpn_type_from_settings(
    kind: VpnKind,
    settings: &HashMap<String, HashMap<String, zvariant::Value<'_>>>,
) -> VpnType {
    match kind {
        VpnKind::WireGuard => decode_wireguard_type(settings),
        VpnKind::Plugin => decode_plugin_type(settings),
    }
}

fn decode_wireguard_type(
    settings: &HashMap<String, HashMap<String, zvariant::Value<'_>>>,
) -> VpnType {
    let wg = settings.get("wireguard");

    let private_key = wg.and_then(|s| s.get("private-key")).and_then(|v| match v {
        zvariant::Value::Str(s) if !s.is_empty() => Some(s.to_string()),
        _ => None,
    });

    let (peer_public_key, endpoint, allowed_ips, persistent_keepalive) =
        if let Some(peers_val) = wg.and_then(|s| s.get("peers")) {
            decode_wg_first_peer(peers_val)
        } else {
            (None, None, vec![], None)
        };

    VpnType::WireGuard {
        private_key,
        peer_public_key,
        endpoint,
        allowed_ips,
        persistent_keepalive,
    }
}

fn decode_wg_first_peer(
    peers_val: &zvariant::Value<'_>,
) -> (Option<String>, Option<String>, Vec<String>, Option<u32>) {
    match peers_val {
        zvariant::Value::Str(s) => {
            let text = s.as_str();
            let first = text.split(',').next().unwrap_or(text).trim();
            let mut pk = None;
            let mut ep = None;
            let mut ips = vec![];
            let mut ka = None;
            for tok in first.split_whitespace() {
                if let Some(v) = tok.strip_prefix("public-key=") {
                    pk = Some(v.to_string());
                } else if let Some(v) = tok.strip_prefix("endpoint=") {
                    ep = Some(v.to_string());
                } else if let Some(v) = tok.strip_prefix("allowed-ips=") {
                    ips = v.split(';').map(|s| s.trim().to_string()).collect();
                } else if let Some(v) = tok.strip_prefix("persistent-keepalive=") {
                    ka = v.parse().ok();
                }
            }
            (pk, ep, ips, ka)
        }
        zvariant::Value::Array(arr) => {
            if let Some(first) = arr.first()
                && let zvariant::Value::Dict(dict) = unbox_variant(first)
            {
                let pk = dict_str(dict, "public-key");
                let ep = dict_str(dict, "endpoint");
                let ips = match dict_value(dict, "allowed-ips") {
                    Some(zvariant::Value::Str(value)) => value
                        .split(';')
                        .map(str::trim)
                        .filter(|value| !value.is_empty())
                        .map(str::to_string)
                        .collect(),
                    Some(zvariant::Value::Array(values)) => values
                        .iter()
                        .filter_map(|value| match unbox_variant(value) {
                            zvariant::Value::Str(value) => Some(value.to_string()),
                            _ => None,
                        })
                        .collect(),
                    _ => Vec::new(),
                };
                let ka = match dict_value(dict, "persistent-keepalive") {
                    Some(zvariant::Value::U32(value)) => Some(*value),
                    Some(zvariant::Value::Str(value)) => value.parse().ok(),
                    _ => None,
                };
                return (pk, ep, ips, ka);
            }
            (None, None, vec![], None)
        }
        _ => (None, None, vec![], None),
    }
}

fn decode_plugin_type(settings: &HashMap<String, HashMap<String, zvariant::Value<'_>>>) -> VpnType {
    let vpn_sec = match settings.get("vpn") {
        Some(s) => s,
        None => {
            return VpnType::Generic {
                service_type: String::new(),
                data: HashMap::new(),
                secrets: HashMap::new(),
                user_name: None,
                password_flags: VpnSecretFlags::default(),
            };
        }
    };

    let service_type = vpn_sec
        .get("service-type")
        .and_then(|v| match v {
            zvariant::Value::Str(s) => Some(s.to_string()),
            _ => None,
        })
        .unwrap_or_default();

    let user_name = vpn_sec.get("user-name").and_then(|v| match v {
        zvariant::Value::Str(s) if !s.is_empty() => Some(s.to_string()),
        _ => None,
    });

    let pf_raw = vpn_sec
        .get("password-flags")
        .and_then(|v| match v {
            zvariant::Value::U32(n) => Some(*n),
            _ => None,
        })
        .unwrap_or(0);
    let password_flags = VpnSecretFlags(pf_raw);

    let data_dict = vpn_sec.get("data");
    let secrets_dict = vpn_sec.get("secrets");

    if service_type.ends_with(".openvpn") {
        return decode_openvpn(data_dict, user_name, password_flags);
    }
    if service_type.ends_with(".openconnect") {
        return decode_openconnect(data_dict, user_name, password_flags);
    }
    if service_type.ends_with(".strongswan") {
        return decode_strongswan(data_dict, user_name, password_flags);
    }
    if service_type.ends_with(".pptp") {
        return decode_pptp(data_dict, user_name, password_flags);
    }
    if service_type.ends_with(".l2tp") {
        return decode_l2tp(data_dict, user_name, password_flags);
    }

    let data = data_dict
        .and_then(|v| match v {
            zvariant::Value::Dict(d) => Some(dict_to_map(d)),
            _ => None,
        })
        .unwrap_or_default();
    let secrets = secrets_dict
        .and_then(|v| match v {
            zvariant::Value::Dict(d) => Some(dict_to_map(d)),
            _ => None,
        })
        .unwrap_or_default();

    VpnType::Generic {
        service_type,
        data,
        secrets,
        user_name,
        password_flags,
    }
}

fn data_str(data_dict: Option<&zvariant::Value<'_>>, key: &str) -> Option<String> {
    match data_dict? {
        zvariant::Value::Dict(d) => dict_str(d, key),
        _ => None,
    }
}

fn data_pf(data_dict: Option<&zvariant::Value<'_>>, key: &str) -> VpnSecretFlags {
    VpnSecretFlags(
        data_str(data_dict, key)
            .and_then(|s| s.parse().ok())
            .unwrap_or(0),
    )
}

fn decode_openvpn(
    data_dict: Option<&zvariant::Value<'_>>,
    user_name: Option<String>,
    _section_pf: VpnSecretFlags,
) -> VpnType {
    let remote = data_str(data_dict, "remote");
    let ct =
        data_str(data_dict, "connection-type").and_then(|s| OpenVpnConnectionType::from_nm_str(&s));
    let un = data_str(data_dict, "username").or(user_name);
    let ca = data_str(data_dict, "ca");
    let cert = data_str(data_dict, "cert");
    let key = data_str(data_dict, "key");
    let ta = data_str(data_dict, "ta");
    let pf = data_pf(data_dict, "password-flags");

    VpnType::OpenVpn {
        remote,
        connection_type: ct,
        user_name: un,
        ca,
        cert,
        key,
        ta,
        password_flags: pf,
    }
}

fn decode_openconnect(
    data_dict: Option<&zvariant::Value<'_>>,
    user_name: Option<String>,
    password_flags: VpnSecretFlags,
) -> VpnType {
    VpnType::OpenConnect {
        gateway: data_str(data_dict, "gateway"),
        user_name: data_str(data_dict, "username").or(user_name),
        protocol: data_str(data_dict, "protocol"),
        password_flags,
    }
}

fn decode_strongswan(
    data_dict: Option<&zvariant::Value<'_>>,
    user_name: Option<String>,
    password_flags: VpnSecretFlags,
) -> VpnType {
    VpnType::StrongSwan {
        address: data_str(data_dict, "address"),
        method: data_str(data_dict, "method"),
        user_name: data_str(data_dict, "user").or(user_name),
        certificate: data_str(data_dict, "certificate"),
        password_flags,
    }
}

fn decode_pptp(
    data_dict: Option<&zvariant::Value<'_>>,
    user_name: Option<String>,
    password_flags: VpnSecretFlags,
) -> VpnType {
    VpnType::Pptp {
        gateway: data_str(data_dict, "gateway"),
        user_name: data_str(data_dict, "user").or(user_name),
        password_flags,
    }
}

fn decode_l2tp(
    data_dict: Option<&zvariant::Value<'_>>,
    user_name: Option<String>,
    password_flags: VpnSecretFlags,
) -> VpnType {
    let ipsec = data_str(data_dict, "ipsec-enabled")
        .map(|v| v == "yes" || v == "true" || v == "1")
        .unwrap_or(false);
    VpnType::L2tp {
        gateway: data_str(data_dict, "gateway"),
        user_name: data_str(data_dict, "user").or(user_name),
        password_flags,
        ipsec_enabled: ipsec,
    }
}

/// Extracts `connection.uuid` from settings.
fn extract_uuid(
    settings: &HashMap<String, HashMap<String, zvariant::Value<'_>>>,
) -> Option<String> {
    settings
        .get("connection")?
        .get("uuid")
        .and_then(|v| match v {
            zvariant::Value::Str(s) => Some(s.to_string()),
            _ => None,
        })
}

/// Extracts `connection.id` from settings.
fn extract_id(settings: &HashMap<String, HashMap<String, zvariant::Value<'_>>>) -> Option<String> {
    settings.get("connection")?.get("id").and_then(|v| match v {
        zvariant::Value::Str(s) => Some(s.to_string()),
        _ => None,
    })
}

/// Extracts `vpn.service-type` or returns empty string for WireGuard.
fn extract_service_type(
    kind: VpnKind,
    settings: &HashMap<String, HashMap<String, zvariant::Value<'_>>>,
) -> String {
    if kind == VpnKind::WireGuard {
        return String::new();
    }
    settings
        .get("vpn")
        .and_then(|s| s.get("service-type"))
        .and_then(|v| match v {
            zvariant::Value::Str(s) => Some(s.to_string()),
            _ => None,
        })
        .unwrap_or_default()
}

fn extract_vpn_user_name(
    settings: &HashMap<String, HashMap<String, zvariant::Value<'_>>>,
) -> Option<String> {
    settings.get("vpn")?.get("user-name").and_then(|v| match v {
        zvariant::Value::Str(s) if !s.is_empty() => Some(s.to_string()),
        _ => None,
    })
}

fn extract_password_flags(
    settings: &HashMap<String, HashMap<String, zvariant::Value<'_>>>,
) -> VpnSecretFlags {
    let raw = settings
        .get("vpn")
        .and_then(|s| s.get("password-flags"))
        .and_then(|v| match v {
            zvariant::Value::U32(n) => Some(*n),
            _ => None,
        })
        .unwrap_or(0);
    VpnSecretFlags(raw)
}

// ── Public core functions ──────────────────────────────────────────────

/// Lists all saved VPN connections with rich metadata.
pub(crate) async fn list_vpn_connections(conn: &Connection) -> Result<Vec<VpnConnection>> {
    let nm = NMProxy::new(conn).await?;

    let settings_proxy = nm_proxy(
        conn,
        "/org/freedesktop/NetworkManager/Settings",
        "org.freedesktop.NetworkManager.Settings",
    )
    .await?;

    let list_reply = settings_proxy
        .call_method("ListConnections", &())
        .await
        .map_err(|e| ConnectionError::DbusOperation {
            context: "failed to list saved connections".to_string(),
            source: e,
        })?;

    let saved_paths: Vec<OwnedObjectPath> = list_reply.body().deserialize()?;

    let active_map = build_active_vpn_map(conn, &nm).await;

    let mut vpn_conns = Vec::new();

    for cpath in saved_paths {
        let cproxy = match nm_proxy(
            conn,
            cpath.clone(),
            "org.freedesktop.NetworkManager.Settings.Connection",
        )
        .await
        {
            Ok(p) => p,
            Err(_) => continue,
        };

        let msg = match cproxy.call_method("GetSettings", &()).await {
            Ok(m) => m,
            Err(_) => continue,
        };

        let body = msg.body();
        let settings_map: HashMap<String, HashMap<String, zvariant::Value>> =
            match body.deserialize() {
                Ok(m) => m,
                Err(_) => continue,
            };

        let Some(kind) = detect_vpn_kind(&settings_map) else {
            continue;
        };

        let Some(uuid) = extract_uuid(&settings_map) else {
            continue;
        };
        let id = extract_id(&settings_map).unwrap_or_default();

        let vpn_type = vpn_type_from_settings(kind, &settings_map);
        let service_type = extract_service_type(kind, &settings_map);
        let user_name = extract_vpn_user_name(&settings_map);
        let password_flags = extract_password_flags(&settings_map);

        let (state, interface, active) =
            active_map
                .get(&uuid)
                .cloned()
                .unwrap_or((DeviceState::Other(0), None, false));

        vpn_conns.push(VpnConnection {
            uuid,
            id: id.clone(),
            name: id,
            vpn_type,
            state,
            interface,
            active,
            user_name,
            password_flags,
            service_type,
            kind,
        });
    }

    Ok(vpn_conns)
}

/// Only active VPN connections.
pub(crate) async fn active_vpn_connections(conn: &Connection) -> Result<Vec<VpnConnection>> {
    let all = list_vpn_connections(conn).await?;
    Ok(all.into_iter().filter(|v| v.active).collect())
}

/// Builds uuid → (state, interface, active) map from NM active connections.
async fn build_active_vpn_map(
    conn: &Connection,
    nm: &NMProxy<'_>,
) -> HashMap<String, (DeviceState, Option<String>, bool)> {
    let mut map = HashMap::new();

    let active_conns = match nm.active_connections().await {
        Ok(c) => c,
        Err(_) => return map,
    };

    for ac_path in active_conns {
        let ac_proxy = match nm_proxy(
            conn,
            ac_path.clone(),
            "org.freedesktop.NetworkManager.Connection.Active",
        )
        .await
        {
            Ok(p) => p,
            Err(_) => continue,
        };

        let uuid: String = match ac_proxy.get_property("Uuid").await {
            Ok(u) => u,
            Err(_) => continue,
        };

        let conn_type: String = match ac_proxy.get_property("Type").await {
            Ok(t) => t,
            Err(_) => continue,
        };

        if conn_type != "vpn" && conn_type != "wireguard" {
            continue;
        }

        let state = ac_proxy
            .get_property::<u32>("State")
            .await
            .map(DeviceState::from)
            .unwrap_or(DeviceState::Other(0));

        let interface = match ac_proxy
            .get_property::<Vec<OwnedObjectPath>>("Devices")
            .await
            .ok()
            .and_then(|devs| devs.first().cloned())
        {
            Some(dev_path) => {
                let dp = nm_proxy(conn, dev_path, "org.freedesktop.NetworkManager.Device")
                    .await
                    .ok();
                match dp {
                    Some(proxy) => proxy.get_property::<String>("Interface").await.ok(),
                    None => None,
                }
            }
            None => None,
        };

        map.insert(uuid, (state, interface, true));
    }

    map
}

/// Activate a saved VPN by UUID.
pub(crate) async fn connect_vpn_by_uuid(
    conn: &Connection,
    uuid: &str,
    timeout_config: Option<TimeoutConfig>,
) -> Result<()> {
    let nm = NMProxy::new(conn).await?;

    let settings_proxy = nm_proxy(
        conn,
        "/org/freedesktop/NetworkManager/Settings",
        "org.freedesktop.NetworkManager.Settings",
    )
    .await?;

    let reply = settings_proxy
        .call_method("GetConnectionByUuid", &(uuid,))
        .await
        .map_err(|_| ConnectionError::VpnNotFound(uuid.to_string()))?;

    let conn_path: OwnedObjectPath = reply.body().deserialize()?;

    let active_conn = nm
        .activate_connection(
            conn_path,
            OwnedObjectPath::default(),
            OwnedObjectPath::default(),
        )
        .await?;

    let timeout = timeout_config.map(|c| c.connection_timeout);
    wait_for_connection_activation(conn, &active_conn, timeout).await
}

/// Activate a saved VPN by connection id (display name).
pub(crate) async fn connect_vpn_by_id(
    conn: &Connection,
    id: &str,
    timeout_config: Option<TimeoutConfig>,
) -> Result<()> {
    let all = list_vpn_connections(conn).await?;
    let matches: Vec<_> = all.iter().filter(|v| v.id == id).collect();

    match matches.len() {
        0 => Err(ConnectionError::VpnNotFound(id.to_string())),
        1 => connect_vpn_by_uuid(conn, &matches[0].uuid, timeout_config).await,
        _ => Err(ConnectionError::VpnIdAmbiguous(id.to_string())),
    }
}

/// Disconnect a VPN by UUID.
pub(crate) async fn disconnect_vpn_by_uuid(conn: &Connection, uuid: &str) -> Result<()> {
    let nm = NMProxy::new(conn).await?;
    let active_conns = nm.active_connections().await.unwrap_or_default();

    for ac_path in active_conns {
        let ac_proxy = match nm_proxy(
            conn,
            ac_path.clone(),
            "org.freedesktop.NetworkManager.Connection.Active",
        )
        .await
        {
            Ok(p) => p,
            Err(_) => continue,
        };

        let ac_uuid: String = match ac_proxy.get_property("Uuid").await {
            Ok(u) => u,
            Err(_) => continue,
        };

        if ac_uuid == uuid {
            nm.deactivate_connection(ac_path).await?;
            return Ok(());
        }
    }

    Ok(())
}

/// Connects to a VPN (WireGuard or OpenVPN) from configuration.
pub(crate) async fn connect_vpn(
    conn: &Connection,
    config: VpnConfiguration,
    timeout_config: Option<TimeoutConfig>,
) -> Result<()> {
    let name = config.name().to_string();
    debug!("Connecting to VPN: {}", name);

    let nm = NMProxy::new(conn).await?;

    let saved = crate::core::connection_settings::get_saved_connection_path(conn, &name).await?;

    let vpn_device_path = OwnedObjectPath::default();
    let specific_object = OwnedObjectPath::default();

    let active_conn = if let Some(saved_path) = saved {
        trace!("Activating existent VPN connection");
        nm.activate_connection(saved_path, vpn_device_path.clone(), specific_object.clone())
            .await?
    } else {
        trace!("Creating new VPN connection");
        let opts = ConnectionOptions {
            autoconnect: false,
            autoconnect_priority: None,
            autoconnect_retries: None,
        };

        let settings = match config {
            VpnConfiguration::WireGuard(ref wg) => {
                let creds: VpnCredentials = wg.clone().into();
                validate_vpn_credentials(&creds)?;
                build_wireguard_connection(&creds, &opts)?
            }
            VpnConfiguration::OpenVpn(ref ovpn) => {
                validate_openvpn_config(ovpn)?;
                build_openvpn_connection(ovpn, &opts)?
            }
        };

        let settings_api = settings_proxy(conn).await?;

        trace!("Adding connection via Settings API");
        let add_reply = settings_api
            .call_method("AddConnection", &(settings,))
            .await?;
        let conn_path: OwnedObjectPath = add_reply.body().deserialize()?;
        trace!("Connection added, activating VPN connection");

        nm.activate_connection(conn_path, vpn_device_path, specific_object)
            .await?
    };

    let timeout = timeout_config.map(|c| c.connection_timeout);
    wait_for_connection_activation(conn, &active_conn, timeout).await?;
    trace!("Connection reached Activated state, waiting briefly...");

    match NMActiveConnectionProxy::builder(conn).path(active_conn.clone()) {
        Ok(builder) => match builder.build().await {
            Ok(active_conn_check) => {
                let final_state = active_conn_check.state().await?;
                let state = crate::api::models::ActiveConnectionState::from(final_state);
                trace!("Connection state after delay: {:?}", state);

                match state {
                    crate::api::models::ActiveConnectionState::Activated => {
                        info!("Successfully connected to VPN: {}", name);
                        Ok(())
                    }
                    crate::api::models::ActiveConnectionState::Deactivated => {
                        warn!("Connection deactivated immediately after activation");
                        let reason = extract_connection_state_reason(conn, &active_conn).await;
                        Err(crate::api::models::ConnectionError::ActivationFailed(
                            reason,
                        ))
                    }
                    _ => {
                        warn!("Connection in unexpected state: {:?}", state);
                        Err(crate::api::models::ConnectionError::Stuck(format!(
                            "connection in state {:?}",
                            state
                        )))
                    }
                }
            }
            Err(e) => {
                warn!("Failed to build active connection proxy after delay: {}", e);
                let reason = extract_connection_state_reason(conn, &active_conn).await;
                Err(crate::api::models::ConnectionError::ActivationFailed(
                    reason,
                ))
            }
        },
        Err(e) => {
            warn!(
                "Failed to create active connection proxy builder after delay: {}",
                e
            );
            let reason = extract_connection_state_reason(conn, &active_conn).await;
            Err(crate::api::models::ConnectionError::ActivationFailed(
                reason,
            ))
        }
    }
}

/// Disconnects from a VPN connection by name (legacy — prefer `disconnect_vpn_by_uuid`).
pub(crate) async fn disconnect_vpn(conn: &Connection, name: &str) -> Result<()> {
    validate_connection_name(name)?;

    debug!("Disconnecting VPN: {name}");

    let nm = NMProxy::new(conn).await?;
    let active_conns = nm.active_connections().await?;

    for ac_path in active_conns {
        let ac_proxy = match nm_proxy(
            conn,
            ac_path.clone(),
            "org.freedesktop.NetworkManager.Connection.Active",
        )
        .await
        {
            Ok(p) => p,
            Err(_) => continue,
        };

        let conn_path: OwnedObjectPath = match ac_proxy.get_property("Connection").await {
            Ok(p) => p,
            Err(_) => continue,
        };

        let cproxy = match nm_proxy(
            conn,
            conn_path.clone(),
            "org.freedesktop.NetworkManager.Settings.Connection",
        )
        .await
        {
            Ok(p) => p,
            Err(_) => continue,
        };

        let msg = match cproxy.call_method("GetSettings", &()).await {
            Ok(msg) => msg,
            Err(_) => continue,
        };

        let body = msg.body();
        let settings_map: HashMap<String, HashMap<String, zvariant::Value>> =
            match body.deserialize() {
                Ok(map) => map,
                Err(_) => continue,
            };

        let id_match = extract_id(&settings_map)
            .map(|id| id == name)
            .unwrap_or(false);
        let is_vpn = detect_vpn_kind(&settings_map).is_some();

        if id_match && is_vpn {
            trace!("Found active VPN connection, deactivating: {name}");
            nm.deactivate_connection(ac_path.clone()).await?;
            info!("Successfully disconnected VPN: {name}");
            return Ok(());
        }
    }

    info!("Disconnected VPN: {name} (not active)");
    Ok(())
}

/// Forgets (deletes) a saved VPN connection by name.
pub(crate) async fn forget_vpn(conn: &Connection, name: &str) -> Result<()> {
    validate_connection_name(name)?;

    debug!("Starting forget operation for VPN: {name}");

    match disconnect_vpn(conn, name).await {
        Ok(_) => trace!("VPN disconnected before deletion"),
        Err(e) => warn!(
            "Failed to disconnect VPN before deletion (may already be disconnected): {}",
            e
        ),
    }

    let settings = nm_proxy(
        conn,
        "/org/freedesktop/NetworkManager/Settings",
        "org.freedesktop.NetworkManager.Settings",
    )
    .await?;

    let list_reply = settings.call_method("ListConnections", &()).await?;
    let conns: Vec<OwnedObjectPath> = list_reply.body().deserialize()?;

    for cpath in conns {
        let cproxy = match nm_proxy(
            conn,
            cpath.clone(),
            "org.freedesktop.NetworkManager.Settings.Connection",
        )
        .await
        {
            Ok(p) => p,
            Err(_) => continue,
        };

        let msg = match cproxy.call_method("GetSettings", &()).await {
            Ok(msg) => msg,
            Err(_) => continue,
        };

        let body = msg.body();
        let settings_map: HashMap<String, HashMap<String, zvariant::Value>> = body.deserialize()?;

        let id_ok = extract_id(&settings_map)
            .map(|id| id == name)
            .unwrap_or(false);
        let vpn_kind = detect_vpn_kind(&settings_map);

        if id_ok && vpn_kind.is_some() {
            trace!("Found VPN connection, deleting: {name}");
            cproxy.call_method("Delete", &()).await.map_err(|e| {
                ConnectionError::DbusOperation {
                    context: format!("failed to delete VPN connection '{}'", name),
                    source: e,
                }
            })?;
            info!("Successfully deleted VPN connection: {name}");

            if vpn_kind == Some(VpnKind::Plugin)
                && let Err(e) = crate::util::cert_store::cleanup_certs(name)
            {
                warn!("Failed to remove nmrs cert directory for '{}': {}", name, e);
            }
            return Ok(());
        }
    }

    debug!("No saved VPN connection found for '{name}'");
    Ok(())
}

/// Gets detailed information about an active VPN connection.
pub(crate) async fn get_vpn_info(conn: &Connection, name: &str) -> Result<VpnConnectionInfo> {
    validate_connection_name(name)?;

    let nm = NMProxy::new(conn).await?;
    let active_conns = nm.active_connections().await?;

    for ac_path in active_conns {
        let ac_proxy = match nm_proxy(
            conn,
            ac_path.clone(),
            "org.freedesktop.NetworkManager.Connection.Active",
        )
        .await
        {
            Ok(p) => p,
            Err(_) => continue,
        };

        let conn_path: OwnedObjectPath = match ac_proxy.get_property("Connection").await {
            Ok(p) => p,
            Err(_) => continue,
        };

        let cproxy = match nm_proxy(
            conn,
            conn_path.clone(),
            "org.freedesktop.NetworkManager.Settings.Connection",
        )
        .await
        {
            Ok(p) => p,
            Err(_) => continue,
        };

        let msg = match cproxy.call_method("GetSettings", &()).await {
            Ok(m) => m,
            Err(_) => continue,
        };

        let body = msg.body();
        let settings_map: HashMap<String, HashMap<String, zvariant::Value>> =
            match body.deserialize() {
                Ok(m) => m,
                Err(_) => continue,
            };

        let id = match extract_id(&settings_map) {
            Some(i) => i,
            None => continue,
        };

        let Some(kind) = detect_vpn_kind(&settings_map) else {
            continue;
        };

        if id != name {
            continue;
        }

        let state_val: u32 = ac_proxy.get_property("State").await?;
        let state = DeviceState::from(state_val);

        let dev_paths: Vec<OwnedObjectPath> = ac_proxy.get_property("Devices").await?;
        let interface = if let Some(dev_path) = dev_paths.first() {
            let dev_proxy = nm_proxy(
                conn,
                dev_path.clone(),
                "org.freedesktop.NetworkManager.Device",
            )
            .await?;
            Some(dev_proxy.get_property::<String>("Interface").await?)
        } else {
            None
        };

        let gateway = match kind {
            VpnKind::WireGuard => settings_map
                .get("wireguard")
                .and_then(|wg_sec| wg_sec.get("peers"))
                .and_then(|peers| decode_wg_first_peer(peers).1),
            VpnKind::Plugin => extract_openvpn_gateway(&settings_map),
        };

        let ip4_path: OwnedObjectPath = ac_proxy.get_property("Ip4Config").await?;
        let (ip4_address, dns_servers) = if ip4_path.as_str() != "/" {
            let ip4_proxy =
                nm_proxy(conn, ip4_path, "org.freedesktop.NetworkManager.IP4Config").await?;

            let ip4_address = if let Ok(addr_array) = ip4_proxy
                .get_property::<Vec<HashMap<String, zvariant::Value>>>("AddressData")
                .await
            {
                addr_array.first().and_then(|addr_map| {
                    let address = addr_map.get("address").and_then(|v| match v {
                        zvariant::Value::Str(s) => Some(s.as_str().to_string()),
                        _ => None,
                    })?;
                    let prefix = addr_map.get("prefix").and_then(|v| match v {
                        zvariant::Value::U32(p) => Some(p),
                        _ => None,
                    })?;
                    Some(format!("{}/{}", address, prefix))
                })
            } else {
                None
            };

            let dns_servers =
                if let Ok(dns_array) = ip4_proxy.get_property::<Vec<u32>>("Nameservers").await {
                    dns_array
                        .iter()
                        .map(|ip| {
                            format!(
                                "{}.{}.{}.{}",
                                ip & 0xFF,
                                (ip >> 8) & 0xFF,
                                (ip >> 16) & 0xFF,
                                (ip >> 24) & 0xFF
                            )
                        })
                        .collect()
                } else {
                    vec![]
                };

            (ip4_address, dns_servers)
        } else {
            (None, vec![])
        };

        let ip6_path: OwnedObjectPath = ac_proxy.get_property("Ip6Config").await?;
        let ip6_address = if ip6_path.as_str() != "/" {
            let ip6_proxy =
                nm_proxy(conn, ip6_path, "org.freedesktop.NetworkManager.IP6Config").await?;

            if let Ok(addr_array) = ip6_proxy
                .get_property::<Vec<HashMap<String, zvariant::Value>>>("AddressData")
                .await
            {
                addr_array.first().and_then(|addr_map| {
                    let address = addr_map.get("address").and_then(|v| match v {
                        zvariant::Value::Str(s) => Some(s.as_str().to_string()),
                        _ => None,
                    })?;
                    let prefix = addr_map.get("prefix").and_then(|v| match v {
                        zvariant::Value::U32(p) => Some(p),
                        _ => None,
                    })?;
                    Some(format!("{}/{}", address, prefix))
                })
            } else {
                None
            }
        } else {
            None
        };

        let details = match kind {
            VpnKind::WireGuard => extract_wireguard_details(&settings_map),
            VpnKind::Plugin => extract_openvpn_details(&settings_map),
        };

        return Ok(VpnConnectionInfo {
            name: id,
            vpn_kind: kind,
            state,
            interface,
            gateway,
            ip4_address,
            ip6_address,
            dns_servers,
            details,
        });
    }

    Err(crate::api::models::ConnectionError::NoVpnConnection)
}

fn extract_openvpn_gateway(
    settings_map: &HashMap<String, HashMap<String, zvariant::Value<'_>>>,
) -> Option<String> {
    let zvariant::Value::Dict(dict) = settings_map.get("vpn")?.get("data")? else {
        return None;
    };
    dict_str(dict, "remote")
}

fn extract_openvpn_data_value(
    settings_map: &HashMap<String, HashMap<String, zvariant::Value<'_>>>,
    key: &str,
) -> Option<String> {
    let zvariant::Value::Dict(dict) = settings_map.get("vpn")?.get("data")? else {
        return None;
    };
    dict_str(dict, key)
}

fn extract_openvpn_details(
    settings_map: &HashMap<String, HashMap<String, zvariant::Value<'_>>>,
) -> Option<VpnDetails> {
    let remote_raw = extract_openvpn_data_value(settings_map, "remote")?;

    let (remote, port) = parse_openvpn_remote(&remote_raw);

    let protocol =
        if extract_openvpn_data_value(settings_map, "proto-tcp").as_deref() == Some("yes") {
            "tcp".to_string()
        } else {
            "udp".to_string()
        };

    let cipher = extract_openvpn_data_value(settings_map, "cipher");
    let auth = extract_openvpn_data_value(settings_map, "auth");

    let compression = extract_openvpn_data_value(settings_map, "compress")
        .or_else(|| extract_openvpn_data_value(settings_map, "comp-lzo").map(|_| "lzo".into()));

    Some(VpnDetails::OpenVpn {
        remote,
        port,
        protocol,
        cipher,
        auth,
        compression,
    })
}

fn parse_openvpn_remote(remote: &str) -> (String, u16) {
    const DEFAULT_PORT: u16 = 1194;

    if let Some(bracketed) = remote.strip_prefix('[')
        && let Some((host, suffix)) = bracketed.split_once(']')
    {
        let port = suffix
            .strip_prefix(':')
            .and_then(|value| value.parse::<u16>().ok())
            .unwrap_or(DEFAULT_PORT);
        return (host.to_string(), port);
    }

    if remote.matches(':').count() == 1
        && let Some((host, raw_port)) = remote.rsplit_once(':')
    {
        return (
            host.to_string(),
            raw_port.parse::<u16>().unwrap_or(DEFAULT_PORT),
        );
    }

    (remote.to_string(), DEFAULT_PORT)
}

fn extract_wireguard_details(
    settings_map: &HashMap<String, HashMap<String, zvariant::Value<'_>>>,
) -> Option<VpnDetails> {
    let wg_sec = settings_map.get("wireguard")?;

    let (peer_public_key, endpoint, _, _) = wg_sec
        .get("peers")
        .map(decode_wg_first_peer)
        .unwrap_or((None, None, Vec::new(), None));
    let public_key = wg_sec
        .get("public-key")
        .and_then(|value| match unbox_variant(value) {
            zvariant::Value::Str(value) => Some(value.to_string()),
            _ => None,
        })
        .or(peer_public_key);

    Some(VpnDetails::WireGuard {
        public_key,
        endpoint,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    fn openvpn_settings_with_data(
        data: HashMap<String, String>,
    ) -> HashMap<String, HashMap<String, zvariant::Value<'static>>> {
        let dict = zvariant::Dict::from(data);
        let vpn_sec = HashMap::from([("data".to_string(), zvariant::Value::Dict(dict))]);
        HashMap::from([("vpn".to_string(), vpn_sec)])
    }

    fn vpn_settings_with_service(
        service: &str,
        data: HashMap<String, String>,
    ) -> HashMap<String, HashMap<String, zvariant::Value<'static>>> {
        let dict = zvariant::Dict::from(data);
        let vpn_sec = HashMap::from([
            (
                "service-type".to_string(),
                zvariant::Value::Str(service.to_string().into()),
            ),
            ("data".to_string(), zvariant::Value::Dict(dict)),
        ]);
        let conn_sec = HashMap::from([("type".to_string(), zvariant::Value::Str("vpn".into()))]);
        HashMap::from([
            ("vpn".to_string(), vpn_sec),
            ("connection".to_string(), conn_sec),
        ])
    }

    #[test]
    fn detect_wireguard() {
        let conn_sec =
            HashMap::from([("type".to_string(), zvariant::Value::Str("wireguard".into()))]);
        let settings = HashMap::from([("connection".to_string(), conn_sec)]);
        assert_eq!(detect_vpn_kind(&settings), Some(VpnKind::WireGuard));
    }

    #[test]
    fn detect_plugin() {
        let conn_sec = HashMap::from([("type".to_string(), zvariant::Value::Str("vpn".into()))]);
        let settings = HashMap::from([("connection".to_string(), conn_sec)]);
        assert_eq!(detect_vpn_kind(&settings), Some(VpnKind::Plugin));
    }

    #[test]
    fn detect_non_vpn() {
        let conn_sec = HashMap::from([(
            "type".to_string(),
            zvariant::Value::Str("802-11-wireless".into()),
        )]);
        let settings = HashMap::from([("connection".to_string(), conn_sec)]);
        assert_eq!(detect_vpn_kind(&settings), None);
    }

    #[test]
    fn decode_openvpn_full() {
        let data = HashMap::from([
            ("remote".to_string(), "vpn.example.com:1194".to_string()),
            ("connection-type".to_string(), "password-tls".to_string()),
            ("username".to_string(), "alice".to_string()),
            ("ca".to_string(), "/etc/openvpn/ca.crt".to_string()),
            ("password-flags".to_string(), "1".to_string()),
        ]);
        let settings = vpn_settings_with_service("org.freedesktop.NetworkManager.openvpn", data);
        let vt = vpn_type_from_settings(VpnKind::Plugin, &settings);
        match vt {
            VpnType::OpenVpn {
                remote,
                connection_type,
                user_name,
                ca,
                password_flags,
                ..
            } => {
                assert_eq!(remote, Some("vpn.example.com:1194".into()));
                assert_eq!(connection_type, Some(OpenVpnConnectionType::PasswordTls));
                assert_eq!(user_name, Some("alice".into()));
                assert_eq!(ca, Some("/etc/openvpn/ca.crt".into()));
                assert!(password_flags.agent_owned());
            }
            _ => panic!("expected OpenVpn"),
        }
    }

    #[test]
    fn decode_openconnect_prefers_data_username_and_preserves_flags() {
        let data = HashMap::from([
            ("gateway".to_string(), "vpn.example.com".to_string()),
            ("username".to_string(), "data-user".to_string()),
            ("protocol".to_string(), "anyconnect".to_string()),
        ]);
        let mut settings =
            vpn_settings_with_service("org.freedesktop.NetworkManager.openconnect", data);
        settings.get_mut("vpn").unwrap().insert(
            "user-name".into(),
            zvariant::Value::Str("section-user".into()),
        );
        settings
            .get_mut("vpn")
            .unwrap()
            .insert("password-flags".into(), zvariant::Value::U32(1));

        assert!(matches!(
            vpn_type_from_settings(VpnKind::Plugin, &settings),
            VpnType::OpenConnect {
                gateway: Some(ref gateway),
                user_name: Some(ref user),
                protocol: Some(ref protocol),
                password_flags,
            } if gateway == "vpn.example.com"
                && user == "data-user"
                && protocol == "anyconnect"
                && password_flags.agent_owned()
        ));
    }

    #[test]
    fn decode_pptp_falls_back_to_section_username() {
        let data = HashMap::from([("gateway".to_string(), "pptp.example.com".to_string())]);
        let mut settings = vpn_settings_with_service("org.freedesktop.NetworkManager.pptp", data);
        settings.get_mut("vpn").unwrap().insert(
            "user-name".into(),
            zvariant::Value::Str("fallback-user".into()),
        );

        assert!(matches!(
            vpn_type_from_settings(VpnKind::Plugin, &settings),
            VpnType::Pptp {
                gateway: Some(ref gateway),
                user_name: Some(ref user),
                password_flags: VpnSecretFlags(0),
            } if gateway == "pptp.example.com" && user == "fallback-user"
        ));
    }

    #[test]
    fn decode_plugin_without_vpn_section_is_empty_generic() {
        let settings = HashMap::new();
        assert!(matches!(
            vpn_type_from_settings(VpnKind::Plugin, &settings),
            VpnType::Generic {
                ref service_type,
                ref data,
                ref secrets,
                user_name: None,
                password_flags: VpnSecretFlags(0),
            } if service_type.is_empty() && data.is_empty() && secrets.is_empty()
        ));
    }

    #[test]
    fn malformed_plugin_sections_fall_back_without_panicking() {
        let vpn = HashMap::from([
            ("service-type".into(), zvariant::Value::U32(7)),
            ("data".into(), zvariant::Value::Str("not-a-dict".into())),
            ("secrets".into(), zvariant::Value::U32(9)),
            ("user-name".into(), zvariant::Value::Str("".into())),
            ("password-flags".into(), zvariant::Value::Str("bad".into())),
        ]);
        let settings = HashMap::from([("vpn".into(), vpn)]);

        assert!(matches!(
            vpn_type_from_settings(VpnKind::Plugin, &settings),
            VpnType::Generic {
                ref service_type,
                ref data,
                ref secrets,
                user_name: None,
                password_flags: VpnSecretFlags(0),
            } if service_type.is_empty() && data.is_empty() && secrets.is_empty()
        ));
    }

    #[test]
    fn decode_strongswan() {
        let data = HashMap::from([
            ("address".to_string(), "ipsec.corp.com".to_string()),
            ("method".to_string(), "eap".to_string()),
            ("user".to_string(), "bob".to_string()),
        ]);
        let settings = vpn_settings_with_service("org.freedesktop.NetworkManager.strongswan", data);
        let vt = vpn_type_from_settings(VpnKind::Plugin, &settings);
        match vt {
            VpnType::StrongSwan {
                address,
                method,
                user_name,
                ..
            } => {
                assert_eq!(address, Some("ipsec.corp.com".into()));
                assert_eq!(method, Some("eap".into()));
                assert_eq!(user_name, Some("bob".into()));
            }
            _ => panic!("expected StrongSwan"),
        }
    }

    #[test]
    fn decode_l2tp_with_ipsec() {
        let data = HashMap::from([
            ("gateway".to_string(), "l2tp.example.com".to_string()),
            ("ipsec-enabled".to_string(), "yes".to_string()),
        ]);
        let settings = vpn_settings_with_service("org.freedesktop.NetworkManager.l2tp", data);
        let vt = vpn_type_from_settings(VpnKind::Plugin, &settings);
        match vt {
            VpnType::L2tp {
                gateway,
                ipsec_enabled,
                ..
            } => {
                assert_eq!(gateway, Some("l2tp.example.com".into()));
                assert!(ipsec_enabled);
            }
            _ => panic!("expected L2tp"),
        }
    }

    #[test]
    fn decode_generic_unknown_plugin() {
        let data = HashMap::from([("server".to_string(), "my.server.com".to_string())]);
        let settings =
            vpn_settings_with_service("org.freedesktop.NetworkManager.my-custom-vpn", data);
        let vt = vpn_type_from_settings(VpnKind::Plugin, &settings);
        match vt {
            VpnType::Generic {
                service_type, data, ..
            } => {
                assert_eq!(service_type, "org.freedesktop.NetworkManager.my-custom-vpn");
                assert_eq!(data.get("server").unwrap(), "my.server.com");
            }
            _ => panic!("expected Generic"),
        }
    }

    #[test]
    fn openvpn_connection_type_roundtrip() {
        for (s, expected) in [
            ("tls", OpenVpnConnectionType::Tls),
            ("static-key", OpenVpnConnectionType::StaticKey),
            ("password", OpenVpnConnectionType::Password),
            ("password-tls", OpenVpnConnectionType::PasswordTls),
        ] {
            assert_eq!(OpenVpnConnectionType::from_nm_str(s), Some(expected));
        }
        assert_eq!(OpenVpnConnectionType::from_nm_str("bogus"), None);
    }

    #[test]
    fn vpn_secret_flags_roundtrip() {
        let f = VpnSecretFlags(0x3);
        assert!(f.agent_owned());
        assert_eq!(f.0 & 0x2, 0x2); // NOT_SAVED
    }

    #[test]
    fn openvpn_gateway_extracted_from_vpn_data() {
        let data = HashMap::from([("remote".to_string(), "vpn.example.com:1194".to_string())]);
        let settings = openvpn_settings_with_data(data);
        assert_eq!(
            extract_openvpn_gateway(&settings),
            Some("vpn.example.com:1194".to_string())
        );
    }

    #[test]
    fn openvpn_gateway_none_when_remote_key_absent() {
        let data = HashMap::from([("dev".to_string(), "tun".to_string())]);
        let settings = openvpn_settings_with_data(data);
        assert_eq!(extract_openvpn_gateway(&settings), None);
    }

    #[test]
    fn openvpn_gateway_none_when_vpn_section_absent() {
        let settings: HashMap<String, HashMap<String, zvariant::Value<'static>>> =
            HashMap::from([("connection".to_string(), HashMap::new())]);
        assert_eq!(extract_openvpn_gateway(&settings), None);
    }

    #[test]
    fn openvpn_details_full() {
        let data = HashMap::from([
            ("remote".to_string(), "vpn.example.com:1194".to_string()),
            ("proto-tcp".to_string(), "yes".to_string()),
            ("cipher".to_string(), "AES-256-GCM".to_string()),
            ("auth".to_string(), "SHA256".to_string()),
            ("compress".to_string(), "lz4-v2".to_string()),
        ]);
        let settings = openvpn_settings_with_data(data);
        let details = extract_openvpn_details(&settings).unwrap();
        match details {
            VpnDetails::OpenVpn {
                remote,
                port,
                protocol,
                cipher,
                auth,
                compression,
            } => {
                assert_eq!(remote, "vpn.example.com");
                assert_eq!(port, 1194);
                assert_eq!(protocol, "tcp");
                assert_eq!(cipher, Some("AES-256-GCM".into()));
                assert_eq!(auth, Some("SHA256".into()));
                assert_eq!(compression, Some("lz4-v2".into()));
            }
            _ => panic!("expected OpenVpn variant"),
        }
    }

    #[test]
    fn openvpn_details_minimal() {
        let data = HashMap::from([("remote".to_string(), "vpn.example.com:443".to_string())]);
        let settings = openvpn_settings_with_data(data);
        let details = extract_openvpn_details(&settings).unwrap();
        match details {
            VpnDetails::OpenVpn {
                remote,
                port,
                protocol,
                cipher,
                auth,
                compression,
            } => {
                assert_eq!(remote, "vpn.example.com");
                assert_eq!(port, 443);
                assert_eq!(protocol, "udp");
                assert!(cipher.is_none());
                assert!(auth.is_none());
                assert!(compression.is_none());
            }
            _ => panic!("expected OpenVpn variant"),
        }
    }

    #[test]
    fn openvpn_remote_parser_handles_hosts_ports_and_ipv6() {
        assert_eq!(
            parse_openvpn_remote("vpn.example.com:443"),
            ("vpn.example.com".into(), 443)
        );
        assert_eq!(
            parse_openvpn_remote("vpn.example.com"),
            ("vpn.example.com".into(), 1194)
        );
        assert_eq!(
            parse_openvpn_remote("vpn.example.com:not-a-port"),
            ("vpn.example.com".into(), 1194)
        );
        assert_eq!(
            parse_openvpn_remote("vpn.example.com:70000"),
            ("vpn.example.com".into(), 1194)
        );
        assert_eq!(
            parse_openvpn_remote("[2001:db8::1]:443"),
            ("2001:db8::1".into(), 443)
        );
        assert_eq!(
            parse_openvpn_remote("2001:db8::1"),
            ("2001:db8::1".into(), 1194)
        );
    }

    #[test]
    fn openvpn_details_uses_comp_lzo_fallback() {
        let settings = openvpn_settings_with_data(HashMap::from([
            ("remote".to_string(), "vpn.example.com".to_string()),
            ("comp-lzo".to_string(), "yes".to_string()),
        ]));
        assert!(matches!(
            extract_openvpn_details(&settings),
            Some(VpnDetails::OpenVpn {
                port: 1194,
                compression: Some(ref compression),
                ..
            }) if compression == "lzo"
        ));
    }

    fn wireguard_settings(
        pairs: Vec<(&str, zvariant::Value<'static>)>,
    ) -> HashMap<String, HashMap<String, zvariant::Value<'static>>> {
        let wg_sec: HashMap<String, zvariant::Value<'static>> =
            pairs.into_iter().map(|(k, v)| (k.to_string(), v)).collect();
        HashMap::from([("wireguard".to_string(), wg_sec)])
    }

    #[test]
    fn wireguard_details_full() {
        let settings = wireguard_settings(vec![
            (
                "public-key",
                zvariant::Value::Str("YBk6X3pP8KjKz7+HFWzVHNqL3qTZq8hX9VxFQJ4zVmM=".into()),
            ),
            (
                "peers",
                zvariant::Value::Str("endpoint=vpn.example.com:51820 allowed-ips=0.0.0.0/0".into()),
            ),
        ]);
        let details = extract_wireguard_details(&settings).unwrap();
        match details {
            VpnDetails::WireGuard {
                public_key,
                endpoint,
            } => {
                assert_eq!(
                    public_key,
                    Some("YBk6X3pP8KjKz7+HFWzVHNqL3qTZq8hX9VxFQJ4zVmM=".into())
                );
                assert_eq!(endpoint, Some("vpn.example.com:51820".into()));
            }
            _ => panic!("expected WireGuard variant"),
        }
    }

    #[test]
    fn decode_wireguard_string_peer_representation() {
        let settings = wireguard_settings(vec![
            (
                "private-key",
                zvariant::Value::Str("private-key-material".into()),
            ),
            (
                "peers",
                zvariant::Value::Str(
                    "public-key=peer-key endpoint=vpn.example.com:51820 \
                     allowed-ips=0.0.0.0/0;::/0 persistent-keepalive=25, \
                     public-key=second"
                        .into(),
                ),
            ),
        ]);

        assert!(matches!(
            vpn_type_from_settings(VpnKind::WireGuard, &settings),
            VpnType::WireGuard {
                private_key: Some(ref private_key),
                peer_public_key: Some(ref public_key),
                endpoint: Some(ref endpoint),
                ref allowed_ips,
                persistent_keepalive: Some(25),
            } if private_key == "private-key-material"
                && public_key == "peer-key"
                && endpoint == "vpn.example.com:51820"
                && allowed_ips == &["0.0.0.0/0".to_string(), "::/0".to_string()]
        ));
    }

    #[test]
    fn decode_wireguard_native_array_peer_representation() {
        let peer: HashMap<String, zvariant::Value<'static>> = HashMap::from([
            ("public-key".into(), zvariant::Value::Str("peer-key".into())),
            (
                "endpoint".into(),
                zvariant::Value::Str("vpn.example.com:51820".into()),
            ),
            (
                "allowed-ips".into(),
                zvariant::Value::from(vec!["10.0.0.0/8".to_string(), "::/0".to_string()]),
            ),
            ("persistent-keepalive".into(), zvariant::Value::U32(30)),
        ]);
        let settings = wireguard_settings(vec![("peers", zvariant::Value::from(vec![peer]))]);

        assert!(matches!(
            vpn_type_from_settings(VpnKind::WireGuard, &settings),
            VpnType::WireGuard {
                private_key: None,
                peer_public_key: Some(ref public_key),
                endpoint: Some(ref endpoint),
                ref allowed_ips,
                persistent_keepalive: Some(30),
            } if public_key == "peer-key"
                && endpoint == "vpn.example.com:51820"
                && allowed_ips == &["10.0.0.0/8".to_string(), "::/0".to_string()]
        ));

        assert!(matches!(
            extract_wireguard_details(&settings),
            Some(VpnDetails::WireGuard {
                public_key: Some(ref public_key),
                endpoint: Some(ref endpoint),
            }) if public_key == "peer-key" && endpoint == "vpn.example.com:51820"
        ));
    }

    #[test]
    fn malformed_wireguard_fields_return_empty_details() {
        let settings = wireguard_settings(vec![
            ("private-key", zvariant::Value::U32(1)),
            ("peers", zvariant::Value::U32(2)),
        ]);
        assert_eq!(
            vpn_type_from_settings(VpnKind::WireGuard, &settings),
            VpnType::WireGuard {
                private_key: None,
                peer_public_key: None,
                endpoint: None,
                allowed_ips: Vec::new(),
                persistent_keepalive: None,
            }
        );
    }
}
