use std::collections::HashMap;
use std::future::Future;
use std::panic::{AssertUnwindSafe, resume_unwind};
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::Duration;

use futures::{FutureExt, StreamExt};
use nmrs::agent::{SecretAgent, SecretAgentFlags, SecretAgentHandle, SecretSetting};
use nmrs::builders::WireGuardBuilder;
use nmrs::raw::zvariant::{OwnedObjectPath, OwnedValue, Value};
use nmrs::{
    ActiveConnection, ActiveConnectionState, ConnectionError, DeviceState, MonitorHandle,
    NetworkEvent, NetworkEventStream, NetworkManager, SettingsChange, SettingsEventStream,
    SettingsPatch, SettingsSummary, TimeoutConfig, WifiKeyMgmt, WifiScope, WifiSecurity,
    WireGuardPeer,
};
use serial_test::serial;
use tokio::time::{sleep, timeout};
use uuid::Uuid;

const DBUS_TIMEOUT: Duration = Duration::from_secs(10);
const EVENT_TIMEOUT: Duration = Duration::from_secs(10);
const WIFI_TIMEOUT: Duration = Duration::from_secs(50);

fn required_env(name: &str) -> String {
    match std::env::var(name) {
        Ok(value) if !value.trim().is_empty() => value,
        Ok(_) => panic!("{name} must not be empty"),
        Err(error) => panic!(
            "{name} is required for this ignored integration test ({error}); use the isolated test harness"
        ),
    }
}

fn required_capability(name: &str) {
    let value = required_env(name);
    assert_eq!(value, "1", "{name} must be set to 1, got {value:?}");
}

async fn bounded<T>(
    description: &str,
    duration: Duration,
    operation: impl Future<Output = T>,
) -> T {
    timeout(duration, operation)
        .await
        .unwrap_or_else(|_| panic!("timed out after {duration:?}: {description}"))
}

async fn network_manager() -> NetworkManager {
    required_capability("NMRS_REQUIRE_NETWORKMANAGER");

    let config = TimeoutConfig::new()
        .with_connection_timeout(Duration::from_secs(40))
        .with_disconnect_timeout(Duration::from_secs(15));
    bounded(
        "connect to the system D-Bus and NetworkManager",
        DBUS_TIMEOUT,
        NetworkManager::with_config(config),
    )
    .await
    .expect("the harness declared NetworkManager available, but initialization failed")
}

async fn next_settings_change(
    stream: &mut SettingsEventStream,
    description: &str,
    mut matches: impl FnMut(&SettingsChange) -> bool,
) -> SettingsChange {
    timeout(EVENT_TIMEOUT, async {
        loop {
            match stream.next().await {
                Some(Ok(change)) if matches(&change) => return change,
                Some(Ok(_)) => {}
                Some(Err(error)) => panic!("settings event stream failed: {error}"),
                None => panic!("settings event stream ended before {description}"),
            }
        }
    })
    .await
    .unwrap_or_else(|_| panic!("timed out waiting for {description}"))
}

async fn next_network_event(
    stream: &mut NetworkEventStream,
    description: &str,
    mut matches: impl FnMut(&NetworkEvent) -> bool,
) -> NetworkEvent {
    timeout(EVENT_TIMEOUT, async {
        loop {
            match stream.next().await {
                Some(Ok(event)) if matches(&event) => return event,
                Some(Ok(_)) => {}
                Some(Err(error)) => panic!("network event stream failed: {error}"),
                None => panic!("network event stream ended before {description}"),
            }
        }
    })
    .await
    .unwrap_or_else(|_| panic!("timed out waiting for {description}"))
}

fn change_has_path(change: &SettingsChange, expected_kind: &str, expected_path: &str) -> bool {
    match (expected_kind, change) {
        ("added", SettingsChange::Added { path })
        | ("updated", SettingsChange::Updated { path })
        | ("removed", SettingsChange::Removed { path }) => path.as_str() == expected_path,
        _ => false,
    }
}

async fn cleanup_saved_profile(nm: &NetworkManager, uuid: &str) -> Vec<String> {
    match timeout(DBUS_TIMEOUT, nm.delete_saved_connection(uuid)).await {
        Ok(Ok(())) => Vec::new(),
        Ok(Err(ConnectionError::SavedConnectionNotFound(missing))) if missing == uuid => Vec::new(),
        Ok(Err(error)) => vec![format!("delete saved profile {uuid}: {error}")],
        Err(_) => vec![format!("delete saved profile {uuid}: timed out")],
    }
}

async fn cleanup_wifi_profile(wifi: &WifiScope, ssid: &str) -> Vec<String> {
    let mut failures = Vec::new();

    match timeout(DBUS_TIMEOUT, wifi.disconnect()).await {
        Ok(Ok(())) => {}
        Ok(Err(error)) => failures.push(format!("disconnect {ssid:?}: {error}")),
        Err(_) => failures.push(format!("disconnect {ssid:?}: timed out")),
    }
    match timeout(WIFI_TIMEOUT, wifi.forget(ssid)).await {
        Ok(Ok(())) => {}
        Ok(Err(error)) => failures.push(format!("forget {ssid:?}: {error}")),
        Err(_) => failures.push(format!("forget {ssid:?}: timed out")),
    }

    failures
}

async fn disconnect_device(nm: &NetworkManager, interface: &str) -> nmrs::Result<()> {
    let path = nm.get_device_by_interface(interface).await?;
    let proxy = nmrs::raw::zbus::Proxy::new(
        nm.dbus_connection(),
        "org.freedesktop.NetworkManager",
        path,
        "org.freedesktop.NetworkManager.Device",
    )
    .await?;
    let state = DeviceState::from(proxy.get_property::<u32>("State").await?);
    if matches!(
        state,
        DeviceState::Unmanaged | DeviceState::Unavailable | DeviceState::Disconnected
    ) {
        return Ok(());
    }

    proxy.call_method("Disconnect", &()).await?;
    loop {
        let state = DeviceState::from(proxy.get_property::<u32>("State").await?);
        if matches!(state, DeviceState::Unavailable | DeviceState::Disconnected) {
            return Ok(());
        }
        sleep(Duration::from_millis(25)).await;
    }
}

async fn cleanup_wired_profile(nm: &NetworkManager, interface: &str) -> Vec<String> {
    let mut failures = Vec::new();
    match timeout(DBUS_TIMEOUT, disconnect_device(nm, interface)).await {
        Ok(Ok(())) => {}
        Ok(Err(error)) => failures.push(format!("disconnect {interface}: {error}")),
        Err(_) => failures.push(format!("disconnect {interface}: timed out")),
    }

    match timeout(DBUS_TIMEOUT, nm.get_saved_connection_uuid(interface)).await {
        Ok(Ok(Some(uuid))) => failures.extend(cleanup_saved_profile(nm, &uuid).await),
        Ok(Ok(None)) => {}
        Ok(Err(error)) => failures.push(format!("resolve {interface} profile: {error}")),
        Err(_) => failures.push(format!("resolve {interface} profile: timed out")),
    }

    failures
}

async fn cleanup_vpn_profile(nm: &NetworkManager, uuid: &str) -> Vec<String> {
    let mut failures = Vec::new();
    match timeout(DBUS_TIMEOUT, nm.disconnect_vpn_by_uuid(uuid)).await {
        Ok(Ok(())) => {}
        Ok(Err(ConnectionError::VpnNotFound(missing))) if missing == uuid => {}
        Ok(Err(error)) => failures.push(format!("disconnect VPN {uuid}: {error}")),
        Err(_) => failures.push(format!("disconnect VPN {uuid}: timed out")),
    }
    failures.extend(cleanup_saved_profile(nm, uuid).await);
    failures
}

async fn cleanup_secret_agent(handle: SecretAgentHandle) -> Option<String> {
    match timeout(DBUS_TIMEOUT, handle.unregister()).await {
        Ok(Ok(())) => None,
        Ok(Err(error)) => Some(format!("unregister secret agent: {error}")),
        Err(_) => Some("unregister secret agent: timed out".into()),
    }
}

#[derive(Debug)]
struct RawActiveConnection {
    path: OwnedObjectPath,
    connection_path: OwnedObjectPath,
    id: String,
    connection_type: String,
    state: u32,
}

async fn raw_active_connection(
    nm: &NetworkManager,
    uuid: &str,
) -> nmrs::Result<Option<RawActiveConnection>> {
    let active_paths = raw_active_paths(nm).await?;

    for path in active_paths {
        let active = nmrs::raw::zbus::Proxy::new(
            nm.dbus_connection(),
            "org.freedesktop.NetworkManager",
            path.clone(),
            "org.freedesktop.NetworkManager.Connection.Active",
        )
        .await?;
        if active.get_property::<String>("Uuid").await? != uuid {
            continue;
        }

        return Ok(Some(RawActiveConnection {
            path,
            connection_path: active.get_property("Connection").await?,
            id: active.get_property("Id").await?,
            connection_type: active.get_property("Type").await?,
            state: active.get_property("State").await?,
        }));
    }

    Ok(None)
}

async fn raw_active_paths(nm: &NetworkManager) -> nmrs::Result<Vec<OwnedObjectPath>> {
    let manager = nmrs::raw::zbus::Proxy::new(
        nm.dbus_connection(),
        "org.freedesktop.NetworkManager",
        "/org/freedesktop/NetworkManager",
        "org.freedesktop.NetworkManager",
    )
    .await?;
    manager
        .get_property::<Vec<OwnedObjectPath>>("ActiveConnections")
        .await
        .map_err(Into::into)
}

async fn stop_monitor(description: &str, handle: MonitorHandle) -> Option<String> {
    match timeout(DBUS_TIMEOUT, handle.stop()).await {
        Ok(Ok(())) => None,
        Ok(Err(error)) => Some(format!("stop {description}: {error}")),
        Err(_) => Some(format!("stop {description}: timed out")),
    }
}

fn finish_after_cleanup(
    outcome: Result<(), Box<dyn std::any::Any + Send>>,
    cleanup_failures: Vec<String>,
) {
    if let Err(payload) = outcome {
        for failure in cleanup_failures {
            eprintln!("cleanup after integration-test panic failed: {failure}");
        }
        resume_unwind(payload);
    }

    assert!(
        cleanup_failures.is_empty(),
        "integration cleanup failed: {}",
        cleanup_failures.join("; ")
    );
}

async fn active_connections(nm: &NetworkManager) -> Vec<ActiveConnection> {
    bounded(
        "list typed active connections",
        DBUS_TIMEOUT,
        nm.list_active_connections(),
    )
    .await
    .expect("failed to list typed active connections")
}

/// Exercises NetworkManager's settings API against the isolated D-Bus harness.
///
/// This is ignored intentionally: a normal `cargo test` must never discover or
/// mutate the developer's host NetworkManager. The CI/Docker harness opts in.
#[tokio::test]
#[serial]
#[ignore = "requires NMRS_REQUIRE_NETWORKMANAGER=1 and an isolated NetworkManager"]
async fn networkmanager_profile_crud_and_settings_events() {
    let nm = network_manager().await;
    let mut events = bounded(
        "subscribe to saved-connection settings events",
        DBUS_TIMEOUT,
        nm.settings_events(),
    )
    .await
    .expect("failed to subscribe to saved-connection settings events");
    let mut network_events = bounded(
        "subscribe to unified NetworkManager events",
        DBUS_TIMEOUT,
        nm.network_events(),
    )
    .await
    .expect("failed to subscribe to unified NetworkManager events");

    let id = format!("nmrs-integration-{}", Uuid::new_v4());
    let renamed_id = format!("{id}-updated");
    let uuid = Uuid::new_v4();
    let uuid_string = uuid.to_string();
    let outcome = AssertUnwindSafe(async {
        let settings = WireGuardBuilder::new(&id)
            .private_key("YBk6X3pP8KjKz7+HFWzVHNqL3qTZq8hX9VxFQJ4zVmM=")
            .address("10.203.0.2/24")
            .add_peer(WireGuardPeer::new(
                "HIgo9xNzJMWLKAShlKl6/bUT1VI9Q0SDBXGtLXkPFXc=",
                "192.0.2.1:51820",
                vec!["10.204.0.0/16".into()],
            ))
            .mtu(1380)
            .uuid(uuid)
            .autoconnect(false)
            .build()
            .expect("the integration WireGuard profile must be valid");

        let path = bounded(
            "add a WireGuard settings profile",
            DBUS_TIMEOUT,
            nm.add_connection(settings),
        )
        .await
        .expect("NetworkManager rejected a valid WireGuard settings profile");
        let path_string = path.as_str().to_owned();

        let added = next_settings_change(&mut events, "the profile Added event", |change| {
            change_has_path(change, "added", &path_string)
        })
        .await;
        assert!(
            matches!(added, SettingsChange::Added { .. }),
            "expected an Added event, got {added:?}"
        );
        let unified_added = next_network_event(
            &mut network_events,
            "the unified SettingsChanged(Added) event",
            |event| {
                matches!(
                    event,
                    NetworkEvent::SettingsChanged(SettingsChange::Added { path })
                        if path.as_str() == path_string
                )
            },
        )
        .await;
        assert!(
            matches!(
                unified_added,
                NetworkEvent::SettingsChanged(SettingsChange::Added { ref path })
                    if path.as_str() == path_string
            ),
            "expected the exact unified Added event, got {unified_added:?}"
        );

        let brief = bounded(
            "list saved connection identities",
            DBUS_TIMEOUT,
            nm.list_saved_connections_brief(),
        )
        .await
        .expect("failed to list saved connection identities")
        .into_iter()
        .find(|profile| profile.uuid == uuid_string)
        .expect("the newly added profile was absent from the brief listing");
        assert_eq!(brief.path, path);
        assert_eq!(brief.id, id);
        assert_eq!(brief.connection_type, "wireguard");

        let profile = bounded(
            "decode the saved WireGuard profile",
            DBUS_TIMEOUT,
            nm.get_saved_connection(&uuid_string),
        )
        .await
        .expect("failed to load the newly added WireGuard profile");
        assert_eq!(profile.path, path);
        assert_eq!(profile.id, id);
        assert_eq!(profile.connection_type, "wireguard");
        assert!(!profile.autoconnect);
        match profile.summary {
            SettingsSummary::WireGuard {
                mtu,
                peer_count,
                first_peer_endpoint,
                ..
            } => {
                assert_eq!(mtu, Some(1380));
                assert_eq!(peer_count, 1);
                assert_eq!(first_peer_endpoint.as_deref(), Some("192.0.2.1:51820"));
            }
            other => panic!("expected a WireGuard settings summary, got {other:?}"),
        }

        let mut patch = SettingsPatch::default();
        patch.id = Some(renamed_id.clone());
        patch.autoconnect = Some(true);
        patch.autoconnect_priority = Some(42);
        bounded(
            "update the saved profile",
            DBUS_TIMEOUT,
            nm.update_saved_connection(&uuid_string, patch),
        )
        .await
        .expect("failed to update the saved profile");

        let updated_event = next_settings_change(&mut events, "the profile Updated event", |change| {
            change_has_path(change, "updated", &path_string)
        })
        .await;
        assert!(
            matches!(updated_event, SettingsChange::Updated { .. }),
            "expected an Updated event, got {updated_event:?}"
        );
        let unified_updated = next_network_event(
            &mut network_events,
            "the unified SettingsChanged(Updated) event",
            |event| {
                matches!(
                    event,
                    NetworkEvent::SettingsChanged(SettingsChange::Updated { path })
                        if path.as_str() == path_string
                )
            },
        )
        .await;
        assert!(
            matches!(
                unified_updated,
                NetworkEvent::SettingsChanged(SettingsChange::Updated { ref path })
                    if path.as_str() == path_string
            ),
            "expected the exact unified Updated event, got {unified_updated:?}"
        );
        let updated = bounded(
            "reload the updated profile",
            DBUS_TIMEOUT,
            nm.get_saved_connection(&uuid_string),
        )
        .await
        .expect("failed to reload the updated profile");
        assert_eq!(updated.id, renamed_id);
        assert!(updated.autoconnect);
        assert_eq!(updated.autoconnect_priority, 42);

        bounded(
            "delete the saved profile",
            DBUS_TIMEOUT,
            nm.delete_saved_connection(&uuid_string),
        )
        .await
        .expect("failed to delete the saved profile");
        let removed_event = next_settings_change(&mut events, "the profile Removed event", |change| {
            change_has_path(change, "removed", &path_string)
        })
        .await;
        assert!(
            matches!(removed_event, SettingsChange::Removed { .. }),
            "expected a Removed event, got {removed_event:?}"
        );
        let unified_removed = next_network_event(
            &mut network_events,
            "the unified SettingsChanged(Removed) event",
            |event| {
                matches!(
                    event,
                    NetworkEvent::SettingsChanged(SettingsChange::Removed { path })
                        if path.as_str() == path_string
                )
            },
        )
        .await;
        assert!(
            matches!(
                unified_removed,
                NetworkEvent::SettingsChanged(SettingsChange::Removed { ref path })
                    if path.as_str() == path_string
            ),
            "expected the exact unified Removed event, got {unified_removed:?}"
        );

        let ids = bounded(
            "list profiles after deletion",
            DBUS_TIMEOUT,
            nm.list_saved_connection_ids(),
        )
        .await
        .expect("failed to list profiles after deletion");
        assert!(!ids.iter().any(|candidate| candidate == &renamed_id));

        let error = bounded(
            "load a deleted profile",
            DBUS_TIMEOUT,
            nm.get_saved_connection(&uuid_string),
        )
        .await
        .expect_err("loading a deleted profile must fail");
        assert!(
            matches!(error, ConnectionError::SavedConnectionNotFound(ref missing) if missing == &uuid_string),
            "expected SavedConnectionNotFound for {uuid_string}, got {error:?}"
        );
    })
    .catch_unwind()
    .await;

    let cleanup_failures = cleanup_saved_profile(&nm, &uuid_string).await;
    finish_after_cleanup(outcome, cleanup_failures);
}

/// Exercises a real NetworkManager-to-agent secret request while activating a
/// native WireGuard VPN, plus registration ownership and cleanup rules.
#[tokio::test]
#[serial]
#[ignore = "requires NMRS_REQUIRE_NETWORKMANAGER=1 and an isolated NetworkManager"]
async fn networkmanager_secret_agent_registration_lifecycle() {
    let nm = network_manager().await;
    let suffix = Uuid::new_v4().simple().to_string();
    let invalid_identifier = format!("com.nmrs:integration.Agent{suffix}");
    let invalid_error = match bounded(
        "reject an invalid secret-agent identifier",
        DBUS_TIMEOUT,
        SecretAgent::builder()
            .with_identifier(&invalid_identifier)
            .register(),
    )
    .await
    {
        Err(error) => error,
        Ok((handle, _requests)) => {
            bounded(
                "unregister unexpectedly accepted invalid agent",
                DBUS_TIMEOUT,
                handle.unregister(),
            )
            .await
            .expect("failed to clean up unexpectedly accepted invalid agent");
            panic!("NetworkManager accepted invalid agent identifier {invalid_identifier:?}");
        }
    };
    assert!(
        matches!(
            invalid_error,
            ConnectionError::AgentRegistration { ref context }
                if context.contains("registering secret agent")
                    && context.contains("InvalidIdentifier")
        ),
        "expected NetworkManager's InvalidIdentifier registration rejection, got {invalid_error:?}"
    );

    let identifier = format!("com.nmrs.integration.Agent{suffix}");
    let (handle, mut requests) = bounded(
        "register the first secret agent",
        DBUS_TIMEOUT,
        SecretAgent::builder()
            .with_identifier(&identifier)
            .register(),
    )
    .await
    .expect("failed to register the first secret agent");
    let mut active_handle = Some(handle);
    let profile_id = format!("nmrs-agent-wireguard-{suffix}");
    let profile_uuid = Uuid::new_v4().to_string();
    let private_key = "YBk6X3pP8KjKz7+HFWzVHNqL3qTZq8hX9VxFQJ4zVmM=";

    let outcome = AssertUnwindSafe(async {
        let duplicate_error = match bounded(
            "reject a duplicate secret-agent identifier",
            DBUS_TIMEOUT,
            SecretAgent::builder()
                .with_identifier(&identifier)
                .register(),
        )
        .await
        {
            Err(error) => error,
            Ok((duplicate, _duplicate_requests)) => {
                bounded(
                    "unregister unexpectedly accepted duplicate agent",
                    DBUS_TIMEOUT,
                    duplicate.unregister(),
                )
                .await
                .expect("failed to clean up unexpectedly accepted duplicate agent");
                panic!("NetworkManager accepted duplicate agent identifier {identifier:?}");
            }
        };
        assert!(
            matches!(duplicate_error, ConnectionError::AgentAlreadyRegistered),
            "expected AgentAlreadyRegistered for duplicate registration, got {duplicate_error:?}"
        );

        let reregister_error = bounded(
            "reject re-registration while the agent is active",
            DBUS_TIMEOUT,
            active_handle
                .as_ref()
                .expect("the primary agent handle disappeared")
                .reregister(),
        )
        .await
        .expect_err("an active secret agent must not re-register");
        assert!(
            matches!(reregister_error, ConnectionError::AgentAlreadyRegistered),
            "expected AgentAlreadyRegistered for active re-registration, got {reregister_error:?}"
        );

        let profile_uuid_value = Uuid::parse_str(&profile_uuid)
            .expect("the generated integration profile UUID must parse");
        let mut settings = WireGuardBuilder::new(&profile_id)
            .private_key(private_key)
            .address("10.207.0.2/24")
            .add_peer(WireGuardPeer::new(
                "HIgo9xNzJMWLKAShlKl6/bUT1VI9Q0SDBXGtLXkPFXc=",
                "192.0.2.1:51820",
                vec!["10.208.0.0/16".into()],
            ))
            .uuid(profile_uuid_value)
            .autoconnect(false)
            .build()
            .expect("the agent-owned WireGuard profile must be valid");
        let wireguard = settings
            .get_mut("wireguard")
            .expect("the WireGuard builder omitted its settings section");
        assert!(
            wireguard.remove("private-key").is_some(),
            "the WireGuard builder omitted its private key"
        );
        wireguard.insert("private-key-flags", Value::from(1u32));

        let profile_path = bounded(
            "add the agent-owned WireGuard profile",
            DBUS_TIMEOUT,
            nm.add_connection(settings),
        )
        .await
        .expect("NetworkManager rejected the agent-owned WireGuard profile");

        let missing_uuid = Uuid::new_v4().to_string();
        let missing_error = bounded(
            "reject activation of a missing VPN UUID",
            DBUS_TIMEOUT,
            nm.connect_vpn_by_uuid(&missing_uuid),
        )
        .await
        .expect_err("activation of a missing VPN UUID must fail");
        assert!(
            matches!(missing_error, ConnectionError::VpnNotFound(ref missing) if missing == &missing_uuid),
            "expected VpnNotFound for {missing_uuid}, got {missing_error:?}"
        );

        let get_secrets = async {
            let profile = nmrs::raw::zbus::Proxy::new(
                nm.dbus_connection(),
                "org.freedesktop.NetworkManager",
                profile_path.clone(),
                "org.freedesktop.NetworkManager.Settings.Connection",
            )
            .await
            .expect("failed to create the saved-profile D-Bus proxy");
            let reply = profile
                .call_method("GetSecrets", &("wireguard",))
                .await
                .expect("NetworkManager failed to route GetSecrets to the registered agent");
            reply
                .body()
                .deserialize::<HashMap<String, HashMap<String, OwnedValue>>>()
                .expect("NetworkManager returned a malformed GetSecrets reply")
        };
        let secret_exchange = async {
            let request = bounded(
                "receive NetworkManager's saved-profile GetSecrets request",
                DBUS_TIMEOUT,
                requests.next(),
            )
            .await
            .expect("the secret-agent request stream closed during activation");
            assert_eq!(request.connection_uuid, profile_uuid);
            assert_eq!(request.connection_id, profile_id);
            assert_eq!(request.connection_type, "wireguard");
            assert_eq!(request.connection_path, profile_path);
            assert!(
                matches!(request.setting, SecretSetting::Other(ref name) if name == "wireguard"),
                "expected a wireguard secret request, got {:?}",
                request.setting
            );
            assert_eq!(
                request.flags,
                SecretAgentFlags::USER_REQUESTED,
                "saved-profile GetSecrets used unexpected request flags: {:?}",
                request.flags,
            );
            assert!(request.hints.is_empty());
            assert!(request.existing_secrets.is_empty());

            let mut reply = HashMap::new();
            reply.insert(
                "private-key".into(),
                OwnedValue::from(nmrs::raw::zvariant::Str::from(private_key)),
            );
            request
                .responder
                .raw("wireguard", reply)
                .await
                .expect("failed to route the WireGuard secret reply to NetworkManager");
        };
        let (returned_secrets, ()) = tokio::join!(get_secrets, secret_exchange);
        let returned_private_key = <&str>::try_from(
            returned_secrets
                .get("wireguard")
                .and_then(|setting| setting.get("private-key"))
                .expect("the GetSecrets reply omitted wireguard.private-key"),
        )
        .expect("wireguard.private-key was not returned as a string");
        assert_eq!(returned_private_key, private_key);

        let mut wireguard_overlay = HashMap::new();
        wireguard_overlay.insert(
            "private-key".into(),
            OwnedValue::from(nmrs::raw::zvariant::Str::from(returned_private_key)),
        );
        wireguard_overlay.insert("private-key-flags".into(), OwnedValue::from(0u32));
        let mut overlay = HashMap::new();
        overlay.insert("wireguard".into(), wireguard_overlay);
        let mut patch = SettingsPatch::default();
        patch.raw_overlay = Some(overlay);
        bounded(
            "persist the agent-provided WireGuard private key",
            DBUS_TIMEOUT,
            nm.update_saved_connection(&profile_uuid, patch),
        )
        .await
        .expect("failed to persist the agent-provided WireGuard private key");

        bounded(
            "activate the configured WireGuard profile",
            WIFI_TIMEOUT,
            nm.connect_vpn_by_uuid(&profile_uuid),
        )
        .await
        .expect("WireGuard activation failed after persisting the agent-provided key");

        let raw_active = bounded(
            "inspect the active WireGuard connection over D-Bus",
            DBUS_TIMEOUT,
            raw_active_connection(&nm, &profile_uuid),
        )
        .await
        .expect("failed to inspect active connections over D-Bus")
        .expect("NetworkManager omitted the activated WireGuard connection");
        assert_ne!(raw_active.path.as_str(), "/");
        assert_eq!(raw_active.connection_path, profile_path);
        assert_eq!(raw_active.id, profile_id);
        assert_eq!(raw_active.connection_type, "wireguard");
        assert_eq!(raw_active.state, 2, "raw active state was not Activated");

        let mut last_active = Vec::new();
        let active_vpn = timeout(EVENT_TIMEOUT, async {
            loop {
                last_active = active_connections(&nm).await;
                if let Some(vpn) = last_active.iter().find_map(|connection| match connection {
                    ActiveConnection::Vpn(vpn)
                        if vpn.uuid == profile_uuid
                            && vpn.state == ActiveConnectionState::Activated
                            && vpn.interface.as_deref() == Some("wg-nmrs-agent")
                            && vpn
                                .ip4_address
                                .as_deref()
                                .is_some_and(|address| address.starts_with("10.207.0.2/")) =>
                    {
                        Some(vpn.clone())
                    }
                    _ => None,
                }) {
                    break vpn;
                }
                sleep(Duration::from_millis(25)).await;
            }
        })
        .await
        .unwrap_or_else(|_| {
            panic!(
                "typed active connections never exposed the configured WireGuard state: {last_active:?}"
            )
        });
        assert_eq!(active_vpn.id, profile_id);
        assert_eq!(active_vpn.state, ActiveConnectionState::Activated);
        assert_eq!(active_vpn.interface.as_deref(), Some("wg-nmrs-agent"));
        assert!(
            active_vpn
                .ip4_address
                .as_deref()
                .is_some_and(|address| address.starts_with("10.207.0.2/")),
            "typed VPN connection omitted its configured address: {active_vpn:?}"
        );

        bounded(
            "deactivate the WireGuard VPN",
            DBUS_TIMEOUT,
            nm.disconnect_vpn_by_uuid(&profile_uuid),
        )
        .await
        .expect("failed to deactivate the WireGuard VPN");
        timeout(EVENT_TIMEOUT, async {
            loop {
                let raw_absent = raw_active_paths(&nm)
                    .await
                    .expect("failed to inspect D-Bus state after VPN deactivation")
                    .iter()
                    .all(|path| path != &raw_active.path);
                if raw_absent {
                    let typed_absent =
                        !active_connections(&nm).await.iter().any(|connection| {
                            matches!(connection, ActiveConnection::Vpn(vpn) if vpn.uuid == profile_uuid)
                        });
                    if typed_absent {
                        break;
                    }
                }
                sleep(Duration::from_millis(25)).await;
            }
        })
        .await
        .expect("WireGuard remained active after D-Bus deactivation");

        bounded(
            "delete the agent-owned WireGuard profile",
            DBUS_TIMEOUT,
            nm.delete_saved_connection(&profile_uuid),
        )
        .await
        .expect("failed to delete the agent-owned WireGuard profile");

        let primary = active_handle
            .take()
            .expect("the primary agent handle disappeared before unregister");
        bounded(
            "unregister the first secret agent",
            DBUS_TIMEOUT,
            primary.unregister(),
        )
        .await
        .expect("failed to unregister the first secret agent");
        assert!(
            bounded(
                "wait for the first request stream to close",
                DBUS_TIMEOUT,
                requests.next(),
            )
            .await
            .is_none(),
            "secret request stream remained open after unregister"
        );

        let (replacement, mut replacement_requests) = bounded(
            "re-register the released secret-agent identifier",
            DBUS_TIMEOUT,
            SecretAgent::builder()
                .with_identifier(&identifier)
                .register(),
        )
        .await
        .expect("the identifier was not released after unregister");
        active_handle = Some(replacement);
        let replacement = active_handle
            .take()
            .expect("the replacement agent handle disappeared before unregister");
        bounded(
            "unregister the replacement secret agent",
            DBUS_TIMEOUT,
            replacement.unregister(),
        )
        .await
        .expect("failed to unregister the replacement secret agent");
        assert!(
            bounded(
                "wait for the replacement request stream to close",
                DBUS_TIMEOUT,
                replacement_requests.next(),
            )
            .await
            .is_none(),
            "replacement request stream remained open after unregister"
        );
    })
    .catch_unwind()
    .await;

    let mut cleanup_failures = cleanup_vpn_profile(&nm, &profile_uuid).await;
    if let Some(handle) = active_handle.take()
        && let Some(failure) = cleanup_secret_agent(handle).await
    {
        cleanup_failures.push(failure);
    }
    finish_after_cleanup(outcome, cleanup_failures);
}

/// Exercises a deterministic veth/DHCP wired connection without touching the
/// container's Docker-provided `eth0` interface.
#[tokio::test]
#[serial]
#[ignore = "requires NMRS_REQUIRE_WIRED=1 and the isolated veth harness"]
async fn wired_connection_lifecycle() {
    required_capability("NMRS_REQUIRE_WIRED");
    let interface = required_env("NMRS_WIRED_INTERFACE");
    let nm = network_manager().await;

    let outcome = AssertUnwindSafe(async {
        let devices = bounded("list wired devices", DBUS_TIMEOUT, nm.list_wired_devices())
            .await
            .expect("failed to list wired devices");
        let device = devices
            .iter()
            .find(|device| device.interface == interface)
            .unwrap_or_else(|| {
                panic!("managed veth interface {interface:?} was missing: {devices:?}")
            });
        assert_eq!(device.managed, Some(true));
        assert!(!device.path.is_empty());

        let details = bounded(
            "list detailed wired devices",
            DBUS_TIMEOUT,
            nm.list_wired_device_details(),
        )
        .await
        .expect("failed to list detailed wired devices");
        let detail = details
            .iter()
            .find(|device| device.interface == interface)
            .expect("managed veth was absent from detailed wired devices");
        assert!(!detail.path.is_empty());
        assert!(!detail.hw_address.is_empty());
        assert!(detail.active_connection_id.is_none());

        bounded(
            "connect the managed veth client",
            WIFI_TIMEOUT,
            nm.connect_wired(),
        )
        .await
        .expect("wired activation or DHCP failed");
        let saved_uuid = bounded(
            "resolve the wired profile UUID",
            DBUS_TIMEOUT,
            nm.get_saved_connection_uuid(&interface),
        )
        .await
        .expect("failed to resolve the wired profile UUID")
        .expect("wired activation did not create a saved profile");

        let active = active_connections(&nm).await;
        let active_wired = active
            .iter()
            .find_map(|connection| match connection {
                ActiveConnection::Wired(wired)
                    if wired.interface.as_deref() == Some(interface.as_str()) =>
                {
                    Some(wired.clone())
                }
                _ => None,
            })
            .unwrap_or_else(|| {
                panic!("typed active connections omitted the veth connection: {active:?}")
            });
        assert_eq!(active_wired.id, interface);
        assert_eq!(active_wired.uuid, saved_uuid);
        assert_eq!(active_wired.state, ActiveConnectionState::Activated);
        assert!(
            active_wired
                .ip4_address
                .as_deref()
                .is_some_and(|address| address.starts_with("192.168.251.")),
            "typed wired connection omitted its DHCP address"
        );

        let connected_details = bounded(
            "read connected wired details",
            DBUS_TIMEOUT,
            nm.list_wired_device_details(),
        )
        .await
        .expect("failed to read connected wired details");
        let connected = connected_details
            .iter()
            .find(|device| device.interface == interface)
            .expect("connected veth was absent from detailed wired devices");
        assert_eq!(connected.state, DeviceState::Activated);
        assert_eq!(
            connected.active_connection_id.as_deref(),
            Some(interface.as_str())
        );
        assert!(
            connected
                .ip4_address
                .as_deref()
                .is_some_and(|address| address.starts_with("192.168.251."))
        );

        bounded(
            "disconnect the managed veth client",
            DBUS_TIMEOUT,
            disconnect_device(&nm, &interface),
        )
        .await
        .expect("failed to disconnect the managed veth client");
        timeout(EVENT_TIMEOUT, async {
            loop {
                if !active_connections(&nm).await.iter().any(|connection| {
                    matches!(connection, ActiveConnection::Wired(wired) if wired.uuid == saved_uuid)
                }) {
                    break;
                }
                sleep(Duration::from_millis(25)).await;
            }
        })
        .await
        .expect("typed wired connection remained active after disconnect");

        let disconnected_details = bounded(
            "read disconnected wired details",
            DBUS_TIMEOUT,
            nm.list_wired_device_details(),
        )
        .await
        .expect("failed to read disconnected wired details");
        let disconnected = disconnected_details
            .iter()
            .find(|device| device.interface == interface)
            .expect("disconnected veth was absent from detailed wired devices");
        assert_eq!(disconnected.state, DeviceState::Disconnected);
        assert!(disconnected.active_connection_id.is_none());

        bounded(
            "delete the wired profile",
            DBUS_TIMEOUT,
            nm.delete_saved_connection(&saved_uuid),
        )
        .await
        .expect("failed to delete the wired profile");
        assert!(
            bounded(
                "resolve wired profile after deletion",
                DBUS_TIMEOUT,
                nm.get_saved_connection_uuid(&interface),
            )
            .await
            .expect("failed to resolve wired profile after deletion")
            .is_none()
        );
    })
    .catch_unwind()
    .await;

    let cleanup_failures = cleanup_wired_profile(&nm, &interface).await;
    finish_after_cleanup(outcome, cleanup_failures);
}

/// Proves discovery, WPA authentication, DHCP, saved-secret reuse, and cleanup
/// against the deterministic mac80211_hwsim access point.
#[tokio::test]
#[serial]
#[ignore = "requires the isolated mac80211_hwsim WiFi harness"]
async fn wifi_wpa_saved_connection_lifecycle() {
    required_capability("NMRS_REQUIRE_WIFI");
    let interface = required_env("NMRS_WIFI_INTERFACE");
    let ssid = required_env("NMRS_EXPECT_WIFI_SSID");
    let absent_ssid = format!("{ssid}-absent");
    let password = required_env("NMRS_WIFI_PASSWORD");
    assert!(
        (8..=63).contains(&password.len()),
        "NMRS_WIFI_PASSWORD must be a valid WPA passphrase"
    );

    let nm = network_manager().await;
    let initial_wifi_enabled = bounded(
        "read the initial WiFi radio state",
        DBUS_TIMEOUT,
        nm.wifi_state(),
    )
    .await
    .expect("failed to capture the WiFi radio state before the test")
    .enabled;
    let wifi = nm.wifi(&interface);
    let device_callback_count = Arc::new(AtomicUsize::new(0));
    let callback_count = Arc::clone(&device_callback_count);
    let network_callback_count = Arc::new(AtomicUsize::new(0));
    let network_callback = Arc::clone(&network_callback_count);
    let mut device_monitor = None;
    let mut network_monitor = None;

    let outcome = AssertUnwindSafe(async {
        device_monitor = Some(
            bounded(
                "start the WiFi device callback monitor",
                DBUS_TIMEOUT,
                nm.monitor_device_changes(move || {
                    callback_count.fetch_add(1, Ordering::SeqCst);
                }),
            )
            .await
            .expect("the WiFi device callback monitor did not become ready"),
        );
        network_monitor = Some(
            bounded(
                "start the WiFi network callback monitor",
                DBUS_TIMEOUT,
                nm.monitor_network_changes(move || {
                    network_callback.fetch_add(1, Ordering::SeqCst);
                }),
            )
            .await
            .expect("the WiFi network callback monitor did not become ready"),
        );

        bounded(
            "enable the WiFi radio",
            DBUS_TIMEOUT,
            nm.set_wireless_enabled(true),
        )
        .await
        .expect("the harness declared WiFi available, but enabling it failed");
        bounded(
            "wait for the WiFi device to become ready",
            DBUS_TIMEOUT,
            nm.wait_for_wifi_ready(),
        )
        .await
        .expect("the harness WiFi device did not become ready");

        let devices = bounded(
            "list wireless devices",
            DBUS_TIMEOUT,
            nm.list_wireless_devices(),
        )
        .await
        .expect("failed to list wireless devices");
        let device = devices
            .iter()
            .find(|device| device.interface == interface)
            .unwrap_or_else(|| {
                panic!("harness WiFi interface {interface:?} was not managed: {devices:?}")
            });
        assert!(!device.path.is_empty());
        assert_eq!(device.managed, Some(true));

        bounded(
            "remove any stale test profile",
            DBUS_TIMEOUT,
            wifi.forget(&ssid),
        )
        .await
        .expect("failed to remove a stale test profile");
        bounded(
            "remove any stale absent-network profile",
            DBUS_TIMEOUT,
            wifi.forget(&absent_ssid),
        )
        .await
        .expect("failed to remove a stale absent-network profile");

        let absent_error = bounded(
            "reject an absent SSID",
            WIFI_TIMEOUT,
            wifi.connect(
                &absent_ssid,
                WifiSecurity::WpaPsk {
                    psk: password.clone(),
                },
            ),
        )
        .await
        .expect_err("connecting to an absent SSID must fail");
        assert!(
            matches!(absent_error, ConnectionError::NotFound),
            "expected NotFound for absent SSID, got {absent_error:?}"
        );
        assert!(
            !bounded(
                "check absent SSID profile",
                DBUS_TIMEOUT,
                nm.has_saved_connection(&absent_ssid),
            )
            .await
            .expect("failed to check the absent SSID profile"),
            "an absent SSID created a saved profile"
        );

        bounded("scan for the harness AP", DBUS_TIMEOUT, wifi.scan())
            .await
            .expect("the harness WiFi scan failed");
        let network = timeout(Duration::from_secs(15), async {
            loop {
                let networks = wifi
                    .list_networks()
                    .await
                    .expect("listing WiFi scan results failed");
                if let Some(network) = networks.into_iter().find(|network| network.ssid == ssid) {
                    return network;
                }
                sleep(Duration::from_millis(500)).await;
            }
        })
        .await
        .unwrap_or_else(|_| panic!("the expected access point {ssid:?} was not discovered"));
        assert_eq!(network.device, interface);
        assert!(network.secured);
        assert!(network.is_psk);
        assert!(!network.is_eap);
        assert!(!network.best_bssid.is_empty());
        assert!(
            network
                .bssids
                .iter()
                .any(|bssid| bssid == &network.best_bssid)
        );

        network_callback_count.store(0, Ordering::SeqCst);
        bounded(
            "disable WiFi to remove the monitored access point",
            DBUS_TIMEOUT,
            nm.set_wireless_enabled(false),
        )
        .await
        .expect("failed to disable WiFi for the network-monitor contract");
        timeout(EVENT_TIMEOUT, async {
            while network_callback_count.load(Ordering::SeqCst) == 0 {
                sleep(Duration::from_millis(25)).await;
            }
        })
        .await
        .expect("network callback was not delivered when the access point disappeared");
        bounded(
            "re-enable WiFi after the network-monitor contract",
            DBUS_TIMEOUT,
            nm.set_wireless_enabled(true),
        )
        .await
        .expect("failed to re-enable WiFi after the network-monitor contract");
        bounded(
            "wait for WiFi after the network-monitor contract",
            DBUS_TIMEOUT,
            nm.wait_for_wifi_ready(),
        )
        .await
        .expect("the WiFi device did not recover after re-enabling it");
        bounded(
            "rescan after the network-monitor contract",
            DBUS_TIMEOUT,
            wifi.scan(),
        )
        .await
        .expect("the post-monitor WiFi scan failed");
        let access_point = timeout(Duration::from_secs(15), async {
            loop {
                let access_points = wifi
                    .list_access_points()
                    .await
                    .expect("listing per-BSSID access points failed");
                if let Some(access_point) = access_points
                    .into_iter()
                    .find(|access_point| access_point.ssid == ssid)
                {
                    return access_point;
                }
                sleep(Duration::from_millis(500)).await;
            }
        })
        .await
        .unwrap_or_else(|_| panic!("the access point {ssid:?} did not return after re-enabling"));
        assert_eq!(access_point.interface, interface);
        assert_eq!(access_point.ssid_bytes, ssid.as_bytes());
        assert!(!access_point.bssid.is_empty());
        assert!(access_point.frequency_mhz > 0);
        assert!(access_point.security.psk);
        let expected_bssid = access_point.bssid.clone();

        let wrong_psk_error = bounded(
            "reject an incorrect WPA passphrase",
            WIFI_TIMEOUT,
            wifi.connect(
                &ssid,
                WifiSecurity::WpaPsk {
                    psk: "nmrs-definitely-wrong-password".into(),
                },
            ),
        )
        .await
        .expect_err("an incorrect WPA passphrase must fail");
        assert!(
            matches!(wrong_psk_error, ConnectionError::AuthFailed),
            "expected AuthFailed for an incorrect WPA passphrase, got {wrong_psk_error:?}"
        );
        assert!(
            !bounded(
                "check state after rejected WPA authentication",
                DBUS_TIMEOUT,
                nm.is_connected(&ssid),
            )
            .await
            .expect("failed to query state after rejected WPA authentication")
        );
        assert!(
            !active_connections(&nm)
                .await
                .iter()
                .any(|active| matches!(active, ActiveConnection::Wifi(wifi) if wifi.ssid == ssid)),
            "rejected WPA authentication left an active WiFi connection"
        );
        assert!(
            !bounded(
                "check profile after rejected WPA authentication",
                DBUS_TIMEOUT,
                nm.has_saved_connection(&ssid),
            )
            .await
            .expect("failed to query profile after rejected WPA authentication"),
            "rejected WPA authentication left a saved bad profile"
        );

        device_callback_count.store(0, Ordering::SeqCst);
        bounded(
            "connect to the WPA access point",
            WIFI_TIMEOUT,
            wifi.connect(
                &ssid,
                WifiSecurity::WpaPsk {
                    psk: password.clone(),
                },
            ),
        )
        .await
        .expect("WPA authentication or DHCP activation failed");
        timeout(EVENT_TIMEOUT, async {
            while device_callback_count.load(Ordering::SeqCst) == 0 {
                sleep(Duration::from_millis(25)).await;
            }
        })
        .await
        .expect("device callback was not delivered during WiFi activation");
        assert!(
            bounded(
                "check connected state",
                DBUS_TIMEOUT,
                nm.is_connected(&ssid)
            )
            .await
            .expect("failed to query connected state")
        );
        let current_ssid = bounded("read the current SSID", DBUS_TIMEOUT, nm.current_ssid()).await;
        assert_eq!(current_ssid.as_deref(), Some(ssid.as_str()));

        let active = bounded(
            "read the active WiFi network",
            DBUS_TIMEOUT,
            nm.current_network(),
        )
        .await
        .expect("failed to read the active WiFi network")
        .expect("connect returned success without an active WiFi network");
        assert_eq!(active.ssid, ssid);
        assert_eq!(active.device, interface);
        assert!(active.is_active);
        let ip4_address = active
            .ip4_address
            .as_deref()
            .expect("successful activation did not acquire an IPv4 DHCP lease");
        assert!(
            ip4_address.starts_with("192.168.250."),
            "unexpected DHCP address {ip4_address:?}"
        );

        assert!(
            bounded(
                "check for the saved WiFi profile",
                DBUS_TIMEOUT,
                nm.has_saved_connection(&ssid),
            )
            .await
            .expect("failed to query the saved WiFi profile")
        );
        let saved_path = bounded(
            "resolve the saved WiFi path",
            DBUS_TIMEOUT,
            nm.get_saved_connection_path(&ssid),
        )
        .await
        .expect("failed to resolve the saved WiFi path")
        .expect("successful WPA connection did not create a saved profile");
        assert_ne!(saved_path.as_str(), "/");
        let saved_uuid = bounded(
            "resolve the saved WiFi UUID",
            DBUS_TIMEOUT,
            nm.get_saved_connection_uuid(&ssid),
        )
        .await
        .expect("failed to resolve the saved WiFi UUID")
        .expect("successful WPA connection had no saved UUID");
        let saved = bounded(
            "decode the saved WiFi profile",
            DBUS_TIMEOUT,
            nm.get_saved_connection(&saved_uuid),
        )
        .await
        .expect("failed to decode the saved WiFi profile");
        assert_eq!(saved.id, ssid);
        assert_eq!(saved.connection_type, "802-11-wireless");
        match saved.summary {
            SettingsSummary::Wifi {
                ssid: saved_ssid,
                security: Some(security),
                ..
            } => {
                assert_eq!(saved_ssid, ssid);
                assert_eq!(security.key_mgmt, WifiKeyMgmt::WpaPsk);
            }
            other => panic!("expected a WPA WiFi settings summary, got {other:?}"),
        }

        let active = active_connections(&nm).await;
        let typed_wifi = active
            .iter()
            .find_map(|connection| match connection {
                ActiveConnection::Wifi(wifi) if wifi.ssid == ssid => Some(wifi.clone()),
                _ => None,
            })
            .unwrap_or_else(|| {
                panic!("typed active connections omitted the connected WiFi network: {active:?}")
            });
        assert_eq!(typed_wifi.id, ssid);
        assert_eq!(typed_wifi.uuid, saved_uuid);
        assert_eq!(typed_wifi.ssid, ssid);
        assert_eq!(typed_wifi.interface.as_deref(), Some(interface.as_str()));
        assert_eq!(typed_wifi.bssid.as_deref(), Some(expected_bssid.as_str()));
        assert!(typed_wifi.strength.is_some());
        assert_eq!(typed_wifi.state, ActiveConnectionState::Activated);
        assert!(
            typed_wifi
                .ip4_address
                .as_deref()
                .is_some_and(|address| address.starts_with("192.168.250.")),
            "typed active WiFi connection omitted its DHCP address"
        );

        bounded(
            "disconnect the WiFi device",
            DBUS_TIMEOUT,
            wifi.disconnect(),
        )
        .await
        .expect("failed to disconnect after the initial WPA connection");
        assert!(
            !bounded(
                "check disconnected state",
                DBUS_TIMEOUT,
                nm.is_connected(&ssid),
            )
            .await
            .expect("failed to query disconnected state")
        );
        assert!(
            !active_connections(&nm).await.iter().any(
                |active| matches!(active, ActiveConnection::Wifi(wifi) if wifi.uuid == saved_uuid)
            ),
            "typed active WiFi connection remained after disconnect"
        );

        bounded(
            "reconnect with NetworkManager's saved PSK",
            WIFI_TIMEOUT,
            wifi.connect(&ssid, WifiSecurity::WpaPsk { psk: String::new() }),
        )
        .await
        .expect("saved-credential WPA reconnect failed");
        assert!(
            bounded(
                "check saved-credential reconnect",
                DBUS_TIMEOUT,
                nm.is_connected(&ssid),
            )
            .await
            .expect("failed to query the saved-credential reconnect")
        );

        bounded(
            "forget the active WiFi profile",
            WIFI_TIMEOUT,
            wifi.forget(&ssid),
        )
        .await
        .expect("failed to disconnect and forget the WiFi profile");
        assert!(
            !bounded(
                "check profile removal",
                DBUS_TIMEOUT,
                nm.has_saved_connection(&ssid),
            )
            .await
            .expect("failed to query profile removal")
        );
        assert!(
            bounded(
                "resolve path after forgetting",
                DBUS_TIMEOUT,
                nm.get_saved_connection_path(&ssid),
            )
            .await
            .expect("failed to resolve the profile path after forgetting")
            .is_none()
        );
        assert!(
            bounded(
                "resolve UUID after forgetting",
                DBUS_TIMEOUT,
                nm.get_saved_connection_uuid(&ssid),
            )
            .await
            .expect("failed to resolve the profile UUID after forgetting")
            .is_none()
        );

        let error = bounded(
            "reject an empty PSK without saved credentials",
            DBUS_TIMEOUT,
            wifi.connect(&ssid, WifiSecurity::WpaPsk { psk: String::new() }),
        )
        .await
        .expect_err("an empty PSK without a saved profile must fail");
        assert!(
            matches!(error, ConnectionError::MissingPassword),
            "expected MissingPassword after forgetting saved credentials, got {error:?}"
        );
    })
    .catch_unwind()
    .await;

    let mut cleanup_failures = cleanup_wifi_profile(&wifi, &ssid).await;
    match timeout(WIFI_TIMEOUT, wifi.forget(&absent_ssid)).await {
        Ok(Ok(())) => {}
        Ok(Err(error)) => cleanup_failures.push(format!("forget {absent_ssid:?}: {error}")),
        Err(_) => cleanup_failures.push(format!("forget {absent_ssid:?}: timed out")),
    }
    if let Some(handle) = device_monitor.take()
        && let Some(failure) = stop_monitor("WiFi device callback monitor", handle).await
    {
        cleanup_failures.push(failure);
    }
    if let Some(handle) = network_monitor.take()
        && let Some(failure) = stop_monitor("WiFi network callback monitor", handle).await
    {
        cleanup_failures.push(failure);
    }
    match timeout(DBUS_TIMEOUT, nm.set_wireless_enabled(initial_wifi_enabled)).await {
        Ok(Ok(())) => {}
        Ok(Err(error)) => cleanup_failures.push(format!(
            "restore WiFi radio enabled={initial_wifi_enabled}: {error}"
        )),
        Err(_) => cleanup_failures.push(format!(
            "restore WiFi radio enabled={initial_wifi_enabled}: timed out"
        )),
    }
    finish_after_cleanup(outcome, cleanup_failures);
}
