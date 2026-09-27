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
//! During activation both are watched at once. NetworkManager reports a
//! failed activation on the active connection only as `DeviceDisconnected`;
//! the specific reason (wrong passphrase, missing SSID, DHCP failure, ...)
//! is carried by the device's `StateChanged` signal on its way into `FAILED`
//! and is captured from there.
//!
//! This provides a few benefits:
//! - Immediate response to state changes (no polling delay)
//! - Lower CPU usage (no spinning loops)
//! - More reliable; at least in the sense that we won't miss rapid state transitions.
//! - Better error messages with specific failure reasons

use futures::stream::{FusedStream, SelectAll};
use futures::{FutureExt, Stream, StreamExt, select};
use futures_timer::Delay;
use log::{debug, trace, warn};
use std::future::Future;
use std::pin::{Pin, pin};
use std::time::Duration;
use zbus::Connection;
use zbus::proxy::CacheProperties;

use crate::Result;
use crate::api::models::{
    ActiveConnectionState, ConnectionError, ConnectionStateReason, StateReason,
    connection_state_reason_to_error, reason_to_error,
};
use crate::dbus::{NMActiveConnectionProxy, NMDeviceProxy};
use crate::types::constants::{device_state, timeouts};

/// Default timeout for connection activation (30 seconds).
const CONNECTION_TIMEOUT: Duration = Duration::from_secs(30);

/// A device `StateChanged` signal reduced to `(new_state, reason)`.
///
/// `None` marks a signal whose arguments could not be parsed.
type DeviceTransition = Option<(u32, u32)>;

/// Merged `StateChanged` signals from every device of an active connection.
type DeviceTransitionStream = SelectAll<Pin<Box<dyn Stream<Item = DeviceTransition> + Send>>>;

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

/// Remembers the reason a device reported when it entered `FAILED`.
///
/// NetworkManager emits the device's `StateChanged(FAILED, _, reason)` signal
/// before it deactivates the active connection, then immediately queues a
/// `FAILED -> DISCONNECTED` transition with reason `NONE`. That follow-up
/// overwrites the device's `StateReason` property, so the signal is the only
/// reliable carrier of the real failure reason.
fn record_device_failure(failure_reason: &mut Option<u32>, transition: DeviceTransition) {
    if let Some((new_state, reason)) = transition
        && new_state == device_state::FAILED
    {
        trace!("Device entered FAILED state (reason: {reason})");
        *failure_reason = Some(reason);
    }
}

/// Picks up device signals that are already queued, without waiting for more.
///
/// The device's failure signal is on the bus before the active connection's
/// `Deactivated` signal, so it is buffered by the time the latter is handled.
/// `select!` may still have polled the active connection stream first; this
/// makes sure the buffered device signal is not overlooked.
fn drain_device_transitions<D>(mut device_stream: Pin<&mut D>, failure_reason: &mut Option<u32>)
where
    D: FusedStream<Item = DeviceTransition>,
{
    loop {
        match device_stream.next().now_or_never() {
            Some(Some(transition)) => record_device_failure(failure_reason, transition),
            // Nothing queued right now, or every device stream has ended.
            Some(None) | None => return,
        }
    }
}

async fn activation_decision_result<D, Refine, RefineFuture>(
    decision: ActivationDecision,
    device_stream: Pin<&mut D>,
    failure_reason: &mut Option<u32>,
    refine_error: &mut Refine,
) -> Option<Result<()>>
where
    D: FusedStream<Item = DeviceTransition>,
    Refine: FnMut(Option<u32>) -> RefineFuture,
    RefineFuture: Future<Output = ConnectionError>,
{
    match decision {
        ActivationDecision::Pending => None,
        ActivationDecision::Activated => Some(Ok(())),
        ActivationDecision::RefineDeviceError => {
            drain_device_transitions(device_stream, failure_reason);
            Some(Err(refine_error(*failure_reason).await))
        }
        ActivationDecision::Failed(error) => Some(Err(error)),
    }
}

async fn wait_for_activation_state<S, D, Read, ReadFuture, Refine, RefineFuture>(
    stream: S,
    device_stream: D,
    mut read_state: Read,
    mut refine_error: Refine,
    timeout_duration: Duration,
) -> Result<()>
where
    S: Stream<Item = Option<(u32, u32)>>,
    D: Stream<Item = DeviceTransition>,
    Read: FnMut() -> ReadFuture,
    ReadFuture: Future<Output = zbus::Result<u32>>,
    Refine: FnMut(Option<u32>) -> RefineFuture,
    RefineFuture: Future<Output = ConnectionError>,
{
    let mut stream = pin!(stream);
    let mut device_stream = pin!(device_stream.fuse());
    let mut failure_reason = None;

    let current_state = ActiveConnectionState::from(read_state().await?);
    trace!("Current active connection state: {current_state}");
    if let Some(result) = activation_decision_result(
        classify_activation_state(current_state, None),
        device_stream.as_mut(),
        &mut failure_reason,
        &mut refine_error,
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
            device_stream.as_mut(),
            &mut failure_reason,
            &mut refine_error,
        )
        .await
        {
            return result;
        }

        let decision = select! {
            _ = timeout_delay => {
                // The target transition can race with the timer becoming ready.
                let final_state = ActiveConnectionState::from(read_state().await?);
                match classify_activation_state(final_state, None) {
                    ActivationDecision::Pending => {
                        warn!("Connection activation timed out after {timeout_duration:?}");
                        return Err(ConnectionError::Timeout);
                    }
                    decision => decision,
                }
            }
            signal = stream.next().fuse() => {
                match signal {
                    Some(Some((state_code, reason_code))) => {
                        let state = ActiveConnectionState::from(state_code);
                        let reason = ConnectionStateReason::from(reason_code);
                        trace!("Active connection state changed to: {state} (reason: {reason})");
                        classify_activation_state(state, Some(reason_code))
                    }
                    Some(None) => ActivationDecision::Pending,
                    None => return Err(signal_stream_ended_error(WaitTarget::Activation)),
                }
            }
            transition = device_stream.next() => {
                // `None` means every device stream ended. The fused stream stays
                // quiet from here on; activation is still tracked through the
                // active connection.
                if let Some(transition) = transition {
                    record_device_failure(&mut failure_reason, transition);
                }
                ActivationDecision::Pending
            }
        };

        if let Some(result) = activation_decision_result(
            decision,
            device_stream.as_mut(),
            &mut failure_reason,
            &mut refine_error,
        )
        .await
        {
            return result;
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

/// Resolves proxies for the devices backing an active connection.
///
/// Property caching is disabled so that the `StateReason` fallback read in
/// [`refine_device_disconnected_error`] reflects the device's state at that
/// moment rather than a value cached before activation started.
async fn connection_devices(
    conn: &Connection,
    active_conn: &NMActiveConnectionProxy<'_>,
) -> Vec<NMDeviceProxy<'static>> {
    let device_paths = match active_conn.devices().await {
        Ok(paths) => paths,
        Err(error) => {
            warn!("Failed to read the active connection's devices: {error}");
            return Vec::new();
        }
    };

    let mut devices = Vec::with_capacity(device_paths.len());
    for dev_path in device_paths {
        let Ok(builder) = NMDeviceProxy::builder(conn).path(dev_path.clone()) else {
            continue;
        };
        match builder.cache_properties(CacheProperties::No).build().await {
            Ok(dev) => devices.push(dev),
            Err(error) => warn!("Failed to build device proxy for {dev_path}: {error}"),
        }
    }
    devices
}

/// Subscribes to `StateChanged` on each device and merges the signals.
///
/// A device whose subscription fails is skipped with a warning; its failure
/// reason can still be picked up by the `StateReason` fallback read.
async fn device_transition_stream(devices: &[NMDeviceProxy<'_>]) -> DeviceTransitionStream {
    let mut streams = SelectAll::new();
    for dev in devices {
        let path = dev.inner().path();
        match dev.receive_device_state_changed().await {
            Ok(signals) => {
                trace!("Subscribed to device StateChanged signal on {path}");
                streams.push(Box::pin(signals.map(|signal| {
                    signal
                        .args()
                        .map(|args| (args.new_state, args.reason))
                        .map_err(|error| {
                            warn!("Failed to parse device StateChanged signal args: {error}");
                        })
                        .ok()
                }))
                    as Pin<Box<dyn Stream<Item = DeviceTransition> + Send>>);
            }
            Err(error) => {
                warn!("Failed to subscribe to device StateChanged signal on {path}: {error}");
            }
        }
    }
    if streams.is_empty() {
        // An empty `SelectAll` ends immediately. Keep the merged stream open so
        // "no devices" is not treated as a terminated stream on every poll.
        streams.push(Box::pin(futures::stream::pending()));
    }
    streams
}

/// When the active connection reports `DeviceDisconnected`, the real failure
/// reason lives on the device itself.
///
/// The reason captured from the device's `StateChanged` signal on its way into
/// `FAILED` is preferred. The `StateReason` property is only consulted when no
/// such signal was seen, because NetworkManager queues a follow-up
/// `DISCONNECTED` transition with reason `NONE` that overwrites the property.
async fn refine_device_disconnected_error(
    devices: &[NMDeviceProxy<'_>],
    failure_reason: Option<u32>,
) -> ConnectionError {
    if let Some(reason_code) = failure_reason {
        debug!("Device failure reason from StateChanged signal: {reason_code}");
        return reason_to_error(reason_code);
    }

    for dev in devices {
        let Ok((state, reason_code)) = dev.state_reason().await else {
            continue;
        };
        debug!("Device StateReason property: state {state}, reason {reason_code}");
        if StateReason::from(reason_code) == StateReason::None {
            // A later transition already cleared the reason; there is nothing
            // more specific to report than the active connection's own reason.
            continue;
        }
        return reason_to_error(reason_code);
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

    // Subscribe to signals FIRST to avoid race conditions. Device signals go
    // first: NetworkManager emits `Device.StateChanged(FAILED)` before it
    // emits `ActiveConnection.StateChanged(DEACTIVATED)`.
    let devices = connection_devices(conn, &active_conn).await;
    let device_stream = device_transition_stream(&devices).await;
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
        device_stream,
        || active_conn.state(),
        |failure_reason| refine_device_disconnected_error(&devices, failure_reason),
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
    // NMDeviceState / NMDeviceStateReason codes as seen on the device signal.
    const DEVICE_CONFIG_STATE: u32 = 50;
    const DEVICE_REASON_NONE: u32 = 0;
    const DEVICE_REASON_NO_SECRETS: u32 = 7;
    const DEVICE_REASON_SSID_NOT_FOUND: u32 = 53;

    fn fallback_error() -> ConnectionError {
        ConnectionError::ActivationFailed(ConnectionStateReason::DeviceDisconnected)
    }

    /// Mirrors the production refinement: a captured device reason wins,
    /// otherwise the fallback is used.
    fn refine_with_fallback(
        fallback: fn() -> ConnectionError,
    ) -> impl FnMut(Option<u32>) -> ConnectionError {
        move |reason| reason.map_or_else(fallback, reason_to_error)
    }

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

    fn run_activation_wait<S, D>(
        states: impl IntoIterator<Item = u32>,
        stream: S,
        device_stream: D,
        mut refined_error: impl FnMut(Option<u32>) -> ConnectionError,
        timeout: Duration,
    ) -> Result<()>
    where
        S: Stream<Item = Option<(u32, u32)>>,
        D: Stream<Item = DeviceTransition>,
    {
        let states = std::rc::Rc::new(std::cell::RefCell::new(states.into_iter().collect()));
        futures::executor::block_on(wait_for_activation_state(
            stream,
            device_stream,
            state_reader(states),
            |failure_reason| futures::future::ready(refined_error(failure_reason)),
            timeout,
        ))
    }

    #[test]
    fn activation_wait_accepts_initial_activated_state() {
        let result = run_activation_wait(
            [ACTIVATED_STATE],
            futures::stream::pending(),
            futures::stream::pending(),
            |_| fallback_error(),
            Duration::from_secs(1),
        );

        assert!(matches!(result, Ok(())));
    }

    #[test]
    fn activation_wait_observes_state_that_raced_with_subscription() {
        let result = run_activation_wait(
            [ACTIVATING_STATE, ACTIVATED_STATE],
            futures::stream::pending(),
            futures::stream::pending(),
            |_| fallback_error(),
            Duration::from_secs(1),
        );

        assert!(matches!(result, Ok(())));
    }

    #[test]
    fn activation_wait_maps_signal_reason_to_typed_error() {
        let result = run_activation_wait(
            [ACTIVATING_STATE, ACTIVATING_STATE],
            futures::stream::iter([Some((DEACTIVATED_STATE, NO_SECRETS_REASON))]),
            futures::stream::pending(),
            |_| ConnectionError::DhcpFailed,
            Duration::from_secs(1),
        );

        assert!(matches!(result, Err(ConnectionError::AuthFailed)));
    }

    #[test]
    fn activation_wait_uses_refined_device_error() {
        let result = run_activation_wait(
            [ACTIVATING_STATE, ACTIVATING_STATE],
            futures::stream::iter([Some((DEACTIVATED_STATE, DEVICE_DISCONNECTED_REASON))]),
            futures::stream::pending(),
            |failure_reason| {
                assert_eq!(failure_reason, None, "no device failure was signalled");
                ConnectionError::ActivationFailed(ConnectionStateReason::DeviceRemoved)
            },
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
    fn activation_wait_uses_device_failure_reason_signalled_before_deactivation() {
        // The device reports FAILED first; the active connection only follows
        // up with the generic DeviceDisconnected a little later.
        let active_connection_signals = futures::stream::once(
            Delay::new(Duration::from_millis(20))
                .map(|_| Some((DEACTIVATED_STATE, DEVICE_DISCONNECTED_REASON))),
        );
        let device_signals =
            futures::stream::iter([Some((device_state::FAILED, DEVICE_REASON_NO_SECRETS))]);

        let result = run_activation_wait(
            [ACTIVATING_STATE; 4],
            active_connection_signals,
            device_signals,
            refine_with_fallback(fallback_error),
            Duration::from_secs(1),
        );

        assert!(matches!(result, Err(ConnectionError::AuthFailed)));
    }

    #[test]
    fn activation_wait_uses_device_failure_reason_delivered_alongside_deactivation() {
        // Both signals are already queued when the wait starts, so whichever
        // stream is polled first the device reason must still be honoured.
        let result = run_activation_wait(
            [ACTIVATING_STATE; 4],
            futures::stream::iter([Some((DEACTIVATED_STATE, DEVICE_DISCONNECTED_REASON))]),
            futures::stream::iter([Some((device_state::FAILED, DEVICE_REASON_SSID_NOT_FOUND))]),
            refine_with_fallback(fallback_error),
            Duration::from_secs(1),
        );

        assert!(matches!(result, Err(ConnectionError::NotFound)));
    }

    #[test]
    fn activation_wait_keeps_failure_reason_after_device_settles_to_disconnected() {
        // NetworkManager follows FAILED with DISCONNECTED (reason NONE). The
        // original failure reason must survive that follow-up.
        let result = run_activation_wait(
            [ACTIVATING_STATE; 5],
            futures::stream::iter([Some((DEACTIVATED_STATE, DEVICE_DISCONNECTED_REASON))]),
            futures::stream::iter([
                Some((device_state::FAILED, DEVICE_REASON_NO_SECRETS)),
                Some((device_state::DISCONNECTED, DEVICE_REASON_NONE)),
            ]),
            refine_with_fallback(fallback_error),
            Duration::from_secs(1),
        );

        assert!(matches!(result, Err(ConnectionError::AuthFailed)));
    }

    #[test]
    fn activation_wait_ignores_device_transitions_that_are_not_failures() {
        let result = run_activation_wait(
            [ACTIVATING_STATE; 5],
            futures::stream::iter([Some((DEACTIVATED_STATE, DEVICE_DISCONNECTED_REASON))]),
            futures::stream::iter([None, Some((DEVICE_CONFIG_STATE, DEVICE_REASON_NONE))]),
            refine_with_fallback(fallback_error),
            Duration::from_secs(1),
        );

        assert!(matches!(
            result,
            Err(ConnectionError::ActivationFailed(
                ConnectionStateReason::DeviceDisconnected
            ))
        ));
    }

    #[test]
    fn activation_wait_survives_device_stream_ending() {
        let result = run_activation_wait(
            [ACTIVATING_STATE; 4],
            futures::stream::iter([Some((ACTIVATED_STATE, NO_SPECIFIC_REASON))]),
            futures::stream::empty(),
            refine_with_fallback(fallback_error),
            Duration::from_secs(1),
        );

        assert!(matches!(result, Ok(())));
    }

    #[test]
    fn activation_wait_refines_initial_deactivated_state() {
        let result = run_activation_wait(
            [DEACTIVATED_STATE],
            futures::stream::pending(),
            futures::stream::pending(),
            |_| ConnectionError::DhcpFailed,
            Duration::from_secs(1),
        );

        assert!(matches!(result, Err(ConnectionError::DhcpFailed)));
    }

    #[test]
    fn activation_wait_ignores_malformed_signal_then_accepts_success() {
        let result = run_activation_wait(
            [ACTIVATING_STATE, ACTIVATING_STATE, ACTIVATING_STATE],
            futures::stream::iter([None, Some((ACTIVATED_STATE, NO_SPECIFIC_REASON))]),
            futures::stream::pending(),
            |_| fallback_error(),
            Duration::from_secs(1),
        );

        assert!(matches!(result, Ok(())));
    }

    #[test]
    fn activation_wait_reports_closed_signal_stream() {
        let result = run_activation_wait(
            [ACTIVATING_STATE, ACTIVATING_STATE],
            futures::stream::empty(),
            futures::stream::pending(),
            |_| fallback_error(),
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
            futures::stream::pending(),
            |_| fallback_error(),
            Duration::ZERO,
        );

        assert!(matches!(result, Ok(())));
    }

    #[test]
    fn activation_wait_timeout_reports_final_pending_state() {
        let result = run_activation_wait(
            [ACTIVATING_STATE, ACTIVATING_STATE, ACTIVATING_STATE],
            futures::stream::pending(),
            futures::stream::pending(),
            |_| fallback_error(),
            Duration::ZERO,
        );

        assert!(matches!(result, Err(ConnectionError::Timeout)));
    }

    #[test]
    fn activation_wait_timeout_uses_device_failure_reason_for_final_deactivated_state() {
        let result = run_activation_wait(
            [ACTIVATING_STATE, ACTIVATING_STATE, DEACTIVATED_STATE],
            futures::stream::pending(),
            futures::stream::iter([Some((device_state::FAILED, DEVICE_REASON_NO_SECRETS))]),
            refine_with_fallback(fallback_error),
            Duration::ZERO,
        );

        assert!(matches!(result, Err(ConnectionError::AuthFailed)));
    }

    #[test]
    fn activation_wait_propagates_state_read_error() {
        let result = futures::executor::block_on(wait_for_activation_state(
            futures::stream::pending::<Option<(u32, u32)>>(),
            futures::stream::pending::<DeviceTransition>(),
            || futures::future::ready(Err(zbus::Error::Failure("state read failed".into()))),
            |_| futures::future::ready(ConnectionError::DhcpFailed),
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
