//! Core internal logic for connection management.
//!
//! This module contains the internal implementation details for managing
//! network connections, devices, scanning, and state monitoring.

pub(crate) mod active_connection;
pub(crate) mod airplane;
pub(crate) mod bluetooth;
pub(crate) mod connection;
pub(crate) mod connection_settings;
pub(crate) mod connectivity;
pub(crate) mod custom_connection;
pub(crate) mod device;
pub(crate) mod ovpn_parser;
pub(crate) mod rfkill;
pub(crate) mod saved_connection;
pub(crate) mod scan;
pub(crate) mod state_wait;
pub(crate) mod vpn;
pub(crate) mod wifi_device;
