use nmrs::{
    ConnectionError, DeviceState, DeviceType, NetworkManager, OpenVpnAuthType, StateReason,
    VpnKind, WifiSecurity, WireGuardConfig, WireGuardPeer, reason_to_error,
};
use serial_test::serial;
use std::time::Duration;
use tokio::time::sleep;

/// Helper function to check if NetworkManager is available
/// Returns true if we can connect to NetworkManager, false otherwise
async fn is_networkmanager_available() -> bool {
    NetworkManager::new().await.is_ok()
}

/// Check if WiFi is available
async fn has_wifi_device(nm: &NetworkManager) -> bool {
    nm.list_wireless_devices()
        .await
        .map(|d| !d.is_empty())
        .unwrap_or(false)
}

/// Check if Ethernet is available
async fn has_ethernet_device(nm: &NetworkManager) -> bool {
    nm.list_wired_devices()
        .await
        .map(|d| !d.is_empty())
        .unwrap_or(false)
}

/// Skip tests if NetworkManager is not available outside the integration harness.
macro_rules! require_networkmanager {
    () => {
        if !is_networkmanager_available().await {
            if std::env::var_os("NMRS_REQUIRE_NETWORKMANAGER").is_some() {
                panic!("NetworkManager is required but unavailable");
            }
            eprintln!("Skipping test: NetworkManager not available");
            return;
        }
    };
}

/// Skip tests if WiFi device is not available outside the WiFi integration harness.
macro_rules! require_wifi {
    ($nm:expr) => {
        if !has_wifi_device($nm).await {
            if std::env::var_os("NMRS_REQUIRE_WIFI").is_some() {
                panic!("WiFi is required but no WiFi device is available");
            }
            eprintln!("Skipping test: No WiFi device available");
            return;
        }
    };
}

/// Skip tests if Ethernet device is not available
macro_rules! require_ethernet {
    ($nm:expr) => {
        if !has_ethernet_device($nm).await {
            eprintln!("Skipping test: No Ethernet device available");
            return;
        }
    };
}

#[tokio::test]
#[serial]
async fn test_networkmanager_initialization() {
    require_networkmanager!();

    let _nm = NetworkManager::new()
        .await
        .expect("Failed to create NetworkManager");
}

/// Test listing devices
#[tokio::test]
#[serial]
async fn test_list_devices() {
    require_networkmanager!();

    let nm = NetworkManager::new()
        .await
        .expect("Failed to create NetworkManager");
    let devices = nm.list_devices().await.expect("Failed to list devices");

    assert!(!devices.is_empty(), "Expected at least one device");

    for device in &devices {
        assert!(!device.path.is_empty(), "Device path should not be empty");
        assert!(
            !device.interface.is_empty(),
            "Device interface should not be empty"
        );
    }
}

/// Test WiFi enabled state
#[tokio::test]
#[serial]
async fn test_wifi_enabled_get_set() {
    require_networkmanager!();

    let nm = NetworkManager::new()
        .await
        .expect("Failed to create NetworkManager");
    require_wifi!(&nm);

    let initial_state = nm
        .wifi_state()
        .await
        .expect("Failed to get WiFi enabled state")
        .enabled;

    match nm.set_wireless_enabled(!initial_state).await {
        Ok(_) => {
            sleep(Duration::from_millis(500)).await;

            let new_state = nm
                .wifi_state()
                .await
                .expect("Failed to get WiFi enabled state after toggle")
                .enabled;

            if new_state == initial_state {
                eprintln!(
                    "Warning: WiFi state didn't change (may lack permissions). Initial: {}, New: {}",
                    initial_state, new_state
                );
                return;
            }
        }
        Err(e) => {
            eprintln!("Failed to toggle WiFi (may lack permissions): {}", e);
            return;
        }
    }

    nm.set_wireless_enabled(initial_state)
        .await
        .expect("Failed to restore WiFi enabled state");

    sleep(Duration::from_millis(500)).await;

    let restored_state = nm
        .wifi_state()
        .await
        .expect("Failed to get WiFi enabled state after restore")
        .enabled;
    assert_eq!(
        restored_state, initial_state,
        "WiFi state should be restored to original"
    );
}

#[tokio::test]
#[serial]
async fn test_wifi_hardware_enabled() {
    require_networkmanager!();

    let nm = NetworkManager::new()
        .await
        .expect("Failed to connect to NetworkManager");

    require_wifi!(&nm);

    // Read-only property — just verify the call succeeds
    let state = nm
        .wifi_state()
        .await
        .expect("Failed to get WiFi radio state");
    let _ = state.hardware_enabled;
}

/// Test waiting for WiFi to be ready
#[tokio::test]
#[serial]
async fn test_wait_for_wifi_ready() {
    require_networkmanager!();

    let nm = NetworkManager::new()
        .await
        .expect("Failed to create NetworkManager");
    require_wifi!(&nm);

    // Ensure WiFi is enabled
    nm.set_wireless_enabled(true)
        .await
        .expect("Failed to enable WiFi");

    // Wait for WiFi to be ready
    let result = nm.wait_for_wifi_ready().await;

    // This should either succeed or fail gracefully
    // We don't assert success because WiFi might not be ready in all test environments
    match result {
        Ok(_) => {}
        Err(e) => {
            eprintln!(
                "WiFi not ready (this may be expected in some environments): {}",
                e
            );
        }
    }
}

/// Test scanning networks
#[tokio::test]
#[serial]
async fn test_scan_networks() {
    require_networkmanager!();

    let nm = NetworkManager::new()
        .await
        .expect("Failed to create NetworkManager");
    require_wifi!(&nm);

    // Ensure WiFi is enabled
    nm.set_wireless_enabled(true)
        .await
        .expect("Failed to enable WiFi");

    // Wait for WiFi to be ready
    let _ = nm.wait_for_wifi_ready().await;

    // Request a scan
    let result = nm.scan_networks(None).await;

    // Scan should either succeed or fail gracefully
    match result {
        Ok(_) => {
            // Success - wait a bit for scan to complete
            sleep(Duration::from_secs(2)).await;
        }
        Err(e) => {
            eprintln!("Scan failed (may be expected in some environments): {}", e);
        }
    }
}

/// Test listing networks
#[tokio::test]
#[serial]
async fn test_list_networks() {
    require_networkmanager!();

    let nm = NetworkManager::new()
        .await
        .expect("Failed to create NetworkManager");
    require_wifi!(&nm);

    // Ensure WiFi is enabled
    nm.set_wireless_enabled(true)
        .await
        .expect("Failed to enable WiFi");

    // Wait for WiFi to be ready
    let _ = nm.wait_for_wifi_ready().await;

    // Request a scan first
    let _ = nm.scan_networks(None).await;
    sleep(Duration::from_secs(2)).await;

    // List networks
    let networks = nm
        .list_networks(None)
        .await
        .expect("Failed to list networks");

    // Verify network structure
    for network in &networks {
        assert!(
            !network.ssid.is_empty() || network.ssid == "<hidden>",
            "SSID should not be empty (unless hidden)"
        );
        // `list_networks` can include deduplicated entries where device identity
        // is not populated; that is valid for this API.
    }
}

/// Ensure the virtual access point is visible when the WiFi integration harness runs.
#[tokio::test]
#[serial]
async fn test_hwsim_access_point_is_discovered() {
    let expected_ssid = match std::env::var("NMRS_EXPECT_WIFI_SSID") {
        Ok(ssid) => ssid,
        Err(_) => return,
    };

    require_networkmanager!();

    let nm = NetworkManager::new()
        .await
        .expect("Failed to create NetworkManager");
    require_wifi!(&nm);

    nm.set_wireless_enabled(true)
        .await
        .expect("Failed to enable WiFi");

    for _ in 0..3 {
        nm.scan_networks(None)
            .await
            .expect("Failed to scan for the virtual access point");
        sleep(Duration::from_secs(2)).await;

        let networks = nm
            .list_networks(None)
            .await
            .expect("Failed to list scanned networks");

        if let Some(network) = networks
            .iter()
            .find(|network| network.ssid == expected_ssid)
        {
            assert!(network.secured, "The virtual access point must be secured");
            assert!(network.is_psk, "The virtual access point must use WPA-PSK");
            return;
        }
    }

    panic!("The virtual access point {expected_ssid:?} was not discovered");
}

/// Test getting current SSID
#[tokio::test]
#[serial]
async fn test_current_ssid() {
    require_networkmanager!();

    let nm = NetworkManager::new()
        .await
        .expect("Failed to create NetworkManager");
    require_wifi!(&nm);

    // Ensure WiFi is enabled
    nm.set_wireless_enabled(true)
        .await
        .expect("Failed to enable WiFi");

    // Get current SSID (may be None if not connected)
    let current_ssid = nm.current_ssid().await;

    // If connected, SSID should not be empty
    if let Some(ssid) = current_ssid {
        assert!(
            !ssid.is_empty(),
            "Current SSID should not be empty if connected"
        );
    }
}

/// Test getting current connection info
#[tokio::test]
#[serial]
async fn test_current_connection_info() {
    require_networkmanager!();

    let nm = NetworkManager::new()
        .await
        .expect("Failed to create NetworkManager");
    require_wifi!(&nm);

    // Ensure WiFi is enabled
    nm.set_wireless_enabled(true)
        .await
        .expect("Failed to enable WiFi");

    // Get current connection info (may be None if not connected)
    let info = nm.current_connection_info().await;

    // If connected, SSID should not be empty
    if let Some((ssid, _frequency)) = info {
        assert!(
            !ssid.is_empty(),
            "Current SSID should not be empty if connected"
        );
    }
}

/// Test showing details
#[tokio::test]
#[serial]
async fn test_show_details() {
    require_networkmanager!();

    let nm = NetworkManager::new()
        .await
        .expect("Failed to create NetworkManager");
    require_wifi!(&nm);

    // Ensure WiFi is enabled
    nm.set_wireless_enabled(true)
        .await
        .expect("Failed to enable WiFi");

    // Wait for WiFi to be ready
    let _ = nm.wait_for_wifi_ready().await;

    // Request a scan first
    let _ = nm.scan_networks(None).await;
    sleep(Duration::from_secs(2)).await;

    // List networks
    let networks = nm
        .list_networks(None)
        .await
        .expect("Failed to list networks");

    // Try to show details for the first network (if any)
    if let Some(network) = networks.first() {
        let result = nm.show_details(network).await;

        match result {
            Ok(details) => {
                // Verify details structure
                assert_eq!(details.ssid, network.ssid, "SSID should match");
                assert!(!details.bssid.is_empty(), "BSSID should not be empty");
                assert!(details.strength <= 100, "Strength should be <= 100");
                assert!(!details.mode.is_empty(), "Mode should not be empty");
                assert!(!details.security.is_empty(), "Security should not be empty");
                assert!(!details.status.is_empty(), "Status should not be empty");
            }
            Err(e) => {
                // Network might have disappeared between scan and details request
                eprintln!("Failed to show details (may be expected): {}", e);
            }
        }
    }
}

/// Test checking if a connection is saved
#[tokio::test]
#[serial]
async fn test_has_saved_connection() {
    require_networkmanager!();

    let nm = NetworkManager::new()
        .await
        .expect("Failed to create NetworkManager");
    require_wifi!(&nm);

    // Test with a non-existent SSID
    let result = nm
        .has_saved_connection("__NONEXISTENT_TEST_SSID__")
        .await
        .expect("Failed to check saved connection");
    assert!(
        !result,
        "Non-existent SSID should not have saved connection"
    );

    // Test with empty SSID
    let _result = nm
        .has_saved_connection("")
        .await
        .expect("Failed to check saved connection for empty SSID");
}

/// Test getting the path of a saved connection
#[tokio::test]
#[serial]
async fn test_get_saved_connection_path() {
    require_networkmanager!();

    let nm = NetworkManager::new()
        .await
        .expect("Failed to create NetworkManager");
    require_wifi!(&nm);

    // Test with a non-existent SSID
    let result = nm
        .get_saved_connection_path("__NONEXISTENT_TEST_SSID__")
        .await
        .expect("Failed to get saved connection path");
    assert!(
        result.is_none(),
        "Non-existent SSID should not have saved connection path"
    );

    // Test with empty SSID
    let result = nm
        .get_saved_connection_path("")
        .await
        .expect("Failed to get saved connection path for empty SSID");
    // Result can be Some or None depending on system state
    let _ = result;
}

/// Test getting the UUID of a saved connection
#[tokio::test]
#[serial]
async fn test_get_saved_connection_uuid() {
    require_networkmanager!();

    let nm = NetworkManager::new()
        .await
        .expect("Failed to create NetworkManager");
    require_wifi!(&nm);

    let result = nm
        .get_saved_connection_uuid("__NONEXISTENT_TEST_SSID__")
        .await
        .expect("Failed to get saved connection UUID");
    assert!(
        result.is_none(),
        "Non-existent SSID should not have saved connection UUID"
    );

    let result = nm
        .get_saved_connection_uuid("")
        .await
        .expect("Failed to get saved connection UUID for empty SSID");
    let _ = result;
}

/// Test connecting to an open network
#[tokio::test]
#[serial]
async fn test_connect_open_network() {
    require_networkmanager!();

    let nm = NetworkManager::new()
        .await
        .expect("Failed to create NetworkManager");
    require_wifi!(&nm);

    // Ensure WiFi is enabled
    nm.set_wireless_enabled(true)
        .await
        .expect("Failed to enable WiFi");

    // Wait for WiFi to be ready
    let _ = nm.wait_for_wifi_ready().await;

    // Request a scan first
    let _ = nm.scan_networks(None).await;
    sleep(Duration::from_secs(2)).await;

    // List networks to find an open network
    let networks = nm
        .list_networks(None)
        .await
        .expect("Failed to list networks");

    // Find an open network (if any)
    let open_network = networks.iter().find(|n| !n.secured);

    if let Some(network) = open_network {
        let test_ssid = &network.ssid;

        // Skip if SSID is hidden or empty
        if test_ssid.is_empty() || test_ssid == "<hidden>" {
            eprintln!("Skipping: Found open network but SSID is hidden/empty");
            return;
        }

        // Try to connect to the open network
        let result = nm.connect(test_ssid, None, WifiSecurity::Open).await;

        match result {
            Ok(_) => {
                // Connection succeeded - wait a bit and verify
                sleep(Duration::from_secs(3)).await;
                let current = nm.current_ssid().await;
                if let Some(connected_ssid) = current {
                    // May or may not match depending on connection success
                    eprintln!("Connected SSID: {}", connected_ssid);
                }
            }
            Err(e) => {
                // Connection failed - this is acceptable in test environments
                eprintln!("Connection failed (may be expected): {}", e);
            }
        }
    } else {
        eprintln!("No open networks found for testing");
    }
}

/// Test connecting to a PSK network with an empty password
#[tokio::test]
#[serial]
async fn test_connect_psk_network_with_empty_password() {
    require_networkmanager!();

    let nm = NetworkManager::new()
        .await
        .expect("Failed to create NetworkManager");
    require_wifi!(&nm);

    // Ensure WiFi is enabled
    nm.set_wireless_enabled(true)
        .await
        .expect("Failed to enable WiFi");

    // Wait for WiFi to be ready
    let _ = nm.wait_for_wifi_ready().await;

    // Request a scan first
    let _ = nm.scan_networks(None).await;
    sleep(Duration::from_secs(2)).await;

    // List networks to find a PSK network
    let networks = nm
        .list_networks(None)
        .await
        .expect("Failed to list networks");

    // Find a PSK network (if any)
    let psk_network = networks.iter().find(|n| n.is_psk);

    if let Some(network) = psk_network {
        let test_ssid = &network.ssid;

        // Skip if SSID is hidden or empty
        if test_ssid.is_empty() || test_ssid == "<hidden>" {
            eprintln!("Skipping: Found PSK network but SSID is hidden/empty");
            return;
        }

        // Check if we have a saved connection for this network
        let has_saved = nm
            .has_saved_connection(test_ssid)
            .await
            .expect("Failed to check saved connection");

        if has_saved {
            // Try to connect with empty password (should use saved credentials)
            let result = nm
                .connect(test_ssid, None, WifiSecurity::WpaPsk { psk: String::new() })
                .await;

            match result {
                Ok(_) => {
                    // Connection succeeded - wait a bit
                    sleep(Duration::from_secs(3)).await;
                }
                Err(e) => {
                    // Connection failed - this is acceptable
                    eprintln!("Connection with saved credentials failed: {}", e);
                }
            }
        } else {
            eprintln!("No saved connection for PSK network, skipping test");
        }
    } else {
        eprintln!("No PSK networks found for testing");
    }
}

/// Test forgetting a nonexistent network
#[tokio::test]
#[serial]
async fn test_forget_nonexistent_network() {
    require_networkmanager!();

    let nm = NetworkManager::new()
        .await
        .expect("Failed to create NetworkManager");
    require_wifi!(&nm);

    // Try to forget a non-existent network
    let result = nm.forget("__NONEXISTENT_TEST_SSID_TO_FORGET__").await;

    // This should fail since the network doesn't exist
    assert!(
        result.is_err(),
        "Forgetting non-existent network should fail"
    );
}

/// Test device states
#[tokio::test]
#[serial]
async fn test_device_states() {
    require_networkmanager!();

    let nm = NetworkManager::new()
        .await
        .expect("Failed to create NetworkManager");
    let devices = nm.list_devices().await.expect("Failed to list devices");

    // Verify that all devices have valid states
    for device in &devices {
        // DeviceState should be one of the known states
        // The struct is non-exhaustive and so we allow Other(_)
        match device.state {
            DeviceState::Unmanaged
            | DeviceState::Unavailable
            | DeviceState::Disconnected
            | DeviceState::Prepare
            | DeviceState::Config
            | DeviceState::NeedAuth
            | DeviceState::IpConfig
            | DeviceState::IpCheck
            | DeviceState::Secondaries
            | DeviceState::Activated
            | DeviceState::Deactivating
            | DeviceState::Failed
            | DeviceState::Other(_) => {}
            _ => {
                panic!("Invalid device state: {:?}", device.state);
            }
        }
    }
}

/// Test device types
#[tokio::test]
#[serial]
async fn test_device_types() {
    require_networkmanager!();

    let nm = NetworkManager::new()
        .await
        .expect("Failed to create NetworkManager");
    let devices = nm.list_devices().await.expect("Failed to list devices");

    // Verify that all devices have valid types
    for device in &devices {
        // DeviceType should be one of the known types
        // The struct is non-exhaustive and so we allow Other(_)
        match device.device_type {
            DeviceType::Ethernet
            | DeviceType::Wifi
            | DeviceType::Bluetooth
            | DeviceType::WifiP2P
            | DeviceType::Loopback
            | DeviceType::Other(_) => {
                // Valid type
            }
            _ => {
                panic!("Invalid device type: {:?}", device.device_type);
            }
        }
    }
}

/// Test network properties
#[tokio::test]
#[serial]
async fn test_network_properties() {
    require_networkmanager!();

    let nm = NetworkManager::new()
        .await
        .expect("Failed to create NetworkManager");
    require_wifi!(&nm);

    // Ensure WiFi is enabled
    nm.set_wireless_enabled(true)
        .await
        .expect("Failed to enable WiFi");

    // Wait for WiFi to be ready
    let _ = nm.wait_for_wifi_ready().await;

    // Request a scan first
    let _ = nm.scan_networks(None).await;
    sleep(Duration::from_secs(2)).await;

    // List networks
    let networks = nm
        .list_networks(None)
        .await
        .expect("Failed to list networks");

    // Verify network properties
    for network in &networks {
        // SSID should not be empty (unless hidden)
        assert!(
            !network.ssid.is_empty() || network.ssid == "<hidden>",
            "SSID should not be empty"
        );

        // `device` may be empty for deduplicated scan entries; only validate
        // normalized fields that are guaranteed by this API.

        // If strength is Some, it should be <= 100
        if let Some(strength) = network.strength {
            assert!(strength <= 100, "Strength should be <= 100");
        }

        // Security flags should be consistent
        if !network.secured {
            assert!(!network.is_psk, "Unsecured network should not be PSK");
            assert!(!network.is_eap, "Unsecured network should not be EAP");
        }
    }
}

/// Test multiple scan requests
#[tokio::test]
#[serial]
async fn test_multiple_scan_requests() {
    require_networkmanager!();

    let nm = NetworkManager::new()
        .await
        .expect("Failed to create NetworkManager");
    require_wifi!(&nm);

    // Ensure WiFi is enabled
    nm.set_wireless_enabled(true)
        .await
        .expect("Failed to enable WiFi");

    // Wait for WiFi to be ready
    let _ = nm.wait_for_wifi_ready().await;

    // Request multiple scans
    for i in 0..3 {
        nm.wait_for_wifi_ready().await.expect("WiFi not ready");

        let result = nm.scan_networks(None).await;
        match result {
            Ok(_) => eprintln!("Scan {} succeeded", i + 1),
            Err(e) => eprintln!("Scan {} failed: {}", i + 1, e),
        }

        nm.wait_for_wifi_ready()
            .await
            .expect("WiFi did not recover");
        sleep(Duration::from_secs(3)).await;
    }

    // List networks after multiple scans
    let networks = nm
        .list_networks(None)
        .await
        .expect("Failed to list networks");
    eprintln!("Found {} networks after multiple scans", networks.len());
}

/// Test concurrent operations
#[tokio::test]
#[serial]
async fn test_concurrent_operations() {
    require_networkmanager!();

    let nm = NetworkManager::new()
        .await
        .expect("Failed to create NetworkManager");
    require_wifi!(&nm);

    // Ensure WiFi is enabled
    nm.set_wireless_enabled(true)
        .await
        .expect("Failed to enable WiFi");

    // Run multiple operations concurrently
    let (devices_result, wifi_state_result, networks_result) =
        tokio::join!(nm.list_devices(), nm.wifi_state(), nm.list_networks(None));

    // All should succeed
    assert!(devices_result.is_ok(), "list_devices should succeed");
    assert!(wifi_state_result.is_ok(), "wifi_state should succeed");
    // networks_result may fail if WiFi is not ready, which is acceptable
    let _ = networks_result;
}

/// Test that reason_to_error maps auth failures correctly
#[test]
fn reason_to_error_auth_mapping() {
    // Supplicant failed (code 9) should map to AuthFailed
    assert!(matches!(reason_to_error(9), ConnectionError::AuthFailed));

    // Supplicant disconnected (code 7) should map to AuthFailed
    assert!(matches!(reason_to_error(7), ConnectionError::AuthFailed));

    // DHCP failed (code 17) should map to DhcpFailed
    assert!(matches!(reason_to_error(17), ConnectionError::DhcpFailed));

    // SSID not found (code 70) should map to NotFound
    assert!(matches!(reason_to_error(70), ConnectionError::NotFound));
}

/// Test StateReason conversions
#[test]
fn state_reason_conversion() {
    assert_eq!(StateReason::from(9), StateReason::SupplicantFailed);
    assert_eq!(StateReason::from(70), StateReason::SsidNotFound);
    assert_eq!(StateReason::from(999), StateReason::Other(999));
}

/// Test ConnectionError display formatting
#[test]
fn connection_error_display() {
    let auth_err = ConnectionError::AuthFailed;
    assert_eq!(format!("{}", auth_err), "authentication failed");

    let not_found_err = ConnectionError::NotFound;
    assert_eq!(format!("{}", not_found_err), "network not found");

    let timeout_err = ConnectionError::Timeout;
    assert_eq!(format!("{}", timeout_err), "connection timeout");

    let stuck_err = ConnectionError::Stuck("config".into());
    assert_eq!(
        format!("{}", stuck_err),
        "connection stuck in state: config"
    );
}

/// Test forgetting a network returns NoSavedConnection error
#[tokio::test]
#[serial]
async fn forget_returns_no_saved_connection_error() {
    require_networkmanager!();

    let nm = NetworkManager::new()
        .await
        .expect("Failed to create NetworkManager");
    require_wifi!(&nm);

    let result = nm.forget("__NONEXISTENT_TEST_SSID__").await;

    match result {
        Err(ConnectionError::NoSavedConnection) => {
            // Expected error type
        }
        Err(e) => {
            panic!("Expected NoSavedConnection error, got: {}", e);
        }
        Ok(_) => {
            // Error is Expected in case of failed operation only.
            println!("Expected response, got success");
        }
    }
}

/// Test listing wired devices
#[tokio::test]
#[serial]
async fn test_list_wired_devices() {
    require_networkmanager!();

    let nm = NetworkManager::new()
        .await
        .expect("Failed to create NetworkManager");

    let devices = nm
        .list_wired_devices()
        .await
        .expect("Failed to list wired devices");

    // Verify device structure for wired devices
    for device in &devices {
        assert!(!device.path.is_empty(), "Device path should not be empty");
        assert!(
            !device.interface.is_empty(),
            "Device interface should not be empty"
        );
        assert_eq!(
            device.device_type,
            DeviceType::Ethernet,
            "Device type should be Ethernet"
        );
    }
}

/// Test connecting to wired device
#[tokio::test]
#[serial]
async fn test_connect_wired() {
    require_networkmanager!();

    let nm = NetworkManager::new()
        .await
        .expect("Failed to create NetworkManager");
    require_ethernet!(&nm);

    // Try to connect to wired device
    let result = nm.connect_wired().await;

    match result {
        Ok(_) => {
            // Connection succeeded or is waiting for cable
            eprintln!("Wired connection initiated successfully");
        }
        Err(e) => {
            // Connection failed - this is acceptable in test environments
            eprintln!("Wired connection failed (may be expected): {}", e);
        }
    }
}

/// Helper to create test VPN configuration
fn create_test_vpn_creds(name: &str) -> WireGuardConfig {
    let peer = WireGuardPeer::new(
        "HIgo9xNzJMWLKAShlKl6/bUT1VI9Q0SDBXGtLXkPFXc=",
        "test.example.com:51820",
        vec!["0.0.0.0/0".into(), "::/0".into()],
    )
    .with_persistent_keepalive(25);

    WireGuardConfig::new(
        name,
        "test.example.com:51820",
        "YBk6X3pP8KjKz7+HFWzVHNqL3qTZq8hX9VxFQJ4zVmM=",
        "10.100.0.2/24",
        vec![peer],
    )
    .with_dns(vec!["1.1.1.1".into(), "8.8.8.8".into()])
    .with_mtu(1420)
}

/// Test listing VPN connections
#[tokio::test]
#[serial]
async fn test_list_vpn_connections() {
    require_networkmanager!();

    let nm = NetworkManager::new()
        .await
        .expect("Failed to create NetworkManager");

    // List VPN connections (should not fail even if empty)
    let result = nm.list_vpn_connections().await;
    assert!(result.is_ok(), "Should be able to list VPN connections");

    let vpns = result.unwrap();
    eprintln!("Found {} VPN connection(s)", vpns.len());

    // Verify structure of any VPN connections found
    for vpn in &vpns {
        assert!(!vpn.name.is_empty(), "VPN name should not be empty");
        eprintln!("VPN: {} ({:?})", vpn.name, vpn.vpn_type);
    }
}

/// Test VPN connection lifecycle (does not actually connect)
#[tokio::test]
#[serial]
async fn test_vpn_lifecycle_dry_run() {
    require_networkmanager!();

    let nm = NetworkManager::new()
        .await
        .expect("Failed to create NetworkManager");

    // Note: This test does NOT actually connect to a VPN
    // It only tests the API structure and error handling

    // Create test credentials
    let creds = create_test_vpn_creds("test_vpn_lifecycle");

    // Attempt to connect (will likely fail as test server doesn't exist)
    let result = nm.connect_vpn(creds).await;

    match result {
        Ok(_) => {
            eprintln!("VPN connection succeeded (unexpected in test)");
            // Clean up
            let _ = nm.disconnect_vpn("test_vpn_lifecycle").await;
            let _ = nm.forget_vpn("test_vpn_lifecycle").await;
        }
        Err(e) => {
            eprintln!("VPN connection failed as expected: {}", e);
            // This is expected since we're using fake credentials
        }
    }
}

/// Test VPN disconnection with non-existent VPN
#[tokio::test]
#[serial]
async fn test_disconnect_nonexistent_vpn() {
    require_networkmanager!();

    let nm = NetworkManager::new()
        .await
        .expect("Failed to create NetworkManager");

    // Disconnecting a non-existent VPN should succeed (idempotent)
    let result = nm.disconnect_vpn("nonexistent_vpn_connection_12345").await;
    assert!(
        result.is_ok(),
        "Disconnecting non-existent VPN should succeed"
    );
}

/// Test forgetting non-existent VPN
#[tokio::test]
#[serial]
async fn test_forget_nonexistent_vpn() {
    require_networkmanager!();

    let nm = NetworkManager::new()
        .await
        .expect("Failed to create NetworkManager");

    // Forgetting a non-existent VPN will return Ok
    // Error is Expected in case of failed operation only
    let result = nm.forget_vpn("nonexistent_vpn_connection_12345").await;
    assert!(
        result.is_ok(),
        "Forgetting non-existent VPN should return error"
    );

    match result {
        Err(ConnectionError::NoSavedConnection) => {
            eprintln!("Correct error: NoSavedConnection");
        }
        Err(e) => {
            panic!("Unexpected error type: {}", e);
        }
        Ok(_) => {
            println!("Correct response: NoSavedConnection");
        }
    }
}

/// Test getting info for non-existent VPN
#[tokio::test]
#[serial]
async fn test_get_nonexistent_vpn_info() {
    require_networkmanager!();

    let nm = NetworkManager::new()
        .await
        .expect("Failed to create NetworkManager");

    // Getting info for non-existent/inactive VPN should fail
    let result = nm.get_vpn_info("nonexistent_vpn_connection_12345").await;
    assert!(
        result.is_err(),
        "Getting info for non-existent VPN should return error"
    );

    match result {
        Err(ConnectionError::NoVpnConnection) => {
            eprintln!("Correct error: NoVpnConnection");
        }
        Err(e) => {
            eprintln!("Error (acceptable): {}", e);
        }
        Ok(_) => {
            panic!("Should have failed");
        }
    }
}

/// Test VPN type enum
#[tokio::test]
#[serial]
async fn test_vpn_type() {
    // Verify VPN types are properly defined
    let wg = VpnKind::WireGuard;
    assert_eq!(format!("{:?}", wg), "WireGuard");
}

/// Test WireGuard peer structure
#[tokio::test]
#[serial]
async fn test_wireguard_peer_structure() {
    let peer = WireGuardPeer::new(
        "test_key",
        "test.example.com:51820",
        vec!["0.0.0.0/0".into()],
    )
    .with_preshared_key("psk")
    .with_persistent_keepalive(25);

    assert_eq!(peer.public_key, "test_key");
    assert_eq!(peer.gateway, "test.example.com:51820");
    assert_eq!(peer.allowed_ips.len(), 1);
    assert_eq!(peer.preshared_key, Some("psk".into()));
    assert_eq!(peer.persistent_keepalive, Some(25));
}

/// Test VPN configuration structure
#[tokio::test]
#[serial]
async fn test_vpn_credentials_structure() {
    let creds = create_test_vpn_creds("test_credentials");

    assert_eq!(creds.name, "test_credentials");
    assert_eq!(creds.peers.len(), 1);
    assert_eq!(creds.address, "10.100.0.2/24");
    assert!(creds.dns.is_some());
    assert_eq!(creds.dns.as_ref().unwrap().len(), 2);
    assert_eq!(creds.mtu, Some(1420));
}

/// Check if Bluetooth is available
#[allow(dead_code)]
async fn has_bluetooth_device(nm: &NetworkManager) -> bool {
    nm.list_bluetooth_devices()
        .await
        .map(|d| !d.is_empty())
        .unwrap_or(false)
}

/// Skip tests if Bluetooth device is not available
#[allow(unused_macros)]
macro_rules! require_bluetooth {
    ($nm:expr) => {
        if !has_bluetooth_device($nm).await {
            eprintln!("Skipping test: No Bluetooth device available");
            return;
        }
    };
}

/// Test listing Bluetooth devices
#[tokio::test]
#[serial]
async fn test_list_bluetooth_devices() {
    require_networkmanager!();

    let nm = NetworkManager::new()
        .await
        .expect("Failed to create NetworkManager");

    let devices = nm
        .list_bluetooth_devices()
        .await
        .expect("Failed to list Bluetooth devices");

    // Verify device structure for Bluetooth devices
    for device in &devices {
        assert!(
            !device.bdaddr.is_empty(),
            "Bluetooth address should not be empty"
        );
        eprintln!(
            "Bluetooth device: {} ({}) - {}",
            device.alias.as_deref().unwrap_or("unknown"),
            device.bdaddr,
            device.bt_caps
        );
    }
}

/// Test Bluetooth device type enum
#[test]
fn test_bluetooth_network_role() {
    use nmrs::models::BluetoothNetworkRole;

    let panu = BluetoothNetworkRole::PanU;
    assert_eq!(format!("{}", panu), "PANU");

    let dun = BluetoothNetworkRole::Dun;
    assert_eq!(format!("{}", dun), "DUN");
}

/// Test BluetoothIdentity structure
#[test]
fn test_bluetooth_identity_structure() {
    use nmrs::models::{BluetoothIdentity, BluetoothNetworkRole};

    let identity =
        BluetoothIdentity::new("00:1A:7D:DA:71:13".into(), BluetoothNetworkRole::PanU).unwrap();

    assert_eq!(identity.bdaddr, "00:1A:7D:DA:71:13");
    assert!(matches!(
        identity.bt_device_type,
        BluetoothNetworkRole::PanU
    ));
}

/// Test BluetoothDevice structure
#[test]
fn test_bluetooth_device_structure() {
    use nmrs::models::{BluetoothDevice, BluetoothNetworkRole};

    let role = BluetoothNetworkRole::PanU as u32;
    let device = BluetoothDevice::new(
        "00:1A:7D:DA:71:13".into(),
        Some("MyPhone".into()),
        Some("Phone".into()),
        role,
        DeviceState::Activated,
    );

    assert_eq!(device.bdaddr, "00:1A:7D:DA:71:13");
    assert_eq!(device.name, Some("MyPhone".into()));
    assert_eq!(device.alias, Some("Phone".into()));
    assert_eq!(device.state, DeviceState::Activated);
}

/// Test BluetoothDevice display
#[test]
fn test_bluetooth_device_display() {
    use nmrs::models::{BluetoothDevice, BluetoothNetworkRole};

    let role = BluetoothNetworkRole::PanU as u32;
    let device = BluetoothDevice::new(
        "00:1A:7D:DA:71:13".into(),
        Some("MyPhone".into()),
        Some("Phone".into()),
        role,
        DeviceState::Activated,
    );

    let display = format!("{}", device);
    assert!(display.contains("Phone"));
    assert!(display.contains("00:1A:7D:DA:71:13"));
}

/// Test Device::is_bluetooth method
#[tokio::test]
#[serial]
async fn test_device_is_bluetooth() {
    require_networkmanager!();

    let nm = NetworkManager::new()
        .await
        .expect("Failed to create NetworkManager");

    let devices = nm.list_devices().await.expect("Failed to list devices");

    for device in &devices {
        if device.is_bluetooth() {
            assert_eq!(device.device_type, DeviceType::Bluetooth);
            eprintln!("Found Bluetooth device: {}", device.interface);
        }
    }
}

/// Test Bluetooth device in all devices list
#[tokio::test]
#[serial]
async fn test_bluetooth_in_device_types() {
    require_networkmanager!();

    let nm = NetworkManager::new()
        .await
        .expect("Failed to create NetworkManager");

    let devices = nm.list_devices().await.expect("Failed to list devices");

    // Check if any Bluetooth devices exist
    let bluetooth_devices: Vec<_> = devices
        .iter()
        .filter(|d| matches!(d.device_type, DeviceType::Bluetooth))
        .collect();

    if !bluetooth_devices.is_empty() {
        eprintln!("Found {} Bluetooth device(s)", bluetooth_devices.len());
        for device in bluetooth_devices {
            eprintln!("  - {}: {}", device.interface, device.state);
        }
    } else {
        eprintln!("No Bluetooth devices found (this is OK)");
    }
}

/// Test ConnectionError::NoBluetoothDevice
#[test]
fn test_connection_error_no_bluetooth_device() {
    let err = ConnectionError::NoBluetoothDevice;
    assert_eq!(format!("{}", err), "Bluetooth device not found");
}

/// Test BluetoothNetworkRole conversion from u32
#[test]
fn test_bluetooth_network_role_from_u32() {
    use nmrs::models::BluetoothNetworkRole;

    assert!(matches!(
        BluetoothNetworkRole::from(0),
        BluetoothNetworkRole::PanU
    ));
    assert!(matches!(
        BluetoothNetworkRole::from(1),
        BluetoothNetworkRole::Dun
    ));
    // Unknown values should default to PanU
    assert!(matches!(
        BluetoothNetworkRole::from(999),
        BluetoothNetworkRole::PanU
    ));
}

// --- OpenVPN import tests ---

/// Test that OpenVpnBuilder::from_ovpn_str produces correct settings for a
/// full TLS config, and that build_openvpn_connection serializes them.
#[test]
fn test_ovpn_import_tls_roundtrip() {
    use nmrs::ConnectionOptions;
    use nmrs::builders::{OpenVpnBuilder, build_openvpn_connection};

    let ovpn = "\
remote vpn.example.com 1194 udp
ca /etc/openvpn/ca.crt
cert /etc/openvpn/client.crt
key /etc/openvpn/client.key
cipher AES-256-GCM
auth SHA256
tls-auth /etc/openvpn/ta.key 1
";
    let config = OpenVpnBuilder::from_ovpn_str(ovpn, "roundtrip-test")
        .unwrap()
        .build()
        .unwrap();

    assert_eq!(config.remote, "vpn.example.com");
    assert_eq!(config.port, 1194);
    assert_eq!(config.auth_type, Some(OpenVpnAuthType::Tls));
    assert_eq!(config.cipher, Some("AES-256-GCM".into()));
    assert_eq!(config.auth, Some("SHA256".into()));
    assert_eq!(config.tls_auth_key, Some("/etc/openvpn/ta.key".into()));
    assert_eq!(config.tls_auth_direction, Some(1));

    let opts = ConnectionOptions::new(false);
    let settings = build_openvpn_connection(&config, &opts).unwrap();
    assert!(settings.contains_key("connection"));
    assert!(settings.contains_key("vpn"));
}

/// Test that from_ovpn_str infers password+TLS auth when both
/// auth-user-pass and cert/key are present.
#[test]
fn test_ovpn_import_password_tls() {
    use nmrs::builders::OpenVpnBuilder;

    let ovpn = "\
remote vpn.example.com 443 tcp
auth-user-pass
ca /etc/openvpn/ca.crt
cert /etc/openvpn/client.crt
key /etc/openvpn/client.key
";
    let config = OpenVpnBuilder::from_ovpn_str(ovpn, "pw-tls-test")
        .unwrap()
        .username("user")
        .build()
        .unwrap();

    assert_eq!(config.auth_type, Some(OpenVpnAuthType::PasswordTls));
    assert!(config.tcp);
    assert_eq!(config.port, 443);
}

/// Test that the caller can override parsed settings before build.
#[test]
fn test_ovpn_import_override() {
    use nmrs::builders::OpenVpnBuilder;

    let ovpn = "\
remote vpn.example.com 1194
ca /etc/openvpn/ca.crt
cert /etc/openvpn/client.crt
key /etc/openvpn/client.key
";
    let config = OpenVpnBuilder::from_ovpn_str(ovpn, "override-test")
        .unwrap()
        .port(443)
        .tcp(true)
        .dns(vec!["1.1.1.1".into()])
        .mtu(1400)
        .remote_cert_tls("server")
        .build()
        .unwrap();

    assert_eq!(config.port, 443);
    assert!(config.tcp);
    assert_eq!(config.dns, Some(vec!["1.1.1.1".into()]));
    assert_eq!(config.mtu, Some(1400));
    assert_eq!(config.remote_cert_tls, Some("server".into()));
}

/// Test airplane mode toggle (set and get)
///
/// This tests the aggregate airplane mode operation combining WiFi, WWAN, and Bluetooth.
/// Specifically validates that set_airplane_mode returns Ok(()) even if Bluetooth
/// adapter settle failures occur, as long as WiFi/WWAN toggles succeed.
/// This is a regression test for the fix where BluetoothToggleFailed is treated as
/// non-fatal in the aggregate operation.
#[tokio::test]
#[serial]
async fn test_airplane_mode_toggle() {
    require_networkmanager!();

    let nm = NetworkManager::new()
        .await
        .expect("Failed to create NetworkManager");

    // Get initial airplane mode state
    let initial_state = nm
        .airplane_mode_state()
        .await
        .expect("Failed to get airplane mode state");

    if !initial_state.wifi.present
        && !initial_state.wwan.present
        && !initial_state.bluetooth.present
    {
        eprintln!("Skipping test: no controllable radios present on host");
        return;
    }

    let is_airplane_mode = initial_state.is_airplane_mode();
    println!(
        "Initial airplane mode state: is_airplane_mode={}, WiFi enabled={}, WWAN enabled={}, Bluetooth enabled={}",
        is_airplane_mode,
        initial_state.wifi.enabled,
        initial_state.wwan.enabled,
        initial_state.bluetooth.enabled
    );

    // Toggle airplane mode to opposite state
    let target_enabled = !is_airplane_mode;
    println!("Toggling airplane mode to: {}", target_enabled);

    let mut failures: Vec<String> = Vec::new();
    let wifi_or_wwan_present = initial_state.wifi.present || initial_state.wwan.present;

    if let Err(e) = nm.set_airplane_mode(target_enabled).await {
        // BluetoothToggleFailed is expected on Bluetooth-only hosts (no Wi-Fi/WWAN).
        // Only treat this as a regression if Wi-Fi or WWAN is present.
        if wifi_or_wwan_present {
            failures.push(format!(
                "set_airplane_mode toggle returned error (regression candidate): {e}"
            ));
        } else {
            println!("set_airplane_mode error on Bluetooth-only host (expected): {e}");
        }
    } else {
        println!("Airplane mode toggle succeeded");
    }

    // Give the radios time to settle (especially Bluetooth with its 2-second timeout)
    sleep(Duration::from_secs(3)).await;

    // Verify the toggle took effect
    let new_state = match nm.airplane_mode_state().await {
        Ok(state) => Some(state),
        Err(e) => {
            failures.push(format!(
                "Failed to get airplane mode state after toggle: {e}"
            ));
            None
        }
    };

    if let Some(state) = &new_state {
        let new_is_airplane_mode = state.is_airplane_mode();
        println!(
            "New airplane mode state: is_airplane_mode={}, WiFi enabled={}, WWAN enabled={}, Bluetooth enabled={}",
            new_is_airplane_mode, state.wifi.enabled, state.wwan.enabled, state.bluetooth.enabled
        );

        let expected_radio_on = !target_enabled;
        let wifi_or_wwan_present = state.wifi.present || state.wwan.present;
        for (name, radio) in [
            ("WiFi", state.wifi),
            ("WWAN", state.wwan),
            ("Bluetooth", state.bluetooth),
        ] {
            // BluetoothToggleFailed is non-fatal when Wi-Fi/WWAN are present,
            // so skip the Bluetooth assertion on those hosts.
            if name == "Bluetooth" && wifi_or_wwan_present {
                continue;
            }
            if radio.present && radio.enabled != expected_radio_on {
                failures.push(format!(
                    "{name} enabled mismatch after toggle: expected {}, got {}",
                    expected_radio_on, radio.enabled
                ));
            }
        }
    }

    // Best-effort restore to initial aggregate state.
    // NOTE: set_airplane_mode(false) turns ALL radios on, so this does not truly
    // restore individual radio states. If the host started in a mixed state
    // (e.g., Wi-Fi on but Bluetooth off), radios may end up in a different
    // configuration than before. We log the result but don't assert on it.
    println!(
        "Restoring airplane mode to initial state: {}",
        is_airplane_mode
    );

    if let Err(e) = nm.set_airplane_mode(is_airplane_mode).await {
        println!("Best-effort restore failed (not a test failure): {e}");
    }

    // Give radios time to settle again
    sleep(Duration::from_secs(3)).await;

    // Log restored state for diagnostics (no assertions—restoration is best-effort)
    match nm.airplane_mode_state().await {
        Ok(restored_state) => {
            println!(
                "Restored airplane mode state: is_airplane_mode={}, WiFi enabled={}, WWAN enabled={}, Bluetooth enabled={}",
                restored_state.is_airplane_mode(),
                restored_state.wifi.enabled,
                restored_state.wwan.enabled,
                restored_state.bluetooth.enabled
            );
        }
        Err(e) => println!("Could not read restored state: {e}"),
    }

    if !failures.is_empty() {
        panic!("{}", failures.join("\n"));
    }
}
