use crate::api::settings::error::SettingsError as ConnectionError;
use std::{collections::HashMap, str::FromStr};
use zbus::zvariant::{Array, Dict, OwnedValue, Value};

/// VLAN membership attached to a bridge or bridge port.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BridgeVlan {
    /// The VLAN identifier.
    pub id: u32,
    /// Whether this is the port's PVID.
    pub pvid: bool,
    /// Whether frames for this VLAN leave the port untagged.
    pub untagged: bool,
}

impl BridgeVlan {
    fn from_dict(dict: &Dict<'_, '_>) -> Result<Self, ConnectionError> {
        Ok(Self {
            id: dict
                .get::<_, u32>(&"id")?
                .ok_or(ConnectionError::MissingField("bridge vlan id"))?,
            pvid: dict.get::<_, bool>(&"pvid")?.unwrap_or(false),
            untagged: dict.get::<_, bool>(&"untagged")?.unwrap_or(false),
        })
    }
}

impl From<BridgeVlan> for HashMap<&'static str, Value<'static>> {
    fn from(value: BridgeVlan) -> Self {
        HashMap::from([
            ("id", Value::from(value.id)),
            ("pvid", Value::from(value.pvid)),
            ("untagged", Value::from(value.untagged)),
        ])
    }
}

string_enum!(BridgeMulticastRouter {
    Auto => "auto",
    Disabled => "disabled",
    Enabled => "enabled",
});

string_enum!(BridgeVlanProtocol {
    Dot1Q => "802.1Q",
    Dot1Ad => "802.1ad",
});

/// Complete typed NetworkManager `bridge` settings.
#[derive(Debug, Clone)]
pub struct BridgeSection {
    /// The NetworkManager `bridge.ageing-time` property.
    pub ageing_time: u32,
    /// The NetworkManager `bridge.forward-delay` property.
    pub forward_delay: u32,
    /// The NetworkManager `bridge.group-address` property.
    pub group_address: Option<Vec<u8>>,
    /// The NetworkManager `bridge.group-forward-mask` property.
    pub group_forward_mask: u16,
    /// The NetworkManager `bridge.hello-time` property.
    pub hello_time: u32,
    /// The NetworkManager `bridge.mac-address` property.
    pub mac_address: Option<Vec<u8>>,
    /// The NetworkManager `bridge.max-age` property.
    pub max_age: u32,
    /// The NetworkManager `bridge.multicast-hash-max` property.
    pub multicast_hash_max: u32,
    /// The NetworkManager `bridge.multicast-last-member-count` property.
    pub multicast_last_member_count: u32,
    /// The NetworkManager `bridge.multicast-last-member-interval` property.
    pub multicast_last_member_interval: u64,
    /// The NetworkManager `bridge.multicast-membership-interval` property.
    pub multicast_membership_interval: u64,
    /// The NetworkManager `bridge.multicast-querier` property.
    pub multicast_querier: bool,
    /// The NetworkManager `bridge.multicast-querier-interval` property.
    pub multicast_querier_interval: u64,
    /// The NetworkManager `bridge.multicast-query-interval` property.
    pub multicast_query_interval: u64,
    /// The NetworkManager `bridge.multicast-query-response-interval` property.
    pub multicast_query_response_interval: u64,
    /// The NetworkManager `bridge.multicast-query-use-ifaddr` property.
    pub multicast_query_use_ifaddr: bool,
    /// The NetworkManager `bridge.multicast-router` property.
    pub multicast_router: BridgeMulticastRouter,
    /// The NetworkManager `bridge.multicast-snooping` property.
    pub multicast_snooping: bool,
    /// The NetworkManager `bridge.multicast-startup-query-count` property.
    pub multicast_startup_query_count: u32,
    /// The NetworkManager `bridge.multicast-startup-query-interval` property.
    pub multicast_startup_query_interval: u64,
    /// The NetworkManager `bridge.priority` property.
    pub priority: u16,
    /// The NetworkManager `bridge.stp` property.
    pub stp: bool,
    /// The NetworkManager `bridge.vlan-default-pvid` property.
    pub vlan_default_pvid: u16,
    /// The NetworkManager `bridge.vlan-filtering` property.
    pub vlan_filtering: bool,
    /// The NetworkManager `bridge.vlan-protocol` property.
    pub vlan_protocol: Option<BridgeVlanProtocol>,
    /// The NetworkManager `bridge.vlan-stats-enabled` property.
    pub vlan_stats_enabled: bool,
    /// The NetworkManager `bridge.vlans` property.
    pub vlans: Vec<BridgeVlan>,
}

fn parse_bytes(
    value: &HashMap<String, OwnedValue>,
    key: &'static str,
) -> Result<Option<Vec<u8>>, ConnectionError> {
    let Some(raw) = value.get(key) else {
        return Ok(None);
    };
    let array = raw
        .downcast_ref::<Array<'_>>()
        .map_err(|_| ConnectionError::FailedDowncast((key, "Array")))?;
    array
        .iter()
        .map(|item| {
            item.downcast_ref::<u8>()
                .map_err(|_| ConnectionError::FailedDowncast((key, "u8")))
        })
        .collect::<Result<Vec<_>, _>>()
        .map(Some)
}

impl TryFrom<&HashMap<String, OwnedValue>> for BridgeSection {
    type Error = ConnectionError;
    fn try_from(value: &HashMap<String, OwnedValue>) -> Result<Self, Self::Error> {
        let vlans = match value.get("vlans") {
            Some(value) => {
                let array = value
                    .downcast_ref::<Array<'_>>()
                    .map_err(|_| ConnectionError::FailedDowncast(("vlans", "Array")))?;
                array
                    .iter()
                    .map(|value| {
                        BridgeVlan::from_dict(
                            &value
                                .downcast_ref::<Dict<'_, '_>>()
                                .map_err(|_| ConnectionError::FailedDowncast(("vlans", "Dict")))?,
                        )
                    })
                    .collect::<Result<_, _>>()?
            }
            None => Vec::new(),
        };
        Ok(Self {
            ageing_time: extract_field!(value, "ageing-time", u32).unwrap_or(0),
            forward_delay: extract_field!(value, "forward-delay", u32).unwrap_or(0),
            group_address: parse_bytes(value, "group-address")?,
            group_forward_mask: extract_field!(value, "group-forward-mask", u32).unwrap_or(0)
                as u16,
            hello_time: extract_field!(value, "hello-time", u32).unwrap_or(0),
            mac_address: parse_bytes(value, "mac-address")?,
            max_age: extract_field!(value, "max-age", u32).unwrap_or(0),
            multicast_hash_max: extract_field!(value, "multicast-hash-max", u32).unwrap_or(0),
            multicast_last_member_count: extract_field!(value, "multicast-last-member-count", u32)
                .unwrap_or(0),
            multicast_last_member_interval: extract_field!(
                value,
                "multicast-last-member-interval",
                u64
            )
            .unwrap_or(0),
            multicast_membership_interval: extract_field!(
                value,
                "multicast-membership-interval",
                u64
            )
            .unwrap_or(0),
            multicast_querier: extract_field!(value, "multicast-querier", bool).unwrap_or(false),
            multicast_querier_interval: extract_field!(value, "multicast-querier-interval", u64)
                .unwrap_or(0),
            multicast_query_interval: extract_field!(value, "multicast-query-interval", u64)
                .unwrap_or(0),
            multicast_query_response_interval: extract_field!(
                value,
                "multicast-query-response-interval",
                u64
            )
            .unwrap_or(0),
            multicast_query_use_ifaddr: extract_field!(value, "multicast-query-use-ifaddr", bool)
                .unwrap_or(false),
            multicast_router: extract_optional_field!(value, "multicast-router", String)?
                .map(|v| BridgeMulticastRouter::from_str(&v))
                .transpose()?
                .unwrap_or(BridgeMulticastRouter::Auto),
            multicast_snooping: extract_field!(value, "multicast-snooping", bool).unwrap_or(true),
            multicast_startup_query_count: extract_field!(
                value,
                "multicast-startup-query-count",
                u32
            )
            .unwrap_or(0),
            multicast_startup_query_interval: extract_field!(
                value,
                "multicast-startup-query-interval",
                u64
            )
            .unwrap_or(0),
            priority: extract_field!(value, "priority", u32).unwrap_or(0) as u16,
            stp: extract_field!(value, "stp", bool).unwrap_or(false),
            vlan_default_pvid: extract_field!(value, "vlan-default-pvid", u32).unwrap_or(0) as u16,
            vlan_filtering: extract_field!(value, "vlan-filtering", bool).unwrap_or(false),
            vlan_protocol: extract_optional_field!(value, "vlan-protocol", String)?
                .map(|v| BridgeVlanProtocol::from_str(&v))
                .transpose()?,
            vlan_stats_enabled: extract_field!(value, "vlan-stats-enabled", bool).unwrap_or(false),
            vlans,
        })
    }
}

impl From<BridgeSection> for HashMap<&'static str, Value<'static>> {
    fn from(v: BridgeSection) -> Self {
        let mut m = HashMap::new();
        insert_value!(m, "ageing-time", v.ageing_time);
        insert_value!(m, "forward-delay", v.forward_delay);
        insert_value!(m, "group-forward-mask", u32::from(v.group_forward_mask));
        insert_value!(m, "hello-time", v.hello_time);
        insert_value!(m, "max-age", v.max_age);
        insert_value!(m, "multicast-hash-max", v.multicast_hash_max);
        insert_value!(
            m,
            "multicast-last-member-count",
            v.multicast_last_member_count
        );
        insert_value!(
            m,
            "multicast-last-member-interval",
            v.multicast_last_member_interval
        );
        insert_value!(
            m,
            "multicast-membership-interval",
            v.multicast_membership_interval
        );
        insert_value!(m, "multicast-querier", v.multicast_querier);
        insert_value!(
            m,
            "multicast-querier-interval",
            v.multicast_querier_interval
        );
        insert_value!(m, "multicast-query-interval", v.multicast_query_interval);
        insert_value!(
            m,
            "multicast-query-response-interval",
            v.multicast_query_response_interval
        );
        insert_value!(
            m,
            "multicast-query-use-ifaddr",
            v.multicast_query_use_ifaddr
        );
        insert_value!(m, "multicast-router", v.multicast_router.to_string());
        insert_value!(m, "multicast-snooping", v.multicast_snooping);
        insert_value!(
            m,
            "multicast-startup-query-count",
            v.multicast_startup_query_count
        );
        insert_value!(
            m,
            "multicast-startup-query-interval",
            v.multicast_startup_query_interval
        );
        insert_value!(m, "priority", u32::from(v.priority));
        insert_value!(m, "stp", v.stp);
        insert_value!(m, "vlan-default-pvid", u32::from(v.vlan_default_pvid));
        insert_value!(m, "vlan-filtering", v.vlan_filtering);
        insert_value!(m, "vlan-stats-enabled", v.vlan_stats_enabled);
        if let Some(x) = v.group_address {
            insert_value!(m, "group-address", x);
        }
        if let Some(x) = v.mac_address {
            insert_value!(m, "mac-address", x);
        }
        if let Some(x) = v.vlan_protocol {
            insert_value!(m, "vlan-protocol", x.to_string());
        }
        let vlans: Vec<HashMap<&str, Value<'_>>> = v.vlans.into_iter().map(Into::into).collect();
        insert_value!(m, "vlans", vlans);
        m
    }
}

define_typed_profile!(BridgeSettings, BridgeSection, bridge, "bridge");
