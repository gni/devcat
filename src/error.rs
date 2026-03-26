use thiserror::Error;

pub type Result<T, E = Error> = std::result::Result<T, E>;

#[derive(Debug, Error)]
pub enum Error {
    #[error("I/O Error: {0}")]
    Io(#[from] std::io::Error),
    #[error("File walk error: {0}")]
    Walk(#[from] ignore::Error),
    #[error("Regex Error: {0}")]
    Regex(#[from] regex::Error),
    #[error("JSON Error: {0}")]
    Json(#[from] serde_json::Error),
    #[error("Invalid glob pattern: {0}")]
    Glob(#[from] globset::Error),
    #[error("TOML parsing error: {0}")]
    Toml(#[from] toml::de::Error),
    #[error("File watcher error: {0}")]
    Notify(#[from] notify::Error),
    #[error("Snapshot ID `{0}` not found. Run `devcat log` to see available snapshots.")]
    SnapshotIdNotFound(u32),
    #[error("No snapshots found. Run `devcat save <message>` to create one.")]
    NoSnapshots,
    #[error("Could not find object with hash `{0}` in the object store.")]
    ObjectNotFound(String),
    #[error("Failed to format output string: {0}")]
    Format(#[from] std::fmt::Error),
    #[error("{0}")]
    Custom(String),
}
