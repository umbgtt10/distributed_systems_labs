#[derive(Debug, Clone, PartialEq, Eq)]
pub enum StorageError {
    /// Key was not found (Get on non-existent key, or Put with version > 0 on non-existent key)
    KeyNotFound(String),

    /// Key already exists (Put with version = 0 on existing key)
    KeyAlreadyExists(String),

    /// Version mismatch (Put with wrong expected version)
    VersionMismatch { expected: u64, actual: u64 },
}

impl std::fmt::Display for StorageError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            StorageError::KeyNotFound(key) => write!(f, "Key '{}' not found", key),
            StorageError::KeyAlreadyExists(key) => write!(f, "Key '{}' already exists", key),
            StorageError::VersionMismatch { expected, actual } => {
                write!(
                    f,
                    "Version mismatch: expected {}, actual {}",
                    expected, actual
                )
            }
        }
    }
}

impl std::error::Error for StorageError {}
