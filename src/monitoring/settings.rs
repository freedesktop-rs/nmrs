//! Stream-based monitoring for NetworkManager saved connection settings.

use std::pin::Pin;

use futures::channel::{mpsc, oneshot};
use futures::stream::{Stream, StreamExt};
use log::{trace, warn};
use zbus::Connection;
use zvariant::OwnedObjectPath;

use crate::Result;
use crate::api::models::{ConnectionError, SettingsChange, SettingsEventStream};
use crate::dbus::{NMSettingsConnectionProxy, NMSettingsProxy};

type SettingsSignalStream = Pin<Box<dyn Stream<Item = SettingsSignal> + Send>>;

enum SettingsSignal {
    Added(OwnedObjectPath),
    Removed(OwnedObjectPath),
    Updated(OwnedObjectPath),
    Reloaded,
    Unknown,
}

/// Creates a stream of saved-connection settings changes.
pub(crate) async fn settings_events(conn: &Connection) -> Result<SettingsEventStream> {
    NMSettingsProxy::new(conn).await?;

    let (tx, rx) = mpsc::unbounded();
    let (ready_tx, ready_rx) = oneshot::channel();
    let conn = conn.clone();

    tokio::spawn(async move {
        if let Err(err) = run_settings_events(conn, tx.clone(), ready_tx).await {
            let _ = tx.unbounded_send(Err(err));
        }
    });

    ready_rx.await.map_err(|_| {
        ConnectionError::Stuck("settings event task ended before becoming ready".into())
    })??;

    Ok(Box::pin(rx))
}

async fn run_settings_events(
    conn: Connection,
    tx: mpsc::UnboundedSender<Result<SettingsChange>>,
    ready_tx: oneshot::Sender<Result<()>>,
) -> Result<()> {
    let setup: Result<_> = async {
        let settings = NMSettingsProxy::new(&conn).await?;
        let mut streams: Vec<SettingsSignalStream> = Vec::new();

        let new_connection = settings.receive_new_connection().await?;
        streams.push(Box::pin(new_connection.map(|signal| {
            signal.args().map_or_else(
                |_| SettingsSignal::Unknown,
                |args| SettingsSignal::Added(args.connection().clone()),
            )
        })));

        let connection_removed = settings.receive_connection_removed().await?;
        streams.push(Box::pin(connection_removed.map(|signal| {
            signal.args().map_or_else(
                |_| SettingsSignal::Unknown,
                |args| SettingsSignal::Removed(args.connection().clone()),
            )
        })));

        streams.push(Box::pin(
            settings
                .receive_connections_changed()
                .await
                .skip(1)
                .map(|_| SettingsSignal::Reloaded),
        ));

        for path in settings.list_connections().await? {
            match connection_settings_streams(&conn, path.clone()).await {
                Ok(connection_streams) => streams.extend(connection_streams),
                Err(err) => warn!("failed to monitor settings connection {path}: {err}"),
            }
        }

        Ok((settings, streams))
    }
    .await;

    let (_settings, streams) = match setup {
        Ok(setup) => setup,
        Err(error) => {
            let _ = ready_tx.send(Err(error));
            return Ok(());
        }
    };

    if ready_tx.send(Ok(())).is_err() {
        return Ok(());
    }

    let mut merged = futures::stream::select_all(streams);
    while let Some(signal) = merged.next().await {
        match signal {
            SettingsSignal::Added(path) => {
                match connection_settings_streams(&conn, path.clone()).await {
                    Ok(connection_streams) => {
                        for stream in connection_streams {
                            merged.push(stream);
                        }
                    }
                    Err(err) => warn!("failed to monitor new settings connection {path}: {err}"),
                }
                if !send_change(&tx, settings_signal_to_change(SettingsSignal::Added(path))) {
                    return Ok(());
                }
            }
            signal => {
                if !send_change(&tx, settings_signal_to_change(signal)) {
                    return Ok(());
                }
            }
        }
    }

    Err(ConnectionError::Stuck("settings event stream ended".into()))
}

async fn connection_settings_streams(
    conn: &Connection,
    path: OwnedObjectPath,
) -> Result<Vec<SettingsSignalStream>> {
    let connection = NMSettingsConnectionProxy::builder(conn)
        .path(path.clone())?
        .build()
        .await?;

    let updated_path = path.clone();
    let updated = connection
        .receive_updated()
        .await?
        .map(move |_| SettingsSignal::Updated(updated_path.clone()));

    let removed = connection
        .receive_removed()
        .await?
        .map(move |_| SettingsSignal::Removed(path.clone()));

    trace!("subscribed to settings connection signals");
    let streams: Vec<SettingsSignalStream> = vec![Box::pin(updated), Box::pin(removed)];
    Ok(streams)
}

fn send_change(tx: &mpsc::UnboundedSender<Result<SettingsChange>>, change: SettingsChange) -> bool {
    tx.unbounded_send(Ok(change)).is_ok()
}

fn settings_signal_to_change(signal: SettingsSignal) -> SettingsChange {
    match signal {
        SettingsSignal::Added(path) => SettingsChange::Added { path },
        SettingsSignal::Removed(path) => SettingsChange::Removed { path },
        SettingsSignal::Updated(path) => SettingsChange::Updated { path },
        SettingsSignal::Reloaded => SettingsChange::Reloaded,
        SettingsSignal::Unknown => SettingsChange::Unknown,
    }
}

#[cfg(test)]
mod tests {
    use futures::StreamExt;

    use super::*;

    fn path(value: &str) -> OwnedObjectPath {
        OwnedObjectPath::try_from(value).expect("valid object path")
    }

    #[test]
    fn every_settings_signal_maps_to_the_exact_public_change() {
        let added_path = path("/org/freedesktop/NetworkManager/Settings/1");
        let removed_path = path("/org/freedesktop/NetworkManager/Settings/2");
        let updated_path = path("/org/freedesktop/NetworkManager/Settings/3");

        match settings_signal_to_change(SettingsSignal::Added(added_path.clone())) {
            SettingsChange::Added { path } => assert_eq!(path, added_path),
            other => panic!("expected added change, got {other:?}"),
        }
        match settings_signal_to_change(SettingsSignal::Removed(removed_path.clone())) {
            SettingsChange::Removed { path } => assert_eq!(path, removed_path),
            other => panic!("expected removed change, got {other:?}"),
        }
        match settings_signal_to_change(SettingsSignal::Updated(updated_path.clone())) {
            SettingsChange::Updated { path } => assert_eq!(path, updated_path),
            other => panic!("expected updated change, got {other:?}"),
        }
        assert!(matches!(
            settings_signal_to_change(SettingsSignal::Reloaded),
            SettingsChange::Reloaded
        ));
        assert!(matches!(
            settings_signal_to_change(SettingsSignal::Unknown),
            SettingsChange::Unknown
        ));
    }

    #[tokio::test]
    async fn send_change_delivers_the_value_and_reports_closed_consumer() {
        let (tx, mut rx) = mpsc::unbounded();
        let expected_path = path("/org/freedesktop/NetworkManager/Settings/9");

        assert!(send_change(
            &tx,
            SettingsChange::Removed {
                path: expected_path.clone(),
            }
        ));
        match rx.next().await.expect("settings result").unwrap() {
            SettingsChange::Removed { path } => assert_eq!(path, expected_path),
            other => panic!("expected removed change, got {other:?}"),
        }

        drop(rx);
        assert!(!send_change(&tx, SettingsChange::Unknown));
    }
}
