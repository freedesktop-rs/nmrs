//! Decode and manage NetworkManager saved connection settings.

use std::collections::HashMap;

use futures::stream::{self, StreamExt};
use log::warn;
use zbus::Connection;
use zvariant::{OwnedObjectPath, OwnedValue, Str};

use crate::Result;
use crate::api::models::{
    ConnectionError, SavedConnection, SavedConnectionBrief, SettingsPatch, SettingsSummary,
    VpnSecretFlags, WifiKeyMgmt, WifiSecuritySummary,
};
use crate::dbus::{NMSettingsConnectionProxy, NMSettingsProxy};
use crate::util::utils::decode_ssid_or_empty;

/// Builds the `a{sa{sv}}` delta for [`SettingsPatch`] (unit-tested).
pub(crate) fn build_settings_patch_delta(
    patch: &SettingsPatch,
) -> HashMap<String, HashMap<String, OwnedValue>> {
    let mut delta: HashMap<String, HashMap<String, OwnedValue>> = HashMap::new();

    if let Some(v) = patch.autoconnect {
        delta
            .entry("connection".to_string())
            .or_default()
            .insert("autoconnect".to_string(), OwnedValue::from(v));
    }
    if let Some(v) = patch.autoconnect_priority {
        delta
            .entry("connection".to_string())
            .or_default()
            .insert("autoconnect-priority".to_string(), OwnedValue::from(v));
    }
    if let Some(ref s) = patch.id {
        delta
            .entry("connection".to_string())
            .or_default()
            .insert("id".to_string(), OwnedValue::from(Str::from(s.as_str())));
    }
    if let Some(opt) = &patch.interface_name {
        let v = match opt {
            Some(name) => OwnedValue::from(Str::from(name.as_str())),
            None => OwnedValue::from(Str::from("")),
        };
        delta
            .entry("connection".to_string())
            .or_default()
            .insert("interface-name".to_string(), v);
    }
    if let Some(ref overlay) = patch.raw_overlay {
        for (sec, entries) in overlay {
            let e = delta.entry(sec.clone()).or_default();
            for (k, v) in entries {
                e.insert(k.clone(), v.clone());
            }
        }
    }

    delta
}

fn merge_settings_patch_delta(
    settings: &mut HashMap<String, HashMap<String, OwnedValue>>,
    delta: HashMap<String, HashMap<String, OwnedValue>>,
) {
    for (section, entries) in delta {
        let target = settings.entry(section).or_default();
        for (key, value) in entries {
            target.insert(key, value);
        }
    }
}

fn owned_to_str(v: &OwnedValue) -> Option<String> {
    Str::try_from(v.clone())
        .ok()
        .map(|s| s.to_string())
        .or_else(|| String::try_from(v.clone()).ok())
}

fn owned_to_bool(v: &OwnedValue) -> Option<bool> {
    bool::try_from(v.clone()).ok()
}

fn owned_to_u32(v: &OwnedValue) -> Option<u32> {
    u32::try_from(v.clone()).ok()
}

fn owned_to_i32(v: &OwnedValue) -> Option<i32> {
    i32::try_from(v.clone()).ok()
}

fn owned_to_u64(v: &OwnedValue) -> Option<u64> {
    u64::try_from(v.clone()).ok()
}

fn owned_to_bytes(v: &OwnedValue) -> Option<Vec<u8>> {
    Vec::<u8>::try_from(v.clone()).ok()
}

fn unbox_value<'a, 'v>(mut value: &'a zvariant::Value<'v>) -> &'a zvariant::Value<'v> {
    while let zvariant::Value::Value(inner) = value {
        value = inner;
    }
    value
}

fn take_str(m: &HashMap<String, OwnedValue>, key: &str) -> Option<String> {
    m.get(key).and_then(owned_to_str)
}

fn take_bool(m: &HashMap<String, OwnedValue>, key: &str) -> Option<bool> {
    m.get(key).and_then(owned_to_bool)
}

fn take_u32(m: &HashMap<String, OwnedValue>, key: &str) -> Option<u32> {
    m.get(key).and_then(owned_to_u32)
}

fn take_i32(m: &HashMap<String, OwnedValue>, key: &str) -> Option<i32> {
    m.get(key).and_then(owned_to_i32)
}

fn take_u64(m: &HashMap<String, OwnedValue>, key: &str) -> Option<u64> {
    m.get(key).and_then(owned_to_u64)
}

fn take_str_vec(m: &HashMap<String, OwnedValue>, key: &str) -> Vec<String> {
    let Some(v) = m.get(key) else {
        return Vec::new();
    };
    let Ok(arr) = zvariant::Array::try_from(v.clone()) else {
        return Vec::new();
    };
    let mut out = Vec::new();
    for item in arr.iter() {
        if let zvariant::Value::Str(value) = unbox_value(item) {
            out.push(value.to_string());
        }
    }
    out
}

/// Decodes a full [`SavedConnection`] from `GetSettings` output.
pub(crate) fn decode_saved(
    path: OwnedObjectPath,
    unsaved: bool,
    filename: Option<String>,
    settings: HashMap<String, HashMap<String, OwnedValue>>,
) -> Result<SavedConnection> {
    let Some(conn) = settings.get("connection") else {
        return Err(ConnectionError::MalformedSavedConnection(
            "missing 'connection' section".into(),
        ));
    };

    let uuid = take_str(conn, "uuid").ok_or_else(|| {
        ConnectionError::MalformedSavedConnection("missing connection.uuid".into())
    })?;
    let id = take_str(conn, "id")
        .ok_or_else(|| ConnectionError::MalformedSavedConnection("missing connection.id".into()))?;
    let connection_type = take_str(conn, "type").ok_or_else(|| {
        ConnectionError::MalformedSavedConnection("missing connection.type".into())
    })?;

    let interface_name = take_str(conn, "interface-name").filter(|s| !s.is_empty());
    let autoconnect = take_bool(conn, "autoconnect").unwrap_or(true);
    let autoconnect_priority = take_i32(conn, "autoconnect-priority").unwrap_or(0);
    let timestamp_unix = take_u64(conn, "timestamp").unwrap_or(0);
    let permissions = take_str_vec(conn, "permissions");

    let summary = decode_summary(&connection_type, &settings);

    Ok(SavedConnection {
        path,
        uuid,
        id,
        connection_type,
        interface_name,
        autoconnect,
        autoconnect_priority,
        timestamp_unix,
        permissions,
        unsaved,
        filename,
        summary,
    })
}

/// Brief row without building [`SettingsSummary`].
pub(crate) fn decode_saved_brief(
    path: OwnedObjectPath,
    settings: &HashMap<String, HashMap<String, OwnedValue>>,
) -> Result<SavedConnectionBrief> {
    let Some(conn) = settings.get("connection") else {
        return Err(ConnectionError::MalformedSavedConnection(
            "missing 'connection' section".into(),
        ));
    };
    let uuid = take_str(conn, "uuid").ok_or_else(|| {
        ConnectionError::MalformedSavedConnection("missing connection.uuid".into())
    })?;
    let id = take_str(conn, "id")
        .ok_or_else(|| ConnectionError::MalformedSavedConnection("missing connection.id".into()))?;
    let connection_type = take_str(conn, "type").ok_or_else(|| {
        ConnectionError::MalformedSavedConnection("missing connection.type".into())
    })?;

    Ok(SavedConnectionBrief {
        path,
        uuid,
        id,
        connection_type,
    })
}

fn decode_summary(
    conn_type: &str,
    settings: &HashMap<String, HashMap<String, OwnedValue>>,
) -> SettingsSummary {
    match conn_type {
        "802-11-wireless" => decode_wifi(settings),
        "802-3-ethernet" => decode_ethernet(settings),
        "wireguard" => decode_wireguard(settings),
        "vpn" => {
            if is_wireguard_vpn_service(settings) {
                decode_wireguard(settings)
            } else {
                decode_vpn(settings)
            }
        }
        "gsm" => decode_gsm(settings),
        "cdma" => decode_cdma(settings),
        "bluetooth" => decode_bluetooth(settings),
        _ => SettingsSummary::Other {
            sections: settings.keys().cloned().collect(),
        },
    }
}

fn is_wireguard_vpn_service(settings: &HashMap<String, HashMap<String, OwnedValue>>) -> bool {
    let Some(vpn) = settings.get("vpn") else {
        return false;
    };
    let Some(st) = take_str(vpn, "service-type") else {
        return false;
    };
    st.contains("wireguard")
}

fn decode_wifi(settings: &HashMap<String, HashMap<String, OwnedValue>>) -> SettingsSummary {
    let w = settings.get("802-11-wireless").cloned().unwrap_or_default();
    let ssid_bytes = w.get("ssid").and_then(owned_to_bytes).unwrap_or_default();
    let ssid = decode_ssid_or_empty(&ssid_bytes).into_owned();
    let mode = take_str(&w, "mode");
    let band = take_str(&w, "band");
    let channel = take_u32(&w, "channel");
    let bssid = take_str(&w, "bssid");
    let hidden = take_bool(&w, "hidden").unwrap_or(false);
    let mac_randomization = take_str(&w, "mac-address-randomization");

    let has_sec_key = w
        .get("security")
        .map(|v| owned_to_str(v).is_some())
        .unwrap_or(false);
    let security = if has_sec_key
        || settings.contains_key("802-11-wireless-security")
        || settings.contains_key("802-1x")
    {
        Some(decode_wifi_security(settings))
    } else {
        None
    };

    SettingsSummary::Wifi {
        ssid,
        mode,
        security,
        band,
        channel,
        bssid,
        hidden,
        mac_randomization,
    }
}

fn decode_wifi_security(
    settings: &HashMap<String, HashMap<String, OwnedValue>>,
) -> WifiSecuritySummary {
    let ws = settings
        .get("802-11-wireless-security")
        .cloned()
        .unwrap_or_default();
    let eap = settings.get("802-1x").cloned().unwrap_or_default();

    let key_mgmt_str = take_str(&ws, "key-mgmt").unwrap_or_default();
    let key_mgmt = match key_mgmt_str.as_str() {
        "none" => WifiKeyMgmt::None,
        "" if !eap.is_empty() => WifiKeyMgmt::WpaEap,
        "" => WifiKeyMgmt::None,
        "ieee8021x" => WifiKeyMgmt::WpaEap,
        "wpa-none" => WifiKeyMgmt::Wep,
        "wpa-psk" | "wpa-psk-sha256" => WifiKeyMgmt::WpaPsk,
        "wpa-eap" | "wpa-eap-suite-b-192" | "wpa-eap-sha256" => WifiKeyMgmt::WpaEap,
        "sae" | "sae-ext" => WifiKeyMgmt::Sae,
        "owe" => WifiKeyMgmt::Owe,
        "owe-transition-mode" => WifiKeyMgmt::OweTransitionMode,
        s if s.contains("wep") => WifiKeyMgmt::Wep,
        _ if !eap.is_empty() => WifiKeyMgmt::WpaEap,
        _ => WifiKeyMgmt::None,
    };

    let has_psk_field = ws.contains_key("psk");
    let psk_flags = take_u32(&ws, "psk-flags").unwrap_or(0);
    let psk_agent_owned = VpnSecretFlags(psk_flags).agent_owned();

    let eap_methods = take_str_vec(&eap, "eap");

    WifiSecuritySummary {
        key_mgmt,
        has_psk_field,
        psk_agent_owned,
        eap_methods,
    }
}

fn decode_ethernet(settings: &HashMap<String, HashMap<String, OwnedValue>>) -> SettingsSummary {
    let e = settings.get("802-3-ethernet").cloned().unwrap_or_default();
    SettingsSummary::Ethernet {
        mac_address: take_str(&e, "mac-address"),
        auto_negotiate: take_bool(&e, "auto-negotiate"),
        speed_mbps: take_u32(&e, "speed"),
        mtu: take_u32(&e, "mtu"),
    }
}

fn decode_vpn(settings: &HashMap<String, HashMap<String, OwnedValue>>) -> SettingsSummary {
    let v = settings.get("vpn").cloned().unwrap_or_default();
    let service_type = take_str(&v, "service-type").unwrap_or_default();
    let user_name = take_str(&v, "user-name");
    let password_flags = VpnSecretFlags(take_u32(&v, "password-flags").unwrap_or(0));
    let persistent = take_bool(&v, "persistent").unwrap_or(false);

    let mut data_keys = Vec::new();
    if let Some(data_v) = v.get("data")
        && let Ok(dict) = zvariant::Dict::try_from(data_v.clone())
    {
        for (k, _) in dict.iter() {
            if let Ok(key) = Str::try_from(k.clone()) {
                data_keys.push(key.to_string());
            }
        }
    }
    data_keys.sort();

    SettingsSummary::Vpn {
        service_type,
        user_name,
        password_flags,
        data_keys,
        persistent,
    }
}

fn decode_wireguard(settings: &HashMap<String, HashMap<String, OwnedValue>>) -> SettingsSummary {
    let wg = settings.get("wireguard").cloned().unwrap_or_default();
    let listen_port = take_u32(&wg, "listen-port").and_then(|p| u16::try_from(p).ok());
    let mtu = take_u32(&wg, "mtu");
    let fwmark = take_u32(&wg, "fwmark");

    let mut peer_count = 0usize;
    let mut first_peer_endpoint = None;

    if let Some(peers_v) = wg.get("peers")
        && let Ok(arr) = zvariant::Array::try_from(peers_v.clone())
    {
        peer_count = arr.len();
        if let Some(first) = arr.iter().next()
            && let zvariant::Value::Dict(dict) = unbox_value(first)
        {
            for (k, val) in dict.iter() {
                if let zvariant::Value::Str(key) = unbox_value(k)
                    && key.as_str() == "endpoint"
                    && let zvariant::Value::Str(endpoint) = unbox_value(val)
                {
                    first_peer_endpoint = Some(endpoint.to_string());
                    break;
                }
            }
        }
    }

    SettingsSummary::WireGuard {
        listen_port,
        mtu,
        fwmark,
        peer_count,
        first_peer_endpoint,
    }
}

fn decode_gsm(settings: &HashMap<String, HashMap<String, OwnedValue>>) -> SettingsSummary {
    let g = settings.get("gsm").cloned().unwrap_or_default();
    SettingsSummary::Gsm {
        apn: take_str(&g, "apn"),
        user_name: take_str(&g, "username"),
        password_flags: take_u32(&g, "password-flags").unwrap_or(0),
        pin_flags: take_u32(&g, "pin-flags").unwrap_or(0),
    }
}

fn decode_cdma(settings: &HashMap<String, HashMap<String, OwnedValue>>) -> SettingsSummary {
    let c = settings.get("cdma").cloned().unwrap_or_default();
    SettingsSummary::Cdma {
        number: take_str(&c, "number"),
        user_name: take_str(&c, "username"),
        password_flags: take_u32(&c, "password-flags").unwrap_or(0),
    }
}

fn decode_bluetooth(settings: &HashMap<String, HashMap<String, OwnedValue>>) -> SettingsSummary {
    let b = settings.get("bluetooth").cloned().unwrap_or_default();
    let bdaddr = take_str(&b, "bdaddr").unwrap_or_default();
    let bt_type = take_str(&b, "type").unwrap_or_else(|| "panu".into());
    SettingsSummary::Bluetooth { bdaddr, bt_type }
}

async fn fetch_one_full(
    conn: &Connection,
    path: OwnedObjectPath,
) -> Result<Option<SavedConnection>> {
    let proxy = match NMSettingsConnectionProxy::builder(conn)
        .path(path.clone())
        .map_err(ConnectionError::Dbus)?
        .build()
        .await
    {
        Ok(p) => p,
        Err(e) => {
            warn!(
                "saved connection {}: failed to build proxy: {}",
                path.as_str(),
                e
            );
            return Ok(None);
        }
    };

    let unsaved = proxy.unsaved().await.unwrap_or(false);
    let filename = proxy.filename().await.ok().filter(|s| !s.is_empty());

    let settings = match proxy.get_settings().await {
        Ok(s) => s,
        Err(e) => {
            warn!(
                "saved connection {}: GetSettings failed: {}",
                path.as_str(),
                e
            );
            return Ok(None);
        }
    };

    let path_str = path.as_str().to_string();
    match decode_saved(path, unsaved, filename, settings) {
        Ok(c) => Ok(Some(c)),
        Err(ConnectionError::MalformedSavedConnection(msg)) => {
            warn!(
                "skipping malformed saved connection at {}: {}",
                path_str, msg
            );
            Ok(None)
        }
        Err(e) => Err(e),
    }
}

async fn fetch_one_brief(
    conn: &Connection,
    path: OwnedObjectPath,
) -> Result<Option<SavedConnectionBrief>> {
    let proxy = match NMSettingsConnectionProxy::builder(conn)
        .path(path.clone())
        .map_err(ConnectionError::Dbus)?
        .build()
        .await
    {
        Ok(p) => p,
        Err(e) => {
            warn!(
                "saved connection {}: failed to build proxy: {}",
                path.as_str(),
                e
            );
            return Ok(None);
        }
    };

    let settings = match proxy.get_settings().await {
        Ok(s) => s,
        Err(e) => {
            warn!(
                "saved connection {}: GetSettings failed: {}",
                path.as_str(),
                e
            );
            return Ok(None);
        }
    };

    let path_str = path.as_str().to_string();
    match decode_saved_brief(path, &settings) {
        Ok(b) => Ok(Some(b)),
        Err(ConnectionError::MalformedSavedConnection(msg)) => {
            warn!(
                "skipping malformed saved connection at {}: {}",
                path_str, msg
            );
            Ok(None)
        }
        Err(e) => Err(e),
    }
}

/// Lists all saved profiles with full summaries (bounded concurrency).
pub(crate) async fn list_saved_connections(conn: &Connection) -> Result<Vec<SavedConnection>> {
    const IN_FLIGHT: usize = 16;

    let settings =
        NMSettingsProxy::new(conn)
            .await
            .map_err(|e| ConnectionError::DbusOperation {
                context: "failed to create NM Settings proxy".into(),
                source: e,
            })?;

    let paths = settings
        .list_connections()
        .await
        .map_err(|e| ConnectionError::DbusOperation {
            context: "failed to list saved connections".into(),
            source: e,
        })?;

    let conn = conn.clone();
    let mut out: Vec<SavedConnection> = stream::iter(paths)
        .map(|path| {
            let conn = conn.clone();
            async move { fetch_one_full(&conn, path).await }
        })
        .buffer_unordered(IN_FLIGHT)
        .filter_map(|r| async move {
            match r {
                Ok(Some(c)) => Some(c),
                Ok(None) => None,
                Err(e) => {
                    warn!("list_saved_connections: {e}");
                    None
                }
            }
        })
        .collect()
        .await;

    out.sort_by(|a, b| a.id.cmp(&b.id));
    Ok(out)
}

/// Lists saved profiles with only `connection` identity fields.
pub(crate) async fn list_saved_connections_brief(
    conn: &Connection,
) -> Result<Vec<SavedConnectionBrief>> {
    const IN_FLIGHT: usize = 16;

    let settings =
        NMSettingsProxy::new(conn)
            .await
            .map_err(|e| ConnectionError::DbusOperation {
                context: "failed to create NM Settings proxy".into(),
                source: e,
            })?;

    let paths = settings
        .list_connections()
        .await
        .map_err(|e| ConnectionError::DbusOperation {
            context: "failed to list saved connections".into(),
            source: e,
        })?;

    let conn = conn.clone();
    let mut out: Vec<SavedConnectionBrief> = stream::iter(paths)
        .map(|path| {
            let conn = conn.clone();
            async move { fetch_one_brief(&conn, path).await }
        })
        .buffer_unordered(IN_FLIGHT)
        .filter_map(|r| async move {
            match r {
                Ok(Some(c)) => Some(c),
                Ok(None) => None,
                Err(e) => {
                    warn!("list_saved_connections_brief: {e}");
                    None
                }
            }
        })
        .collect()
        .await;

    out.sort_by(|a, b| a.id.cmp(&b.id));
    Ok(out)
}

pub(crate) async fn resolve_saved_path_by_uuid(
    conn: &Connection,
    uuid: &str,
) -> Result<OwnedObjectPath> {
    let settings =
        NMSettingsProxy::new(conn)
            .await
            .map_err(|e| ConnectionError::DbusOperation {
                context: "failed to create NM Settings proxy".into(),
                source: e,
            })?;

    settings
        .get_connection_by_uuid(uuid)
        .await
        .map_err(|_| ConnectionError::SavedConnectionNotFound(uuid.to_string()))
}

pub(crate) async fn get_saved_connection(conn: &Connection, uuid: &str) -> Result<SavedConnection> {
    let path = resolve_saved_path_by_uuid(conn, uuid).await?;
    fetch_one_full(conn, path)
        .await?
        .ok_or_else(|| ConnectionError::MalformedSavedConnection(uuid.to_string()))
}

pub(crate) async fn get_saved_connection_raw(
    conn: &Connection,
    uuid: &str,
) -> Result<HashMap<String, HashMap<String, OwnedValue>>> {
    let path = resolve_saved_path_by_uuid(conn, uuid).await?;
    let proxy = NMSettingsConnectionProxy::builder(conn)
        .path(path)
        .map_err(ConnectionError::Dbus)?
        .build()
        .await
        .map_err(|e| ConnectionError::DbusOperation {
            context: "failed to build Settings.Connection proxy".into(),
            source: e,
        })?;

    proxy
        .get_settings()
        .await
        .map_err(|e| ConnectionError::DbusOperation {
            context: "GetSettings failed".into(),
            source: e,
        })
}

pub(crate) async fn delete_saved_connection(conn: &Connection, uuid: &str) -> Result<()> {
    let path = resolve_saved_path_by_uuid(conn, uuid).await?;
    let proxy = NMSettingsConnectionProxy::builder(conn)
        .path(path)
        .map_err(ConnectionError::Dbus)?
        .build()
        .await
        .map_err(|e| ConnectionError::DbusOperation {
            context: "failed to build Settings.Connection proxy".into(),
            source: e,
        })?;

    proxy
        .delete()
        .await
        .map_err(|e| ConnectionError::DbusOperation {
            context: "Delete failed".into(),
            source: e,
        })
}

pub(crate) async fn update_saved_connection(
    conn: &Connection,
    uuid: &str,
    patch: &SettingsPatch,
) -> Result<()> {
    let path = resolve_saved_path_by_uuid(conn, uuid).await?;
    let proxy = NMSettingsConnectionProxy::builder(conn)
        .path(path)
        .map_err(ConnectionError::Dbus)?
        .build()
        .await
        .map_err(|e| ConnectionError::DbusOperation {
            context: "failed to build Settings.Connection proxy".into(),
            source: e,
        })?;

    let delta = build_settings_patch_delta(patch);
    if delta.is_empty() {
        return Ok(());
    }

    let mut settings = proxy
        .get_settings()
        .await
        .map_err(|e| ConnectionError::DbusOperation {
            context: "GetSettings failed before update".into(),
            source: e,
        })?;
    merge_settings_patch_delta(&mut settings, delta);

    let unsaved = proxy
        .unsaved()
        .await
        .map_err(|e| ConnectionError::DbusOperation {
            context: "read unsaved property".into(),
            source: e,
        })?;

    if unsaved {
        proxy
            .update_unsaved(settings)
            .await
            .map_err(|e| ConnectionError::DbusOperation {
                context: "UpdateUnsaved failed".into(),
                source: e,
            })?;
    } else {
        proxy
            .update(settings)
            .await
            .map_err(|e| ConnectionError::DbusOperation {
                context: "Update failed".into(),
                source: e,
            })?;
    }

    Ok(())
}

pub(crate) async fn reload_saved_connections(conn: &Connection) -> Result<()> {
    let settings =
        NMSettingsProxy::new(conn)
            .await
            .map_err(|e| ConnectionError::DbusOperation {
                context: "failed to create NM Settings proxy".into(),
                source: e,
            })?;

    let _ok = settings
        .reload_connections()
        .await
        .map_err(|e| ConnectionError::DbusOperation {
            context: "ReloadConnections failed".into(),
            source: e,
        })?;

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use zvariant::Str;

    fn path(suffix: u32) -> OwnedObjectPath {
        OwnedObjectPath::try_from(format!("/org/freedesktop/NetworkManager/Settings/{suffix}"))
            .expect("valid object path")
    }

    fn owned_string_array(values: &[&str]) -> OwnedValue {
        OwnedValue::try_from(zvariant::Value::from(
            values
                .iter()
                .map(|value| (*value).to_string())
                .collect::<Vec<_>>(),
        ))
        .expect("owned string array")
    }

    fn conn_section(uuid: &str, id: &str, ty: &str) -> HashMap<String, OwnedValue> {
        let mut m = HashMap::new();
        m.insert("uuid".into(), OwnedValue::from(Str::from(uuid)));
        m.insert("id".into(), OwnedValue::from(Str::from(id)));
        m.insert("type".into(), OwnedValue::from(Str::from(ty)));
        m.insert("autoconnect".into(), OwnedValue::from(true));
        m.insert("autoconnect-priority".into(), OwnedValue::from(0i32));
        m.insert("timestamp".into(), OwnedValue::from(0u64));
        m
    }

    #[test]
    fn decode_malformed_required_identity_fields() {
        let cases = [
            (HashMap::new(), "missing 'connection' section"),
            (
                HashMap::from([(
                    "connection".into(),
                    HashMap::from([
                        ("id".into(), OwnedValue::from(Str::from("x"))),
                        (
                            "type".into(),
                            OwnedValue::from(Str::from("802-11-wireless")),
                        ),
                    ]),
                )]),
                "missing connection.uuid",
            ),
            (
                HashMap::from([(
                    "connection".into(),
                    HashMap::from([
                        ("uuid".into(), OwnedValue::from(Str::from("u"))),
                        (
                            "type".into(),
                            OwnedValue::from(Str::from("802-11-wireless")),
                        ),
                    ]),
                )]),
                "missing connection.id",
            ),
            (
                HashMap::from([(
                    "connection".into(),
                    HashMap::from([
                        ("uuid".into(), OwnedValue::from(Str::from("u"))),
                        ("id".into(), OwnedValue::from(Str::from("x"))),
                    ]),
                )]),
                "missing connection.type",
            ),
            (
                HashMap::from([(
                    "connection".into(),
                    HashMap::from([
                        ("uuid".into(), OwnedValue::from(42u32)),
                        ("id".into(), OwnedValue::from(Str::from("x"))),
                        (
                            "type".into(),
                            OwnedValue::from(Str::from("802-11-wireless")),
                        ),
                    ]),
                )]),
                "missing connection.uuid",
            ),
        ];

        for (settings, expected) in cases {
            let result = decode_saved(path(1), false, None, settings);
            assert!(matches!(
                result,
                Err(ConnectionError::MalformedSavedConnection(message)) if message == expected
            ));
        }
    }

    #[test]
    fn decode_wifi_open() {
        let mut settings = HashMap::new();
        settings.insert(
            "connection".into(),
            conn_section("u1", "Coffee", "802-11-wireless"),
        );

        let mut w = HashMap::new();
        w.insert(
            "ssid".into(),
            OwnedValue::try_from(zvariant::Array::from(vec![67u8, 111, 102, 102, 101, 101]))
                .expect("ssid array"),
        );
        settings.insert("802-11-wireless".into(), w);

        let c = decode_saved(
            path(2),
            false,
            Some("/etc/NetworkManager/system-connections/coffee.nmconnection".into()),
            settings,
        )
        .unwrap();

        assert_eq!(c.uuid, "u1");
        assert_eq!(c.id, "Coffee");
        match c.summary {
            SettingsSummary::Wifi {
                ref ssid,
                security: None,
                ..
            } => {
                assert_eq!(ssid, "Coffee");
            }
            _ => panic!("expected wifi summary"),
        }
    }

    #[test]
    fn decode_wifi_psk_security() {
        let mut settings = HashMap::new();
        settings.insert(
            "connection".into(),
            conn_section("u2", "Home", "802-11-wireless"),
        );

        let mut w = HashMap::new();
        w.insert(
            "ssid".into(),
            OwnedValue::try_from(zvariant::Array::from(vec![72u8, 111, 109, 101]))
                .expect("ssid array"),
        );
        w.insert(
            "security".into(),
            OwnedValue::from(Str::from("802-11-wireless-security")),
        );
        settings.insert("802-11-wireless".into(), w);

        let mut sec = HashMap::new();
        sec.insert("key-mgmt".into(), OwnedValue::from(Str::from("wpa-psk")));
        sec.insert("psk-flags".into(), OwnedValue::from(1u32));
        sec.insert(
            "psk".into(),
            OwnedValue::from(Str::from("not-a-secret-in-test")),
        );
        settings.insert("802-11-wireless-security".into(), sec);

        let c = decode_saved(path(3), false, None, settings).unwrap();

        match c.summary {
            SettingsSummary::Wifi {
                security: Some(s), ..
            } => {
                assert_eq!(s.key_mgmt, WifiKeyMgmt::WpaPsk);
                assert!(s.has_psk_field);
                assert!(s.psk_agent_owned);
            }
            _ => panic!("expected wifi with security"),
        }
    }

    #[test]
    fn decode_wifi_eap_security_and_optional_fields() {
        let wireless = HashMap::from([
            (
                "ssid".into(),
                OwnedValue::try_from(zvariant::Value::from(b"Enterprise".to_vec()))
                    .expect("owned SSID"),
            ),
            ("mode".into(), OwnedValue::from(Str::from("infrastructure"))),
            ("band".into(), OwnedValue::from(Str::from("a"))),
            ("channel".into(), OwnedValue::from(36u32)),
            (
                "bssid".into(),
                OwnedValue::from(Str::from("00:11:22:33:44:55")),
            ),
            ("hidden".into(), OwnedValue::from(true)),
            (
                "mac-address-randomization".into(),
                OwnedValue::from(Str::from("always")),
            ),
        ]);
        let eap = HashMap::from([("eap".into(), owned_string_array(&["peap", "ttls"]))]);
        let saved = decode_saved(
            path(11),
            false,
            None,
            HashMap::from([
                (
                    "connection".into(),
                    conn_section("eap-u", "Enterprise", "802-11-wireless"),
                ),
                ("802-11-wireless".into(), wireless),
                ("802-1x".into(), eap),
            ]),
        )
        .unwrap();

        let SettingsSummary::Wifi {
            ssid,
            mode,
            security,
            band,
            channel,
            bssid,
            hidden,
            mac_randomization,
        } = saved.summary
        else {
            panic!("expected Wi-Fi summary")
        };
        assert_eq!(ssid, "Enterprise");
        assert_eq!(mode.as_deref(), Some("infrastructure"));
        assert_eq!(band.as_deref(), Some("a"));
        assert_eq!(channel, Some(36));
        assert_eq!(bssid.as_deref(), Some("00:11:22:33:44:55"));
        assert!(hidden);
        assert_eq!(mac_randomization.as_deref(), Some("always"));
        let security = security.expect("EAP security summary");
        assert_eq!(security.key_mgmt, WifiKeyMgmt::WpaEap);
        assert_eq!(security.eap_methods, ["peap", "ttls"]);
    }

    #[test]
    fn decode_vpn_wireguard_service() {
        let mut settings = HashMap::new();
        settings.insert("connection".into(), conn_section("u3", "wg", "vpn"));

        let mut vpn = HashMap::new();
        vpn.insert(
            "service-type".into(),
            OwnedValue::from(Str::from("org.freedesktop.NetworkManager.wireguard")),
        );
        vpn.insert("password-flags".into(), OwnedValue::from(0u32));
        settings.insert("vpn".into(), vpn);

        let mut wg = HashMap::new();
        wg.insert("listen-port".into(), OwnedValue::from(51820u32));
        settings.insert("wireguard".into(), wg);

        let c = decode_saved(path(4), false, None, settings).unwrap();

        match c.summary {
            SettingsSummary::WireGuard {
                listen_port: Some(51820),
                peer_count: 0,
                first_peer_endpoint: None,
                ..
            } => {}
            ref s => panic!("expected wireguard summary, got {s:?}"),
        }
    }

    #[test]
    fn decode_other_type() {
        let mut settings = HashMap::new();
        settings.insert("connection".into(), conn_section("u4", "tun", "tun"));

        let c = decode_saved(path(5), false, None, settings).unwrap();

        match c.summary {
            SettingsSummary::Other { sections } => {
                assert!(sections.contains(&"connection".to_string()));
            }
            _ => panic!("expected other"),
        }
    }

    #[test]
    fn patch_delta_autoconnect() {
        let patch = SettingsPatch {
            autoconnect: Some(false),
            ..Default::default()
        };
        let d = build_settings_patch_delta(&patch);
        assert_eq!(
            d.get("connection").unwrap().get("autoconnect"),
            Some(&OwnedValue::from(false))
        );
    }

    #[test]
    fn patch_delta_merges_into_full_settings() {
        let mut settings = HashMap::new();
        settings.insert(
            "connection".into(),
            conn_section("u5", "Home", "802-11-wireless"),
        );

        let patch = SettingsPatch {
            autoconnect: Some(false),
            ..Default::default()
        };
        let delta = build_settings_patch_delta(&patch);
        merge_settings_patch_delta(&mut settings, delta);

        let conn = settings.get("connection").unwrap();
        assert_eq!(
            owned_to_str(conn.get("type").unwrap()).as_deref(),
            Some("802-11-wireless")
        );
        assert_eq!(conn.get("autoconnect"), Some(&OwnedValue::from(false)));
    }

    #[test]
    fn patch_delta_overlay_merges_section() {
        let mut overlay = HashMap::new();
        let mut inner = HashMap::new();
        inner.insert("foo".into(), OwnedValue::from(Str::from("bar")));
        overlay.insert("ipv4".into(), inner);

        let patch = SettingsPatch {
            raw_overlay: Some(overlay),
            ..Default::default()
        };
        let d = build_settings_patch_delta(&patch);
        assert_eq!(
            owned_to_str(d.get("ipv4").unwrap().get("foo").unwrap()).as_deref(),
            Some("bar")
        );
    }

    #[test]
    fn decode_saved_brief_reads_identity_and_path() {
        let settings = HashMap::from([(
            "connection".into(),
            conn_section("brief-uuid", "Brief Name", "802-3-ethernet"),
        )]);
        let expected_path = path(6);

        let brief = decode_saved_brief(expected_path.clone(), &settings).unwrap();

        assert_eq!(brief.path, expected_path);
        assert_eq!(brief.uuid, "brief-uuid");
        assert_eq!(brief.id, "Brief Name");
        assert_eq!(brief.connection_type, "802-3-ethernet");
    }

    #[test]
    fn decode_saved_brief_rejects_each_missing_identity_field() {
        for (missing, expected) in [
            ("uuid", "missing connection.uuid"),
            ("id", "missing connection.id"),
            ("type", "missing connection.type"),
        ] {
            let mut connection = conn_section("u", "id", "vpn");
            connection.remove(missing);
            let settings = HashMap::from([("connection".into(), connection)]);
            assert!(matches!(
                decode_saved_brief(path(7), &settings),
                Err(ConnectionError::MalformedSavedConnection(message)) if message == expected
            ));
        }

        assert!(matches!(
            decode_saved_brief(path(7), &HashMap::new()),
            Err(ConnectionError::MalformedSavedConnection(message))
                if message == "missing 'connection' section"
        ));
    }

    #[test]
    fn decode_ethernet_summary_and_connection_metadata() {
        let mut connection = conn_section("eth-u", "Wired", "802-3-ethernet");
        connection.insert(
            "interface-name".into(),
            OwnedValue::from(Str::from("enp1s0")),
        );
        connection.insert("autoconnect".into(), OwnedValue::from(false));
        connection.insert("autoconnect-priority".into(), OwnedValue::from(-10i32));
        connection.insert("timestamp".into(), OwnedValue::from(1234u64));
        connection.insert(
            "permissions".into(),
            owned_string_array(&["user:alice:", "user:bob:"]),
        );
        let ethernet = HashMap::from([
            (
                "mac-address".into(),
                OwnedValue::from(Str::from("00:11:22:33:44:55")),
            ),
            ("auto-negotiate".into(), OwnedValue::from(true)),
            ("speed".into(), OwnedValue::from(1000u32)),
            ("mtu".into(), OwnedValue::from(9000u32)),
        ]);
        let saved = decode_saved(
            path(8),
            true,
            Some("/tmp/wired.nmconnection".into()),
            HashMap::from([
                ("connection".into(), connection),
                ("802-3-ethernet".into(), ethernet),
            ]),
        )
        .unwrap();

        assert_eq!(saved.interface_name.as_deref(), Some("enp1s0"));
        assert!(!saved.autoconnect);
        assert_eq!(saved.autoconnect_priority, -10);
        assert_eq!(saved.timestamp_unix, 1234);
        assert_eq!(saved.permissions, ["user:alice:", "user:bob:"]);
        assert!(saved.unsaved);
        assert_eq!(saved.filename.as_deref(), Some("/tmp/wired.nmconnection"));
        assert!(matches!(
            saved.summary,
            SettingsSummary::Ethernet {
                mac_address: Some(ref mac),
                auto_negotiate: Some(true),
                speed_mbps: Some(1000),
                mtu: Some(9000),
            } if mac == "00:11:22:33:44:55"
        ));
    }

    #[test]
    fn decode_connection_metadata_uses_documented_defaults_for_wrong_types() {
        let mut connection = conn_section("defaults-u", "Defaults", "802-3-ethernet");
        connection.insert("interface-name".into(), OwnedValue::from(Str::from("")));
        connection.insert("autoconnect".into(), OwnedValue::from(1u32));
        connection.insert(
            "autoconnect-priority".into(),
            OwnedValue::from(Str::from("high")),
        );
        connection.insert("timestamp".into(), OwnedValue::from(false));
        connection.insert("permissions".into(), OwnedValue::from(7u32));

        let saved = decode_saved(
            path(13),
            false,
            None,
            HashMap::from([("connection".into(), connection)]),
        )
        .unwrap();

        assert_eq!(saved.interface_name, None);
        assert!(saved.autoconnect);
        assert_eq!(saved.autoconnect_priority, 0);
        assert_eq!(saved.timestamp_unix, 0);
        assert!(saved.permissions.is_empty());
    }

    #[test]
    fn decode_generic_vpn_summary_omits_secret_values() {
        let data = zvariant::Dict::from(HashMap::from([
            ("remote".to_string(), "vpn.example.com".to_string()),
            ("cipher".to_string(), "AES-256-GCM".to_string()),
        ]));
        let vpn = HashMap::from([
            (
                "service-type".into(),
                OwnedValue::from(Str::from("org.freedesktop.NetworkManager.openvpn")),
            ),
            ("user-name".into(), OwnedValue::from(Str::from("alice"))),
            ("password-flags".into(), OwnedValue::from(1u32)),
            ("persistent".into(), OwnedValue::from(true)),
            (
                "data".into(),
                OwnedValue::try_from(zvariant::Value::Dict(data)).expect("owned dict"),
            ),
        ]);
        let saved = decode_saved(
            path(9),
            false,
            None,
            HashMap::from([
                ("connection".into(), conn_section("vpn-u", "VPN", "vpn")),
                ("vpn".into(), vpn),
            ]),
        )
        .unwrap();

        assert!(matches!(
            saved.summary,
            SettingsSummary::Vpn {
                ref service_type,
                user_name: Some(ref user),
                password_flags,
                ref data_keys,
                persistent: true,
            } if service_type == "org.freedesktop.NetworkManager.openvpn"
                && user == "alice"
                && password_flags.agent_owned()
                && data_keys == &["cipher".to_string(), "remote".to_string()]
        ));
    }

    #[test]
    fn decode_native_wireguard_summary_checks_port_range() {
        for (port, expected) in [(51820, Some(51820)), (u32::from(u16::MAX) + 1, None)] {
            let wireguard = HashMap::from([
                ("listen-port".into(), OwnedValue::from(port)),
                ("mtu".into(), OwnedValue::from(1420u32)),
                ("fwmark".into(), OwnedValue::from(7u32)),
            ]);
            let saved = decode_saved(
                path(10),
                false,
                None,
                HashMap::from([
                    (
                        "connection".into(),
                        conn_section("wg-u", "WireGuard", "wireguard"),
                    ),
                    ("wireguard".into(), wireguard),
                ]),
            )
            .unwrap();

            assert!(matches!(
                saved.summary,
                SettingsSummary::WireGuard {
                    listen_port,
                    mtu: Some(1420),
                    fwmark: Some(7),
                    peer_count: 0,
                    first_peer_endpoint: None,
                } if listen_port == expected
            ));
        }
    }

    #[test]
    fn decode_native_wireguard_peer_array() {
        let peer: HashMap<String, zvariant::Value<'static>> = HashMap::from([
            (
                "public-key".into(),
                zvariant::Value::Str("peer-public-key".into()),
            ),
            (
                "endpoint".into(),
                zvariant::Value::Str("vpn.example.com:51820".into()),
            ),
        ]);
        let peers =
            OwnedValue::try_from(zvariant::Value::from(vec![peer])).expect("owned peer array");
        let saved = decode_saved(
            path(12),
            false,
            None,
            HashMap::from([
                (
                    "connection".into(),
                    conn_section("wg-peer-u", "WireGuard peer", "wireguard"),
                ),
                ("wireguard".into(), HashMap::from([("peers".into(), peers)])),
            ]),
        )
        .unwrap();

        let SettingsSummary::WireGuard {
            peer_count,
            first_peer_endpoint,
            ..
        } = saved.summary
        else {
            panic!("expected WireGuard summary")
        };
        assert_eq!(peer_count, 1);
        assert_eq!(
            first_peer_endpoint.as_deref(),
            Some("vpn.example.com:51820")
        );
    }

    #[test]
    fn decode_mobile_and_bluetooth_summaries() {
        let gsm = decode_summary(
            "gsm",
            &HashMap::from([(
                "gsm".into(),
                HashMap::from([
                    ("apn".into(), OwnedValue::from(Str::from("internet"))),
                    ("username".into(), OwnedValue::from(Str::from("mobile"))),
                    ("password-flags".into(), OwnedValue::from(1u32)),
                    ("pin-flags".into(), OwnedValue::from(2u32)),
                ]),
            )]),
        );
        assert!(matches!(
            gsm,
            SettingsSummary::Gsm {
                apn: Some(ref apn),
                user_name: Some(ref user),
                password_flags: 1,
                pin_flags: 2,
            } if apn == "internet" && user == "mobile"
        ));

        let cdma = decode_summary(
            "cdma",
            &HashMap::from([(
                "cdma".into(),
                HashMap::from([
                    ("number".into(), OwnedValue::from(Str::from("#777"))),
                    ("username".into(), OwnedValue::from(Str::from("carrier"))),
                    ("password-flags".into(), OwnedValue::from(1u32)),
                ]),
            )]),
        );
        assert!(matches!(
            cdma,
            SettingsSummary::Cdma {
                number: Some(ref number),
                user_name: Some(ref user),
                password_flags: 1,
            } if number == "#777" && user == "carrier"
        ));

        let bluetooth = decode_summary(
            "bluetooth",
            &HashMap::from([(
                "bluetooth".into(),
                HashMap::from([(
                    "bdaddr".into(),
                    OwnedValue::from(Str::from("00:11:22:33:44:55")),
                )]),
            )]),
        );
        assert!(matches!(
            bluetooth,
            SettingsSummary::Bluetooth { ref bdaddr, ref bt_type }
                if bdaddr == "00:11:22:33:44:55" && bt_type == "panu"
        ));
    }

    #[test]
    fn patch_delta_empty_patch_is_empty() {
        assert!(build_settings_patch_delta(&SettingsPatch::default()).is_empty());
    }

    #[test]
    fn patch_delta_serializes_all_typed_fields_and_interface_clear() {
        let patch = SettingsPatch {
            autoconnect: Some(false),
            autoconnect_priority: Some(-42),
            id: Some("Renamed".into()),
            interface_name: Some(None),
            raw_overlay: None,
        };
        let delta = build_settings_patch_delta(&patch);
        let connection = delta.get("connection").unwrap();

        assert_eq!(
            owned_to_bool(connection.get("autoconnect").unwrap()),
            Some(false)
        );
        assert_eq!(
            owned_to_i32(connection.get("autoconnect-priority").unwrap()),
            Some(-42)
        );
        assert_eq!(
            owned_to_str(connection.get("id").unwrap()).as_deref(),
            Some("Renamed")
        );
        assert_eq!(
            owned_to_str(connection.get("interface-name").unwrap()).as_deref(),
            Some("")
        );
    }

    #[test]
    fn raw_overlay_has_documented_precedence_over_typed_fields() {
        let overlay = HashMap::from([(
            "connection".into(),
            HashMap::from([
                ("autoconnect".into(), OwnedValue::from(true)),
                ("id".into(), OwnedValue::from(Str::from("Overlay Name"))),
            ]),
        )]);
        let patch = SettingsPatch {
            autoconnect: Some(false),
            id: Some("Typed Name".into()),
            raw_overlay: Some(overlay),
            ..Default::default()
        };
        let delta = build_settings_patch_delta(&patch);
        let connection = delta.get("connection").unwrap();

        assert_eq!(
            owned_to_bool(connection.get("autoconnect").unwrap()),
            Some(true)
        );
        assert_eq!(
            owned_to_str(connection.get("id").unwrap()).as_deref(),
            Some("Overlay Name")
        );
    }

    #[test]
    fn merge_patch_creates_missing_sections_without_losing_existing_values() {
        let mut settings = HashMap::from([(
            "connection".into(),
            conn_section("merge-u", "Original", "802-3-ethernet"),
        )]);
        let delta = HashMap::from([(
            "ipv4".into(),
            HashMap::from([("method".into(), OwnedValue::from(Str::from("manual")))]),
        )]);

        merge_settings_patch_delta(&mut settings, delta);

        assert_eq!(
            owned_to_str(settings["connection"].get("id").unwrap()).as_deref(),
            Some("Original")
        );
        assert_eq!(
            owned_to_str(settings["ipv4"].get("method").unwrap()).as_deref(),
            Some("manual")
        );
    }
}
