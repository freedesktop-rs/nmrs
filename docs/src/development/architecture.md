# Architecture

This page describes the internal architecture of the nmrs library. Understanding this helps when contributing or debugging.

## Crate Structure

```
nmrs/src/
├── lib.rs              # Crate root: re-exports, Result type alias, raw module
├── api/                # Public API layer
│   ├── mod.rs
│   ├── network_manager.rs   # NetworkManager struct and methods
│   ├── models/              # Data types (Device, Network, etc.)
│   │   ├── mod.rs
│   │   ├── device.rs
│   │   ├── wifi.rs
│   │   ├── vpn.rs
│   │   ├── bluetooth.rs
│   │   ├── config.rs
│   │   ├── error.rs
│   │   ├── connection_state.rs
│   │   └── state_reason.rs
│   └── builders/            # Connection settings builders
│       ├── mod.rs
│       ├── connection_builder.rs
│       ├── wifi.rs
│       ├── wifi_builder.rs
│       ├── vpn.rs
│       ├── wireguard_builder.rs
│       └── bluetooth.rs
├── core/               # Business logic
│   ├── mod.rs
│   ├── connection.rs        # Wi-Fi/Ethernet connect/disconnect
│   ├── connection_settings.rs  # Saved connection management
│   ├── device.rs            # Device listing, Wi-Fi control
│   ├── scan.rs              # Wi-Fi scanning
│   ├── vpn.rs               # VPN connect/disconnect/list
│   ├── bluetooth.rs         # Bluetooth connections
│   └── state_wait.rs        # Wait for state transitions
├── dbus/               # D-Bus proxy types
│   ├── mod.rs
│   ├── main_nm.rs           # NetworkManager proxy
│   ├── device.rs            # Device proxy
│   ├── wireless.rs          # Wireless device proxy
│   ├── access_point.rs      # Access point proxy
│   ├── active_connection.rs # Active connection proxy
│   ├── wired.rs             # Wired device proxy
│   └── bluetooth.rs         # Bluetooth device proxy
├── monitoring/         # D-Bus signal monitoring
│   ├── mod.rs
│   ├── network.rs           # AP added/removed signals
│   ├── device.rs            # Device state change signals
│   ├── wifi.rs              # Current connection info
│   ├── bluetooth.rs         # Bluetooth signals
│   ├── info.rs              # Network detail retrieval
│   └── transport.rs         # Signal transport
├── types/              # Constants and registries
│   ├── mod.rs
│   ├── constants.rs         # NM device type codes
│   └── device_type_registry.rs  # Device type capabilities
└── util/               # Utilities
    ├── mod.rs
    ├── utils.rs             # Channel calculation, SSID decoding, etc.
    └── validation.rs        # Input validation
```

## Layer Architecture

```
┌──────────────────────────────────────────────────────────┐
│  Your Application                                        │
├──────────────────────────────────────────────────────────┤
│  api/network_manager.rs  ← Public API (NetworkManager)   │
│  api/models/             ← Public data types              │
│  api/builders/           ← Public connection builders     │
│  lib.rs::raw             ← zbus re-export                │
├──────────────────────────────────────────────────────────┤
│  core/                   ← Business logic (not public)    │
│  monitoring/             ← Signal monitoring (not public) │
├──────────────────────────────────────────────────────────┤
│  dbus/                   ← Internal D-Bus proxy types       │
│  util/                   ← Utilities (not public)         │
│  types/                  ← Constants (not public)         │
├──────────────────────────────────────────────────────────┤
│  zbus                    ← D-Bus library                  │
├──────────────────────────────────────────────────────────┤
│  D-Bus System Bus → NetworkManager Daemon                 │
└──────────────────────────────────────────────────────────┘
```

### API Layer

The `api` module defines the public interface:
- `NetworkManager` delegates to `core` functions
- `models` define all public data types
- `builders` construct NM settings dictionaries
- `raw` re-exports `zbus` for advanced D-Bus callers
- `NetworkManager::dbus_connection()` exposes the shared system bus connection

### Core Layer

The `core` module contains the actual business logic:
- `connection.rs` handles Wi-Fi/Ethernet connect/disconnect
- `scan.rs` handles network scanning and listing
- `vpn.rs` handles WireGuard VPN operations
- `state_wait.rs` uses D-Bus signals to wait for state transitions

### D-Bus Layer

The `dbus` module defines typed proxy structs generated with `zbus::proxy` macros. Each proxy corresponds to a NetworkManager D-Bus interface.

### Monitoring Layer

The `monitoring` module subscribes to D-Bus signals for real-time updates:
- Network list changes (AP added/removed)
- Device state changes
- Active connection state

## Key Design Decisions

### Signal-Based State Waiting

Instead of polling, nmrs uses D-Bus signals to wait for state transitions. When you call `connect()`, it:

1. Sends the `AddAndActivateConnection` D-Bus call
2. Subscribes to `StateChanged` signals on the device
3. Awaits the signal with a timeout
4. Returns success on `Activated`, or maps the failure reason to a `ConnectionError`

This is more efficient and responsive than polling.

### Non-Exhaustive Types

All public enums and structs are `#[non_exhaustive]`. This allows adding new fields, variants, and error types without breaking downstream code.

### Connection Reuse

When connecting to a network, nmrs checks for an existing saved profile first. If found, it activates the saved profile rather than creating a new one. This preserves user settings and avoids duplicate profiles.

### Validation

Input validation happens at two levels:
- **Model constructors** (e.g., `BluetoothIdentity::new()` validates MAC format)
- **Builder build methods** (e.g., `WireGuardBuilder::build()` validates keys and addresses)

## Next Steps

- [Testing](./testing.md) – how to run tests
- [Contributing](./contributing.md) – development workflow
