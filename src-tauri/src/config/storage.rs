use crate::config::schema::{AppConfig, BrowserSource};
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

/// Read backup file if it exists; return None if not present or unreadable.
fn backup_content_from_path(backup_path: &PathBuf) -> Option<String> {
    if backup_path.exists() {
        fs::read_to_string(backup_path).ok()
    } else {
        None
    }
}

/// Parse TOML config; on failure try backup content if provided.
pub(super) fn parse_config_with_fallback(
    content: &str,
    backup_content: Option<String>,
) -> Result<AppConfig> {
    match toml::from_str(content) {
        Ok(c) => Ok(c),
        Err(e) => {
            tracing::error!("Failed to parse config: {}. Trying backup.", e);
            match backup_content {
                Some(bak) => toml::from_str(&bak).map_err(|e2| {
                    BrowsionError::Config(format!(
                        "Both config and backup failed to parse.\nConfig: {}\nBackup: {}",
                        e, e2
                    ))
                }),
                None => Err(e.into()),
            }
        }
    }
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

    let mut config: AppConfig = parse_config_with_fallback(&content, backup_content_from_path(
        &config_path.with_extension("toml.bak"),
    ))?;

    let mut needs_save = false;

    // One-time migration: old config had chrome_path at top level
    if let Some(legacy_path) = config.chrome_path.take() {
        config.browser_source = BrowserSource::Custom {
            path: legacy_path,
            fingerprint_chromium: false,
        };
        needs_save = true;
        tracing::info!("Migrated legacy chrome_path to browser_source");
    }

    // One-time migration: old config had bare `api_port` at top level
    if let Some(old_port) = config.api_port.take() {
        config.mcp.api_port = old_port;
        config.mcp.enabled = old_port > 0;
        needs_save = true;
        tracing::info!("Migrated legacy api_port ({}) to mcp section", old_port);
    }

    if needs_save {
        let _ = save_config(&config);
    }

    tracing::info!("Loaded config from {:?}", config_path);
    Ok(config)
}

/// Save configuration to file.
/// Creates a `.bak` backup of the existing config before overwriting.
pub fn save_config(config: &AppConfig) -> Result<()> {
    let config_path = get_config_path();

    if let Some(parent) = config_path.parent() {
        fs::create_dir_all(parent).map_err(|e| {
            BrowsionError::Config(format!(
                "Failed to create config directory {:?}: {}",
                parent, e
            ))
        })?;
    }

    // Backup existing config before overwriting
    if config_path.exists() {
        let backup_path = config_path.with_extension("toml.bak");
        if let Err(e) = fs::copy(&config_path, &backup_path) {
            tracing::warn!("Failed to create config backup: {}", e);
        }
    }

    let content = toml::to_string_pretty(config)?;

    fs::write(&config_path, content).map_err(|e| {
        BrowsionError::Config(format!(
            "Failed to write config to {:?}: {}",
            config_path, e
        ))
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
        assert!(config.chrome_path.is_none());
        assert_eq!(config.profiles.len(), 0);
        assert!(config.settings.minimize_to_tray);
        assert!(config.mcp.enabled);
        assert_eq!(config.mcp.api_port, 38472);
        assert!(config.mcp.api_key.is_none());
    }

    #[test]
    fn test_parse_config_with_fallback_valid_content() {
        let toml = r#"
            recent_profiles = []
            [mcp]
            enabled = true
            api_port = 38472
            [[profiles]]
            id = "p1"
            name = "P1"
            description = ""
            user_data_dir = "/tmp/p1"
            lang = "en-US"
            custom_args = []
            tags = []
        "#;
        let config = parse_config_with_fallback(toml, None).unwrap();
        assert_eq!(config.mcp.api_port, 38472);
        assert_eq!(config.profiles.len(), 1);
        assert_eq!(config.profiles[0].id, "p1");
    }

    #[test]
    fn test_parse_config_with_fallback_invalid_content_no_backup() {
        let toml = "invalid toml ??? [[";
        let res = parse_config_with_fallback(toml, None);
        assert!(res.is_err());
    }

    #[test]
    fn test_parse_config_with_fallback_invalid_content_with_valid_backup() {
        let invalid = "not valid toml at all";
        let backup = r#"
            recent_profiles = []
            [mcp]
            enabled = false
            api_port = 9999
        "#;
        let config = parse_config_with_fallback(invalid, Some(backup.to_string())).unwrap();
        assert!(!config.mcp.enabled);
        assert_eq!(config.mcp.api_port, 9999);
    }

    #[test]
    fn test_parse_config_with_fallback_invalid_content_and_invalid_backup() {
        let invalid = "invalid";
        let backup = "also invalid {{{";
        let res = parse_config_with_fallback(invalid, Some(backup.to_string()));
        assert!(res.is_err());
    }
}
