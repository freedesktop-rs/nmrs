//! Unified stream-based NetworkManager event monitoring.

use std::pin::Pin;

use futures::channel::mpsc;
use futures::stream::{Stream, StreamExt};
use log::{trace, warn};
use zbus::Connection;
use zvariant::OwnedObjectPath;

use crate::Result;
use crate::api::models::{ConnectionError, NetworkEvent, NetworkEventStream, SettingsChange};
use crate::dbus::{NMAccessPointProxy, NMDeviceProxy, NMProxy, NMWirelessProxy};
use crate::monitoring::settings;
use crate::types::constants::device_type;

type InternalEventStream<'a> = Pin<Box<dyn Stream<Item = InternalEvent> + Send + 'a>>;

enum InternalEvent {
    Event(NetworkEvent),
    Error(ConnectionError),
    AccessPointAdded(OwnedObjectPath),
    AccessPointRemoved,
    DeviceAdded(OwnedObjectPath),
    DeviceRemoved,
}

/// Creates a unified refresh-oriented stream of NetworkManager events.
pub(crate) async fn network_events(conn: &Connection) -> Result<NetworkEventStream> {
    NMProxy::new(conn).await?;

    let (tx, rx) = mpsc::unbounded();
    let conn = conn.clone();

    tokio::spawn(async move {
        if let Err(err) = run_network_events(conn, tx.clone()).await {
            let _ = tx.unbounded_send(Err(err));
        }
    });

    Ok(Box::pin(rx))
}

async fn run_network_events(
    conn: Connection,
    tx: mpsc::UnboundedSender<Result<NetworkEvent>>,
) -> Result<()> {
    let nm = NMProxy::new(&conn).await?;
    let dbus = zbus::fdo::DBusProxy::new(&conn).await?;
    let mut streams = base_network_event_streams(&nm, &dbus).await?;

    match settings::settings_events(&conn).await {
        Ok(settings_stream) => {
            streams.push(Box::pin(settings_stream.map(|item| match item {
                Ok(change) => InternalEvent::Event(settings_change_event(change)),
                Err(err) => InternalEvent::Error(err),
            })));
        }
        Err(err) => warn!("failed to subscribe to settings events: {err}"),
    }

    for stream in device_state_streams(&conn, &nm).await? {
        streams.push(stream);
    }

    for stream in access_point_streams(&conn, &nm).await? {
        streams.push(stream);
    }

    let mut merged = futures::stream::select_all(streams);
    while let Some(internal) = merged.next().await {
        match internal {
            InternalEvent::Event(event) => {
                if !send_event(&tx, event) {
                    return Ok(());
                }
            }
            InternalEvent::Error(err) => {
                if !send_error(&tx, err) {
                    return Ok(());
                }
            }
            InternalEvent::AccessPointAdded(path) => {
                if !send_event(&tx, NetworkEvent::AccessPointsChanged) {
                    return Ok(());
                }
                match access_point_strength_stream(&conn, path.clone()).await {
                    Ok(stream) => merged.push(stream),
                    Err(err) => trace!("failed to monitor access point {path}: {err}"),
                }
            }
            InternalEvent::AccessPointRemoved => {
                if !send_event(&tx, NetworkEvent::AccessPointsChanged) {
                    return Ok(());
                }
            }
            InternalEvent::DeviceAdded(path) => {
                let event = device_changed_for_path(&conn, &path).await;
                if !send_event(&tx, event) {
                    return Ok(());
                }
                match device_state_stream(&conn, path.clone()).await {
                    Ok(stream) => merged.push(stream),
                    Err(err) => trace!("failed to monitor device {path}: {err}"),
                }
                match wireless_device_streams(&conn, path.clone()).await {
                    Ok(streams) => {
                        for stream in streams {
                            merged.push(stream);
                        }
                    }
                    Err(err) => trace!("failed to monitor wireless device {path}: {err}"),
                }
            }
            InternalEvent::DeviceRemoved => {
                if !send_event(&tx, device_change_event(None)) {
                    return Ok(());
                }
            }
        }
    }

    Err(ConnectionError::Stuck("network event stream ended".into()))
}

async fn base_network_event_streams<'a>(
    nm: &'a NMProxy<'a>,
    dbus: &'a zbus::fdo::DBusProxy<'a>,
) -> Result<Vec<InternalEventStream<'a>>> {
    let mut streams: Vec<InternalEventStream<'_>> = Vec::new();

    let device_added = nm.receive_device_added().await?;
    streams.push(Box::pin(device_added.map(|signal| {
        signal
            .args()
            .map_or(InternalEvent::Event(device_change_event(None)), |args| {
                InternalEvent::DeviceAdded(args.device().clone())
            })
    })));

    let device_removed = nm.receive_device_removed().await?;
    streams.push(Box::pin(
        device_removed.map(|_| InternalEvent::DeviceRemoved),
    ));

    let nm_state_changed = nm.receive_state_changed().await?;
    streams.push(Box::pin(
        nm_state_changed.map(|_| InternalEvent::Event(device_change_event(None))),
    ));

    streams.push(Box::pin(
        nm.receive_active_connections_changed()
            .await
            .skip(1)
            .map(|_| InternalEvent::Event(NetworkEvent::ActiveConnectionsChanged)),
    ));

    streams.push(Box::pin(
        nm.receive_wireless_enabled_changed()
            .await
            .skip(1)
            .map(|_| InternalEvent::Event(NetworkEvent::WirelessEnabledChanged)),
    ));

    streams.push(Box::pin(
        nm.receive_wireless_hardware_enabled_changed()
            .await
            .skip(1)
            .map(|_| InternalEvent::Event(NetworkEvent::WirelessEnabledChanged)),
    ));

    streams.push(Box::pin(
        nm.receive_connectivity_changed()
            .await
            .skip(1)
            .map(|_| InternalEvent::Event(NetworkEvent::ConnectivityChanged)),
    ));

    let name_owner_changed = dbus
        .receive_name_owner_changed_with_args(&[(0, "org.freedesktop.NetworkManager")])
        .await?;
    streams.push(Box::pin(name_owner_changed.map(|_| {
        InternalEvent::Event(NetworkEvent::NetworkManagerRestarted)
    })));

    Ok(streams)
}

async fn device_state_streams<'a>(
    conn: &'a Connection,
    nm: &'a NMProxy<'a>,
) -> Result<Vec<InternalEventStream<'a>>> {
    let mut streams: Vec<InternalEventStream<'_>> = Vec::new();

    for path in nm.get_devices().await? {
        match device_state_stream(conn, path.clone()).await {
            Ok(stream) => streams.push(stream),
            Err(err) => trace!("failed to monitor device {path}: {err}"),
        }
    }

    Ok(streams)
}

async fn device_state_stream<'a>(
    conn: &'a Connection,
    path: OwnedObjectPath,
) -> Result<InternalEventStream<'a>> {
    let device = NMDeviceProxy::builder(conn)
        .path(path.clone())?
        .build()
        .await?;
    let interface = device.interface().await.ok();
    let state_changed = device.receive_device_state_changed().await?;

    Ok(Box::pin(state_changed.map(move |_| {
        InternalEvent::Event(device_change_event(interface.clone()))
    })))
}

async fn device_changed_for_path(conn: &Connection, path: &OwnedObjectPath) -> NetworkEvent {
    let interface = async {
        let device = NMDeviceProxy::builder(conn)
            .path(path.clone())
            .ok()?
            .build()
            .await
            .ok()?;
        device.interface().await.ok()
    }
    .await;

    device_change_event(interface)
}

async fn access_point_streams<'a>(
    conn: &'a Connection,
    nm: &'a NMProxy<'a>,
) -> Result<Vec<InternalEventStream<'a>>> {
    let mut streams: Vec<InternalEventStream<'_>> = Vec::new();

    for device_path in nm.get_devices().await? {
        match wireless_device_streams(conn, device_path.clone()).await {
            Ok(device_streams) => streams.extend(device_streams),
            Err(err) => trace!("failed to monitor wireless device {device_path}: {err}"),
        }
    }

    Ok(streams)
}

async fn wireless_device_streams<'a>(
    conn: &'a Connection,
    device_path: OwnedObjectPath,
) -> Result<Vec<InternalEventStream<'a>>> {
    let mut streams: Vec<InternalEventStream<'_>> = Vec::new();
    let device = NMDeviceProxy::builder(conn)
        .path(device_path.clone())?
        .build()
        .await?;

    if device.device_type().await? != device_type::WIFI {
        return Ok(streams);
    }

    let wifi = NMWirelessProxy::builder(conn)
        .path(device_path.clone())?
        .build()
        .await?;

    let added = wifi.receive_access_point_added().await?;
    streams.push(Box::pin(added.map(|signal| {
        signal.args().map_or(
            InternalEvent::Event(NetworkEvent::AccessPointsChanged),
            |args| InternalEvent::AccessPointAdded(args.path().clone()),
        )
    })));

    let removed = wifi.receive_access_point_removed().await?;
    streams.push(Box::pin(removed.map(|signal| {
        signal.args().map_or(
            InternalEvent::Event(NetworkEvent::AccessPointsChanged),
            |_| InternalEvent::AccessPointRemoved,
        )
    })));

    match wifi.access_points().await {
        Ok(access_points) => {
            for access_point in access_points {
                match access_point_strength_stream(conn, access_point.clone()).await {
                    Ok(stream) => streams.push(stream),
                    Err(err) => trace!("failed to monitor access point {access_point}: {err}"),
                }
            }
        }
        Err(err) => trace!("failed to list access points on {device_path}: {err}"),
    }

    Ok(streams)
}

async fn access_point_strength_stream<'a>(
    conn: &'a Connection,
    path: OwnedObjectPath,
) -> Result<InternalEventStream<'a>> {
    let access_point = NMAccessPointProxy::builder(conn)
        .path(path.clone())?
        .build()
        .await?;

    Ok(Box::pin(
        access_point
            .receive_strength_changed()
            .await
            .skip(1)
            .map(|_| InternalEvent::Event(NetworkEvent::AccessPointsChanged)),
    ))
}

pub(crate) fn settings_change_event(change: SettingsChange) -> NetworkEvent {
    NetworkEvent::SettingsChanged(change)
}

pub(crate) fn device_change_event(interface: Option<String>) -> NetworkEvent {
    NetworkEvent::DeviceChanged { interface }
}

fn send_event(tx: &mpsc::UnboundedSender<Result<NetworkEvent>>, event: NetworkEvent) -> bool {
    tx.unbounded_send(Ok(event)).is_ok()
}

fn send_error(tx: &mpsc::UnboundedSender<Result<NetworkEvent>>, err: ConnectionError) -> bool {
    tx.unbounded_send(Err(err)).is_ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn settings_change_maps_to_network_event() {
        let event = settings_change_event(SettingsChange::Reloaded);

        assert!(matches!(
            event,
            NetworkEvent::SettingsChanged(SettingsChange::Reloaded)
        ));
    }

    #[test]
    fn device_change_keeps_interface_name() {
        let event = device_change_event(Some("wlan0".into()));

        match event {
            NetworkEvent::DeviceChanged { interface } => {
                assert_eq!(interface.as_deref(), Some("wlan0"));
            }
            _ => panic!("unexpected event"),
        }
    }
}
