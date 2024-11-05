use std::error::Error;
use std::fmt::{Display, Formatter};

#[derive(Debug)]
pub struct ArchiveError(String);

impl Display for ArchiveError {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl From<&str> for ArchiveError {
    fn from(value: &str) -> Self {
        Self(value.to_string())
    }
}

impl Error for ArchiveError {}
