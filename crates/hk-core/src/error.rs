use serde::Serialize;
use std::fmt;

#[derive(Debug, Clone, Serialize)]
#[serde(tag = "kind", content = "message")]
pub enum HkError {
    /// Extension, agent, project, or config entry not found
    NotFound(String),
    /// Network request failed (timeout, DNS, connection refused)
    Network(String),
    /// File or directory permission denied
    PermissionDenied(String),
    /// Config file is malformed or corrupted
    ConfigCorrupted(String),
    /// Operation would conflict with existing state
    Conflict(String),
    /// Path is outside allowed directories (security)
    PathNotAllowed(String),
    /// Database operation failed
    Database(String),
    /// External tool/command failed
    CommandFailed(String),
    /// Catch-all for unexpected errors
    Internal(String),
}

impl fmt::Display for HkError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NotFound(msg) => write!(f, "Not found: {msg}"),
            Self::Network(msg) => write!(f, "Network error: {msg}"),
            Self::PermissionDenied(msg) => write!(f, "Permission denied: {msg}"),
            Self::ConfigCorrupted(msg) => write!(f, "Config error: {msg}"),
            Self::Conflict(msg) => write!(f, "Conflict: {msg}"),
            Self::PathNotAllowed(msg) => write!(f, "Path not allowed: {msg}"),
            Self::Database(msg) => write!(f, "Database error: {msg}"),
            Self::CommandFailed(msg) => write!(f, "Command failed: {msg}"),
            Self::Internal(msg) => write!(f, "Internal error: {msg}"),
        }
    }
}

impl std::error::Error for HkError {}

impl From<rusqlite::Error> for HkError {
    fn from(e: rusqlite::Error) -> Self {
        Self::Database(e.to_string())
    }
}

impl From<std::io::Error> for HkError {
    fn from(e: std::io::Error) -> Self {
        match e.kind() {
            std::io::ErrorKind::PermissionDenied => Self::PermissionDenied(e.to_string()),
            std::io::ErrorKind::NotFound => Self::NotFound(e.to_string()),
            _ => Self::Internal(e.to_string()),
        }
    }
}

impl From<reqwest::Error> for HkError {
    fn from(e: reqwest::Error) -> Self {
        Self::Network(e.to_string())
    }
}

impl From<serde_json::Error> for HkError {
    fn from(e: serde_json::Error) -> Self {
        Self::ConfigCorrupted(e.to_string())
    }
}
