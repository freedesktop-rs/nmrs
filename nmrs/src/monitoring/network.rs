//! Real-time network monitoring using D-Bus signals.
//!
//! Provides functionality to monitor access point changes (additions/removals)
//! and signal strength changes in real-time without needing to poll. This
//! enables live UI updates.

use futures::stream::{Stream, StreamExt};
use log::{debug, warn};
use std::collections::HashSet;
use std::pin::Pin;
use tokio::select;
use tokio::sync::watch;
use zbus::Connection;
use zvariant::OwnedObjectPath;

use crate::Result;
use crate::api::models::ConnectionError;
use crate::dbus::{NMAccessPointProxy, NMDeviceProxy, NMProxy, NMWirelessProxy};
use crate::types::constants::device_type;

type NetworkChangeStream = Pin<Box<dyn Stream<Item = NetworkChange> + Send>>;

enum NetworkChange {
    Added(OwnedObjectPath),
    Removed(OwnedObjectPath),
    SignalStrengthChanged,
    DeviceAdded(OwnedObjectPath),
}

/// Monitors access point changes on all Wi-Fi devices.
///
/// Subscribes to `AccessPointAdded` and `AccessPointRemoved` signals on all
/// wireless devices, plus `Strength` property changes on visible access points.
/// When any signal is received, invokes the callback to notify the caller that
/// the network list or signal data has changed.
///
/// This function runs indefinitely until an error occurs or the connection
/// is lost. Run it in a background task.
///
/// # Example
///
/// ```ignore
/// let nm = NetworkManager::new().await?;
/// nm.monitor_network_changes(|| {
///     println!("Network list changed, refresh UI!");
/// }).await?;
/// ```
pub async fn monitor_network_changes<F>(
    conn: &Connection,
    mut shutdown: watch::Receiver<()>,
    callback: F,
) -> Result<()>
where
    F: Fn() + Send + 'static,
{
    let nm = NMProxy::new(conn).await?;
    let devices = nm.get_devices().await?;

    // Use dynamic dispatch to handle different signal stream types
    let mut streams: Vec<NetworkChangeStream> = Vec::new();
    let mut monitored_access_points = HashSet::new();

    // Subscribe to signals on all Wi-Fi devices
    for dev_path in devices {
        let dev = NMDeviceProxy::builder(conn)
            .path(dev_path.clone())?
            .build()
            .await?;

        if dev.device_type().await? != device_type::WIFI {
            continue;
        }

        let wifi = NMWirelessProxy::builder(conn)
            .path(dev_path.clone())?
            .build()
            .await?;

        let added_stream = wifi.receive_access_point_added().await?;
        let removed_stream = wifi.receive_access_point_removed().await?;

        streams.push(Box::pin(added_stream.map(|signal| {
            signal.args().map_or_else(
                |err| {
                    debug!("Failed to parse AccessPointAdded signal: {err}");
                    NetworkChange::SignalStrengthChanged
                },
                |args| NetworkChange::Added(args.path().clone()),
            )
        })));
        streams.push(Box::pin(removed_stream.map(|signal| {
            signal.args().map_or_else(
                |err| {
                    debug!("Failed to parse AccessPointRemoved signal: {err}");
                    NetworkChange::SignalStrengthChanged
                },
                |args| NetworkChange::Removed(args.path().clone()),
            )
        })));

        match wifi.access_points().await {
            Ok(ap_paths) => {
                for ap_path in ap_paths {
                    if !monitored_access_points.insert(ap_path.to_string()) {
                        continue;
                    }

                    match access_point_strength_stream(conn, ap_path.clone()).await {
                        Ok(stream) => streams.push(stream),
                        Err(err) => debug!(
                            "Failed to monitor signal strength for access point {}: {}",
                            ap_path, err
                        ),
                    }
                }
            }
            Err(err) => debug!("Failed to list access points on device {dev_path}: {err}"),
        }

        debug!("Subscribed to network change signals on device: {dev_path}");
    }

    let device_added_stream = nm.receive_device_added().await?;
    streams.push(Box::pin(device_added_stream.map(|signal| {
        signal.args().map_or_else(
            |err| {
                debug!("Failed to parse DeviceAdded signal: {err}");
                NetworkChange::SignalStrengthChanged
            },
            |args| NetworkChange::DeviceAdded(args.device().clone()),
        )
    })));

    if streams.len() == 1 {
        warn!("No Wi-Fi devices found to monitor (listening for hotplug)");
    }

    debug!(
        "Monitoring {} signal streams for network changes",
        streams.len()
    );

    // Merge all streams and listen for any signal
    let mut merged = futures::stream::select_all(streams);

    loop {
        select! {
            _ = shutdown.changed() => {
                debug!("Network monitoring shutdown requested");
                return Ok(());
            }
            signal = merged.next() => {
                match signal {
                    Some(NetworkChange::Added(path)) => {
                        if monitored_access_points.insert(path.to_string()) {
                            match access_point_strength_stream(conn, path.clone()).await {
                                Ok(stream) => merged.push(stream),
                                Err(err) => debug!(
                                    "Failed to monitor signal strength for access point {}: {}",
                                    path, err
                                ),
                            }
                        }
                        callback();
                    }
                    Some(NetworkChange::Removed(path)) => {
                        monitored_access_points.remove(path.as_str());
                        callback();
                    }
                    Some(NetworkChange::SignalStrengthChanged) => callback(),
                    Some(NetworkChange::DeviceAdded(dev_path)) => {
                        if let Err(err) = subscribe_wifi_device(
                            conn,
                            &dev_path,
                            &mut merged,
                            &mut monitored_access_points,
                        )
                        .await
                        {
                            debug!("Hotplugged device {dev_path} is not Wi-Fi or failed: {err}");
                        } else {
                            debug!("Subscribed to hotplugged Wi-Fi device: {dev_path}");
                            callback();
                        }
                    }
                    None => return Err(ConnectionError::Stuck(
                        "network monitoring stream ended unexpectedly".into(),
                    )),
                }
            }
        }
    }
}

async fn subscribe_wifi_device(
    conn: &Connection,
    dev_path: &OwnedObjectPath,
    merged: &mut futures::stream::SelectAll<NetworkChangeStream>,
    monitored_access_points: &mut HashSet<String>,
) -> Result<()> {
    let dev = NMDeviceProxy::builder(conn)
        .path(dev_path.clone())?
        .build()
        .await?;

    if dev.device_type().await? != device_type::WIFI {
        return Err(ConnectionError::Stuck("not a Wi-Fi device".into()));
    }

    let wifi = NMWirelessProxy::builder(conn)
        .path(dev_path.clone())?
        .build()
        .await?;

    let added_stream = wifi.receive_access_point_added().await?;
    let removed_stream = wifi.receive_access_point_removed().await?;

    merged.push(Box::pin(added_stream.map(|signal| {
        signal.args().map_or_else(
            |err| {
                debug!("Failed to parse AccessPointAdded signal: {err}");
                NetworkChange::SignalStrengthChanged
            },
            |args| NetworkChange::Added(args.path().clone()),
        )
    })));
    merged.push(Box::pin(removed_stream.map(|signal| {
        signal.args().map_or_else(
            |err| {
                debug!("Failed to parse AccessPointRemoved signal: {err}");
                NetworkChange::SignalStrengthChanged
            },
            |args| NetworkChange::Removed(args.path().clone()),
        )
    })));

    if let Ok(ap_paths) = wifi.access_points().await {
        for ap_path in ap_paths {
            if !monitored_access_points.insert(ap_path.to_string()) {
                continue;
            }
            match access_point_strength_stream(conn, ap_path.clone()).await {
                Ok(stream) => merged.push(stream),
                Err(err) => debug!(
                    "Failed to monitor signal strength for access point {}: {}",
                    ap_path, err
                ),
            }
        }
    }

    Ok(())
}

async fn access_point_strength_stream(
    conn: &Connection,
    ap_path: OwnedObjectPath,
) -> Result<NetworkChangeStream> {
    let ap = NMAccessPointProxy::builder(conn)
        .path(ap_path.clone())?
        .build()
        .await?;

    let stream = ap.receive_strength_changed().await.skip(1).map(move |_| {
        debug!("Access point signal strength changed: {ap_path}");
        NetworkChange::SignalStrengthChanged
    });

    Ok(Box::pin(stream))
}
