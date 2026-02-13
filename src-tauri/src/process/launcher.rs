use crate::config::schema::BrowserProfile;
use std::path::Path;
use std::process::Command;

/// Build Chrome launch command with all parameters
pub fn build_command(chrome_path: &Path, profile: &BrowserProfile) -> Command {
    let mut cmd = Command::new(chrome_path);

    // User data directory (required)
    cmd.arg(format!(
        "--user-data-dir={}",
        profile.user_data_dir.display()
    ));

    // Fingerprint (if specified)
    if let Some(fp) = &profile.fingerprint {
        cmd.arg(format!("--fingerprint={}", fp));
    }

    // Proxy server
    if let Some(proxy) = &profile.proxy_server {
        cmd.arg(format!("--proxy-server={}", proxy));
    }

    // Language
    cmd.arg(format!("--lang={}", profile.lang));

    // Timezone (Chrome doesn't have a native --timezone flag, but some extensions use it)
    // We'll pass it as a custom argument for extensions or future Chrome features
    if let Some(tz) = &profile.timezone {
        cmd.arg(format!("--tz={}", tz));
    }

    // Custom arguments
    for arg in &profile.custom_args {
        cmd.arg(arg);
    }

    // Don't wait for Chrome to exit
    #[cfg(unix)]
    {
        use std::os::unix::process::CommandExt;
        // Detach from parent process
        unsafe {
            cmd.pre_exec(|| {
                // Create new process group
                libc::setsid();
                Ok(())
            });
        }
    }

    cmd
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    #[test]
    fn test_build_command_basic() {
        let profile = BrowserProfile {
            id: "test".to_string(),
            name: "Test".to_string(),
            description: "".to_string(),
            user_data_dir: PathBuf::from("/tmp/chrome-profile"),
            proxy_server: None,
            lang: "en-US".to_string(),
            timezone: None,
            fingerprint: None,
            color: None,
            custom_args: vec![],
        };

        let cmd = build_command(Path::new("/usr/bin/google-chrome"), &profile);
        let args: Vec<String> = cmd
            .get_args()
            .map(|s| s.to_string_lossy().to_string())
            .collect();

        assert!(args.contains(&"--user-data-dir=/tmp/chrome-profile".to_string()));
        assert!(args.contains(&"--lang=en-US".to_string()));
    }

    #[test]
    fn test_build_command_with_proxy() {
        let profile = BrowserProfile {
            id: "test".to_string(),
            name: "Test".to_string(),
            description: "".to_string(),
            user_data_dir: PathBuf::from("/tmp/chrome-profile"),
            proxy_server: Some("http://192.168.0.220:8889".to_string()),
            lang: "en-US".to_string(),
            timezone: Some("America/Los_Angeles".to_string()),
            fingerprint: Some("10000".to_string()),
            color: None,
            custom_args: vec!["--disable-gpu".to_string()],
        };

        let cmd = build_command(Path::new("/usr/bin/google-chrome"), &profile);
        let args: Vec<String> = cmd
            .get_args()
            .map(|s| s.to_string_lossy().to_string())
            .collect();

        assert!(args.contains(&"--proxy-server=http://192.168.0.220:8889".to_string()));
        assert!(args.contains(&"--fingerprint=10000".to_string()));
        assert!(args.contains(&"--tz=America/Los_Angeles".to_string()));
        assert!(args.contains(&"--disable-gpu".to_string()));
    }
}
