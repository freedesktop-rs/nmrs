//! Parser for WireGuard `.conf` files in the wg-quick format.
//!
//! Only the keys NetworkManager has a setting for are read. Everything else
//! wg-quick understands (`PreUp`, `PostUp`, `Table`, `ListenPort`, ...) is
//! skipped, so a file written for wg-quick still imports.

use std::net::IpAddr;

use crate::api::models::{ConnectionError, WireGuardPeer};

/// What nmrs reads from a WireGuard `.conf` file.
///
/// `Debug` is deliberately not derived: the struct holds the private key.
#[derive(Default)]
pub(crate) struct WireGuardConf {
    pub(crate) private_key: Option<String>,
    pub(crate) addresses: Vec<String>,
    pub(crate) dns: Vec<String>,
    pub(crate) mtu: Option<u32>,
    pub(crate) peers: Vec<WireGuardPeer>,
}

impl WireGuardConf {
    /// The one interface address a NetworkManager WireGuard profile built by
    /// [`WireGuardBuilder`](crate::builders::WireGuardBuilder) can carry.
    ///
    /// wg-quick accepts several addresses. The first IPv4 address wins, and
    /// the first address of any family is used when there is none. An address
    /// without a prefix gets the host prefix, as wg-quick assigns it.
    pub(crate) fn primary_address(&self) -> Option<String> {
        let ip_of = |address: &str| {
            address
                .split_once('/')
                .map_or(address, |(ip, _)| ip)
                .trim()
                .parse::<IpAddr>()
                .ok()
        };
        let address = self
            .addresses
            .iter()
            .find(|address| matches!(ip_of(address), Some(IpAddr::V4(_))))
            .or_else(|| self.addresses.first())?;

        Some(match (address.contains('/'), ip_of(address)) {
            (false, Some(IpAddr::V4(_))) => format!("{address}/32"),
            (false, Some(IpAddr::V6(_))) => format!("{address}/128"),
            _ => address.clone(),
        })
    }

    /// The `DNS` entries that are server addresses.
    ///
    /// wg-quick also allows search domains in `DNS`; those are dropped.
    pub(crate) fn dns_servers(&self) -> Vec<String> {
        self.dns
            .iter()
            .filter(|entry| entry.parse::<IpAddr>().is_ok())
            .cloned()
            .collect()
    }
}

#[derive(Default)]
struct PeerSection {
    line: usize,
    public_key: Option<String>,
    endpoint: Option<String>,
    allowed_ips: Vec<String>,
    preshared_key: Option<String>,
    persistent_keepalive: Option<u32>,
}

impl PeerSection {
    fn finish(self) -> Result<WireGuardPeer, ConnectionError> {
        let missing = |key: &str| {
            ConnectionError::InvalidPeers(format!("[Peer] at line {} is missing {key}", self.line))
        };
        let public_key = self.public_key.ok_or_else(|| missing("PublicKey"))?;
        let endpoint = self.endpoint.ok_or_else(|| missing("Endpoint"))?;
        if self.allowed_ips.is_empty() {
            return Err(missing("AllowedIPs"));
        }

        let mut peer = WireGuardPeer::new(public_key, endpoint, self.allowed_ips);
        if let Some(key) = self.preshared_key {
            peer = peer.with_preshared_key(key);
        }
        if let Some(interval) = self.persistent_keepalive {
            peer = peer.with_persistent_keepalive(interval);
        }
        Ok(peer)
    }
}

enum Section {
    None,
    Interface,
    Peer(PeerSection),
}

/// Parses the content of a WireGuard `.conf` file.
///
/// Section names and keys are matched case-insensitively, `#` starts a
/// comment, and `Address`, `DNS`, and `AllowedIPs` may be repeated. A
/// missing `PrivateKey` or `Address` is not an error here: the builder
/// reports it, after the caller has had a chance to supply one.
pub(crate) fn parse(content: &str) -> Result<WireGuardConf, ConnectionError> {
    let mut conf = WireGuardConf::default();
    let mut section = Section::None;

    for (index, raw_line) in content.trim_start_matches('\u{feff}').lines().enumerate() {
        let line_number = index + 1;
        let line = raw_line
            .split_once('#')
            .map_or(raw_line, |(before, _)| before)
            .trim();
        if line.is_empty() {
            continue;
        }

        if let Some(name) = line.strip_prefix('[').and_then(|l| l.strip_suffix(']')) {
            if let Section::Peer(peer) = std::mem::replace(&mut section, Section::None) {
                conf.peers.push(peer.finish()?);
            }
            section = match name.trim().to_ascii_lowercase().as_str() {
                "interface" => Section::Interface,
                "peer" => Section::Peer(PeerSection {
                    line: line_number,
                    ..PeerSection::default()
                }),
                _ => return Err(invalid(line_number, format!("unknown section [{name}]"))),
            };
            continue;
        }

        let Some((key, value)) = line.split_once('=') else {
            return Err(invalid(line_number, "expected `Key = Value`".into()));
        };
        let key = key.trim().to_ascii_lowercase();
        let value = value.trim();

        match &mut section {
            Section::None => {
                return Err(invalid(
                    line_number,
                    "setting before any [Interface] or [Peer] section".into(),
                ));
            }
            Section::Interface => match key.as_str() {
                "privatekey" => conf.private_key = Some(value.to_string()),
                "address" => conf.addresses.extend(list(value)),
                "dns" => conf.dns.extend(list(value)),
                "mtu" => conf.mtu = Some(number("MTU", value, line_number)?),
                _ => {}
            },
            Section::Peer(peer) => match key.as_str() {
                "publickey" => peer.public_key = Some(value.to_string()),
                "endpoint" => peer.endpoint = Some(value.to_string()),
                "allowedips" => peer.allowed_ips.extend(list(value)),
                "presharedkey" => peer.preshared_key = Some(value.to_string()),
                "persistentkeepalive" => {
                    peer.persistent_keepalive = if value.eq_ignore_ascii_case("off") {
                        None
                    } else {
                        Some(number("PersistentKeepalive", value, line_number)?)
                    };
                }
                _ => {}
            },
        }
    }

    if let Section::Peer(peer) = section {
        conf.peers.push(peer.finish()?);
    }

    Ok(conf)
}

fn list(value: &str) -> impl Iterator<Item = String> + '_ {
    value
        .split(',')
        .map(str::trim)
        .filter(|entry| !entry.is_empty())
        .map(str::to_string)
}

fn number(key: &str, value: &str, line: usize) -> Result<u32, ConnectionError> {
    value
        .parse()
        .map_err(|_| invalid(line, format!("{key} is not a number: '{value}'")))
}

fn invalid(line: usize, reason: String) -> ConnectionError {
    ConnectionError::InvalidInput {
        field: "WireGuard config".into(),
        reason: format!("line {line}: {reason}"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const PRIVATE_KEY: &str = "YBk6X3pP8KjKz7+HFWzVHNqL3qTZq8hX9VxFQJ4zVmM=";
    const PUBLIC_KEY: &str = "HIgo9xNzJMWLKAShlKl6/bUT1VI9Q0SDBXGtLXkPFXc=";
    const PRESHARED_KEY: &str = "PSKABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklm=";

    fn interface(body: &str) -> String {
        format!("[Interface]\nPrivateKey = {PRIVATE_KEY}\n{body}")
    }

    fn invalid_reason(content: &str) -> String {
        match parse(content).err() {
            Some(ConnectionError::InvalidInput { field, reason }) => {
                assert_eq!(field, "WireGuard config");
                reason
            }
            other => panic!("expected InvalidInput, got {other:?}"),
        }
    }

    fn peer_error(content: &str) -> String {
        match parse(content).err() {
            Some(ConnectionError::InvalidPeers(message)) => message,
            other => panic!("expected InvalidPeers, got {other:?}"),
        }
    }

    #[test]
    fn parses_a_wg_quick_file() {
        let content = format!(
            "# exported by the provider\n\
             [Interface]\n\
             PrivateKey = {PRIVATE_KEY}\n\
             Address = 10.64.0.2/32, fc00:bbbb::2/128\n\
             DNS = 10.64.0.1\n\
             MTU = 1380\n\
             PostUp = iptables -A FORWARD -i %i -j ACCEPT\n\
             \n\
             [Peer]\n\
             PublicKey = {PUBLIC_KEY}\n\
             PresharedKey = {PRESHARED_KEY}\n\
             AllowedIPs = 0.0.0.0/0, ::/0\n\
             Endpoint = vpn.example.com:51820 # primary\n\
             PersistentKeepalive = 25\n"
        );

        let conf = parse(&content).unwrap();

        assert_eq!(conf.private_key.as_deref(), Some(PRIVATE_KEY));
        assert_eq!(conf.addresses, ["10.64.0.2/32", "fc00:bbbb::2/128"]);
        assert_eq!(conf.dns, ["10.64.0.1"]);
        assert_eq!(conf.mtu, Some(1380));
        assert_eq!(conf.peers.len(), 1);
        let peer = &conf.peers[0];
        assert_eq!(peer.public_key, PUBLIC_KEY);
        assert_eq!(peer.gateway, "vpn.example.com:51820");
        assert_eq!(peer.allowed_ips, ["0.0.0.0/0", "::/0"]);
        assert_eq!(peer.preshared_key.as_deref(), Some(PRESHARED_KEY));
        assert_eq!(peer.persistent_keepalive, Some(25));
    }

    #[test]
    fn sections_and_keys_are_case_insensitive() {
        let content = format!(
            "[interface]\nprivatekey={PRIVATE_KEY}\nADDRESS=10.0.0.2/24\n\
             [PEER]\npublickey={PUBLIC_KEY}\nendpoint=host:51820\nallowedips=0.0.0.0/0\n"
        );

        let conf = parse(&content).unwrap();

        assert_eq!(conf.private_key.as_deref(), Some(PRIVATE_KEY));
        assert_eq!(conf.addresses, ["10.0.0.2/24"]);
        assert_eq!(conf.peers[0].gateway, "host:51820");
    }

    #[test]
    fn repeated_list_keys_accumulate() {
        let content = interface(&format!(
            "Address = 10.0.0.2/24\nAddress = fd00::2/64\nDNS = 1.1.1.1\nDNS = 9.9.9.9\n\
             [Peer]\nPublicKey = {PUBLIC_KEY}\nEndpoint = host:51820\n\
             AllowedIPs = 10.0.0.0/24\nAllowedIPs = 192.168.0.0/16\n"
        ));

        let conf = parse(&content).unwrap();

        assert_eq!(conf.addresses, ["10.0.0.2/24", "fd00::2/64"]);
        assert_eq!(conf.dns, ["1.1.1.1", "9.9.9.9"]);
        assert_eq!(conf.peers[0].allowed_ips, ["10.0.0.0/24", "192.168.0.0/16"]);
    }

    #[test]
    fn every_peer_section_becomes_a_peer() {
        let content = interface(&format!(
            "[Peer]\nPublicKey = {PUBLIC_KEY}\nEndpoint = a.example.com:51820\nAllowedIPs = 10.0.0.0/24\n\
             [Peer]\nPublicKey = {PUBLIC_KEY}\nEndpoint = b.example.com:51820\nAllowedIPs = 10.1.0.0/24\n"
        ));

        let conf = parse(&content).unwrap();

        let endpoints: Vec<&str> = conf.peers.iter().map(|p| p.gateway.as_str()).collect();
        assert_eq!(endpoints, ["a.example.com:51820", "b.example.com:51820"]);
    }

    #[test]
    fn byte_order_mark_and_crlf_are_accepted() {
        let content = format!(
            "\u{feff}[Interface]\r\nPrivateKey = {PRIVATE_KEY}\r\nAddress = 10.0.0.2/24\r\n"
        );

        let conf = parse(&content).unwrap();

        assert_eq!(conf.private_key.as_deref(), Some(PRIVATE_KEY));
        assert_eq!(conf.addresses, ["10.0.0.2/24"]);
    }

    #[test]
    fn keepalive_off_means_no_keepalive() {
        let content = interface(&format!(
            "[Peer]\nPublicKey = {PUBLIC_KEY}\nEndpoint = host:51820\n\
             AllowedIPs = 0.0.0.0/0\nPersistentKeepalive = off\n"
        ));

        assert_eq!(parse(&content).unwrap().peers[0].persistent_keepalive, None);
    }

    #[test]
    fn primary_address_prefers_ipv4() {
        let conf = WireGuardConf {
            addresses: vec!["fd00::2/64".into(), "10.0.0.2/24".into()],
            ..WireGuardConf::default()
        };

        assert_eq!(conf.primary_address().as_deref(), Some("10.0.0.2/24"));
    }

    #[test]
    fn primary_address_falls_back_to_ipv6() {
        let conf = WireGuardConf {
            addresses: vec!["fd00::2/64".into()],
            ..WireGuardConf::default()
        };

        assert_eq!(conf.primary_address().as_deref(), Some("fd00::2/64"));
    }

    #[test]
    fn address_without_a_prefix_gets_the_host_prefix() {
        let v4 = WireGuardConf {
            addresses: vec!["10.0.0.2".into()],
            ..WireGuardConf::default()
        };
        let v6 = WireGuardConf {
            addresses: vec!["fd00::2".into()],
            ..WireGuardConf::default()
        };

        assert_eq!(v4.primary_address().as_deref(), Some("10.0.0.2/32"));
        assert_eq!(v6.primary_address().as_deref(), Some("fd00::2/128"));
    }

    #[test]
    fn no_address_means_no_primary_address() {
        assert_eq!(WireGuardConf::default().primary_address(), None);
    }

    #[test]
    fn dns_search_domains_are_not_servers() {
        let conf = WireGuardConf {
            dns: vec![
                "10.64.0.1".into(),
                "corp.example.com".into(),
                "fd00::53".into(),
            ],
            ..WireGuardConf::default()
        };

        assert_eq!(conf.dns_servers(), ["10.64.0.1", "fd00::53"]);
    }

    #[test]
    fn line_without_an_equals_sign_is_rejected() {
        let reason = invalid_reason(&interface("Address 10.0.0.2/24\n"));

        assert_eq!(reason, "line 3: expected `Key = Value`");
    }

    #[test]
    fn setting_before_a_section_is_rejected() {
        let reason = invalid_reason(&format!("PrivateKey = {PRIVATE_KEY}\n"));

        assert_eq!(
            reason,
            "line 1: setting before any [Interface] or [Peer] section"
        );
    }

    #[test]
    fn unknown_section_is_rejected() {
        let reason = invalid_reason(&interface("[Server]\nPort = 1\n"));

        assert_eq!(reason, "line 3: unknown section [Server]");
    }

    #[test]
    fn non_numeric_mtu_is_rejected() {
        let reason = invalid_reason(&interface("MTU = big\n"));

        assert_eq!(reason, "line 3: MTU is not a number: 'big'");
    }

    #[test]
    fn non_numeric_keepalive_is_rejected() {
        let reason = invalid_reason(&interface(&format!(
            "[Peer]\nPublicKey = {PUBLIC_KEY}\nPersistentKeepalive = soon\n"
        )));

        assert_eq!(
            reason,
            "line 5: PersistentKeepalive is not a number: 'soon'"
        );
    }

    #[test]
    fn peer_without_a_public_key_is_rejected() {
        let message = peer_error(&interface(
            "[Peer]\nEndpoint = host:51820\nAllowedIPs = 0.0.0.0/0\n",
        ));

        assert_eq!(message, "[Peer] at line 3 is missing PublicKey");
    }

    #[test]
    fn peer_without_an_endpoint_is_rejected() {
        let message = peer_error(&interface(&format!(
            "[Peer]\nPublicKey = {PUBLIC_KEY}\nAllowedIPs = 0.0.0.0/0\n"
        )));

        assert_eq!(message, "[Peer] at line 3 is missing Endpoint");
    }

    #[test]
    fn peer_without_allowed_ips_is_rejected() {
        let message = peer_error(&interface(&format!(
            "[Peer]\nPublicKey = {PUBLIC_KEY}\nEndpoint = host:51820\n[Peer]\n"
        )));

        assert_eq!(message, "[Peer] at line 3 is missing AllowedIPs");
    }
}
