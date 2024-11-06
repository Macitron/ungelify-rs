use std::error::Error;
use std::fmt::{Display, Formatter};

#[derive(Debug)]
pub struct ArchiveError(String);

impl Display for ArchiveError {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl<S: AsRef<str>> From<S> for ArchiveError {
    fn from(value: S) -> Self {
        Self(value.as_ref().to_string())
    }
}

impl Error for ArchiveError {}
