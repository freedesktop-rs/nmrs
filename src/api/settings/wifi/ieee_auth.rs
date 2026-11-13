use std::{collections::HashMap, path::PathBuf, str::FromStr};

use zbus::zvariant::{OwnedValue, Value};

use crate::api::settings::error::SettingsError as ConnectionError;

/// Editable IEEE 802.1X authentication settings.
#[derive(Debug, Clone)]
pub struct IeeeAuthSection {
    /// The NetworkManager `802-1x.eap` property.
    pub eap: Vec<IeeeAuthEap>,
    /// The NetworkManager `802-1x.anonymous-identity` property.
    pub anonymous_identity: Option<String>,
    /// The NetworkManager `802-1x.domain-match` property.
    pub domain_match: Option<String>,
    /// The NetworkManager `802-1x.identity` property.
    pub identity: String,
    /// The NetworkManager `802-1x.password` property.
    pub password: Option<String>,
    /// The NetworkManager `802-1x.ca-cert` property.
    pub ca_cert: Option<PathBuf>,
    /// The NetworkManager `802-1x.client-cert` property.
    pub client_cert: Option<PathBuf>,
    /// The NetworkManager `802-1x.private-key` property.
    pub private_key: Option<String>,
    /// The NetworkManager `802-1x.phase2-auth` property.
    pub phase2_auth: Option<IeeeAuthPhaseTwoAuth>,
    /// The NetworkManager `802-1x.altsubject-matches` property.
    pub altsubject_matches: Vec<String>,
    /// The NetworkManager `802-1x.auth-timeout` property.
    pub auth_timeout: i32,
    /// The NetworkManager `802-1x.ca-cert-password` property.
    pub ca_cert_password: Option<String>,
    /// The NetworkManager `802-1x.ca-cert-password-flags` property.
    pub ca_cert_password_flags: u32,
    /// The NetworkManager `802-1x.ca-path` property.
    pub ca_path: Option<String>,
    /// The NetworkManager `802-1x.client-cert-password` property.
    pub client_cert_password: Option<String>,
    /// The NetworkManager `802-1x.client-cert-password-flags` property.
    pub client_cert_password_flags: u32,
    /// The NetworkManager `802-1x.domain-suffix-match` property.
    pub domain_suffix_match: Option<String>,
    /// The NetworkManager `802-1x.optional` property.
    pub optional: bool,
    /// The NetworkManager `802-1x.pac-file` property.
    pub pac_file: Option<String>,
    /// The NetworkManager `802-1x.password-flags` property.
    pub password_flags: u32,
    /// The NetworkManager `802-1x.password-raw` property.
    pub password_raw: Option<Vec<u8>>,
    /// The NetworkManager `802-1x.password-raw-flags` property.
    pub password_raw_flags: u32,
    /// The NetworkManager `802-1x.phase1-auth-flags` property.
    pub phase1_auth_flags: u32,
    /// The NetworkManager `802-1x.phase1-fast-provisioning` property.
    pub phase1_fast_provisioning: Option<String>,
    /// The NetworkManager `802-1x.phase1-peaplabel` property.
    pub phase1_peaplabel: Option<String>,
    /// The NetworkManager `802-1x.phase1-peapver` property.
    pub phase1_peapver: Option<String>,
    /// The NetworkManager `802-1x.phase2-altsubject-matches` property.
    pub phase2_altsubject_matches: Vec<String>,
    /// The NetworkManager `802-1x.phase2-autheap` property.
    pub phase2_autheap: Option<IeeeAuthEap>,
    /// The NetworkManager `802-1x.phase2-ca-cert` property.
    pub phase2_ca_cert: Option<PathBuf>,
    /// The NetworkManager `802-1x.phase2-ca-cert-password` property.
    pub phase2_ca_cert_password: Option<String>,
    /// The NetworkManager `802-1x.phase2-ca-cert-password-flags` property.
    pub phase2_ca_cert_password_flags: u32,
    /// The NetworkManager `802-1x.phase2-ca-path` property.
    pub phase2_ca_path: Option<String>,
    /// The NetworkManager `802-1x.phase2-client-cert` property.
    pub phase2_client_cert: Option<PathBuf>,
    /// The NetworkManager `802-1x.phase2-client-cert-password` property.
    pub phase2_client_cert_password: Option<String>,
    /// The NetworkManager `802-1x.phase2-client-cert-password-flags` property.
    pub phase2_client_cert_password_flags: u32,
    /// The NetworkManager `802-1x.phase2-domain-match` property.
    pub phase2_domain_match: Option<String>,
    /// The NetworkManager `802-1x.phase2-domain-suffix-match` property.
    pub phase2_domain_suffix_match: Option<String>,
    /// The NetworkManager `802-1x.phase2-private-key` property.
    pub phase2_private_key: Option<String>,
    /// The NetworkManager `802-1x.phase2-private-key-password` property.
    pub phase2_private_key_password: Option<String>,
    /// The NetworkManager `802-1x.phase2-private-key-password-flags` property.
    pub phase2_private_key_password_flags: u32,
    /// The NetworkManager `802-1x.phase2-subject-match` property.
    pub phase2_subject_match: Option<String>,
    /// The NetworkManager `802-1x.pin` property.
    pub pin: Option<String>,
    /// The NetworkManager `802-1x.pin-flags` property.
    pub pin_flags: u32,
    /// The NetworkManager `802-1x.private-key-password` property.
    pub private_key_password: Option<String>,
    /// The NetworkManager `802-1x.private-key-password-flags` property.
    pub private_key_password_flags: u32,
    /// The NetworkManager `802-1x.subject-match` property.
    pub subject_match: Option<String>,
    /// The NetworkManager `802-1x.system-ca-certs` property.
    pub system_ca_certs: bool,
}

impl IeeeAuthSection {
    pub(crate) const fn get_name() -> &'static str {
        "802-1x"
    }
}

impl From<IeeeAuthSection> for HashMap<&str, Value<'_>> {
    fn from(value: IeeeAuthSection) -> Self {
        let mut ieee_auth = HashMap::new();
        insert_value!(
            ieee_auth,
            "eap",
            value.eap.iter().map(|x| x.to_string()).collect::<Vec<_>>()
        );
        insert_value!(ieee_auth, "identity", value.identity);
        insert_opt!(ieee_auth, "anonymous-identity", value.anonymous_identity);
        insert_opt!(ieee_auth, "domain-match", value.domain_match);
        insert_opt!(ieee_auth, "password", value.password);
        insert_opt!(
            ieee_auth,
            "ca-cert",
            value
                .ca_cert
                .map(|path| path.to_string_lossy().to_string().into_bytes())
        );
        insert_opt!(
            ieee_auth,
            "client-cert",
            value
                .client_cert
                .map(|path| path.to_string_lossy().to_string().into_bytes())
        );
        insert_opt!(
            ieee_auth,
            "private-key",
            value.private_key.map(String::into_bytes)
        );
        insert_opt!(
            ieee_auth,
            "phase2-auth",
            value.phase2_auth.map(|v| v.to_string())
        );
        if !value.altsubject_matches.is_empty() {
            insert_value!(ieee_auth, "altsubject-matches", value.altsubject_matches);
        }
        insert_value!(ieee_auth, "auth-timeout", value.auth_timeout);
        insert_opt!(ieee_auth, "ca-cert-password", value.ca_cert_password);
        insert_value!(
            ieee_auth,
            "ca-cert-password-flags",
            value.ca_cert_password_flags
        );
        insert_opt!(ieee_auth, "ca-path", value.ca_path);
        insert_opt!(
            ieee_auth,
            "client-cert-password",
            value.client_cert_password
        );
        insert_value!(
            ieee_auth,
            "client-cert-password-flags",
            value.client_cert_password_flags
        );
        insert_opt!(ieee_auth, "domain-suffix-match", value.domain_suffix_match);
        insert_value!(ieee_auth, "optional", value.optional);
        insert_opt!(ieee_auth, "pac-file", value.pac_file);
        insert_value!(ieee_auth, "password-flags", value.password_flags);
        insert_opt!(ieee_auth, "password-raw", value.password_raw);
        insert_value!(ieee_auth, "password-raw-flags", value.password_raw_flags);
        insert_value!(ieee_auth, "phase1-auth-flags", value.phase1_auth_flags);
        insert_opt!(
            ieee_auth,
            "phase1-fast-provisioning",
            value.phase1_fast_provisioning
        );
        insert_opt!(ieee_auth, "phase1-peaplabel", value.phase1_peaplabel);
        insert_opt!(ieee_auth, "phase1-peapver", value.phase1_peapver);
        if !value.phase2_altsubject_matches.is_empty() {
            insert_value!(
                ieee_auth,
                "phase2-altsubject-matches",
                value.phase2_altsubject_matches
            );
        }
        insert_opt!(
            ieee_auth,
            "phase2-autheap",
            value.phase2_autheap.map(|v| v.to_string())
        );
        insert_opt!(
            ieee_auth,
            "phase2-ca-cert",
            value
                .phase2_ca_cert
                .map(|path| path.to_string_lossy().to_string().into_bytes())
        );
        insert_opt!(
            ieee_auth,
            "phase2-ca-cert-password",
            value.phase2_ca_cert_password
        );
        insert_value!(
            ieee_auth,
            "phase2-ca-cert-password-flags",
            value.phase2_ca_cert_password_flags
        );
        insert_opt!(ieee_auth, "phase2-ca-path", value.phase2_ca_path);
        insert_opt!(
            ieee_auth,
            "phase2-client-cert",
            value
                .phase2_client_cert
                .map(|path| path.to_string_lossy().to_string().into_bytes())
        );
        insert_opt!(
            ieee_auth,
            "phase2-client-cert-password",
            value.phase2_client_cert_password
        );
        insert_value!(
            ieee_auth,
            "phase2-client-cert-password-flags",
            value.phase2_client_cert_password_flags
        );
        insert_opt!(ieee_auth, "phase2-domain-match", value.phase2_domain_match);
        insert_opt!(
            ieee_auth,
            "phase2-domain-suffix-match",
            value.phase2_domain_suffix_match
        );
        insert_opt!(
            ieee_auth,
            "phase2-private-key",
            value.phase2_private_key.map(String::into_bytes)
        );
        insert_opt!(
            ieee_auth,
            "phase2-private-key-password",
            value.phase2_private_key_password
        );
        insert_value!(
            ieee_auth,
            "phase2-private-key-password-flags",
            value.phase2_private_key_password_flags
        );
        insert_opt!(
            ieee_auth,
            "phase2-subject-match",
            value.phase2_subject_match
        );
        insert_opt!(ieee_auth, "pin", value.pin);
        insert_value!(ieee_auth, "pin-flags", value.pin_flags);
        insert_opt!(
            ieee_auth,
            "private-key-password",
            value.private_key_password
        );
        insert_value!(
            ieee_auth,
            "private-key-password-flags",
            value.private_key_password_flags
        );
        insert_opt!(ieee_auth, "subject-match", value.subject_match);
        insert_value!(ieee_auth, "system-ca-certs", value.system_ca_certs);
        ieee_auth
    }
}

impl TryFrom<&HashMap<String, OwnedValue>> for IeeeAuthSection {
    type Error = ConnectionError;
    fn try_from(value: &HashMap<String, OwnedValue>) -> Result<Self, Self::Error> {
        let mut ca_cert = None;
        if let Some(ca_cert_u8) = extract_optional_vec_field!(value, "ca-cert", u8)? {
            let path_str = str::from_utf8(&ca_cert_u8)?;
            ca_cert = Some(
                PathBuf::from_str(path_str)
                    .map_err(|_| ConnectionError::FromStrError(path_str.to_string(), "PathBuf"))?,
            );
        }

        let mut client_cert = None;
        if let Some(client_cert_u8) = extract_optional_vec_field!(value, "client-cert", u8)? {
            let path_str = str::from_utf8(&client_cert_u8)?;
            client_cert = Some(
                PathBuf::from_str(path_str)
                    .map_err(|_| ConnectionError::FromStrError(path_str.to_string(), "PathBuf"))?,
            );
        }

        let mut phase2_ca_cert = None;
        if let Some(phase2_ca_cert_u8) = extract_optional_vec_field!(value, "phase2-ca-cert", u8)? {
            let path_str = str::from_utf8(&phase2_ca_cert_u8)?;
            phase2_ca_cert = Some(
                PathBuf::from_str(path_str)
                    .map_err(|_| ConnectionError::FromStrError(path_str.to_string(), "PathBuf"))?,
            );
        }

        let mut phase2_client_cert = None;
        if let Some(phase2_client_cert_u8) =
            extract_optional_vec_field!(value, "phase2-client-cert", u8)?
        {
            let path_str = str::from_utf8(&phase2_client_cert_u8)?;
            phase2_client_cert = Some(
                PathBuf::from_str(path_str)
                    .map_err(|_| ConnectionError::FromStrError(path_str.to_string(), "PathBuf"))?,
            );
        }

        Ok(IeeeAuthSection {
            eap: extract_vec_field_or_default!(value, "eap", String)?
                .iter()
                .map(|s| IeeeAuthEap::from_str(s))
                .collect::<Result<Vec<_>, _>>()?,
            identity: extract_field!(value, "identity", String)?,
            anonymous_identity: extract_optional_field!(value, "anonymous-identity", String)?,
            domain_match: extract_optional_field!(value, "domain-match", String)?,
            password: extract_optional_field!(value, "password", String)?,
            ca_cert,
            client_cert,
            private_key: extract_optional_field!(value, "private-key", String)?,
            phase2_auth: extract_optional_field!(value, "phase2-auth", String)?
                .map(|s| IeeeAuthPhaseTwoAuth::from_str(&s))
                .transpose()?,
            altsubject_matches: extract_vec_field_or_default!(value, "altsubject-matches", String)?,
            auth_timeout: extract_field!(value, "auth-timeout", i32).unwrap_or(0),
            ca_cert_password: extract_optional_field!(value, "ca-cert-password", String)?,
            ca_cert_password_flags: extract_field!(value, "ca-cert-password-flags", u32)
                .unwrap_or(0),
            ca_path: extract_optional_field!(value, "ca-path", String)?,
            client_cert_password: extract_optional_field!(value, "client-cert-password", String)?,
            client_cert_password_flags: extract_field!(value, "client-cert-password-flags", u32)
                .unwrap_or(0),
            domain_suffix_match: extract_optional_field!(value, "domain-suffix-match", String)?,
            optional: extract_field!(value, "optional", bool).unwrap_or(false),
            pac_file: extract_optional_field!(value, "pac-file", String)?,
            password_flags: extract_field!(value, "password-flags", u32).unwrap_or(0),
            password_raw: extract_optional_vec_field!(value, "password-raw", u8)?,
            password_raw_flags: extract_field!(value, "password-raw-flags", u32).unwrap_or(0),
            phase1_auth_flags: extract_field!(value, "phase1-auth-flags", u32).unwrap_or(0),
            phase1_fast_provisioning: extract_optional_field!(
                value,
                "phase1-fast-provisioning",
                String
            )?,
            phase1_peaplabel: extract_optional_field!(value, "phase1-peaplabel", String)?,
            phase1_peapver: extract_optional_field!(value, "phase1-peapver", String)?,
            phase2_altsubject_matches: extract_vec_field_or_default!(
                value,
                "phase2-altsubject-matches",
                String
            )?,
            phase2_autheap: extract_optional_field!(value, "phase2-autheap", String)?
                .map(|s| IeeeAuthEap::from_str(&s))
                .transpose()?,
            phase2_ca_cert,
            phase2_ca_cert_password: extract_optional_field!(
                value,
                "phase2-ca-cert-password",
                String
            )?,
            phase2_ca_cert_password_flags: extract_field!(
                value,
                "phase2-ca-cert-password-flags",
                u32
            )
            .unwrap_or(0),
            phase2_ca_path: extract_optional_field!(value, "phase2-ca-path", String)?,
            phase2_client_cert,
            phase2_client_cert_password: extract_optional_field!(
                value,
                "phase2-client-cert-password",
                String
            )?,
            phase2_client_cert_password_flags: extract_field!(
                value,
                "phase2-client-cert-password-flags",
                u32
            )
            .unwrap_or(0),
            phase2_domain_match: extract_optional_field!(value, "phase2-domain-match", String)?,
            phase2_domain_suffix_match: extract_optional_field!(
                value,
                "phase2-domain-suffix-match",
                String
            )?,
            phase2_private_key: extract_optional_field!(value, "phase2-private-key", String)?,
            phase2_private_key_password: extract_optional_field!(
                value,
                "phase2-private-key-password",
                String
            )?,
            phase2_private_key_password_flags: extract_field!(
                value,
                "phase2-private-key-password-flags",
                u32
            )
            .unwrap_or(0),
            phase2_subject_match: extract_optional_field!(value, "phase2-subject-match", String)?,
            pin: extract_optional_field!(value, "pin", String)?,
            pin_flags: extract_field!(value, "pin-flags", u32).unwrap_or(0),
            private_key_password: extract_optional_field!(value, "private-key-password", String)?,
            private_key_password_flags: extract_field!(value, "private-key-password-flags", u32)
                .unwrap_or(0),
            subject_match: extract_optional_field!(value, "subject-match", String)?,
            system_ca_certs: extract_field!(value, "system-ca-certs", bool).unwrap_or(false),
        })
    }
}

string_enum!(IeeeAuthEap {
    Peap => "peap",
    Ttls => "ttls",
    Tls => "tls",
    Leap => "leap",
    Md5 => "md5",
    Pwd => "pwd",
    Fast => "fast",
});

string_enum!(IeeeAuthPhaseTwoAuth {
    Pap => "pap",
    Chap => "chap",
    Mschap => "mschap",
    MschapV2 => "mschapv2",
});
