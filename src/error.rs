use std::error::Error as StdError;
use std::fmt::{Display, Formatter};

pub type Result<T> = std::result::Result<T, RingmasterError>;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RingmasterError {
    Usage(String),
    Config(String),
}

impl Display for RingmasterError {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Usage(message) => write!(f, "usage error: {message}"),
            Self::Config(message) => write!(f, "config error: {message}"),
        }
    }
}

impl StdError for RingmasterError {}
