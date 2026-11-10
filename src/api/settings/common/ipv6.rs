use std::{collections::HashMap, fmt::Display, net::Ipv6Addr, str::FromStr};

use zbus::zvariant::{Array, Dict, OwnedValue, Value};

use crate::api::settings::error::SettingsError as ConnectionError;
use crate::{IpAddress, IpRoute, IpSettings};

use super::{BoolWithDefault, DhcpDscp, Forwarding, IpMethod};

/// Read IPv6 DNS servers from `dns-data`, falling back to NetworkManager's legacy `dns` array.
pub fn parse_dns_v6(map: &HashMap<String, OwnedValue>) -> Result<Vec<String>, ConnectionError> {
    if let Some(v) = map.get("dns-data")
        && let Ok(arr) = v.downcast_ref::<Array<'_>>()
    {
        let mut out = Vec::new();
        for entry in arr.iter() {
            if let Ok(s) = entry.downcast_ref::<String>() {
                out.push(s);
            }
        }
        if !out.is_empty() {
            return Ok(out);
        }
    }
    if let Some(v) = map.get("dns")
        && let Ok(arr) = v.downcast_ref::<Array<'_>>()
    {
        let mut out = Vec::new();
        for entry in arr.iter() {
            let Ok(inner) = entry.downcast_ref::<Array<'_>>() else {
                continue;
            };
            let Ok(bytes) = inner
                .iter()
                .map(|v| v.downcast_ref::<u8>())
                .collect::<Result<Vec<u8>, _>>()
            else {
                continue;
            };
            if bytes.len() == 16 {
                let mut octets = [0u8; 16];
                octets.copy_from_slice(&bytes);
                out.push(Ipv6Addr::from(octets).to_string());
            }
        }
        if !out.is_empty() {
            return Ok(out);
        }
    }
    Ok(Vec::new())
}

/// Editable NetworkManager IPv6 settings.
#[derive(Debug, Clone)]
pub struct IpV6Section {
    /// The NetworkManager `ipv6.addr-gen-mode` property.
    pub addr_gen_mode: AddrGenMode,
    /// The NetworkManager `ipv6.address-data` property.
    pub address_data: Vec<Ipv6AddressData>,
    /// The NetworkManager `ipv6.auto-route-ext-gw` property.
    pub auto_route_ext_gw: Option<BoolWithDefault>,
    /// The NetworkManager `ipv6.dad-timeout` property.
    pub dad_timeout: i32,
    /// The NetworkManager `ipv6.dhcp-dscp` property.
    pub dhcp_dscp: Option<DhcpDscp>,
    /// The NetworkManager `ipv6.dhcp-duid` property.
    pub dhcp_duid: Option<String>,
    /// The NetworkManager `ipv6.dhcp-hostname` property.
    pub dhcp_hostname: Option<String>,
    /// The NetworkManager `ipv6.dhcp-hostname-flags` property.
    pub dhcp_hostname_flags: u32,
    /// The NetworkManager `ipv6.dhcp-iaid` property.
    pub dhcp_iaid: Option<String>,
    /// The NetworkManager `ipv6.dhcp-pd-hint` property.
    pub dhcp_pd_hint: Option<String>,
    /// The NetworkManager `ipv6.dhcp-reject-servers` property.
    pub dhcp_reject_servers: Vec<String>,
    /// The NetworkManager `ipv6.dhcp-send-hostname-v2` property.
    pub dhcp_send_hostname_v2: BoolWithDefault,
    /// The NetworkManager `ipv6.dhcp-send-release` property.
    pub dhcp_send_release: BoolWithDefault,
    /// The NetworkManager `ipv6.dhcp-timeout` property.
    pub dhcp_timeout: i32,
    /// The NetworkManager `ipv6.dns-data` property.
    pub dns_data: Vec<String>,
    /// The NetworkManager `ipv6.dns-options` property.
    pub dns_options: Vec<DnsOptionsV6>,
    /// The NetworkManager `ipv6.dns-priority` property.
    pub dns_priority: i32,
    /// The NetworkManager `ipv6.dns-search` property.
    pub dns_search: Vec<String>,
    /// The NetworkManager `ipv6.forwarding` property.
    pub forwarding: Forwarding,
    /// The NetworkManager `ipv6.ignore-auto-dns` property.
    pub ignore_auto_dns: bool,
    /// The NetworkManager `ipv6.ignore-auto-routes` property.
    pub ignore_auto_routes: bool,
    /// The NetworkManager `ipv6.ip6-privacy` property.
    pub ip6_privacy: Option<Ip6Privacy>,
    /// The NetworkManager `ipv6.may-fail` property.
    pub may_fail: bool,
    /// The NetworkManager `ipv6.method` property.
    pub method: IpMethod,
    /// The NetworkManager `ipv6.mtu` property.
    pub mtu: u32,
    /// The NetworkManager `ipv6.never-default` property.
    pub never_default: bool,
    /// The NetworkManager `ipv6.ra-timeout` property.
    pub ra_timeout: i32,
    /// The NetworkManager `ipv6.replace-local-rule` property.
    pub replace_local_rule: Option<BoolWithDefault>,
    /// The NetworkManager `ipv6.required-timeout` property.
    pub required_timeout: i32,
    /// The NetworkManager `ipv6.route-data` property.
    pub route_data: Vec<Ipv6RouteData>,
    /// The NetworkManager `ipv6.route-metric` property.
    pub route_metric: i64,
    /// The NetworkManager `ipv6.route-table` property.
    pub route_table: u32,
    /// The NetworkManager `ipv6.routed-dns` property.
    pub routed_dns: BoolWithDefault,
    // TODO: implement this properly
    // pub routing_rules
    /// The NetworkManager `ipv6.shared-dhcp-lease-time` property.
    pub shared_dhcp_lease_time: i32,
    /// The NetworkManager `ipv6.shared-dhcp-range` property.
    pub shared_dhcp_range: Option<SharedDhcpRangeV6>,
    /// The NetworkManager `ipv6.temp-preferred-lifetime` property.
    pub temp_preferred_lifetime: i32,
    /// The NetworkManager `ipv6.temp-valid-lifetime` property.
    pub temp_valid_lifetime: i32,
    /// The NetworkManager `ipv6.token` property.
    pub token: Option<String>,
    /// The NetworkManager `ipv6.gateway` property.
    pub gateway: Option<String>,
}

impl IpV6Section {
    pub(crate) const fn get_name() -> &'static str {
        "ipv6"
    }

    pub(crate) fn validate(&self) -> Result<(), ConnectionError> {
        for address in &self.address_data {
            if address.prefix > 128 {
                return Err(ConnectionError::FromStrError(
                    address.prefix.to_string(),
                    "IPv6 prefix (0..=128)",
                ));
            }
        }
        for route in &self.route_data {
            if route.prefix > 128 {
                return Err(ConnectionError::FromStrError(
                    route.prefix.to_string(),
                    "IPv6 route prefix (0..=128)",
                ));
            }
        }
        Ok(())
    }
}

impl From<IpV6Section> for HashMap<&str, Value<'_>> {
    fn from(value: IpV6Section) -> Self {
        let mut ipv6 = HashMap::new();

        insert_value!(ipv6, "addr-gen-mode", i32::from(value.addr_gen_mode));

        if !value.address_data.is_empty() {
            let addresses: Vec<HashMap<&str, Value<'static>>> =
                value.address_data.iter().map(|a| a.to_dict()).collect();
            insert_value!(ipv6, "address-data", addresses);
        }

        insert_opt!(
            ipv6,
            "auto-route-ext-gw",
            value.auto_route_ext_gw.map(i32::from)
        );

        insert_value!(ipv6, "dad-timeout", value.dad_timeout);

        insert_opt!(ipv6, "dhcp-dscp", value.dhcp_dscp.map(|v| v.to_string()));
        insert_opt!(ipv6, "dhcp-duid", value.dhcp_duid);
        insert_opt!(ipv6, "dhcp-hostname", value.dhcp_hostname);
        insert_value!(ipv6, "dhcp-hostname-flags", value.dhcp_hostname_flags);
        insert_opt!(ipv6, "dhcp-iaid", value.dhcp_iaid);
        insert_opt!(ipv6, "dhcp-pd-hint", value.dhcp_pd_hint);

        if !value.dhcp_reject_servers.is_empty() {
            insert_value!(ipv6, "dhcp-reject-servers", value.dhcp_reject_servers);
        }

        insert_value!(
            ipv6,
            "dhcp-send-hostname-v2",
            i32::from(value.dhcp_send_hostname_v2)
        );
        insert_value!(
            ipv6,
            "dhcp-send-release",
            i32::from(value.dhcp_send_release)
        );
        insert_value!(ipv6, "dhcp-timeout", value.dhcp_timeout);

        if !value.dns_data.is_empty() {
            insert_value!(ipv6, "dns-data", value.dns_data);
        }

        if !value.dns_options.is_empty() {
            let v: Vec<String> = value
                .dns_options
                .iter()
                .map(DnsOptionsV6::to_string)
                .collect();
            insert_value!(ipv6, "dns-options", v);
        }

        insert_value!(ipv6, "dns-priority", value.dns_priority);

        if !value.dns_search.is_empty() {
            insert_value!(ipv6, "dns-search", value.dns_search);
        }

        insert_value!(ipv6, "forwarding", i32::from(value.forwarding));
        insert_value!(ipv6, "ignore-auto-dns", value.ignore_auto_dns);
        insert_value!(ipv6, "ignore-auto-routes", value.ignore_auto_routes);

        insert_opt!(ipv6, "ip6-privacy", value.ip6_privacy.map(i32::from));

        insert_value!(ipv6, "may-fail", value.may_fail);
        insert_value!(ipv6, "method", value.method.to_string());
        insert_value!(ipv6, "mtu", value.mtu);
        insert_value!(ipv6, "never-default", value.never_default);
        insert_value!(ipv6, "ra-timeout", value.ra_timeout);

        insert_opt!(
            ipv6,
            "replace-local-rule",
            value.replace_local_rule.map(i32::from)
        );

        insert_value!(ipv6, "required-timeout", value.required_timeout);

        if !value.route_data.is_empty() {
            let routes: Vec<HashMap<&str, Value<'static>>> =
                value.route_data.iter().map(|r| r.to_dict()).collect();
            insert_value!(ipv6, "route-data", routes);
        }

        insert_value!(ipv6, "route-metric", value.route_metric);
        insert_value!(ipv6, "route-table", value.route_table);
        insert_value!(ipv6, "routed-dns", i32::from(value.routed_dns));
        insert_value!(ipv6, "shared-dhcp-lease-time", value.shared_dhcp_lease_time);

        insert_opt!(
            ipv6,
            "shared-dhcp-range",
            value.shared_dhcp_range.map(|v| v.to_string())
        );

        insert_value!(
            ipv6,
            "temp-preferred-lifetime",
            value.temp_preferred_lifetime
        );
        insert_value!(ipv6, "temp-valid-lifetime", value.temp_valid_lifetime);

        insert_opt!(ipv6, "token", value.token);
        insert_opt!(ipv6, "gateway", value.gateway);

        ipv6
    }
}

impl TryFrom<&HashMap<String, OwnedValue>> for IpV6Section {
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
                    out.push(Ipv6AddressData::from_dict(dict)?);
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
                    out.push(Ipv6RouteData::from_dict(dict)?);
                }
                out
            }
            None => Vec::new(),
        };

        let shared_dhcp_range_str = extract_optional_field!(value, "shared-dhcp-range", String)?;
        let shared_dhcp_range = match shared_dhcp_range_str {
            Some(s) => Some(SharedDhcpRangeV6::from_str(&s)?),
            None => None,
        };

        let dhcp_dscp_str = extract_optional_field!(value, "dhcp-dscp", String)?;
        let dhcp_dscp = dhcp_dscp_str.map(|s| DhcpDscp::from_str(&s)).transpose()?;

        let forwarding_int = extract_field!(value, "forwarding", i32).unwrap_or(-1);
        let forwarding = Forwarding::from(forwarding_int);

        let addr_gen_mode_int = extract_field!(value, "addr-gen-mode", i32).unwrap_or(3);
        let addr_gen_mode = AddrGenMode::from(addr_gen_mode_int);

        let ip6_privacy_int = extract_field!(value, "ip6-privacy", i32).unwrap_or(-1);
        let ip6_privacy = if (0..=2).contains(&ip6_privacy_int) {
            Some(Ip6Privacy::from(ip6_privacy_int))
        } else {
            None
        };

        Ok(IpV6Section {
            addr_gen_mode,
            address_data,
            auto_route_ext_gw: extract_optional_field!(value, "auto-route-ext-gw", i32)?
                .map(BoolWithDefault::from),
            dad_timeout: extract_field!(value, "dad-timeout", i32).unwrap_or(-1),
            dhcp_dscp,
            dhcp_duid: extract_optional_field!(value, "dhcp-duid", String)?,
            dhcp_hostname: extract_optional_field!(value, "dhcp-hostname", String)?,
            dhcp_hostname_flags: extract_field!(value, "dhcp-hostname-flags", u32).unwrap_or(0),
            dhcp_iaid: extract_optional_field!(value, "dhcp-iaid", String)?,
            dhcp_pd_hint: extract_optional_field!(value, "dhcp-pd-hint", String)?,
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
            dns_data: parse_dns_v6(value)?,
            dns_options: extract_vec_field_or_default!(value, "dns-options", String)?
                .iter()
                .map(|s| DnsOptionsV6::from_str(s))
                .collect::<Result<Vec<_>, _>>()?,
            dns_priority: extract_field!(value, "dns-priority", i32).unwrap_or(0),
            dns_search: extract_vec_field_or_default!(value, "dns-search", String)?,
            forwarding,
            ignore_auto_dns: extract_field!(value, "ignore-auto-dns", bool).unwrap_or(false),
            ignore_auto_routes: extract_field!(value, "ignore-auto-routes", bool).unwrap_or(false),
            ip6_privacy,
            may_fail: extract_field!(value, "may-fail", bool).unwrap_or(true),
            method: extract_optional_field!(value, "method", String)?
                .map(IpMethod::from)
                .unwrap_or(IpMethod::Auto),
            mtu: extract_field!(value, "mtu", u32).unwrap_or(0),
            never_default: extract_field!(value, "never-default", bool).unwrap_or(false),
            ra_timeout: extract_field!(value, "ra-timeout", i32).unwrap_or(0),
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
            temp_preferred_lifetime: extract_field!(value, "temp-preferred-lifetime", i32)
                .unwrap_or(0),
            temp_valid_lifetime: extract_field!(value, "temp-valid-lifetime", i32).unwrap_or(0),
            token: extract_optional_field!(value, "token", String)?,
            gateway: extract_optional_field!(value, "gateway", String)?,
        })
    }
}

int_enum!(AddrGenMode : i32 {
    EUI64 = 0,
    StablePrivacy = 1,
    DefaultOrEUI64 = 2,
    Default = 3,
} default Default);

int_enum!(Ip6Privacy : i32 {
    Unknown = -1,
    Disabled = 0,
    EnabledPrefPub = 1,
    EnabledPrefTemp = 2,
} default Unknown);

string_enum!(DnsOptionsV6 {
    Attempts => "attempts",
    Debug => "debug",
    Edns0 => "edns0",
    Inet6 => "inet6",
    Ip6Bytestring => "ip6-bytestring",
    Ip6Dotint => "ip6-dotint",
    Ndots => "ndots",
    NoAAAA => "no-aaaa",
    NoCheckNames => "no-check-names",
    NoIp6Dotint => "no-ip6-dotint",
    NoReload => "no-reload",
    NoTldQuery => "no-tld-query",
    Rotate => "rotate",
    SingleRequest => "single-request",
    SingleRequestReopen => "single-request-reopen",
    Timeout => "timeout",
    TrustAd => "trust-ad",
    UseVc => "use-vc",
    NoAddEdns0 => "no-add-edns0",
    NoAddTrustAd => "no-add-trust-ad",
});

/// Start and end addresses for a shared IPv6 DHCP range.
#[derive(Debug, Clone)]
pub struct SharedDhcpRangeV6 {
    /// The start IPv6 address in the shared DHCP range.
    pub start_add: Ipv6Addr,
    /// The end IPv6 address in the shared DHCP range.
    pub end_add: Ipv6Addr,
}

impl Display for SharedDhcpRangeV6 {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "{:?},{:?}",
            self.start_add.to_string(),
            self.end_add.to_string()
        )
    }
}

impl FromStr for SharedDhcpRangeV6 {
    type Err = ConnectionError;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let (start, end) = s.split_once(',').ok_or(ConnectionError::FromStrError(
            s.to_string(),
            "SharedDhcpRange",
        ))?;

        let start_add = Ipv6Addr::from_str(start.trim())
            .map_err(|_| ConnectionError::FromStrError(s.to_string(), "SharedDhcpRange"))?;

        let end_add = Ipv6Addr::from_str(end.trim())
            .map_err(|_| ConnectionError::FromStrError(s.to_string(), "SharedDhcpRange"))?;

        Ok(SharedDhcpRangeV6 { start_add, end_add })
    }
}
/// An IPv6 route in NetworkManager's `ipv6.route-data` array.
#[derive(Debug, Clone)]
pub struct Ipv6RouteData {
    /// The `address` value in a `ipv6.route-data` entry.
    pub address: Ipv6Addr, // e.g. "192.168.1.10"
    /// The `prefix` value in a `ipv6.route-data` entry.
    pub prefix: u32, // e.g. 24
    /// The `next-hop` value in a `ipv6.route-data` entry.
    pub next_hop: Option<Ipv6Addr>,
}

impl Ipv6RouteData {
    fn from_dict(dict: Dict) -> Result<Ipv6RouteData, ConnectionError> {
        let address_s: String = dict
            .get(&"address")?
            .ok_or(ConnectionError::MissingField("ipv6 route address"))?;
        let address = Ipv6Addr::from_str(&address_s).map_err(|e| {
            ConnectionError::FromStrError(
                format!("ipv6 route address {address_s}: {e}"),
                "Ipv6Addr",
            )
        })?;

        let prefix: u32 = dict
            .get(&"prefix")?
            .ok_or(ConnectionError::MissingField("ipv6 route prefix"))?;
        if prefix > 128 {
            return Err(ConnectionError::FromStrError(
                prefix.to_string(),
                "IPv6 route prefix (0..=128)",
            ));
        }

        let mut next_hop = None;
        if let Some(next_hop_s) = dict.get::<_, String>(&"next-hop")? {
            next_hop = Some(Ipv6Addr::from_str(&next_hop_s).map_err(|e| {
                ConnectionError::FromStrError(
                    format!("ipv6 route next-hop {next_hop_s}: {e}"),
                    "Ipv6Addr",
                )
            })?);
        }

        Ok(Ipv6RouteData {
            address,
            prefix,
            next_hop,
        })
    }

    fn to_dict(&self) -> HashMap<&'static str, Value<'static>> {
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

/// An IPv6 address in NetworkManager's `ipv6.address-data` array.
#[derive(Debug, Clone)]
pub struct Ipv6AddressData {
    /// The `address` value in a `ipv6.address-data` entry.
    pub address: Ipv6Addr,
    /// The address prefix length in bits.
    pub prefix: u32, // e.g. 24
    /// The `gateway` value in a `ipv6.address-data` entry.
    pub gateway: Option<Ipv6Addr>,
}

impl Ipv6AddressData {
    fn from_dict(dict: Dict) -> Result<Ipv6AddressData, ConnectionError> {
        let address_s: String = dict
            .get(&"address")?
            .ok_or(ConnectionError::MissingField("ipv6 address"))?;
        let address = Ipv6Addr::from_str(&address_s).map_err(|e| {
            ConnectionError::FromStrError(format!("ipv6 address {address_s}: {e}"), "Ipv6Addr")
        })?;

        let prefix: u32 = dict
            .get(&"prefix")?
            .ok_or(ConnectionError::MissingField("ipv6 address prefix"))?;
        if prefix > 128 {
            return Err(ConnectionError::FromStrError(
                prefix.to_string(),
                "IPv6 prefix (0..=128)",
            ));
        }

        let mut gateway = None;
        if let Some(gateway_s) = dict.get::<_, String>(&"gateway")? {
            gateway = Some(Ipv6Addr::from_str(&gateway_s).map_err(|e| {
                ConnectionError::FromStrError(
                    format!("ipv6 address gateway {gateway_s}: {e}"),
                    "Ipv6Addr",
                )
            })?);
        }

        Ok(Ipv6AddressData {
            address,
            prefix,
            gateway,
        })
    }

    fn to_dict(&self) -> HashMap<&'static str, Value<'static>> {
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

impl From<IpSettings<Ipv6Addr>> for IpV6Section {
    fn from(value: IpSettings<Ipv6Addr>) -> Self {
        Self {
            addr_gen_mode: AddrGenMode::Default,
            address_data: value
                .addresses
                .into_iter()
                .map(|a| Ipv6AddressData {
                    address: a.address,
                    prefix: u32::from(a.prefix),
                    gateway: None,
                })
                .collect(),
            auto_route_ext_gw: None,
            dad_timeout: 0,
            dhcp_dscp: None,
            dhcp_duid: None,
            dhcp_hostname: None,
            dhcp_hostname_flags: 0,
            dhcp_iaid: None,
            dhcp_pd_hint: None,
            dhcp_reject_servers: Vec::new(),
            dhcp_send_hostname_v2: BoolWithDefault::Default,
            dhcp_send_release: BoolWithDefault::Default,
            dhcp_timeout: 0,
            dns_data: value.dns.iter().map(ToString::to_string).collect(),
            dns_options: Vec::new(),
            dns_priority: 0,
            dns_search: value.dns_search,
            forwarding: Forwarding::Default,
            ignore_auto_dns: value.ignore_auto_dns,
            ignore_auto_routes: false,
            ip6_privacy: None,
            may_fail: true,
            method: value.method,
            mtu: 0,
            never_default: value.never_default,
            ra_timeout: 0,
            replace_local_rule: None,
            required_timeout: -1,
            route_data: value
                .routes
                .into_iter()
                .map(|r| Ipv6RouteData {
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
            temp_preferred_lifetime: -1,
            temp_valid_lifetime: -1,
            token: None,
            gateway: value.gateway.map(|g| g.to_string()),
        }
    }
}

impl From<IpV6Section> for IpSettings<Ipv6Addr> {
    fn from(value: IpV6Section) -> Self {
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
