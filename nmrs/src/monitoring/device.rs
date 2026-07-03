//! Real-time device state monitoring using D-Bus signals.
//!
//! Provides functionality to monitor device state changes (e.g., ethernet cable
//! plugged in/out, device activation/deactivation) in real-time without needing
//! to poll. This enables live UI updates for both wired and wireless devices.

use futures::stream::{Stream, StreamExt};
use log::debug;
use std::pin::Pin;
use tokio::select;
use tokio::sync::watch;
use zbus::Connection;

use crate::Result;
use crate::api::models::ConnectionError;
use crate::dbus::{NMDeviceProxy, NMProxy};

/// Monitors device state changes on all network devices.
///
/// Subscribes to `StateChanged` signals on all network devices. When any signal
/// is received (device activated, disconnected, cable plugged in, etc.), invokes
/// the callback to notify the caller that device states have changed.
///
/// This function runs indefinitely until an error occurs or the connection
/// is lost. Run it in a background task.
///
/// # Example
///
/// ```ignore
/// let nm = NetworkManager::new().await?;
/// nm.monitor_device_changes(|| {
///     println!("Device state changed, refresh UI!");
/// }).await?;
/// ```
pub async fn monitor_device_changes<F>(
    conn: &Connection,
    mut shutdown: watch::Receiver<()>,
    callback: F,
) -> Result<()>
where
    F: Fn() + Send + 'static,
{
    let nm = NMProxy::new(conn).await?;

    // Use dynamic dispatch to handle different signal stream types
    let mut streams: Vec<Pin<Box<dyn Stream<Item = _> + Send>>> = Vec::new();

    // Subscribe to DeviceAdded and DeviceRemoved signals from main NetworkManager
    // This is more reliable than subscribing to individual devices
    let device_added_stream = nm.receive_device_added().await?;
    let device_removed_stream = nm.receive_device_removed().await?;
    let state_changed_stream = nm.receive_state_changed().await?;

    streams.push(Box::pin(device_added_stream.map(|_| ())));
    streams.push(Box::pin(device_removed_stream.map(|_| ())));
    streams.push(Box::pin(state_changed_stream.map(|_| ())));

    debug!("Subscribed to NetworkManager device signals");

    // Also subscribe to individual device state changes for existing devices
    let devices = nm.get_devices().await?;
    for dev_path in devices {
        if let Ok(dev) = NMDeviceProxy::builder(conn)
            .path(dev_path.clone())?
            .build()
            .await
            && let Ok(state_stream) = dev.receive_device_state_changed().await
        {
            streams.push(Box::pin(state_stream.map(|_| ())));
            debug!("Subscribed to state change signals on device: {dev_path}");
        }
    }

    debug!(
        "Monitoring {} signal streams for device changes",
        streams.len()
    );

    // Merge all streams and listen for any signal
    let mut merged = futures::stream::select_all(streams);

    loop {
        select! {
            _ = shutdown.changed() => {
                debug!("Device monitoring shutdown requested");
                return Ok(());
            }
            signal = merged.next() => {
                match signal {
                    Some(_) => callback(),
                    None => return Err(ConnectionError::Stuck(
                        "device monitoring stream ended unexpectedly".into(),
                    )),
                }
            }
        }
    }
}
