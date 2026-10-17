//! D-Bus proxy interfaces for NetworkManager.
//!
//! This module contains low-level D-Bus proxy definitions for communicating
//! with NetworkManager over the system bus.

mod access_point;
mod active_connection;
pub(crate) mod agent_manager;
mod bluetooth;
mod bluez_adapter;
mod device;
mod main_nm;
mod settings;
mod settings_connection;
mod wired;
mod wireless;

pub(crate) use access_point::NMAccessPointProxy;
pub(crate) use active_connection::NMActiveConnectionProxy;
pub(crate) use agent_manager::AgentManagerProxy;
pub(crate) use bluetooth::{BluezDeviceExtProxy, NMBluetoothProxy};
pub(crate) use bluez_adapter::BluezAdapterProxy;
pub(crate) use device::NMDeviceProxy;
pub(crate) use main_nm::NMProxy;
pub(crate) use settings::NMSettingsProxy;
pub(crate) use settings_connection::NMSettingsConnectionProxy;
pub(crate) use wired::NMWiredProxy;
pub(crate) use wireless::NMWirelessProxy;
