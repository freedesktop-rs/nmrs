use std::{collections::HashMap, net::Ipv4Addr};

use zbus::zvariant::{Array, OwnedValue};

use crate::api::settings::error::SettingsError as ConnectionError;

mod connection;
mod ipv4;
mod ipv6;
mod proxy;
mod types;

pub use connection::{
    ConnectionSection, DnsOverTls, Llmnr, Mdns, Metered, MultiConnect, SettingsType,
};
pub use ipv4::{
    DhcpDscp, Forwarding, IpV4Section, Ipv4AddressData, Ipv4RouteData, Ipv4RoutingRule, LinkLocal,
    SharedDhcpRangeV4,
};
pub use ipv6::{
    AddrGenMode, DnsOptionsV6, Ip6Privacy, IpV6Section, Ipv6AddressData, Ipv6RouteData,
    SharedDhcpRangeV6, parse_dns_v6,
};
pub use proxy::ProxySection;
pub use types::BoolWithDefault;

// The canonical IP method enum lives in `nmrs::IpMethod`; the settings layer
// re-exports it so the editable sections and the decoded `IpSettings` share one
// type.
pub use crate::IpMethod;

impl std::fmt::Display for IpMethod {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let method = match self {
            IpMethod::Auto => "auto",
            IpMethod::Dhcp => "dhcp",
            IpMethod::Manual => "manual",
            IpMethod::Disabled => "disabled",
            IpMethod::LinkLocal => "link-local",
            IpMethod::Shared => "shared",
            IpMethod::Ignore => "ignore",
            IpMethod::Other(other) => other.as_str(),
        };
        f.write_str(method)
    }
}

/// Read IPv4 DNS servers from `dns-data`, falling back to NetworkManager's legacy `dns` array.
pub fn parse_dns_v4(map: &HashMap<String, OwnedValue>) -> Result<Vec<String>, ConnectionError> {
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
            if let Ok(ip) = entry.downcast_ref::<u32>() {
                out.push(Ipv4Addr::from(ip.to_ne_bytes()).to_string());
            }
        }
        if !out.is_empty() {
            return Ok(out);
        }
    }
    Ok(Vec::new())
}
