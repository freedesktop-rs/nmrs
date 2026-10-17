//! NetworkManager connection settings builder.
//!
//! Constructs the D-Bus settings dictionaries required by NetworkManager's
//! `AddAndActivateConnection` method. These settings define the connection
//! type, security parameters, and IP configuration.
//!
//! # NetworkManager Settings Structure
//!
//! A connection is represented as a nested dictionary:
//! - `connection`: General settings (type, id, uuid, autoconnect)
//! - `802-11-wireless`: Wi-Fi specific settings (ssid, mode, security reference)
//! - `802-11-wireless-security`: Security settings (key-mgmt, psk, auth-alg)
//! - `802-1x`: Enterprise authentication settings (for WPA-EAP)
//! - `ipv4` / `ipv6`: IP configuration (usually "auto" for DHCP)
//!
//! # New Builder API
//!
//! For new code, consider using the builder API from `wifi_builder` module:
//!
//! ```rust
//! use nmrs::builders::WifiConnectionBuilder;
//!
//! let settings = WifiConnectionBuilder::new("MyNetwork")
//!     .wpa_psk("password")
//!     .autoconnect(true)
//!     .ipv4_auto()
//!     .ipv6_auto()
//!     .build();
//! ```

use std::collections::HashMap;
use zvariant::Value;

use super::connection_builder::ConnectionBuilder;
use super::wifi_builder::WifiConnectionBuilder;
use crate::api::models::{self, ConnectionError, ConnectionOptions};

/// Builds a complete Wi-Fi connection settings dictionary.
///
/// Constructs all required sections for NetworkManager based on the
/// security type. The returned dictionary can be passed directly to
/// `AddAndActivateConnection`.
///
/// # Sections Created
///
/// - `connection`: Always present
/// - `802-11-wireless`: Always present
/// - `ipv4` / `ipv6`: Always present (set to "auto" for DHCP)
/// - `802-11-wireless-security`: Present for PSK and EAP networks
/// - `802-1x`: Present only for EAP networks
///
/// # Note
///
/// This function always creates an infrastructure-mode connection. For access
/// point (hotspot) or ad-hoc connections, use [`WifiConnectionBuilder`] directly
/// with [`WifiMode`](super::wifi_builder::WifiMode).
///
/// This function is maintained for backward compatibility. For new code,
/// consider using `WifiConnectionBuilder` for a more ergonomic API.
///
/// If an EAP certificate or key is supplied as both a path and a blob, the path
/// is used and a warning is logged. Prefer [`try_build_wifi_connection`], which
/// reports the conflict as an error instead.
#[must_use]
#[deprecated(
    since = "3.5.0",
    note = "use `try_build_wifi_connection`, which reports conflicting EAP certificate path/blob inputs as an error instead of silently preferring the path"
)]
pub fn build_wifi_connection(
    ssid: &str,
    security: &models::WifiSecurity,
    opts: &ConnectionOptions,
) -> HashMap<&'static str, HashMap<&'static str, Value<'static>>> {
    // The deprecated EAP builders warn about path/blob conflicts themselves.
    #[allow(deprecated)]
    let builder = match security {
        models::WifiSecurity::Open => base_wifi_builder(ssid, opts).open(),
        models::WifiSecurity::WpaPsk { psk } => base_wifi_builder(ssid, opts).wpa_psk(psk),
        models::WifiSecurity::Sae { psk } => base_wifi_builder(ssid, opts).sae(psk),
        models::WifiSecurity::WpaEap { opts: eap } => {
            base_wifi_builder(ssid, opts).wpa_eap(eap.clone())
        }
        models::WifiSecurity::Wpa3Eap192bit { opts: eap } => {
            base_wifi_builder(ssid, opts).wpa3_eap_192_bit(eap.clone())
        }
    };

    builder.build()
}

/// Builds a complete Wi-Fi connection settings dictionary, reporting invalid
/// EAP input instead of silently resolving it.
///
/// Behaves exactly like [`build_wifi_connection`] for every valid input.
///
/// # Errors
///
/// Returns [`ConnectionError::InvalidInput`] when an EAP certificate or key
/// is supplied as both a path and a blob.
///
/// # Example
///
/// ```rust
/// use nmrs::builders::try_build_wifi_connection;
/// use nmrs::{ConnectionOptions, WifiSecurity};
///
/// let settings = try_build_wifi_connection(
///     "MyNetwork",
///     &WifiSecurity::WpaPsk { psk: "password".into() },
///     &ConnectionOptions::new(true),
/// )?;
/// # Ok::<(), nmrs::ConnectionError>(())
/// ```
pub fn try_build_wifi_connection(
    ssid: &str,
    security: &models::WifiSecurity,
    opts: &ConnectionOptions,
) -> Result<HashMap<&'static str, HashMap<&'static str, Value<'static>>>, ConnectionError> {
    let base = base_wifi_builder(ssid, opts);

    let builder = match security {
        models::WifiSecurity::Open => base.open(),
        models::WifiSecurity::WpaPsk { psk } => base.wpa_psk(psk),
        models::WifiSecurity::Sae { psk } => base.sae(psk),
        models::WifiSecurity::WpaEap { opts: eap } => base.try_wpa_eap(eap.clone())?,
        models::WifiSecurity::Wpa3Eap192bit { opts: eap } => {
            base.try_wpa3_eap_192_bit(eap.clone())?
        }
    };

    Ok(builder.build())
}

fn base_wifi_builder(ssid: &str, opts: &ConnectionOptions) -> WifiConnectionBuilder {
    WifiConnectionBuilder::new(ssid)
        .options(opts)
        .ipv4_auto()
        .ipv6_auto()
}

/// Builds a complete Ethernet connection settings dictionary.
///
/// Constructs all required sections for NetworkManager. The returned dictionary
/// can be passed directly to `AddAndActivateConnection`.
///
/// # Sections Created
///
/// - `connection`: Always present (type: "802-3-ethernet")
/// - `802-3-ethernet`: Ethernet-specific settings (currently empty, can be extended)
/// - `ipv4` / `ipv6`: Always present (set to "auto" for DHCP)
///
/// # Note
///
/// This function is maintained for backward compatibility. For new code,
/// consider using `EthernetConnectionBuilder` for a more ergonomic API.
#[must_use]
pub fn build_ethernet_connection(
    connection_id: &str,
    opts: &ConnectionOptions,
) -> HashMap<&'static str, HashMap<&'static str, Value<'static>>> {
    let ethernet = HashMap::new();

    ConnectionBuilder::new("802-3-ethernet", connection_id)
        .options(opts)
        .with_section("802-3-ethernet", ethernet)
        .ipv4_auto()
        .ipv6_auto()
        .build()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::{ConnectionOptions, EapMethod, EapOptions, Phase2, WifiSecurity};
    use zvariant::Value;

    fn build_wifi_connection(
        ssid: &str,
        security: &WifiSecurity,
        opts: &ConnectionOptions,
    ) -> HashMap<&'static str, HashMap<&'static str, Value<'static>>> {
        super::try_build_wifi_connection(ssid, security, opts).expect("valid Wi-Fi settings")
    }

    fn default_opts() -> ConnectionOptions {
        ConnectionOptions {
            autoconnect: true,
            autoconnect_priority: None,
            autoconnect_retries: None,
        }
    }

    fn opts_with_priority() -> ConnectionOptions {
        ConnectionOptions {
            autoconnect: false,
            autoconnect_priority: Some(10),
            autoconnect_retries: Some(3),
        }
    }

    #[test]
    fn builds_open_wifi_connection() {
        let conn = build_wifi_connection("testnet", &WifiSecurity::Open, &default_opts());
        assert!(conn.contains_key("connection"));
        assert!(conn.contains_key("802-11-wireless"));
        assert!(conn.contains_key("ipv4"));
        assert!(conn.contains_key("ipv6"));
        // Open networks should NOT have security section
        assert!(!conn.contains_key("802-11-wireless-security"));
    }

    #[test]
    fn conflicting_eap_ca_cert_path_and_blob_returns_invalid_input() {
        let mut opts = EapOptions::new("user@example.com", "secret");
        opts.ca_cert_path = Some("file:///etc/ssl/certs/ca.pem".into());
        opts.ca_cert_blob = Some(vec![1, 2, 3]);

        match super::try_build_wifi_connection(
            "enterprise",
            &WifiSecurity::WpaEap { opts },
            &default_opts(),
        ) {
            Err(ConnectionError::InvalidInput { field, reason }) => {
                assert_eq!(field, "ca_cert");
                assert_eq!(reason, "cannot specify both ca_cert_path and ca_cert_blob");
            }
            Ok(_) => panic!("conflicting EAP certificate inputs should be rejected"),
            Err(error) => panic!("expected InvalidInput, got {error:?}"),
        }
    }

    #[test]
    fn sae_security_maps_to_the_sae_builder() {
        let conn = build_wifi_connection(
            "wpa3net",
            &WifiSecurity::Sae {
                psk: "password123".into(),
            },
            &default_opts(),
        );

        let security = conn
            .get("802-11-wireless-security")
            .expect("SAE must produce a security section");
        assert_eq!(security.get("key-mgmt"), Some(&Value::from("sae")));
        assert!(security.get("auth-alg").is_none());
    }

    #[test]
    fn open_connection_has_correct_type() {
        let conn = build_wifi_connection("open_net", &WifiSecurity::Open, &default_opts());
        let connection_section = conn.get("connection").unwrap();
        assert_eq!(
            connection_section.get("type"),
            Some(&Value::from("802-11-wireless"))
        );
    }

    #[test]
    fn builds_psk_wifi_connection_with_security_section() {
        let conn = build_wifi_connection(
            "secure",
            &WifiSecurity::WpaPsk {
                psk: "pw123".into(),
            },
            &default_opts(),
        );
        assert!(
            conn.contains_key("802-11-wireless-security"),
            "security section missing"
        );
        let sec = conn.get("802-11-wireless-security").unwrap();
        assert_eq!(sec.get("psk"), Some(&Value::from("pw123".to_string())));
        assert_eq!(sec.get("key-mgmt"), Some(&Value::from("wpa-psk")));
    }

    #[test]
    fn psk_connection_links_wireless_to_security() {
        let conn = build_wifi_connection(
            "secure",
            &WifiSecurity::WpaPsk { psk: "test".into() },
            &default_opts(),
        );
        let wireless = conn.get("802-11-wireless").unwrap();
        assert_eq!(
            wireless.get("security"),
            Some(&Value::from("802-11-wireless-security"))
        );
    }

    #[test]
    fn builds_eap_peap_connection() {
        let eap_opts = EapOptions {
            identity: "user@example.com".into(),
            password: "secret123".into(),
            anonymous_identity: Some("anonymous@example.com".into()),
            domain_suffix_match: Some("example.com".into()),
            ca_cert_path: None,
            ca_cert_blob: None,
            system_ca_certs: true,
            method: EapMethod::Peap,
            phase2: Phase2::Mschapv2,
            private_key_path: None,
            private_key_blob: None,
            private_key_password: None,
            client_cert_path: None,
            client_cert_blob: None,
        };
        let conn = build_wifi_connection(
            "enterprise",
            &WifiSecurity::WpaEap { opts: eap_opts },
            &default_opts(),
        );

        assert!(conn.contains_key("802-11-wireless-security"));
        assert!(conn.contains_key("802-1x"));

        let sec = conn.get("802-11-wireless-security").unwrap();
        assert_eq!(sec.get("key-mgmt"), Some(&Value::from("wpa-eap")));

        let e1x = conn.get("802-1x").unwrap();
        assert_eq!(
            e1x.get("identity"),
            Some(&Value::from("user@example.com".to_string()))
        );
        assert_eq!(
            e1x.get("password"),
            Some(&Value::from("secret123".to_string()))
        );
        assert_eq!(e1x.get("phase2-auth"), Some(&Value::from("mschapv2")));
        assert_eq!(e1x.get("system-ca-certs"), Some(&Value::from(true)));
    }

    #[test]
    fn builds_eap_ttls_connection() {
        let eap_opts = EapOptions {
            identity: "student@uni.edu".into(),
            password: "campus123".into(),
            anonymous_identity: None,
            domain_suffix_match: None,
            ca_cert_path: Some("file:///etc/ssl/certs/ca.pem".into()),
            ca_cert_blob: None,
            system_ca_certs: false,
            method: EapMethod::Ttls,
            phase2: Phase2::Pap,
            private_key_path: None,
            private_key_blob: None,
            private_key_password: None,
            client_cert_path: None,
            client_cert_blob: None,
        };
        let conn = build_wifi_connection(
            "eduroam",
            &WifiSecurity::WpaEap { opts: eap_opts },
            &default_opts(),
        );

        let e1x = conn.get("802-1x").unwrap();
        assert_eq!(e1x.get("phase2-auth"), Some(&Value::from("pap")));
        assert_eq!(
            e1x.get("ca-cert"),
            Some(&Value::from(b"file:///etc/ssl/certs/ca.pem\0".to_vec()))
        );
        // system-ca-certs should NOT be present when false
        assert!(e1x.get("system-ca-certs").is_none());
    }

    #[test]
    fn builds_eap_tls_connection() {
        let eap_opts = EapOptions {
            identity: "student@uni.edu".into(),
            password: String::new(),
            anonymous_identity: None,
            domain_suffix_match: None,
            ca_cert_path: Some("file:///etc/ssl/certs/ca.pem".into()),
            ca_cert_blob: None,
            system_ca_certs: false,
            method: EapMethod::Tls,
            phase2: Phase2::Mschapv2,
            private_key_path: Some("file:///etc/ssl/private/client.key".into()),
            private_key_blob: None,
            private_key_password: Some("password".into()),
            client_cert_path: Some("file:///etc/ssl/certs/client.crt".into()),
            client_cert_blob: None,
        };
        let conn = build_wifi_connection(
            "eduroam",
            &WifiSecurity::WpaEap { opts: eap_opts },
            &default_opts(),
        );

        let security = conn.get("802-11-wireless-security").unwrap();
        assert_eq!(security.get("key-mgmt"), Some(&Value::from("wpa-eap")));

        let e1x = conn.get("802-1x").unwrap();
        assert_eq!(e1x.get("phase2-auth"), None);
        assert_eq!(
            e1x.get("private-key"),
            Some(&Value::from(
                b"file:///etc/ssl/private/client.key\0".to_vec()
            ))
        );
        assert_eq!(
            e1x.get("private-key-password"),
            Some(&Value::from("password"))
        );
        assert_eq!(
            e1x.get("client-cert"),
            Some(&Value::from(b"file:///etc/ssl/certs/client.crt\0".to_vec()))
        );
        assert_eq!(
            e1x.get("ca-cert"),
            Some(&Value::from(b"file:///etc/ssl/certs/ca.pem\0".to_vec()))
        );
        // system-ca-certs should NOT be present when false
        assert!(e1x.get("system-ca-certs").is_none());
    }

    #[test]
    fn builds_eap_192bit_connection() {
        let eap_opts = EapOptions {
            identity: "student@uni.edu".into(),
            password: String::new(),
            anonymous_identity: None,
            domain_suffix_match: None,
            ca_cert_path: None,
            ca_cert_blob: Some(b"ca_cert_blob".into()),
            system_ca_certs: false,
            method: EapMethod::Tls,
            phase2: Phase2::Mschapv2,
            private_key_path: None,
            private_key_blob: Some(b"private_key_blob".into()),
            private_key_password: Some("password".into()),
            client_cert_path: None,
            client_cert_blob: Some(b"client_cert_blob".into()),
        };
        let conn = build_wifi_connection(
            "eduroam",
            &WifiSecurity::Wpa3Eap192bit { opts: eap_opts },
            &default_opts(),
        );

        let security = conn.get("802-11-wireless-security").unwrap();
        assert_eq!(
            security.get("key-mgmt"),
            Some(&Value::from("wpa-eap-suite-b-192"))
        );

        let e1x = conn.get("802-1x").unwrap();
        assert_eq!(e1x.get("phase2-auth"), None);
        assert_eq!(
            e1x.get("private-key"),
            Some(&Value::from(b"private_key_blob".to_vec()))
        );
        assert_eq!(
            e1x.get("private-key-password"),
            Some(&Value::from("password"))
        );
        assert_eq!(
            e1x.get("client-cert"),
            Some(&Value::from(b"client_cert_blob".to_vec()))
        );
        assert_eq!(
            e1x.get("ca-cert"),
            Some(&Value::from(b"ca_cert_blob".to_vec()))
        );
        // system-ca-certs should NOT be present when false
        assert!(e1x.get("system-ca-certs").is_none());
    }

    #[test]
    fn connection_with_priority_and_retries() {
        let conn =
            build_wifi_connection("priority_net", &WifiSecurity::Open, &opts_with_priority());
        let connection_section = conn.get("connection").unwrap();

        assert_eq!(
            connection_section.get("autoconnect"),
            Some(&Value::from(false))
        );
        assert_eq!(
            connection_section.get("autoconnect-priority"),
            Some(&Value::from(10i32))
        );
        assert_eq!(
            connection_section.get("autoconnect-retries"),
            Some(&Value::from(3i32))
        );
    }

    #[test]
    fn connection_without_optional_fields() {
        let conn = build_wifi_connection("simple", &WifiSecurity::Open, &default_opts());
        let connection_section = conn.get("connection").unwrap();

        assert_eq!(
            connection_section.get("autoconnect"),
            Some(&Value::from(true))
        );
        // Optional fields should not be present
        assert!(connection_section.get("autoconnect-priority").is_none());
        assert!(connection_section.get("autoconnect-retries").is_none());
    }

    #[test]
    fn ssid_is_stored_as_bytes() {
        let conn = build_wifi_connection("MyNetwork", &WifiSecurity::Open, &default_opts());
        let wireless = conn.get("802-11-wireless").unwrap();
        let ssid = wireless.get("ssid").unwrap();
        assert_eq!(ssid, &Value::from(b"MyNetwork".to_vec()));
    }

    #[test]
    fn ssid_with_special_characters() {
        let conn = build_wifi_connection("Café-Wïfì_123", &WifiSecurity::Open, &default_opts());
        let wireless = conn.get("802-11-wireless").unwrap();
        let ssid = wireless.get("ssid").unwrap();
        assert_eq!(ssid, &Value::from("Café-Wïfì_123".as_bytes().to_vec()));
    }

    #[test]
    fn connection_with_negative_priority_and_zero_retries() {
        let opts = ConnectionOptions {
            autoconnect: true,
            autoconnect_priority: Some(-5),
            autoconnect_retries: Some(0), // 0 means try indefinitely in NM
        };

        let conn = build_wifi_connection("fallback_net", &WifiSecurity::Open, &opts);
        let connection_section = conn.get("connection").unwrap();

        assert_eq!(
            connection_section.get("autoconnect-priority"),
            Some(&Value::from(-5i32))
        );
        assert_eq!(
            connection_section.get("autoconnect-retries"),
            Some(&Value::from(0i32))
        );
    }

    #[test]
    fn connection_with_max_valid_boundaries() {
        let opts = ConnectionOptions {
            autoconnect: true,
            autoconnect_priority: Some(999),     // NM max priority
            autoconnect_retries: Some(i32::MAX), // NM max retries
        };

        let conn = build_wifi_connection("max_boundaries_net", &WifiSecurity::Open, &opts);
        let connection_section = conn.get("connection").unwrap();

        assert_eq!(
            connection_section.get("autoconnect-priority"),
            Some(&Value::from(999i32))
        );
        assert_eq!(
            connection_section.get("autoconnect-retries"),
            Some(&Value::from(i32::MAX))
        );
    }

    #[test]
    fn connection_with_min_valid_boundaries() {
        let opts = ConnectionOptions {
            autoconnect: true,
            autoconnect_priority: Some(-999), // NM min priority
            autoconnect_retries: Some(-1),    // NM default retries
        };

        let conn = build_wifi_connection("min_boundaries_net", &WifiSecurity::Open, &opts);
        let connection_section = conn.get("connection").unwrap();

        assert_eq!(
            connection_section.get("autoconnect-priority"),
            Some(&Value::from(-999i32))
        );
        assert_eq!(
            connection_section.get("autoconnect-retries"),
            Some(&Value::from(-1i32))
        );
    }
}
