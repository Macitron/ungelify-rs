use thiserror::Error;

#[derive(Error, Debug)]
pub enum ArchiveError {
    #[error("Invalid archive signature '{0}' != 'MPK\\0'")]
    BadSignature(String),
    #[error("Unknown archive error")]
    Unknown,
}
