//! Real-time monitoring of network and device changes.
//!
//! This module provides functions for monitoring network state changes,
//! device state changes, and retrieving current connection information.

pub(crate) mod bluetooth;
pub(crate) mod device;
pub(crate) mod events;
pub(crate) mod info;
pub(crate) mod network;
pub(crate) mod settings;
pub(crate) mod transport;
pub(crate) mod wifi;
