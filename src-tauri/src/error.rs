use thiserror::Error;

#[derive(Error, Debug)]
pub enum BrowsionError {
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("Configuration error: {0}")]
    Config(String),

    #[error("TOML serialization error: {0}")]
    TomlSerialize(#[from] toml::ser::Error),

    #[error("TOML deserialization error: {0}")]
    TomlDeserialize(#[from] toml::de::Error),

    #[error("Process error: {0}")]
    Process(String),

    #[error("Window error: {0}")]
    Window(String),

    #[error("Profile not found: {0}")]
    ProfileNotFound(String),

    #[error("Validation error: {0}")]
    Validation(String),
}

pub type Result<T> = std::result::Result<T, BrowsionError>;
