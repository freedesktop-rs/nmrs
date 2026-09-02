use std::{
    fmt::{self, Display},
    net::{AddrParseError, Ipv4Addr, Ipv6Addr},
    num::ParseIntError,
    str::FromStr,
};

use thiserror::Error;

use crate::ConnectionError;

/// An IP address with its prefix.
#[derive(Copy, Clone, Eq, PartialEq)]
pub struct IpAddress<A> {
    pub address: A,
    pub prefix: u8,
}

impl<A> IpAddress<A> {
    /// Create the IP address from the address and prefix.
    pub fn new(address: A, prefix: u8) -> Self {
        Self { address, prefix }
    }
}

impl<A> Display for IpAddress<A>
where
    A: Display,
{
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
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

#[derive(Debug, Clone, Error)]
pub enum IpAddressParseError {
    #[error("address parsing failed: {0}")]
    Addr(#[from] AddrParseError),
    #[error("prefix parsing failed: {0}")]
    Prefix(#[from] ParseIntError),
    #[error("could not split into address and prefix")]
    Split,
}

impl From<IpAddressParseError> for ConnectionError {
    fn from(value: IpAddressParseError) -> Self {
        Self::AddressParse(value)
    }
}
