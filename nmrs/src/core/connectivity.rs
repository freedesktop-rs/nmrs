//! Connectivity state reads and captive-portal URL discovery.

use log::trace;
use zbus::Connection;

use crate::Result;
use crate::api::models::{ConnectionError, ConnectivityReport, ConnectivityState};
use crate::dbus::NMProxy;

/// Reads `Connectivity` property.
pub(crate) async fn connectivity(conn: &Connection) -> Result<ConnectivityState> {
    let nm = NMProxy::new(conn).await?;
    let raw = nm
        .connectivity()
        .await
        .map_err(|e| ConnectionError::DbusOperation {
            context: "read Connectivity property".into(),
            source: e,
        })?;
    Ok(ConnectivityState::from(raw))
}

/// Calls `CheckConnectivity` (blocks until NM finishes its probe).
pub(crate) async fn check_connectivity(conn: &Connection) -> Result<ConnectivityState> {
    let nm = NMProxy::new(conn).await?;

    let enabled = nm.connectivity_check_enabled().await.unwrap_or(false);
    if !enabled {
        return Err(ConnectionError::ConnectivityCheckDisabled);
    }

    let raw = nm
        .check_connectivity()
        .await
        .map_err(|e| ConnectionError::DbusOperation {
            context: "CheckConnectivity call".into(),
            source: e,
        })?;
    Ok(ConnectivityState::from(raw))
}

/// Builds a full [`ConnectivityReport`] from property reads.
pub(crate) async fn connectivity_report(conn: &Connection) -> Result<ConnectivityReport> {
    let nm = NMProxy::new(conn).await?;

    let raw_state = nm.connectivity().await.unwrap_or(0);
    let state = ConnectivityState::from(raw_state);
    let check_enabled = nm.connectivity_check_enabled().await.unwrap_or(false);
    let check_uri = nm
        .connectivity_check_uri()
        .await
        .ok()
        .filter(|s| !s.is_empty());

    let captive_portal_url = if state.is_captive() {
        detect_captive_portal_url(conn, &nm).await
    } else {
        None
    };

    Ok(ConnectivityReport {
        state,
        check_enabled,
        check_uri,
        captive_portal_url,
    })
}

/// Best-effort captive portal URL detection.
///
/// Tries NM's `Ip4Config` properties on the primary connection first
/// (newer NM versions). Falls back to the configured `ConnectivityCheckUri`.
async fn detect_captive_portal_url(conn: &Connection, nm: &NMProxy<'_>) -> Option<String> {
    let primary = nm.primary_connection().await.ok()?;
    if primary.as_str() == "/" {
        return fallback_check_uri(nm).await;
    }

    let active = crate::dbus::NMActiveConnectionProxy::builder(conn)
        .path(primary)
        .ok()?
        .build()
        .await
        .ok()?;

    if let Ok(ip4_path) = active.ip4_config().await
        && ip4_path.as_str() != "/"
        && let Some(url) = try_ip4_captive_portal(conn, &ip4_path).await
    {
        return Some(url);
    }

    fallback_check_uri(nm).await
}

/// Newer NM versions expose a `CaptivePortal` or `WebPortalUrl` property on Ip4Config.
async fn try_ip4_captive_portal(
    conn: &Connection,
    ip4_path: &zvariant::OwnedObjectPath,
) -> Option<String> {
    let raw = crate::util::utils::nm_proxy(
        conn,
        ip4_path.clone(),
        "org.freedesktop.NetworkManager.IP4Config",
    )
    .await
    .ok()?;

    for prop in ["CaptivePortal", "WebPortalUrl"] {
        if let Ok(v) = raw.get_property::<String>(prop).await
            && !v.is_empty()
        {
            trace!("captive portal URL from IP4Config.{prop}: {v}");
            return Some(v);
        }
    }
    None
}

async fn fallback_check_uri(nm: &NMProxy<'_>) -> Option<String> {
    nm.connectivity_check_uri()
        .await
        .ok()
        .filter(|s| !s.is_empty())
}
