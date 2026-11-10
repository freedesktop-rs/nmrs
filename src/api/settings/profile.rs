use std::collections::HashMap;

use zbus::zvariant::{OwnedValue, Value};

use super::common::{ConnectionSection, IpV4Section, IpV6Section, ProxySection};
use crate::api::settings::error::SettingsError as ConnectionError;

#[derive(Debug, Clone)]
pub(crate) struct CommonProfileParts {
    pub connection: ConnectionSection,
    pub proxy: Option<ProxySection>,
    pub ipv4: IpV4Section,
    pub ipv6: IpV6Section,
}

pub(crate) fn parse_common_profile(
    value: &HashMap<String, HashMap<String, OwnedValue>>,
) -> Result<CommonProfileParts, ConnectionError> {
    Ok(CommonProfileParts {
        connection: ConnectionSection::try_from(
            value
                .get("connection")
                .ok_or(ConnectionError::MissingField("connection"))?,
        )?,
        proxy: value.get("proxy").map(ProxySection::try_from).transpose()?,
        ipv4: IpV4Section::try_from(
            value
                .get("ipv4")
                .ok_or(ConnectionError::MissingField("ipv4"))?,
        )?,
        ipv6: IpV6Section::try_from(
            value
                .get("ipv6")
                .ok_or(ConnectionError::MissingField("ipv6"))?,
        )?,
    })
}

pub(crate) fn serialize_common_profile(
    parts: &CommonProfileParts,
    output: &mut HashMap<String, HashMap<String, OwnedValue>>,
) -> Result<(), ConnectionError> {
    parts.ipv4.validate()?;
    parts.ipv6.validate()?;
    output.insert(
        "connection".into(),
        section_to_owned(parts.connection.clone())?,
    );
    output.insert("ipv4".into(), section_to_owned(parts.ipv4.clone())?);
    output.insert("ipv6".into(), section_to_owned(parts.ipv6.clone())?);
    if let Some(proxy) = &parts.proxy {
        output.insert("proxy".into(), section_to_owned(proxy.clone())?);
    }
    Ok(())
}

pub(crate) fn section_to_owned<T>(
    section: T,
) -> Result<HashMap<String, OwnedValue>, ConnectionError>
where
    T: Into<HashMap<&'static str, Value<'static>>>,
{
    section
        .into()
        .into_iter()
        .map(|(key, value)| Ok((key.to_string(), value.try_into_owned()?)))
        .collect()
}

pub(crate) fn sections_to_owned(
    sections: HashMap<&str, HashMap<&str, Value<'_>>>,
) -> Result<HashMap<String, HashMap<String, OwnedValue>>, ConnectionError> {
    sections
        .into_iter()
        .map(|(section, fields)| {
            let fields = fields
                .into_iter()
                .map(|(key, value)| Ok((key.to_string(), value.try_into_owned()?)))
                .collect::<Result<_, ConnectionError>>()?;
            Ok((section.to_string(), fields))
        })
        .collect()
}

macro_rules! define_typed_profile {
    ($name:ident, $section_type:ty, $field:ident, $section_name:literal) => {
        #[derive(Debug, Clone)]
        #[doc = concat!("Editable typed settings for the NetworkManager `", $section_name, "` profile.")]
        pub struct $name {
            /// Shared connection metadata.
            pub connection: $crate::api::settings::common::ConnectionSection,
            #[doc = concat!("The `", $section_name, "` section.")]
            pub $field: $section_type,
            /// Optional proxy settings.
            pub proxy: Option<$crate::api::settings::common::ProxySection>,
            /// IPv4 settings.
            pub ipv4: $crate::api::settings::common::IpV4Section,
            /// IPv6 settings.
            pub ipv6: $crate::api::settings::common::IpV6Section,
        }

        impl $name {
            /// Parse the raw NetworkManager settings dictionary into this profile type.
            ///
            /// # Errors
            /// Returns an error when a required section or value is absent or invalid.
            pub fn from_raw_dict(
                value: std::collections::HashMap<
                    String,
                    std::collections::HashMap<String, ::zbus::zvariant::OwnedValue>,
                >,
            ) -> Result<Self, crate::api::settings::error::SettingsError> {
                let common = $crate::api::settings::profile::parse_common_profile(&value)?;
                let $field = <$section_type>::try_from(value.get($section_name).ok_or(
                    crate::api::settings::error::SettingsError::MissingField($section_name),
                )?)?;
                Ok(Self {
                    connection: common.connection,
                    $field,
                    proxy: common.proxy,
                    ipv4: common.ipv4,
                    ipv6: common.ipv6,
                })
            }

            /// Serialize this profile into NetworkManager's owned settings dictionary.
            ///
            /// # Errors
            /// Returns an error if an IP section is invalid or a value cannot be serialized.
            pub fn to_owned_dict(
                &self,
            ) -> Result<
                std::collections::HashMap<
                    String,
                    std::collections::HashMap<String, ::zbus::zvariant::OwnedValue>,
                >,
                crate::api::settings::error::SettingsError,
            > {
                let mut result = std::collections::HashMap::new();
                $crate::api::settings::profile::serialize_common_profile(
                    &$crate::api::settings::profile::CommonProfileParts {
                        connection: self.connection.clone(),
                        proxy: self.proxy.clone(),
                        ipv4: self.ipv4.clone(),
                        ipv6: self.ipv6.clone(),
                    },
                    &mut result,
                )?;
                result.insert(
                    $section_name.into(),
                    $crate::api::settings::profile::section_to_owned(self.$field.clone())?,
                );
                Ok(result)
            }

            /// Build a merge patch containing this profile's settings.
            ///
            /// # Errors
            /// Returns an error if the profile cannot be serialized.
            pub fn to_patch(&self) -> Result<crate::SettingsPatch, crate::ConnectionError> {
                Ok(crate::SettingsPatch {
                    raw_overlay: Some(self.to_owned_dict()?),
                    ..Default::default()
                })
            }
        }
    };
}
