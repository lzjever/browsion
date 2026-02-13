use crate::config::schema::AppConfig;
use crate::error::{BrowsionError, Result};
use std::fs;
use std::path::PathBuf;

/// Get the configuration file path based on platform
pub fn get_config_path() -> PathBuf {
    let config_dir = if cfg!(target_os = "linux") {
        dirs::config_dir()
            .map(|p| p.join("browsion"))
            .unwrap_or_else(|| PathBuf::from("."))
    } else if cfg!(target_os = "macos") {
        dirs::data_dir()
            .map(|p| p.join("com.browsion.app"))
            .unwrap_or_else(|| PathBuf::from("."))
    } else if cfg!(target_os = "windows") {
        dirs::config_dir()
            .map(|p| p.join("browsion"))
            .unwrap_or_else(|| PathBuf::from("."))
    } else {
        PathBuf::from(".")
    };

    config_dir.join("config.toml")
}

/// Load configuration from file, creating default if not exists
pub fn load_config() -> Result<AppConfig> {
    let config_path = get_config_path();

    if !config_path.exists() {
        tracing::info!(
            "Config file not found at {:?}, creating default",
            config_path
        );
        return init_config();
    }

    let content = fs::read_to_string(&config_path).map_err(|e| {
        BrowsionError::Config(format!(
            "Failed to read config from {:?}: {}",
            config_path, e
        ))
    })?;

    let config: AppConfig = toml::from_str(&content)?;

    tracing::info!("Loaded config from {:?}", config_path);
    Ok(config)
}

/// Save configuration to file
pub fn save_config(config: &AppConfig) -> Result<()> {
    let config_path = get_config_path();

    // Ensure parent directory exists
    if let Some(parent) = config_path.parent() {
        fs::create_dir_all(parent).map_err(|e| {
            BrowsionError::Config(format!(
                "Failed to create config directory {:?}: {}",
                parent, e
            ))
        })?;
    }

    let content = toml::to_string_pretty(config)?;

    fs::write(&config_path, content).map_err(|e| {
        BrowsionError::Config(format!("Failed to write config to {:?}: {}", config_path, e))
    })?;

    tracing::info!("Saved config to {:?}", config_path);
    Ok(())
}

/// Initialize default configuration and save to file
pub fn init_config() -> Result<AppConfig> {
    let config = AppConfig::default();
    save_config(&config)?;
    Ok(config)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_config_path() {
        let path = get_config_path();
        assert!(path.ends_with("config.toml"));
    }

    #[test]
    fn test_default_config() {
        let config = AppConfig::default();
        assert!(!config.chrome_path.as_os_str().is_empty());
        assert_eq!(config.profiles.len(), 0);
        assert!(config.settings.minimize_to_tray);
    }
}
