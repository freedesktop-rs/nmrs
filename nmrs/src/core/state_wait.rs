//! Connection state monitoring using D-Bus signals.
//!
//! Provides functions to wait for device and connection state transitions
//! using NetworkManager's signal-based API instead of polling. This approach
//! is more efficient and provides faster response times.
//!
//! # Signal-Based Monitoring
//!
//! Instead of polling device state in a loop, these functions subscribe to
//! D-Bus signals that NetworkManager emits when state changes occur:
//!
//! - `NMDevice.StateChanged` - Emitted when device state changes
//! - `NMActiveConnection.StateChanged` - Emitted when connection activation state changes
//!
//! This provides a few benefits:
//! - Immediate response to state changes (no polling delay)
//! - Lower CPU usage (no spinning loops)
//! - More reliable; at least in the sense that we won't miss rapid state transitions.
//! - Better error messages with specific failure reasons

use futures::{FutureExt, Stream, StreamExt, select};
use futures_timer::Delay;
use log::{debug, trace, warn};
use std::future::Future;
use std::pin::{Pin, pin};
use std::time::Duration;
use zbus::Connection;

use crate::Result;
use crate::api::models::{
    ActiveConnectionState, ConnectionError, ConnectionStateReason,
    connection_state_reason_to_error, reason_to_error,
};
use crate::dbus::{NMActiveConnectionProxy, NMDeviceProxy};
use crate::types::constants::{device_state, timeouts};

/// Default timeout for connection activation (30 seconds).
const CONNECTION_TIMEOUT: Duration = Duration::from_secs(30);

#[derive(Debug)]
enum ActivationDecision {
    Pending,
    Activated,
    RefineDeviceError,
    Failed(ConnectionError),
}

#[derive(Clone, Copy)]
enum WaitTarget {
    Activation,
    Disconnect,
    WifiReady,
}

fn signal_stream_ended_error(target: WaitTarget) -> ConnectionError {
    match target {
        WaitTarget::Activation | WaitTarget::Disconnect => {
            ConnectionError::Stuck("signal stream ended".into())
        }
        WaitTarget::WifiReady => ConnectionError::WifiNotReady,
    }
}

fn classify_activation_state(
    state: ActiveConnectionState,
    reason_code: Option<u32>,
) -> ActivationDecision {
    match state {
        ActiveConnectionState::Activated => ActivationDecision::Activated,
        ActiveConnectionState::Deactivated => match reason_code {
            Some(code)
                if ConnectionStateReason::from(code)
                    == ConnectionStateReason::DeviceDisconnected =>
            {
                ActivationDecision::RefineDeviceError
            }
            Some(code) => ActivationDecision::Failed(connection_state_reason_to_error(code)),
            None => ActivationDecision::RefineDeviceError,
        },
        _ => ActivationDecision::Pending,
    }
}

async fn activation_decision_result<RefineFuture>(
    decision: ActivationDecision,
    refine_error: Pin<&mut RefineFuture>,
) -> Option<Result<()>>
where
    RefineFuture: Future<Output = ConnectionError>,
{
    match decision {
        ActivationDecision::Pending => None,
        ActivationDecision::Activated => Some(Ok(())),
        ActivationDecision::RefineDeviceError => Some(Err(refine_error.await)),
        ActivationDecision::Failed(error) => Some(Err(error)),
    }
}

async fn wait_for_activation_state<S, Read, ReadFuture, RefineFuture>(
    stream: S,
    mut read_state: Read,
    refine_error: RefineFuture,
    timeout_duration: Duration,
) -> Result<()>
where
    S: Stream<Item = Option<(u32, u32)>>,
    Read: FnMut() -> ReadFuture,
    ReadFuture: Future<Output = zbus::Result<u32>>,
    RefineFuture: Future<Output = ConnectionError>,
{
    let mut stream = pin!(stream);
    let mut refine_error = pin!(refine_error);

    let current_state = ActiveConnectionState::from(read_state().await?);
    trace!("Current active connection state: {current_state}");
    if let Some(result) = activation_decision_result(
        classify_activation_state(current_state, None),
        refine_error.as_mut(),
    )
    .await
    {
        return result;
    }

    let mut timeout_delay = pin!(Delay::new(timeout_duration).fuse());
    loop {
        // A transition may race with signal subscription. Re-read before waiting.
        let current_state = ActiveConnectionState::from(read_state().await?);
        if let Some(result) = activation_decision_result(
            classify_activation_state(current_state, None),
            refine_error.as_mut(),
        )
        .await
        {
            return result;
        }

        select! {
            _ = timeout_delay => {
                // The target transition can race with the timer becoming ready.
                let final_state = ActiveConnectionState::from(read_state().await?);
                if let Some(result) = activation_decision_result(
                    classify_activation_state(final_state, None),
                    refine_error.as_mut(),
                ).await {
                    return result;
                }

                warn!("Connection activation timed out after {timeout_duration:?}");
                return Err(ConnectionError::Timeout);
            }
            signal = stream.next().fuse() => {
                match signal {
                    Some(Some((state_code, reason_code))) => {
                        let state = ActiveConnectionState::from(state_code);
                        let reason = ConnectionStateReason::from(reason_code);
                        trace!("Active connection state changed to: {state} (reason: {reason})");

                        if let Some(result) = activation_decision_result(
                            classify_activation_state(state, Some(reason_code)),
                            refine_error.as_mut(),
                        ).await {
                            return result;
                        }
                    }
                    Some(None) => {}
                    None => return Err(signal_stream_ended_error(WaitTarget::Activation)),
                }
            }
        }
    }
}

fn is_disconnected_state(state: u32) -> bool {
    state == device_state::DISCONNECTED || state == device_state::UNAVAILABLE
}

fn is_wifi_ready_state(state: u32) -> bool {
    state == device_state::DISCONNECTED || state == device_state::ACTIVATED
}

fn disconnect_timeout_result(final_state: u32) -> Result<()> {
    if is_disconnected_state(final_state) {
        Ok(())
    } else {
        Err(ConnectionError::Stuck(format!("state {final_state}")))
    }
}

fn wifi_ready_timeout_result(final_state: u32) -> Result<()> {
    if is_wifi_ready_state(final_state) {
        Ok(())
    } else {
        Err(ConnectionError::WifiNotReady)
    }
}

async fn wait_for_device_state<S, Read, ReadFuture, Target, TimeoutResult>(
    stream: S,
    mut read_state: Read,
    is_target: Target,
    timeout_duration: Duration,
    timeout_result: TimeoutResult,
    wait_target: WaitTarget,
) -> Result<()>
where
    S: Stream<Item = Option<u32>>,
    Read: FnMut() -> ReadFuture,
    ReadFuture: Future<Output = zbus::Result<u32>>,
    Target: Fn(u32) -> bool,
    TimeoutResult: Fn(u32) -> Result<()>,
{
    let mut stream = pin!(stream);

    if is_target(read_state().await?) {
        return Ok(());
    }

    let mut timeout_delay = pin!(Delay::new(timeout_duration).fuse());
    loop {
        // A transition may race with signal subscription. Re-read before waiting.
        if is_target(read_state().await?) {
            return Ok(());
        }

        select! {
            _ = timeout_delay => {
                return timeout_result(read_state().await?);
            }
            state = stream.next().fuse() => {
                match state {
                    Some(Some(state)) if is_target(state) => return Ok(()),
                    Some(_) => {}
                    None => return Err(signal_stream_ended_error(wait_target)),
                }
            }
        }
    }
}

/// When the active connection reports `DeviceDisconnected`, the real failure
/// reason lives on the device itself. Query it and return a more specific error.
async fn refine_device_disconnected_error(
    conn: &Connection,
    active_conn: &NMActiveConnectionProxy<'_>,
) -> ConnectionError {
    if let Ok(devices) = active_conn.devices().await {
        for dev_path in &devices {
            let Ok(builder) = NMDeviceProxy::builder(conn).path(dev_path.clone()) else {
                continue;
            };
            let Ok(dev) = builder.build().await else {
                continue;
            };
            if let Ok((_state, reason_code)) = dev.state_reason().await {
                debug!("Device state reason: {reason_code}");
                return reason_to_error(reason_code);
            }
        }
    }
    ConnectionError::ActivationFailed(ConnectionStateReason::DeviceDisconnected)
}

/// Default timeout for device disconnection (10 seconds).
const DISCONNECT_TIMEOUT: Duration = Duration::from_secs(10);

/// Waits for an active connection to reach the activated state.
///
/// Monitors the connection activation process by subscribing to the
/// `StateChanged` signal on the active connection object. This provides
/// more detailed error information than device-level monitoring.
///
/// # Arguments
///
/// * `conn` - D-Bus connection
/// * `active_conn_path` - Path to the active connection object
/// * `timeout` - Optional timeout duration (uses default if None)
pub(crate) async fn wait_for_connection_activation(
    conn: &Connection,
    active_conn_path: &zvariant::OwnedObjectPath,
    timeout: Option<Duration>,
) -> Result<()> {
    let active_conn = NMActiveConnectionProxy::builder(conn)
        .path(active_conn_path.clone())?
        .build()
        .await?;

    // Subscribe to signals FIRST to avoid race condition
    let stream = active_conn
        .receive_activation_state_changed()
        .await?
        .map(|signal| {
            signal
                .args()
                .map(|args| (args.state, args.reason))
                .map_err(|error| warn!("Failed to parse StateChanged signal args: {error}"))
                .ok()
        });
    trace!("Subscribed to ActiveConnection StateChanged signal");

    let timeout_duration = timeout.unwrap_or(CONNECTION_TIMEOUT);
    wait_for_activation_state(
        stream,
        || active_conn.state(),
        refine_device_disconnected_error(conn, &active_conn),
        timeout_duration,
    )
    .await
}

/// Waits for a device to reach the disconnected state using D-Bus signals.
///
/// # Arguments
///
/// * `dev` - Device proxy
/// * `timeout` - Optional timeout duration (uses default if None)
pub(crate) async fn wait_for_device_disconnect(
    dev: &NMDeviceProxy<'_>,
    timeout: Option<Duration>,
) -> Result<()> {
    // Subscribe to signals FIRST to avoid race condition
    let stream = dev.receive_device_state_changed().await?.map(|signal| {
        signal
            .args()
            .map(|args| args.new_state)
            .map_err(|error| warn!("Failed to parse StateChanged signal args: {error}"))
            .ok()
    });
    trace!("Subscribed to device StateChanged signal for disconnect");
    let timeout_duration = timeout.unwrap_or(DISCONNECT_TIMEOUT);
    wait_for_device_state(
        stream,
        || dev.state(),
        is_disconnected_state,
        timeout_duration,
        disconnect_timeout_result,
        WaitTarget::Disconnect,
    )
    .await
}

/// Waits for a Wi-Fi device to be ready (Disconnected or Activated state).
pub(crate) async fn wait_for_wifi_device_ready(dev: &NMDeviceProxy<'_>) -> Result<()> {
    // Subscribe to signals FIRST to avoid race condition
    let stream = dev.receive_device_state_changed().await?.map(|signal| {
        signal
            .args()
            .map(|args| args.new_state)
            .map_err(|error| warn!("Failed to parse StateChanged signal args: {error}"))
            .ok()
    });
    trace!("Subscribed to device StateChanged signal for ready check");
    let ready_timeout = timeouts::wifi_ready_timeout();
    wait_for_device_state(
        stream,
        || dev.state(),
        is_wifi_ready_state,
        ready_timeout,
        wifi_ready_timeout_result,
        WaitTarget::WifiReady,
    )
    .await
}

#[cfg(test)]
mod tests {
    use super::*;

    const ACTIVATING_STATE: u32 = 1;
    const ACTIVATED_STATE: u32 = 2;
    const DEACTIVATED_STATE: u32 = 4;
    const NO_SPECIFIC_REASON: u32 = 1;
    const DEVICE_DISCONNECTED_REASON: u32 = 3;
    const NO_SECRETS_REASON: u32 = 9;

    #[test]
    fn activation_states_classify_pending_and_success() {
        for state in [
            ActiveConnectionState::Unknown,
            ActiveConnectionState::Activating,
            ActiveConnectionState::Deactivating,
            ActiveConnectionState::Other(99),
        ] {
            assert!(matches!(
                classify_activation_state(state, Some(9)),
                ActivationDecision::Pending
            ));
        }

        assert!(matches!(
            classify_activation_state(ActiveConnectionState::Activated, None),
            ActivationDecision::Activated
        ));
    }

    #[test]
    fn deactivated_state_refines_device_disconnection() {
        assert!(matches!(
            classify_activation_state(ActiveConnectionState::Deactivated, None),
            ActivationDecision::RefineDeviceError
        ));
        assert!(matches!(
            classify_activation_state(ActiveConnectionState::Deactivated, Some(3)),
            ActivationDecision::RefineDeviceError
        ));
    }

    #[test]
    fn deactivated_state_maps_signal_reason_to_typed_error() {
        assert!(matches!(
            classify_activation_state(ActiveConnectionState::Deactivated, Some(9)),
            ActivationDecision::Failed(ConnectionError::AuthFailed)
        ));
        assert!(matches!(
            classify_activation_state(ActiveConnectionState::Deactivated, Some(5)),
            ActivationDecision::Failed(ConnectionError::DhcpFailed)
        ));
        assert!(matches!(
            classify_activation_state(ActiveConnectionState::Deactivated, Some(6)),
            ActivationDecision::Failed(ConnectionError::Timeout)
        ));
        assert!(matches!(
            classify_activation_state(ActiveConnectionState::Deactivated, Some(14)),
            ActivationDecision::Failed(ConnectionError::ActivationFailed(
                ConnectionStateReason::DeviceRemoved
            ))
        ));
    }

    #[test]
    fn disconnect_target_states_are_exact() {
        assert!(is_disconnected_state(device_state::DISCONNECTED));
        assert!(is_disconnected_state(device_state::UNAVAILABLE));
        assert!(!is_disconnected_state(device_state::ACTIVATED));
        assert!(!is_disconnected_state(0));
    }

    #[test]
    fn wifi_ready_target_states_are_exact() {
        assert!(is_wifi_ready_state(device_state::DISCONNECTED));
        assert!(is_wifi_ready_state(device_state::ACTIVATED));
        assert!(!is_wifi_ready_state(device_state::UNAVAILABLE));
        assert!(!is_wifi_ready_state(50));
    }

    #[test]
    fn disconnect_timeout_rechecks_final_state() {
        assert!(disconnect_timeout_result(device_state::DISCONNECTED).is_ok());
        assert!(disconnect_timeout_result(device_state::UNAVAILABLE).is_ok());
        assert!(matches!(
            disconnect_timeout_result(110),
            Err(ConnectionError::Stuck(state)) if state == "state 110"
        ));
    }

    #[test]
    fn wifi_ready_timeout_rechecks_final_state() {
        assert!(wifi_ready_timeout_result(device_state::ACTIVATED).is_ok());
        assert!(wifi_ready_timeout_result(device_state::DISCONNECTED).is_ok());
        assert!(matches!(
            wifi_ready_timeout_result(device_state::UNAVAILABLE),
            Err(ConnectionError::WifiNotReady)
        ));
    }

    #[test]
    fn closed_signal_stream_maps_to_target_specific_error() {
        for target in [WaitTarget::Activation, WaitTarget::Disconnect] {
            assert!(matches!(
                signal_stream_ended_error(target),
                ConnectionError::Stuck(message) if message == "signal stream ended"
            ));
        }
        assert!(matches!(
            signal_stream_ended_error(WaitTarget::WifiReady),
            ConnectionError::WifiNotReady
        ));
    }

    fn state_reader(
        states: std::rc::Rc<std::cell::RefCell<std::collections::VecDeque<u32>>>,
    ) -> impl FnMut() -> futures::future::Ready<zbus::Result<u32>> {
        move || {
            futures::future::ready(Ok(states
                .borrow_mut()
                .pop_front()
                .expect("test provided enough state reads")))
        }
    }

    fn run_activation_wait<S>(
        states: impl IntoIterator<Item = u32>,
        stream: S,
        refined_error: ConnectionError,
        timeout: Duration,
    ) -> Result<()>
    where
        S: Stream<Item = Option<(u32, u32)>>,
    {
        let states = std::rc::Rc::new(std::cell::RefCell::new(states.into_iter().collect()));
        futures::executor::block_on(wait_for_activation_state(
            stream,
            state_reader(states),
            futures::future::ready(refined_error),
            timeout,
        ))
    }

    #[test]
    fn activation_wait_accepts_initial_activated_state() {
        let result = run_activation_wait(
            [ACTIVATED_STATE],
            futures::stream::pending(),
            ConnectionError::ActivationFailed(ConnectionStateReason::DeviceDisconnected),
            Duration::from_secs(1),
        );

        assert!(matches!(result, Ok(())));
    }

    #[test]
    fn activation_wait_observes_state_that_raced_with_subscription() {
        let result = run_activation_wait(
            [ACTIVATING_STATE, ACTIVATED_STATE],
            futures::stream::pending(),
            ConnectionError::ActivationFailed(ConnectionStateReason::DeviceDisconnected),
            Duration::from_secs(1),
        );

        assert!(matches!(result, Ok(())));
    }

    #[test]
    fn activation_wait_maps_signal_reason_to_typed_error() {
        let result = run_activation_wait(
            [ACTIVATING_STATE, ACTIVATING_STATE],
            futures::stream::iter([Some((DEACTIVATED_STATE, NO_SECRETS_REASON))]),
            ConnectionError::DhcpFailed,
            Duration::from_secs(1),
        );

        assert!(matches!(result, Err(ConnectionError::AuthFailed)));
    }

    #[test]
    fn activation_wait_uses_refined_device_error() {
        let result = run_activation_wait(
            [ACTIVATING_STATE, ACTIVATING_STATE],
            futures::stream::iter([Some((DEACTIVATED_STATE, DEVICE_DISCONNECTED_REASON))]),
            ConnectionError::ActivationFailed(ConnectionStateReason::DeviceRemoved),
            Duration::from_secs(1),
        );

        assert!(matches!(
            result,
            Err(ConnectionError::ActivationFailed(
                ConnectionStateReason::DeviceRemoved
            ))
        ));
    }

    #[test]
    fn activation_wait_refines_initial_deactivated_state() {
        let result = run_activation_wait(
            [DEACTIVATED_STATE],
            futures::stream::pending(),
            ConnectionError::DhcpFailed,
            Duration::from_secs(1),
        );

        assert!(matches!(result, Err(ConnectionError::DhcpFailed)));
    }

    #[test]
    fn activation_wait_ignores_malformed_signal_then_accepts_success() {
        let result = run_activation_wait(
            [ACTIVATING_STATE, ACTIVATING_STATE, ACTIVATING_STATE],
            futures::stream::iter([None, Some((ACTIVATED_STATE, NO_SPECIFIC_REASON))]),
            ConnectionError::ActivationFailed(ConnectionStateReason::DeviceDisconnected),
            Duration::from_secs(1),
        );

        assert!(matches!(result, Ok(())));
    }

    #[test]
    fn activation_wait_reports_closed_signal_stream() {
        let result = run_activation_wait(
            [ACTIVATING_STATE, ACTIVATING_STATE],
            futures::stream::empty(),
            ConnectionError::ActivationFailed(ConnectionStateReason::DeviceDisconnected),
            Duration::from_secs(1),
        );

        assert!(matches!(
            result,
            Err(ConnectionError::Stuck(message)) if message == "signal stream ended"
        ));
    }

    #[test]
    fn activation_wait_timeout_rechecks_final_activated_state() {
        let result = run_activation_wait(
            [ACTIVATING_STATE, ACTIVATING_STATE, ACTIVATED_STATE],
            futures::stream::pending(),
            ConnectionError::ActivationFailed(ConnectionStateReason::DeviceDisconnected),
            Duration::ZERO,
        );

        assert!(matches!(result, Ok(())));
    }

    #[test]
    fn activation_wait_timeout_reports_final_pending_state() {
        let result = run_activation_wait(
            [ACTIVATING_STATE, ACTIVATING_STATE, ACTIVATING_STATE],
            futures::stream::pending(),
            ConnectionError::ActivationFailed(ConnectionStateReason::DeviceDisconnected),
            Duration::ZERO,
        );

        assert!(matches!(result, Err(ConnectionError::Timeout)));
    }

    #[test]
    fn activation_wait_propagates_state_read_error() {
        let result = futures::executor::block_on(wait_for_activation_state(
            futures::stream::pending::<Option<(u32, u32)>>(),
            || futures::future::ready(Err(zbus::Error::Failure("state read failed".into()))),
            futures::future::ready(ConnectionError::DhcpFailed),
            Duration::from_secs(1),
        ));

        assert!(matches!(
            result,
            Err(ConnectionError::Dbus(zbus::Error::Failure(message)))
                if message == "state read failed"
        ));
    }

    #[test]
    fn device_wait_observes_state_that_raced_with_subscription() {
        let states = std::rc::Rc::new(std::cell::RefCell::new(
            [50, device_state::DISCONNECTED].into(),
        ));
        let stream = futures::stream::pending::<Option<u32>>();

        let result = futures::executor::block_on(wait_for_device_state(
            stream,
            state_reader(states),
            is_disconnected_state,
            Duration::from_secs(1),
            disconnect_timeout_result,
            WaitTarget::Disconnect,
        ));

        assert!(matches!(result, Ok(())));
    }

    #[test]
    fn device_wait_handles_malformed_then_terminal_signal() {
        let states = std::rc::Rc::new(std::cell::RefCell::new([50, 50, 50].into()));
        let stream = futures::stream::iter([None, Some(device_state::DISCONNECTED)]);

        let result = futures::executor::block_on(wait_for_device_state(
            stream,
            state_reader(states),
            is_disconnected_state,
            Duration::from_secs(1),
            disconnect_timeout_result,
            WaitTarget::Disconnect,
        ));

        assert!(matches!(result, Ok(())));
    }

    #[test]
    fn device_wait_reports_closed_stream() {
        let states = std::rc::Rc::new(std::cell::RefCell::new([50, 50].into()));
        let stream = futures::stream::empty::<Option<u32>>();

        let result = futures::executor::block_on(wait_for_device_state(
            stream,
            state_reader(states),
            is_disconnected_state,
            Duration::from_secs(1),
            disconnect_timeout_result,
            WaitTarget::Disconnect,
        ));

        assert!(matches!(
            result,
            Err(ConnectionError::Stuck(message)) if message == "signal stream ended"
        ));
    }

    #[test]
    fn device_wait_timeout_uses_final_state_recheck() {
        let states = std::rc::Rc::new(std::cell::RefCell::new(
            [50, 50, device_state::DISCONNECTED].into(),
        ));
        let stream = futures::stream::pending::<Option<u32>>();

        let result = futures::executor::block_on(wait_for_device_state(
            stream,
            state_reader(states),
            is_disconnected_state,
            Duration::ZERO,
            disconnect_timeout_result,
            WaitTarget::Disconnect,
        ));

        assert!(matches!(result, Ok(())));
    }

    #[test]
    fn device_wait_timeout_reports_final_non_target_state() {
        let states = std::rc::Rc::new(std::cell::RefCell::new([50, 50, 110].into()));
        let stream = futures::stream::pending::<Option<u32>>();

        let result = futures::executor::block_on(wait_for_device_state(
            stream,
            state_reader(states),
            is_disconnected_state,
            Duration::ZERO,
            disconnect_timeout_result,
            WaitTarget::Disconnect,
        ));

        assert!(matches!(
            result,
            Err(ConnectionError::Stuck(message)) if message == "state 110"
        ));
    }

    #[test]
    fn device_wait_propagates_state_read_error() {
        let stream = futures::stream::pending::<Option<u32>>();

        let result = futures::executor::block_on(wait_for_device_state(
            stream,
            || futures::future::ready(Err(zbus::Error::Failure("state read failed".into()))),
            is_disconnected_state,
            Duration::from_secs(1),
            disconnect_timeout_result,
            WaitTarget::Disconnect,
        ));

        assert!(matches!(
            result,
            Err(ConnectionError::Dbus(zbus::Error::Failure(message)))
                if message == "state read failed"
        ));
    }
}
