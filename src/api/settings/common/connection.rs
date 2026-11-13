use std::{collections::HashMap, str::FromStr};

use uuid::Uuid;
use zbus::zvariant::{OwnedValue, Value};

use crate::api::settings::error::SettingsError as ConnectionError;

int_enum!(DnsOverTls {
    Default = -1,
    No = 0,
    Opportunistic = 1,
    Yes = 2
} default Default);

int_enum!(Llmnr {
    Default = -1,
    No = 0,
    Resolve = 1,
    Yes = 2
} default Default);

int_enum!(Mdns {
    Default = -1,
    No = 0,
    Resolve = 1,
    Yes = 2
} default Default);

int_enum!(Metered {
    Unknown = 0,
    Yes = 1,
    No = 2
} default Unknown);

int_enum!(MultiConnect {
    Default = 0,
    Single = 1,
    ManualMultiple = 2,
    Multiple = 3
} default Default);

/// Common NetworkManager connection metadata and behavior.
#[derive(Debug, Clone)]
pub struct ConnectionSection {
    /// The NetworkManager `connection.interface-name` property.
    pub interface_name: Option<String>,
    /// The NetworkManager `connection.auth-retries` property.
    pub auth_retries: i32,
    /// The NetworkManager `connection.autoconnect` property.
    pub autoconnect: bool,
    /// The NetworkManager `connection.autoconnect-priority` property.
    pub autoconnect_priority: i32,
    /// The NetworkManager `connection.autoconnect-retries` property.
    pub autoconnect_retries: i32,
    /// The NetworkManager profile type (`connection.type`).
    pub connection_type: SettingsType,
    /// The connection UUID (`connection.uuid`).
    pub id: Uuid,
    /// The human-readable profile name (`connection.id`).
    pub name: String,
    /// The NetworkManager `connection.autoconnect-slaves` property.
    pub autoconnect_slaves: Option<i32>,
    /// The NetworkManager `connection.dns-over-tls` property.
    pub dns_over_tls: DnsOverTls,
    /// The NetworkManager `connection.gateway-ping-timeout` property.
    pub gateway_ping_timeout: u32,
    /// The NetworkManager `connection.lldp` property.
    pub lldp: i32,
    /// The NetworkManager `connection.llmnr` property.
    pub llmnr: Llmnr,
    /// The NetworkManager `connection.master` property.
    pub master: Option<String>,
    /// The NetworkManager `connection.mdns` property.
    pub mdns: Mdns,
    /// The NetworkManager `connection.metered` property.
    pub metered: Metered,
    /// The NetworkManager `connection.mptcp-flags` property.
    pub mptcp_flags: u32,
    /// The NetworkManager `connection.mud-url` property.
    pub mud_url: Option<String>,
    /// The NetworkManager `connection.multi-connect` property.
    pub multi_connect: MultiConnect,
    /// The NetworkManager `connection.permissions` property.
    pub permissions: Vec<String>,
    /// The NetworkManager `connection.read-only` property.
    pub read_only: bool,
    /// The NetworkManager `connection.secondaries` property.
    pub secondaries: Vec<String>,
    /// The NetworkManager `connection.slave-type` property.
    pub slave_type: Option<String>,
    /// The NetworkManager `connection.stable-id` property.
    pub stable_id: Option<String>,
    /// The NetworkManager `connection.wait-activation-delay` property.
    pub wait_activation_delay: i32,
    /// The NetworkManager `connection.wait-device-timeout` property.
    pub wait_device_timeout: i32,
    /// The NetworkManager `connection.zone` property.
    pub zone: Option<String>,
}

impl ConnectionSection {
    pub(crate) const fn get_name() -> &'static str {
        "connection"
    }
}

impl From<ConnectionSection> for HashMap<&str, Value<'_>> {
    fn from(value: ConnectionSection) -> Self {
        let mut connection = HashMap::new();
        insert_opt!(connection, "interface-name", value.interface_name);
        insert_value!(connection, "auth-retries", value.auth_retries);
        insert_value!(connection, "autoconnect", value.autoconnect);
        insert_value!(
            connection,
            "autoconnect-priority",
            value.autoconnect_priority
        );
        insert_value!(connection, "autoconnect-retries", value.autoconnect_retries);
        insert_value!(connection, "id", value.name);
        insert_value!(connection, "type", value.connection_type.to_string());
        insert_value!(connection, "uuid", value.id.to_string());
        insert_opt!(connection, "autoconnect-slaves", value.autoconnect_slaves);
        insert_value!(connection, "dns-over-tls", i32::from(value.dns_over_tls));
        insert_value!(
            connection,
            "gateway-ping-timeout",
            value.gateway_ping_timeout
        );
        insert_value!(connection, "lldp", value.lldp);
        insert_value!(connection, "llmnr", i32::from(value.llmnr));
        insert_opt!(connection, "master", value.master);
        insert_value!(connection, "mdns", i32::from(value.mdns));
        insert_value!(connection, "metered", i32::from(value.metered));
        insert_value!(connection, "mptcp-flags", value.mptcp_flags);
        insert_opt!(connection, "mud-url", value.mud_url);
        insert_value!(connection, "multi-connect", i32::from(value.multi_connect));
        if !value.permissions.is_empty() {
            insert_value!(connection, "permissions", value.permissions);
        }
        insert_value!(connection, "read-only", value.read_only);
        if !value.secondaries.is_empty() {
            insert_value!(connection, "secondaries", value.secondaries);
        }
        insert_opt!(connection, "slave-type", value.slave_type);
        insert_opt!(connection, "stable-id", value.stable_id);
        insert_value!(
            connection,
            "wait-activation-delay",
            value.wait_activation_delay
        );
        insert_value!(connection, "wait-device-timeout", value.wait_device_timeout);
        insert_opt!(connection, "zone", value.zone);
        connection
    }
}

impl TryFrom<&HashMap<String, OwnedValue>> for ConnectionSection {
    type Error = ConnectionError;
    fn try_from(value: &HashMap<String, OwnedValue>) -> Result<Self, Self::Error> {
        Ok(ConnectionSection {
            interface_name: extract_optional_field!(value, "interface-name", String)?,
            auth_retries: extract_field!(value, "auth-retries", i32).unwrap_or(-1),
            autoconnect: extract_field!(value, "autoconnect", bool).unwrap_or(true),
            autoconnect_priority: extract_field!(value, "autoconnect-priority", i32).unwrap_or(0),
            autoconnect_retries: extract_field!(value, "autoconnect-retries", i32).unwrap_or(-1),
            connection_type: SettingsType::from_str(&extract_field!(value, "type", String)?)?,
            id: Uuid::try_parse(&extract_field!(value, "uuid", String)?)?,
            name: extract_field!(value, "id", String)?,
            autoconnect_slaves: extract_optional_field!(value, "autoconnect-slaves", i32)?,
            dns_over_tls: DnsOverTls::from(
                extract_field!(value, "dns-over-tls", i32).unwrap_or(-1),
            ),
            gateway_ping_timeout: extract_field!(value, "gateway-ping-timeout", u32).unwrap_or(0),
            lldp: extract_field!(value, "lldp", i32).unwrap_or(-1),
            llmnr: Llmnr::from(extract_field!(value, "llmnr", i32).unwrap_or(-1)),
            master: extract_optional_field!(value, "master", String)?,
            mdns: Mdns::from(extract_field!(value, "mdns", i32).unwrap_or(-1)),
            metered: Metered::from(extract_field!(value, "metered", i32).unwrap_or(0)),
            mptcp_flags: extract_field!(value, "mptcp-flags", u32).unwrap_or(0),
            mud_url: extract_optional_field!(value, "mud-url", String)?,
            multi_connect: MultiConnect::from(
                extract_field!(value, "multi-connect", i32).unwrap_or(0),
            ),
            permissions: extract_vec_field_or_default!(value, "permissions", String)?,
            read_only: extract_field!(value, "read-only", bool).unwrap_or(false),
            secondaries: extract_vec_field_or_default!(value, "secondaries", String)?,
            slave_type: extract_optional_field!(value, "slave-type", String)?,
            stable_id: extract_optional_field!(value, "stable-id", String)?,
            wait_activation_delay: extract_field!(value, "wait-activation-delay", i32)
                .unwrap_or(-1),
            wait_device_timeout: extract_field!(value, "wait-device-timeout", i32).unwrap_or(-1),
            zone: extract_optional_field!(value, "zone", String)?,
        })
    }
}

string_enum!(SettingsType {
    Lowpan => "6lowpan",
    OlpcMesh => "802-11-olpc-mesh",
    WiFi => "802-11-wireless",
    Ethernet => "802-3-ethernet",
    Adsl => "adsl",
    Bluetooth => "bluetooth",
    Bond => "bond",
    BondSlave => "bond-slave",
    Bridge => "bridge",
    BridgeSlave => "bridge-slave",
    Cdma => "cdma",
    Dummy => "dummy",
    Generic => "generic",
    GSM => "gsm",
    Infiniband => "infiniband",
    IpTunnel => "ip-tunnel",
    MacSec => "macsec",
    MacVLan => "macvlan",
    OvsBridge => "ovs-bridge",
    OvsDpdk => "ovs-dpdk",
    OvsInterface => "ovs-interface",
    OvsPatch => "ovs-patch",
    OvsPort => "ovs-port",
    PPPoe => "pppoe",
    Team => "team",
    TeamSlave => "team-slave",
    Tun => "tun",
    Veth => "veth",
    Vlan => "vlan",
    VPN => "vpn",
    Vrf => "vrf",
    VxLan => "vxlan",
    WiFiP2P => "wifi-p2p",
    WiMax => "wimax",
    Wireguard => "wireguard",
    Wpan => "wpan",
} error "ConnectionType");
