# Connection Options

`ConnectionOptions` controls how NetworkManager handles saved connection profiles — specifically, automatic connection behavior, priority, and retry limits.

## Default Options

```rust
use nmrs::ConnectionOptions;

let opts = ConnectionOptions::default();
// autoconnect: true
// autoconnect_priority: None (NM default = 0)
// autoconnect_retries: None (unlimited)
```

## Configuration Fields

| Field | Type | Default | Description |
|-------|------|---------|-------------|
| `autoconnect` | `bool` | `true` | Connect automatically when available |
| `autoconnect_priority` | `Option<i32>` | `None` (0) | Higher values are preferred when multiple networks are available |
| `autoconnect_retries` | `Option<i32>` | `None` (unlimited) | Maximum retry attempts before giving up |

## Creating Options

### Enable Autoconnect (Default)

```rust
use nmrs::ConnectionOptions;

let opts = ConnectionOptions::new(true);
```

### Disable Autoconnect

```rust
let opts = ConnectionOptions::new(false);
```

### High-Priority Connection

```rust
let opts = ConnectionOptions::new(true)
    .with_priority(10)
    .with_retries(3);
```

Higher priority values make NetworkManager prefer this connection over others when multiple are available.

## How Priority Works

When multiple saved connections are available (e.g., you're in range of both "HomeWiFi" and "CafeWiFi"), NetworkManager connects to the one with the highest `autoconnect_priority`. If priorities are equal, NetworkManager uses its own heuristics (most recently used, signal strength, etc.).

| Priority | Use Case |
|----------|----------|
| 0 (default) | Normal connections |
| Positive (`> 0`) | Preferred connections (higher values take precedence, e.g., `100` over `10`) |
| Negative (`< 0`) | Fallback connections (lower values are tried last, e.g., `-50` after `-10`) |

## How Retries Work

`autoconnect_retries` limits how many times NetworkManager will try to auto-connect a failing connection:

- `None` (default) — Uses NetworkManager's global default configuration (4 attempts).
- `Some(0)` — Explicitly forces **unlimited** retry attempts.
- `Some(n)` — Stops attempting to auto-connect after exactly `n` failed retries.

This is useful for connections that might intermittently fail (e.g., a network at the edge of range).

## Using with Builders

Connection options are used by the low-level [builders](../api/builders.md):

```rust
use nmrs::builders::ConnectionBuilder;
use nmrs::ConnectionOptions;

let opts = ConnectionOptions::new(true)
    .with_priority(5)
    .with_retries(3);

let settings = ConnectionBuilder::new("802-11-wireless", "MyNetwork")
    .options(&opts)
    .ipv4_auto()
    .ipv6_auto()
    .build();
```

The high-level `NetworkManager` API uses `ConnectionOptions::default()` internally. For custom options, build a settings dictionary with the builder APIs and submit it via [`add_connection`](../api/network-manager.md#saving-profiles-without-activating) or [`add_and_activate_connection`](../api/network-manager.md#activating-builder-output).

## Next Steps

- [Custom Timeouts](./timeouts.md) – control how long operations wait
- [Builders Module](../api/builders.md) – low-level connection building
- [Raw Module](../api/raw.md) – `zbus` re-export
- [D-Bus Architecture](./dbus.md) – how settings are sent to NetworkManager
