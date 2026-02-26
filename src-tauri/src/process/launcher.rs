use crate::config::schema::BrowserProfile;
use std::path::Path;
use std::process::Command;

/// Build Chrome launch command with all parameters.
/// `cdp_port` enables `--remote-debugging-port` so CDP can attach later.
pub fn build_command(chrome_path: &Path, profile: &BrowserProfile, cdp_port: u16) -> Command {
    let mut cmd = Command::new(chrome_path);

    // User data directory (required)
    cmd.arg(format!(
        "--user-data-dir={}",
        profile.user_data_dir.display()
    ));

    // CDP remote-debugging port (always set so the browser is controllable)
    cmd.arg(format!("--remote-debugging-port={}", cdp_port));

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

    // Timezone (for fingerprint-chromium and general Chromium)
    // Set both --timezone (used by fingerprint-chromium) and TZ env (used by process/libc)
    // so the browser and JS see the correct timezone.
    if let Some(tz) = &profile.timezone {
        cmd.arg(format!("--timezone={}", tz));
        cmd.env("TZ", tz);
    }

    // Stability and compatibility flags
    cmd.arg("--no-first-run");           // Skip first run wizards
    cmd.arg("--no-default-browser-check"); // Don't check if default browser
    cmd.arg("--disable-background-networking"); // Disable various background network features
    cmd.arg("--disable-client-side-phishing-detection");
    cmd.arg("--disable-default-apps");
    cmd.arg("--disable-sync");            // Disable sync
    cmd.arg("--metrics-recording-only");  // Disable metrics upload
    cmd.arg("--disable-blink-features=AutomationControlled"); // Hide automation
    cmd.arg("--disable-crash-reporter");  // Disable crash reporter to avoid crashpad issues
    cmd.arg("--disable-in-process-stack-traces"); // Disable stack traces

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
            tags: vec![],
        };

        let cmd = build_command(Path::new("/usr/bin/google-chrome"), &profile, 9300);
        let args: Vec<String> = cmd
            .get_args()
            .map(|s| s.to_string_lossy().to_string())
            .collect();

        assert!(args.contains(&"--user-data-dir=/tmp/chrome-profile".to_string()));
        assert!(args.contains(&"--lang=en-US".to_string()));
        assert!(args.contains(&"--remote-debugging-port=9300".to_string()));
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
            tags: vec![],
        };

        let cmd = build_command(Path::new("/usr/bin/google-chrome"), &profile, 9301);
        let args: Vec<String> = cmd
            .get_args()
            .map(|s| s.to_string_lossy().to_string())
            .collect();

        assert!(args.contains(&"--proxy-server=http://192.168.0.220:8889".to_string()));
        assert!(args.contains(&"--fingerprint=10000".to_string()));
        assert!(args.contains(&"--timezone=America/Los_Angeles".to_string()));
        assert!(args.contains(&"--disable-gpu".to_string()));
        assert!(args.contains(&"--remote-debugging-port=9301".to_string()));
    }
}
