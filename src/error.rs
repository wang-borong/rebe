use std::fmt;
use std::io;

pub type RebeResult<T> = Result<T, RebeError>;

#[derive(Debug)]
pub enum RebeError {
    Io(io::Error),
    InvalidArgument(String),
}

impl fmt::Display for RebeError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            RebeError::Io(err) => write!(formatter, "{err}"),
            RebeError::InvalidArgument(message) => write!(formatter, "{message}"),
        }
    }
}

impl std::error::Error for RebeError {}

impl From<io::Error> for RebeError {
    fn from(err: io::Error) -> Self {
        RebeError::Io(err)
    }
}
