//! Internal error type for the typed settings layer.
//!
//! Every public method in this module returns [`crate::ConnectionError`]; the
//! [`From`] impl below maps the internal parse/downcast failures onto it so the
//! settings code can use `?` and still present a single error type to callers.

use crate::ConnectionError;

/// Errors produced while parsing or serializing NetworkManager settings.
///
/// Every typed settings method that touches the wire format returns this type.
/// It converts into [`crate::ConnectionError`] (via [`std::convert::From`]), so
/// callers using `?` in a `nmrs::Result` context never have to name it.
#[derive(Debug, thiserror::Error)]
pub enum SettingsError {
    /// A required section or key was absent from the settings dictionary.
    #[error("missing settings field '{0}'")]
    MissingField(&'static str),
    /// A stored value could not be downcast to the expected type. The tuple is
    /// `(field, expected_type_name)`.
    #[error("settings field {0:?} is not the expected type")]
    FailedDowncast((&'static str, &'static str)),
    /// A stored string could not be parsed into the expected Rust type. The
    /// tuple is `(reason, field)`.
    #[error("settings field '{1}' has an invalid value: {0}")]
    FromStrError(String, &'static str),
    /// A UUID string was malformed.
    #[error("invalid UUID: {0}")]
    Uuid(#[from] uuid::Error),
    /// A `zvariant` downcast / conversion error.
    #[error("variant error: {0}")]
    Variant(#[from] zvariant::Error),
    /// An invalid UTF-8 string (e.g. an SSID or secret).
    #[error("invalid UTF-8: {0}")]
    Utf8(#[from] std::str::Utf8Error),
    /// An underlying `zbus` / D-Bus error.
    #[error("D-Bus error: {0}")]
    Dbus(#[from] zbus::Error),
    /// An underlying `nmrs` error (e.g. from fetching raw settings).
    #[error(transparent)]
    Nm(#[from] ConnectionError),
}

impl From<SettingsError> for ConnectionError {
    fn from(value: SettingsError) -> Self {
        match value {
            SettingsError::MissingField(field) => ConnectionError::SettingsParse {
                field,
                reason: "missing field".into(),
            },
            SettingsError::FailedDowncast((field, expected)) => ConnectionError::SettingsParse {
                field,
                reason: format!("expected {expected}"),
            },
            SettingsError::FromStrError(reason, field) => ConnectionError::SettingsParse {
                field,
                reason: format!("invalid value: {reason}"),
            },
            SettingsError::Uuid(e) => ConnectionError::InvalidInput {
                field: "uuid".into(),
                reason: e.to_string(),
            },
            SettingsError::Variant(e) => ConnectionError::Dbus(zbus::Error::Variant(e)),
            SettingsError::Utf8(e) => ConnectionError::InvalidUtf8(e),
            SettingsError::Dbus(e) => ConnectionError::Dbus(e),
            SettingsError::Nm(e) => e,
        }
    }
}
