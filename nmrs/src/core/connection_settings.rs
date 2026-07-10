//! Saved connection profile management.
//!
//! Provides functions for querying and deleting saved NetworkManager
//! connection profiles. Saved connections persist across reboots and
//! store credentials for automatic reconnection.

use log::trace;
use std::collections::HashMap;
use zbus::Connection;
use zvariant::{OwnedObjectPath, Value};

use crate::Result;
use crate::api::models::ConnectionError;
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

        let msg = cproxy.call_method("GetSettings", &()).await.map_err(|e| {
            ConnectionError::DbusOperation {
                context: format!("failed to get settings for {}", cpath.as_str()),
                source: e,
            }
        })?;

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
}
