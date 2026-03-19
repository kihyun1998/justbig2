use std::fmt;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Jbig2Error {
    InvalidData(String),
    UnsupportedFeature(String),
    InternalError(String),
}

impl fmt::Display for Jbig2Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Jbig2Error::InvalidData(msg) => write!(f, "invalid data: {msg}"),
            Jbig2Error::UnsupportedFeature(msg) => write!(f, "unsupported feature: {msg}"),
            Jbig2Error::InternalError(msg) => write!(f, "internal error: {msg}"),
        }
    }
}

impl std::error::Error for Jbig2Error {}

pub type Result<T> = std::result::Result<T, Jbig2Error>;
