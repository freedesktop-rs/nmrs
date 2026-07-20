#![allow(deprecated)]

use std::time::Duration;
use uuid::Uuid;

use super::bluetooth::*;
use super::config::*;
use super::connection_state::*;
use super::device::*;
use super::error::*;
use super::state_reason::*;
use super::vpn::*;
use super::wifi::*;
use super::wireguard::*;
use crate::api::models::DeviceType;

#[test]
fn device_type_code_round_trips_all_variants() {
    let cases = [
        (1, DeviceType::Ethernet),
        (2, DeviceType::Wifi),
        (5, DeviceType::Bluetooth),
        (11, DeviceType::Vlan),
        (30, DeviceType::WifiP2P),
        (32, DeviceType::Loopback),
        (999, DeviceType::Other(999)),
        (0, DeviceType::Other(0)),
    ];

    for (code, expected) in cases {
        let actual = DeviceType::from(code);
        assert_eq!(actual, expected);
        assert_eq!(actual.to_code(), code);
    }
}

#[test]
fn device_type_from_u32_registry_types() {
    // VLAN is now a first-class variant
    assert_eq!(DeviceType::from(11), DeviceType::Vlan);
    // These still fall through to Other since they're only in the registry
    assert_eq!(DeviceType::from(12), DeviceType::Other(12));
    assert_eq!(DeviceType::from(13), DeviceType::Other(13));
    assert_eq!(DeviceType::from(16), DeviceType::Other(16));
    assert_eq!(DeviceType::from(20), DeviceType::Other(20));
    assert_eq!(DeviceType::from(29), DeviceType::Other(29));
}

#[test]
fn device_type_display() {
    assert_eq!(format!("{}", DeviceType::Ethernet), "Ethernet");
    assert_eq!(format!("{}", DeviceType::Wifi), "Wi-Fi");
    assert_eq!(format!("{}", DeviceType::WifiP2P), "Wi-Fi P2P");
    assert_eq!(format!("{}", DeviceType::Loopback), "Loopback");
    assert_eq!(format!("{}", DeviceType::Bluetooth), "Bluetooth");
    assert_eq!(format!("{}", DeviceType::Vlan), "VLAN");
    assert_eq!(format!("{}", DeviceType::Other(42)), "Other(42)");
}

#[test]
fn device_type_display_registry() {
    assert_eq!(format!("{}", DeviceType::Other(13)), "Bridge");
    assert_eq!(format!("{}", DeviceType::Other(12)), "Bond");
    assert_eq!(format!("{}", DeviceType::Other(11)), "VLAN");
    assert_eq!(format!("{}", DeviceType::Other(16)), "TUN");
    assert_eq!(format!("{}", DeviceType::Other(20)), "Veth");
    assert_eq!(format!("{}", DeviceType::Other(29)), "WireGuard");
}

#[test]
fn device_type_supports_scanning() {
    assert!(DeviceType::Wifi.supports_scanning());
    assert!(DeviceType::WifiP2P.supports_scanning());
    assert!(!DeviceType::Ethernet.supports_scanning());
    assert!(!DeviceType::Loopback.supports_scanning());
}

#[test]
fn device_type_supports_scanning_registry() {
    assert!(DeviceType::Other(30).supports_scanning());
    assert!(!DeviceType::Other(13).supports_scanning());
    assert!(!DeviceType::Other(29).supports_scanning());
}

#[test]
fn device_type_requires_specific_object() {
    assert!(DeviceType::Wifi.requires_specific_object());
    assert!(DeviceType::WifiP2P.requires_specific_object());
    assert!(!DeviceType::Ethernet.requires_specific_object());
    assert!(!DeviceType::Loopback.requires_specific_object());
}

#[test]
fn device_type_requires_specific_object_registry() {
    assert!(DeviceType::Other(2).requires_specific_object());
    assert!(!DeviceType::Other(1).requires_specific_object());
    assert!(!DeviceType::Other(29).requires_specific_object());
}

#[test]
fn device_type_has_global_enabled_state() {
    assert!(DeviceType::Wifi.has_global_enabled_state());
    assert!(!DeviceType::Ethernet.has_global_enabled_state());
    assert!(!DeviceType::WifiP2P.has_global_enabled_state());
}

#[test]
fn device_type_has_global_enabled_state_registry() {
    assert!(DeviceType::Other(2).has_global_enabled_state());
    assert!(!DeviceType::Other(1).has_global_enabled_state());
}

#[test]
fn device_type_connection_type_str() {
    assert_eq!(DeviceType::Ethernet.connection_type_str(), "802-3-ethernet");
    assert_eq!(DeviceType::Wifi.connection_type_str(), "802-11-wireless");
    assert_eq!(DeviceType::WifiP2P.connection_type_str(), "wifi-p2p");
    assert_eq!(DeviceType::Loopback.connection_type_str(), "loopback");
    assert_eq!(DeviceType::Bluetooth.connection_type_str(), "bluetooth");
    assert_eq!(DeviceType::Vlan.connection_type_str(), "vlan");
}

#[test]
fn device_type_connection_type_str_registry() {
    assert_eq!(DeviceType::Other(13).connection_type_str(), "bridge");
    assert_eq!(DeviceType::Other(12).connection_type_str(), "bond");
    assert_eq!(DeviceType::Other(11).connection_type_str(), "vlan");
    assert_eq!(
        DeviceType::Other(20).connection_type_str(),
        "802-3-ethernet"
    );
    assert_eq!(DeviceType::Other(29).connection_type_str(), "wireguard");
}

#[test]
fn device_type_to_code_registry() {
    assert_eq!(DeviceType::Other(11).to_code(), 11);
    assert_eq!(DeviceType::Other(12).to_code(), 12);
    assert_eq!(DeviceType::Other(13).to_code(), 13);
    assert_eq!(DeviceType::Other(16).to_code(), 16);
    assert_eq!(DeviceType::Other(20).to_code(), 20);
    assert_eq!(DeviceType::Other(29).to_code(), 29);
}

#[test]
fn device_state_from_u32_all_variants() {
    assert_eq!(DeviceState::from(10), DeviceState::Unmanaged);
    assert_eq!(DeviceState::from(20), DeviceState::Unavailable);
    assert_eq!(DeviceState::from(30), DeviceState::Disconnected);
    assert_eq!(DeviceState::from(40), DeviceState::Prepare);
    assert_eq!(DeviceState::from(50), DeviceState::Config);
    assert_eq!(DeviceState::from(100), DeviceState::Activated);
    assert_eq!(DeviceState::from(110), DeviceState::Deactivating);
    assert_eq!(DeviceState::from(120), DeviceState::Failed);
    assert_eq!(DeviceState::from(7), DeviceState::Other(7));
    assert_eq!(DeviceState::from(0), DeviceState::Other(0));
}

#[test]
fn device_state_display() {
    assert_eq!(format!("{}", DeviceState::Unmanaged), "Unmanaged");
    assert_eq!(format!("{}", DeviceState::Unavailable), "Unavailable");
    assert_eq!(format!("{}", DeviceState::Disconnected), "Disconnected");
    assert_eq!(format!("{}", DeviceState::Prepare), "Preparing");
    assert_eq!(format!("{}", DeviceState::Config), "Configuring");
    assert_eq!(format!("{}", DeviceState::Activated), "Activated");
    assert_eq!(format!("{}", DeviceState::Deactivating), "Deactivating");
    assert_eq!(format!("{}", DeviceState::Failed), "Failed");
    assert_eq!(format!("{}", DeviceState::Other(99)), "Other(99)");
}

#[test]
fn wifi_security_open() {
    let open = WifiSecurity::Open;
    assert!(!open.secured());
    assert!(!open.is_psk());
    assert!(!open.is_eap());
}

#[test]
fn wifi_security_psk() {
    let psk = WifiSecurity::WpaPsk {
        psk: "password123".into(),
    };
    assert!(psk.secured());
    assert!(psk.is_psk());
    assert!(!psk.is_eap());
}

#[test]
fn wifi_security_eap() {
    let eap = WifiSecurity::WpaEap {
        opts: EapOptions {
            identity: "user@example.com".into(),
            password: "secret".into(),
            anonymous_identity: None,
            domain_suffix_match: None,
            ca_cert_path: None,
            ca_cert_blob: None,
            system_ca_certs: false,
            method: EapMethod::Peap,
            phase2: Phase2::Mschapv2,
            private_key_path: None,
            private_key_blob: None,
            private_key_password: None,
            client_cert_path: None,
            client_cert_blob: None,
        },
    };
    assert!(eap.secured());
    assert!(!eap.is_psk());
    assert!(eap.is_eap());
}

#[test]
fn wifi_security_eap_192bit() {
    let eap = WifiSecurity::Wpa3Eap192bit {
        opts: EapOptions {
            identity: "user@example.com".into(),
            password: "".into(),
            anonymous_identity: None,
            domain_suffix_match: None,
            ca_cert_path: Some("file:///etc/ssl/certs/ca.crt".into()),
            ca_cert_blob: None,
            system_ca_certs: false,
            method: EapMethod::Tls,
            phase2: Phase2::Mschapv2,
            private_key_path: Some("file:///etc/ssl/private/client.key".into()),
            private_key_blob: None,
            private_key_password: Some("password".into()),
            client_cert_path: Some("file:///etc/ssl/certs/client.crt".into()),
            client_cert_blob: None,
        },
    };
    assert!(eap.secured());
    assert!(!eap.is_psk());
    assert!(eap.is_eap());
}

#[test]
fn state_reason_from_u32_known_codes() {
    assert_eq!(StateReason::from(0), StateReason::Unknown);
    assert_eq!(StateReason::from(1), StateReason::None);
    assert_eq!(StateReason::from(7), StateReason::SupplicantDisconnected);
    assert_eq!(StateReason::from(8), StateReason::SupplicantConfigFailed);
    assert_eq!(StateReason::from(9), StateReason::SupplicantFailed);
    assert_eq!(StateReason::from(10), StateReason::SupplicantTimeout);
    assert_eq!(StateReason::from(16), StateReason::DhcpError);
    assert_eq!(StateReason::from(17), StateReason::DhcpFailed);
    assert_eq!(StateReason::from(70), StateReason::SsidNotFound);
    assert_eq!(StateReason::from(76), StateReason::SimPinIncorrect);
}

#[test]
fn state_reason_from_u32_unknown_code() {
    assert_eq!(StateReason::from(999), StateReason::Other(999));
    assert_eq!(StateReason::from(255), StateReason::Other(255));
}

#[test]
fn state_reason_display() {
    assert_eq!(format!("{}", StateReason::Unknown), "unknown");
    assert_eq!(
        format!("{}", StateReason::SupplicantFailed),
        "supplicant failed"
    );
    assert_eq!(format!("{}", StateReason::DhcpFailed), "DHCP failed");
    assert_eq!(format!("{}", StateReason::SsidNotFound), "SSID not found");
    assert_eq!(
        format!("{}", StateReason::Other(123)),
        "unknown reason (123)"
    );
}

#[test]
fn reason_to_error_auth_failures() {
    assert!(matches!(reason_to_error(9), ConnectionError::AuthFailed));
    assert!(matches!(reason_to_error(7), ConnectionError::AuthFailed));
    assert!(matches!(reason_to_error(76), ConnectionError::AuthFailed));
    assert!(matches!(reason_to_error(51), ConnectionError::AuthFailed));
}

#[test]
fn reason_to_error_supplicant_config() {
    assert!(matches!(
        reason_to_error(8),
        ConnectionError::SupplicantConfigFailed
    ));
}

#[test]
fn reason_to_error_supplicant_timeout() {
    assert!(matches!(
        reason_to_error(10),
        ConnectionError::SupplicantTimeout
    ));
}

#[test]
fn reason_to_error_dhcp_failures() {
    assert!(matches!(reason_to_error(15), ConnectionError::DhcpFailed));
    assert!(matches!(reason_to_error(16), ConnectionError::DhcpFailed));
    assert!(matches!(reason_to_error(17), ConnectionError::DhcpFailed));
}

#[test]
fn reason_to_error_network_not_found() {
    assert!(matches!(reason_to_error(70), ConnectionError::NotFound));
}

#[test]
fn reason_to_error_generic_failure() {
    match reason_to_error(2) {
        ConnectionError::DeviceFailed(reason) => {
            assert_eq!(reason, StateReason::UserDisconnected);
        }
        _ => panic!("expected ConnectionError::Failed"),
    }
}

#[test]
fn connection_error_display() {
    assert_eq!(
        format!("{}", ConnectionError::NotFound),
        "network not found"
    );
    assert_eq!(
        format!("{}", ConnectionError::AuthFailed),
        "authentication failed"
    );
    assert_eq!(format!("{}", ConnectionError::DhcpFailed), "DHCP failed");
    assert_eq!(
        format!("{}", ConnectionError::Timeout),
        "connection timeout"
    );
    assert_eq!(
        format!("{}", ConnectionError::NoWifiDevice),
        "no Wi-Fi device found"
    );
    assert_eq!(
        format!("{}", ConnectionError::Stuck("config".into())),
        "connection stuck in state: config"
    );
    assert_eq!(
        format!(
            "{}",
            ConnectionError::DeviceFailed(StateReason::CarrierChanged)
        ),
        "connection failed: carrier changed"
    );
}

#[test]
fn active_connection_state_from_u32() {
    assert_eq!(
        ActiveConnectionState::from(0),
        ActiveConnectionState::Unknown
    );
    assert_eq!(
        ActiveConnectionState::from(1),
        ActiveConnectionState::Activating
    );
    assert_eq!(
        ActiveConnectionState::from(2),
        ActiveConnectionState::Activated
    );
    assert_eq!(
        ActiveConnectionState::from(3),
        ActiveConnectionState::Deactivating
    );
    assert_eq!(
        ActiveConnectionState::from(4),
        ActiveConnectionState::Deactivated
    );
    assert_eq!(
        ActiveConnectionState::from(99),
        ActiveConnectionState::Other(99)
    );
}

#[test]
fn active_connection_state_display() {
    assert_eq!(format!("{}", ActiveConnectionState::Unknown), "unknown");
    assert_eq!(
        format!("{}", ActiveConnectionState::Activating),
        "activating"
    );
    assert_eq!(format!("{}", ActiveConnectionState::Activated), "activated");
    assert_eq!(
        format!("{}", ActiveConnectionState::Deactivating),
        "deactivating"
    );
    assert_eq!(
        format!("{}", ActiveConnectionState::Deactivated),
        "deactivated"
    );
    assert_eq!(
        format!("{}", ActiveConnectionState::Other(42)),
        "unknown state (42)"
    );
}

#[test]
fn connection_state_reason_from_u32() {
    assert_eq!(
        ConnectionStateReason::from(0),
        ConnectionStateReason::Unknown
    );
    assert_eq!(ConnectionStateReason::from(1), ConnectionStateReason::None);
    assert_eq!(
        ConnectionStateReason::from(2),
        ConnectionStateReason::UserDisconnected
    );
    assert_eq!(
        ConnectionStateReason::from(3),
        ConnectionStateReason::DeviceDisconnected
    );
    assert_eq!(
        ConnectionStateReason::from(6),
        ConnectionStateReason::ConnectTimeout
    );
    assert_eq!(
        ConnectionStateReason::from(9),
        ConnectionStateReason::NoSecrets
    );
    assert_eq!(
        ConnectionStateReason::from(10),
        ConnectionStateReason::LoginFailed
    );
    assert_eq!(
        ConnectionStateReason::from(99),
        ConnectionStateReason::Other(99)
    );
}

#[test]
fn connection_state_reason_display() {
    assert_eq!(format!("{}", ConnectionStateReason::Unknown), "unknown");
    assert_eq!(
        format!("{}", ConnectionStateReason::NoSecrets),
        "no secrets (password) provided"
    );
    assert_eq!(
        format!("{}", ConnectionStateReason::LoginFailed),
        "login/authentication failed"
    );
    assert_eq!(
        format!("{}", ConnectionStateReason::ConnectTimeout),
        "connection timed out"
    );
    assert_eq!(
        format!("{}", ConnectionStateReason::Other(123)),
        "unknown reason (123)"
    );
}

#[test]
fn connection_state_reason_to_error_auth_failures() {
    assert!(matches!(
        connection_state_reason_to_error(9),
        ConnectionError::AuthFailed
    ));
    assert!(matches!(
        connection_state_reason_to_error(10),
        ConnectionError::AuthFailed
    ));
}

#[test]
fn connection_state_reason_to_error_timeout() {
    assert!(matches!(
        connection_state_reason_to_error(6),
        ConnectionError::Timeout
    ));
    assert!(matches!(
        connection_state_reason_to_error(7),
        ConnectionError::Timeout
    ));
}

#[test]
fn connection_state_reason_to_error_dhcp() {
    assert!(matches!(
        connection_state_reason_to_error(5),
        ConnectionError::DhcpFailed
    ));
}

#[test]
fn connection_state_reason_to_error_generic() {
    match connection_state_reason_to_error(2) {
        ConnectionError::ActivationFailed(reason) => {
            assert_eq!(reason, ConnectionStateReason::UserDisconnected);
        }
        _ => panic!("expected ConnectionError::ConnectionFailed"),
    }
}

#[test]
fn connection_failed_error_display() {
    assert_eq!(
        format!(
            "{}",
            ConnectionError::ActivationFailed(ConnectionStateReason::NoSecrets)
        ),
        "connection activation failed: no secrets (password) provided"
    );
}

#[test]
fn test_bluetooth_network_role_from_u32() {
    assert_eq!(BluetoothNetworkRole::from(0), BluetoothNetworkRole::PanU);
    assert_eq!(BluetoothNetworkRole::from(1), BluetoothNetworkRole::Dun);
    assert_eq!(BluetoothNetworkRole::from(999), BluetoothNetworkRole::PanU);
}

#[test]
fn test_bluetooth_network_role_display() {
    assert_eq!(format!("{}", BluetoothNetworkRole::PanU), "PANU");
    assert_eq!(format!("{}", BluetoothNetworkRole::Dun), "DUN");
}

#[test]
fn test_bluetooth_identity_creation() {
    let identity =
        BluetoothIdentity::new("00:1A:7D:DA:71:13".into(), BluetoothNetworkRole::PanU).unwrap();

    assert_eq!(identity.bdaddr, "00:1A:7D:DA:71:13");
    assert!(matches!(
        identity.bt_device_type,
        BluetoothNetworkRole::PanU
    ));
}

#[test]
fn test_bluetooth_identity_dun() {
    let identity =
        BluetoothIdentity::new("C8:1F:E8:F0:51:57".into(), BluetoothNetworkRole::Dun).unwrap();

    assert_eq!(identity.bdaddr, "C8:1F:E8:F0:51:57");
    assert!(matches!(identity.bt_device_type, BluetoothNetworkRole::Dun));
}

#[test]
fn test_bluetooth_identity_creation_error() {
    let error =
        BluetoothIdentity::new("SomeInvalidAddress".into(), BluetoothNetworkRole::Dun).unwrap_err();
    assert!(matches!(
        error,
        ConnectionError::InvalidAddress(message)
            if message
                == "Invalid Bluetooth Address 'SomeInvalidAddress' (must have 6 segments)"
    ));
}

#[test]
fn test_bluetooth_device_creation() {
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
    assert_eq!(device.bt_caps, role);
    assert_eq!(device.state, DeviceState::Activated);
}

#[test]
fn test_bluetooth_device_display() {
    let role = BluetoothNetworkRole::PanU as u32;
    let device = BluetoothDevice::new(
        "00:1A:7D:DA:71:13".into(),
        Some("MyPhone".into()),
        Some("Phone".into()),
        role,
        DeviceState::Activated,
    );

    let display_str = format!("{}", device);
    assert!(display_str.contains("Phone"));
    assert!(display_str.contains("00:1A:7D:DA:71:13"));
    assert!(display_str.contains("PANU"));
}

#[test]
fn test_bluetooth_device_display_no_alias() {
    let role = BluetoothNetworkRole::Dun as u32;
    let device = BluetoothDevice::new(
        "00:1A:7D:DA:71:13".into(),
        Some("MyPhone".into()),
        None,
        role,
        DeviceState::Disconnected,
    );

    let display_str = format!("{}", device);
    assert!(display_str.contains("unknown"));
    assert!(display_str.contains("00:1A:7D:DA:71:13"));
    assert!(display_str.contains("DUN"));
}

fn device_with_type(device_type: DeviceType) -> Device {
    Device {
        path: "/org/freedesktop/NetworkManager/Devices/1".into(),
        interface: "test0".into(),
        identity: DeviceIdentity::new("00:1A:7D:DA:71:13".into(), "test0".into()),
        device_type,
        state: DeviceState::Activated,
        managed: Some(true),
        driver: Some("test".into()),
        ip4_address: None,
        ip6_address: None,
        frequency: None,
        speed_mbps: None,
    }
}

#[test]
fn device_transport_predicates_are_mutually_exclusive() {
    let cases = [
        (DeviceType::Ethernet, (true, false, false)),
        (DeviceType::Other(20), (true, false, false)),
        (DeviceType::Wifi, (false, true, false)),
        (DeviceType::Bluetooth, (false, false, true)),
        (DeviceType::Loopback, (false, false, false)),
    ];

    for (device_type, expected) in cases {
        let device = device_with_type(device_type);
        assert_eq!(
            (
                device.is_wired(),
                device.is_wireless(),
                device.is_bluetooth(),
            ),
            expected
        );
    }
}

#[test]
fn test_connection_error_no_bluetooth_device() {
    let err = ConnectionError::NoBluetoothDevice;
    assert_eq!(format!("{}", err), "Bluetooth device not found");
}

fn assert_wireguard_peer(
    peer: &WireGuardPeer,
    public_key: &str,
    gateway: &str,
    allowed_ips: &[&str],
    preshared_key: Option<&str>,
    persistent_keepalive: Option<u32>,
) {
    assert_eq!(peer.public_key, public_key);
    assert_eq!(peer.gateway, gateway);
    assert_eq!(
        peer.allowed_ips,
        allowed_ips
            .iter()
            .map(|address| (*address).to_string())
            .collect::<Vec<_>>()
    );
    assert_eq!(peer.preshared_key.as_deref(), preshared_key);
    assert_eq!(peer.persistent_keepalive, persistent_keepalive);
}

#[test]
fn test_vpn_credentials_builder_basic() {
    let peer = WireGuardPeer::new(
        "HIgo9xNzJMWLKAShlKl6/bUT1VI9Q0SDBXGtLXkPFXc=",
        "vpn.example.com:51820",
        vec!["0.0.0.0/0".into()],
    );

    let creds = VpnCredentials::builder()
        .name("TestVPN")
        .wireguard()
        .gateway("vpn.example.com:51820")
        .private_key("YBk6X3pP8KjKz7+HFWzVHNqL3qTZq8hX9VxFQJ4zVmM=")
        .address("10.0.0.2/24")
        .add_peer(peer)
        .build()
        .unwrap();

    assert_eq!(creds.name, "TestVPN");
    assert_eq!(creds.vpn_type, VpnKind::WireGuard);
    assert_eq!(creds.gateway, "vpn.example.com:51820");
    assert_eq!(
        creds.private_key,
        "YBk6X3pP8KjKz7+HFWzVHNqL3qTZq8hX9VxFQJ4zVmM="
    );
    assert_eq!(creds.address, "10.0.0.2/24");
    assert_eq!(creds.peers.len(), 1);
    assert_wireguard_peer(
        &creds.peers[0],
        "HIgo9xNzJMWLKAShlKl6/bUT1VI9Q0SDBXGtLXkPFXc=",
        "vpn.example.com:51820",
        &["0.0.0.0/0"],
        None,
        None,
    );
    assert!(creds.dns.is_none());
    assert!(creds.mtu.is_none());
}

#[test]
fn test_wireguard_config_basic() {
    let peer = WireGuardPeer::new(
        "HIgo9xNzJMWLKAShlKl6/bUT1VI9Q0SDBXGtLXkPFXc=",
        "vpn.example.com:51820",
        vec!["0.0.0.0/0".into()],
    );

    let config = WireGuardConfig::new(
        "TestVPN",
        "vpn.example.com:51820",
        "YBk6X3pP8KjKz7+HFWzVHNqL3qTZq8hX9VxFQJ4zVmM=",
        "10.0.0.2/24",
        vec![peer],
    );

    assert_eq!(config.name, "TestVPN");
    assert_eq!(config.gateway, "vpn.example.com:51820");
    assert_eq!(
        config.private_key,
        "YBk6X3pP8KjKz7+HFWzVHNqL3qTZq8hX9VxFQJ4zVmM="
    );
    assert_eq!(config.address, "10.0.0.2/24");
    assert_eq!(config.peers.len(), 1);
    assert_wireguard_peer(
        &config.peers[0],
        "HIgo9xNzJMWLKAShlKl6/bUT1VI9Q0SDBXGtLXkPFXc=",
        "vpn.example.com:51820",
        &["0.0.0.0/0"],
        None,
        None,
    );
    assert!(config.dns.is_none());
    assert!(config.mtu.is_none());
}

#[test]
fn test_wireguard_config_implements_vpn_config() {
    let uuid = Uuid::new_v4();
    let config = WireGuardConfig::new(
        "TestVPN",
        "vpn.example.com:51820",
        "private_key",
        "10.0.0.2/24",
        vec![WireGuardPeer::new(
            "public_key",
            "vpn.example.com:51820",
            vec!["0.0.0.0/0".into()],
        )],
    )
    .with_dns(vec!["1.1.1.1".into(), "8.8.8.8".into()])
    .with_mtu(1420)
    .with_uuid(uuid);

    let vpn_config: &dyn VpnConfig = &config;

    assert_eq!(vpn_config.vpn_kind(), VpnKind::WireGuard);
    assert_eq!(vpn_config.name(), "TestVPN");
    assert_eq!(
        vpn_config.dns(),
        Some(["1.1.1.1".to_string(), "8.8.8.8".to_string()].as_slice())
    );
    assert_eq!(vpn_config.mtu(), Some(1420));
    assert_eq!(vpn_config.uuid(), Some(uuid));
}

#[test]
fn test_wireguard_config_roundtrips_through_vpn_credentials() {
    let uuid = Uuid::new_v4();
    let config = WireGuardConfig::new(
        "TestVPN",
        "vpn.example.com:51820",
        "private_key",
        "10.0.0.2/24",
        vec![
            WireGuardPeer::new(
                "public_key",
                "vpn.example.com:51820",
                vec!["0.0.0.0/0".into(), "10.0.0.0/8".into()],
            )
            .with_preshared_key("preshared_key")
            .with_persistent_keepalive(25),
        ],
    )
    .with_dns(vec!["1.1.1.1".into()])
    .with_mtu(1420)
    .with_uuid(uuid);

    let legacy: VpnCredentials = config.clone().into();
    let roundtrip = WireGuardConfig::from(legacy);

    assert_eq!(roundtrip.name, config.name);
    assert_eq!(roundtrip.gateway, config.gateway);
    assert_eq!(roundtrip.private_key, config.private_key);
    assert_eq!(roundtrip.address, config.address);
    assert_eq!(roundtrip.peers.len(), 1);
    assert_wireguard_peer(
        &roundtrip.peers[0],
        "public_key",
        "vpn.example.com:51820",
        &["0.0.0.0/0", "10.0.0.0/8"],
        Some("preshared_key"),
        Some(25),
    );
    assert_eq!(roundtrip.dns, config.dns);
    assert_eq!(roundtrip.mtu, config.mtu);
    assert_eq!(roundtrip.uuid, Some(uuid));
}

#[test]
fn test_vpn_credentials_builder_with_optionals() {
    let peer = WireGuardPeer::new(
        "public_key",
        "vpn.example.com:51820",
        vec!["0.0.0.0/0".into()],
    );

    let uuid = Uuid::new_v4();
    let creds = VpnCredentials::builder()
        .name("TestVPN")
        .wireguard()
        .gateway("vpn.example.com:51820")
        .private_key("private_key")
        .address("10.0.0.2/24")
        .add_peer(peer)
        .with_dns(vec!["1.1.1.1".into(), "8.8.8.8".into()])
        .with_mtu(1420)
        .with_uuid(uuid)
        .build()
        .unwrap();

    assert_eq!(creds.dns, Some(vec!["1.1.1.1".into(), "8.8.8.8".into()]));
    assert_eq!(creds.mtu, Some(1420));
    assert_eq!(creds.uuid, Some(uuid));
}

#[test]
fn test_vpn_credentials_builder_multiple_peers() {
    let peer1 = WireGuardPeer::new(
        "key1",
        "vpn1.example.com:51820",
        vec!["10.0.0.0/24".into(), "10.0.1.0/24".into()],
    )
    .with_persistent_keepalive(15);
    let peer2 = WireGuardPeer::new(
        "key2",
        "vpn2.example.com:51820",
        vec!["192.168.0.0/24".into()],
    )
    .with_preshared_key("peer2-psk");

    let creds = VpnCredentials::builder()
        .name("MultiPeerVPN")
        .wireguard()
        .gateway("vpn.example.com:51820")
        .private_key("private_key")
        .address("10.0.0.2/24")
        .add_peer(peer1)
        .add_peer(peer2)
        .build()
        .unwrap();

    assert_eq!(creds.peers.len(), 2);
    assert_wireguard_peer(
        &creds.peers[0],
        "key1",
        "vpn1.example.com:51820",
        &["10.0.0.0/24", "10.0.1.0/24"],
        None,
        Some(15),
    );
    assert_wireguard_peer(
        &creds.peers[1],
        "key2",
        "vpn2.example.com:51820",
        &["192.168.0.0/24"],
        Some("peer2-psk"),
        None,
    );
}

#[test]
fn test_vpn_credentials_builder_peers_method() {
    let peers = vec![
        WireGuardPeer::new("key1", "vpn1.example.com:51820", vec!["0.0.0.0/0".into()])
            .with_persistent_keepalive(20),
        WireGuardPeer::new("key2", "vpn2.example.com:51821", vec!["::/0".into()])
            .with_preshared_key("key2-psk"),
    ];

    let creds = VpnCredentials::builder()
        .name("TestVPN")
        .wireguard()
        .gateway("vpn.example.com:51820")
        .private_key("private_key")
        .address("10.0.0.2/24")
        .peers(peers)
        .build()
        .unwrap();

    assert_eq!(creds.peers.len(), 2);
    assert_wireguard_peer(
        &creds.peers[0],
        "key1",
        "vpn1.example.com:51820",
        &["0.0.0.0/0"],
        None,
        Some(20),
    );
    assert_wireguard_peer(
        &creds.peers[1],
        "key2",
        "vpn2.example.com:51821",
        &["::/0"],
        Some("key2-psk"),
        None,
    );
}

#[test]
fn test_vpn_credentials_builder_missing_name() {
    let peer = WireGuardPeer::new("key", "vpn.example.com:51820", vec!["0.0.0.0/0".into()]);

    let err = VpnCredentials::builder()
        .wireguard()
        .gateway("vpn.example.com:51820")
        .private_key("private_key")
        .address("10.0.0.2/24")
        .add_peer(peer)
        .build()
        .unwrap_err();
    assert!(matches!(
        err,
        ConnectionError::IncompleteBuilder(message)
            if message == "connection name is required (use .name())"
    ));
}

#[test]
fn test_vpn_credentials_builder_missing_vpn_type() {
    let peer = WireGuardPeer::new("key", "vpn.example.com:51820", vec!["0.0.0.0/0".into()]);

    let err = VpnCredentials::builder()
        .name("TestVPN")
        .gateway("vpn.example.com:51820")
        .private_key("private_key")
        .address("10.0.0.2/24")
        .add_peer(peer)
        .build()
        .unwrap_err();
    assert!(matches!(
        err,
        ConnectionError::IncompleteBuilder(message)
            if message == "VPN type is required (use .wireguard())"
    ));
}

#[test]
fn test_vpn_credentials_builder_missing_peers() {
    let err = VpnCredentials::builder()
        .name("TestVPN")
        .wireguard()
        .gateway("vpn.example.com:51820")
        .private_key("private_key")
        .address("10.0.0.2/24")
        .build()
        .unwrap_err();
    assert!(matches!(
        err,
        ConnectionError::InvalidPeers(message)
            if message == "at least one peer is required (use .add_peer())"
    ));
}

#[test]
fn test_eap_options_builder_basic() {
    let opts = EapOptions::builder()
        .identity("user@example.com")
        .password("password")
        .method(EapMethod::Peap)
        .phase2(Phase2::Mschapv2)
        .build()
        .unwrap();

    assert_eq!(opts.identity, "user@example.com");
    assert_eq!(opts.password, "password");
    assert_eq!(opts.method, EapMethod::Peap);
    assert_eq!(opts.phase2, Phase2::Mschapv2);
    assert!(opts.anonymous_identity.is_none());
    assert!(opts.domain_suffix_match.is_none());
    assert!(opts.ca_cert_path.is_none());
    assert!(!opts.system_ca_certs);
}

#[test]
fn test_eap_options_builder_with_optionals() {
    let opts = EapOptions::builder()
        .identity("user@company.com")
        .password("password")
        .method(EapMethod::Ttls)
        .phase2(Phase2::Pap)
        .anonymous_identity("anonymous@company.com")
        .domain_suffix_match("company.com")
        .ca_cert_path("file:///etc/ssl/certs/ca.pem")
        .system_ca_certs(true)
        .build()
        .unwrap();

    assert_eq!(opts.identity, "user@company.com");
    assert_eq!(opts.password, "password");
    assert_eq!(opts.method, EapMethod::Ttls);
    assert_eq!(opts.phase2, Phase2::Pap);
    assert_eq!(
        opts.anonymous_identity,
        Some("anonymous@company.com".into())
    );
    assert_eq!(opts.domain_suffix_match, Some("company.com".into()));
    assert_eq!(
        opts.ca_cert_path,
        Some("file:///etc/ssl/certs/ca.pem".into())
    );
    assert!(opts.system_ca_certs);
}

#[test]
fn test_eap_options_builder_peap_mschapv2() {
    let opts = EapOptions::builder()
        .identity("employee@corp.com")
        .password("secret")
        .method(EapMethod::Peap)
        .phase2(Phase2::Mschapv2)
        .system_ca_certs(true)
        .build()
        .unwrap();

    assert_eq!(opts.method, EapMethod::Peap);
    assert_eq!(opts.phase2, Phase2::Mschapv2);
    assert!(opts.system_ca_certs);
}

#[test]
fn test_eap_options_builder_ttls_pap() {
    let opts = EapOptions::builder()
        .identity("student@university.edu")
        .password("password")
        .method(EapMethod::Ttls)
        .phase2(Phase2::Pap)
        .ca_cert_path("file:///etc/ssl/certs/university.pem")
        .build()
        .unwrap();

    assert_eq!(opts.method, EapMethod::Ttls);
    assert_eq!(opts.phase2, Phase2::Pap);
    assert_eq!(
        opts.ca_cert_path,
        Some("file:///etc/ssl/certs/university.pem".into())
    );
}

#[test]
fn test_eap_options_builder_tls() {
    let opts = EapOptions::builder()
        .identity("student@university.edu")
        .method(EapMethod::Tls)
        .ca_cert_path("file:///etc/ssl/certs/ca.pem")
        .private_key_path("file:///etc/ssl/private/client.key")
        .private_key_password("password")
        .client_cert_path("file:///etc/ssl/certs/client.pem")
        .build()
        .unwrap();

    assert_eq!(opts.method, EapMethod::Tls);
    assert_eq!(
        opts.ca_cert_path,
        Some("file:///etc/ssl/certs/ca.pem".into())
    );
    assert_eq!(
        opts.private_key_path,
        Some("file:///etc/ssl/private/client.key".into())
    );
    assert_eq!(opts.private_key_password, Some("password".into()));
    assert_eq!(
        opts.client_cert_path,
        Some("file:///etc/ssl/certs/client.pem".into())
    );
}

#[test]
fn test_eap_options_builder_tls_missing_private_key() {
    let err = EapOptions::builder()
        .identity("student@university.edu")
        .method(EapMethod::Tls)
        .client_cert_path("file:///etc/ssl/certs/client.pem")
        .build()
        .unwrap_err();

    match err {
        ConnectionError::IncompleteBuilder(message) => assert!(message.contains("private key")),
        err => panic!("expected IncompleteBuilder, got {err:?}"),
    }
}

#[test]
fn test_eap_options_builder_tls_missing_client_cert() {
    let err = EapOptions::builder()
        .identity("student@university.edu")
        .method(EapMethod::Tls)
        .private_key_path("file:///etc/ssl/private/client.key")
        .build()
        .unwrap_err();

    match err {
        ConnectionError::IncompleteBuilder(message) => {
            assert!(message.contains("client certificate"));
        }
        err => panic!("expected IncompleteBuilder, got {err:?}"),
    }
}

#[test]
fn test_eap_options_builder_ca_cert_blob_overrides_path() {
    let opts = EapOptions::builder()
        .identity("student@university.edu")
        .method(EapMethod::Tls)
        .ca_cert_path("file:///etc/ssl/certs/ca.pem")
        .ca_cert_blob(vec![1])
        .private_key_path("file:///etc/ssl/private/client.key")
        .private_key_password("password")
        .client_cert_path("file:///etc/ssl/certs/client.pem")
        .build()
        .unwrap();

    assert_eq!(opts.method, EapMethod::Tls);
    assert_eq!(opts.ca_cert_path, None);
    assert_eq!(opts.ca_cert_blob, Some(vec![1]));
}

#[test]
fn test_eap_options_builder_path_blob_private_key() {
    let opts = EapOptions::builder()
        .identity("student@university.edu")
        .method(EapMethod::Tls)
        .ca_cert_path("file:///etc/ssl/certs/ca.pem")
        .private_key_path("file:///etc/ssl/private/client.key")
        .private_key_blob(vec![1])
        .private_key_password("password")
        .client_cert_path("file:///etc/ssl/certs/client.pem")
        .build()
        .unwrap();

    assert_eq!(opts.method, EapMethod::Tls);
    assert_eq!(opts.private_key_path, None);
    assert_eq!(opts.private_key_blob, Some(vec![1]));
}

#[test]
fn test_eap_options_builder_path_blob_client_cert() {
    let opts = EapOptions::builder()
        .identity("student@university.edu")
        .method(EapMethod::Tls)
        .ca_cert_path("file:///etc/ssl/certs/ca.pem")
        .private_key_path("file:///etc/ssl/private/client.key")
        .private_key_password("password")
        .client_cert_path("file:///etc/ssl/certs/client.pem")
        .client_cert_blob(vec![1])
        .build()
        .unwrap();

    assert_eq!(opts.method, EapMethod::Tls);
    assert_eq!(opts.client_cert_path, None);
    assert_eq!(opts.client_cert_blob, Some(vec![1]));
}

#[test]
fn test_eap_options_builder_missing_identity() {
    let err = EapOptions::builder()
        .password("password")
        .method(EapMethod::Peap)
        .phase2(Phase2::Mschapv2)
        .build()
        .unwrap_err();
    assert!(matches!(
        err,
        ConnectionError::IncompleteBuilder(message)
            if message == "EAP identity is required (use .identity())"
    ));
}

#[test]
fn test_eap_options_builder_missing_password() {
    let err = EapOptions::builder()
        .identity("user@example.com")
        .method(EapMethod::Peap)
        .phase2(Phase2::Mschapv2)
        .build()
        .unwrap_err();
    assert!(matches!(
        err,
        ConnectionError::IncompleteBuilder(message)
            if message == "EAP password is required (use .password())"
    ));
}

#[test]
fn test_eap_options_builder_missing_method() {
    let err = EapOptions::builder()
        .identity("user@example.com")
        .password("password")
        .phase2(Phase2::Mschapv2)
        .build()
        .unwrap_err();
    assert!(matches!(
        err,
        ConnectionError::IncompleteBuilder(message)
            if message == "EAP method is required (use .method())"
    ));
}

#[test]
fn test_eap_options_builder_missing_phase2() {
    let err = EapOptions::builder()
        .identity("user@example.com")
        .password("password")
        .method(EapMethod::Peap)
        .build()
        .unwrap_err();
    assert!(matches!(
        err,
        ConnectionError::IncompleteBuilder(message)
            if message == "EAP phase 2 method is required (use .phase2())"
    ));
}

#[test]
fn test_eap_options_builder_equivalence_to_new() {
    let opts_new = EapOptions::new("user@example.com", "password")
        .with_method(EapMethod::Peap)
        .with_phase2(Phase2::Mschapv2);

    let opts_builder = EapOptions::builder()
        .identity("user@example.com")
        .password("password")
        .method(EapMethod::Peap)
        .phase2(Phase2::Mschapv2)
        .build()
        .unwrap();

    assert_eq!(opts_new.identity, opts_builder.identity);
    assert_eq!(opts_new.password, opts_builder.password);
    assert_eq!(opts_new.method, opts_builder.method);
    assert_eq!(opts_new.phase2, opts_builder.phase2);
}

#[test]
fn test_vpn_credentials_builder_equivalence_to_new() {
    let peer = WireGuardPeer::new(
        "public_key",
        "vpn.example.com:51820",
        vec!["0.0.0.0/0".into()],
    );

    let creds_new = VpnCredentials::new(
        VpnKind::WireGuard,
        "TestVPN",
        "vpn.example.com:51820",
        "private_key",
        "10.0.0.2/24",
        vec![peer.clone()],
    );

    let creds_builder = VpnCredentials::builder()
        .name("TestVPN")
        .wireguard()
        .gateway("vpn.example.com:51820")
        .private_key("private_key")
        .address("10.0.0.2/24")
        .add_peer(peer)
        .build()
        .unwrap();

    assert_eq!(creds_new.name, creds_builder.name);
    assert_eq!(creds_new.vpn_type, creds_builder.vpn_type);
    assert_eq!(creds_new.gateway, creds_builder.gateway);
    assert_eq!(creds_new.private_key, creds_builder.private_key);
    assert_eq!(creds_new.address, creds_builder.address);
    assert_eq!(creds_new.peers.len(), 1);
    assert_eq!(creds_builder.peers.len(), 1);
    assert_wireguard_peer(
        &creds_new.peers[0],
        "public_key",
        "vpn.example.com:51820",
        &["0.0.0.0/0"],
        None,
        None,
    );
    assert_wireguard_peer(
        &creds_builder.peers[0],
        "public_key",
        "vpn.example.com:51820",
        &["0.0.0.0/0"],
        None,
        None,
    );
    assert_eq!(creds_new.dns, creds_builder.dns);
    assert_eq!(creds_new.mtu, creds_builder.mtu);
    assert_eq!(creds_new.uuid, creds_builder.uuid);
}

#[test]
fn test_timeout_config_default() {
    let config = TimeoutConfig::default();
    assert_eq!(config.connection_timeout, Duration::from_secs(30));
    assert_eq!(config.disconnect_timeout, Duration::from_secs(10));
}

#[test]
fn test_timeout_config_setters_compose_and_last_value_wins() {
    let config = TimeoutConfig::new()
        .with_connection_timeout(Duration::from_secs(45))
        .with_disconnect_timeout(Duration::from_secs(15))
        .with_connection_timeout(Duration::from_secs(60));

    assert_eq!(config.connection_timeout, Duration::from_secs(60));
    assert_eq!(config.disconnect_timeout, Duration::from_secs(15));
}

#[test]
fn test_device_state_is_transitional() {
    let transitional = [
        DeviceState::Prepare,
        DeviceState::Config,
        DeviceState::NeedAuth,
        DeviceState::IpConfig,
        DeviceState::IpCheck,
        DeviceState::Secondaries,
        DeviceState::Deactivating,
    ];
    for state in &transitional {
        assert!(state.is_transitional(), "{state:?} should be transitional");
    }

    let stable = [
        DeviceState::Unmanaged,
        DeviceState::Unavailable,
        DeviceState::Disconnected,
        DeviceState::Activated,
        DeviceState::Failed,
        DeviceState::Other(999),
    ];
    for state in &stable {
        assert!(
            !state.is_transitional(),
            "{state:?} should not be transitional"
        );
    }
}

#[test]
fn test_device_state_is_enabled() {
    let enabled = [
        DeviceState::Disconnected,
        DeviceState::Prepare,
        DeviceState::Config,
        DeviceState::NeedAuth,
        DeviceState::IpConfig,
        DeviceState::IpCheck,
        DeviceState::Secondaries,
        DeviceState::Activated,
        DeviceState::Deactivating,
    ];
    for state in &enabled {
        assert!(state.is_enabled(), "{state:?} should be enabled");
    }

    let disabled = [
        DeviceState::Unmanaged,
        DeviceState::Unavailable,
        DeviceState::Failed,
        DeviceState::Other(999),
    ];
    for state in &disabled {
        assert!(!state.is_enabled(), "{state:?} should not be enabled");
    }
}

#[test]
fn test_device_state_from_u32_intermediate_states() {
    assert_eq!(DeviceState::from(40), DeviceState::Prepare);
    assert_eq!(DeviceState::from(50), DeviceState::Config);
    assert_eq!(DeviceState::from(60), DeviceState::NeedAuth);
    assert_eq!(DeviceState::from(70), DeviceState::IpConfig);
    assert_eq!(DeviceState::from(80), DeviceState::IpCheck);
    assert_eq!(DeviceState::from(90), DeviceState::Secondaries);
    assert_eq!(DeviceState::from(110), DeviceState::Deactivating);
}
