pub(crate) mod access_point;
mod active_connection;
mod bluetooth;
mod config;
mod connection_state;
mod connectivity;
mod device;
mod error;
mod ip_address;
mod monitor;
mod network_event;
mod openvpn;
mod radio;
mod saved_connection;
pub(crate) mod snapshot;
mod state_reason;
mod vlan;
mod vpn;
mod wifi;
mod wireguard;

use std::fmt;

pub(crate) struct Redacted;

impl fmt::Debug for Redacted {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("[REDACTED]")
    }
}

pub(crate) fn redact_option<T>(value: &Option<T>) -> Option<Redacted> {
    value.as_ref().map(|_| Redacted)
}

#[cfg(test)]
#[path = "tests.rs"]
mod tests;

pub use access_point::*;
pub use active_connection::*;
pub use bluetooth::*;
pub use config::*;
pub use connection_state::*;
pub use connectivity::*;
pub use device::*;
pub use error::*;
pub use ip_address::*;
pub use monitor::*;
pub use network_event::*;
pub use openvpn::*;
pub use radio::*;
pub use saved_connection::*;
pub use snapshot::{AppletNetworkSummary, NetworkSnapshot};
pub use state_reason::*;
pub use vlan::*;
pub use vpn::*;
pub use wifi::*;
pub use wireguard::*;
