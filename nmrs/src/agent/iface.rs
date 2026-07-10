//! D-Bus object-server implementation for `org.freedesktop.NetworkManager.SecretAgent`.
//!
//! This is the interface that NetworkManager calls *into* when it needs
//! credentials for a connection. Each method translates the raw D-Bus call
//! into the channel-based API exposed by [`super::agent`].

use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use futures::SinkExt;
use futures::channel::{mpsc, oneshot};
use futures::future::{self, Either};
use log::{debug, trace, warn};
use zvariant::{ObjectPath, OwnedObjectPath};

use crate::types::constants::timeouts;

use super::request::{
    CancelReason, ConnectionDict, SecretAgentFlags, SecretReply, SecretRequest, SecretResponder,
    SecretStoreEvent, extract_existing_secrets, extract_setting_string, parse_secret_setting,
};

/// Custom D-Bus error type for the SecretAgent interface.
///
/// NM expects these specific error names when the agent refuses to provide
/// secrets.
#[derive(Debug, zbus::DBusError)]
#[zbus(prefix = "org.freedesktop.NetworkManager.SecretAgent")]
pub(crate) enum SecretAgentDBusError {
    #[zbus(error)]
    ZBus(zbus::Error),
    UserCanceled(String),
    NoSecrets(String),
}

type PendingMap = Arc<Mutex<HashMap<(String, String), oneshot::Sender<()>>>>;

/// The object served at the agent's D-Bus path. Not part of the public API —
/// consumers interact through [`SecretRequest`] / [`SecretResponder`].
pub(crate) struct SecretAgentInterface {
    pub(crate) request_tx: mpsc::Sender<SecretRequest>,
    pub(crate) cancel_tx: mpsc::UnboundedSender<CancelReason>,
    pub(crate) store_tx: mpsc::UnboundedSender<SecretStoreEvent>,
    pub(crate) pending: PendingMap,
}

#[zbus::interface(name = "org.freedesktop.NetworkManager.SecretAgent")]
impl SecretAgentInterface {
    /// Called by NetworkManager when a connection needs secrets.
    ///
    /// The method blocks (from NM's perspective) until the consumer replies
    /// via [`SecretResponder`], the request is cancelled, or the timeout
    /// expires.
    async fn get_secrets(
        &self,
        connection: ConnectionDict,
        connection_path: ObjectPath<'_>,
        setting_name: &str,
        hints: Vec<String>,
        flags: u32,
    ) -> Result<ConnectionDict, SecretAgentDBusError> {
        let path_owned: OwnedObjectPath = connection_path.into();
        let key = (path_owned.to_string(), setting_name.to_owned());

        debug!(
            "GetSecrets: path={} setting={} flags={:#x}",
            path_owned, setting_name, flags
        );

        let (reply_tx, reply_rx) = oneshot::channel::<SecretReply>();
        let (cancel_tx, cancel_rx) = oneshot::channel::<()>();

        // Track this pending request so CancelGetSecrets can find it.
        self.pending
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .insert(key.clone(), cancel_tx);

        let setting = parse_secret_setting(&connection, setting_name);
        let request = SecretRequest {
            connection_uuid: extract_setting_string(&connection, "connection", "uuid")
                .unwrap_or_default(),
            connection_id: extract_setting_string(&connection, "connection", "id")
                .unwrap_or_default(),
            connection_type: extract_setting_string(&connection, "connection", "type")
                .unwrap_or_default(),
            connection_path: path_owned,
            setting,
            hints,
            flags: SecretAgentFlags::from_bits_truncate(flags),
            responder: SecretResponder::new(reply_tx, setting_name.to_owned()),
            existing_secrets: extract_existing_secrets(&connection, setting_name),
        };

        // Send to the consumer stream. If the channel is full or closed,
        // reply NoSecrets immediately.
        if self.request_tx.clone().send(request).await.is_err() {
            self.pending
                .lock()
                .unwrap_or_else(|e| e.into_inner())
                .remove(&key);
            return Err(SecretAgentDBusError::NoSecrets(
                "agent request channel closed".into(),
            ));
        }

        let timeout = futures_timer::Delay::new(timeouts::secret_agent_response_timeout());

        // Wait for: consumer response, NM cancellation, or timeout.
        let result = future::select(reply_rx, future::select(cancel_rx, timeout)).await;

        self.pending
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .remove(&key);

        match result {
            Either::Left((Ok(SecretReply::Secrets(map)), _)) => Ok(map),
            Either::Left((Ok(SecretReply::UserCanceled), _)) => {
                Err(SecretAgentDBusError::UserCanceled("user canceled".into()))
            }
            Either::Left((Ok(SecretReply::NoSecrets) | Err(_), _)) => Err(
                SecretAgentDBusError::NoSecrets("no secrets available".into()),
            ),
            Either::Right((Either::Left(_cancel), _)) => {
                debug!("GetSecrets cancelled by NetworkManager for {}", key.1);
                Err(SecretAgentDBusError::UserCanceled(
                    "canceled by NetworkManager".into(),
                ))
            }
            Either::Right((Either::Right(_timeout), _)) => {
                warn!("GetSecrets timed out for setting {}", key.1);
                Err(SecretAgentDBusError::NoSecrets(
                    "timeout waiting for consumer response".into(),
                ))
            }
        }
    }

    /// Called by NetworkManager when a pending `GetSecrets` should be aborted.
    async fn cancel_get_secrets(
        &self,
        connection_path: ObjectPath<'_>,
        setting_name: &str,
    ) -> Result<(), SecretAgentDBusError> {
        let key = (connection_path.to_string(), setting_name.to_owned());

        debug!("CancelGetSecrets: path={} setting={}", key.0, key.1);

        if let Some(cancel_tx) = self
            .pending
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .remove(&key)
        {
            let _ = cancel_tx.send(());
        }

        let _ = self.cancel_tx.unbounded_send(CancelReason {
            connection_path: connection_path.into(),
            setting_name: setting_name.to_owned(),
        });

        Ok(())
    }

    /// Acknowledges a `SaveSecrets` call and forwards it to the store stream.
    async fn save_secrets(
        &self,
        _connection: ConnectionDict,
        connection_path: ObjectPath<'_>,
    ) -> Result<(), SecretAgentDBusError> {
        trace!("SaveSecrets: path={}", connection_path);
        let _ = self.store_tx.unbounded_send(SecretStoreEvent::Save {
            connection_path: connection_path.into(),
        });
        Ok(())
    }

    /// Acknowledges a `DeleteSecrets` call and forwards it to the store stream.
    async fn delete_secrets(
        &self,
        _connection: ConnectionDict,
        connection_path: ObjectPath<'_>,
    ) -> Result<(), SecretAgentDBusError> {
        trace!("DeleteSecrets: path={}", connection_path);
        let _ = self.store_tx.unbounded_send(SecretStoreEvent::Delete {
            connection_path: connection_path.into(),
        });
        Ok(())
    }
}
