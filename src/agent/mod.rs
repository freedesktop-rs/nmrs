//! NetworkManager secret agent for credential prompting over D-Bus.
//!
//! When NetworkManager needs credentials it does not already have — a Wi-Fi
//! password was forgotten, a VPN token expired, an 802.1X password is required
//! — it calls every registered **secret agent** via D-Bus. This module lets
//! `nmrs` consumers register such an agent and respond to those requests
//! without touching raw D-Bus.
//!
//! # Three-stream model
//!
//! [`SecretAgentBuilder::register()`](crate::agent::SecretAgentBuilder::register)
//! returns a handle and three logical streams:
//!
//! 1. **Request stream** — the primary
//!    [`mpsc::Receiver<SecretRequest>`](futures::channel::mpsc::Receiver)
//!    returned alongside the handle. Each item is a credential prompt from
//!    NetworkManager. Respond through the attached
//!    [`SecretResponder`](crate::agent::SecretResponder).
//!
//! 2. **Cancellation stream** — accessed via
//!    [`SecretAgentHandle::cancellations()`](crate::agent::SecretAgentHandle::cancellations). Yields
//!    [`CancelReason`](crate::agent::CancelReason) items when
//!    NetworkManager aborts a pending request. The agent replies to
//!    NetworkManager automatically; this stream exists so the consumer can
//!    tear down any UI it may have shown.
//!
//! 3. **Store event stream** — accessed via
//!    [`SecretAgentHandle::store_events()`](crate::agent::SecretAgentHandle::store_events). Yields
//!    [`SecretStoreEvent`](crate::agent::SecretStoreEvent) items when
//!    NetworkManager asks the agent to save or delete persisted secrets.
//!    Since `nmrs` delegates persistence to the consumer, these events are
//!    optional and the agent always acknowledges them.
//!
//! # Lifecycle
//!
//! ```text
//! SecretAgent::builder()
//!     .with_identifier("com.example.MyApp")
//!     .register().await?
//!         │
//!         ├── (SecretAgentHandle, request stream)
//!         │
//!         │   ┌──────── consumer loop ────────┐
//!         │   │ while let Some(req) = rx … {  │
//!         │   │   req.responder.wifi_psk(…)   │
//!         │   │ }                             │
//!         │   └───────────────────────────────┘
//!         │
//!         └── handle.unregister().await?
//! ```
//!
//! The identifier is only passed to NetworkManager; it is not a D-Bus
//! well-known name. The agent object is served at NetworkManager's standard
//! `/org/freedesktop/NetworkManager/SecretAgent` path by default.
//!
//! GUI apps should register one long-lived agent during application or applet
//! startup and keep the returned [`SecretAgentHandle`](crate::agent::SecretAgentHandle)
//! alive for the process lifetime. Dropping or unregistering the handle tears
//! down the registration and closes the request stream.
//!
//! If NetworkManager restarts while the agent is running, call
//! [`SecretAgentHandle::reregister()`](crate::agent::SecretAgentHandle::reregister)
//! after detecting that NetworkManager is available again.
//!
//! # Example
//!
//! ```no_run
//! use futures::StreamExt;
//! use nmrs::agent::{SecretAgent, SecretAgentFlags, SecretSetting};
//!
//! # async fn run() -> nmrs::Result<()> {
//! let (handle, mut requests) = SecretAgent::builder()
//!     .with_identifier("com.example.demo")
//!     .register()
//!     .await?;
//!
//! let request_task = tokio::spawn(async move {
//!     while let Some(req) = requests.next().await {
//!         if !req.flags.contains(SecretAgentFlags::ALLOW_INTERACTION) {
//!             req.responder.no_secrets().await?;
//!             continue;
//!         }
//!
//!         match req.setting {
//!             SecretSetting::WifiPsk { ref ssid } => {
//!                 println!("Password needed for {ssid}");
//!                 req.responder.wifi_psk("secret").await?;
//!             }
//!             _ => req.responder.cancel().await?,
//!         }
//!     }
//!
//!     Ok::<(), nmrs::ConnectionError>(())
//! });
//!
//! // Keep `handle` alive for as long as the app should answer prompts.
//! // After a NetworkManager restart, call:
//! // handle.reregister().await?;
//!
//! request_task.abort();
//! handle.unregister().await?;
//! # Ok(())
//! # }
//! ```
//!
//! A foreground loop works too:
//!
//! ```no_run
//! use futures::StreamExt;
//! use nmrs::agent::{SecretAgent, SecretAgentFlags, SecretSetting};
//!
//! # async fn run() -> nmrs::Result<()> {
//! let (handle, mut requests) = SecretAgent::builder()
//!     .with_identifier("com.example.demo")
//!     .register()
//!     .await?;
//!
//! while let Some(req) = requests.next().await {
//!     if !req.flags.contains(SecretAgentFlags::ALLOW_INTERACTION) {
//!         req.responder.no_secrets().await?;
//!         continue;
//!     }
//!     match req.setting {
//!         SecretSetting::WifiPsk { ref ssid } => {
//!             println!("Password needed for {ssid}");
//!             req.responder.wifi_psk("secret").await?;
//!         }
//!         _ => req.responder.cancel().await?,
//!     }
//! }
//!
//! handle.unregister().await?;
//! # Ok(())
//! # }
//! ```

mod builder;
pub(crate) mod iface;
mod request;

pub use builder::{SecretAgent, SecretAgentBuilder, SecretAgentHandle};
pub use request::{
    CancelReason, SecretAgentCapabilities, SecretAgentFlags, SecretRequest, SecretResponder,
    SecretSetting, SecretStoreEvent,
};

/// Type alias so agent consumers only need one error type.
pub type AgentError = crate::ConnectionError;
