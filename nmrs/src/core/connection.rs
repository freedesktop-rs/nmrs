use futures_timer::Delay;
use log::{debug, error, info, trace, warn};
use std::collections::HashMap;
use zbus::Connection;
use zvariant::OwnedObjectPath;

use crate::Result;
use crate::api::builders::wifi::{build_ethernet_connection, build_wifi_connection};
use crate::api::models::{ConnectionError, ConnectionOptions, TimeoutConfig, WifiSecurity};
use crate::core::connection_settings::{delete_connection, get_saved_connection_path};
use crate::core::state_wait::{wait_for_connection_activation, wait_for_device_disconnect};
use crate::dbus::{NMAccessPointProxy, NMDeviceProxy, NMProxy, NMWiredProxy, NMWirelessProxy};
use crate::monitoring::info::current_ssid;
use crate::monitoring::transport::ActiveTransport;
use crate::monitoring::wifi::Wifi;
use crate::types::constants::{device_state, device_type, timeouts};
use crate::types::device_type_registry;
use crate::util::utils::{decode_ssid_or_empty, nm_proxy};
use crate::util::validation::{validate_bssid, validate_ssid, validate_wifi_security};

/// Decision on whether to reuse a saved connection or create a fresh one.
#[derive(Debug, PartialEq, Eq)]
enum SavedDecision {
    /// Reuse the saved connection at this path.
    UseSaved(OwnedObjectPath),
    /// Create a new connection profile using the supplied credentials.
    RebuildFresh,
}

/// Connects to a Wi-Fi network.
///
/// This is the main entry point for establishing a Wi-Fi connection. The flow:
/// 1. Check for an existing saved connection for this SSID
/// 2. Decide whether to reuse it or create fresh (based on credentials)
/// 3. Find the Wi-Fi device and target access point
/// 4. Either activate the saved connection or create and activate a new one
/// 5. Wait for the connection to reach the activated state
///
/// If a saved connection exists but fails, it is deleted and a fresh
/// connection is attempted only when the caller supplied usable fallback
/// settings. An empty PSK requests stored credentials, so a failed saved
/// profile is preserved and its activation error is returned.
pub(crate) async fn connect(
    conn: &Connection,
    ssid: &str,
    creds: WifiSecurity,
    interface: Option<&str>,
    timeout_config: Option<TimeoutConfig>,
) -> Result<()> {
    // Validate inputs before attempting connection
    validate_ssid(ssid)?;
    validate_wifi_security(&creds)?;

    debug!(
        "Connecting to '{}' on {:?} | secured={} is_psk={} is_eap={}",
        ssid,
        interface,
        creds.secured(),
        creds.is_psk(),
        creds.is_eap()
    );

    let nm = NMProxy::new(conn).await?;

    let saved_raw = get_saved_connection_path(conn, ssid).await?;
    let decision = decide_saved_connection(saved_raw, &creds)?;

    let wifi_device = resolve_wifi_device(conn, &nm, interface).await?;
    trace!("Resolved WiFi device: {}", wifi_device.as_str());

    let wifi = NMWirelessProxy::builder(conn)
        .path(wifi_device.clone())?
        .build()
        .await?;

    if let Some(active) = Wifi::current(conn).await {
        debug!("Currently connected to: {active}");
        if active == ssid {
            debug!("Already connected to {active}, skipping connect()");
            return Ok(());
        }
    } else {
        trace!("Not currently connected to any network");
    }

    let specific_object = scan_and_resolve_ap(conn, &wifi, ssid).await?;

    match decision {
        SavedDecision::UseSaved(saved) => {
            ensure_disconnected(conn, &wifi_device, timeout_config).await?;
            connect_via_saved(
                conn,
                &nm,
                &wifi_device,
                &specific_object,
                ssid,
                &creds,
                saved,
                timeout_config,
            )
            .await?;
        }
        SavedDecision::RebuildFresh => {
            build_and_activate_new(
                conn,
                &nm,
                &wifi_device,
                &specific_object,
                ssid,
                creds,
                timeout_config,
            )
            .await?;
        }
    }

    // Connection activation is now handled within connect_via_saved() and
    // build_and_activate_new() using signal-based monitoring
    info!("Successfully connected to '{ssid}'");

    Ok(())
}

/// Connects to a wired (Ethernet) device.
///
/// This is the main entry point for establishing a wired connection. The flow:
/// 1. Find a wired device
/// 2. Check for an existing saved connection
/// 3. Either activate the saved connection or create and activate a new one
/// 4. Wait for the connection to reach the activated state
///
/// Ethernet connections are typically simpler than Wi-Fi - no scanning or
/// access points needed. The connection will activate when a cable is plugged in.
pub(crate) async fn connect_wired(
    conn: &Connection,
    timeout_config: Option<TimeoutConfig>,
) -> Result<()> {
    debug!("Connecting to wired device");

    let nm = NMProxy::new(conn).await?;

    let wired_device = find_wired_device(conn, &nm).await?;
    trace!("Found wired device: {}", wired_device.as_str());

    // Check if already connected
    let dev = NMDeviceProxy::builder(conn)
        .path(wired_device.clone())?
        .build()
        .await?;
    let current_state = dev.state().await?;
    if current_state == device_state::ACTIVATED {
        debug!("Wired device already activated, skipping connect()");
        return Ok(());
    }

    // Check for saved connection (by interface name)
    let interface = dev.interface().await?;
    let saved = get_saved_connection_path(conn, &interface).await?;

    // For Ethernet, we use "/" as the specific_object (no access point needed)
    let specific_object = OwnedObjectPath::default();

    match saved {
        Some(saved_path) => {
            debug!("Activating saved wired connection: {}", saved_path.as_str());
            let active_conn = nm
                .activate_connection(saved_path, wired_device.clone(), specific_object)
                .await?;
            let timeout = timeout_config.map(|c| c.connection_timeout);
            wait_for_connection_activation(conn, &active_conn, timeout).await?;
        }
        None => {
            debug!("No saved connection found, creating new wired connection");
            let opts = ConnectionOptions {
                autoconnect: true,
                autoconnect_priority: None,
                autoconnect_retries: None,
            };

            let settings = build_ethernet_connection(&interface, &opts);
            let (_, active_conn) = nm
                .add_and_activate_connection(settings, wired_device.clone(), specific_object)
                .await?;
            let timeout = timeout_config.map(|c| c.connection_timeout);
            wait_for_connection_activation(conn, &active_conn, timeout).await?;
        }
    }

    if let Ok(wired) = NMWiredProxy::builder(conn)
        .path(wired_device.clone())?
        .build()
        .await
        && let Ok(speed) = wired.speed().await
    {
        info!("Connected to wired device at {speed} Mb/s");
    }

    info!("Successfully connected to wired device");
    Ok(())
}

/// Generic function to forget (delete) connections by name and optionally by device type.
///
/// This handles disconnection if currently active, then deletes the connection profile(s).
/// Can be used for WiFi, Bluetooth, or any NetworkManager connection type.
///
/// # Arguments
///
/// * `conn` - D-Bus connection
/// * `name` - Connection name/identifier to forget
/// * `device_filter` - Optional device type filter (e.g., `Some(device_type::BLUETOOTH)`)
///
/// # Returns
///
/// Returns `Ok(())` if at least one connection was deleted successfully.
/// Returns `NoSavedConnection` if no matching connections were found.
pub(crate) async fn forget_by_name_and_type(
    conn: &Connection,
    name: &str,
    device_filter: Option<u32>,
    timeout_config: Option<TimeoutConfig>,
) -> Result<()> {
    use std::collections::HashMap;
    use zvariant::{OwnedObjectPath, Value};

    // Validate SSID
    validate_ssid(name)?;

    debug!(
        "Starting forget operation for: {name} (device filter: {:?})",
        device_filter
    );

    let nm = NMProxy::new(conn).await?;

    // Disconnect if currently active
    let devices = nm.get_devices().await?;
    for dev_path in &devices {
        let dev = NMDeviceProxy::builder(conn)
            .path(dev_path.clone())?
            .build()
            .await?;

        let dev_type = dev.device_type().await?;

        // Skip if device type doesn't match our filter
        if let Some(filter) = device_filter
            && dev_type != filter
        {
            continue;
        }

        // Handle WiFi-specific disconnect logic
        if dev_type == device_type::WIFI {
            let wifi = NMWirelessProxy::builder(conn)
                .path(dev_path.clone())?
                .build()
                .await?;
            if let Ok(ap_path) = wifi.active_access_point().await
                && ap_path.as_str() != "/"
            {
                let ap = NMAccessPointProxy::builder(conn)
                    .path(ap_path.clone())?
                    .build()
                    .await?;
                if let Ok(bytes) = ap.ssid().await
                    && decode_ssid_or_empty(&bytes) == name
                {
                    debug!("Disconnecting from active WiFi network: {name}");
                    if let Err(e) = disconnect_wifi_and_wait(conn, dev_path, timeout_config).await {
                        warn!("Disconnect wait failed: {e}");
                        let final_state = dev.state().await?;
                        if final_state != device_state::DISCONNECTED
                            && final_state != device_state::UNAVAILABLE
                        {
                            error!(
                                "Device still connected (state: {final_state}), cannot safely delete"
                            );
                            return Err(ConnectionError::Stuck(format!(
                                "disconnect failed, device in state {final_state}"
                            )));
                        }
                        debug!("Device confirmed disconnected, proceeding with deletion");
                    }
                    trace!("WiFi disconnect phase completed");
                }
            }
        }
        // Handle Bluetooth-specific disconnect logic
        else if dev_type == device_type::BLUETOOTH {
            // Check if this Bluetooth device is currently active
            let state = dev.state().await?;
            if state != device_state::DISCONNECTED && state != device_state::UNAVAILABLE {
                debug!("Disconnecting from active Bluetooth device: {name}");
                if let Err(e) = crate::core::bluetooth::disconnect_bluetooth_and_wait(
                    conn,
                    dev_path,
                    timeout_config,
                )
                .await
                {
                    warn!("Bluetooth disconnect failed: {e}");
                    let final_state = dev.state().await?;
                    if final_state != device_state::DISCONNECTED
                        && final_state != device_state::UNAVAILABLE
                    {
                        error!(
                            "Bluetooth device still connected (state: {final_state}), cannot safely delete"
                        );
                        return Err(ConnectionError::Stuck(format!(
                            "disconnect failed, device in state {final_state}"
                        )));
                    }
                }
                trace!("Bluetooth disconnect phase completed");
            }
        }
    }

    // Delete connection profiles (generic, works for all types)
    trace!("Starting connection deletion phase...");

    let settings = nm_proxy(
        conn,
        "/org/freedesktop/NetworkManager/Settings",
        "org.freedesktop.NetworkManager.Settings",
    )
    .await?;

    let list_reply = settings.call_method("ListConnections", &()).await?;
    let conns: Vec<OwnedObjectPath> = list_reply.body().deserialize()?;

    let mut deleted_count = 0;

    for cpath in conns {
        let cproxy = nm_proxy(
            conn,
            cpath.clone(),
            "org.freedesktop.NetworkManager.Settings.Connection",
        )
        .await?;

        if let Ok(msg) = cproxy.call_method("GetSettings", &()).await {
            let body = msg.body();
            let settings_map: HashMap<String, HashMap<String, Value>> = body.deserialize()?;

            let mut should_delete = false;

            // Match by connection ID (works for all connection types)
            if let Some(conn_sec) = settings_map.get("connection")
                && let Some(Value::Str(id)) = conn_sec.get("id")
                && id.as_str() == name
            {
                should_delete = true;
                trace!("Found connection by ID: {id}");
            }

            // Additional WiFi-specific matching by SSID
            if let Some(wifi_sec) = settings_map.get("802-11-wireless")
                && let Some(Value::Array(arr)) = wifi_sec.get("ssid")
            {
                let mut raw = Vec::new();
                for v in arr.iter() {
                    if let Ok(b) = u8::try_from(v.clone()) {
                        raw.push(b);
                    }
                }
                if decode_ssid_or_empty(&raw) == name {
                    should_delete = true;
                    trace!("Found WiFi connection by SSID match");
                }
            }

            // Matching by bdaddr for Bluetooth connections
            if let Some(bt_sec) = settings_map.get("bluetooth")
                && let Some(Value::Str(bdaddr)) = bt_sec.get("bdaddr")
                && bdaddr.as_str() == name
            {
                should_delete = true;
                trace!("Found Bluetooth connection by bdaddr match");
            }

            if let Some(wsec) = settings_map.get("802-11-wireless-security") {
                let missing_psk = !wsec.contains_key("psk");
                let empty_psk = matches!(wsec.get("psk"), Some(Value::Str(s)) if s.is_empty());

                if (missing_psk || empty_psk) && should_delete {
                    trace!("Connection has missing/empty PSK, will delete");
                }
            }

            if should_delete {
                match cproxy.call_method("Delete", &()).await {
                    Ok(_) => {
                        deleted_count += 1;
                        trace!("Deleted connection: {}", cpath.as_str());
                    }
                    Err(e) => {
                        warn!("Failed to delete connection {}: {}", cpath.as_str(), e);
                    }
                }
            }
        }
    }

    if deleted_count > 0 {
        info!("Successfully deleted {deleted_count} connection(s) for '{name}'");
        Ok(())
    } else {
        debug!("No saved connections found for '{name}'");

        // For Bluetooth, it's normal to have no NetworkManager connection profile if the device is only paired in BlueZ.
        if device_filter == Some(device_type::BLUETOOTH) {
            debug!(
                "Bluetooth device '{name}' has no NetworkManager connection profile (device may only be paired in BlueZ)"
            );
            Ok(())
        } else {
            Ok(())
        }
    }
}

/// Disconnects a Wi-Fi device and waits for it to reach disconnected state.
///
/// Calls the Disconnect method on the device and waits for the `StateChanged`
/// signal to indicate the device has reached Disconnected or Unavailable state.
/// This is more efficient than polling and responds immediately when the
/// device disconnects.
pub(crate) async fn disconnect_wifi_and_wait(
    conn: &Connection,
    dev_path: &OwnedObjectPath,
    timeout_config: Option<TimeoutConfig>,
) -> Result<()> {
    let dev = NMDeviceProxy::builder(conn)
        .path(dev_path.clone())?
        .build()
        .await?;

    // Check if already disconnected
    let current_state = dev.state().await?;
    if current_state == device_state::DISCONNECTED || current_state == device_state::UNAVAILABLE {
        debug!("Device already disconnected");
        return Ok(());
    }

    let raw = nm_proxy(
        conn,
        dev_path.clone(),
        "org.freedesktop.NetworkManager.Device",
    )
    .await?;

    trace!("Sending disconnect request");
    raw.call_method("Disconnect", &()).await?;
    trace!("Disconnect method called successfully");

    // Wait for disconnect using signal-based monitoring
    let timeout = timeout_config.map(|c| c.disconnect_timeout);
    wait_for_device_disconnect(&dev, timeout).await?;

    // Brief stabilization delay
    Delay::new(timeouts::stabilization_delay()).await;

    Ok(())
}

/// Finds a network device by its type.
///
/// Iterates through all devices managed by NetworkManager
/// and returns the path of the first device matching the specified type.
/// Returns an appropriate error if no matching device is found.
async fn find_device_by_type(
    conn: &Connection,
    nm: &NMProxy<'_>,
    device_type_id: u32,
) -> Result<OwnedObjectPath> {
    let devices = nm.get_devices().await?;

    for dp in devices {
        let dev = NMDeviceProxy::builder(conn)
            .path(dp.clone())?
            .build()
            .await?;
        if device_matches_type(
            dev.device_type().await?,
            dev.managed().await?,
            device_type_id,
        ) {
            return Ok(dp);
        }
    }

    match device_type_id {
        device_type::WIFI => Err(ConnectionError::NoWifiDevice),
        device_type::ETHERNET => Err(ConnectionError::NoWiredDevice),
        _ => Err(ConnectionError::NoWifiDevice),
    }
}

fn device_matches_type(actual_type: u32, managed: bool, expected_type: u32) -> bool {
    managed
        && (actual_type == expected_type
            || (expected_type == device_type::ETHERNET
                && device_type_registry::is_wired(actual_type)))
}

pub(crate) async fn find_wired_device(
    conn: &Connection,
    nm: &NMProxy<'_>,
) -> Result<OwnedObjectPath> {
    find_device_by_type(conn, nm, device_type::ETHERNET).await
}

async fn find_wifi_device(conn: &Connection, nm: &NMProxy<'_>) -> Result<OwnedObjectPath> {
    find_device_by_type(conn, nm, device_type::WIFI).await
}

/// Resolves a Wi-Fi device path from an optional interface name.
///
/// `None` returns the first Wi-Fi device NM reports (back-compat behavior).
/// `Some(name)` looks up the device by interface and verifies it is Wi-Fi:
/// returns [`WifiInterfaceNotFound`] or [`NotAWifiDevice`] if not.
///
/// [`WifiInterfaceNotFound`]: ConnectionError::WifiInterfaceNotFound
/// [`NotAWifiDevice`]: ConnectionError::NotAWifiDevice
pub(crate) async fn resolve_wifi_device(
    conn: &Connection,
    nm: &NMProxy<'_>,
    interface: Option<&str>,
) -> Result<OwnedObjectPath> {
    match interface {
        None => find_wifi_device(conn, nm).await,
        Some(name) => {
            let path = match get_device_by_interface(conn, name).await {
                Ok(p) => p,
                Err(ConnectionError::NotFound) => {
                    return Err(ConnectionError::WifiInterfaceNotFound {
                        interface: name.to_string(),
                    });
                }
                Err(e) => return Err(e),
            };
            let dev = NMDeviceProxy::builder(conn)
                .path(path.clone())?
                .build()
                .await?;
            if dev.device_type().await? != device_type::WIFI {
                return Err(ConnectionError::NotAWifiDevice {
                    interface: name.to_string(),
                });
            }
            Ok(path)
        }
    }
}

/// Finds an access point by SSID.
///
/// Searches through all visible access points on the wireless device
/// and returns the path of the first one matching the target SSID.
/// Returns `NotFound` if no matching access point is visible.
async fn find_ap(
    conn: &Connection,
    wifi: &NMWirelessProxy<'_>,
    target_ssid: &str,
) -> Result<OwnedObjectPath> {
    let access_points = wifi.access_points().await?;

    for ap_path in access_points {
        let ap = NMAccessPointProxy::builder(conn)
            .path(ap_path.clone())?
            .build()
            .await?;

        let ssid_bytes = ap.ssid().await?;
        let ssid = decode_ssid_or_empty(&ssid_bytes);

        if ssid == target_ssid {
            return Ok(ap_path);
        }
    }

    Err(ConnectionError::NotFound)
}

/// Finds an access point matching both SSID and BSSID.
async fn find_ap_by_bssid(
    conn: &Connection,
    wifi: &NMWirelessProxy<'_>,
    target_ssid: &str,
    target_bssid: &str,
) -> Result<OwnedObjectPath> {
    let access_points = wifi.access_points().await?;

    for ap_path in access_points {
        let ap = NMAccessPointProxy::builder(conn)
            .path(ap_path.clone())?
            .build()
            .await?;

        let ssid_bytes = ap.ssid().await?;
        let ssid = decode_ssid_or_empty(&ssid_bytes);

        if ssid != target_ssid {
            continue;
        }

        let bssid = ap.hw_address().await?;
        if bssid.eq_ignore_ascii_case(target_bssid) {
            return Ok(ap_path);
        }
    }

    Err(ConnectionError::ApBssidNotFound {
        ssid: target_ssid.to_string(),
        bssid: target_bssid.to_string(),
    })
}

/// Connects to a specific access point identified by SSID and optionally BSSID.
///
/// If `bssid` is `Some`, the connection targets that specific AP.
/// If `None`, falls through to the existing best-match behavior.
pub(crate) async fn connect_to_bssid(
    conn: &Connection,
    ssid: &str,
    bssid: Option<&str>,
    creds: WifiSecurity,
    interface: Option<&str>,
    timeout_config: Option<TimeoutConfig>,
) -> Result<()> {
    if let Some(b) = bssid {
        validate_bssid(b)?;
    }

    match bssid {
        None => connect(conn, ssid, creds, interface, timeout_config).await,
        Some(target_bssid) => {
            validate_ssid(ssid)?;
            validate_wifi_security(&creds)?;

            debug!(
                "Connecting to '{}' BSSID={} on {:?} | secured={} is_psk={} is_eap={}",
                ssid,
                target_bssid,
                interface,
                creds.secured(),
                creds.is_psk(),
                creds.is_eap()
            );

            let nm = NMProxy::new(conn).await?;
            let saved_raw = get_saved_connection_path(conn, ssid).await?;
            let decision = decide_saved_connection(saved_raw, &creds)?;
            let wifi_device = resolve_wifi_device(conn, &nm, interface).await?;
            let wifi = NMWirelessProxy::builder(conn)
                .path(wifi_device.clone())?
                .build()
                .await?;

            match wifi.request_scan(HashMap::new()).await {
                Ok(_) => trace!("Scan requested successfully"),
                Err(e) => warn!("Scan request failed: {e}"),
            }
            futures_timer::Delay::new(timeouts::scan_wait()).await;

            let specific_object = find_ap_by_bssid(conn, &wifi, ssid, target_bssid).await?;

            match decision {
                SavedDecision::UseSaved(saved) => {
                    ensure_disconnected(conn, &wifi_device, timeout_config).await?;
                    connect_via_saved(
                        conn,
                        &nm,
                        &wifi_device,
                        &specific_object,
                        ssid,
                        &creds,
                        saved,
                        timeout_config,
                    )
                    .await?;
                }
                SavedDecision::RebuildFresh => {
                    build_and_activate_new(
                        conn,
                        &nm,
                        &wifi_device,
                        &specific_object,
                        ssid,
                        creds,
                        timeout_config,
                    )
                    .await?;
                }
            }

            info!("Successfully connected to '{ssid}' (BSSID: {target_bssid})");
            Ok(())
        }
    }
}

/// Ensures the target Wi-Fi device is torn down before attempting a new connection.
///
/// Only the given `wifi_device` is affected. Other interfaces (e.g. VPN, wired,
/// a second Wi-Fi radio) are not deactivated.
async fn ensure_disconnected(
    conn: &Connection,
    wifi_device: &OwnedObjectPath,
    timeout_config: Option<TimeoutConfig>,
) -> Result<()> {
    disconnect_wifi_and_wait(conn, wifi_device, timeout_config).await
}

/// Attempts to connect using a saved connection profile.
///
/// Activates the saved connection and monitors the activation state using
/// D-Bus signals. If activation fails (device disconnects or enters failed
/// state), deletes the saved connection and creates a fresh one when the
/// provided settings can stand alone. A request to use a stored PSK has no
/// usable fallback, so the profile is preserved and the failure is returned.
///
/// This handles cases where saved passwords are outdated or corrupted.
async fn connect_via_saved(
    conn: &Connection,
    nm: &NMProxy<'_>,
    wifi_device: &OwnedObjectPath,
    ap: &OwnedObjectPath,
    ssid: &str,
    creds: &WifiSecurity,
    saved: OwnedObjectPath,
    timeout_config: Option<TimeoutConfig>,
) -> Result<()> {
    debug!("Activating saved connection: {}", saved.as_str());

    match nm
        .activate_connection(saved.clone(), wifi_device.clone(), ap.clone())
        .await
    {
        Ok(active_conn) => {
            trace!(
                "activate_connection() succeeded, active connection: {}",
                active_conn.as_str()
            );

            // Wait for connection activation using signal-based monitoring
            let timeout = timeout_config.map(|c| c.connection_timeout);
            match wait_for_connection_activation(conn, &active_conn, timeout).await {
                Ok(()) => {
                    debug!("Saved connection activated successfully");
                }
                Err(e) => {
                    warn!("Saved connection activation failed: {e}");

                    if !can_rebuild_after_saved_failure(creds) {
                        warn!("No fresh credentials were supplied; preserving the saved profile");
                        return Err(e);
                    }

                    warn!("Deleting saved connection and retrying with fresh credentials");

                    match nm.deactivate_connection(active_conn.clone()).await {
                        Ok(_) => debug!("Connection deactivated during cleanup"),
                        Err(e) => warn!("Failed to deactivate connection during cleanup: {}", e),
                    }
                    match delete_connection(conn, saved.clone()).await {
                        Ok(_) => debug!("Saved connection deleted"),
                        Err(e) => warn!("Failed to delete saved connection during recovery: {}", e),
                    }

                    let opts = ConnectionOptions {
                        autoconnect: true,
                        autoconnect_priority: None,
                        autoconnect_retries: None,
                    };

                    let settings = build_wifi_connection(ssid, creds, &opts);

                    debug!("Creating fresh connection with corrected settings");
                    let (new_connection, new_active_conn) = nm
                        .add_and_activate_connection(settings, wifi_device.clone(), ap.clone())
                        .await
                        .map_err(|e| {
                            error!("Fresh connection also failed: {e}");
                            e
                        })?;

                    // Wait for the fresh connection to activate
                    let timeout = timeout_config.map(|c| c.connection_timeout);
                    wait_for_fresh_activation(conn, nm, &new_connection, &new_active_conn, timeout)
                        .await?;
                }
            }
        }

        Err(e) => {
            warn!("activate_connection() failed: {e}");

            if !can_rebuild_after_saved_failure(creds) {
                warn!("No fresh credentials were supplied; preserving the saved profile");
                return Err(e.into());
            }

            warn!("Saved connection may be corrupted, deleting and retrying with fresh connection");

            match delete_connection(conn, saved.clone()).await {
                Ok(_) => debug!("Saved connection deleted"),
                Err(e) => warn!("Failed to delete saved connection during recovery: {}", e),
            }

            let opts = ConnectionOptions {
                autoconnect: true,
                autoconnect_priority: None,
                autoconnect_retries: None,
            };

            let settings = build_wifi_connection(ssid, creds, &opts);

            let (new_connection, active_conn) = nm
                .add_and_activate_connection(settings, wifi_device.clone(), ap.clone())
                .await
                .map_err(|e| {
                    error!("Fresh connection also failed: {e}");
                    e
                })?;

            // Wait for the fresh connection to activate
            let timeout = timeout_config.map(|c| c.connection_timeout);
            wait_for_fresh_activation(conn, nm, &new_connection, &active_conn, timeout).await?;
        }
    }

    Ok(())
}

async fn wait_for_fresh_activation(
    conn: &Connection,
    nm: &NMProxy<'_>,
    connection_path: &OwnedObjectPath,
    active_connection_path: &OwnedObjectPath,
    timeout: Option<std::time::Duration>,
) -> Result<()> {
    if let Err(error) = wait_for_connection_activation(conn, active_connection_path, timeout).await
    {
        if let Err(cleanup_error) = nm
            .deactivate_connection(active_connection_path.clone())
            .await
        {
            warn!("Failed to deactivate rejected fresh connection: {cleanup_error}");
        }
        if let Err(cleanup_error) = delete_connection(conn, connection_path.clone()).await {
            warn!("Failed to delete rejected fresh connection profile: {cleanup_error}");
        }
        return Err(error);
    }

    Ok(())
}

/// Creates a new connection profile and activates it.
///
/// Builds connection settings from the provided credentials, ensures the
/// device is disconnected, then calls AddAndActivateConnection to create
/// and activate the connection in one step. Monitors activation using
/// D-Bus signals for immediate feedback on success or failure.
async fn build_and_activate_new(
    conn: &Connection,
    nm: &NMProxy<'_>,
    wifi_device: &OwnedObjectPath,
    ap: &OwnedObjectPath,
    ssid: &str,
    creds: WifiSecurity,
    timeout_config: Option<TimeoutConfig>,
) -> Result<()> {
    let opts = ConnectionOptions {
        autoconnect: true,
        autoconnect_retries: None,
        autoconnect_priority: None,
    };

    let settings = build_wifi_connection(ssid, &creds, &opts);

    trace!("Creating new connection, settings: \n{settings:#?}");

    ensure_disconnected(conn, wifi_device, timeout_config).await?;

    let (connection_path, active_conn) = match nm
        .add_and_activate_connection(settings, wifi_device.clone(), ap.clone())
        .await
    {
        Ok(paths) => {
            trace!(
                "add_and_activate_connection() succeeded, active connection: {}",
                paths.1.as_str()
            );
            paths
        }
        Err(e) => {
            error!("add_and_activate_connection() failed: {e}");
            return Err(e.into());
        }
    };

    trace!("Waiting for connection activation using signal monitoring...");

    // Wait for connection activation using the ActiveConnection signals
    let timeout = timeout_config.map(|c| c.connection_timeout);
    wait_for_fresh_activation(conn, nm, &connection_path, &active_conn, timeout).await?;

    info!("Connection to '{ssid}' activated successfully");

    Ok(())
}

/// Triggers a Wi-Fi scan and finds the target access point.
///
/// Requests a scan, waits briefly for results, then searches for an
/// access point matching the target SSID. The wait time is shorter than
/// polling-based approaches since we just need the scan to populate
/// initial results.
async fn scan_and_resolve_ap(
    conn: &Connection,
    wifi: &NMWirelessProxy<'_>,
    ssid: &str,
) -> Result<OwnedObjectPath> {
    match wifi.request_scan(HashMap::new()).await {
        Ok(_) => trace!("Scan requested successfully"),
        Err(e) => warn!("Scan request failed: {e}"),
    }

    // Brief wait for scan results to populate
    Delay::new(timeouts::scan_wait()).await;
    trace!("Scan wait complete");

    let ap = find_ap(conn, wifi, ssid).await?;
    trace!("Matched target SSID '{ssid}'");
    Ok(ap)
}

/// Decides whether to use a saved connection or create a fresh one.
///
/// Decision logic:
/// - If a saved connection exists and credentials are empty PSK, use saved
///   (user wants to connect with stored password)
/// - If a saved connection exists for an open network, use saved
/// - If a saved connection exists but fresh PSK or EAP credentials were
///   provided, create a fresh profile so those credentials are not ignored
/// - If no saved connection and PSK is empty, error (can't connect without password)
/// - Otherwise, create a fresh connection
fn decide_saved_connection(
    saved: Option<OwnedObjectPath>,
    creds: &WifiSecurity,
) -> Result<SavedDecision> {
    match saved {
        Some(path)
            if matches!(creds, WifiSecurity::Open)
                || matches!(creds, WifiSecurity::WpaPsk { psk } if psk.is_empty()) =>
        {
            Ok(SavedDecision::UseSaved(path))
        }
        Some(_) => Ok(SavedDecision::RebuildFresh),
        None if matches!(creds, WifiSecurity::WpaPsk { psk } if psk.is_empty()) => {
            Err(ConnectionError::MissingPassword)
        }
        None => Ok(SavedDecision::RebuildFresh),
    }
}

/// Whether a failed saved-profile activation can be retried without relying on
/// that profile's stored secret.
fn can_rebuild_after_saved_failure(creds: &WifiSecurity) -> bool {
    !matches!(creds, WifiSecurity::WpaPsk { psk } if psk.is_empty())
}

/// Checks if currently connected to the specified SSID.
///
/// If already connected, returns true. Otherwise, returns false.
/// This can be used to skip redundant connection attempts.
pub(crate) async fn is_connected(conn: &Connection, ssid: &str) -> Result<bool> {
    if let Some(active) = current_ssid(conn).await {
        debug!("Currently connected to: {active}");
        if active == ssid {
            debug!("Already connected to {active}");
            return Ok(true);
        }
    } else {
        trace!("Not currently connected to any network");
    }
    Ok(false)
}

/// Disconnects from the currently active network.
///
/// This finds the current active WiFi connection and deactivates it,
/// then waits for the device to reach disconnected state.
///
/// Returns `Ok(())` if disconnected successfully or if no active connection exists.
pub(crate) async fn disconnect(
    conn: &Connection,
    interface: Option<&str>,
    timeout_config: Option<TimeoutConfig>,
) -> Result<()> {
    let nm = NMProxy::new(conn).await?;

    let wifi_device = match resolve_wifi_device(conn, &nm, interface).await {
        Ok(dev) => dev,
        Err(ConnectionError::NoWifiDevice) => {
            debug!("No WiFi device found");
            return Ok(());
        }
        Err(e) => return Err(e),
    };

    let dev = NMDeviceProxy::builder(conn)
        .path(wifi_device.clone())?
        .build()
        .await?;

    let current_state = dev.state().await?;
    if current_state == device_state::DISCONNECTED || current_state == device_state::UNAVAILABLE {
        debug!("Device already disconnected");
        return Ok(());
    }

    if let Ok(conns) = nm.active_connections().await {
        for conn_path in conns {
            let active = match crate::dbus::NMActiveConnectionProxy::builder(conn)
                .path(conn_path.clone())?
                .build()
                .await
            {
                Ok(p) => p,
                Err(e) => {
                    warn!("Failed to build active connection proxy: {}", e);
                    continue;
                }
            };
            let owns_device = match active.devices().await {
                Ok(devs) => devs.iter().any(|d| d == &wifi_device),
                Err(_) => false,
            };
            if !owns_device {
                continue;
            }
            match nm.deactivate_connection(conn_path.clone()).await {
                Ok(_) => trace!("Connection deactivated"),
                Err(e) => warn!("Failed to deactivate connection: {}", e),
            }
        }
    }

    disconnect_wifi_and_wait(conn, &wifi_device, timeout_config).await?;

    info!("Disconnected from network");
    Ok(())
}

/// Finds a device by its interface name.
///
/// Returns the device path if found, or an error if not found.
pub(crate) async fn get_device_by_interface(
    conn: &Connection,
    interface_name: &str,
) -> Result<OwnedObjectPath> {
    let nm = NMProxy::new(conn).await?;
    let devices = nm.get_devices().await?;

    for dev_path in devices {
        let dev = NMDeviceProxy::builder(conn)
            .path(dev_path.clone())?
            .build()
            .await?;

        if let Ok(iface) = dev.interface().await
            && iface == interface_name
        {
            trace!("Found device with interface: {}", interface_name);
            return Ok(dev_path);
        }
    }

    Err(ConnectionError::NotFound)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::api::models::EapOptions;

    fn saved_path() -> OwnedObjectPath {
        OwnedObjectPath::try_from("/org/freedesktop/NetworkManager/Settings/1")
            .expect("valid object path")
    }

    #[test]
    fn automatic_device_selection_requires_matching_type_and_managed_state() {
        assert!(device_matches_type(
            device_type::ETHERNET,
            true,
            device_type::ETHERNET
        ));
        assert!(device_matches_type(
            device_type::VETH,
            true,
            device_type::ETHERNET
        ));
        assert!(!device_matches_type(
            device_type::ETHERNET,
            false,
            device_type::ETHERNET
        ));
        assert!(!device_matches_type(
            device_type::WIFI,
            true,
            device_type::ETHERNET
        ));
        assert!(!device_matches_type(
            device_type::VETH,
            false,
            device_type::ETHERNET
        ));
    }

    fn enterprise_credentials() -> WifiSecurity {
        WifiSecurity::WpaEap {
            opts: EapOptions::new("user", "password"),
        }
    }

    fn wpa3_enterprise_credentials() -> WifiSecurity {
        WifiSecurity::Wpa3Eap192bit {
            opts: EapOptions::new_tls_blob("user", vec![1], vec![2]),
        }
    }

    #[test]
    fn saved_profile_is_reused_only_without_fresh_credentials() {
        let path = saved_path();

        assert_eq!(
            decide_saved_connection(Some(path.clone()), &WifiSecurity::Open).unwrap(),
            SavedDecision::UseSaved(path.clone())
        );
        assert_eq!(
            decide_saved_connection(
                Some(path.clone()),
                &WifiSecurity::WpaPsk { psk: String::new() },
            )
            .unwrap(),
            SavedDecision::UseSaved(path)
        );
    }

    #[test]
    fn saved_profile_is_rebuilt_for_every_supplied_credential_kind() {
        let cases = [
            WifiSecurity::WpaPsk {
                psk: "new password".into(),
            },
            WifiSecurity::WpaPsk {
                psk: "        ".into(),
            },
            enterprise_credentials(),
            wpa3_enterprise_credentials(),
        ];

        for creds in cases {
            validate_wifi_security(&creds).expect("test credential should be valid");
            assert_eq!(
                decide_saved_connection(Some(saved_path()), &creds).unwrap(),
                SavedDecision::RebuildFresh,
                "fresh credentials must not be ignored: {creds:?}"
            );
        }
    }

    #[test]
    fn absent_profile_rejects_only_the_empty_stored_secret_sentinel() {
        assert!(matches!(
            decide_saved_connection(None, &WifiSecurity::WpaPsk { psk: String::new() }),
            Err(ConnectionError::MissingPassword)
        ));

        let whitespace_psk = WifiSecurity::WpaPsk {
            psk: "        ".into(),
        };
        validate_wifi_security(&whitespace_psk).expect("eight spaces is a valid-length PSK");
        assert_eq!(
            decide_saved_connection(None, &whitespace_psk).unwrap(),
            SavedDecision::RebuildFresh
        );
    }

    #[test]
    fn absent_profile_builds_open_and_enterprise_connections() {
        assert_eq!(
            decide_saved_connection(None, &WifiSecurity::Open).unwrap(),
            SavedDecision::RebuildFresh
        );
        assert_eq!(
            decide_saved_connection(None, &enterprise_credentials()).unwrap(),
            SavedDecision::RebuildFresh
        );
        assert_eq!(
            decide_saved_connection(None, &wpa3_enterprise_credentials()).unwrap(),
            SavedDecision::RebuildFresh
        );
    }

    #[test]
    fn saved_failure_recovery_requires_usable_fresh_settings() {
        assert!(!can_rebuild_after_saved_failure(&WifiSecurity::WpaPsk {
            psk: String::new(),
        }));
        assert!(can_rebuild_after_saved_failure(&WifiSecurity::Open));
        assert!(can_rebuild_after_saved_failure(&WifiSecurity::WpaPsk {
            psk: "password".into(),
        }));
        assert!(can_rebuild_after_saved_failure(&enterprise_credentials()));
        assert!(can_rebuild_after_saved_failure(
            &wpa3_enterprise_credentials()
        ));
    }
}
