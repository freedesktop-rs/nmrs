macro_rules! extract_field {
    ($map:expr, $key:expr, $typ:ty) => {
        extract_optional_field!($map, $key, $typ)?.ok_or(
            crate::api::settings::error::SettingsError::MissingField($key),
        )
    };
}

macro_rules! extract_vec_field {
    ($map:expr, $key:expr, $typ:ty) => {
        extract_field!($map, $key, ::zbus::zvariant::Array)?
            .iter()
            .map(|v| {
                v.downcast_ref::<$typ>().map_err(|_| {
                    crate::api::settings::error::SettingsError::FailedDowncast((
                        $key,
                        stringify!($typ),
                    ))
                })
            })
            .collect::<Result<Vec<$typ>, _>>()
    };
}

macro_rules! extract_vec_field_or_default {
    ($map:expr, $key:expr, $typ:ty) => {
        match $map.get($key) {
            Some(v) => match v.downcast_ref::<::zbus::zvariant::Array>() {
                Ok(a) => a
                    .iter()
                    .map(|v| {
                        v.downcast_ref::<$typ>().map_err(|_| {
                            crate::api::settings::error::SettingsError::FailedDowncast((
                                $key,
                                stringify!($typ),
                            ))
                        })
                    })
                    .collect::<Result<Vec<$typ>, _>>(),
                Err(_) => Err(crate::api::settings::error::SettingsError::FailedDowncast(
                    ($key, stringify!($typ)),
                )),
            },
            None => Ok(Vec::new()),
        }
    };
}

macro_rules! extract_optional_vec_field {
    ($map:expr, $key:expr, $typ:ty) => {
        match extract_optional_field!($map, $key, ::zbus::zvariant::Array)? {
            Some(a) => a
                .iter()
                .map(|v| match v.downcast_ref::<$typ>() {
                    Ok(v) => Ok(Some(v)),
                    Err(_) => Err(crate::api::settings::error::SettingsError::FailedDowncast(
                        ($key, stringify!($typ)),
                    )),
                })
                .collect::<Result<Option<Vec<$typ>>, _>>(),
            None => Ok(None),
        }
    };
}

macro_rules! extract_optional_field {
    ($map:expr, $key:expr, $typ:ty) => {
        match $map.get($key) {
            Some(v) => match v.downcast_ref::<$typ>() {
                Ok(v) => Ok(Some(v)),
                Err(_) => Err(crate::api::settings::error::SettingsError::FailedDowncast(
                    ($key, stringify!($typ)),
                )),
            },
            None => Ok(None),
        }
    };
}

macro_rules! extract_map {
    ($map:expr, $key:expr) => {
        $map.get($key)
            .ok_or(crate::api::settings::error::SettingsError::MissingField(
                $key,
            ))
    };
}

/// Insert `$field` into `$map` under `$key` as a zvariant [`Value`], but only
/// when the field is `Some`. Used by the `From<…> for HashMap<…, Value>`
/// serialization impls so an absent optional field is simply omitted.
///
/// This replaces the module-local `s!` / `insert_opt!` macros that did the same
/// thing in `mobile.rs`, `common/ipv4.rs`, and elsewhere.
macro_rules! insert_opt {
    ($map:expr, $key:expr, $field:expr) => {
        if let Some(value) = $field {
            $map.insert($key, ::zbus::zvariant::Value::from(value));
        }
    };
}

/// Insert `$value` into `$map` under `$key` as a zvariant [`Value`]
/// unconditionally. Counterpart of [`insert_opt`] for fields that are always
/// present. Replaces the module-local `put!` macro used in `bridge.rs`.
macro_rules! insert_value {
    ($map:expr, $key:expr, $value:expr) => {
        $map.insert($key, ::zbus::zvariant::Value::from($value));
    };
}

/// Define a field-less enum whose variants map to integer values of `$type`,
/// with a `Default` variant and `From` conversions in both directions.
///
/// Two forms are accepted:
/// - `int_enum!(Name : u32 { A = 1, B = 2, } default A)`
/// - `int_enum!(Name { A = 1, B = 2, } default A)` (shorthand for `: i32`)
///
/// Replaces the duplicate definitions that lived in `common/connection.rs`
/// and `wifi/wireless_security.rs`.
macro_rules! int_enum {
    ($(#[$meta:meta])* $name:ident { $($variant:ident = $value:literal),+ $(,)? } default $default:ident) => {
        int_enum!($(#[$meta])* $name : i32 { $($variant = $value),+ } default $default);
    };
    ($(#[$meta:meta])* $name:ident : $type:ty { $($variant:ident = $value:literal),+ $(,)? } default $default:ident) => {
        $(#[$meta])*
        #[derive(Debug, Copy, Clone, PartialEq, Eq)]
        #[doc = concat!("Values for the NetworkManager `", stringify!($name), "` setting.")]
        pub enum $name {
            $(
                #[doc = concat!("NetworkManager integer value `", stringify!($value), "`.")]
                $variant
            ),+
        }

        impl Default for $name {
            fn default() -> Self {
                Self::$default
            }
        }

        impl From<$name> for $type {
            fn from(value: $name) -> Self {
                match value {
                    $($name::$variant => $value),+
                }
            }
        }

        impl From<$type> for $name {
            fn from(value: $type) -> Self {
                match value {
                    $($value => Self::$variant),+,
                    _ => Self::$default,
                }
            }
        }
    };
}

/// Define an enum with strict string representations, `Display`, and `FromStr`
/// conversions that report invalid input as `SettingsError::FromStrError`.
///
/// Optional derive attributes are accepted immediately before the enum name;
/// an optional `default Variant` clause implements `Default` without requiring
/// a `#[default]` variant attribute.
macro_rules! string_enum {
    ($(#[$meta:meta])* $name:ident { $($variant:ident => $value:literal),+ $(,)? } error $error_name:literal $(default $default:ident)?) => {
        string_enum!(@impl $name, $error_name; $(#[$meta])*; { $($variant => $value),+ }; $(default $default)?);
    };
    ($(#[$meta:meta])* $name:ident { $($variant:ident => $value:literal),+ $(,)? } $(default $default:ident)?) => {
        string_enum!(@impl $name, stringify!($name); $(#[$meta])*; { $($variant => $value),+ }; $(default $default)?);
    };
    (@impl $name:ident, $error_name:expr; $(#[$meta:meta])*; { $($variant:ident => $value:literal),+ $(,)? }; $(default $default:ident)?) => {
        $(#[$meta])*
        #[derive(Debug, Copy, Clone, PartialEq, Eq)]
        #[doc = concat!("Values for the NetworkManager `", stringify!($name), "` setting.")]
        pub enum $name {
            $(
                #[doc = concat!("NetworkManager value `", $value, "`.")]
                $variant
            ),+
        }

        $(
            impl Default for $name {
                fn default() -> Self {
                    Self::$default
                }
            }
        )?

        impl ::std::fmt::Display for $name {
            fn fmt(&self, f: &mut ::std::fmt::Formatter<'_>) -> ::std::fmt::Result {
                let value = match self {
                    $($name::$variant => $value),+
                };
                f.write_str(value)
            }
        }

        impl ::std::str::FromStr for $name {
            type Err = crate::api::settings::error::SettingsError;

            fn from_str(value: &str) -> Result<Self, Self::Err> {
                match value {
                    $($value => Ok(Self::$variant)),+,
                    _ => Err(crate::api::settings::error::SettingsError::FromStrError(
                        value.to_string(),
                        $error_name,
                    )),
                }
            }
        }
    };
}
