use std::{collections::HashMap, net::Ipv4Addr};

use nmrs::settings::common::{BoolWithDefault, DnsOverTls, IpMethod, Ipv4AddressData, Metered};
use nmrs::settings::ethernet::EthernetSettings;
use nmrs::settings::wifi::{FilsMode, WiFiSettings, WirelessBand, WirelessPowersave};
use nmrs::settings::{
    BluetoothSettings, BluetoothType, BondSettings, BridgeSettings, CdmaSettings, GsmSettings,
    VlanProtocol, VlanSettings, VpnSettings, WireGuardSettings,
};
use uuid::Uuid;
use zbus::zvariant::{OwnedValue, Value};

fn str_val(value: &str) -> OwnedValue {
    OwnedValue::try_from(Value::from(value.to_string())).unwrap()
}

fn u32_val(value: u32) -> OwnedValue {
    OwnedValue::from(value)
}
fn i32_val(value: i32) -> OwnedValue {
    OwnedValue::from(value)
}
fn bool_val(value: bool) -> OwnedValue {
    OwnedValue::from(value)
}

fn string_array_val(values: &[&str]) -> OwnedValue {
    OwnedValue::try_from(Value::from(
        values
            .iter()
            .map(|value| value.to_string())
            .collect::<Vec<_>>(),
    ))
    .unwrap()
}

fn build_dict(
    connection_id: &str,
    connection_uuid: Uuid,
    ssid: &[u8],
    ipv4_method: &str,
    ipv4_addresses: &[(&str, u32)],
    ipv4_dns: &[&str],
) -> HashMap<String, HashMap<String, OwnedValue>> {
    let mut dict = HashMap::new();
    let mut connection = HashMap::new();
    connection.insert("id".into(), str_val(connection_id));
    connection.insert("uuid".into(), str_val(&connection_uuid.to_string()));
    connection.insert("type".into(), str_val("802-11-wireless"));
    connection.insert("autoconnect".into(), bool_val(false));
    dict.insert("connection".into(), connection);

    let mut wireless = HashMap::new();
    wireless.insert(
        "ssid".into(),
        OwnedValue::try_from(Value::from(ssid.to_vec())).unwrap(),
    );
    wireless.insert("mode".into(), str_val("infrastructure"));
    wireless.insert("channel".into(), u32_val(0));
    wireless.insert("mtu".into(), u32_val(0));
    dict.insert("802-11-wireless".into(), wireless);

    let mut ipv4 = HashMap::new();
    ipv4.insert("method".into(), str_val(ipv4_method));
    if !ipv4_addresses.is_empty() {
        let entries: Vec<HashMap<&str, Value<'_>>> = ipv4_addresses
            .iter()
            .map(|(address, prefix)| {
                HashMap::from([
                    ("address", Value::from((*address).to_string())),
                    ("prefix", Value::from(*prefix)),
                ])
            })
            .collect();
        ipv4.insert(
            "address-data".into(),
            OwnedValue::try_from(Value::from(entries)).unwrap(),
        );
    }
    if !ipv4_dns.is_empty() {
        ipv4.insert("dns-data".into(), string_array_val(ipv4_dns));
    }
    dict.insert("ipv4".into(), ipv4);
    dict.insert(
        "ipv6".into(),
        HashMap::from([(String::from("method"), str_val("auto"))]),
    );
    dict
}

fn build_ethernet_dict(uuid: Uuid) -> HashMap<String, HashMap<String, OwnedValue>> {
    let mut dict = HashMap::new();
    dict.insert(
        "connection".into(),
        HashMap::from([
            (String::from("id"), str_val("eth0")),
            (String::from("uuid"), str_val(&uuid.to_string())),
            (String::from("type"), str_val("802-3-ethernet")),
        ]),
    );
    dict.insert(
        "802-3-ethernet".into(),
        HashMap::from([
            (String::from("auto-negotiate"), bool_val(false)),
            (String::from("mtu"), u32_val(1500)),
        ]),
    );
    dict.insert(
        "ipv4".into(),
        HashMap::from([(String::from("method"), str_val("auto"))]),
    );
    dict.insert(
        "ipv6".into(),
        HashMap::from([(String::from("method"), str_val("auto"))]),
    );
    dict
}

// Parsing and section serialization.
#[test]
fn parses_wifi_settings_minimal() {
    let uuid = Uuid::new_v4();
    let dict = build_dict("home-wifi", uuid, b"HomeWiFi", "auto", &[], &[]);

    let wifi = WiFiSettings::from_raw_dict(dict).expect("parse ok");
    assert_eq!(wifi.wifi.ssid, "HomeWiFi");
    assert_eq!(wifi.connection.name, "home-wifi");
    assert_eq!(wifi.connection.id, uuid);
    assert!(!wifi.connection.autoconnect);
    assert_eq!(wifi.ipv4.method, IpMethod::Auto);
    assert!(wifi.ipv4.address_data.is_empty());
    assert!(wifi.ipv4.dns_data.is_empty());
    assert_eq!(wifi.ipv6.method, IpMethod::Auto);
}

#[test]
fn parses_wifi_ipv4_manual_with_addresses_and_dns() {
    let uuid = Uuid::new_v4();
    let dict = build_dict(
        "test",
        uuid,
        b"Test",
        "manual",
        &[("192.168.1.10", 24)],
        &["8.8.8.8", "1.1.1.1"],
    );

    let wifi = WiFiSettings::from_raw_dict(dict).expect("parse ok");
    assert_eq!(wifi.ipv4.method, IpMethod::Manual);
    assert_eq!(wifi.ipv4.address_data.len(), 1);
    assert_eq!(
        wifi.ipv4.address_data[0].address.to_string(),
        "192.168.1.10"
    );
    assert_eq!(wifi.ipv4.address_data[0].prefix, 24);
    assert_eq!(wifi.ipv4.dns_data, vec!["8.8.8.8", "1.1.1.1"]);
}

#[test]
fn wifi_to_owned_dict_round_trips_basic_fields() {
    let uuid = Uuid::new_v4();
    let dict = build_dict("home", uuid, b"Home", "auto", &[], &[]);

    let wifi = WiFiSettings::from_raw_dict(dict).expect("parse ok");
    let out = wifi.to_owned_dict().expect("to_owned_dict ok");

    let conn = out.get("connection").unwrap();
    assert_eq!(conn.get("id").unwrap(), &str_val("home"));
    assert_eq!(conn.get("type").unwrap(), &str_val("802-11-wireless"));
    assert_eq!(conn.get("autoconnect").unwrap(), &bool_val(false));

    let ipv4 = out.get("ipv4").unwrap();
    assert_eq!(ipv4.get("method").unwrap(), &str_val("auto"));

    let wifi_sec = out.get("802-11-wireless").unwrap();
    let ssid_v = wifi_sec.get("ssid").unwrap();
    let sig = ssid_v.value_signature();
    let _ = sig;
}

#[test]
fn ethernet_settings_minimal() {
    let uuid = Uuid::new_v4();
    let dict = build_ethernet_dict(uuid);

    let eth_typed = EthernetSettings::from_raw_dict(dict).expect("parse ok");
    assert_eq!(eth_typed.connection.name, "eth0");
    assert_eq!(eth_typed.ethernet.mtu, 1500);
    assert!(!eth_typed.ethernet.auto_negotiate);
    assert_eq!(eth_typed.ipv4.method, IpMethod::Auto);
}

#[test]
fn ethernet_accept_all_mac_addresses_parses_as_ternary_int() {
    let uuid = Uuid::new_v4();
    let mut dict = build_ethernet_dict(uuid);
    let eth = dict.get_mut("802-3-ethernet").unwrap();
    eth.insert("accept-all-mac-addresses".to_string(), i32_val(1));

    let eth_typed = EthernetSettings::from_raw_dict(dict).expect("parse ok");
    assert_eq!(
        eth_typed.ethernet.accept_all_mac_addresses,
        BoolWithDefault::Yes
    );
    let out = eth_typed.to_owned_dict().expect("to_owned_dict ok");
    assert_eq!(
        out.get("802-3-ethernet")
            .unwrap()
            .get("accept-all-mac-addresses")
            .unwrap(),
        &i32_val(1)
    );
}

#[test]
fn ipv4_address_data_emits_with_aa_sv_signature() {
    let uuid = Uuid::new_v4();
    let dict = build_dict("home", uuid, b"Home", "manual", &[("10.0.0.1", 24)], &[]);

    let mut wifi = WiFiSettings::from_raw_dict(dict).expect("parse ok");
    wifi.ipv4.address_data.push(Ipv4AddressData {
        address: std::net::Ipv4Addr::new(10, 0, 0, 50),
        prefix: 24,
        gateway: None,
    });

    let out = wifi.to_owned_dict().expect("to_owned_dict ok");
    let ipv4 = out.get("ipv4").unwrap();
    let addr_data = ipv4.get("address-data").unwrap();

    let sig = format!("{}", addr_data.value_signature());
    assert_eq!(sig, "aa{sv}", "expected aa{{sv}} signature");
}

#[test]
fn parses_finite_values_as_enums() {
    let uuid = Uuid::new_v4();
    let mut dict = build_dict("eduroam", uuid, b"eduroam", "auto", &[], &[]);

    dict.get_mut("connection")
        .unwrap()
        .insert("dns-over-tls".to_string(), i32_val(1));
    dict.get_mut("connection")
        .unwrap()
        .insert("metered".to_string(), i32_val(2));
    dict.get_mut("802-11-wireless")
        .unwrap()
        .insert("band".to_string(), str_val("a"));
    dict.get_mut("802-11-wireless")
        .unwrap()
        .insert("powersave".to_string(), u32_val(3));
    dict.get_mut("802-11-wireless")
        .unwrap()
        .insert("ap-isolation".to_string(), i32_val(1));

    dict.get_mut("ipv4")
        .unwrap()
        .insert("auto-route-ext-gw".to_string(), i32_val(0));
    dict.get_mut("ipv4")
        .unwrap()
        .insert("replace-local-rule".to_string(), i32_val(1));
    dict.get_mut("ipv4")
        .unwrap()
        .insert("routed-dns".to_string(), i32_val(0));

    dict.get_mut("ipv6")
        .unwrap()
        .insert("auto-route-ext-gw".to_string(), i32_val(1));
    dict.get_mut("ipv6")
        .unwrap()
        .insert("replace-local-rule".to_string(), i32_val(0));
    dict.get_mut("ipv6")
        .unwrap()
        .insert("routed-dns".to_string(), i32_val(1));
    let mut ipv6_address = HashMap::new();
    ipv6_address.insert("address", Value::from("2001:db8::10".to_string()));
    ipv6_address.insert("prefix", Value::from(64_u32));
    dict.get_mut("ipv6").unwrap().insert(
        "address-data".to_string(),
        OwnedValue::try_from(Value::from(vec![ipv6_address])).unwrap(),
    );
    let mut ipv6_route = HashMap::new();
    ipv6_route.insert("address", Value::from("2001:db8:1::".to_string()));
    ipv6_route.insert("prefix", Value::from(64_u32));
    ipv6_route.insert("next-hop", Value::from("2001:db8::1".to_string()));
    dict.get_mut("ipv6").unwrap().insert(
        "route-data".to_string(),
        OwnedValue::try_from(Value::from(vec![ipv6_route])).unwrap(),
    );

    let mut security = HashMap::new();
    security.insert("key-mgmt".to_string(), str_val("wpa-eap"));
    security.insert("fils".to_string(), i32_val(2));
    dict.insert("802-11-wireless-security".to_string(), security);

    let wifi = WiFiSettings::from_raw_dict(dict).expect("parse ok");
    assert_eq!(wifi.connection.dns_over_tls, DnsOverTls::Opportunistic);
    assert_eq!(wifi.connection.metered, Metered::No);
    assert_eq!(wifi.wifi.band, Some(WirelessBand::Ghz5));
    assert_eq!(wifi.wifi.powersave, WirelessPowersave::Enable);
    assert_eq!(wifi.wifi.ap_isolation, BoolWithDefault::Yes);
    assert_eq!(wifi.ipv4.auto_route_ext_gw, Some(BoolWithDefault::No));
    assert_eq!(wifi.ipv4.replace_local_rule, Some(BoolWithDefault::Yes));
    assert_eq!(wifi.ipv4.routed_dns, BoolWithDefault::No);
    assert_eq!(wifi.ipv6.auto_route_ext_gw, Some(BoolWithDefault::Yes));
    assert_eq!(wifi.ipv6.replace_local_rule, Some(BoolWithDefault::No));
    assert_eq!(wifi.ipv6.routed_dns, BoolWithDefault::Yes);
    assert_eq!(wifi.ipv6.address_data.len(), 1);
    assert_eq!(wifi.ipv6.address_data[0].prefix, 64);
    assert_eq!(wifi.ipv6.route_data.len(), 1);
    assert_eq!(wifi.ipv6.route_data[0].prefix, 64);
    assert_eq!(
        wifi.wifi_security.as_ref().unwrap().fils,
        FilsMode::Optional
    );

    let out = wifi.to_owned_dict().expect("to_owned_dict ok");
    for (section, fields) in [
        (
            "ipv4",
            [
                ("auto-route-ext-gw", 0),
                ("replace-local-rule", 1),
                ("routed-dns", 0),
            ],
        ),
        (
            "ipv6",
            [
                ("auto-route-ext-gw", 1),
                ("replace-local-rule", 0),
                ("routed-dns", 1),
            ],
        ),
    ] {
        let values = out.get(section).unwrap();
        for (field, expected) in fields {
            assert_eq!(values.get(field).unwrap(), &i32_val(expected));
        }
    }
    assert_eq!(
        out.get("802-11-wireless")
            .unwrap()
            .get("ap-isolation")
            .unwrap(),
        &i32_val(1)
    );
    assert!(out.get("ipv6").unwrap().contains_key("address-data"));
    assert!(out.get("ipv6").unwrap().contains_key("route-data"));
}

#[test]
fn rejects_invalid_ip_prefixes() {
    let uuid = Uuid::new_v4();
    let mut dict = build_dict("invalid", uuid, b"invalid", "auto", &[], &[]);
    let mut address = HashMap::new();
    address.insert("address", Value::from("192.0.2.1".to_string()));
    address.insert("prefix", Value::from(33_u32));
    dict.get_mut("ipv4").unwrap().insert(
        "address-data".to_string(),
        OwnedValue::try_from(Value::from(vec![address])).unwrap(),
    );
    assert!(WiFiSettings::from_raw_dict(dict).is_err());
}

#[test]
fn ternary_values_round_trip_to_networkmanager_integers() {
    assert_eq!(i32::from(BoolWithDefault::Default), -1);
    assert_eq!(i32::from(BoolWithDefault::No), 0);
    assert_eq!(i32::from(BoolWithDefault::Yes), 1);
    assert_eq!(BoolWithDefault::from(-1), BoolWithDefault::Default);
    assert_eq!(BoolWithDefault::from(0), BoolWithDefault::No);
    assert_eq!(BoolWithDefault::from(1), BoolWithDefault::Yes);
    assert_eq!(BoolWithDefault::from(99), BoolWithDefault::Default);
}

#[test]
fn vlan_settings_parse_and_round_trip_typed_fields() {
    let uuid = Uuid::new_v4();
    let mut dict = build_dict("office-vlan", uuid, b"vlan", "auto", &[], &[]);
    let mut vlan = HashMap::new();
    vlan.insert("parent".to_string(), str_val("eth0"));
    vlan.insert("id".to_string(), u32_val(100));
    vlan.insert("flags".to_string(), u32_val(5));
    vlan.insert("protocol".to_string(), str_val("802.1Q"));
    vlan.insert(
        "ingress-priority-map".to_string(),
        string_array_val(&["0:0"]),
    );
    dict.insert("vlan".to_string(), vlan);

    let settings = VlanSettings::from_raw_dict(dict).expect("VLAN parse");
    assert_eq!(settings.vlan.parent, "eth0");
    assert_eq!(settings.vlan.id, 100);
    assert_eq!(settings.vlan.flags, Some(5));
    assert_eq!(settings.vlan.protocol, Some(VlanProtocol::Dot1Q));
    assert_eq!(settings.vlan.ingress_priority_map, Some(vec!["0:0".into()]));

    let output = settings.to_owned_dict().expect("VLAN serialize");
    assert_eq!(output["vlan"]["id"], u32_val(100));
    assert_eq!(output["vlan"]["protocol"], str_val("802.1Q"));
}

#[test]
fn vpn_settings_parse_and_round_trip_typed_fields() {
    let uuid = Uuid::new_v4();
    let mut dict = build_dict("office-vpn", uuid, b"vpn", "auto", &[], &[]);
    let mut data = HashMap::new();
    data.insert("remote".to_string(), "vpn.example.com".to_string());
    data.insert("proto-tcp".to_string(), "yes".to_string());
    let mut secrets = HashMap::new();
    secrets.insert("password".to_string(), "secret".to_string());
    let mut vpn = HashMap::new();
    vpn.insert(
        "service-type".to_string(),
        str_val("org.freedesktop.NetworkManager.openvpn"),
    );
    vpn.insert("user-name".to_string(), str_val("alice"));
    vpn.insert("persistent".to_string(), bool_val(true));
    vpn.insert("timeout".to_string(), u32_val(30));
    vpn.insert(
        "data".to_string(),
        OwnedValue::try_from(Value::from(data)).unwrap(),
    );
    vpn.insert(
        "secrets".to_string(),
        OwnedValue::try_from(Value::from(secrets)).unwrap(),
    );
    dict.insert("vpn".to_string(), vpn);

    let settings = VpnSettings::from_raw_dict(dict).expect("VPN parse");
    assert_eq!(
        settings.vpn.service_type,
        "org.freedesktop.NetworkManager.openvpn"
    );
    assert_eq!(settings.vpn.user_name.as_deref(), Some("alice"));
    assert!(settings.vpn.persistent);
    assert_eq!(settings.vpn.timeout, 30);
    assert_eq!(settings.vpn.data["remote"], "vpn.example.com");
    assert_eq!(settings.vpn.secrets["password"], "secret");
    let openvpn = settings.openvpn_options().expect("OpenVPN options");
    assert_eq!(openvpn.remote.as_deref(), Some("vpn.example.com"));
    assert_eq!(openvpn.proto_tcp, Some(true));

    let output = settings.to_owned_dict().expect("VPN serialize");
    assert_eq!(output["vpn"]["timeout"], u32_val(30));
    assert_eq!(output["vpn"]["data"].value_signature(), "a{ss}");
    assert_eq!(output["vpn"]["secrets"].value_signature(), "a{ss}");
}

#[test]
fn wireguard_settings_parse_and_round_trip_peers() {
    let uuid = Uuid::new_v4();
    let mut dict = build_dict("office-wg", uuid, b"wg", "auto", &[], &[]);
    let mut peer: HashMap<&str, Value<'_>> = HashMap::new();
    peer.insert("public-key", Value::from("peer-public-key".to_string()));
    peer.insert(
        "allowed-ips",
        Value::from(vec!["10.0.0.0/24".to_string(), "::/0".to_string()]),
    );
    peer.insert("endpoint", Value::from("vpn.example.com:51820".to_string()));
    peer.insert("persistent-keepalive", Value::from(25_u32));
    let mut wireguard = HashMap::new();
    wireguard.insert("listen-port".to_string(), u32_val(51820));
    wireguard.insert("mtu".to_string(), u32_val(1420));
    wireguard.insert("fwmark".to_string(), u32_val(42));
    wireguard.insert("peer-routes".to_string(), bool_val(true));
    wireguard.insert("ip4-auto-default-route".to_string(), i32_val(1));
    wireguard.insert("ip6-auto-default-route".to_string(), i32_val(0));
    wireguard.insert("private-key".to_string(), str_val("private-key"));
    wireguard.insert("private-key-flags".to_string(), u32_val(1));
    wireguard.insert(
        "peers".to_string(),
        OwnedValue::try_from(Value::from(vec![peer])).unwrap(),
    );
    dict.insert("wireguard".to_string(), wireguard);

    let settings = WireGuardSettings::from_raw_dict(dict).expect("WireGuard parse");
    assert_eq!(settings.wireguard.listen_port, 51820);
    assert_eq!(settings.wireguard.mtu, 1420);
    assert_eq!(settings.wireguard.fwmark, 42);
    assert_eq!(
        settings.wireguard.ip4_auto_default_route,
        BoolWithDefault::Yes
    );
    assert_eq!(
        settings.wireguard.ip6_auto_default_route,
        BoolWithDefault::No
    );
    assert_eq!(settings.wireguard.peers.len(), 1);
    assert_eq!(settings.wireguard.peers[0].allowed_ips.len(), 2);
    assert_eq!(
        settings.wireguard.peers[0].endpoint.as_deref(),
        Some("vpn.example.com:51820")
    );

    let output = settings.to_owned_dict().expect("WireGuard serialize");
    assert_eq!(output["wireguard"]["listen-port"], u32_val(51820));
    assert_eq!(output["wireguard"]["peers"].value_signature(), "aa{sv}");
}

#[test]
fn bluetooth_settings_parse_and_round_trip_typed_fields() {
    let uuid = Uuid::new_v4();
    let mut dict = build_dict("phone", uuid, b"bluetooth", "auto", &[], &[]);
    let mut bluetooth = HashMap::new();
    bluetooth.insert("bdaddr".to_string(), str_val("00:1A:7D:DA:71:13"));
    bluetooth.insert("type".to_string(), str_val("panu"));
    dict.insert("bluetooth".to_string(), bluetooth);

    let settings = BluetoothSettings::from_raw_dict(dict).expect("Bluetooth parse");
    assert_eq!(settings.bluetooth.bdaddr, "00:1A:7D:DA:71:13");
    assert_eq!(settings.bluetooth.bt_type, BluetoothType::PanU);

    let output = settings.to_owned_dict().expect("Bluetooth serialize");
    assert_eq!(output["bluetooth"]["bdaddr"], str_val("00:1A:7D:DA:71:13"));
    assert_eq!(output["bluetooth"]["type"], str_val("panu"));
}

#[test]
fn bond_settings_parse_and_round_trip_options() {
    let uuid = Uuid::new_v4();
    let mut dict = build_dict("bond0", uuid, b"bond", "auto", &[], &[]);
    let mut options = HashMap::new();
    options.insert("mode".to_string(), "802.3ad".to_string());
    options.insert("miimon".to_string(), "100".to_string());
    let mut bond = HashMap::new();
    bond.insert(
        "options".to_string(),
        OwnedValue::try_from(Value::from(options)).unwrap(),
    );
    dict.insert("bond".to_string(), bond);

    let settings = BondSettings::from_raw_dict(dict).expect("bond parse");
    assert_eq!(settings.bond.options["mode"], "802.3ad");
    let output = settings.to_owned_dict().expect("bond serialize");
    assert_eq!(output["bond"]["options"].value_signature(), "a{ss}");
}

#[test]
fn bridge_and_mobile_settings_use_typed_sections() {
    let uuid = Uuid::new_v4();
    let mut bridge = build_dict("bridge0", uuid, b"bridge", "auto", &[], &[]);
    bridge.insert("bridge".into(), HashMap::new());
    let bridge = BridgeSettings::from_raw_dict(bridge).expect("bridge parse");
    assert!(bridge.to_owned_dict().unwrap().contains_key("bridge"));

    let mut gsm = build_dict("gsm", Uuid::new_v4(), b"gsm", "auto", &[], &[]);
    gsm.insert("gsm".into(), HashMap::new());
    let gsm = GsmSettings::from_raw_dict(gsm).expect("gsm parse");
    assert_eq!(gsm.gsm.mtu, 0);

    let mut cdma = build_dict("cdma", Uuid::new_v4(), b"cdma", "auto", &[], &[]);
    cdma.insert("cdma".into(), HashMap::new());
    let cdma = CdmaSettings::from_raw_dict(cdma).expect("cdma parse");
    assert_eq!(cdma.cdma.mtu, 0);
}

// Mutation and round-trip behavior.
#[test]
fn public_mutations_survive_serialization() {
    let mut settings = WiFiSettings::from_raw_dict(build_dict(
        "wifi",
        Uuid::new_v4(),
        b"wifi",
        "auto",
        &[],
        &[],
    ))
    .unwrap();
    settings.connection.autoconnect = true;
    settings.ipv4.method = IpMethod::Manual;
    settings.ipv4.address_data.push(Ipv4AddressData {
        address: Ipv4Addr::new(198, 51, 100, 10),
        prefix: 24,
        gateway: None,
    });
    let output = settings.to_owned_dict().unwrap();
    assert_eq!(output["connection"]["autoconnect"], bool_val(true));
    assert_eq!(output["ipv4"]["method"], str_val("manual"));
    assert!(output["ipv4"].contains_key("address-data"));
}

#[test]
fn parse_serialize_parse_preserves_core_fields() {
    let original = build_dict(
        "wifi",
        Uuid::new_v4(),
        b"round-trip",
        "auto",
        &[("192.0.2.20", 24)],
        &["1.1.1.1"],
    );
    let first = WiFiSettings::from_raw_dict(original).unwrap();
    let serialized = first.to_owned_dict().unwrap();
    let second = WiFiSettings::from_raw_dict(serialized).unwrap();
    assert_eq!(second.wifi.ssid, "round-trip");
    assert_eq!(second.ipv4.address_data.len(), 1);
    assert_eq!(second.ipv4.dns_data, vec!["1.1.1.1"]);
}

// Invalid input handling.
#[test]
fn missing_required_sections_are_rejected() {
    let mut dict = build_dict("wifi", Uuid::new_v4(), b"wifi", "auto", &[], &[]);
    dict.remove("ipv6");
    assert!(WiFiSettings::from_raw_dict(dict).is_err());
}

#[test]
fn invalid_enum_values_are_rejected() {
    let mut dict = build_dict("wifi", Uuid::new_v4(), b"wifi", "auto", &[], &[]);
    dict.get_mut("802-11-wireless")
        .unwrap()
        .insert("band".into(), str_val("invalid-band"));
    assert!(WiFiSettings::from_raw_dict(dict).is_err());
}

#[test]
fn unknown_ip_methods_become_other() {
    // NetworkManager method strings this crate does not recognise are kept
    // verbatim (as `IpMethod::Other`) so profiles remain forward-compatible,
    // rather than being rejected outright.
    let dict = build_dict("wifi", Uuid::new_v4(), b"wifi", "invalid", &[], &[]);
    let wifi = WiFiSettings::from_raw_dict(dict).expect("unknown method is preserved");
    assert_eq!(wifi.ipv4.method, IpMethod::Other("invalid".to_string()));
    assert_eq!(IpMethod::from("manual".to_string()), IpMethod::Manual);
}

#[test]
fn invalid_vlan_ids_are_rejected() {
    let mut dict = build_dict("vlan", Uuid::new_v4(), b"vlan", "auto", &[], &[]);
    dict.insert(
        "vlan".into(),
        std::collections::HashMap::from([
            ("parent".into(), str_val("eth0")),
            ("id".into(), u32_val(4095)),
        ]),
    );
    assert!(VlanSettings::from_raw_dict(dict).is_err());
}

#[test]
fn generic_vpn_requires_service_type() {
    let mut dict = build_dict("vpn", Uuid::new_v4(), b"vpn", "auto", &[], &[]);
    dict.insert("vpn".into(), std::collections::HashMap::new());
    assert!(VpnSettings::from_raw_dict(dict).is_err());
}

// D-Bus value signatures.
#[test]
fn nested_networkmanager_sections_have_expected_signatures() {
    let settings = WiFiSettings::from_raw_dict(build_dict(
        "wifi",
        Uuid::new_v4(),
        b"wifi",
        "auto",
        &[("192.0.2.10", 24)],
        &[],
    ))
    .unwrap();
    let output = settings.to_owned_dict().unwrap();
    assert_eq!(output["ipv4"]["address-data"].value_signature(), "aa{sv}");
    assert_eq!(output["802-11-wireless"]["ssid"].value_signature(), "ay");
}

#[test]
fn optional_sections_are_omitted_when_absent() {
    let settings = WiFiSettings::from_raw_dict(build_dict(
        "wifi",
        Uuid::new_v4(),
        b"wifi",
        "auto",
        &[],
        &[],
    ))
    .unwrap();
    let output = settings.to_owned_dict().unwrap();
    assert!(!output.contains_key("802-11-wireless-security"));
    assert!(!output.contains_key("802-1x"));
}

#[test]
fn string_values_keep_string_signatures() {
    let value = OwnedValue::try_from(Value::from("example".to_string())).unwrap();
    assert_eq!(value.value_signature(), "s");
    assert_eq!(value, str_val("example"));
}
