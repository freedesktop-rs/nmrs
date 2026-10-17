//! D-Bus proxy for the NetworkManager AgentManager interface.

use zbus::proxy;

/// Proxy for the NetworkManager AgentManager interface.
///
/// Used to register and unregister secret agents with NetworkManager.
///
/// Reference: <https://networkmanager.dev/docs/api/latest/gdbus-org.freedesktop.NetworkManager.AgentManager.html>
#[proxy(
    interface = "org.freedesktop.NetworkManager.AgentManager",
    default_service = "org.freedesktop.NetworkManager",
    default_path = "/org/freedesktop/NetworkManager/AgentManager"
)]
pub trait AgentManager {
    /// Registers this secret agent with the given capabilities.
    ///
    /// The `identifier` is a reverse-DNS string identifying this agent
    /// (e.g. `"com.system76.CosmicApplets.nmrs.secret_agent"`).
    ///
    /// `capabilities` is a bitmask of `NMSecretAgentCapabilities`:
    /// - `0x0` = none
    /// - `0x1` = `VPN_HINTS` (agent can filter VPN secret hints)
    fn register_with_capabilities(&self, identifier: &str, capabilities: u32) -> zbus::Result<()>;

    /// Unregisters the secret agent from NetworkManager.
    fn unregister(&self) -> zbus::Result<()>;
}
