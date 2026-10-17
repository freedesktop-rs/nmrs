//! Secret agent request and response types.

use std::collections::HashMap;

use log::warn;
use zvariant::{OwnedObjectPath, OwnedValue, Str};

use crate::ConnectionError;

bitflags::bitflags! {
    /// Flags passed by NetworkManager with a `GetSecrets` request.
    ///
    /// These correspond to `NMSecretAgentGetSecretsFlags` in the NetworkManager
    /// D-Bus API.
    ///
    /// Reference: <https://networkmanager.dev/docs/api/latest/nm-dbus-types.html#NMSecretAgentGetSecretsFlags>
    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub struct SecretAgentFlags: u32 {
        /// The agent may interact with the user (e.g. show a dialog).
        const ALLOW_INTERACTION = 0x1;
        /// The agent should discard cached secrets and prompt again.
        const REQUEST_NEW = 0x2;
        /// The request was triggered by an explicit user action, not auto-connect.
        const USER_REQUESTED = 0x4;
        /// WPS push-button mode is active on the access point.
        const WPS_PBC_ACTIVE = 0x8;
    }
}

bitflags::bitflags! {
    /// Capabilities advertised when registering the agent with NetworkManager.
    ///
    /// These correspond to `NMSecretAgentCapabilities` in the NetworkManager
    /// D-Bus API.
    ///
    /// Reference: <https://networkmanager.dev/docs/api/latest/nm-dbus-types.html#NMSecretAgentCapabilities>
    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub struct SecretAgentCapabilities: u32 {
        /// The agent supports VPN secret hints, allowing NetworkManager to
        /// send a list of required secret keys instead of the full setting.
        const VPN_HINTS = 0x1;
    }
}

/// Identifies which connection setting needs secrets.
///
/// NetworkManager sends the setting name as part of a `GetSecrets` request.
/// This enum parses common setting names and extracts relevant context from
/// the connection dictionary.
#[non_exhaustive]
#[derive(Debug, Clone)]
pub enum SecretSetting {
    /// 802.11 wireless security — typically a WPA/WPA2 PSK.
    WifiPsk {
        /// The SSID of the network requesting credentials.
        ssid: String,
    },
    /// 802.1X EAP authentication.
    WifiEap {
        /// The identity (username) if already configured.
        identity: Option<String>,
        /// The EAP method if already configured (e.g. `"peap"`, `"ttls"`).
        method: Option<String>,
    },
    /// VPN secrets (password, OTP, etc.).
    Vpn {
        /// The D-Bus service name of the VPN plugin
        /// (e.g. `"org.freedesktop.NetworkManager.openvpn"`).
        service_type: String,
        /// The VPN username if already configured.
        user_name: Option<String>,
    },
    /// GSM/mobile broadband secrets.
    Gsm,
    /// CDMA mobile broadband secrets.
    Cdma,
    /// PPPoE secrets.
    Pppoe,
    /// A setting name not recognized by this library.
    Other(String),
}

/// A request from NetworkManager for connection secrets.
///
/// When NetworkManager needs credentials it does not have (e.g. a Wi-Fi
/// password was forgotten, a VPN token expired), it calls the registered
/// secret agent's `GetSecrets` method. This struct is the parsed, high-level
/// representation of that call.
///
/// Respond using the [`responder`](Self::responder) field. If the responder is
/// dropped without a response method being called, the agent auto-replies with
/// `NoSecrets` and logs a warning.
///
/// # Example
///
/// ```no_run
/// use futures::StreamExt;
/// use nmrs::agent::{SecretAgent, SecretAgentFlags, SecretSetting};
///
/// # async fn example() -> nmrs::Result<()> {
/// let (handle, mut requests) = SecretAgent::builder().register().await?;
///
/// while let Some(req) = requests.next().await {
///     println!("secrets requested for {}", req.connection_id);
///     if let SecretSetting::WifiPsk { ref ssid } = req.setting {
///         req.responder.wifi_psk("hunter2").await?;
///     }
/// }
/// # Ok(())
/// # }
/// ```
#[non_exhaustive]
pub struct SecretRequest {
    /// UUID of the connection needing secrets.
    pub connection_uuid: String,
    /// Human-readable name of the connection (e.g. `"MyWiFi"`).
    pub connection_id: String,
    /// Connection type string (e.g. `"802-11-wireless"`, `"vpn"`).
    pub connection_type: String,
    /// D-Bus object path of the connection settings object.
    pub connection_path: OwnedObjectPath,
    /// Which setting section needs secrets.
    pub setting: SecretSetting,
    /// Optional hints from NetworkManager about which secrets are needed.
    pub hints: Vec<String>,
    /// Flags describing the context of the request.
    pub flags: SecretAgentFlags,
    /// The responder used to reply with secrets or cancel.
    pub responder: SecretResponder,
    /// Existing secrets NetworkManager sent in the `GetSecrets` payload, for
    /// pre-filling a re-authentication prompt. Currently only populated for
    /// `vpn` connections, and only for system-owned secrets.
    pub existing_secrets: HashMap<String, String>,
}

impl std::fmt::Debug for SecretRequest {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SecretRequest")
            .field("connection_uuid", &self.connection_uuid)
            .field("connection_id", &self.connection_id)
            .field("connection_type", &self.connection_type)
            .field("connection_path", &self.connection_path)
            .field("setting", &self.setting)
            .field("hints", &self.hints)
            .field("flags", &self.flags)
            .finish_non_exhaustive()
    }
}

/// Sends secrets (or a refusal) back to NetworkManager.
///
/// Each `SecretResponder` must be consumed exactly once by calling one of its
/// response methods. If dropped without being consumed, it auto-replies with
/// `NoSecrets` and logs a warning.
///
/// The response methods consume `self` to enforce single-use semantics.
pub struct SecretResponder {
    reply_tx: Option<futures::channel::oneshot::Sender<SecretReply>>,
    setting_name: String,
}

impl std::fmt::Debug for SecretResponder {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SecretResponder")
            .field("setting_name", &self.setting_name)
            .field("consumed", &self.reply_tx.is_none())
            .finish()
    }
}

/// A cancellation notification from NetworkManager.
///
/// Emitted when NetworkManager calls `CancelGetSecrets` for an in-flight
/// request. By the time this is received, the agent has already replied to
/// NetworkManager on the consumer's behalf.
#[non_exhaustive]
#[derive(Debug, Clone)]
pub struct CancelReason {
    /// D-Bus object path of the cancelled connection.
    pub connection_path: OwnedObjectPath,
    /// The setting section that was being requested.
    pub setting_name: String,
}

/// A save or delete event from NetworkManager.
///
/// NetworkManager sends `SaveSecrets` and `DeleteSecrets` so agents can
/// persist or remove secrets from a keyring. Since `nmrs` delegates
/// persistence to the consumer, these are exposed as optional events.
#[non_exhaustive]
#[derive(Debug, Clone)]
pub enum SecretStoreEvent {
    /// NetworkManager asked the agent to persist secrets for a connection.
    Save {
        /// D-Bus object path of the connection.
        connection_path: OwnedObjectPath,
    },
    /// NetworkManager asked the agent to delete stored secrets.
    Delete {
        /// D-Bus object path of the connection.
        connection_path: OwnedObjectPath,
    },
}

pub(crate) type ConnectionDict = HashMap<String, HashMap<String, OwnedValue>>;

pub(crate) enum SecretReply {
    Secrets(ConnectionDict),
    UserCanceled,
    NoSecrets,
}

impl SecretResponder {
    pub(crate) fn new(
        reply_tx: futures::channel::oneshot::Sender<SecretReply>,
        setting_name: String,
    ) -> Self {
        Self {
            reply_tx: Some(reply_tx),
            setting_name,
        }
    }

    /// Respond with a Wi-Fi PSK (pre-shared key / password).
    ///
    /// This is the most common response for WPA/WPA2-Personal networks.
    ///
    /// # Errors
    ///
    /// Returns an error if the reply channel is already closed (e.g. the
    /// request was cancelled by NetworkManager).
    pub async fn wifi_psk(mut self, psk: impl Into<String>) -> crate::Result<()> {
        let mut inner = HashMap::new();
        inner.insert("psk".to_owned(), OwnedValue::from(Str::from(psk.into())));
        let mut outer = HashMap::new();
        outer.insert("802-11-wireless-security".to_owned(), inner);
        self.send_reply(SecretReply::Secrets(outer))
    }

    /// Respond with 802.1X EAP credentials.
    ///
    /// # Errors
    ///
    /// Returns an error if the reply channel is already closed.
    pub async fn wifi_eap(
        mut self,
        identity: Option<String>,
        password: String,
    ) -> crate::Result<()> {
        let mut inner = HashMap::new();
        inner.insert("password".to_owned(), OwnedValue::from(Str::from(password)));
        if let Some(id) = identity {
            inner.insert("identity".to_owned(), OwnedValue::from(Str::from(id)));
        }
        let mut outer = HashMap::new();
        outer.insert("802-1x".to_owned(), inner);
        self.send_reply(SecretReply::Secrets(outer))
    }

    /// Respond with VPN secrets.
    ///
    /// The keys depend on the VPN plugin (e.g. `"password"` for OpenVPN,
    /// `"Xauth password"` for vpnc). Consult the VPN plugin's documentation
    /// for the expected keys.
    ///
    /// # Errors
    ///
    /// Returns an error if the reply channel is already closed.
    pub async fn vpn_secrets(mut self, secrets: HashMap<String, String>) -> crate::Result<()> {
        let mut inner = HashMap::new();
        inner.insert("secrets".to_owned(), OwnedValue::from(secrets));
        let mut outer = HashMap::new();
        outer.insert("vpn".to_owned(), inner);
        self.send_reply(SecretReply::Secrets(outer))
    }

    /// Respond with a raw setting sub-dictionary.
    ///
    /// This is an escape hatch for setting types not covered by the
    /// convenience methods. The `setting_name` must match the setting
    /// NetworkManager requested (e.g. `"802-11-wireless-security"`).
    ///
    /// # Errors
    ///
    /// Returns an error if the reply channel is already closed.
    pub async fn raw(
        mut self,
        setting_name: impl Into<String>,
        data: HashMap<String, OwnedValue>,
    ) -> crate::Result<()> {
        let mut outer = HashMap::new();
        outer.insert(setting_name.into(), data);
        self.send_reply(SecretReply::Secrets(outer))
    }

    /// Tell NetworkManager the user canceled the secret request.
    ///
    /// This raises `org.freedesktop.NetworkManager.SecretAgent.UserCanceled`
    /// on the D-Bus side, which typically aborts the connection attempt.
    ///
    /// # Errors
    ///
    /// Returns an error if the reply channel is already closed.
    pub async fn cancel(mut self) -> crate::Result<()> {
        self.send_reply(SecretReply::UserCanceled)
    }

    /// Tell NetworkManager no secrets are available.
    ///
    /// Unlike [`cancel`](Self::cancel), this signals that the agent simply
    /// doesn't have the requested secrets. NetworkManager will not retry
    /// after receiving this.
    ///
    /// # Errors
    ///
    /// Returns an error if the reply channel is already closed.
    pub async fn no_secrets(mut self) -> crate::Result<()> {
        self.send_reply(SecretReply::NoSecrets)
    }

    fn send_reply(&mut self, reply: SecretReply) -> crate::Result<()> {
        let tx = self
            .reply_tx
            .take()
            .ok_or(ConnectionError::AgentNotRegistered)?;
        tx.send(reply)
            .map_err(|_| ConnectionError::AgentNotRegistered)
    }
}

impl Drop for SecretResponder {
    fn drop(&mut self) {
        if let Some(tx) = self.reply_tx.take() {
            warn!("SecretResponder dropped without responding; auto-replying NoSecrets");
            let _ = tx.send(SecretReply::NoSecrets);
        }
    }
}

/// Extracts a string value from a nested connection settings dictionary.
pub(crate) fn extract_setting_string(
    connection: &ConnectionDict,
    section: &str,
    key: &str,
) -> Option<String> {
    let section_dict = connection.get(section)?;
    let value = section_dict.get(key)?;
    <&str>::try_from(value).ok().map(String::from)
}

/// Extracts the SSID from the wireless setting. The SSID is stored as a byte
/// array (`ay`) in NetworkManager's connection dict.
pub(crate) fn extract_ssid(connection: &ConnectionDict) -> Option<String> {
    let wireless = connection.get("802-11-wireless")?;
    let ssid_value = wireless.get("ssid")?;
    // SSID is stored as `ay` (byte array) by NetworkManager
    if let Ok(bytes) = <Vec<u8>>::try_from(ssid_value.clone()) {
        return Some(String::from_utf8_lossy(&bytes).into_owned());
    }
    <&str>::try_from(ssid_value).ok().map(String::from)
}

/// Parses the raw `GetSecrets` arguments into a [`SecretSetting`].
pub(crate) fn parse_secret_setting(
    connection: &ConnectionDict,
    setting_name: &str,
) -> SecretSetting {
    match setting_name {
        "802-11-wireless-security" => SecretSetting::WifiPsk {
            ssid: extract_ssid(connection).unwrap_or_default(),
        },
        "802-1x" => SecretSetting::WifiEap {
            identity: extract_setting_string(connection, "802-1x", "identity"),
            method: extract_setting_string(connection, "802-1x", "eap"),
        },
        "vpn" => SecretSetting::Vpn {
            service_type: extract_setting_string(connection, "vpn", "service-type")
                .unwrap_or_default(),
            user_name: extract_setting_string(connection, "vpn", "user-name"),
        },
        "gsm" => SecretSetting::Gsm,
        "cdma" => SecretSetting::Cdma,
        "pppoe" => SecretSetting::Pppoe,
        other => SecretSetting::Other(other.to_owned()),
    }
}

/// Extracts the secrets already present in the `GetSecrets` payload.
///
/// Only `vpn` connections are handled, mirroring
/// [`SecretResponder::vpn_secrets`] by reading the map nested under
/// `vpn.secrets`. Other settings return an empty map.
pub(crate) fn extract_existing_secrets(
    connection: &ConnectionDict,
    setting_name: &str,
) -> HashMap<String, String> {
    let Some(section) = connection.get(setting_name) else {
        return HashMap::new();
    };

    if setting_name == "vpn" {
        return section
            .get("secrets")
            .and_then(|v| <HashMap<String, String>>::try_from(v.clone()).ok())
            .unwrap_or_default();
    }

    // Other connection types would require reading their individual secret
    // keys ("psk", "wep-key0", "password", etc.); not currently handled.

    HashMap::new()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn responder() -> (
        SecretResponder,
        futures::channel::oneshot::Receiver<SecretReply>,
    ) {
        let (tx, rx) = futures::channel::oneshot::channel();
        (SecretResponder::new(tx, "requested-setting".into()), rx)
    }

    fn string_value<'a>(dict: &'a HashMap<String, OwnedValue>, key: &str) -> &'a str {
        <&str>::try_from(dict.get(key).expect("missing secret value"))
            .expect("secret value should be a string")
    }

    #[test]
    fn flags_from_bits() {
        let flags = SecretAgentFlags::from_bits_truncate(0x5);
        assert!(flags.contains(SecretAgentFlags::ALLOW_INTERACTION));
        assert!(flags.contains(SecretAgentFlags::USER_REQUESTED));
        assert!(!flags.contains(SecretAgentFlags::REQUEST_NEW));
    }

    #[test]
    fn capabilities_bits_round_trip() {
        let caps = SecretAgentCapabilities::VPN_HINTS;
        assert_eq!(caps.bits(), 0x1);
    }

    #[test]
    fn parse_wifi_psk_setting() {
        let mut wireless = HashMap::new();
        wireless.insert(
            "ssid".to_owned(),
            OwnedValue::try_from(zvariant::Array::from(vec![b'n', b'm', b'r', b's']))
                .expect("owned byte array"),
        );
        let mut connection = HashMap::new();
        connection.insert("802-11-wireless".to_owned(), wireless);
        let setting = parse_secret_setting(&connection, "802-11-wireless-security");

        match setting {
            SecretSetting::WifiPsk { ssid } => assert_eq!(ssid, "nmrs"),
            other => panic!("expected Wi-Fi PSK setting, got {other:?}"),
        }
    }

    #[test]
    fn parse_eap_setting_preserves_context() {
        let mut eap = HashMap::new();
        eap.insert(
            "identity".to_owned(),
            OwnedValue::from(Str::from("alice@example.com")),
        );
        eap.insert("eap".to_owned(), OwnedValue::from(Str::from("peap")));
        let mut connection = HashMap::new();
        connection.insert("802-1x".to_owned(), eap);

        match parse_secret_setting(&connection, "802-1x") {
            SecretSetting::WifiEap { identity, method } => {
                assert_eq!(identity.as_deref(), Some("alice@example.com"));
                assert_eq!(method.as_deref(), Some("peap"));
            }
            other => panic!("expected Wi-Fi EAP setting, got {other:?}"),
        }
    }

    #[test]
    fn parse_vpn_setting_preserves_context() {
        let mut vpn = HashMap::new();
        vpn.insert(
            "service-type".to_owned(),
            OwnedValue::from(Str::from("org.freedesktop.NetworkManager.openvpn")),
        );
        vpn.insert("user-name".to_owned(), OwnedValue::from(Str::from("alice")));
        let mut connection = HashMap::new();
        connection.insert("vpn".to_owned(), vpn);

        match parse_secret_setting(&connection, "vpn") {
            SecretSetting::Vpn {
                service_type,
                user_name,
            } => {
                assert_eq!(service_type, "org.freedesktop.NetworkManager.openvpn");
                assert_eq!(user_name.as_deref(), Some("alice"));
            }
            other => panic!("expected VPN setting, got {other:?}"),
        }
    }

    #[test]
    fn parse_unknown_setting() {
        let connection = HashMap::new();
        let setting = parse_secret_setting(&connection, "some-custom-thing");
        assert!(matches!(setting, SecretSetting::Other(s) if s == "some-custom-thing"));
    }

    #[test]
    fn parse_simple_secret_settings_maps_each_networkmanager_name() {
        let connection = ConnectionDict::new();

        assert!(matches!(
            parse_secret_setting(&connection, "gsm"),
            SecretSetting::Gsm
        ));
        assert!(matches!(
            parse_secret_setting(&connection, "cdma"),
            SecretSetting::Cdma
        ));
        assert!(matches!(
            parse_secret_setting(&connection, "pppoe"),
            SecretSetting::Pppoe
        ));
    }

    #[test]
    fn extract_ssid_accepts_string_fallback_and_lossy_byte_arrays() {
        let mut connection = ConnectionDict::new();
        connection.insert(
            "802-11-wireless".into(),
            HashMap::from([(
                "ssid".into(),
                OwnedValue::from(Str::from("legacy-string-ssid")),
            )]),
        );
        assert_eq!(
            extract_ssid(&connection).as_deref(),
            Some("legacy-string-ssid")
        );

        connection.insert(
            "802-11-wireless".into(),
            HashMap::from([(
                "ssid".into(),
                OwnedValue::try_from(zvariant::Array::from(vec![b'n', 0xff, b'm']))
                    .expect("owned byte array"),
            )]),
        );
        assert_eq!(extract_ssid(&connection).as_deref(), Some("n\u{fffd}m"));
    }

    #[test]
    fn extract_existing_secrets_reads_vpn_secrets() {
        let mut secrets = HashMap::new();
        secrets.insert("password".to_owned(), "hunter2".to_owned());
        secrets.insert("Xauth password".to_owned(), "otp".to_owned());

        let mut vpn = HashMap::new();
        vpn.insert("secrets".to_owned(), OwnedValue::from(secrets));
        let mut connection = ConnectionDict::new();
        connection.insert("vpn".to_owned(), vpn);

        let existing = extract_existing_secrets(&connection, "vpn");
        assert_eq!(
            existing.get("password").map(String::as_str),
            Some("hunter2")
        );
        assert_eq!(
            existing.get("Xauth password").map(String::as_str),
            Some("otp")
        );
    }

    #[test]
    fn extract_existing_secrets_vpn_without_secrets_is_empty() {
        // A `vpn` section present but with no nested `secrets` sub-dict.
        let mut connection = ConnectionDict::new();
        connection.insert("vpn".to_owned(), HashMap::new());
        assert!(extract_existing_secrets(&connection, "vpn").is_empty());
    }

    #[test]
    fn extract_existing_secrets_missing_section_is_empty() {
        let connection = ConnectionDict::new();
        assert!(extract_existing_secrets(&connection, "vpn").is_empty());
    }

    #[test]
    fn extract_existing_secrets_non_vpn_is_empty() {
        // Non-VPN settings are intentionally not populated (see fn docs).
        let mut inner = HashMap::new();
        inner.insert(
            "psk".to_owned(),
            OwnedValue::from(Str::from("should-be-ignored")),
        );
        let mut connection = ConnectionDict::new();
        connection.insert("802-11-wireless-security".to_owned(), inner);

        assert!(extract_existing_secrets(&connection, "802-11-wireless-security").is_empty());
    }

    #[test]
    fn responder_drop_sends_no_secrets() {
        let (tx, mut rx) = futures::channel::oneshot::channel();
        let responder = SecretResponder::new(tx, "test".into());
        drop(responder);
        let reply = rx.try_recv().expect("should have received a reply");
        assert!(reply.is_some(), "drop should have sent a reply");
        assert!(matches!(reply.unwrap(), SecretReply::NoSecrets));
    }

    #[tokio::test]
    async fn responder_wifi_psk_sends_expected_dictionary() {
        let (responder, rx) = responder();

        responder.wifi_psk("correct horse").await.unwrap();
        let SecretReply::Secrets(settings) = rx.await.expect("reply channel closed") else {
            panic!("expected secrets reply");
        };
        let security = settings
            .get("802-11-wireless-security")
            .expect("missing wireless security setting");
        assert_eq!(string_value(security, "psk"), "correct horse");
    }

    #[tokio::test]
    async fn responder_wifi_eap_includes_optional_identity() {
        let (responder, rx) = responder();

        responder
            .wifi_eap(Some("alice".into()), "secret".into())
            .await
            .unwrap();
        let SecretReply::Secrets(settings) = rx.await.expect("reply channel closed") else {
            panic!("expected secrets reply");
        };
        let eap = settings.get("802-1x").expect("missing 802.1X setting");
        assert_eq!(string_value(eap, "identity"), "alice");
        assert_eq!(string_value(eap, "password"), "secret");
    }

    #[tokio::test]
    async fn responder_wifi_eap_omits_absent_identity() {
        let (responder, rx) = responder();

        responder.wifi_eap(None, "secret".into()).await.unwrap();
        let SecretReply::Secrets(settings) = rx.await.expect("reply channel closed") else {
            panic!("expected secrets reply");
        };
        let eap = settings.get("802-1x").expect("missing 802.1X setting");
        assert!(!eap.contains_key("identity"));
        assert_eq!(string_value(eap, "password"), "secret");
    }

    #[tokio::test]
    async fn responder_vpn_secrets_nests_plugin_dictionary() {
        let (responder, rx) = responder();
        let secrets = HashMap::from([
            ("password".to_owned(), "hunter2".to_owned()),
            ("otp".to_owned(), "123456".to_owned()),
        ]);

        responder.vpn_secrets(secrets.clone()).await.unwrap();
        let SecretReply::Secrets(settings) = rx.await.expect("reply channel closed") else {
            panic!("expected secrets reply");
        };
        let nested = settings
            .get("vpn")
            .and_then(|vpn| vpn.get("secrets"))
            .cloned()
            .and_then(|value| HashMap::<String, String>::try_from(value).ok())
            .expect("missing VPN secrets dictionary");
        assert_eq!(nested, secrets);
    }

    #[tokio::test]
    async fn responder_raw_preserves_setting_and_values() {
        let (responder, rx) = responder();
        let data = HashMap::from([("pin".to_owned(), OwnedValue::from(Str::from("1234")))]);

        responder.raw("gsm", data).await.unwrap();
        let SecretReply::Secrets(settings) = rx.await.expect("reply channel closed") else {
            panic!("expected secrets reply");
        };
        assert_eq!(string_value(settings.get("gsm").unwrap(), "pin"), "1234");
    }

    #[tokio::test]
    async fn responder_cancel_sends_user_canceled() {
        let (responder, rx) = responder();
        responder.cancel().await.unwrap();
        assert!(matches!(rx.await.unwrap(), SecretReply::UserCanceled));
    }

    #[tokio::test]
    async fn responder_no_secrets_sends_no_secrets() {
        let (responder, rx) = responder();
        responder.no_secrets().await.unwrap();
        assert!(matches!(rx.await.unwrap(), SecretReply::NoSecrets));
    }

    #[tokio::test]
    async fn responder_reports_closed_reply_channel() {
        let (responder, rx) = responder();
        drop(rx);

        let result = responder.wifi_psk("secret").await;
        assert!(matches!(result, Err(ConnectionError::AgentNotRegistered)));
    }
}
