use std::{collections::HashMap, fmt::Display, net::Ipv4Addr, str::FromStr};

use zbus::zvariant::{Array, Dict, OwnedValue, Value};

use crate::api::settings::error::SettingsError as ConnectionError;
use crate::{IpAddress, IpRoute, IpSettings};

use super::{BoolWithDefault, IpMethod, parse_dns_v4};

/// Editable NetworkManager IPv4 settings.
#[derive(Debug, Clone)]
pub struct IpV4Section {
    /// The NetworkManager `ipv4.address-data` property.
    pub address_data: Vec<Ipv4AddressData>,
    /// The NetworkManager `ipv4.auto-route-ext-gw` property.
    pub auto_route_ext_gw: Option<BoolWithDefault>,
    /// The NetworkManager `ipv4.dad-timeout` property.
    pub dad_timeout: i32,
    /// The NetworkManager `ipv4.dhcp-client-id` property.
    pub dhcp_client_id: Option<String>,
    /// The NetworkManager `ipv4.dhcp-dscp` property.
    pub dhcp_dscp: Option<DhcpDscp>,
    /// The NetworkManager `ipv4.dhcp-fqdn` property.
    pub dhcp_fqdn: Option<String>,
    /// The NetworkManager `ipv4.dhcp-hostname` property.
    pub dhcp_hostname: Option<String>,
    /// The NetworkManager `ipv4.dhcp-hostname-flags` property.
    pub dhcp_hostname_flags: u32,
    /// The NetworkManager `ipv4.dhcp-iaid` property.
    pub dhcp_iaid: Option<String>,
    /// The NetworkManager `ipv4.dhcp-ipv6-only-preferred` property.
    pub dhcp_ipv6_only_preferred: BoolWithDefault,
    /// The NetworkManager `ipv4.dhcp-reject-servers` property.
    pub dhcp_reject_servers: Vec<String>,
    /// The NetworkManager `ipv4.dhcp-send-hostname-v2` property.
    pub dhcp_send_hostname_v2: BoolWithDefault,
    /// The NetworkManager `ipv4.dhcp-send-release` property.
    pub dhcp_send_release: BoolWithDefault,
    /// The NetworkManager `ipv4.dhcp-timeout` property.
    pub dhcp_timeout: i32,
    /// The NetworkManager `ipv4.dhcp-vendor-class-identifier` property.
    pub dhcp_vendor_class_identifier: Option<String>,
    /// The NetworkManager `ipv4.dns-data` property.
    pub dns_data: Vec<String>,
    // TODO: Do a proper enum
    /// The NetworkManager `ipv4.dns-options` property.
    pub dns_options: Vec<String>,
    /// The NetworkManager `ipv4.dns-priority` property.
    pub dns_priority: i32,
    /// The NetworkManager `ipv4.dns-search` property.
    pub dns_search: Vec<String>,
    /// The NetworkManager `ipv4.forwarding` property.
    pub forwarding: Forwarding,
    /// The NetworkManager `ipv4.ignore-auto-dns` property.
    pub ignore_auto_dns: bool,
    /// The NetworkManager `ipv4.ignore-auto-routes` property.
    pub ignore_auto_routes: bool,
    /// The NetworkManager `ipv4.link-local` property.
    pub link_local: LinkLocal,
    /// The NetworkManager `ipv4.may-fail` property.
    pub may_fail: bool,
    /// The NetworkManager `ipv4.method` property.
    pub method: IpMethod,
    /// The NetworkManager `ipv4.never-default` property.
    pub never_default: bool,
    /// The NetworkManager `ipv4.replace-local-rule` property.
    pub replace_local_rule: Option<BoolWithDefault>,
    /// The NetworkManager `ipv4.required-timeout` property.
    pub required_timeout: i32,
    /// The NetworkManager `ipv4.route-data` property.
    pub route_data: Vec<Ipv4RouteData>,
    /// The NetworkManager `ipv4.route-metric` property.
    pub route_metric: i64,
    /// The NetworkManager `ipv4.route-table` property.
    pub route_table: u32,
    /// The NetworkManager `ipv4.routed-dns` property.
    pub routed_dns: BoolWithDefault,
    // pub routing_rules:
    /// The NetworkManager `ipv4.shared-dhcp-lease-time` property.
    pub shared_dhcp_lease_time: i32,
    /// The NetworkManager `ipv4.shared-dhcp-range` property.
    pub shared_dhcp_range: Option<SharedDhcpRangeV4>,
    /// The NetworkManager `ipv4.gateway` property.
    pub gateway: Option<String>,
    /// The NetworkManager `ipv4.routing-rules` property.
    pub routing_rules: Vec<Ipv4RoutingRule>,
}

impl IpV4Section {
    pub(crate) const fn get_name() -> &'static str {
        "ipv4"
    }

    pub(crate) fn validate(&self) -> Result<(), ConnectionError> {
        for address in &self.address_data {
            if address.prefix > 32 {
                return Err(ConnectionError::FromStrError(
                    address.prefix.to_string(),
                    "IPv4 prefix (0..=32)",
                ));
            }
        }
        for route in &self.route_data {
            if route.prefix > 32 {
                return Err(ConnectionError::FromStrError(
                    route.prefix.to_string(),
                    "IPv4 route prefix (0..=32)",
                ));
            }
        }
        Ok(())
    }
}

impl From<IpV4Section> for HashMap<&str, Value<'_>> {
    fn from(value: IpV4Section) -> Self {
        let mut ipv4 = HashMap::new();

        if !value.address_data.is_empty() {
            let addresses: Vec<HashMap<&str, Value<'_>>> =
                value.address_data.iter().map(|a| a.to_dict()).collect();
            insert_value!(ipv4, "address-data", addresses);
        }

        insert_opt!(
            ipv4,
            "auto-route-ext-gw",
            value.auto_route_ext_gw.map(i32::from)
        );

        insert_value!(ipv4, "dad-timeout", value.dad_timeout);
        insert_value!(ipv4, "dns-priority", value.dns_priority);

        insert_opt!(ipv4, "dhcp-client-id", value.dhcp_client_id);
        insert_opt!(ipv4, "dhcp-dscp", value.dhcp_dscp.map(|v| v.to_string()));
        insert_opt!(ipv4, "dhcp-fqdn", value.dhcp_fqdn);
        insert_opt!(ipv4, "dhcp-hostname", value.dhcp_hostname);
        insert_value!(ipv4, "dhcp-hostname-flags", value.dhcp_hostname_flags);
        insert_opt!(ipv4, "dhcp-iaid", value.dhcp_iaid);

        insert_value!(
            ipv4,
            "dhcp-ipv6-only-preferred",
            i32::from(value.dhcp_ipv6_only_preferred)
        );

        if !value.dhcp_reject_servers.is_empty() {
            insert_value!(ipv4, "dhcp-reject-servers", value.dhcp_reject_servers);
        }

        insert_value!(
            ipv4,
            "dhcp-send-hostname-v2",
            i32::from(value.dhcp_send_hostname_v2)
        );
        insert_value!(
            ipv4,
            "dhcp-send-release",
            i32::from(value.dhcp_send_release)
        );

        insert_value!(ipv4, "dhcp-timeout", value.dhcp_timeout);

        insert_opt!(
            ipv4,
            "dhcp-vendor-class-identifier",
            value.dhcp_vendor_class_identifier
        );

        if !value.dns_data.is_empty() {
            insert_value!(ipv4, "dns-data", value.dns_data);
        }

        if !value.dns_options.is_empty() {
            insert_value!(ipv4, "dns-options", value.dns_options);
        }

        if !value.dns_search.is_empty() {
            insert_value!(ipv4, "dns-search", value.dns_search);
        }

        insert_value!(ipv4, "forwarding", i32::from(value.forwarding));
        insert_value!(ipv4, "ignore-auto-dns", value.ignore_auto_dns);
        insert_value!(ipv4, "ignore-auto-routes", value.ignore_auto_routes);
        insert_value!(ipv4, "link-local", i32::from(value.link_local));
        insert_value!(ipv4, "may-fail", value.may_fail);
        insert_value!(ipv4, "method", value.method.to_string());
        insert_value!(ipv4, "never-default", value.never_default);
        insert_opt!(
            ipv4,
            "replace-local-rule",
            value.replace_local_rule.map(i32::from)
        );

        insert_value!(ipv4, "required-timeout", value.required_timeout);

        if !value.route_data.is_empty() {
            let routes: Vec<HashMap<&str, Value<'_>>> =
                value.route_data.iter().map(|r| r.to_dict()).collect();
            insert_value!(ipv4, "route-data", routes);
        }

        insert_value!(ipv4, "route-metric", value.route_metric);
        insert_value!(ipv4, "route-table", value.route_table);
        insert_value!(ipv4, "routed-dns", i32::from(value.routed_dns));
        insert_value!(ipv4, "shared-dhcp-lease-time", value.shared_dhcp_lease_time);

        insert_opt!(
            ipv4,
            "shared-dhcp-range",
            value.shared_dhcp_range.map(|range| range.to_string())
        );
        insert_opt!(ipv4, "gateway", value.gateway);

        if !value.routing_rules.is_empty() {
            let rules: Vec<HashMap<&str, Value<'_>>> = value
                .routing_rules
                .iter()
                .map(|r| r.clone().into())
                .collect();
            insert_value!(ipv4, "routing-rules", rules);
        }

        ipv4
    }
}

impl TryFrom<&HashMap<String, OwnedValue>> for IpV4Section {
    type Error = ConnectionError;

    fn try_from(value: &HashMap<String, OwnedValue>) -> Result<Self, Self::Error> {
        let address_data = match value.get("address-data") {
            Some(v) => {
                let arr = v
                    .downcast_ref::<Array<'_>>()
                    .map_err(|_| ConnectionError::FailedDowncast(("address-data", "Array")))?;
                let mut out = Vec::new();
                for entry in arr.iter() {
                    let dict = entry
                        .downcast_ref::<Dict<'_, '_>>()
                        .map_err(|_| ConnectionError::FailedDowncast(("address-data", "Dict")))?;
                    out.push(Ipv4AddressData::from_dict(dict)?);
                }
                out
            }
            None => Vec::new(),
        };

        let route_data = match value.get("route-data") {
            Some(v) => {
                let arr = v
                    .downcast_ref::<Array<'_>>()
                    .map_err(|_| ConnectionError::FailedDowncast(("route-data", "Array")))?;
                let mut out = Vec::new();
                for entry in arr.iter() {
                    let dict = entry
                        .downcast_ref::<Dict<'_, '_>>()
                        .map_err(|_| ConnectionError::FailedDowncast(("route-data", "Dict")))?;
                    out.push(Ipv4RouteData::from_dict(dict)?);
                }
                out
            }
            None => Vec::new(),
        };

        let shared_dhcp_range_str = extract_optional_field!(value, "shared-dhcp-range", String)?;

        let shared_dhcp_range = match shared_dhcp_range_str {
            Some(s) => Some(SharedDhcpRangeV4::from_str(&s)?),
            None => None,
        };

        let dhcp_dscp_str = extract_optional_field!(value, "dhcp-dscp", String)?;
        let mut dhcp_dscp = None;

        if let Some(s) = dhcp_dscp_str {
            dhcp_dscp = Some(DhcpDscp::from_str(&s)?);
        }

        let forwarding_int = extract_field!(value, "forwarding", i32).unwrap_or(-1);
        let forwarding = Forwarding::from(forwarding_int);

        Ok(IpV4Section {
            address_data,
            auto_route_ext_gw: extract_optional_field!(value, "auto-route-ext-gw", i32)?
                .map(BoolWithDefault::from),
            dad_timeout: extract_field!(value, "dad-timeout", i32).unwrap_or(-1),
            dhcp_client_id: extract_optional_field!(value, "dhcp-client-id", String)?,
            dhcp_dscp,
            dhcp_fqdn: extract_optional_field!(value, "dhcp-fqdn", String)?,
            dhcp_hostname: extract_optional_field!(value, "dhcp-hostname", String)?,
            dhcp_hostname_flags: extract_field!(value, "dhcp-hostname-flags", u32).unwrap_or(0),
            dhcp_iaid: extract_optional_field!(value, "dhcp-iaid", String)?,
            dhcp_ipv6_only_preferred: BoolWithDefault::from(
                extract_field!(value, "dhcp-ipv6-only-preferred", i32).unwrap_or(-1),
            ),
            dhcp_reject_servers: extract_vec_field_or_default!(
                value,
                "dhcp-reject-servers",
                String
            )?,
            dhcp_send_hostname_v2: BoolWithDefault::from(
                extract_field!(value, "dhcp-send-hostname-v2", i32).unwrap_or(-1),
            ),
            dhcp_send_release: BoolWithDefault::from(
                extract_field!(value, "dhcp-send-release", i32).unwrap_or(-1),
            ),
            dhcp_timeout: extract_field!(value, "dhcp-timeout", i32).unwrap_or(0),
            dhcp_vendor_class_identifier: extract_optional_field!(
                value,
                "dhcp-vendor-class-identifier",
                String
            )?,
            dns_data: parse_dns_v4(value)?,
            dns_options: extract_vec_field_or_default!(value, "dns-options", String)?,
            dns_priority: extract_field!(value, "dns-priority", i32).unwrap_or(0),
            dns_search: extract_vec_field_or_default!(value, "dns-search", String)?,
            forwarding,
            ignore_auto_dns: extract_field!(value, "ignore-auto-dns", bool).unwrap_or(false),
            ignore_auto_routes: extract_field!(value, "ignore-auto-routes", bool).unwrap_or(false),
            link_local: LinkLocal::from(extract_field!(value, "link-local", i32).unwrap_or(0)),
            may_fail: extract_field!(value, "may-fail", bool).unwrap_or(true),
            method: extract_optional_field!(value, "method", String)?
                .map(IpMethod::from)
                .unwrap_or(IpMethod::Auto),
            never_default: extract_field!(value, "never-default", bool).unwrap_or(false),
            replace_local_rule: extract_optional_field!(value, "replace-local-rule", i32)?
                .map(BoolWithDefault::from),
            required_timeout: extract_field!(value, "required-timeout", i32).unwrap_or(-1),
            route_data,
            route_metric: extract_field!(value, "route-metric", i64).unwrap_or(-1),
            route_table: extract_field!(value, "route-table", u32).unwrap_or(0),
            routed_dns: BoolWithDefault::from(
                extract_field!(value, "routed-dns", i32).unwrap_or(-1),
            ),
            shared_dhcp_lease_time: extract_field!(value, "shared-dhcp-lease-time", i32)
                .unwrap_or(0),
            shared_dhcp_range,
            gateway: extract_optional_field!(value, "gateway", String)?,
            routing_rules: match value.get("routing-rules") {
                Some(v) => {
                    let arr = v
                        .downcast_ref::<Array<'_>>()
                        .map_err(|_| ConnectionError::FailedDowncast(("routing-rules", "Array")))?;
                    let mut out = Vec::new();
                    for entry in arr.iter() {
                        let dict = entry.downcast_ref::<Dict<'_, '_>>().map_err(|_| {
                            ConnectionError::FailedDowncast(("routing-rules", "Dict"))
                        })?;
                        out.push(Ipv4RoutingRule::from_dict(dict)?);
                    }
                    out
                }
                None => Vec::new(),
            },
        })
    }
}

/// An IPv4 route in NetworkManager's `ipv4.route-data` array.
#[derive(Debug, Clone)]
pub struct Ipv4RouteData {
    /// The `address` value in a `ipv4.route-data` entry.
    pub address: Ipv4Addr,
    /// The `prefix` value in a `ipv4.route-data` entry.
    pub prefix: u32,
    /// The `next-hop` value in a `ipv4.route-data` entry.
    pub next_hop: Option<Ipv4Addr>,
}

impl Ipv4RouteData {
    pub(crate) fn from_dict(dict: Dict) -> Result<Ipv4RouteData, ConnectionError> {
        let address_s: String = dict
            .get(&"address")?
            .ok_or(ConnectionError::MissingField("ipv4 route address"))?;
        let address = Ipv4Addr::from_str(&address_s).map_err(|e| {
            ConnectionError::FromStrError(
                format!("ipv4 route address {address_s}: {e}"),
                "Ipv4Addr",
            )
        })?;

        let prefix: u32 = dict
            .get(&"prefix")?
            .ok_or(ConnectionError::MissingField("ipv4 route prefix"))?;
        if prefix > 32 {
            return Err(ConnectionError::FromStrError(
                prefix.to_string(),
                "IPv4 route prefix (0..=32)",
            ));
        }

        let mut next_hop = None;
        if let Some(next_hop_s) = dict.get::<_, String>(&"next-hop")? {
            next_hop = Some(Ipv4Addr::from_str(&next_hop_s).map_err(|e| {
                ConnectionError::FromStrError(
                    format!("ipv4 route next-hop {next_hop_s}: {e}"),
                    "Ipv4Addr",
                )
            })?);
        }

        Ok(Ipv4RouteData {
            address,
            prefix,
            next_hop,
        })
    }

    pub(crate) fn to_dict(&self) -> HashMap<&'static str, Value<'static>> {
        let mut dict = HashMap::new();
        insert_value!(dict, "address", self.address.to_string());
        insert_value!(dict, "prefix", self.prefix);
        insert_opt!(
            dict,
            "next-hop",
            self.next_hop.map(|next_hop| next_hop.to_string())
        );
        dict
    }
}

/// An IPv4 address in NetworkManager's `ipv4.address-data` array.
#[derive(Debug, Clone)]
pub struct Ipv4AddressData {
    /// The `address` value in a `ipv4.address-data` entry.
    pub address: Ipv4Addr,
    /// The address prefix length in bits.
    pub prefix: u32,
    /// The `gateway` value in a `ipv4.address-data` entry.
    pub gateway: Option<Ipv4Addr>,
}

impl Ipv4AddressData {
    pub(crate) fn from_dict(dict: Dict) -> Result<Ipv4AddressData, ConnectionError> {
        let address_s: String = dict
            .get(&"address")?
            .ok_or(ConnectionError::MissingField("ipv4 address"))?;
        let address = Ipv4Addr::from_str(&address_s).map_err(|e| {
            ConnectionError::FromStrError(format!("ipv4 address {address_s}: {e}"), "Ipv4Addr")
        })?;

        let prefix: u32 = dict
            .get(&"prefix")?
            .ok_or(ConnectionError::MissingField("ipv4 address prefix"))?;
        if prefix > 32 {
            return Err(ConnectionError::FromStrError(
                prefix.to_string(),
                "IPv4 prefix (0..=32)",
            ));
        }

        let mut gateway = None;
        if let Some(gateway_s) = dict.get::<_, String>(&"gateway")? {
            gateway = Some(Ipv4Addr::from_str(&gateway_s).map_err(|e| {
                ConnectionError::FromStrError(
                    format!("ipv4 address gateway {gateway_s}: {e}"),
                    "Ipv4Addr",
                )
            })?);
        }

        Ok(Ipv4AddressData {
            address,
            prefix,
            gateway,
        })
    }

    pub(crate) fn to_dict(&self) -> HashMap<&'static str, Value<'static>> {
        let mut dict = HashMap::new();
        insert_value!(dict, "address", self.address.to_string());
        insert_value!(dict, "prefix", self.prefix);
        insert_opt!(
            dict,
            "gateway",
            self.gateway.map(|gateway| gateway.to_string())
        );
        dict
    }
}

string_enum!(#[derive(Hash)] DhcpDscp {
    CS0 => "CS0",
    CS4 => "CS4",
    CS6 => "CS6",
} default CS0);

int_enum!(#[derive(Hash)] Forwarding {
    Default = -1,
    No = 0,
    Yes = 1,
    Auto = 2,
} default Default);

int_enum!(LinkLocal : i32 {
    Default = 0,
    Enabled = 1,
    Disabled = 2,
    Fallback = 3,
} default Default);

/// Start and end addresses for a shared IPv4 DHCP range.
#[derive(Debug, Clone)]
pub struct SharedDhcpRangeV4 {
    /// The start IPv4 address in the shared DHCP range.
    pub start_add: Ipv4Addr,
    /// The end IPv4 address in the shared DHCP range.
    pub end_add: Ipv4Addr,
}

impl Display for SharedDhcpRangeV4 {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "{:?},{:?}",
            self.start_add.to_string(),
            self.end_add.to_string()
        )
    }
}

impl FromStr for SharedDhcpRangeV4 {
    type Err = ConnectionError;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let (start, end) = s.split_once(',').ok_or(ConnectionError::FromStrError(
            s.to_string(),
            "SharedDhcpRange",
        ))?;

        let start_add = Ipv4Addr::from_str(start.trim())
            .map_err(|_| ConnectionError::FromStrError(s.to_string(), "SharedDhcpRange"))?;

        let end_add = Ipv4Addr::from_str(end.trim())
            .map_err(|_| ConnectionError::FromStrError(s.to_string(), "SharedDhcpRange"))?;

        Ok(SharedDhcpRangeV4 { start_add, end_add })
    }
}

/// An IPv4 policy-routing rule.
#[derive(Debug, Default, Clone)]
pub struct Ipv4RoutingRule {
    /// The `action` value in a `ipv4.routing-rules` entry.
    pub action: Option<u8>,
    /// The `dport-end` value in a `ipv4.routing-rules` entry.
    pub dport_end: Option<u16>,
    /// The `dport-start` value in a `ipv4.routing-rules` entry.
    pub dport_start: Option<u16>,
    /// The `family` value in a `ipv4.routing-rules` entry.
    pub family: Option<i32>,
    /// The `from` value in a `ipv4.routing-rules` entry.
    pub from: Option<String>,
    /// The `from-len` value in a `ipv4.routing-rules` entry.
    pub from_len: Option<u8>,
    /// The `fwmark` value in a `ipv4.routing-rules` entry.
    pub fwmark: Option<u32>,
    /// The `fwmask` value in a `ipv4.routing-rules` entry.
    pub fwmask: Option<u32>,
    /// The `iifname` value in a `ipv4.routing-rules` entry.
    pub iifname: Option<String>,
    /// The `invert` value in a `ipv4.routing-rules` entry.
    pub invert: Option<bool>,
    /// The `ipproto` value in a `ipv4.routing-rules` entry.
    pub ipproto: Option<String>,
    /// The `oifname` value in a `ipv4.routing-rules` entry.
    pub oifname: Option<String>,
    /// The `priority` value in a `ipv4.routing-rules` entry.
    pub priority: Option<u32>,
    /// The `sport-end` value in a `ipv4.routing-rules` entry.
    pub sport_end: Option<u16>,
    /// The `sport-start` value in a `ipv4.routing-rules` entry.
    pub sport_start: Option<u16>,
    /// The route-table lookup suppression prefix length (`supress-prefixlength`).
    pub supress_prefixlength: Option<i32>,
    /// The `table` value in a `ipv4.routing-rules` entry.
    pub table: Option<u32>,
    /// The `to` value in a `ipv4.routing-rules` entry.
    pub to: Option<String>,
    /// The `tos` value in a `ipv4.routing-rules` entry.
    pub tos: Option<u8>,
    /// The `to-len` value in a `ipv4.routing-rules` entry.
    pub to_len: Option<u8>,
    /// The `range-end` value in a `ipv4.routing-rules` entry.
    pub range_end: Option<u32>,
    /// The `range-start` value in a `ipv4.routing-rules` entry.
    pub range_start: Option<u32>,
}

impl From<Ipv4RoutingRule> for HashMap<&str, Value<'_>> {
    fn from(rule: Ipv4RoutingRule) -> Self {
        let mut map = HashMap::new();

        insert_opt!(map, "action", rule.action);
        insert_opt!(map, "dport-end", rule.dport_end);
        insert_opt!(map, "dport-start", rule.dport_start);
        insert_opt!(map, "family", rule.family);
        insert_opt!(map, "from", rule.from);
        insert_opt!(map, "from-len", rule.from_len);
        insert_opt!(map, "fwmark", rule.fwmark);
        insert_opt!(map, "fwmask", rule.fwmask);
        insert_opt!(map, "iifname", rule.iifname);
        insert_opt!(map, "invert", rule.invert);
        insert_opt!(map, "ipproto", rule.ipproto);
        insert_opt!(map, "oifname", rule.oifname);
        insert_opt!(map, "priority", rule.priority);
        insert_opt!(map, "sport-end", rule.sport_end);
        insert_opt!(map, "sport-start", rule.sport_start);
        insert_opt!(map, "supress-prefixlength", rule.supress_prefixlength);
        insert_opt!(map, "table", rule.table);
        insert_opt!(map, "to", rule.to);
        insert_opt!(map, "tos", rule.tos);
        insert_opt!(map, "to-len", rule.to_len);
        insert_opt!(map, "range-end", rule.range_end);
        insert_opt!(map, "range-start", rule.range_start);

        map
    }
}

impl Ipv4RoutingRule {
    pub(crate) fn from_dict(dict: Dict) -> Result<Ipv4RoutingRule, ConnectionError> {
        let action: Option<u8> = dict.get(&"action")?;
        let dport_end: Option<u16> = dict.get(&"dport-end")?;
        let dport_start: Option<u16> = dict.get(&"dport-start")?;
        let family: Option<i32> = dict.get(&"family")?;
        let from: Option<String> = dict.get(&"from")?;
        let from_len: Option<u8> = dict.get(&"from-len")?;
        let fwmark: Option<u32> = dict.get(&"fwmark")?;
        let fwmask: Option<u32> = dict.get(&"fwmask")?;
        let iifname: Option<String> = dict.get(&"iifname")?;
        let invert: Option<bool> = dict.get(&"invert")?;
        let ipproto: Option<String> = dict.get(&"ipproto")?;
        let oifname: Option<String> = dict.get(&"oifname")?;
        let priority: Option<u32> = dict.get(&"priority")?;
        let sport_end: Option<u16> = dict.get(&"sport-end")?;
        let sport_start: Option<u16> = dict.get(&"sport-start")?;
        let supress_prefixlength: Option<i32> = dict.get(&"supress-prefixlength")?;
        let table: Option<u32> = dict.get(&"table")?;
        let to: Option<String> = dict.get(&"to")?;
        let tos: Option<u8> = dict.get(&"tos")?;
        let to_len: Option<u8> = dict.get(&"to-len")?;
        let range_end: Option<u32> = dict.get(&"range-end")?;
        let range_start: Option<u32> = dict.get(&"range-start")?;

        Ok(Ipv4RoutingRule {
            action,
            dport_end,
            dport_start,
            family,
            from,
            from_len,
            fwmark,
            fwmask,
            iifname,
            invert,
            ipproto,
            oifname,
            priority,
            sport_end,
            sport_start,
            supress_prefixlength,
            table,
            to,
            tos,
            to_len,
            range_end,
            range_start,
        })
    }
}

impl From<IpSettings<Ipv4Addr>> for IpV4Section {
    fn from(value: IpSettings<Ipv4Addr>) -> Self {
        Self {
            address_data: value
                .addresses
                .into_iter()
                .map(|a| Ipv4AddressData {
                    address: a.address,
                    prefix: u32::from(a.prefix),
                    gateway: None,
                })
                .collect(),
            auto_route_ext_gw: None,
            dad_timeout: 0,
            dhcp_client_id: None,
            dhcp_dscp: None,
            dhcp_fqdn: None,
            dhcp_hostname: None,
            dhcp_hostname_flags: 0,
            dhcp_iaid: None,
            dhcp_ipv6_only_preferred: BoolWithDefault::Default,
            dhcp_reject_servers: Vec::new(),
            dhcp_send_hostname_v2: BoolWithDefault::Default,
            dhcp_send_release: BoolWithDefault::Default,
            dhcp_timeout: 0,
            dhcp_vendor_class_identifier: None,
            dns_data: value.dns.iter().map(ToString::to_string).collect(),
            dns_options: Vec::new(),
            dns_priority: 0,
            dns_search: value.dns_search,
            forwarding: Forwarding::Default,
            ignore_auto_dns: value.ignore_auto_dns,
            ignore_auto_routes: false,
            link_local: LinkLocal::Default,
            may_fail: true,
            method: value.method,
            never_default: value.never_default,
            replace_local_rule: None,
            required_timeout: -1,
            route_data: value
                .routes
                .into_iter()
                .map(|r| Ipv4RouteData {
                    address: r.dest.address,
                    prefix: u32::from(r.dest.prefix),
                    next_hop: r.next_hop,
                })
                .collect(),
            route_metric: -1,
            route_table: 0,
            routed_dns: BoolWithDefault::Default,
            shared_dhcp_lease_time: 0,
            shared_dhcp_range: None,
            gateway: value.gateway.map(|g| g.to_string()),
            routing_rules: Vec::new(),
        }
    }
}

impl From<IpV4Section> for IpSettings<Ipv4Addr> {
    fn from(value: IpV4Section) -> Self {
        Self {
            method: value.method,
            addresses: value
                .address_data
                .into_iter()
                .map(|a| IpAddress::new(a.address, a.prefix as u8))
                .collect(),
            gateway: value.gateway.and_then(|g| g.parse().ok()),
            dns: value
                .dns_data
                .into_iter()
                .filter_map(|d| d.parse().ok())
                .collect(),
            dns_search: value.dns_search,
            routes: value
                .route_data
                .into_iter()
                .map(|r| IpRoute {
                    dest: IpAddress::new(r.address, r.prefix as u8),
                    next_hop: r.next_hop,
                    metric: None,
                })
                .collect(),
            never_default: value.never_default,
            ignore_auto_dns: value.ignore_auto_dns,
        }
    }
}
