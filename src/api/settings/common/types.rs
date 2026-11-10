use std::fmt::Display;

/// NetworkManager's three-state integer representation for default, no, or yes.
#[derive(Debug, Default, Clone, Copy, Hash, PartialEq, Eq)]
pub enum BoolWithDefault {
    /// Leave the setting at NetworkManager's default value (`-1`).
    #[default]
    Default,
    /// Explicitly disable the setting (`0`).
    No,
    /// Explicitly enable the setting (`1`).
    Yes,
}

impl From<BoolWithDefault> for i32 {
    fn from(value: BoolWithDefault) -> Self {
        match value {
            BoolWithDefault::Default => -1,
            BoolWithDefault::No => 0,
            BoolWithDefault::Yes => 1,
        }
    }
}

impl Display for BoolWithDefault {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let value = match self {
            Self::Default => "default",
            Self::No => "no",
            Self::Yes => "yes",
        };
        write!(f, "{value}")
    }
}

impl From<i32> for BoolWithDefault {
    fn from(value: i32) -> Self {
        match value {
            1 => Self::Yes,
            0 => Self::No,
            _ => Self::Default,
        }
    }
}
