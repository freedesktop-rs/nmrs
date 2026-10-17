//! Typed access to any key of a saved connection profile.
//!
//! [`SavedConnection`](crate::models::SavedConnection) promotes only the
//! common sections to typed fields. Everything else stays reachable through
//! [`SavedConnection::get_property`](crate::models::SavedConnection::get_property),
//! which reads one `section.key` from the raw `GetSettings` map and decodes
//! it as `T`. The [`properties`] module names the keys nmrs already
//! understands; [`Property::new`] covers the rest.

use std::marker::PhantomData;

use zvariant::{OwnedValue, Value};

/// One `section.key` of a NetworkManager profile, decoded as `T`.
///
/// ```rust
/// use nmrs::models::{Property, properties};
///
/// // Pre-defined:
/// let zone = properties::connection::ZONE;
/// // Or any key NetworkManager documents:
/// let mtu: Property<u32> = Property::new("802-3-ethernet", "mtu");
/// assert_eq!(zone.section(), "connection");
/// assert_eq!(mtu.key(), "mtu");
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Property<T> {
    section: &'static str,
    key: &'static str,
    value: PhantomData<fn() -> T>,
}

impl<T> Property<T> {
    /// Names `section.key`, to be decoded as `T`.
    #[must_use]
    pub const fn new(section: &'static str, key: &'static str) -> Self {
        Self {
            section,
            key,
            value: PhantomData,
        }
    }

    /// Settings section, e.g. `connection` or `802-11-wireless`.
    #[must_use]
    pub const fn section(&self) -> &'static str {
        self.section
    }

    /// Key inside the section, e.g. `autoconnect-retries`.
    #[must_use]
    pub const fn key(&self) -> &'static str {
        self.key
    }
}

/// Types that can be decoded from a profile value.
///
/// Implementations return `None` when the value has another D-Bus type, so a
/// [`Property`] declared with the wrong `T` reads as absent rather than
/// panicking.
pub trait FromSetting: Sized {
    /// Decodes `value`, or `None` if it is not a `Self`.
    fn from_setting(value: &OwnedValue) -> Option<Self>;
}

/// Strips the `v` boxes NetworkManager wraps around `a{sv}` values.
pub(crate) fn unbox_value<'a, 'v>(mut value: &'a Value<'v>) -> &'a Value<'v> {
    while let Value::Value(inner) = value {
        value = inner;
    }
    value
}

macro_rules! from_setting_scalar {
    ($($ty:ty => $variant:ident),* $(,)?) => {
        $(
            impl FromSetting for $ty {
                fn from_setting(value: &OwnedValue) -> Option<Self> {
                    match unbox_value(value) {
                        Value::$variant(inner) => Some(*inner),
                        _ => None,
                    }
                }
            }
        )*
    };
}

from_setting_scalar! {
    bool => Bool,
    u8 => U8,
    i16 => I16,
    u16 => U16,
    i32 => I32,
    u32 => U32,
    i64 => I64,
    u64 => U64,
    f64 => F64,
}

impl FromSetting for String {
    fn from_setting(value: &OwnedValue) -> Option<Self> {
        match unbox_value(value) {
            Value::Str(inner) => Some(inner.to_string()),
            _ => None,
        }
    }
}

impl FromSetting for Vec<String> {
    fn from_setting(value: &OwnedValue) -> Option<Self> {
        match unbox_value(value) {
            Value::Array(array) => array
                .iter()
                .map(|item| match unbox_value(item) {
                    Value::Str(inner) => Some(inner.to_string()),
                    _ => None,
                })
                .collect(),
            _ => None,
        }
    }
}

impl FromSetting for Vec<u8> {
    fn from_setting(value: &OwnedValue) -> Option<Self> {
        match unbox_value(value) {
            Value::Array(array) => array
                .iter()
                .map(|item| match unbox_value(item) {
                    Value::U8(inner) => Some(*inner),
                    _ => None,
                })
                .collect(),
            _ => None,
        }
    }
}

impl FromSetting for Vec<u32> {
    fn from_setting(value: &OwnedValue) -> Option<Self> {
        match unbox_value(value) {
            Value::Array(array) => array
                .iter()
                .map(|item| match unbox_value(item) {
                    Value::U32(inner) => Some(*inner),
                    _ => None,
                })
                .collect(),
            _ => None,
        }
    }
}

/// The raw value, for keys whose type has no `FromSetting` impl.
impl FromSetting for OwnedValue {
    fn from_setting(value: &OwnedValue) -> Option<Self> {
        value.try_clone().ok()
    }
}

/// Keys of the sections nmrs decodes, named after the NetworkManager
/// [settings reference](https://networkmanager.dev/docs/api/latest/nm-settings-dbus.html).
///
/// Typed fields on [`SavedConnection`](crate::models::SavedConnection) cover
/// the common ones; these constants reach the rest without a crate release.
pub mod properties {
    use super::Property;

    /// `connection.*`
    pub mod connection {
        use super::Property;

        /// `connection.id`
        pub const ID: Property<String> = Property::new("connection", "id");
        /// `connection.uuid`
        pub const UUID: Property<String> = Property::new("connection", "uuid");
        /// `connection.type`
        pub const TYPE: Property<String> = Property::new("connection", "type");
        /// `connection.interface-name`
        pub const INTERFACE_NAME: Property<String> = Property::new("connection", "interface-name");
        /// `connection.autoconnect`
        pub const AUTOCONNECT: Property<bool> = Property::new("connection", "autoconnect");
        /// `connection.autoconnect-priority`
        pub const AUTOCONNECT_PRIORITY: Property<i32> =
            Property::new("connection", "autoconnect-priority");
        /// `connection.autoconnect-retries`
        pub const AUTOCONNECT_RETRIES: Property<i32> =
            Property::new("connection", "autoconnect-retries");
        /// `connection.timestamp`
        pub const TIMESTAMP: Property<u64> = Property::new("connection", "timestamp");
        /// `connection.permissions`
        pub const PERMISSIONS: Property<Vec<String>> = Property::new("connection", "permissions");
        /// `connection.zone`
        pub const ZONE: Property<String> = Property::new("connection", "zone");
        /// `connection.metered`
        pub const METERED: Property<i32> = Property::new("connection", "metered");
        /// `connection.lldp`
        pub const LLDP: Property<i32> = Property::new("connection", "lldp");
        /// `connection.mdns`
        pub const MDNS: Property<i32> = Property::new("connection", "mdns");
        /// `connection.llmnr`
        pub const LLMNR: Property<i32> = Property::new("connection", "llmnr");
        /// `connection.dns-over-tls`
        pub const DNS_OVER_TLS: Property<i32> = Property::new("connection", "dns-over-tls");
        /// `connection.stable-id`
        pub const STABLE_ID: Property<String> = Property::new("connection", "stable-id");
        /// `connection.secondaries`
        pub const SECONDARIES: Property<Vec<String>> = Property::new("connection", "secondaries");
        /// `connection.master` (deprecated in NetworkManager for `controller`)
        pub const MASTER: Property<String> = Property::new("connection", "master");
        /// `connection.controller`
        pub const CONTROLLER: Property<String> = Property::new("connection", "controller");
        /// `connection.slave-type` (deprecated in NetworkManager for `port-type`)
        pub const SLAVE_TYPE: Property<String> = Property::new("connection", "slave-type");
        /// `connection.port-type`
        pub const PORT_TYPE: Property<String> = Property::new("connection", "port-type");
    }

    macro_rules! ip_section {
        ($name:ident, $section:literal) => {
            #[doc = concat!("`", $section, ".*`")]
            pub mod $name {
                use super::Property;

                #[doc = concat!("`", $section, ".method`")]
                pub const METHOD: Property<String> = Property::new($section, "method");
                #[doc = concat!("`", $section, ".gateway`")]
                pub const GATEWAY: Property<String> = Property::new($section, "gateway");
                #[doc = concat!("`", $section, ".dns-data`")]
                pub const DNS_DATA: Property<Vec<String>> = Property::new($section, "dns-data");
                #[doc = concat!("`", $section, ".dns-search`")]
                pub const DNS_SEARCH: Property<Vec<String>> = Property::new($section, "dns-search");
                #[doc = concat!("`", $section, ".dns-options`")]
                pub const DNS_OPTIONS: Property<Vec<String>> =
                    Property::new($section, "dns-options");
                #[doc = concat!("`", $section, ".dns-priority`")]
                pub const DNS_PRIORITY: Property<i32> = Property::new($section, "dns-priority");
                #[doc = concat!("`", $section, ".ignore-auto-dns`")]
                pub const IGNORE_AUTO_DNS: Property<bool> =
                    Property::new($section, "ignore-auto-dns");
                #[doc = concat!("`", $section, ".ignore-auto-routes`")]
                pub const IGNORE_AUTO_ROUTES: Property<bool> =
                    Property::new($section, "ignore-auto-routes");
                #[doc = concat!("`", $section, ".never-default`")]
                pub const NEVER_DEFAULT: Property<bool> = Property::new($section, "never-default");
                #[doc = concat!("`", $section, ".may-fail`")]
                pub const MAY_FAIL: Property<bool> = Property::new($section, "may-fail");
                #[doc = concat!("`", $section, ".route-metric`")]
                pub const ROUTE_METRIC: Property<i64> = Property::new($section, "route-metric");
                #[doc = concat!("`", $section, ".route-table`")]
                pub const ROUTE_TABLE: Property<u32> = Property::new($section, "route-table");
                #[doc = concat!("`", $section, ".dhcp-hostname`")]
                pub const DHCP_HOSTNAME: Property<String> =
                    Property::new($section, "dhcp-hostname");
                #[doc = concat!("`", $section, ".dhcp-send-hostname`")]
                pub const DHCP_SEND_HOSTNAME: Property<bool> =
                    Property::new($section, "dhcp-send-hostname");
                #[doc = concat!("`", $section, ".dhcp-timeout`")]
                pub const DHCP_TIMEOUT: Property<i32> = Property::new($section, "dhcp-timeout");
                #[doc = concat!("`", $section, ".dhcp-iaid`")]
                pub const DHCP_IAID: Property<String> = Property::new($section, "dhcp-iaid");
            }
        };
    }

    ip_section!(ipv4, "ipv4");
    ip_section!(ipv6, "ipv6");

    /// `ipv4.*` keys that have no IPv6 counterpart.
    pub mod ipv4_only {
        use super::Property;

        /// `ipv4.dhcp-client-id`
        pub const DHCP_CLIENT_ID: Property<String> = Property::new("ipv4", "dhcp-client-id");
        /// `ipv4.dhcp-vendor-class-identifier`
        pub const DHCP_VENDOR_CLASS_IDENTIFIER: Property<String> =
            Property::new("ipv4", "dhcp-vendor-class-identifier");
        /// `ipv4.link-local`
        pub const LINK_LOCAL: Property<i32> = Property::new("ipv4", "link-local");
    }

    /// `ipv6.*` keys that have no IPv4 counterpart.
    pub mod ipv6_only {
        use super::Property;

        /// `ipv6.addr-gen-mode`
        pub const ADDR_GEN_MODE: Property<i32> = Property::new("ipv6", "addr-gen-mode");
        /// `ipv6.ip6-privacy`
        pub const IP6_PRIVACY: Property<i32> = Property::new("ipv6", "ip6-privacy");
        /// `ipv6.dhcp-duid`
        pub const DHCP_DUID: Property<String> = Property::new("ipv6", "dhcp-duid");
        /// `ipv6.token`
        pub const TOKEN: Property<String> = Property::new("ipv6", "token");
        /// `ipv6.ra-timeout`
        pub const RA_TIMEOUT: Property<i32> = Property::new("ipv6", "ra-timeout");
    }

    /// `802-11-wireless.*`
    pub mod wifi {
        use super::Property;

        /// `802-11-wireless.ssid` (raw bytes)
        pub const SSID: Property<Vec<u8>> = Property::new("802-11-wireless", "ssid");
        /// `802-11-wireless.mode`
        pub const MODE: Property<String> = Property::new("802-11-wireless", "mode");
        /// `802-11-wireless.band`
        pub const BAND: Property<String> = Property::new("802-11-wireless", "band");
        /// `802-11-wireless.channel`
        pub const CHANNEL: Property<u32> = Property::new("802-11-wireless", "channel");
        /// `802-11-wireless.bssid`
        pub const BSSID: Property<Vec<u8>> = Property::new("802-11-wireless", "bssid");
        /// `802-11-wireless.hidden`
        pub const HIDDEN: Property<bool> = Property::new("802-11-wireless", "hidden");
        /// `802-11-wireless.mac-address`
        pub const MAC_ADDRESS: Property<String> = Property::new("802-11-wireless", "mac-address");
        /// `802-11-wireless.cloned-mac-address`
        pub const CLONED_MAC_ADDRESS: Property<String> =
            Property::new("802-11-wireless", "cloned-mac-address");
        /// `802-11-wireless.mac-address-randomization`
        pub const MAC_ADDRESS_RANDOMIZATION: Property<u32> =
            Property::new("802-11-wireless", "mac-address-randomization");
        /// `802-11-wireless.mtu`
        pub const MTU: Property<u32> = Property::new("802-11-wireless", "mtu");
        /// `802-11-wireless.powersave`
        pub const POWERSAVE: Property<u32> = Property::new("802-11-wireless", "powersave");
        /// `802-11-wireless.seen-bssids`
        pub const SEEN_BSSIDS: Property<Vec<String>> =
            Property::new("802-11-wireless", "seen-bssids");
        /// `802-11-wireless.wake-on-wlan`
        pub const WAKE_ON_WLAN: Property<u32> = Property::new("802-11-wireless", "wake-on-wlan");
    }

    /// `802-11-wireless-security.*` (no secrets: `GetSettings` omits them)
    pub mod wifi_security {
        use super::Property;

        /// `802-11-wireless-security.key-mgmt`
        pub const KEY_MGMT: Property<String> =
            Property::new("802-11-wireless-security", "key-mgmt");
        /// `802-11-wireless-security.auth-alg`
        pub const AUTH_ALG: Property<String> =
            Property::new("802-11-wireless-security", "auth-alg");
        /// `802-11-wireless-security.proto`
        pub const PROTO: Property<Vec<String>> = Property::new("802-11-wireless-security", "proto");
        /// `802-11-wireless-security.pairwise`
        pub const PAIRWISE: Property<Vec<String>> =
            Property::new("802-11-wireless-security", "pairwise");
        /// `802-11-wireless-security.group`
        pub const GROUP: Property<Vec<String>> = Property::new("802-11-wireless-security", "group");
        /// `802-11-wireless-security.pmf`
        pub const PMF: Property<i32> = Property::new("802-11-wireless-security", "pmf");
        /// `802-11-wireless-security.psk-flags`
        pub const PSK_FLAGS: Property<u32> = Property::new("802-11-wireless-security", "psk-flags");
        /// `802-11-wireless-security.wep-key-type`
        pub const WEP_KEY_TYPE: Property<u32> =
            Property::new("802-11-wireless-security", "wep-key-type");
    }

    /// `802-3-ethernet.*`
    pub mod ethernet {
        use super::Property;

        /// `802-3-ethernet.mac-address`
        pub const MAC_ADDRESS: Property<String> = Property::new("802-3-ethernet", "mac-address");
        /// `802-3-ethernet.cloned-mac-address`
        pub const CLONED_MAC_ADDRESS: Property<String> =
            Property::new("802-3-ethernet", "cloned-mac-address");
        /// `802-3-ethernet.auto-negotiate`
        pub const AUTO_NEGOTIATE: Property<bool> =
            Property::new("802-3-ethernet", "auto-negotiate");
        /// `802-3-ethernet.speed`
        pub const SPEED: Property<u32> = Property::new("802-3-ethernet", "speed");
        /// `802-3-ethernet.duplex`
        pub const DUPLEX: Property<String> = Property::new("802-3-ethernet", "duplex");
        /// `802-3-ethernet.mtu`
        pub const MTU: Property<u32> = Property::new("802-3-ethernet", "mtu");
        /// `802-3-ethernet.wake-on-lan`
        pub const WAKE_ON_LAN: Property<u32> = Property::new("802-3-ethernet", "wake-on-lan");
    }

    /// `vpn.*`
    pub mod vpn {
        use super::Property;

        /// `vpn.service-type`
        pub const SERVICE_TYPE: Property<String> = Property::new("vpn", "service-type");
        /// `vpn.user-name`
        pub const USER_NAME: Property<String> = Property::new("vpn", "user-name");
        /// `vpn.persistent`
        pub const PERSISTENT: Property<bool> = Property::new("vpn", "persistent");
        /// `vpn.timeout`
        pub const TIMEOUT: Property<u32> = Property::new("vpn", "timeout");
    }

    /// `wireguard.*`
    pub mod wireguard {
        use super::Property;

        /// `wireguard.listen-port`
        pub const LISTEN_PORT: Property<u32> = Property::new("wireguard", "listen-port");
        /// `wireguard.fwmark`
        pub const FWMARK: Property<u32> = Property::new("wireguard", "fwmark");
        /// `wireguard.mtu`
        pub const MTU: Property<u32> = Property::new("wireguard", "mtu");
        /// `wireguard.ip4-auto-default-route`
        pub const IP4_AUTO_DEFAULT_ROUTE: Property<i32> =
            Property::new("wireguard", "ip4-auto-default-route");
        /// `wireguard.ip6-auto-default-route`
        pub const IP6_AUTO_DEFAULT_ROUTE: Property<i32> =
            Property::new("wireguard", "ip6-auto-default-route");
        /// `wireguard.peer-routes`
        pub const PEER_ROUTES: Property<bool> = Property::new("wireguard", "peer-routes");
        /// `wireguard.private-key-flags`
        pub const PRIVATE_KEY_FLAGS: Property<u32> =
            Property::new("wireguard", "private-key-flags");
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use zvariant::Str;

    fn boxed(value: Value<'static>) -> OwnedValue {
        OwnedValue::try_from(Value::Value(Box::new(value))).expect("owned boxed value")
    }

    #[test]
    fn scalars_decode_boxed_and_unboxed() {
        assert_eq!(bool::from_setting(&OwnedValue::from(true)), Some(true));
        assert_eq!(bool::from_setting(&boxed(Value::from(false))), Some(false));
        assert_eq!(i32::from_setting(&OwnedValue::from(-3i32)), Some(-3));
        assert_eq!(u32::from_setting(&boxed(Value::from(7u32))), Some(7));
        assert_eq!(u64::from_setting(&OwnedValue::from(9u64)), Some(9));
        assert_eq!(
            String::from_setting(&OwnedValue::from(Str::from("zone"))),
            Some("zone".to_string())
        );
        assert_eq!(
            String::from_setting(&boxed(Value::from("boxed"))),
            Some("boxed".to_string())
        );
    }

    #[test]
    fn wrong_type_reads_as_absent() {
        let text = OwnedValue::from(Str::from("not a number"));
        assert_eq!(u32::from_setting(&text), None);
        assert_eq!(bool::from_setting(&text), None);
        assert_eq!(Vec::<String>::from_setting(&text), None);
        assert_eq!(String::from_setting(&OwnedValue::from(1u32)), None);
    }

    #[test]
    fn arrays_decode_elements() {
        let strings = OwnedValue::try_from(Value::from(vec!["a".to_string(), "b".to_string()]))
            .expect("owned as");
        assert_eq!(
            Vec::<String>::from_setting(&strings),
            Some(vec!["a".to_string(), "b".to_string()])
        );
        let bytes = OwnedValue::try_from(Value::from(vec![1u8, 2, 3])).expect("owned ay");
        assert_eq!(Vec::<u8>::from_setting(&bytes), Some(vec![1, 2, 3]));
        let ints = OwnedValue::try_from(Value::from(vec![10u32, 20])).expect("owned au");
        assert_eq!(Vec::<u32>::from_setting(&ints), Some(vec![10, 20]));
        // Mixed element types are not silently truncated.
        assert_eq!(Vec::<u8>::from_setting(&ints), None);
    }

    #[test]
    fn raw_value_is_available_for_anything() {
        let raw = OwnedValue::from(42u16);
        let copy = OwnedValue::from_setting(&raw).expect("clone");
        assert_eq!(u16::from_setting(&copy), Some(42));
    }
}
