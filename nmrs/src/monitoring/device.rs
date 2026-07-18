//! Real-time device state monitoring using D-Bus signals.
//!
//! Provides functionality to monitor device state changes (e.g., ethernet cable
//! plugged in/out, device activation/deactivation) in real-time without needing
//! to poll. This enables live UI updates for both wired and wireless devices.

use futures::stream::{Stream, StreamExt};
use log::{debug, trace};
use std::pin::Pin;
use tokio::select;
use tokio::sync::{oneshot, watch};
use zbus::Connection;

use crate::Result;
use crate::api::models::ConnectionError;
use crate::dbus::{NMDeviceProxy, NMProxy};

type DeviceChangeStream = Pin<Box<dyn Stream<Item = ()> + Send>>;

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
    shutdown: watch::Receiver<()>,
    callback: F,
    ready_tx: oneshot::Sender<Result<()>>,
) -> Result<()>
where
    F: Fn() + Send + 'static,
{
    let setup: Result<Vec<DeviceChangeStream>> = async {
        let nm = NMProxy::new(conn).await?;

        // Use dynamic dispatch to handle different signal stream types.
        let mut streams: Vec<DeviceChangeStream> = Vec::new();

        // Main-manager signals cover hotplug and global state changes.
        let device_added_stream = nm.receive_device_added().await?;
        let device_removed_stream = nm.receive_device_removed().await?;
        let state_changed_stream = nm.receive_state_changed().await?;

        streams.push(Box::pin(device_added_stream.map(|_| ())));
        streams.push(Box::pin(device_removed_stream.map(|_| ())));
        streams.push(Box::pin(state_changed_stream.map(|_| ())));

        trace!("Subscribed to NetworkManager device signals");

        // Existing devices also expose more specific state transitions.
        for dev_path in nm.get_devices().await? {
            if let Ok(dev) = NMDeviceProxy::builder(conn)
                .path(dev_path.clone())?
                .build()
                .await
                && let Ok(state_stream) = dev.receive_device_state_changed().await
            {
                streams.push(Box::pin(state_stream.map(|_| ())));
                trace!("Subscribed to state change signals on device: {dev_path}");
            }
        }

        Ok(streams)
    }
    .await;

    let streams = match setup {
        Ok(streams) => streams,
        Err(error) => {
            let _ = ready_tx.send(Err(error));
            return Ok(());
        }
    };

    debug!(
        "Monitoring {} signal streams for device changes",
        streams.len()
    );

    if ready_tx.send(Ok(())).is_err() {
        return Ok(());
    }

    run_device_change_streams(shutdown, streams, callback).await
}

async fn run_device_change_streams<F>(
    mut shutdown: watch::Receiver<()>,
    streams: Vec<DeviceChangeStream>,
    callback: F,
) -> Result<()>
where
    F: Fn() + Send + 'static,
{
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

#[cfg(test)]
mod tests {
    use std::sync::Arc;
    use std::sync::atomic::{AtomicUsize, Ordering};

    use futures::stream;

    use super::*;

    #[tokio::test]
    async fn signal_invokes_callback_before_ended_stream_is_reported() {
        let (_shutdown_tx, shutdown_rx) = watch::channel(());
        let calls = Arc::new(AtomicUsize::new(0));
        let callback_calls = Arc::clone(&calls);
        let streams: Vec<DeviceChangeStream> = vec![Box::pin(stream::iter([()]))];

        let result = run_device_change_streams(shutdown_rx, streams, move || {
            callback_calls.fetch_add(1, Ordering::SeqCst);
        })
        .await;

        assert_eq!(calls.load(Ordering::SeqCst), 1);
        assert!(matches!(
            result,
            Err(ConnectionError::Stuck(message))
                if message == "device monitoring stream ended unexpectedly"
        ));
    }

    #[tokio::test]
    async fn shutdown_stops_monitor_without_invoking_callback() {
        let (shutdown_tx, shutdown_rx) = watch::channel(());
        let calls = Arc::new(AtomicUsize::new(0));
        let callback_calls = Arc::clone(&calls);
        let streams: Vec<DeviceChangeStream> = vec![Box::pin(stream::pending())];

        let monitor = run_device_change_streams(shutdown_rx, streams, move || {
            callback_calls.fetch_add(1, Ordering::SeqCst);
        });
        let request_shutdown = async move {
            tokio::task::yield_now().await;
            shutdown_tx.send(()).expect("monitor still listening");
        };

        let (result, ()) = tokio::join!(monitor, request_shutdown);
        assert!(result.is_ok());
        assert_eq!(calls.load(Ordering::SeqCst), 0);
    }

    #[tokio::test]
    async fn empty_signal_set_is_an_error() {
        let (_shutdown_tx, shutdown_rx) = watch::channel(());

        let result = run_device_change_streams(shutdown_rx, Vec::new(), || {}).await;

        assert!(matches!(
            result,
            Err(ConnectionError::Stuck(message))
                if message == "device monitoring stream ended unexpectedly"
        ));
    }
}
