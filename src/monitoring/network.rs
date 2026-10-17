//! Real-time network monitoring using D-Bus signals.
//!
//! Provides functionality to monitor access point changes (additions/removals)
//! and signal strength changes in real-time without needing to poll. This
//! enables live UI updates.

use futures::stream::{Stream, StreamExt};
use log::{debug, trace, warn};
use std::collections::HashSet;
use std::pin::Pin;
use tokio::select;
use tokio::sync::{oneshot, watch};
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

#[derive(Debug, PartialEq, Eq)]
enum NetworkChangeAction {
    Notify,
    AccessPointAdded {
        path: OwnedObjectPath,
        newly_monitored: bool,
    },
    DeviceAdded(OwnedObjectPath),
}

fn apply_network_change(
    change: NetworkChange,
    monitored_access_points: &mut HashSet<String>,
) -> NetworkChangeAction {
    match change {
        NetworkChange::Added(path) => {
            let newly_monitored = monitored_access_points.insert(path.to_string());
            NetworkChangeAction::AccessPointAdded {
                path,
                newly_monitored,
            }
        }
        NetworkChange::Removed(path) => {
            monitored_access_points.remove(path.as_str());
            NetworkChangeAction::Notify
        }
        NetworkChange::SignalStrengthChanged => NetworkChangeAction::Notify,
        NetworkChange::DeviceAdded(path) => NetworkChangeAction::DeviceAdded(path),
    }
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
    ready_tx: oneshot::Sender<Result<()>>,
) -> Result<()>
where
    F: Fn() + Send + 'static,
{
    let (streams, mut monitored_access_points) = match initial_network_change_streams(conn).await {
        Ok(setup) => setup,
        Err(error) => {
            let _ = ready_tx.send(Err(error));
            return Ok(());
        }
    };

    debug!(
        "Monitoring {} signal streams for network changes",
        streams.len()
    );

    if ready_tx.send(Ok(())).is_err() {
        return Ok(());
    }

    // Merge all streams and listen for any signal
    let mut merged = futures::stream::select_all(streams);

    loop {
        select! {
            _ = shutdown.changed() => {
                debug!("Network monitoring shutdown requested");
                return Ok(());
            }
            signal = merged.next() => {
                match signal.map(|change| {
                    apply_network_change(change, &mut monitored_access_points)
                }) {
                    Some(NetworkChangeAction::AccessPointAdded {
                        path,
                        newly_monitored,
                    }) => {
                        if newly_monitored {
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
                    Some(NetworkChangeAction::Notify) => callback(),
                    Some(NetworkChangeAction::DeviceAdded(dev_path)) => {
                        if let Err(err) = subscribe_wifi_device(
                            conn,
                            &dev_path,
                            &mut merged,
                            &mut monitored_access_points,
                        )
                        .await
                        {
                            trace!("Hotplugged device {dev_path} is not Wi-Fi or failed: {err}");
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

async fn initial_network_change_streams(
    conn: &Connection,
) -> Result<(Vec<NetworkChangeStream>, HashSet<String>)> {
    let nm = NMProxy::new(conn).await?;
    let devices = nm.get_devices().await?;
    let mut streams: Vec<NetworkChangeStream> = Vec::new();
    let mut monitored_access_points = HashSet::new();

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

        trace!("Subscribed to network change signals on device: {dev_path}");
    }

    let device_added_stream = nm.receive_device_added().await?;
    streams.push(Box::pin(device_added_stream.map(|signal| {
        signal.args().map_or_else(
            |err| {
                trace!("Failed to parse DeviceAdded signal: {err}");
                NetworkChange::SignalStrengthChanged
            },
            |args| NetworkChange::DeviceAdded(args.device().clone()),
        )
    })));

    if streams.len() == 1 {
        warn!("No Wi-Fi devices found to monitor (listening for hotplug)");
    }

    Ok((streams, monitored_access_points))
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
                trace!("Failed to parse AccessPointAdded signal: {err}");
                NetworkChange::SignalStrengthChanged
            },
            |args| NetworkChange::Added(args.path().clone()),
        )
    })));
    merged.push(Box::pin(removed_stream.map(|signal| {
        signal.args().map_or_else(
            |err| {
                trace!("Failed to parse AccessPointRemoved signal: {err}");
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
                Err(err) => trace!(
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
        trace!("Access point signal strength changed: {ap_path}");
        NetworkChange::SignalStrengthChanged
    });

    Ok(Box::pin(stream))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn path(value: &str) -> OwnedObjectPath {
        OwnedObjectPath::try_from(value).expect("valid object path")
    }

    #[test]
    fn access_point_tracking_distinguishes_new_duplicate_and_removed_paths() {
        let access_point = path("/org/freedesktop/NetworkManager/AccessPoint/7");
        let mut monitored = HashSet::new();

        assert_eq!(
            apply_network_change(NetworkChange::Added(access_point.clone()), &mut monitored),
            NetworkChangeAction::AccessPointAdded {
                path: access_point.clone(),
                newly_monitored: true,
            }
        );
        assert_eq!(monitored, HashSet::from([access_point.to_string()]));

        assert_eq!(
            apply_network_change(NetworkChange::Added(access_point.clone()), &mut monitored),
            NetworkChangeAction::AccessPointAdded {
                path: access_point.clone(),
                newly_monitored: false,
            }
        );
        assert_eq!(monitored.len(), 1);

        assert_eq!(
            apply_network_change(NetworkChange::Removed(access_point), &mut monitored),
            NetworkChangeAction::Notify
        );
        assert!(monitored.is_empty());
    }

    #[test]
    fn non_membership_changes_preserve_tracking_and_action() {
        let existing = path("/org/freedesktop/NetworkManager/AccessPoint/3");
        let device = path("/org/freedesktop/NetworkManager/Devices/2");
        let mut monitored = HashSet::from([existing.to_string()]);

        assert_eq!(
            apply_network_change(NetworkChange::SignalStrengthChanged, &mut monitored),
            NetworkChangeAction::Notify
        );
        assert_eq!(
            apply_network_change(NetworkChange::DeviceAdded(device.clone()), &mut monitored),
            NetworkChangeAction::DeviceAdded(device)
        );
        assert_eq!(monitored, HashSet::from([existing.to_string()]));
    }
}
