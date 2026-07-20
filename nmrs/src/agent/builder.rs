//! Secret agent builder, handle, and lifecycle management.

use std::collections::HashMap;
use std::sync::atomic::AtomicU64;
use std::sync::{Arc, Mutex};

use futures::channel::mpsc;
use log::debug;
use zbus::Connection;

use crate::ConnectionError;
use crate::dbus::AgentManagerProxy;

use super::iface::SecretAgentInterface;
use super::request::{CancelReason, SecretAgentCapabilities, SecretRequest, SecretStoreEvent};

const DEFAULT_IDENTIFIER: &str = "com.system76.CosmicApplets.nmrs.secret_agent";
const DEFAULT_OBJECT_PATH: &str = "/org/freedesktop/NetworkManager/SecretAgent";
const DEFAULT_QUEUE_DEPTH: usize = 32;
const AGENT_MANAGER_ERROR_PREFIX: &str = "org.freedesktop.NetworkManager.AgentManager.";

#[derive(Debug, PartialEq, Eq)]
enum AgentManagerFailure {
    AlreadyRegistered,
    Registration,
    NotRegistered,
    Other,
}

fn classify_agent_manager_failure(name: &str, detail: Option<&str>) -> AgentManagerFailure {
    if !name.starts_with(AGENT_MANAGER_ERROR_PREFIX) {
        return AgentManagerFailure::Other;
    }

    match name.strip_prefix(AGENT_MANAGER_ERROR_PREFIX) {
        Some("PermissionDenied")
            if detail.is_some_and(|detail| {
                detail.to_ascii_lowercase().contains("already registered")
            }) =>
        {
            AgentManagerFailure::AlreadyRegistered
        }
        Some("NotRegistered") => AgentManagerFailure::NotRegistered,
        _ => AgentManagerFailure::Registration,
    }
}

fn classify_zbus_agent_manager_failure(error: &zbus::Error) -> AgentManagerFailure {
    match error {
        zbus::Error::MethodError(name, detail, _) => {
            classify_agent_manager_failure(name.as_str(), detail.as_deref())
        }
        _ => AgentManagerFailure::Other,
    }
}

fn registration_error(error: zbus::Error, operation: &str) -> ConnectionError {
    match classify_zbus_agent_manager_failure(&error) {
        AgentManagerFailure::AlreadyRegistered => ConnectionError::AgentAlreadyRegistered,
        AgentManagerFailure::Registration | AgentManagerFailure::NotRegistered => {
            ConnectionError::AgentRegistration {
                context: format!("{operation}: {error}"),
            }
        }
        AgentManagerFailure::Other => ConnectionError::DbusOperation {
            context: operation.into(),
            source: error,
        },
    }
}

fn unregistration_error(error: zbus::Error) -> ConnectionError {
    if classify_zbus_agent_manager_failure(&error) == AgentManagerFailure::NotRegistered {
        ConnectionError::AgentNotRegistered
    } else {
        ConnectionError::DbusOperation {
            context: "unregistering secret agent".into(),
            source: error,
        }
    }
}

/// Entry point for creating a NetworkManager secret agent.
///
/// A secret agent receives credential requests from NetworkManager over D-Bus
/// whenever a connection needs secrets the system does not already have (Wi-Fi
/// password forgotten, VPN token expired, etc.).
///
/// Use [`SecretAgent::builder()`] to configure and register the agent.
///
/// # Example
///
/// ```no_run
/// use futures::StreamExt;
/// use nmrs::agent::{SecretAgent, SecretSetting};
///
/// # async fn example() -> nmrs::Result<()> {
/// let (handle, mut requests) = SecretAgent::builder().register().await?;
///
/// while let Some(req) = requests.next().await {
///     if let SecretSetting::WifiPsk { ref ssid } = req.setting {
///         println!("password requested for {ssid}");
///         req.responder.wifi_psk("my-password").await?;
///     }
/// }
/// # Ok(())
/// # }
/// ```
pub struct SecretAgent;

impl SecretAgent {
    /// Returns a builder for configuring and registering a secret agent.
    #[must_use]
    pub fn builder() -> SecretAgentBuilder {
        SecretAgentBuilder::default()
    }
}

/// Builder for configuring and registering a [`SecretAgent`].
///
/// Use the `with_*` methods to override defaults, then call
/// [`register()`](Self::register) to connect to the system bus and start
/// serving.
///
/// # Defaults
///
/// | Setting | Default |
/// |---------|---------|
/// | identifier | `com.system76.CosmicApplets.nmrs.secret_agent` |
/// | capabilities | [`SecretAgentCapabilities::VPN_HINTS`] |
/// | object path | `/org/freedesktop/NetworkManager/SecretAgent` |
/// | queue depth | 32 |
#[derive(Debug)]
pub struct SecretAgentBuilder {
    identifier: String,
    capabilities: SecretAgentCapabilities,
    object_path: String,
    queue_depth: usize,
}

impl Default for SecretAgentBuilder {
    fn default() -> Self {
        Self {
            identifier: DEFAULT_IDENTIFIER.into(),
            capabilities: SecretAgentCapabilities::VPN_HINTS,
            object_path: DEFAULT_OBJECT_PATH.into(),
            queue_depth: DEFAULT_QUEUE_DEPTH,
        }
    }
}

impl SecretAgentBuilder {
    /// Sets the identifier passed to NetworkManager for this agent.
    ///
    /// This is not a D-Bus bus name and `nmrs` will not try to own it on the
    /// system bus. NetworkManager requires it to be unique within the user's
    /// agent session and to follow D-Bus bus-name formatting rules, except
    /// that `:` is not allowed.
    #[must_use]
    pub fn with_identifier(mut self, identifier: impl Into<String>) -> Self {
        self.identifier = identifier.into();
        self
    }

    /// Sets the capabilities advertised to NetworkManager.
    ///
    /// Defaults to [`SecretAgentCapabilities::VPN_HINTS`].
    #[must_use]
    pub fn with_capabilities(mut self, capabilities: SecretAgentCapabilities) -> Self {
        self.capabilities = capabilities;
        self
    }

    /// Sets the D-Bus object path where the agent interface is served.
    ///
    /// NetworkManager calls secret agents at
    /// `/org/freedesktop/NetworkManager/SecretAgent`; overriding this is only
    /// useful for tests or custom callers that use the same non-standard path.
    #[must_use]
    pub fn with_object_path(mut self, path: impl Into<String>) -> Self {
        self.object_path = path.into();
        self
    }

    /// Sets the maximum number of `GetSecrets` requests to buffer before
    /// back-pressure kicks in. Defaults to 32.
    #[must_use]
    pub fn with_queue_depth(mut self, depth: usize) -> Self {
        self.queue_depth = depth;
        self
    }

    /// Connects to the system bus, registers the agent with NetworkManager,
    /// and returns a handle and a stream of incoming secret requests.
    ///
    /// The returned [`mpsc::Receiver`](futures::channel::mpsc::Receiver)
    /// implements [`Stream`](futures::Stream) and yields
    /// [`SecretRequest`] items as they arrive from NetworkManager.
    ///
    /// # Errors
    ///
    /// - [`ConnectionError::AgentRegistration`] if NetworkManager rejected
    ///   the registration.
    /// - [`ConnectionError::Dbus`] for other D-Bus failures.
    pub async fn register(
        self,
    ) -> crate::Result<(SecretAgentHandle, mpsc::Receiver<SecretRequest>)> {
        let (request_tx, request_rx) = mpsc::channel(self.queue_depth);
        let (cancel_tx, cancel_rx) = mpsc::unbounded();
        let (store_tx, store_rx) = mpsc::unbounded();

        let iface = SecretAgentInterface {
            request_tx,
            cancel_tx,
            store_tx,
            pending: Arc::new(Mutex::new(HashMap::new())),
            next_request_id: AtomicU64::new(1),
            response_timeout: crate::types::constants::timeouts::secret_agent_response_timeout(),
        };

        let conn = Connection::system()
            .await
            .map_err(|e| ConnectionError::DbusOperation {
                context: "connecting to system bus for secret agent".into(),
                source: e,
            })?;

        conn.object_server()
            .at(&*self.object_path, iface)
            .await
            .map_err(|e| ConnectionError::DbusOperation {
                context: format!("serving SecretAgent interface at {}", self.object_path),
                source: e,
            })?;

        debug!(
            "Serving secret agent '{}' at '{}'",
            self.identifier, self.object_path
        );

        let agent_proxy =
            AgentManagerProxy::new(&conn)
                .await
                .map_err(|e| ConnectionError::DbusOperation {
                    context: "creating AgentManager proxy".into(),
                    source: e,
                })?;

        agent_proxy
            .register_with_capabilities(&self.identifier, self.capabilities.bits())
            .await
            .map_err(|error| {
                registration_error(error, "registering secret agent with NetworkManager")
            })?;

        debug!(
            "Registered secret agent '{}' with capabilities {:?}",
            self.identifier, self.capabilities
        );

        let handle = SecretAgentHandle {
            conn,
            identifier: self.identifier,
            capabilities: self.capabilities,
            object_path: self.object_path,
            cancel_rx,
            store_rx,
        };

        Ok((handle, request_rx))
    }
}

/// Handle to a running secret agent.
///
/// Provides methods to re-register after a NetworkManager restart, access
/// the cancellation and store-event streams, and shut the agent down.
///
/// GUI apps should keep this handle alive for as long as they want to answer
/// NetworkManager credential prompts. Dropping the handle drops the D-Bus
/// connection that serves the secret-agent object.
pub struct SecretAgentHandle {
    conn: Connection,
    identifier: String,
    capabilities: SecretAgentCapabilities,
    object_path: String,
    cancel_rx: mpsc::UnboundedReceiver<CancelReason>,
    store_rx: mpsc::UnboundedReceiver<SecretStoreEvent>,
}

impl std::fmt::Debug for SecretAgentHandle {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SecretAgentHandle")
            .field("identifier", &self.identifier)
            .field("object_path", &self.object_path)
            .finish_non_exhaustive()
    }
}

impl SecretAgentHandle {
    /// Re-registers the agent with NetworkManager.
    ///
    /// Call this after detecting that NetworkManager restarted (e.g. its
    /// D-Bus name owner changed). Calling it while the agent is still registered
    /// returns [`ConnectionError::AgentAlreadyRegistered`]. The same long-lived
    /// handle can be kept and re-registered whenever NetworkManager comes back.
    ///
    /// # Errors
    ///
    /// Returns an error if the D-Bus call to `RegisterWithCapabilities` fails.
    pub async fn reregister(&self) -> crate::Result<()> {
        let proxy = AgentManagerProxy::new(&self.conn).await.map_err(|e| {
            ConnectionError::DbusOperation {
                context: "creating AgentManager proxy for re-registration".into(),
                source: e,
            }
        })?;
        proxy
            .register_with_capabilities(&self.identifier, self.capabilities.bits())
            .await
            .map_err(|error| {
                registration_error(error, "re-registering secret agent with NetworkManager")
            })?;
        debug!("Re-registered secret agent '{}'", self.identifier);
        Ok(())
    }

    /// Unregisters the agent from NetworkManager.
    ///
    /// After this call, the request stream returned by
    /// [`SecretAgentBuilder::register`] will complete.
    ///
    /// # Errors
    ///
    /// Returns an error if the D-Bus `Unregister` call fails.
    pub async fn unregister(self) -> crate::Result<()> {
        let proxy = AgentManagerProxy::new(&self.conn).await.map_err(|e| {
            ConnectionError::DbusOperation {
                context: "creating AgentManager proxy for unregistration".into(),
                source: e,
            }
        })?;
        proxy.unregister().await.map_err(unregistration_error)?;
        debug!("Unregistered secret agent '{}'", self.identifier);
        Ok(())
    }

    /// Returns a mutable reference to the cancellation stream.
    ///
    /// Yields [`CancelReason`] items when NetworkManager calls
    /// `CancelGetSecrets` for an in-flight request. By the time the
    /// consumer receives this event, the agent has already replied to
    /// NetworkManager.
    ///
    /// Use with [`StreamExt::next()`](futures::StreamExt::next):
    ///
    /// ```ignore
    /// while let Some(reason) = handle.cancellations().next().await {
    ///     println!("cancelled: {}", reason.setting_name);
    /// }
    /// ```
    pub fn cancellations(&mut self) -> &mut mpsc::UnboundedReceiver<CancelReason> {
        &mut self.cancel_rx
    }

    /// Returns a mutable reference to the save/delete event stream.
    ///
    /// Yields [`SecretStoreEvent`] items when NetworkManager sends
    /// `SaveSecrets` or `DeleteSecrets`. These are informational — the agent
    /// always acknowledges them immediately.
    pub fn store_events(&mut self) -> &mut mpsc::UnboundedReceiver<SecretStoreEvent> {
        &mut self.store_rx
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn agent_manager_errors_map_to_public_lifecycle_failures() {
        let cases = [
            (
                "org.freedesktop.NetworkManager.AgentManager.PermissionDenied",
                Some("An agent with this ID is already registered for this user."),
                AgentManagerFailure::AlreadyRegistered,
            ),
            (
                "org.freedesktop.NetworkManager.AgentManager.PermissionDenied",
                Some("Not authorized"),
                AgentManagerFailure::Registration,
            ),
            (
                "org.freedesktop.NetworkManager.AgentManager.InvalidIdentifier",
                Some("Identifier contains invalid character ':'"),
                AgentManagerFailure::Registration,
            ),
            (
                "org.freedesktop.NetworkManager.AgentManager.NotRegistered",
                None,
                AgentManagerFailure::NotRegistered,
            ),
            (
                "org.freedesktop.DBus.Error.NoReply",
                Some("timed out"),
                AgentManagerFailure::Other,
            ),
        ];

        for (name, detail, expected) in cases {
            assert_eq!(classify_agent_manager_failure(name, detail), expected);
        }
    }

    #[test]
    fn defaults_match_networkmanager_secret_agent_contract() {
        let builder = SecretAgentBuilder::default();

        assert_eq!(builder.object_path, DEFAULT_OBJECT_PATH);
        assert_eq!(builder.identifier, DEFAULT_IDENTIFIER);
        assert_eq!(builder.capabilities, SecretAgentCapabilities::VPN_HINTS);
        assert_eq!(builder.queue_depth, DEFAULT_QUEUE_DEPTH);
    }

    #[test]
    fn builder_methods_replace_every_registration_option() {
        let builder = SecretAgent::builder()
            .with_identifier("org.example.nmrs.agent")
            .with_capabilities(SecretAgentCapabilities::empty())
            .with_object_path("/org/example/nmrs/Agent")
            .with_queue_depth(7);

        assert_eq!(builder.identifier, "org.example.nmrs.agent");
        assert_eq!(builder.capabilities, SecretAgentCapabilities::empty());
        assert_eq!(builder.object_path, "/org/example/nmrs/Agent");
        assert_eq!(builder.queue_depth, 7);
    }
}
