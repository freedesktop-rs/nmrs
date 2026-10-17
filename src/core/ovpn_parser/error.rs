use std::fmt;

use crate::ConnectionError;

#[derive(Debug, Clone)]
pub enum OvpnParseError {
    InvalidDirectiveSyntax {
        line: usize,
    },
    InvalidArgument {
        key: String,
        arg: String,
        line: usize,
    },
    MissingArgument {
        key: String,
        line: usize,
    },
    InvalidContinuation {
        line: usize,
    },
    UnterminatedBlock {
        block: String,
        line: usize,
    },
    UnexpectedBlockEnd {
        block: String,
        line: usize,
    },
    UnexpectedEof {
        line: usize,
    },
    InvalidNumber {
        key: String,
        value: String,
        line: usize,
    },
}

impl fmt::Display for OvpnParseError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            OvpnParseError::InvalidDirectiveSyntax { line } => {
                write!(f, "invalid directive syntax at line {line}")
            }
            OvpnParseError::InvalidArgument { key, arg, line } => {
                write!(
                    f,
                    "invalid argument '{arg}' for directive '{key}' at line {line}"
                )
            }
            OvpnParseError::MissingArgument { key, line } => {
                write!(f, "missing argument for directive '{key}' at line {line}")
            }
            OvpnParseError::InvalidContinuation { line } => {
                write!(f, "invalid continuation at line {line}")
            }
            OvpnParseError::UnterminatedBlock { block, line } => {
                write!(f, "unterminated block '{block}' starting at line {line}")
            }
            OvpnParseError::UnexpectedBlockEnd { block, line } => {
                write!(f, "unexpected end of block '{block}' at line {line}")
            }
            OvpnParseError::UnexpectedEof { line } => {
                write!(f, "unexpected EOF at line {line}")
            }
            OvpnParseError::InvalidNumber { key, value, line } => {
                write!(f, "invalid value '{value}' for '{key}' at line {line}")
            }
        }
    }
}

impl From<OvpnParseError> for ConnectionError {
    fn from(e: OvpnParseError) -> Self {
        ConnectionError::ParseError(e)
    }
}

impl std::error::Error for OvpnParseError {}
