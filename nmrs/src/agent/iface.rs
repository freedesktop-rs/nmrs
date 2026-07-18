//! D-Bus object-server implementation for `org.freedesktop.NetworkManager.SecretAgent`.
//!
//! This is the interface that NetworkManager calls *into* when it needs
//! credentials for a connection. Each method translates the raw D-Bus call
//! into the channel-based API exposed by [`super::agent`].

use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use futures::SinkExt;
use futures::channel::{mpsc, oneshot};
use futures::future::{self, Either};
use log::{debug, trace, warn};
use zvariant::{ObjectPath, OwnedObjectPath};

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

type PendingKey = (String, String);
type PendingMap = Arc<Mutex<HashMap<PendingKey, Vec<(u64, oneshot::Sender<()>)>>>>;

fn remove_pending_request(pending: &PendingMap, key: &PendingKey, request_id: u64) {
    let mut pending = pending.lock().unwrap_or_else(|error| error.into_inner());
    let remove_key = pending.get_mut(key).is_some_and(|requests| {
        requests.retain(|(id, _)| *id != request_id);
        requests.is_empty()
    });
    if remove_key {
        pending.remove(key);
    }
}

/// The object served at the agent's D-Bus path. Not part of the public API —
/// consumers interact through [`SecretRequest`] / [`SecretResponder`].
pub(crate) struct SecretAgentInterface {
    pub(crate) request_tx: mpsc::Sender<SecretRequest>,
    pub(crate) cancel_tx: mpsc::UnboundedSender<CancelReason>,
    pub(crate) store_tx: mpsc::UnboundedSender<SecretStoreEvent>,
    pub(crate) pending: PendingMap,
    pub(crate) next_request_id: AtomicU64,
    pub(crate) response_timeout: Duration,
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
        let request_id = self.next_request_id.fetch_add(1, Ordering::Relaxed);

        // Track this pending request so CancelGetSecrets can find it.
        self.pending
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .entry(key.clone())
            .or_default()
            .push((request_id, cancel_tx));

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

        // Cancellation must remain responsive while a bounded request queue is
        // applying back-pressure.
        let mut request_tx = self.request_tx.clone();
        let send_request = request_tx.send(request);
        let cancel_rx = match future::select(cancel_rx, send_request).await {
            Either::Left((_cancel, _send_request)) => {
                remove_pending_request(&self.pending, &key, request_id);
                return Err(SecretAgentDBusError::UserCanceled(
                    "canceled by NetworkManager".into(),
                ));
            }
            Either::Right((Ok(()), cancel_rx)) => cancel_rx,
            Either::Right((Err(_), _cancel_rx)) => {
                remove_pending_request(&self.pending, &key, request_id);
                return Err(SecretAgentDBusError::NoSecrets(
                    "agent request channel closed".into(),
                ));
            }
        };

        let timeout = futures_timer::Delay::new(self.response_timeout);

        // Wait for: consumer response, NM cancellation, or timeout.
        // Cancellation takes precedence if the consumer drops its responder at
        // the same time NetworkManager cancels the request.
        let result = future::select(cancel_rx, future::select(reply_rx, timeout)).await;

        remove_pending_request(&self.pending, &key, request_id);

        match result {
            Either::Left((_cancel, _)) => {
                debug!("GetSecrets cancelled by NetworkManager for {}", key.1);
                Err(SecretAgentDBusError::UserCanceled(
                    "canceled by NetworkManager".into(),
                ))
            }
            Either::Right((Either::Left((Ok(SecretReply::Secrets(map)), _)), _)) => Ok(map),
            Either::Right((Either::Left((Ok(SecretReply::UserCanceled), _)), _)) => {
                Err(SecretAgentDBusError::UserCanceled("user canceled".into()))
            }
            Either::Right((Either::Left((Ok(SecretReply::NoSecrets) | Err(_), _)), _)) => Err(
                SecretAgentDBusError::NoSecrets("no secrets available".into()),
            ),
            Either::Right((Either::Right((_timeout, _)), _)) => {
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

        if let Some(requests) = self
            .pending
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .remove(&key)
        {
            for (_, cancel_tx) in requests {
                let _ = cancel_tx.send(());
            }
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

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use futures::StreamExt;
    use zvariant::{ObjectPath, OwnedValue, Str};

    use super::*;

    const CONNECTION_PATH: &str = "/org/freedesktop/NetworkManager/Settings/42";

    struct Harness {
        iface: SecretAgentInterface,
        requests: mpsc::Receiver<SecretRequest>,
        cancellations: mpsc::UnboundedReceiver<CancelReason>,
        store_events: mpsc::UnboundedReceiver<SecretStoreEvent>,
    }

    fn harness_with_queue_depth(response_timeout: Duration, queue_depth: usize) -> Harness {
        let (request_tx, requests) = mpsc::channel(queue_depth);
        let (cancel_tx, cancellations) = mpsc::unbounded();
        let (store_tx, store_events) = mpsc::unbounded();
        Harness {
            iface: SecretAgentInterface {
                request_tx,
                cancel_tx,
                store_tx,
                pending: Arc::new(Mutex::new(HashMap::new())),
                next_request_id: AtomicU64::new(1),
                response_timeout,
            },
            requests,
            cancellations,
            store_events,
        }
    }

    fn harness(response_timeout: Duration) -> Harness {
        harness_with_queue_depth(response_timeout, 4)
    }

    fn connection() -> ConnectionDict {
        let mut metadata = HashMap::new();
        metadata.insert("uuid".into(), OwnedValue::from(Str::from("test-uuid")));
        metadata.insert("id".into(), OwnedValue::from(Str::from("test-id")));
        metadata.insert(
            "type".into(),
            OwnedValue::from(Str::from("802-11-wireless")),
        );

        let mut wireless = HashMap::new();
        wireless.insert(
            "ssid".into(),
            OwnedValue::try_from(zvariant::Array::from(b"test-ssid".to_vec()))
                .expect("owned byte array"),
        );

        HashMap::from([
            ("connection".into(), metadata),
            ("802-11-wireless".into(), wireless),
        ])
    }

    fn path() -> ObjectPath<'static> {
        ObjectPath::try_from(CONNECTION_PATH).expect("valid object path")
    }

    fn assert_no_pending(iface: &SecretAgentInterface) {
        assert!(
            iface
                .pending
                .lock()
                .unwrap_or_else(|err| err.into_inner())
                .is_empty()
        );
    }

    fn pending_count(iface: &SecretAgentInterface) -> usize {
        iface
            .pending
            .lock()
            .unwrap_or_else(|error| error.into_inner())
            .values()
            .map(Vec::len)
            .sum()
    }

    fn returned_psk(result: std::result::Result<ConnectionDict, SecretAgentDBusError>) -> String {
        let settings = result.expect("consumer secrets should be returned");
        let security = settings
            .get("802-11-wireless-security")
            .expect("security section");
        <&str>::try_from(security.get("psk").expect("PSK value"))
            .expect("string PSK")
            .to_owned()
    }

    #[tokio::test]
    async fn get_secrets_forwards_context_and_returns_consumer_reply() {
        let mut harness = harness(Duration::from_secs(1));

        let get = harness.iface.get_secrets(
            connection(),
            path(),
            "802-11-wireless-security",
            vec!["psk".into()],
            (SecretAgentFlags::ALLOW_INTERACTION | SecretAgentFlags::USER_REQUESTED).bits(),
        );
        let consume = async {
            let request = harness.requests.next().await.expect("secret request");
            assert_eq!(request.connection_uuid, "test-uuid");
            assert_eq!(request.connection_id, "test-id");
            assert_eq!(request.connection_type, "802-11-wireless");
            assert_eq!(request.connection_path.as_str(), CONNECTION_PATH);
            assert_eq!(request.hints, ["psk"]);
            assert!(request.flags.contains(SecretAgentFlags::ALLOW_INTERACTION));
            assert!(request.flags.contains(SecretAgentFlags::USER_REQUESTED));
            match request.setting {
                super::super::request::SecretSetting::WifiPsk { ssid } => {
                    assert_eq!(ssid, "test-ssid")
                }
                other => panic!("expected Wi-Fi PSK request, got {other:?}"),
            }
            request.responder.wifi_psk("test-password").await.unwrap();
        };

        let (result, ()) = tokio::join!(get, consume);
        let settings = result.expect("consumer secrets should be returned");
        let security = settings
            .get("802-11-wireless-security")
            .expect("security section");
        assert_eq!(
            <&str>::try_from(security.get("psk").expect("PSK value")).unwrap(),
            "test-password"
        );
        assert_no_pending(&harness.iface);
    }

    #[tokio::test]
    async fn get_secrets_maps_consumer_refusals_to_dbus_errors() {
        for user_cancelled in [false, true] {
            let mut harness = harness(Duration::from_secs(1));
            let get = harness.iface.get_secrets(
                connection(),
                path(),
                "802-11-wireless-security",
                Vec::new(),
                0,
            );
            let consume = async {
                let request = harness.requests.next().await.expect("secret request");
                if user_cancelled {
                    request.responder.cancel().await.unwrap();
                } else {
                    request.responder.no_secrets().await.unwrap();
                }
            };

            let (result, ()) = tokio::join!(get, consume);
            if user_cancelled {
                assert!(matches!(
                    result,
                    Err(SecretAgentDBusError::UserCanceled(message))
                        if message == "user canceled"
                ));
            } else {
                assert!(matches!(
                    result,
                    Err(SecretAgentDBusError::NoSecrets(message))
                        if message == "no secrets available"
                ));
            }
            assert_no_pending(&harness.iface);
        }
    }

    #[tokio::test]
    async fn cancel_get_secrets_aborts_pending_request_and_emits_reason() {
        let mut harness = harness(Duration::from_secs(1));
        let get = harness.iface.get_secrets(
            connection(),
            path(),
            "802-11-wireless-security",
            Vec::new(),
            0,
        );
        let cancel = async {
            let request = harness.requests.next().await.expect("secret request");
            harness
                .iface
                .cancel_get_secrets(path(), "802-11-wireless-security")
                .await
                .unwrap();
            drop(request);
        };

        let (result, ()) = tokio::join!(get, cancel);
        assert!(matches!(
            result,
            Err(SecretAgentDBusError::UserCanceled(message))
                if message == "canceled by NetworkManager"
        ));
        let reason = harness
            .cancellations
            .next()
            .await
            .expect("cancellation event");
        assert_eq!(reason.connection_path.as_str(), CONNECTION_PATH);
        assert_eq!(reason.setting_name, "802-11-wireless-security");
        assert_no_pending(&harness.iface);
    }

    #[tokio::test]
    async fn concurrent_same_key_requests_keep_independent_responders() {
        let mut harness = harness(Duration::from_secs(1));
        let first = harness.iface.get_secrets(
            connection(),
            path(),
            "802-11-wireless-security",
            Vec::new(),
            0,
        );
        let second = harness.iface.get_secrets(
            connection(),
            path(),
            "802-11-wireless-security",
            Vec::new(),
            0,
        );
        let respond = async {
            let first_request = harness.requests.next().await.expect("first request");
            let second_request = harness.requests.next().await.expect("second request");
            assert_eq!(pending_count(&harness.iface), 2);

            first_request
                .responder
                .wifi_psk("first-password")
                .await
                .expect("first request remains live");
            second_request
                .responder
                .wifi_psk("second-password")
                .await
                .expect("second request remains live");
        };

        let (first_result, second_result, ()) = tokio::join!(first, second, respond);
        assert_eq!(returned_psk(first_result), "first-password");
        assert_eq!(returned_psk(second_result), "second-password");
        assert_no_pending(&harness.iface);
    }

    #[tokio::test]
    async fn cancellation_aborts_all_concurrent_requests_with_the_same_key() {
        let mut harness = harness(Duration::from_secs(1));
        let first = harness.iface.get_secrets(
            connection(),
            path(),
            "802-11-wireless-security",
            Vec::new(),
            0,
        );
        let second = harness.iface.get_secrets(
            connection(),
            path(),
            "802-11-wireless-security",
            Vec::new(),
            0,
        );
        let cancel = async {
            let first_request = harness.requests.next().await.expect("first request");
            let second_request = harness.requests.next().await.expect("second request");
            assert_eq!(pending_count(&harness.iface), 2);

            harness
                .iface
                .cancel_get_secrets(path(), "802-11-wireless-security")
                .await
                .unwrap();
            drop((first_request, second_request));
        };

        let (first_result, second_result, ()) = tokio::join!(first, second, cancel);
        for result in [first_result, second_result] {
            assert!(matches!(
                result,
                Err(SecretAgentDBusError::UserCanceled(message))
                    if message == "canceled by NetworkManager"
            ));
        }
        let reason = harness
            .cancellations
            .next()
            .await
            .expect("cancellation event");
        assert_eq!(reason.connection_path.as_str(), CONNECTION_PATH);
        assert_eq!(reason.setting_name, "802-11-wireless-security");
        assert_no_pending(&harness.iface);
    }

    #[tokio::test]
    async fn cancellation_interrupts_backpressure_before_request_delivery() {
        let harness = harness_with_queue_depth(Duration::from_secs(1), 0);
        let get = harness.iface.get_secrets(
            connection(),
            path(),
            "802-11-wireless-security",
            Vec::new(),
            0,
        );
        let cancel = async {
            while pending_count(&harness.iface) == 0 {
                tokio::task::yield_now().await;
            }
            harness
                .iface
                .cancel_get_secrets(path(), "802-11-wireless-security")
                .await
                .unwrap();
        };

        let (result, ()) = tokio::time::timeout(Duration::from_millis(100), async {
            tokio::join!(get, cancel)
        })
        .await
        .expect("cancellation must not wait for request queue capacity");

        assert!(matches!(
            result,
            Err(SecretAgentDBusError::UserCanceled(message))
                if message == "canceled by NetworkManager"
        ));
        assert_no_pending(&harness.iface);
    }

    #[tokio::test]
    async fn get_secrets_fails_immediately_when_request_stream_is_closed() {
        let harness = harness(Duration::from_secs(1));
        drop(harness.requests);

        let result = harness
            .iface
            .get_secrets(
                connection(),
                path(),
                "802-11-wireless-security",
                Vec::new(),
                0,
            )
            .await;

        assert!(matches!(
            result,
            Err(SecretAgentDBusError::NoSecrets(message))
                if message == "agent request channel closed"
        ));
        assert_no_pending(&harness.iface);
    }

    #[tokio::test]
    async fn get_secrets_times_out_and_cleans_up_pending_request() {
        let mut harness = harness(Duration::from_millis(1));
        let get = harness.iface.get_secrets(
            connection(),
            path(),
            "802-11-wireless-security",
            Vec::new(),
            0,
        );
        let hold_request = async {
            let _request = harness.requests.next().await.expect("secret request");
            tokio::time::sleep(Duration::from_millis(20)).await;
        };

        let (result, ()) = tokio::join!(get, hold_request);
        assert!(matches!(
            result,
            Err(SecretAgentDBusError::NoSecrets(message))
                if message == "timeout waiting for consumer response"
        ));
        assert_no_pending(&harness.iface);
    }

    #[tokio::test]
    async fn save_and_delete_secrets_emit_exact_store_events() {
        let mut harness = harness(Duration::from_secs(1));

        harness
            .iface
            .save_secrets(ConnectionDict::new(), path())
            .await
            .unwrap();
        harness
            .iface
            .delete_secrets(ConnectionDict::new(), path())
            .await
            .unwrap();

        match harness.store_events.next().await.expect("save event") {
            SecretStoreEvent::Save { connection_path } => {
                assert_eq!(connection_path.as_str(), CONNECTION_PATH)
            }
            other => panic!("expected save event, got {other:?}"),
        }
        match harness.store_events.next().await.expect("delete event") {
            SecretStoreEvent::Delete { connection_path } => {
                assert_eq!(connection_path.as_str(), CONNECTION_PATH)
            }
            other => panic!("expected delete event, got {other:?}"),
        }
    }

    #[tokio::test]
    async fn informational_methods_acknowledge_when_consumers_are_gone() {
        let harness = harness(Duration::from_secs(1));
        drop(harness.cancellations);
        drop(harness.store_events);

        harness
            .iface
            .cancel_get_secrets(path(), "vpn")
            .await
            .expect("cancellation notification is best-effort");
        harness
            .iface
            .save_secrets(ConnectionDict::new(), path())
            .await
            .expect("save notification is best-effort");
        harness
            .iface
            .delete_secrets(ConnectionDict::new(), path())
            .await
            .expect("delete notification is best-effort");
    }
}
