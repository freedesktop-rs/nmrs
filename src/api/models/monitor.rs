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

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use super::*;

    fn task_waiting_for_shutdown(mut shutdown_rx: watch::Receiver<()>) -> JoinHandle<Result<()>> {
        tokio::spawn(async move {
            shutdown_rx
                .changed()
                .await
                .map_err(|error| ConnectionError::Stuck(error.to_string()))?;
            Ok(())
        })
    }

    async fn expect_shutdown(shutdown_rx: &mut watch::Receiver<()>) {
        tokio::time::timeout(Duration::from_secs(1), shutdown_rx.changed())
            .await
            .expect("shutdown signal timed out")
            .expect("shutdown sender dropped without signaling");
    }

    #[tokio::test]
    async fn stop_signals_and_waits_for_clean_task_exit() {
        let (shutdown_tx, shutdown_rx) = watch::channel(());
        let handle = MonitorHandle::new(shutdown_tx, task_waiting_for_shutdown(shutdown_rx));

        handle.stop().await.unwrap();
    }

    #[tokio::test]
    async fn stop_propagates_monitor_task_error() {
        let (shutdown_tx, _shutdown_rx) = watch::channel(());
        let task = tokio::spawn(async {
            Err(ConnectionError::Stuck(
                "monitor returned its own error".into(),
            ))
        });
        let handle = MonitorHandle::new(shutdown_tx, task);

        let error = handle.stop().await.unwrap_err();
        assert!(matches!(
            error,
            ConnectionError::Stuck(message) if message == "monitor returned its own error"
        ));
    }

    #[tokio::test]
    async fn stop_maps_panicked_task_to_stuck_error() {
        let (shutdown_tx, _shutdown_rx) = watch::channel(());
        let task = tokio::spawn(async {
            panic!("monitor task test panic");
            #[allow(unreachable_code)]
            Ok(())
        });
        let handle = MonitorHandle::new(shutdown_tx, task);

        let error = handle.stop().await.unwrap_err();
        assert!(matches!(
            error,
            ConnectionError::Stuck(message)
                if message.contains("monitor task panicked")
                    && message.contains("monitor task test panic")
        ));
    }

    #[tokio::test]
    async fn shutdown_sends_signal_without_consuming_handle() {
        let (shutdown_tx, shutdown_rx) = watch::channel(());
        let mut observer = shutdown_rx.clone();
        let handle = MonitorHandle::new(shutdown_tx, task_waiting_for_shutdown(shutdown_rx));

        handle.shutdown();
        expect_shutdown(&mut observer).await;
        handle.stop().await.unwrap();
    }

    #[tokio::test]
    async fn drop_sends_shutdown_signal() {
        let (shutdown_tx, shutdown_rx) = watch::channel(());
        let mut observer = shutdown_rx.clone();
        let handle = MonitorHandle::new(shutdown_tx, task_waiting_for_shutdown(shutdown_rx));

        drop(handle);

        expect_shutdown(&mut observer).await;
    }
}
