//! Stream-based monitoring for NetworkManager saved connection settings.

use std::pin::Pin;

use futures::channel::mpsc;
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
    let conn = conn.clone();

    tokio::spawn(async move {
        if let Err(err) = run_settings_events(conn, tx.clone()).await {
            let _ = tx.unbounded_send(Err(err));
        }
    });

    Ok(Box::pin(rx))
}

async fn run_settings_events(
    conn: Connection,
    tx: mpsc::UnboundedSender<Result<SettingsChange>>,
) -> Result<()> {
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

    let mut merged = futures::stream::select_all(streams);
    while let Some(signal) = merged.next().await {
        match signal {
            SettingsSignal::Added(path) => {
                if !send_change(
                    &tx,
                    settings_signal_to_change(SettingsSignal::Added(path.clone())),
                ) {
                    return Ok(());
                }
                match connection_settings_streams(&conn, path.clone()).await {
                    Ok(connection_streams) => {
                        for stream in connection_streams {
                            merged.push(stream);
                        }
                    }
                    Err(err) => warn!("failed to monitor new settings connection {path}: {err}"),
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
    use super::*;

    fn path(value: &str) -> OwnedObjectPath {
        OwnedObjectPath::try_from(value).expect("valid object path")
    }

    #[test]
    fn settings_added_signal_maps_to_change() {
        let change = settings_signal_to_change(SettingsSignal::Added(path(
            "/org/freedesktop/NetworkManager/Settings/1",
        )));

        assert!(matches!(change, SettingsChange::Added { .. }));
    }

    #[test]
    fn settings_updated_signal_maps_to_change() {
        let change = settings_signal_to_change(SettingsSignal::Updated(path(
            "/org/freedesktop/NetworkManager/Settings/2",
        )));

        assert!(matches!(change, SettingsChange::Updated { .. }));
    }

    #[test]
    fn settings_reloaded_signal_maps_to_change() {
        let change = settings_signal_to_change(SettingsSignal::Reloaded);

        assert!(matches!(change, SettingsChange::Reloaded));
    }
}
