# Raw Module

The `nmrs::raw` module re-exports the D-Bus dependencies nmrs uses internally:

```rust
pub mod raw {
    pub use zbus;
    pub use zvariant;
}
```

Use it together with [`NetworkManager::dbus_connection()`](./network-manager.md#advanced-d-bus-access)
when you need D-Bus methods that nmrs does not wrap yet. For builder output,
prefer [`add_connection`](./network-manager.md#saving-profiles-without-activating)
and [`add_and_activate_connection`](./network-manager.md#activating-builder-output).

## Why it exists

Builder methods such as `WifiConnectionBuilder::build()` return
`HashMap<&str, HashMap<&str, zvariant::Value<'_>>>`. Without `nmrs::raw`,
consumers would need to depend on their own `zbus` / `zvariant` versions and
hope they match the ones nmrs was built against.

Re-exporting them under `nmrs::raw` keeps those types compatible with builder
output and with the connection returned by `dbus_connection()`.

## Typical workflow

1. Build settings with a builder (for example `WifiConnectionBuilder`).
2. Obtain the shared system bus connection via `nm.dbus_connection()`.
3. Create a zbus proxy on that connection using `nmrs::raw::zbus`.
4. Call `AddConnection` or `AddAndActivateConnection` on NetworkManager.

See [Submitting Builder Output](./builders.md#submitting-builder-output) for the
preferred high-level workflow and [D-Bus Architecture](../advanced/dbus.md) for
background on how nmrs talks to NetworkManager.

## What is not exposed

`nmrs::raw` does **not** re-export nmrs's internal D-Bus proxy types from
`nmrs::dbus`. Those remain implementation details. Advanced callers define
their own minimal `#[zbus::proxy]` traits (or use `call_method` directly) on
top of `dbus_connection()` and `nmrs::raw`.

## Full API Reference

See [docs.rs/nmrs::raw](https://docs.rs/nmrs/latest/nmrs/raw/index.html).
