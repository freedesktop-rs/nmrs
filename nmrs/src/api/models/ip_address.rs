//! Typed IP addresses and routes decoded from saved connection profiles.

use std::{
    fmt::{self, Display},
    net::{AddrParseError, Ipv4Addr, Ipv6Addr},
    num::ParseIntError,
    str::FromStr,
};

use thiserror::Error;

/// An IP address with its prefix length, such as `192.0.2.10/24`.
///
/// `A` is [`Ipv4Addr`] or [`Ipv6Addr`], so an `IpAddress<Ipv4Addr>` can never
/// carry an IPv6 address. The `address/prefix` form parses with [`FromStr`]:
///
/// ```rust
/// use std::net::Ipv4Addr;
/// use nmrs::models::IpAddress;
///
/// let addr: IpAddress<Ipv4Addr> = "192.0.2.10/24".parse().unwrap();
/// assert_eq!(addr.address, Ipv4Addr::new(192, 0, 2, 10));
/// assert_eq!(addr.prefix, 24);
/// assert_eq!(addr.to_string(), "192.0.2.10/24");
/// ```
#[non_exhaustive]
#[derive(Copy, Clone, Eq, PartialEq)]
pub struct IpAddress<A> {
    /// The address itself.
    pub address: A,
    /// Prefix length in bits: `0..=32` for IPv4, `0..=128` for IPv6.
    pub prefix: u8,
}

impl<A> IpAddress<A> {
    /// Creates an address with the given prefix length.
    pub fn new(address: A, prefix: u8) -> Self {
        Self { address, prefix }
    }
}

impl<A> Display for IpAddress<A>
where
    A: Display,
{
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}/{}", self.address, self.prefix)
    }
}

impl<A> fmt::Debug for IpAddress<A>
where
    A: fmt::Debug,
{
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{:?}/{}", self.address, self.prefix)
    }
}

impl FromStr for IpAddress<Ipv4Addr> {
    type Err = IpAddressParseError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let (address, prefix) = s.rsplit_once('/').ok_or(IpAddressParseError::Split)?;
        let address = address.parse()?;
        let prefix = prefix.parse()?;
        Ok(Self { address, prefix })
    }
}

impl FromStr for IpAddress<Ipv6Addr> {
    type Err = IpAddressParseError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let (address, prefix) = s.rsplit_once('/').ok_or(IpAddressParseError::Split)?;
        let address = address.parse()?;
        let prefix = prefix.parse()?;
        Ok(Self { address, prefix })
    }
}

impl From<IpAddress<Ipv4Addr>> for Ipv4Addr {
    fn from(value: IpAddress<Ipv4Addr>) -> Self {
        value.address
    }
}

impl From<IpAddress<Ipv6Addr>> for Ipv6Addr {
    fn from(value: IpAddress<Ipv6Addr>) -> Self {
        value.address
    }
}

/// A static route from a profile's `route-data`.
///
/// `dest` is the destination network with its prefix length. `next_hop` is
/// `None` for routes reachable directly on the link, and `metric` is `None`
/// when the profile leaves the metric to NetworkManager.
#[non_exhaustive]
#[derive(Copy, Clone, Eq, PartialEq, Debug)]
pub struct IpRoute<A> {
    /// Destination network and prefix length.
    pub dest: IpAddress<A>,
    /// Next-hop gateway, if the route has one.
    pub next_hop: Option<A>,
    /// Route metric, if the profile sets one explicitly.
    pub metric: Option<u32>,
}

impl<A> IpRoute<A> {
    /// Creates a route to `dest` with no next hop and no explicit metric.
    pub fn new(dest: IpAddress<A>) -> Self {
        Self {
            dest,
            next_hop: None,
            metric: None,
        }
    }
}

/// Why a string could not be parsed as an [`IpAddress`].
#[non_exhaustive]
#[derive(Debug, Clone, Error)]
pub enum IpAddressParseError {
    /// The part before the `/` is not an address of the requested family.
    #[error("address parsing failed: {0}")]
    Addr(#[from] AddrParseError),
    /// The part after the `/` is not a prefix length.
    #[error("prefix parsing failed: {0}")]
    Prefix(#[from] ParseIntError),
    /// The string has no `/` separating address and prefix.
    #[error("could not split into address and prefix")]
    Split,
}
