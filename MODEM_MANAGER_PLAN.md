# ModemManager Support Plan for nmrs Workspace

This document outlines a practical plan for adding ModemManager (MM) D-Bus bindings as a sibling crate to `nmrs`. The goal is to provide a high-level async Rust API for cellular modem control, mirroring the patterns already established in `nmrs`.

---

## 1. Crate Structure

### Recommended: New `mmrs` Crate in Workspace

```
nmrs/                     # workspace root
├── Cargo.toml            # workspace manifest (already exists)
├── nmrs/                 # existing NetworkManager bindings
│   └── ...
└── mmrs/                 # NEW: ModemManager bindings
    ├── Cargo.toml
    ├── README.md
    ├── CHANGELOG.md
    └── src/
        ├── lib.rs
        ├── api/
        │   ├── mod.rs
        │   ├── modem_manager.rs    # Main entry point (like network_manager.rs)
        │   ├── modem_scope.rs      # Per-modem scoped API (like wifi_scope.rs)
        │   ├── models/
        │   │   ├── mod.rs
        │   │   ├── modem.rs        # Modem, ModemState, AccessTechnology
        │   │   ├── sim.rs          # Sim, SimState, PinState
        │   │   ├── bearer.rs       # Bearer, BearerProperties, BearerStats
        │   │   ├── signal.rs       # SignalQuality, CellInfo
        │   │   ├── location.rs     # LocationInfo, GpsData
        │   │   ├── sms.rs          # Sms, SmsState
        │   │   └── error.rs        # ModemError enum
        │   └── builders/
        │       ├── mod.rs
        │       ├── bearer.rs       # BearerConfig builder
        │       └── sms.rs          # SmsConfig builder
        ├── core/
        │   ├── mod.rs
        │   ├── modem.rs            # Modem operations
        │   ├── sim.rs              # SIM/PIN operations
        │   ├── bearer.rs           # Bearer connect/disconnect
        │   ├── signal.rs           # Signal quality monitoring
        │   ├── location.rs         # GPS/location services
        │   └── sms.rs              # SMS send/receive
        ├── dbus/
        │   ├── mod.rs
        │   ├── manager.rs          # org.freedesktop.ModemManager1
        │   ├── modem.rs            # org.freedesktop.ModemManager1.Modem
        │   ├── modem_simple.rs     # org.freedesktop.ModemManager1.Modem.Simple
        │   ├── modem_3gpp.rs       # org.freedesktop.ModemManager1.Modem.Modem3gpp
        │   ├── modem_cdma.rs       # org.freedesktop.ModemManager1.Modem.ModemCdma
        │   ├── sim.rs              # org.freedesktop.ModemManager1.Sim
        │   ├── bearer.rs           # org.freedesktop.ModemManager1.Bearer
        │   ├── sms.rs              # org.freedesktop.ModemManager1.Sms
        │   ├── location.rs         # org.freedesktop.ModemManager1.Modem.Location
        │   └── signal.rs           # org.freedesktop.ModemManager1.Modem.Signal
        ├── monitoring/
        │   ├── mod.rs
        │   ├── modem.rs            # Modem state change streams
        │   ├── signal.rs           # Signal quality streams
        │   └── sms.rs              # Incoming SMS streams
        └── types/
            ├── mod.rs
            └── constants.rs        # MM D-Bus constants (states, access tech, etc.)
```

### Workspace Cargo.toml Changes

```toml
# Root Cargo.toml
[workspace]
members = [
  "nmrs",
  "mmrs",
]
resolver = "3"

[workspace.dependencies]
# Shared dependencies - both crates use identical versions
zbus = "5.15.0"
zvariant = "5.11.0"
log = "0.4.29"
serde = { version = "1.0.228", features = ["derive"] }
thiserror = "2.0.18"
futures = "0.3.32"
tokio = { version = "1.52.3", features = ["rt-multi-thread", "macros", "sync", "time"] }
async-trait = "0.1.89"
bitflags = "2.11.1"

# Workspace crates can depend on each other
nmrs = { path = "nmrs", version = "3.1" }
mmrs = { path = "mmrs", version = "0.1" }
```

---

## 2. D-Bus Proxy Layer (Phase 1)

The foundation. Mirror the `zbus::proxy` macro pattern from `nmrs/src/dbus/`.

### 2.1 Manager Proxy

```rust
// mmrs/src/dbus/manager.rs
use zbus::proxy;
use zvariant::OwnedObjectPath;

#[proxy(
    interface = "org.freedesktop.ModemManager1",
    default_service = "org.freedesktop.ModemManager1",
    default_path = "/org/freedesktop/ModemManager1"
)]
pub trait MM {
    /// Force MM to re-scan for devices.
    fn scan_devices(&self) -> zbus::Result<()>;

    /// Set logging verbosity ("ERR", "WARN", "INFO", "DEBUG").
    fn set_logging(&self, level: &str) -> zbus::Result<()>;

    /// Report a kernel event (udev passthrough).
    fn report_kernel_event(&self, properties: std::collections::HashMap<&str, zvariant::Value<'_>>) -> zbus::Result<()>;

    /// Inhibit modem device handling (returns inhibition cookie).
    fn inhibit_device(&self, uid: &str) -> zbus::Result<u32>;

    /// Uninhibit a previously inhibited device.
    fn uninhibit_device(&self, uid: &str) -> zbus::Result<()>;

    /// MM daemon version string.
    #[zbus(property)]
    fn version(&self) -> zbus::Result<String>;
}
```

### 2.2 Modem Proxy

```rust
// mmrs/src/dbus/modem.rs
use zbus::proxy;
use zvariant::OwnedObjectPath;

#[proxy(
    interface = "org.freedesktop.ModemManager1.Modem",
    default_service = "org.freedesktop.ModemManager1"
)]
pub trait MMModem {
    /// Enable or disable the modem.
    fn enable(&self, enable: bool) -> zbus::Result<()>;

    /// List bearer object paths.
    fn list_bearers(&self) -> zbus::Result<Vec<OwnedObjectPath>>;

    /// Create a new bearer with given properties.
    fn create_bearer(
        &self,
        properties: std::collections::HashMap<&str, zvariant::Value<'_>>,
    ) -> zbus::Result<OwnedObjectPath>;

    /// Delete a bearer.
    fn delete_bearer(&self, bearer: OwnedObjectPath) -> zbus::Result<()>;

    /// Reset the modem.
    fn reset(&self) -> zbus::Result<()>;

    /// Factory reset (requires unlock code).
    fn factory_reset(&self, code: &str) -> zbus::Result<()>;

    /// Set power state (low, on, off).
    fn set_power_state(&self, state: u32) -> zbus::Result<()>;

    /// Set allowed/preferred network modes.
    fn set_current_modes(&self, modes: (u32, u32)) -> zbus::Result<()>;

    /// Set current bands.
    fn set_current_bands(&self, bands: Vec<u32>) -> zbus::Result<()>;

    /// Request modem command (AT command passthrough).
    fn command(&self, cmd: &str, timeout: u32) -> zbus::Result<String>;

    // --- Properties ---

    #[zbus(property)]
    fn sim(&self) -> zbus::Result<OwnedObjectPath>;

    #[zbus(property)]
    fn sim_slots(&self) -> zbus::Result<Vec<OwnedObjectPath>>;

    #[zbus(property)]
    fn primary_sim_slot(&self) -> zbus::Result<u32>;

    #[zbus(property)]
    fn bearers(&self) -> zbus::Result<Vec<OwnedObjectPath>>;

    #[zbus(property)]
    fn state(&self) -> zbus::Result<i32>;

    #[zbus(property)]
    fn state_failed_reason(&self) -> zbus::Result<u32>;

    #[zbus(property)]
    fn power_state(&self) -> zbus::Result<u32>;

    #[zbus(property)]
    fn access_technologies(&self) -> zbus::Result<u32>;

    #[zbus(property)]
    fn signal_quality(&self) -> zbus::Result<(u32, bool)>;

    #[zbus(property)]
    fn own_numbers(&self) -> zbus::Result<Vec<String>>;

    #[zbus(property)]
    fn manufacturer(&self) -> zbus::Result<String>;

    #[zbus(property)]
    fn model(&self) -> zbus::Result<String>;

    #[zbus(property)]
    fn revision(&self) -> zbus::Result<String>;

    #[zbus(property)]
    fn equipment_identifier(&self) -> zbus::Result<String>;

    #[zbus(property)]
    fn device_identifier(&self) -> zbus::Result<String>;

    #[zbus(property)]
    fn device(&self) -> zbus::Result<String>;

    #[zbus(property)]
    fn drivers(&self) -> zbus::Result<Vec<String>>;

    #[zbus(property)]
    fn plugin(&self) -> zbus::Result<String>;

    #[zbus(property)]
    fn primary_port(&self) -> zbus::Result<String>;

    #[zbus(property)]
    fn ports(&self) -> zbus::Result<Vec<(String, u32)>>;

    #[zbus(property)]
    fn current_modes(&self) -> zbus::Result<(u32, u32)>;

    #[zbus(property)]
    fn supported_modes(&self) -> zbus::Result<Vec<(u32, u32)>>;

    #[zbus(property)]
    fn current_bands(&self) -> zbus::Result<Vec<u32>>;

    #[zbus(property)]
    fn supported_bands(&self) -> zbus::Result<Vec<u32>>;

    #[zbus(property)]
    fn max_bearers(&self) -> zbus::Result<u32>;

    #[zbus(property)]
    fn max_active_bearers(&self) -> zbus::Result<u32>;

    // --- Signals ---

    #[zbus(signal)]
    fn state_changed(&self, old: i32, new: i32, reason: u32);
}
```

### 2.3 Simple Modem Proxy

```rust
// mmrs/src/dbus/modem_simple.rs
use zbus::proxy;
use std::collections::HashMap;
use zvariant::{OwnedObjectPath, OwnedValue};

#[proxy(
    interface = "org.freedesktop.ModemManager1.Modem.Simple",
    default_service = "org.freedesktop.ModemManager1"
)]
pub trait MMModemSimple {
    /// One-shot connect: enable modem, register, create bearer, connect.
    fn connect(
        &self,
        properties: HashMap<&str, zvariant::Value<'_>>,
    ) -> zbus::Result<OwnedObjectPath>;

    /// Disconnect all or a specific bearer.
    fn disconnect(&self, bearer: OwnedObjectPath) -> zbus::Result<()>;

    /// Get current modem status (registration, signal, bearer state, etc.).
    fn get_status(&self) -> zbus::Result<HashMap<String, OwnedValue>>;
}
```

### 2.4 SIM Proxy

```rust
// mmrs/src/dbus/sim.rs
use zbus::proxy;

#[proxy(
    interface = "org.freedesktop.ModemManager1.Sim",
    default_service = "org.freedesktop.ModemManager1"
)]
pub trait MMSim {
    /// Send PIN to unlock SIM.
    fn send_pin(&self, pin: &str) -> zbus::Result<()>;

    /// Send PUK and new PIN.
    fn send_puk(&self, puk: &str, pin: &str) -> zbus::Result<()>;

    /// Enable or disable PIN requirement.
    fn enable_pin(&self, pin: &str, enabled: bool) -> zbus::Result<()>;

    /// Change the SIM PIN.
    fn change_pin(&self, old_pin: &str, new_pin: &str) -> zbus::Result<()>;

    /// Set preferred networks (for roaming).
    fn set_preferred_networks(
        &self,
        preferred: Vec<(String, u32)>,
    ) -> zbus::Result<()>;

    // --- Properties ---

    #[zbus(property)]
    fn active(&self) -> zbus::Result<bool>;

    #[zbus(property)]
    fn sim_identifier(&self) -> zbus::Result<String>;

    #[zbus(property)]
    fn imsi(&self) -> zbus::Result<String>;

    #[zbus(property)]
    fn eid(&self) -> zbus::Result<String>;

    #[zbus(property)]
    fn operator_identifier(&self) -> zbus::Result<String>;

    #[zbus(property)]
    fn operator_name(&self) -> zbus::Result<String>;

    #[zbus(property)]
    fn emergency_numbers(&self) -> zbus::Result<Vec<String>>;

    #[zbus(property)]
    fn preferred_networks(&self) -> zbus::Result<Vec<(String, u32)>>;
}
```

### 2.5 Bearer Proxy

```rust
// mmrs/src/dbus/bearer.rs
use zbus::proxy;
use std::collections::HashMap;
use zvariant::OwnedValue;

#[proxy(
    interface = "org.freedesktop.ModemManager1.Bearer",
    default_service = "org.freedesktop.ModemManager1"
)]
pub trait MMBearer {
    /// Connect the bearer (bring up data session).
    fn connect(&self) -> zbus::Result<()>;

    /// Disconnect the bearer.
    fn disconnect(&self) -> zbus::Result<()>;

    // --- Properties ---

    #[zbus(property)]
    fn interface(&self) -> zbus::Result<String>;

    #[zbus(property)]
    fn connected(&self) -> zbus::Result<bool>;

    #[zbus(property)]
    fn suspended(&self) -> zbus::Result<bool>;

    #[zbus(property)]
    fn multiplexed(&self) -> zbus::Result<bool>;

    #[zbus(property)]
    fn ip4_config(&self) -> zbus::Result<HashMap<String, OwnedValue>>;

    #[zbus(property)]
    fn ip6_config(&self) -> zbus::Result<HashMap<String, OwnedValue>>;

    #[zbus(property)]
    fn stats(&self) -> zbus::Result<HashMap<String, OwnedValue>>;

    #[zbus(property)]
    fn ip_timeout(&self) -> zbus::Result<u32>;

    #[zbus(property)]
    fn bearer_type(&self) -> zbus::Result<u32>;

    #[zbus(property)]
    fn properties(&self) -> zbus::Result<HashMap<String, OwnedValue>>;
}
```

---

## 3. Model Layer (Phase 2)

Type-safe Rust structs wrapping raw D-Bus values.

### 3.1 Core Enums and Types

```rust
// mmrs/src/api/models/modem.rs

/// Modem state (from MM_MODEM_STATE_*).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum ModemState {
    Failed,
    Unknown,
    Initializing,
    Locked,
    Disabled,
    Disabling,
    Enabling,
    Enabled,
    Searching,
    Registered,
    Disconnecting,
    Connecting,
    Connected,
}

/// Access technology flags (bitmask).
#[derive(Debug, Clone, Copy)]
#[non_exhaustive]
pub struct AccessTechnology(u32);

impl AccessTechnology {
    pub const UNKNOWN: u32 = 0;
    pub const POTS: u32 = 1 << 0;
    pub const GSM: u32 = 1 << 1;
    pub const GSM_COMPACT: u32 = 1 << 2;
    pub const GPRS: u32 = 1 << 3;
    pub const EDGE: u32 = 1 << 4;
    pub const UMTS: u32 = 1 << 5;
    pub const HSDPA: u32 = 1 << 6;
    pub const HSUPA: u32 = 1 << 7;
    pub const HSPA: u32 = 1 << 8;
    pub const HSPA_PLUS: u32 = 1 << 9;
    pub const EVDO0: u32 = 1 << 10;
    pub const EVDOA: u32 = 1 << 11;
    pub const EVDOB: u32 = 1 << 12;
    pub const LTE: u32 = 1 << 14;
    pub const FIVE_GNR: u32 = 1 << 15;
    pub const LTE_CAT_M: u32 = 1 << 16;
    pub const LTE_NB_IOT: u32 = 1 << 17;

    pub fn has_lte(&self) -> bool { self.0 & Self::LTE != 0 }
    pub fn has_5g(&self) -> bool { self.0 & Self::FIVE_GNR != 0 }
    pub fn is_3gpp(&self) -> bool {
        (self.0 & (Self::GSM | Self::GPRS | Self::EDGE | Self::UMTS | Self::HSDPA |
                   Self::HSUPA | Self::HSPA | Self::HSPA_PLUS | Self::LTE | Self::FIVE_GNR)) != 0
    }
}

/// Modem information snapshot.
#[derive(Debug, Clone)]
#[non_exhaustive]
pub struct Modem {
    pub path: String,
    pub state: ModemState,
    pub manufacturer: String,
    pub model: String,
    pub revision: String,
    pub equipment_identifier: String,  // IMEI
    pub device: String,                // sysfs path
    pub primary_port: String,
    pub access_technologies: AccessTechnology,
    pub signal_quality: u32,
    pub signal_quality_recent: bool,
    pub own_numbers: Vec<String>,
    pub max_bearers: u32,
    pub max_active_bearers: u32,
}
```

### 3.2 SIM Types

```rust
// mmrs/src/api/models/sim.rs

/// SIM lock state.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum SimLockState {
    Unknown,
    Unlocked,
    PinRequired,
    PukRequired,
    PhNetPinRequired,
    // ... other lock states
}

/// SIM card information.
#[derive(Debug, Clone)]
#[non_exhaustive]
pub struct Sim {
    pub path: String,
    pub active: bool,
    pub iccid: String,          // sim_identifier
    pub imsi: String,
    pub eid: Option<String>,    // eSIM identifier
    pub operator_id: String,
    pub operator_name: String,
    pub emergency_numbers: Vec<String>,
}
```

### 3.3 Bearer Types

```rust
// mmrs/src/api/models/bearer.rs

/// Bearer (data session) state.
#[derive(Debug, Clone)]
#[non_exhaustive]
pub struct Bearer {
    pub path: String,
    pub interface: String,      // e.g., "wwan0"
    pub connected: bool,
    pub suspended: bool,
    pub ip4_config: Option<Ip4Config>,
    pub ip6_config: Option<Ip6Config>,
    pub stats: BearerStats,
}

/// IPv4 configuration from bearer.
#[derive(Debug, Clone)]
#[non_exhaustive]
pub struct Ip4Config {
    pub address: String,
    pub prefix: u32,
    pub gateway: Option<String>,
    pub dns: Vec<String>,
    pub mtu: Option<u32>,
}

/// Bearer statistics.
#[derive(Debug, Clone, Default)]
#[non_exhaustive]
pub struct BearerStats {
    pub rx_bytes: u64,
    pub tx_bytes: u64,
    pub duration: u32,
    pub attempts: u32,
    pub failed_attempts: u32,
}

/// Bearer configuration for creating new bearers.
#[derive(Debug, Clone)]
#[non_exhaustive]
pub struct BearerConfig {
    pub apn: String,
    pub ip_type: IpType,
    pub user: Option<String>,
    pub password: Option<String>,
    pub auth_method: Option<AuthMethod>,
    pub allow_roaming: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum IpType {
    #[default]
    Ipv4,
    Ipv6,
    Ipv4v6,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AuthMethod {
    None,
    Pap,
    Chap,
    MschapV2,
}
```

### 3.4 Error Types

```rust
// mmrs/src/api/models/error.rs

#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum ModemError {
    #[error("D-Bus error: {0}")]
    Dbus(#[from] zbus::Error),

    #[error("no modems found")]
    NoModems,

    #[error("modem not found: {0}")]
    ModemNotFound(String),

    #[error("SIM not inserted")]
    NoSim,

    #[error("SIM locked: {0:?}")]
    SimLocked(SimLockState),

    #[error("wrong PIN")]
    WrongPin,

    #[error("PIN required")]
    PinRequired,

    #[error("PUK required")]
    PukRequired,

    #[error("bearer creation failed: {0}")]
    BearerCreationFailed(String),

    #[error("connection failed: {0}")]
    ConnectionFailed(String),

    #[error("modem disabled")]
    ModemDisabled,

    #[error("not registered to network")]
    NotRegistered,

    #[error("operation timed out")]
    Timeout,

    #[error("operation cancelled")]
    Cancelled,

    #[error("invalid APN: {0}")]
    InvalidApn(String),

    #[error("no signal")]
    NoSignal,

    #[error("roaming not allowed")]
    RoamingNotAllowed,
}

pub type Result<T> = std::result::Result<T, ModemError>;
```

---

## 4. High-Level API (Phase 3)

The main `ModemManager` struct and ergonomic operations.

### 4.1 Main Entry Point

```rust
// mmrs/src/api/modem_manager.rs

use zbus::Connection;
use crate::{Result, Modem, Sim, Bearer, BearerConfig, ModemError};

/// Main entry point for ModemManager operations.
///
/// # Example
///
/// ```rust
/// use mmrs::ModemManager;
///
/// #[tokio::main]
/// async fn main() -> mmrs::Result<()> {
///     let mm = ModemManager::new().await?;
///
///     // List all modems
///     let modems = mm.list_modems().await?;
///     for modem in &modems {
///         println!("{}: {} ({})", modem.model, modem.equipment_identifier, modem.state);
///     }
///
///     // Quick connect using Simple interface
///     mm.connect_simple("internet.apn.com").await?;
///
///     Ok(())
/// }
/// ```
pub struct ModemManager {
    conn: Connection,
}

impl ModemManager {
    /// Connect to the system D-Bus and ModemManager.
    pub async fn new() -> Result<Self>;

    /// Use an existing D-Bus connection.
    pub async fn with_connection(conn: Connection) -> Result<Self>;

    /// Get the D-Bus connection for advanced use.
    pub fn connection(&self) -> &Connection;

    // --- Modem Enumeration ---

    /// List all modems.
    pub async fn list_modems(&self) -> Result<Vec<Modem>>;

    /// Get a specific modem by equipment identifier (IMEI).
    pub async fn modem_by_imei(&self, imei: &str) -> Result<Modem>;

    /// Get the first/primary modem (convenience for single-modem systems).
    pub async fn primary_modem(&self) -> Result<Modem>;

    /// Get a scoped API for a specific modem.
    pub fn modem(&self, path: &str) -> ModemScope<'_>;

    // --- Simple Operations (primary modem) ---

    /// Enable the primary modem.
    pub async fn enable(&self) -> Result<()>;

    /// Disable the primary modem.
    pub async fn disable(&self) -> Result<()>;

    /// Quick connect: enable, register, create bearer, connect.
    /// Uses the Simple.Connect interface.
    pub async fn connect_simple(&self, apn: &str) -> Result<Bearer>;

    /// Connect with full bearer configuration.
    pub async fn connect(&self, config: &BearerConfig) -> Result<Bearer>;

    /// Disconnect all data sessions.
    pub async fn disconnect(&self) -> Result<()>;

    /// Get current connection status.
    pub async fn status(&self) -> Result<ConnectionStatus>;

    // --- SIM Operations (primary modem) ---

    /// Get SIM information.
    pub async fn sim(&self) -> Result<Option<Sim>>;

    /// Unlock SIM with PIN.
    pub async fn unlock_pin(&self, pin: &str) -> Result<()>;

    /// Unlock SIM with PUK and set new PIN.
    pub async fn unlock_puk(&self, puk: &str, new_pin: &str) -> Result<()>;

    /// Enable or disable PIN lock.
    pub async fn set_pin_enabled(&self, pin: &str, enabled: bool) -> Result<()>;

    /// Change PIN.
    pub async fn change_pin(&self, old_pin: &str, new_pin: &str) -> Result<()>;

    // --- Signal & Registration ---

    /// Get current signal quality (0-100).
    pub async fn signal_quality(&self) -> Result<u32>;

    /// Get detailed signal information.
    pub async fn signal_info(&self) -> Result<SignalInfo>;

    /// Get current access technology.
    pub async fn access_technology(&self) -> Result<AccessTechnology>;

    /// Get 3GPP registration info.
    pub async fn registration_info(&self) -> Result<RegistrationInfo>;

    // --- Bearer Management ---

    /// List active bearers.
    pub async fn list_bearers(&self) -> Result<Vec<Bearer>>;

    /// Create a bearer without connecting it.
    pub async fn create_bearer(&self, config: &BearerConfig) -> Result<Bearer>;

    /// Delete a bearer.
    pub async fn delete_bearer(&self, path: &str) -> Result<()>;

    // --- Monitoring ---

    /// Stream of modem state changes.
    pub async fn monitor_state(&self) -> Result<impl Stream<Item = ModemStateChange>>;

    /// Stream of signal quality updates.
    pub async fn monitor_signal(&self) -> Result<impl Stream<Item = SignalUpdate>>;

    // --- Advanced ---

    /// Send an AT command directly.
    pub async fn at_command(&self, cmd: &str, timeout_secs: u32) -> Result<String>;

    /// Reset the modem.
    pub async fn reset(&self) -> Result<()>;

    /// ModemManager daemon version.
    pub async fn version(&self) -> Result<String>;
}
```

### 4.2 Per-Modem Scoped API

```rust
// mmrs/src/api/modem_scope.rs

/// Scoped operations on a specific modem.
///
/// Obtained via `mm.modem("/org/freedesktop/ModemManager1/Modem/0")`.
pub struct ModemScope<'a> {
    mm: &'a ModemManager,
    path: String,
}

impl<'a> ModemScope<'a> {
    /// Get modem information.
    pub async fn info(&self) -> Result<Modem>;

    /// Enable this modem.
    pub async fn enable(&self) -> Result<()>;

    /// Disable this modem.
    pub async fn disable(&self) -> Result<()>;

    /// Simple connect.
    pub async fn connect_simple(&self, apn: &str) -> Result<Bearer>;

    /// Connect with config.
    pub async fn connect(&self, config: &BearerConfig) -> Result<Bearer>;

    /// Disconnect.
    pub async fn disconnect(&self) -> Result<()>;

    // ... all ModemManager methods but scoped to this modem
}
```

---

## 5. Monitoring Layer (Phase 4)

Real-time D-Bus signal subscriptions, following `nmrs/src/monitoring/` patterns.

### 5.1 Modem State Monitoring

```rust
// mmrs/src/monitoring/modem.rs

/// Modem state change event.
#[derive(Debug, Clone)]
pub struct ModemStateChange {
    pub modem_path: String,
    pub old_state: ModemState,
    pub new_state: ModemState,
    pub reason: StateChangeReason,
}

/// Subscribe to modem state changes across all modems.
pub async fn monitor_modem_state(
    conn: &Connection,
) -> Result<impl Stream<Item = ModemStateChange> + Send>;
```

### 5.2 Signal Monitoring

```rust
// mmrs/src/monitoring/signal.rs

/// Signal quality update.
#[derive(Debug, Clone)]
pub struct SignalUpdate {
    pub modem_path: String,
    pub quality: u32,
    pub recent: bool,
    pub access_technology: AccessTechnology,
}

/// Subscribe to signal quality changes.
pub async fn monitor_signal(
    conn: &Connection,
) -> Result<impl Stream<Item = SignalUpdate> + Send>;
```

---

## 6. Integration with nmrs (Phase 5)

Optional integration points where both crates work together.

### 6.1 Combined Connection Management

```rust
// Could live in either crate or a third `nmrs-mmrs` integration crate

/// Unified network manager that coordinates NM and MM.
pub struct UnifiedNetworkManager {
    nm: nmrs::NetworkManager,
    mm: mmrs::ModemManager,
}

impl UnifiedNetworkManager {
    /// Connect via cellular, letting NM manage the connection profile
    /// while MM handles the bearer.
    pub async fn connect_cellular(&self, config: &CellularConfig) -> Result<()>;

    /// Get unified network state across all transports.
    pub async fn network_state(&self) -> NetworkState;
}
```

### 6.2 NM GSM Profile + MM Bearer Coordination

For vehicle gateways, the typical flow is:
1. NM has a saved GSM/CDMA connection profile
2. MM handles the actual modem/bearer
3. When NM activates the profile, it talks to MM under the hood

`mmrs` provides direct MM control for when you need to bypass NM's abstraction (PIN unlock, signal monitoring, AT commands, etc.).

---

## 7. Implementation Phases

### Phase 1: Foundation (Week 1-2)
- [ ] Create `mmrs` crate structure in workspace
- [ ] Implement D-Bus proxies: Manager, Modem, Sim, Bearer
- [ ] Basic constants and type mappings
- [ ] Unit tests for proxy generation

### Phase 2: Core Models (Week 2-3)
- [ ] Implement all model structs with `#[non_exhaustive]`
- [ ] Error enum with all MM failure modes
- [ ] Builders for `BearerConfig`
- [ ] Documentation with examples

### Phase 3: High-Level API (Week 3-4)
- [ ] `ModemManager` struct with `new()`, `list_modems()`
- [ ] Simple connect/disconnect flow
- [ ] SIM PIN operations
- [ ] Signal quality queries
- [ ] `ModemScope` for multi-modem systems

### Phase 4: Monitoring (Week 4-5)
- [ ] State change streams
- [ ] Signal quality streams
- [ ] SMS incoming streams (if scope includes SMS)

### Phase 5: Testing & CI (Week 5-6)
- [ ] Integration tests (require MM + modem hardware or `mock-modem`)
- [ ] Add `mmrs` to CI pipeline (format, clippy, semver-checks)
- [ ] Cross-compile for aarch64
- [ ] Update Dockerfile to include `modemmanager`
- [ ] Documentation pass

### Phase 6: Optional Extensions
- [ ] SMS send/receive
- [ ] Location/GPS services
- [ ] Voice call support
- [ ] USSD commands
- [ ] Firmware management

---

## 8. Testing Strategy

### 8.1 Mock Modem for CI

ModemManager includes `mmcli --test` and mock modem support via `libmm-glib`. For Rust CI:

```dockerfile
# Updated Dockerfile
FROM rust:1.95.0

RUN apt-get update && apt-get install -y \
    libdbus-1-dev \
    pkg-config \
    dbus \
    network-manager \
    modemmanager \
    && rm -rf /var/lib/apt/lists/*

# Start dbus and MM with a virtual modem for testing
CMD dbus-daemon --system && \
    ModemManager --test && \
    cargo test --all-features --workspace
```

### 8.2 Hardware Integration Tests

Similar to `nmrs` Wi-Fi tests with `mac80211_hwsim`:

```rust
// mmrs/tests/integration_test.rs

macro_rules! require_modemmanager {
    () => {
        if !is_modemmanager_available().await {
            eprintln!("Skipping test: ModemManager not available");
            return;
        }
    };
}

macro_rules! require_modem {
    ($mm:expr) => {
        if $mm.list_modems().await?.is_empty() {
            eprintln!("Skipping test: No modem available");
            return;
        }
    };
}
```

---

## 9. Vehicle/IoT Specific Considerations

For your vehicle gateway with 18 CAN buses:

### 9.1 Multi-Modem Support
- Enumerate all modems, not just primary
- Per-modem scoping via `mm.modem(path)`
- Track IMEI to correlate with physical slot

### 9.2 Resilience Patterns
- Auto-reconnect on bearer drop
- Signal-based handover decisions
- Roaming policy enforcement

### 9.3 Typical Flow
```rust
let mm = ModemManager::new().await?;

// Unlock SIM if needed
if let Some(sim) = mm.sim().await? {
    if mm.primary_modem().await?.state == ModemState::Locked {
        mm.unlock_pin("1234").await?;
    }
}

// Connect with APN
let bearer = mm.connect(&BearerConfig {
    apn: "fleet.apn.com".into(),
    ip_type: IpType::Ipv4v6,
    allow_roaming: false,
    ..Default::default()
}).await?;

println!("Connected via {}: {}", bearer.interface, bearer.ip4_config.unwrap().address);

// Monitor signal for handover decisions
let mut signal_stream = mm.monitor_signal().await?;
while let Some(update) = signal_stream.next().await {
    if update.quality < 20 {
        warn!("Low signal: {}%", update.quality);
    }
}
```

---

## 10. Open Questions

1. **SMS scope** — Should `mmrs` v0.1 include SMS, or defer to a later release?
2. **Location services** — GPS via MM is common on embedded; include or separate?
3. **NM coordination** — Should `mmrs` be usable standalone, or tightly couple to `nmrs`?
4. **eSIM provisioning** — Support for `Modem.Profile` interface (LPA)?

---

## References

- [ModemManager D-Bus API Reference](https://freedesktop.org/software/ModemManager/api/latest/ref-dbus.html)
- [mmcli man page](https://www.freedesktop.org/software/ModemManager/man/latest/mmcli.1.html)
- [nmrs BlueZ integration](nmrs/src/core/airplane.rs) — pattern to follow
- [zbus proxy macro](https://docs.rs/zbus/latest/zbus/attr.proxy.html)
