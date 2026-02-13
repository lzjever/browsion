use crate::config::schema::BrowserProfile;
use crate::error::{BrowsionError, Result};
use std::path::Path;

/// Validate Chrome executable path
pub fn validate_chrome_path(path: &Path) -> Result<()> {
    if !path.exists() {
        return Err(BrowsionError::Validation(format!(
            "Chrome executable not found at {:?}",
            path
        )));
    }

    if !path.is_file() {
        return Err(BrowsionError::Validation(format!(
            "Chrome path {:?} is not a file",
            path
        )));
    }

    // On Unix systems, check if executable
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let metadata = path.metadata().map_err(|e| {
            BrowsionError::Validation(format!("Cannot read Chrome file metadata: {}", e))
        })?;
        let permissions = metadata.permissions();
        if permissions.mode() & 0o111 == 0 {
            return Err(BrowsionError::Validation(format!(
                "Chrome executable {:?} is not executable",
                path
            )));
        }
    }

    Ok(())
}

/// Validate browser profile configuration
pub fn validate_profile(profile: &BrowserProfile) -> Result<()> {
    // Validate ID
    if profile.id.is_empty() {
        return Err(BrowsionError::Validation(
            "Profile ID cannot be empty".to_string(),
        ));
    }

    // Validate name
    if profile.name.trim().is_empty() {
        return Err(BrowsionError::Validation(
            "Profile name cannot be empty".to_string(),
        ));
    }

    // Validate user data directory path format (don't check existence, it will be created)
    if profile.user_data_dir.as_os_str().is_empty() {
        return Err(BrowsionError::Validation(
            "User data directory cannot be empty".to_string(),
        ));
    }

    // Validate proxy server format if provided
    if let Some(proxy) = &profile.proxy_server {
        if !proxy.starts_with("http://")
            && !proxy.starts_with("https://")
            && !proxy.starts_with("socks4://")
            && !proxy.starts_with("socks5://")
        {
            return Err(BrowsionError::Validation(format!(
                "Invalid proxy server format: {}. Must start with http://, https://, socks4://, or socks5://",
                proxy
            )));
        }
    }

    // Validate language code format (basic check)
    if profile.lang.is_empty() {
        return Err(BrowsionError::Validation(
            "Language code cannot be empty".to_string(),
        ));
    }

    // Validate color format if provided
    if let Some(color) = &profile.color {
        if !color.starts_with('#') || (color.len() != 4 && color.len() != 7) {
            return Err(BrowsionError::Validation(format!(
                "Invalid color format: {}. Must be hex color like #RGB or #RRGGBB",
                color
            )));
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::schema::BrowserProfile;
    use std::path::PathBuf;

    #[test]
    fn test_validate_valid_profile() {
        let profile = BrowserProfile {
            id: "test-123".to_string(),
            name: "Test Profile".to_string(),
            description: "Test".to_string(),
            user_data_dir: PathBuf::from("/tmp/test"),
            proxy_server: Some("http://localhost:8080".to_string()),
            lang: "en-US".to_string(),
            timezone: Some("America/New_York".to_string()),
            fingerprint: Some("12345".to_string()),
            color: Some("#FF5733".to_string()),
            custom_args: vec![],
        };

        assert!(validate_profile(&profile).is_ok());
    }

    #[test]
    fn test_validate_empty_id() {
        let mut profile = BrowserProfile {
            id: "".to_string(),
            name: "Test".to_string(),
            description: "".to_string(),
            user_data_dir: PathBuf::from("/tmp/test"),
            proxy_server: None,
            lang: "en-US".to_string(),
            timezone: None,
            fingerprint: None,
            color: None,
            custom_args: vec![],
        };

        assert!(validate_profile(&profile).is_err());
    }

    #[test]
    fn test_validate_invalid_proxy() {
        let profile = BrowserProfile {
            id: "test".to_string(),
            name: "Test".to_string(),
            description: "".to_string(),
            user_data_dir: PathBuf::from("/tmp/test"),
            proxy_server: Some("invalid-proxy".to_string()),
            lang: "en-US".to_string(),
            timezone: None,
            fingerprint: None,
            color: None,
            custom_args: vec![],
        };

        assert!(validate_profile(&profile).is_err());
    }

    #[test]
    fn test_validate_invalid_color() {
        let profile = BrowserProfile {
            id: "test".to_string(),
            name: "Test".to_string(),
            description: "".to_string(),
            user_data_dir: PathBuf::from("/tmp/test"),
            proxy_server: None,
            lang: "en-US".to_string(),
            timezone: None,
            fingerprint: None,
            color: Some("red".to_string()),
            custom_args: vec![],
        };

        assert!(validate_profile(&profile).is_err());
    }
}
