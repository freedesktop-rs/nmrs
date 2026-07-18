//! Airplane-mode aggregation logic.
//!
//! Combines radio state from NetworkManager (Wi-Fi, WWAN), BlueZ (Bluetooth
//! adapter power), and kernel rfkill into a single [`AirplaneModeState`].
//!
//! Each radio's state carries a `present` flag so consumers can ignore radios
//! the host does not actually have (no Wi-Fi card, no modem, BlueZ not
//! running) instead of blocking airplane-mode aggregation forever.

use std::collections::HashSet;
use std::time::Duration;

use futures::{FutureExt, StreamExt, future};
use futures_timer::Delay;
use log::warn;
use std::pin::pin;
use zbus::Connection;

use crate::api::models::{AirplaneModeState, RadioState};
use crate::core::rfkill::read_rfkill;
use crate::dbus::{BluezAdapterProxy, NMDeviceProxy, NMProxy};
use crate::types::constants::device_type;
use crate::{ConnectionError, Result};

/// Maximum time to wait for all BlueZ adapters' `Powered` properties to settle
/// after a write. BlueZ usually settles in well under a second; we cap at two
/// to avoid hanging UI consumers. This is an overall timeout for all adapters,
/// not per-adapter.
const BLUEZ_POWER_SETTLE_TIMEOUT: Duration = Duration::from_secs(2);

/// Reads Wi-Fi radio state from NetworkManager, cross-referenced with rfkill.
///
/// If `present_device_types` is `Some(set)`, uses the set to determine whether
/// a Wi-Fi device exists. If `None`, assumes the radio is present (used when
/// the device list couldn't be fetched).
pub(crate) async fn wifi_state(
    conn: &Connection,
    present_device_types: Option<&HashSet<u32>>,
) -> Result<RadioState> {
    let nm = NMProxy::new(conn).await?;
    let enabled = nm.wireless_enabled().await?;
    let nm_hw = nm.wireless_hardware_enabled().await?;

    let rfkill = read_rfkill();
    let hardware_enabled = reconcile_hardware(nm_hw, rfkill.wlan_hard_block, "wifi");
    let present = match present_device_types {
        Some(types) => types.contains(&device_type::WIFI),
        None => true, // Assume present if we couldn't fetch device list
    };

    Ok(RadioState::with_presence(
        enabled,
        hardware_enabled,
        present,
    ))
}

/// Reads WWAN radio state from NetworkManager, cross-referenced with rfkill.
///
/// If `present_device_types` is `Some(set)`, uses the set to determine whether
/// a modem device exists. If `None`, assumes the radio is present (used when
/// the device list couldn't be fetched).
pub(crate) async fn wwan_state(
    conn: &Connection,
    present_device_types: Option<&HashSet<u32>>,
) -> Result<RadioState> {
    let nm = NMProxy::new(conn).await?;
    let enabled = nm.wwan_enabled().await?;
    let nm_hw = nm.wwan_hardware_enabled().await?;

    let rfkill = read_rfkill();
    let hardware_enabled = reconcile_hardware(nm_hw, rfkill.wwan_hard_block, "wwan");
    let present = match present_device_types {
        Some(types) => types.contains(&device_type::MODEM),
        None => true, // Assume present if we couldn't fetch device list
    };

    Ok(RadioState::with_presence(
        enabled,
        hardware_enabled,
        present,
    ))
}

/// Reads Bluetooth radio state from BlueZ adapters, cross-referenced with rfkill.
///
/// If BlueZ is not running or no adapters exist, returns a `RadioState`
/// with `present = false` so callers can ignore Bluetooth entirely on
/// hosts that don't have it.
pub(crate) async fn bluetooth_radio_state(conn: &Connection) -> Result<RadioState> {
    let adapter_paths = match enumerate_bluetooth_adapters(conn).await {
        Ok(paths) if !paths.is_empty() => paths,
        Ok(_) | Err(_) => {
            return Ok(RadioState::with_presence(false, false, false));
        }
    };

    let mut any_powered = false;
    for path in &adapter_paths {
        match BluezAdapterProxy::builder(conn)
            .path(path.as_str())?
            .build()
            .await
        {
            Ok(proxy) => {
                if proxy.powered().await.unwrap_or(false) {
                    any_powered = true;
                    break;
                }
            }
            Err(e) => {
                warn!("failed to query BlueZ adapter {}: {}", path, e);
            }
        }
    }

    let rfkill = read_rfkill();
    let hardware_enabled = !rfkill.bluetooth_hard_block;

    Ok(RadioState::with_presence(
        any_powered,
        hardware_enabled,
        true,
    ))
}

/// Returns the combined airplane mode state for all radios.
///
/// Fetches the device list once and passes it to wifi/wwan state queries to
/// avoid redundant D-Bus round-trips. If the device list can't be fetched,
/// radios are assumed present rather than incorrectly marked absent.
pub(crate) async fn airplane_mode_state(conn: &Connection) -> Result<AirplaneModeState> {
    let present_types = fetch_present_device_types(conn).await;

    let (wifi, wwan, bt) = futures::future::join3(
        wifi_state(conn, present_types.as_ref()),
        wwan_state(conn, present_types.as_ref()),
        bluetooth_radio_state(conn),
    )
    .await;

    Ok(AirplaneModeState::new(wifi?, wwan?, bt?))
}

/// Enables or disables wireless radio (software toggle).
pub(crate) async fn set_wireless_enabled(conn: &Connection, enabled: bool) -> Result<()> {
    let nm = NMProxy::new(conn).await?;
    Ok(nm.set_wireless_enabled(enabled).await?)
}

/// Enables or disables WWAN radio (software toggle).
pub(crate) async fn set_wwan_enabled(conn: &Connection, enabled: bool) -> Result<()> {
    let nm = NMProxy::new(conn).await?;
    Ok(nm.set_wwan_enabled(enabled).await?)
}

/// Enables or disables Bluetooth radio via kernel rfkill and BlueZ adapters.
///
/// Uses `rfkill block bluetooth` / `rfkill unblock bluetooth` as the primary
/// mechanism — this is authoritative, persistent, and matches what other
/// Cosmic components (e.g. `cosmic-settings-airplane-mode-subscription`) read
/// back when determining airplane-mode state.
///
/// After rfkill, we also toggle BlueZ adapter `Powered` properties and wait
/// up to [`BLUEZ_POWER_SETTLE_TIMEOUT`] for them to settle, so that a
/// read-after-write of [`bluetooth_radio_state`] sees the correct value.
///
/// # Errors
///
/// - [`ConnectionError::BluezUnavailable`] if BlueZ is not running or no
///   adapters exist.
/// - [`ConnectionError::BluetoothToggleFailed`] if rfkill failed, or one or
///   more BlueZ adapters could not be toggled / did not reach the requested
///   state.
pub(crate) async fn set_bluetooth_radio_enabled(conn: &Connection, enabled: bool) -> Result<()> {
    let rfkill_arg = if enabled { "unblock" } else { "block" };
    let rfkill_status = tokio::process::Command::new("rfkill")
        .arg(rfkill_arg)
        .arg("bluetooth")
        .status()
        .await;

    match rfkill_status {
        Ok(status) if status.success() => {}
        Ok(status) => {
            warn!("rfkill {rfkill_arg} bluetooth exited with {status}");
        }
        Err(e) => {
            warn!("failed to run rfkill {rfkill_arg} bluetooth: {e}");
        }
    }

    let adapter_paths = enumerate_bluetooth_adapters(conn).await.map_err(|e| {
        ConnectionError::BluezUnavailable(format!("failed to enumerate adapters: {e}"))
    })?;

    if adapter_paths.is_empty() {
        return Err(ConnectionError::BluezUnavailable(
            "no Bluetooth adapters found".to_string(),
        ));
    }

    let n_adapters = adapter_paths.len();

    let toggle_futures = adapter_paths.iter().map(|path| async move {
        let proxy = match BluezAdapterProxy::builder(conn).path(path.as_str()) {
            Ok(builder) => match builder.build().await {
                Ok(proxy) => proxy,
                Err(e) => {
                    warn!("failed to build proxy for adapter {}: {}", path, e);
                    return None;
                }
            },
            Err(e) => {
                warn!("invalid adapter path {}: {}", path, e);
                return None;
            }
        };

        if let Err(e) = proxy.set_powered(enabled).await {
            warn!("failed to set Powered on {}: {}", path, e);
            return None;
        }

        Some(proxy)
    });

    let results: Vec<_> = futures::future::join_all(toggle_futures).await;
    let n_ok = results.iter().filter(|r| r.is_some()).count();
    if n_ok != n_adapters {
        return Err(ConnectionError::BluetoothToggleFailed(format!(
            "failed to toggle {} of {} Bluetooth adapter(s)",
            n_adapters.saturating_sub(n_ok),
            n_adapters
        )));
    }

    let successful_proxies: Vec<_> = results.into_iter().flatten().collect();

    let wait_futures = successful_proxies
        .iter()
        .map(|proxy| wait_for_powered_no_timeout(proxy, enabled));

    let all_waits = futures::future::join_all(wait_futures);
    let timer = Delay::new(BLUEZ_POWER_SETTLE_TIMEOUT);

    let all_waits = pin!(all_waits.fuse());
    let timer = pin!(timer.fuse());
    let _ = future::select(all_waits, timer).await;

    for proxy in &successful_proxies {
        match proxy.powered().await {
            Ok(v) if v == enabled => {}
            Ok(_) => {
                return Err(ConnectionError::BluetoothToggleFailed(
                    "Bluetooth adapter Powered did not reach requested state in time".to_string(),
                ));
            }
            Err(e) => {
                return Err(ConnectionError::BluetoothToggleFailed(format!(
                    "could not read Powered after toggle: {e}"
                )));
            }
        }
    }

    Ok(())
}

/// Flips all three radios in parallel.
///
/// `enabled = true` means airplane mode **on** (radios **off**).
/// Does not fail fast — attempts all three and returns the first error,
/// except that a missing Bluetooth stack (BlueZ not running or no adapters)
/// is treated as a successful no-op. Bluetooth adapter toggle/settle failures
/// are treated as non-fatal for the aggregate operation only when Wi-Fi or
/// WWAN is also present; in that case Wi-Fi/WWAN remain authoritative for the
/// airplane-mode flip and Bluetooth failures are logged for diagnostics. In a
/// Bluetooth-only case, a Bluetooth toggle failure may still be returned.
pub(crate) async fn set_airplane_mode(conn: &Connection, enabled: bool) -> Result<()> {
    let radio_on = !enabled;
    let present_types = fetch_present_device_types(conn).await;
    let allow_nonfatal_bt_toggle_failed = present_types
        .as_ref()
        .map(|types| types.contains(&device_type::WIFI) || types.contains(&device_type::MODEM))
        .unwrap_or(true);

    let (wifi_res, wwan_res, bt_res) = futures::future::join3(
        set_wireless_enabled(conn, radio_on),
        set_wwan_enabled(conn, radio_on),
        set_bluetooth_radio_enabled(conn, radio_on),
    )
    .await;

    finalize_airplane_toggle_results(wifi_res, wwan_res, bt_res, allow_nonfatal_bt_toggle_failed)
}

// Applies aggregate airplane-mode error semantics after all three toggle attempts complete.
fn finalize_airplane_toggle_results(
    wifi_res: Result<()>,
    wwan_res: Result<()>,
    bt_res: Result<()>,
    allow_nonfatal_bt_toggle_failed: bool,
) -> Result<()> {
    // Return the first error, but don't short-circuit — all three have been attempted.
    wifi_res?;
    wwan_res?;
    match bt_res {
        Ok(()) => {}
        Err(ConnectionError::BluezUnavailable(message)) => {
            // No Bluetooth on this host (BlueZ not running or no adapters) —
            // that's fine, don't fail the whole call.
            warn!(
                "Ignoring Bluetooth airplane-mode toggle because BlueZ is unavailable: {}",
                message
            );
        }
        Err(ConnectionError::BluetoothToggleFailed(message)) => {
            if allow_nonfatal_bt_toggle_failed {
                // Adapters exist but one or more did not settle. Avoid reporting
                // total failure after Wi-Fi/WWAN were already applied.
                warn!(
                    "Ignoring Bluetooth airplane-mode toggle failure during aggregate airplane toggle: {message}"
                );
            } else {
                // Bluetooth appears to be the only controllable radio on this host,
                // so failing to toggle it should fail the aggregate operation.
                return Err(ConnectionError::BluetoothToggleFailed(message));
            }
        }
        Err(e) => return Err(e),
    }
    Ok(())
}

/// Enumerates BlueZ Bluetooth adapters via the ObjectManager interface.
///
/// Returns adapter object paths (e.g. `/org/bluez/hci0`).
async fn enumerate_bluetooth_adapters(conn: &Connection) -> Result<Vec<String>> {
    let manager = zbus::fdo::ObjectManagerProxy::builder(conn)
        .destination("org.bluez")?
        .path("/")?
        .build()
        .await
        .map_err(|e| {
            ConnectionError::BluezUnavailable(format!("failed to connect to BlueZ: {e}"))
        })?;

    let objects = manager.get_managed_objects().await.map_err(|e| {
        ConnectionError::BluezUnavailable(format!("failed to enumerate BlueZ objects: {e}"))
    })?;

    let adapters: Vec<String> = objects
        .into_iter()
        .filter(|(_, ifaces)| ifaces.contains_key("org.bluez.Adapter1"))
        .map(|(path, _)| path.to_string())
        .collect();

    Ok(adapters)
}

/// Reconciles NM's hardware-enabled flag with rfkill. If they disagree, trust rfkill.
fn reconcile_hardware(nm_hardware_enabled: bool, rfkill_hard_block: bool, radio: &str) -> bool {
    if nm_hardware_enabled && rfkill_hard_block {
        warn!(
            "{radio}: NM reports hardware enabled but rfkill reports hard block — trusting rfkill"
        );
        return false;
    }
    nm_hardware_enabled && !rfkill_hard_block
}

/// Fetches all device types present in NetworkManager device objects.
///
/// Queries the device list once and returns a set of device type codes.
/// Returns `None` if the device list could not be fetched at all (NM
/// unavailable, `GetDevices` failed) or if any enumerated device could not
/// be introspected (incomplete enumeration), signaling that callers should
/// assume radios are present rather than risk a false negative.
pub(crate) async fn fetch_present_device_types(conn: &Connection) -> Option<HashSet<u32>> {
    let nm = NMProxy::new(conn).await.ok()?;
    let paths = nm.get_devices().await.ok()?;

    let mut types = HashSet::new();
    for p in paths {
        let builder = NMDeviceProxy::builder(conn).path(p).ok()?;
        let dev = builder.build().await.ok()?;
        let t = dev.device_type().await.ok()?;
        types.insert(t);
    }

    Some(types)
}

/// Waits for a BlueZ adapter's `Powered` property to settle on `target`.
///
/// Subscribes to `PropertiesChanged` on `Powered` first, then re-reads the
/// current value (so we don't miss a fast transition that happened between
/// the `set_powered` write and the subscription). Returns when the property
/// matches `target`. This variant has no timeout — use with an external
/// timeout wrapper when waiting on multiple adapters concurrently.
async fn wait_for_powered_no_timeout(proxy: &BluezAdapterProxy<'_>, target: bool) {
    let mut stream = proxy.receive_powered_changed().await;

    if let Ok(value) = proxy.powered().await
        && value == target
    {
        return;
    }

    while let Some(change) = stream.next().await {
        if let Ok(value) = change.get().await
            && value == target
        {
            return;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{finalize_airplane_toggle_results, reconcile_hardware};
    use crate::ConnectionError;

    #[test]
    fn aggregate_toggle_treats_bluetooth_toggle_failed_as_non_fatal() {
        let result = finalize_airplane_toggle_results(
            Ok(()),
            Ok(()),
            Err(ConnectionError::BluetoothToggleFailed(
                "adapter did not settle".to_string(),
            )),
            true,
        );

        assert!(matches!(result, Ok(())));
    }

    #[test]
    fn aggregate_toggle_propagates_bluetooth_toggle_failed_when_bt_only() {
        let result = finalize_airplane_toggle_results(
            Ok(()),
            Ok(()),
            Err(ConnectionError::BluetoothToggleFailed(
                "adapter did not settle".to_string(),
            )),
            false,
        );

        assert!(matches!(
            result,
            Err(ConnectionError::BluetoothToggleFailed(message))
                if message == "adapter did not settle"
        ));
    }

    #[test]
    fn aggregate_toggle_treats_bluez_unavailable_as_non_fatal() {
        let result = finalize_airplane_toggle_results(
            Ok(()),
            Ok(()),
            Err(ConnectionError::BluezUnavailable(
                "org.bluez not running".to_string(),
            )),
            false,
        );

        assert!(matches!(result, Ok(())));
    }

    #[test]
    fn aggregate_toggle_propagates_other_bluetooth_errors() {
        let result = finalize_airplane_toggle_results(
            Ok(()),
            Ok(()),
            Err(ConnectionError::InvalidInput {
                field: "bluetooth".to_string(),
                reason: "unexpected failure".to_string(),
            }),
            true,
        );

        assert!(matches!(
            result,
            Err(ConnectionError::InvalidInput { field, reason })
                if field == "bluetooth" && reason == "unexpected failure"
        ));
    }

    #[test]
    fn aggregate_toggle_propagates_wifi_before_other_results() {
        let result = finalize_airplane_toggle_results(
            Err(ConnectionError::InvalidInput {
                field: "wifi".into(),
                reason: "write failed".into(),
            }),
            Err(ConnectionError::InvalidInput {
                field: "wwan".into(),
                reason: "write failed".into(),
            }),
            Ok(()),
            true,
        );

        assert!(matches!(
            result,
            Err(ConnectionError::InvalidInput { field, reason })
                if field == "wifi" && reason == "write failed"
        ));
    }

    #[test]
    fn aggregate_toggle_propagates_wwan_when_wifi_succeeds() {
        let result = finalize_airplane_toggle_results(
            Ok(()),
            Err(ConnectionError::InvalidInput {
                field: "wwan".into(),
                reason: "write failed".into(),
            }),
            Ok(()),
            true,
        );

        assert!(matches!(
            result,
            Err(ConnectionError::InvalidInput { field, reason })
                if field == "wwan" && reason == "write failed"
        ));
    }

    #[test]
    fn aggregate_toggle_succeeds_when_every_toggle_succeeds() {
        assert!(matches!(
            finalize_airplane_toggle_results(Ok(()), Ok(()), Ok(()), false),
            Ok(())
        ));
    }

    #[test]
    fn hardware_reconciliation_trusts_any_disabled_source() {
        assert!(reconcile_hardware(true, false, "wifi"));
        assert!(!reconcile_hardware(true, true, "wifi"));
        assert!(!reconcile_hardware(false, false, "wifi"));
        assert!(!reconcile_hardware(false, true, "wifi"));
    }
}
