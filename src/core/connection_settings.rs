//! Saved connection profile management.
//!
//! Provides functions for querying and deleting saved NetworkManager
//! connection profiles. Saved connections persist across reboots and
//! store credentials for automatic reconnection.

use log::trace;
use std::collections::HashMap;
use zbus::Connection;
use zvariant::{OwnedObjectPath, OwnedValue, Value};

use crate::Result;
use crate::api::models::ConnectionError;
use crate::api::models::property::FromSetting;
use crate::util::utils::{connection_settings_proxy, settings_proxy};
use crate::util::validation::validate_connection_name;

/// Finds a saved profile whose `connection.id` matches `name` (SSID for typical Wi-Fi).
///
/// Returns the D-Bus path and `connection.uuid` of the first match.
async fn find_saved_connection_by_name(
    conn: &Connection,
    name: &str,
) -> Result<Option<(OwnedObjectPath, String)>> {
    let settings = settings_proxy(conn).await?;

    let reply = settings
        .call_method("ListConnections", &())
        .await
        .map_err(|e| ConnectionError::DbusOperation {
            context: "failed to list saved connections".to_string(),
            source: e,
        })?;

    let conns: Vec<OwnedObjectPath> = reply.body().deserialize()?;

    for cpath in conns {
        let cproxy = connection_settings_proxy(conn, cpath.clone()).await?;

        // `ListConnections` includes profiles restricted to other users via
        // `connection.permissions`; `GetSettings` on those fails with
        // `Settings.PermissionDenied`. Skip them instead of failing the
        // whole lookup, matching the other profile scans in this crate.
        let msg = match cproxy.call_method("GetSettings", &()).await {
            Ok(msg) => msg,
            Err(e) => {
                trace!(
                    "skipping saved connection {}: GetSettings failed: {e}",
                    cpath.as_str()
                );
                continue;
            }
        };

        let body = msg.body();
        let all: HashMap<String, HashMap<String, Value>> = body.deserialize()?;

        if let Some(conn_section) = all.get("connection")
            && let Some(Value::Str(id)) = conn_section.get("id")
            && id == name
            && let Some(Value::Str(uuid)) = conn_section.get("uuid")
        {
            return Ok(Some((cpath, uuid.to_string())));
        }
    }

    Ok(None)
}

/// Whether a saved profile's settings describe a Wi-Fi client profile for `ssid`.
///
/// Matches on `802-11-wireless.ssid` bytes, not `connection.id`, so profiles
/// created by other tools or renamed by the user are still found. Hotspot
/// (`mode = ap`) profiles are skipped.
fn wifi_profile_matches_ssid(
    settings: &HashMap<String, HashMap<String, OwnedValue>>,
    ssid: &[u8],
) -> bool {
    let is_wifi = settings
        .get("connection")
        .and_then(|c| c.get("type"))
        .and_then(String::from_setting)
        .is_some_and(|t| t == "802-11-wireless");
    let Some(wireless) = settings.get("802-11-wireless") else {
        return false;
    };
    let is_ap = wireless
        .get("mode")
        .and_then(String::from_setting)
        .is_some_and(|m| m == "ap");
    let profile_ssid = wireless.get("ssid").and_then(Vec::<u8>::from_setting);

    is_wifi && !is_ap && profile_ssid.as_deref() == Some(ssid)
}

/// Finds the D-Bus path of a saved Wi-Fi client profile for `ssid`.
///
/// Returns the first profile whose `802-11-wireless.ssid` equals `ssid`,
/// regardless of its `connection.id`.
pub(crate) async fn get_saved_wifi_connection_path(
    conn: &Connection,
    ssid: &str,
) -> Result<Option<OwnedObjectPath>> {
    if ssid.is_empty() {
        return Ok(None);
    }

    let settings = settings_proxy(conn).await?;

    let reply = settings
        .call_method("ListConnections", &())
        .await
        .map_err(|e| ConnectionError::DbusOperation {
            context: "failed to list saved connections".to_string(),
            source: e,
        })?;

    let conns: Vec<OwnedObjectPath> = reply.body().deserialize()?;

    for cpath in conns {
        let cproxy = connection_settings_proxy(conn, cpath.clone()).await?;

        // Skip profiles restricted to other users, as in
        // `find_saved_connection_by_name`.
        let msg = match cproxy.call_method("GetSettings", &()).await {
            Ok(msg) => msg,
            Err(e) => {
                trace!(
                    "skipping saved connection {}: GetSettings failed: {e}",
                    cpath.as_str()
                );
                continue;
            }
        };

        let all: HashMap<String, HashMap<String, OwnedValue>> = msg.body().deserialize()?;

        if wifi_profile_matches_ssid(&all, ssid.as_bytes()) {
            return Ok(Some(cpath));
        }
    }

    Ok(None)
}

/// Finds the D-Bus path of a saved connection by SSID or connection name.
///
/// Iterates through all saved connections in NetworkManager's settings
/// and returns the path of the first one whose connection ID matches
/// the given SSID or name.
///
/// Returns `None` if no saved connection exists for this SSID/name.
pub(crate) async fn get_saved_connection_path(
    conn: &Connection,
    name: &str,
) -> Result<Option<OwnedObjectPath>> {
    if should_skip_lookup(name)? {
        return Ok(None);
    }

    Ok(find_saved_connection_by_name(conn, name)
        .await?
        .map(|(path, _)| path))
}

/// Returns the profile UUID for a saved connection whose `connection.id` matches `name`.
///
/// For Wi-Fi profiles created by nmrs, `connection.id` is usually the SSID — the same
/// string accepted by [`has_saved_connection`](crate::NetworkManager::has_saved_connection)
/// and [`forget`](crate::NetworkManager::forget).
///
/// Returns `None` when no profile matches.
pub(crate) async fn get_saved_connection_uuid(
    conn: &Connection,
    name: &str,
) -> Result<Option<String>> {
    if should_skip_lookup(name)? {
        return Ok(None);
    }

    Ok(find_saved_connection_by_name(conn, name)
        .await?
        .map(|(_, uuid)| uuid))
}

fn should_skip_lookup(name: &str) -> Result<bool> {
    if name.trim().is_empty() {
        return Ok(true);
    }

    validate_connection_name(name)?;
    Ok(false)
}

/// Checks whether a saved connection exists for the given SSID.
pub(crate) async fn has_saved_connection(conn: &Connection, ssid: &str) -> Result<bool> {
    get_saved_connection_path(conn, ssid)
        .await
        .map(|p| p.is_some())
}

/// Deletes a saved connection by its D-Bus path.
///
/// Calls the Delete method on the connection settings object.
/// This permanently removes the saved connection from NetworkManager.
pub(crate) async fn delete_connection(conn: &Connection, conn_path: OwnedObjectPath) -> Result<()> {
    let cproxy = connection_settings_proxy(conn, conn_path.clone()).await?;

    cproxy
        .call_method("Delete", &())
        .await
        .map_err(|e| ConnectionError::DbusOperation {
            context: format!("failed to delete connection {}", conn_path.as_str()),
            source: e,
        })?;

    trace!("Deleted connection: {}", conn_path.as_str());
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn saved_connection_lookup_allows_vpn_names_longer_than_ssids() {
        let name = "gw-UDP4-1199-namme.lastname-config";

        assert!(name.len() > 32);
        assert!(!should_skip_lookup(name).unwrap());
    }

    #[test]
    fn saved_connection_lookup_skips_blank_names() {
        assert!(should_skip_lookup("").unwrap());
        assert!(should_skip_lookup("   ").unwrap());
    }

    fn profile(
        id: &str,
        ty: &str,
        ssid: &[u8],
        mode: Option<&str>,
    ) -> HashMap<String, HashMap<String, OwnedValue>> {
        let mut connection = HashMap::new();
        connection.insert("id".into(), OwnedValue::from(zvariant::Str::from(id)));
        connection.insert("type".into(), OwnedValue::from(zvariant::Str::from(ty)));

        let mut wireless = HashMap::new();
        wireless.insert(
            "ssid".into(),
            OwnedValue::try_from(zvariant::Array::from(ssid.to_vec())).expect("ssid array"),
        );
        if let Some(mode) = mode {
            wireless.insert("mode".into(), OwnedValue::from(zvariant::Str::from(mode)));
        }

        let mut settings = HashMap::new();
        settings.insert("connection".into(), connection);
        settings.insert("802-11-wireless".into(), wireless);
        settings
    }

    #[test]
    fn wifi_profile_matches_on_ssid_not_id() {
        // nmtui and NM's auto-generated names often differ from the SSID.
        let renamed = profile("Vodafone 1", "802-11-wireless", b"Vodafone", None);
        assert!(wifi_profile_matches_ssid(&renamed, b"Vodafone"));

        let infra = profile(
            "Vodafone",
            "802-11-wireless",
            b"Vodafone",
            Some("infrastructure"),
        );
        assert!(wifi_profile_matches_ssid(&infra, b"Vodafone"));
    }

    #[test]
    fn wifi_profile_ignores_id_that_looks_like_the_ssid() {
        let other = profile("Vodafone", "802-11-wireless", b"Elsewhere", None);
        assert!(!wifi_profile_matches_ssid(&other, b"Vodafone"));
    }

    #[test]
    fn wifi_profile_skips_hotspots_and_other_types() {
        let hotspot = profile("Hotspot", "802-11-wireless", b"Vodafone", Some("ap"));
        assert!(!wifi_profile_matches_ssid(&hotspot, b"Vodafone"));

        let vpn = profile("Vodafone", "vpn", b"Vodafone", None);
        assert!(!wifi_profile_matches_ssid(&vpn, b"Vodafone"));
    }
}
