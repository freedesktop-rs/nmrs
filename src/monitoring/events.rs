//! Unified stream-based NetworkManager event monitoring.

use std::collections::{HashMap, HashSet};
use std::pin::Pin;

use futures::channel::{mpsc, oneshot};
use futures::stream::{AbortHandle, Stream, StreamExt, abortable};
use log::trace;
use zbus::Connection;
use zvariant::OwnedObjectPath;

use crate::Result;
use crate::api::models::{ConnectionError, NetworkEvent, NetworkEventStream, SettingsChange};
use crate::dbus::{NMAccessPointProxy, NMDeviceProxy, NMProxy, NMWirelessProxy};
use crate::monitoring::settings;
use crate::types::constants::device_type;

type InternalEventStream<'a> = Pin<Box<dyn Stream<Item = InternalEvent> + Send + 'a>>;
struct ObjectEventStream<'a> {
    path: OwnedObjectPath,
    parent: Option<OwnedObjectPath>,
    stream: InternalEventStream<'a>,
}

enum InternalEvent {
    Event(NetworkEvent),
    Error(ConnectionError),
    AccessPointAdded {
        device: OwnedObjectPath,
        access_point: OwnedObjectPath,
    },
    AccessPointRemoved(OwnedObjectPath),
    DeviceAdded(OwnedObjectPath),
    DeviceRemoved(OwnedObjectPath),
}

enum InternalEventAction {
    Event(NetworkEvent),
    Error(ConnectionError),
    MonitorAccessPoint {
        device: OwnedObjectPath,
        access_point: OwnedObjectPath,
    },
    MonitorDevice(OwnedObjectPath),
    ForgetObject(OwnedObjectPath, NetworkEvent),
}

fn classify_internal_event(event: InternalEvent) -> InternalEventAction {
    match event {
        InternalEvent::Event(event) => InternalEventAction::Event(event),
        InternalEvent::Error(error) => InternalEventAction::Error(error),
        InternalEvent::AccessPointAdded {
            device,
            access_point,
        } => InternalEventAction::MonitorAccessPoint {
            device,
            access_point,
        },
        InternalEvent::AccessPointRemoved(path) => {
            InternalEventAction::ForgetObject(path, NetworkEvent::AccessPointsChanged)
        }
        InternalEvent::DeviceAdded(path) => InternalEventAction::MonitorDevice(path),
        InternalEvent::DeviceRemoved(path) => {
            InternalEventAction::ForgetObject(path, device_change_event(None))
        }
    }
}

#[derive(Default)]
struct ObjectStreams {
    handles: HashMap<OwnedObjectPath, Vec<AbortHandle>>,
    children: HashMap<OwnedObjectPath, HashSet<OwnedObjectPath>>,
    parents: HashMap<OwnedObjectPath, OwnedObjectPath>,
}

impl ObjectStreams {
    fn track<'a>(&mut self, object: ObjectEventStream<'a>) -> InternalEventStream<'a> {
        let (stream, handle) = abortable(object.stream);
        if let Some(parent) = object.parent {
            self.children
                .entry(parent.clone())
                .or_default()
                .insert(object.path.clone());
            self.parents.insert(object.path.clone(), parent);
        }
        self.handles.entry(object.path).or_default().push(handle);
        Box::pin(stream)
    }

    fn release(&mut self, path: &OwnedObjectPath) {
        for handle in self.handles.remove(path).unwrap_or_default() {
            handle.abort();
        }
        if let Some(parent) = self.parents.remove(path)
            && let Some(siblings) = self.children.get_mut(&parent)
        {
            siblings.remove(path);
            if siblings.is_empty() {
                self.children.remove(&parent);
            }
        }
        for child in self.children.remove(path).unwrap_or_default() {
            self.release(&child);
        }
    }

    #[cfg(test)]
    fn is_empty(&self) -> bool {
        self.handles.is_empty() && self.children.is_empty() && self.parents.is_empty()
    }
}

/// Creates a unified refresh-oriented stream of NetworkManager events.
pub(crate) async fn network_events(conn: &Connection) -> Result<NetworkEventStream> {
    let (tx, rx) = mpsc::unbounded();
    let (ready_tx, ready_rx) = oneshot::channel();
    let conn = conn.clone();

    tokio::spawn(async move {
        if let Err(err) = run_network_events(conn, tx.clone(), ready_tx).await {
            let _ = tx.unbounded_send(Err(err));
        }
    });

    ready_rx.await.map_err(|_| {
        ConnectionError::Stuck("network event task ended before becoming ready".into())
    })??;

    Ok(Box::pin(rx))
}

async fn run_network_events(
    conn: Connection,
    tx: mpsc::UnboundedSender<Result<NetworkEvent>>,
    ready_tx: oneshot::Sender<Result<()>>,
) -> Result<()> {
    macro_rules! setup_or_report {
        ($future:expr) => {
            match $future.await {
                Ok(value) => value,
                Err(error) => {
                    let _ = ready_tx.send(Err(error.into()));
                    return Ok(());
                }
            }
        };
    }

    let nm = setup_or_report!(NMProxy::new(&conn));
    let dbus = setup_or_report!(zbus::fdo::DBusProxy::new(&conn));
    let mut streams = setup_or_report!(base_network_event_streams(&nm, &dbus));

    let settings_stream = setup_or_report!(settings::settings_events(&conn));
    streams.push(Box::pin(settings_stream.map(|item| match item {
        Ok(change) => InternalEvent::Event(settings_change_event(change)),
        Err(err) => InternalEvent::Error(err),
    })));

    let mut objects = ObjectStreams::default();

    for object in setup_or_report!(device_state_streams(&conn, &nm)) {
        streams.push(objects.track(object));
    }

    for object in setup_or_report!(access_point_streams(&conn, &nm)) {
        streams.push(objects.track(object));
    }

    if ready_tx.send(Ok(())).is_err() {
        return Ok(());
    }

    let mut merged = futures::stream::select_all(streams);
    while let Some(internal) = merged.next().await {
        match classify_internal_event(internal) {
            InternalEventAction::Event(event) => {
                if !send_event(&tx, event) {
                    return Ok(());
                }
            }
            InternalEventAction::Error(err) => {
                if !send_error(&tx, err) {
                    return Ok(());
                }
            }
            InternalEventAction::MonitorAccessPoint {
                device,
                access_point,
            } => {
                if !send_event(&tx, NetworkEvent::AccessPointsChanged) {
                    return Ok(());
                }
                objects.release(&access_point);
                match access_point_strength_stream(&conn, device, access_point.clone()).await {
                    Ok(object) => merged.push(objects.track(object)),
                    Err(err) => trace!("failed to monitor access point {access_point}: {err}"),
                }
            }
            InternalEventAction::MonitorDevice(path) => {
                let event = device_changed_for_path(&conn, &path).await;
                if !send_event(&tx, event) {
                    return Ok(());
                }
                objects.release(&path);
                match device_state_stream(&conn, path.clone()).await {
                    Ok(stream) => merged.push(objects.track(ObjectEventStream {
                        path: path.clone(),
                        parent: None,
                        stream,
                    })),
                    Err(err) => trace!("failed to monitor device {path}: {err}"),
                }
                match wireless_device_streams(&conn, path.clone()).await {
                    Ok(streams) => {
                        for object in streams {
                            merged.push(objects.track(object));
                        }
                    }
                    Err(err) => trace!("failed to monitor wireless device {path}: {err}"),
                }
            }
            InternalEventAction::ForgetObject(path, event) => {
                objects.release(&path);
                if !send_event(&tx, event) {
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
    streams.push(Box::pin(device_removed.map(|signal| {
        signal
            .args()
            .map_or(InternalEvent::Event(device_change_event(None)), |args| {
                InternalEvent::DeviceRemoved(args.device().clone())
            })
    })));

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
) -> Result<Vec<ObjectEventStream<'a>>> {
    let mut streams: Vec<ObjectEventStream<'_>> = Vec::new();

    for path in nm.get_devices().await? {
        match device_state_stream(conn, path.clone()).await {
            Ok(stream) => streams.push(ObjectEventStream {
                path,
                parent: None,
                stream,
            }),
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
) -> Result<Vec<ObjectEventStream<'a>>> {
    let mut streams: Vec<ObjectEventStream<'_>> = Vec::new();

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
) -> Result<Vec<ObjectEventStream<'a>>> {
    let mut streams: Vec<ObjectEventStream<'_>> = Vec::new();
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
    let owner = device_path.clone();
    streams.push(ObjectEventStream {
        path: device_path.clone(),
        parent: None,
        stream: Box::pin(added.map(move |signal| {
            signal.args().map_or(
                InternalEvent::Event(NetworkEvent::AccessPointsChanged),
                |args| InternalEvent::AccessPointAdded {
                    device: owner.clone(),
                    access_point: args.path().clone(),
                },
            )
        })),
    });

    let removed = wifi.receive_access_point_removed().await?;
    streams.push(ObjectEventStream {
        path: device_path.clone(),
        parent: None,
        stream: Box::pin(removed.map(|signal| {
            signal.args().map_or(
                InternalEvent::Event(NetworkEvent::AccessPointsChanged),
                |args| InternalEvent::AccessPointRemoved(args.path().clone()),
            )
        })),
    });

    match wifi.access_points().await {
        Ok(access_points) => {
            for access_point in access_points {
                match access_point_strength_stream(conn, device_path.clone(), access_point.clone())
                    .await
                {
                    Ok(object) => streams.push(object),
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
    device: OwnedObjectPath,
    path: OwnedObjectPath,
) -> Result<ObjectEventStream<'a>> {
    let access_point = NMAccessPointProxy::builder(conn)
        .path(path.clone())?
        .build()
        .await?;

    Ok(ObjectEventStream {
        path,
        parent: Some(device),
        stream: Box::pin(
            access_point
                .receive_strength_changed()
                .await
                .skip(1)
                .map(|_| InternalEvent::Event(NetworkEvent::AccessPointsChanged)),
        ),
    })
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
    use futures::{FutureExt, StreamExt};

    use super::*;

    fn path(value: &str) -> OwnedObjectPath {
        OwnedObjectPath::try_from(value).expect("valid object path")
    }

    #[test]
    fn internal_event_classifier_preserves_every_event_kind() {
        assert!(matches!(
            classify_internal_event(InternalEvent::Event(NetworkEvent::ConnectivityChanged)),
            InternalEventAction::Event(NetworkEvent::ConnectivityChanged)
        ));
        assert!(matches!(
            classify_internal_event(InternalEvent::Error(ConnectionError::Stuck(
                "stream failed".into()
            ))),
            InternalEventAction::Error(ConnectionError::Stuck(message))
                if message == "stream failed"
        ));

        let access_point = path("/org/freedesktop/NetworkManager/AccessPoint/4");
        let wifi = path("/org/freedesktop/NetworkManager/Devices/3");
        match classify_internal_event(InternalEvent::AccessPointAdded {
            device: wifi.clone(),
            access_point: access_point.clone(),
        }) {
            InternalEventAction::MonitorAccessPoint {
                device,
                access_point: actual,
            } => {
                assert_eq!(device, wifi);
                assert_eq!(actual, access_point);
            }
            _ => panic!("expected access-point monitoring action"),
        }
        match classify_internal_event(InternalEvent::AccessPointRemoved(access_point.clone())) {
            InternalEventAction::ForgetObject(actual, NetworkEvent::AccessPointsChanged) => {
                assert_eq!(actual, access_point);
            }
            _ => panic!("expected access-point forget action"),
        }

        let device = path("/org/freedesktop/NetworkManager/Devices/2");
        match classify_internal_event(InternalEvent::DeviceAdded(device.clone())) {
            InternalEventAction::MonitorDevice(actual) => assert_eq!(actual, device),
            _ => panic!("expected device monitoring action"),
        }
        match classify_internal_event(InternalEvent::DeviceRemoved(device.clone())) {
            InternalEventAction::ForgetObject(
                actual,
                NetworkEvent::DeviceChanged { interface: None },
            ) => assert_eq!(actual, device),
            _ => panic!("expected device forget action"),
        }
    }

    fn silent(
        path: &OwnedObjectPath,
        parent: Option<&OwnedObjectPath>,
    ) -> ObjectEventStream<'static> {
        ObjectEventStream {
            path: path.clone(),
            parent: parent.cloned(),
            stream: Box::pin(futures::stream::pending()),
        }
    }

    #[test]
    fn released_object_streams_leave_the_merged_stream() {
        let mut objects = ObjectStreams::default();
        let removed = path("/org/freedesktop/NetworkManager/AccessPoint/7");
        let mut merged = futures::stream::select_all([
            objects.track(silent(&removed, None)),
            objects.track(silent(&removed, None)),
        ]);

        assert!(merged.next().now_or_never().is_none());

        objects.release(&removed);

        assert!(matches!(merged.next().now_or_never(), Some(None)));
        assert!(merged.is_empty());
        assert!(objects.is_empty());
    }

    #[test]
    fn releasing_one_object_keeps_the_others() {
        let mut objects = ObjectStreams::default();
        let removed = path("/org/freedesktop/NetworkManager/Devices/9");
        let kept = path("/org/freedesktop/NetworkManager/Devices/2");
        let mut merged = futures::stream::select_all([
            objects.track(silent(&removed, None)),
            objects.track(silent(&kept, None)),
        ]);

        objects.release(&removed);

        assert!(merged.next().now_or_never().is_none());
        assert_eq!(merged.len(), 1);
    }

    #[test]
    fn releasing_a_device_releases_its_access_points() {
        let mut objects = ObjectStreams::default();
        let wifi = path("/org/freedesktop/NetworkManager/Devices/3");
        let other_wifi = path("/org/freedesktop/NetworkManager/Devices/4");
        let first = path("/org/freedesktop/NetworkManager/AccessPoint/10");
        let second = path("/org/freedesktop/NetworkManager/AccessPoint/11");
        let elsewhere = path("/org/freedesktop/NetworkManager/AccessPoint/12");
        let mut merged = futures::stream::select_all([
            objects.track(silent(&wifi, None)),
            objects.track(silent(&first, Some(&wifi))),
            objects.track(silent(&second, Some(&wifi))),
            objects.track(silent(&other_wifi, None)),
            objects.track(silent(&elsewhere, Some(&other_wifi))),
        ]);

        objects.release(&wifi);

        assert!(merged.next().now_or_never().is_none());
        assert_eq!(merged.len(), 2);

        objects.release(&other_wifi);

        assert!(matches!(merged.next().now_or_never(), Some(None)));
        assert!(objects.is_empty());
    }

    #[test]
    fn releasing_an_access_point_detaches_it_from_its_device() {
        let mut objects = ObjectStreams::default();
        let wifi = path("/org/freedesktop/NetworkManager/Devices/3");
        let mut merged = futures::stream::select_all([objects.track(silent(&wifi, None))]);

        for id in 0..100 {
            let access_point = path(&format!("/org/freedesktop/NetworkManager/AccessPoint/{id}"));
            merged.push(objects.track(silent(&access_point, Some(&wifi))));
            objects.release(&access_point);
        }

        assert!(merged.next().now_or_never().is_none());
        assert_eq!(merged.len(), 1);
        assert!(!objects.children.contains_key(&wifi));
        assert!(objects.parents.is_empty());
    }

    #[test]
    fn settings_change_preserves_variant_and_path() {
        let expected = path("/org/freedesktop/NetworkManager/Settings/17");
        let event = settings_change_event(SettingsChange::Updated {
            path: expected.clone(),
        });

        match event {
            NetworkEvent::SettingsChanged(SettingsChange::Updated { path }) => {
                assert_eq!(path, expected)
            }
            other => panic!("expected updated settings event, got {other:?}"),
        }
    }

    #[test]
    fn device_change_preserves_known_and_unknown_interfaces() {
        let event = device_change_event(Some("wlan0".into()));

        match event {
            NetworkEvent::DeviceChanged { interface } => {
                assert_eq!(interface.as_deref(), Some("wlan0"));
            }
            other => panic!("expected device event, got {other:?}"),
        }

        assert!(matches!(
            device_change_event(None),
            NetworkEvent::DeviceChanged { interface: None }
        ));
    }

    #[tokio::test]
    async fn send_event_delivers_value_and_reports_closed_consumer() {
        let (tx, mut rx) = mpsc::unbounded();

        assert!(send_event(&tx, NetworkEvent::ConnectivityChanged));
        assert!(matches!(
            rx.next().await.expect("network event").unwrap(),
            NetworkEvent::ConnectivityChanged
        ));

        drop(rx);
        assert!(!send_event(&tx, NetworkEvent::AccessPointsChanged));
    }

    #[tokio::test]
    async fn send_error_delivers_error_and_reports_closed_consumer() {
        let (tx, mut rx) = mpsc::unbounded();

        assert!(send_error(
            &tx,
            ConnectionError::Stuck("signal ended".into())
        ));
        assert!(matches!(
            rx.next().await.expect("network error"),
            Err(ConnectionError::Stuck(message)) if message == "signal ended"
        ));

        drop(rx);
        assert!(!send_error(
            &tx,
            ConnectionError::Stuck("consumer gone".into())
        ));
    }
}
