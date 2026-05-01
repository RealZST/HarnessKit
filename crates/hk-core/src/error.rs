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
    /// Input validation failed
    Validation(String),
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
            Self::Validation(msg) => write!(f, "Validation error: {msg}"),
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

impl From<toml::de::Error> for HkError {
    fn from(e: toml::de::Error) -> Self {
        Self::ConfigCorrupted(e.to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_from_rusqlite_error() {
        let err = rusqlite::Error::SqliteFailure(rusqlite::ffi::Error::new(1), Some("test".into()));
        let hk: HkError = err.into();
        assert!(matches!(hk, HkError::Database(_)));
        assert!(hk.to_string().contains("Database error"));
    }

    #[test]
    fn test_from_io_error_permission_denied() {
        let err = std::io::Error::new(std::io::ErrorKind::PermissionDenied, "no access");
        let hk: HkError = err.into();
        assert!(matches!(hk, HkError::PermissionDenied(_)));
    }

    #[test]
    fn test_from_io_error_not_found() {
        let err = std::io::Error::new(std::io::ErrorKind::NotFound, "file missing");
        let hk: HkError = err.into();
        assert!(matches!(hk, HkError::NotFound(_)));
    }

    #[test]
    fn test_from_io_error_other() {
        let err = std::io::Error::other("something else");
        let hk: HkError = err.into();
        assert!(matches!(hk, HkError::Internal(_)));
    }

    #[test]
    fn test_from_reqwest_error() {
        // Build a reqwest error by trying to parse an invalid URL
        let err = reqwest::blocking::Client::new()
            .get("http://[invalid")
            .build()
            .unwrap_err();
        let hk: HkError = err.into();
        assert!(matches!(hk, HkError::Network(_)));
    }

    #[test]
    fn test_from_serde_json_error() {
        let err = serde_json::from_str::<serde_json::Value>("not json").unwrap_err();
        let hk: HkError = err.into();
        assert!(matches!(hk, HkError::ConfigCorrupted(_)));
    }

    #[test]
    fn test_from_toml_error() {
        let err = toml::from_str::<toml::Value>("= invalid").unwrap_err();
        let hk: HkError = err.into();
        assert!(matches!(hk, HkError::ConfigCorrupted(_)));
        assert!(hk.to_string().contains("Config error"));
    }

    #[test]
    fn test_validation_variant() {
        let hk = HkError::Validation("bad input".into());
        assert!(matches!(hk, HkError::Validation(_)));
        assert_eq!(hk.to_string(), "Validation error: bad input");
    }

    #[test]
    fn test_display_all_variants() {
        let variants = vec![
            HkError::NotFound("x".into()),
            HkError::Network("x".into()),
            HkError::PermissionDenied("x".into()),
            HkError::ConfigCorrupted("x".into()),
            HkError::Conflict("x".into()),
            HkError::PathNotAllowed("x".into()),
            HkError::Database("x".into()),
            HkError::CommandFailed("x".into()),
            HkError::Validation("x".into()),
            HkError::Internal("x".into()),
        ];
        for v in &variants {
            // All variants should produce non-empty display strings
            assert!(!v.to_string().is_empty());
        }
    }

    #[test]
    fn test_serialize_format() {
        let err = HkError::Network("timeout".into());
        let json = serde_json::to_string(&err).unwrap();
        assert!(json.contains("\"kind\":\"Network\""));
        assert!(json.contains("\"message\":\"timeout\""));
    }
}
