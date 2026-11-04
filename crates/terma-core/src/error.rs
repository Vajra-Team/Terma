//! Fehler, die bis in die Oberfläche durchgereicht werden. Die Oberfläche macht daraus Sätze.

use std::fmt;
use std::path::PathBuf;

use serde::Serialize;

#[derive(Debug, Clone, PartialEq, Serialize)]
pub enum BuildFailure {
    PgpKeyUnknown,
    ChecksumMismatch,
    SourcesChanged,
    Other,
}

/// Getaggt als `{ kind, data }`, damit auch Varianten mit einem einzelnen Wert serialisierbar sind.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(tag = "kind", content = "data")]
pub enum TermaError {
    DbLocked,
    Cancelled,
    Network(String),
    AurUnavailable,
    NotAnAppImage,
    UnsupportedFile,
    Protected { package: String },
    BuildFailed { package: String, kind: BuildFailure, log: PathBuf },
    DependencyCycle(String),
    UnresolvableDependency(String),
    Command { code: i32, tail: String },
    Io(String),
}

pub type Result<T> = std::result::Result<T, TermaError>;

impl fmt::Display for TermaError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{self:?}")
    }
}

impl std::error::Error for TermaError {}

impl From<std::io::Error> for TermaError {
    fn from(e: std::io::Error) -> Self {
        TermaError::Io(e.to_string())
    }
}
