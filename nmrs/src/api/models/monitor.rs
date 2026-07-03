use std::mem::ManuallyDrop;

use tokio::sync::watch;
use tokio::task::JoinHandle;

use super::ConnectionError;
use crate::Result;

/// A handle to a running monitor task.
///
/// Returned by [`NetworkManager::monitor_network_changes`] and
/// [`NetworkManager::monitor_device_changes`]. The handle lets callers
/// shut the monitor down gracefully instead of having to abort the task.
///
/// Dropping the handle triggers shutdown automatically.
///
/// [`NetworkManager::monitor_network_changes`]: crate::NetworkManager::monitor_network_changes
/// [`NetworkManager::monitor_device_changes`]: crate::NetworkManager::monitor_device_changes
///
/// # Example
///
/// ```ignore
/// # use nmrs::NetworkManager;
/// # async fn example() -> nmrs::Result<()> {
/// let nm = NetworkManager::new().await?;
///
/// let handle = nm.monitor_network_changes(|| {
///     println!("Networks changed!");
/// }).await?;
///
/// // ... later, when you want to stop monitoring:
/// handle.stop().await?;
/// # Ok(())
/// # }
/// ```
#[non_exhaustive]
pub struct MonitorHandle {
    shutdown_tx: watch::Sender<()>,
    task: ManuallyDrop<JoinHandle<Result<()>>>,
}

impl MonitorHandle {
    pub(crate) fn new(shutdown_tx: watch::Sender<()>, task: JoinHandle<Result<()>>) -> Self {
        Self {
            shutdown_tx,
            task: ManuallyDrop::new(task),
        }
    }

    /// Signals the monitor to stop and waits for it to finish.
    ///
    /// Returns `Ok(())` on a clean shutdown, or the error that caused the
    /// monitor to exit early.
    pub async fn stop(mut self) -> Result<()> {
        let _ = self.shutdown_tx.send(());
        // SAFETY: we consume `self` so `drop` won't run and touch the field again.
        let task = unsafe { ManuallyDrop::take(&mut self.task) };
        std::mem::forget(self);
        task.await
            .map_err(|e| ConnectionError::Stuck(format!("monitor task panicked: {e}")))?
    }

    /// Signals the monitor to stop without waiting for it to finish.
    pub fn shutdown(&self) {
        let _ = self.shutdown_tx.send(());
    }
}

impl Drop for MonitorHandle {
    fn drop(&mut self) {
        let _ = self.shutdown_tx.send(());
    }
}
