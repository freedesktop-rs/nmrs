//! Typed, round-trippable NetworkManager saved-connection settings.
//!
//! This module mirrors NetworkManager's `a{sa{sv}}` settings dictionary with
//! strongly typed Rust structs. Each `*Settings` type can be parsed from a raw
//! profile via [`TypedSettings::from_raw_dict`],
//! mutated in place, and serialized back into a merge [`crate::SettingsPatch`]
//! suitable for [`crate::NetworkManager::update_saved_connection`].
//!
//! Unlike the read-only decode in [`crate::models::SettingsSummary`], these
//! structs are *editable* and lossy-free: every section key NetworkManager
//! documents is represented, so a profile can be loaded, changed, and written
//! back without dropping unknown fields handled elsewhere.
//!
//! # Examples
//!
//! ```no_run
//! use nmrs::NetworkManager;
//! use nmrs::settings::WiFiSettings;
//! use nmrs::settings::common::IpMethod;
//! use std::net::Ipv4Addr;
//!
//! # async fn run() -> nmrs::Result<()> {
//! let nm = NetworkManager::new().await?;
//! let saved = nm.list_saved_connections().await?
//!     .into_iter().find(|s| s.id == "HomeWiFi").unwrap();
//!
//! let raw = nm.get_saved_connection_raw(&saved.uuid).await?;
//! let mut wifi = WiFiSettings::from_raw_dict(raw)?;
//! wifi.connection.autoconnect = false;
//! wifi.ipv4.method = IpMethod::Manual;
//! nm.update_saved_connection(&saved.uuid, wifi.to_patch()?).await?;
//! # Ok(())
//! # }
//! ```

#[macro_use]
mod macros;

#[macro_use]
mod profile;

/// Shared connection, IP, proxy, and common NetworkManager settings sections.
pub mod common;
/// Wi-Fi and wireless-security connection settings.
pub mod wifi;

/// Bluetooth connection settings.
pub mod bluetooth;
/// Bond connection settings.
pub mod bond;
/// Bridge and bridge-VLAN settings.
pub mod bridge;
/// Ethernet connection settings.
pub mod ethernet;
/// GSM and CDMA mobile-broadband settings.
pub mod mobile;
/// VLAN connection settings.
pub mod vlan;
/// Generic VPN and OpenVPN settings.
pub mod vpn;
/// Native WireGuard connection settings.
pub mod wireguard;

/// Errors produced while parsing or serializing typed settings.
pub mod error;

use std::collections::HashMap;

use zbus::zvariant::OwnedValue;

use crate::{ConnectionError, SettingsPatch};

pub use error::SettingsError;

pub use bluetooth::{BluetoothSection, BluetoothSettings, BluetoothType};
pub use bond::{BondSection, BondSettings};
pub use bridge::{BridgeSection, BridgeSettings};
pub use common::*;
pub use mobile::{CdmaSection, CdmaSettings, GsmSection, GsmSettings};
pub use vlan::{VlanProtocol, VlanSection, VlanSettings};
pub use vpn::{OpenVpnOptions, OpenVpnSettings, VpnSection, VpnSettings};
pub use wifi::{WiFiSettings, WirelessSection, WirelessSecuritySection};
pub use wireguard::{WireGuardPeer, WireGuardSection, WireGuardSettings};

pub use ethernet::{EthernetSection, EthernetSettings};

/// A NetworkManager saved-connection profile that can be parsed from, and
/// serialized back into NetworkManager's raw `a{sa{sv}}` dictionary.
///
/// Every built-in `*Settings` type implements this trait for common parsing and
/// patch-building behavior.
///
/// The companion [`TypedSettings::to_patch`] produces a merge overlay: keys and
/// sections you do not touch are preserved by NetworkManager when the patch is
/// applied.
pub trait TypedSettings: Sized {
    /// Parse a raw `GetSettings` dictionary into this typed representation.
    ///
    /// Returns [`SettingsParse`](crate::ConnectionError::SettingsParse) when a
    /// required section or key is missing, or a stored value does not match the
    /// expected type.
    fn from_raw_dict(
        settings: HashMap<String, HashMap<String, OwnedValue>>,
    ) -> Result<Self, SettingsError>;

    /// Serialize this profile into a merge [`SettingsPatch`].
    fn to_patch(&self) -> Result<SettingsPatch, ConnectionError>;
}

macro_rules! impl_typed_settings {
    ($($t:ty),* $(,)?) => {
        $(
            impl TypedSettings for $t {
                fn from_raw_dict(
                    settings: HashMap<String, HashMap<String, OwnedValue>>,
                ) -> Result<Self, SettingsError> {
                    <$t>::from_raw_dict(settings)
                }

                fn to_patch(&self) -> Result<SettingsPatch, ConnectionError> {
                    <$t>::to_patch(self)
                }
            }
        )*
    };
}

impl_typed_settings!(
    WiFiSettings,
    EthernetSettings,
    VlanSettings,
    BluetoothSettings,
    BondSettings,
    BridgeSettings,
    GsmSettings,
    CdmaSettings,
    VpnSettings,
    WireGuardSettings,
);
