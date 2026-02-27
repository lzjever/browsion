//! Download and extract CfT Chrome binary; return path to executable.

use crate::cft::api::CftVersionInfo;
use std::path::{Path, PathBuf};
use std::sync::Arc;

/// Progress update during download/extract. Used to emit to frontend.
#[derive(Clone, serde::Serialize)]
#[serde(tag = "phase", rename_all = "snake_case")]
pub enum CftProgress {
    Download { loaded: u64, total: Option<u64> },
    Extracting,
}

/// Map current OS and arch to CfT platform string.
pub fn get_platform() -> &'static str {
    #[cfg(all(target_os = "linux", target_arch = "x86_64"))]
    return "linux64";
    #[cfg(all(target_os = "linux", target_arch = "aarch64"))]
    return "linux64"; // CfT uses linux64 for both; aarch64 may need different mapping
    #[cfg(all(target_os = "macos", target_arch = "aarch64"))]
    return "mac-arm64";
    #[cfg(all(target_os = "macos", target_arch = "x86_64"))]
    return "mac-x64";
    #[cfg(all(target_os = "windows", target_arch = "x86_64"))]
    return "win64";
    #[cfg(all(target_os = "windows", target_arch = "x86"))]
    return "win32";
    #[cfg(not(any(
        all(target_os = "linux", target_arch = "x86_64"),
        all(target_os = "macos", target_arch = "aarch64"),
        all(target_os = "macos", target_arch = "x86_64"),
        all(target_os = "windows", target_arch = "x86_64"),
        all(target_os = "windows", target_arch = "x86")
    )))]
    return "linux64"; // fallback
}

/// Download zip from URL and extract to dir; return path to the Chrome binary inside.
/// If `on_progress` is provided, it is called during download (with loaded/total bytes) and before extracting.
pub async fn download_and_extract(
    url: &str,
    extract_dir: &Path,
    on_progress: Option<Arc<dyn Fn(CftProgress) + Send + Sync>>,
) -> Result<PathBuf, String> {
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(30 * 60))
        .build()
        .map_err(|e| format!("HTTP client: {}", e))?;
    let response = client
        .get(url)
        .send()
        .await
        .map_err(|e| format!("Download failed: {}", e))?
        .error_for_status()
        .map_err(|e| format!("Download failed: {}", e))?;

    let total = response.content_length();
    let mut stream = response.bytes_stream();
    let mut loaded: u64 = 0;

    std::fs::create_dir_all(extract_dir)
        .map_err(|e| format!("Cannot create directory: {}", e))?;

    let zip_path = extract_dir.join("chrome.zip");
    let mut file = std::fs::File::create(&zip_path)
        .map_err(|e| format!("Cannot create zip file: {}", e))?;

    use futures::stream::StreamExt;
    while let Some(chunk) = stream.next().await {
        let chunk = chunk.map_err(|e| format!("Download failed: {}", e))?;
        let len = chunk.len() as u64;
        loaded += len;
        std::io::Write::write_all(&mut file, &chunk)
            .map_err(|e| format!("Cannot write zip: {}", e))?;
        if let Some(ref cb) = on_progress {
            cb(CftProgress::Download {
                loaded,
                total,
            });
        }
    }

    drop(file);

    if let Some(ref cb) = on_progress {
        cb(CftProgress::Extracting);
    }

    let bin_path = extract_zip(&zip_path, extract_dir)?;
    std::fs::remove_file(zip_path).ok();
    Ok(bin_path)
}

fn extract_zip(zip_path: &Path, dest_dir: &Path) -> Result<PathBuf, String> {
    let file = std::fs::File::open(zip_path).map_err(|e| format!("Open zip: {}", e))?;
    let mut archive = zip::ZipArchive::new(file).map_err(|e| format!("Invalid zip: {}", e))?;

    for i in 0..archive.len() {
        let mut entry = archive.by_index(i).map_err(|e| format!("Zip entry: {}", e))?;
        let name = entry.name().to_string();
        let out_path = dest_dir.join(&name);
        if entry.is_dir() {
            std::fs::create_dir_all(&out_path).map_err(|e| format!("Create dir: {}", e))?;
        } else {
            if let Some(p) = out_path.parent() {
                std::fs::create_dir_all(p).map_err(|e| format!("Create dir: {}", e))?;
            }
            let mut out = std::fs::File::create(&out_path).map_err(|e| format!("Create file: {}", e))?;
            std::io::copy(&mut entry, &mut out).map_err(|e| format!("Write file: {}", e))?;
            #[cfg(unix)]
            {
                if name.ends_with("chrome") || name.ends_with("Google Chrome for Testing") {
                    use std::os::unix::fs::PermissionsExt;
                    let perms = std::fs::Permissions::from_mode(0o755);
                    std::fs::set_permissions(&out_path, perms).ok();
                }
            }
        }
    }

    // Return path to binary: linux: chrome-linux64/chrome, mac: *.app/Contents/MacOS/..., win: chrome-win64/chrome.exe
    #[cfg(target_os = "linux")]
    {
        let dir = std::fs::read_dir(dest_dir).map_err(|e| format!("Read dir: {}", e))?;
        for e in dir {
            let e = e.map_err(|e| format!("Read dir: {}", e))?;
            let name = e.file_name();
            let name = name.to_string_lossy();
            if name.starts_with("chrome-linux64") {
                let chrome = dest_dir.join(name.as_ref()).join("chrome");
                if chrome.exists() {
                    return Ok(chrome);
                }
            }
        }
    }
    #[cfg(target_os = "macos")]
    {
        let dir = std::fs::read_dir(dest_dir).map_err(|e| format!("Read dir: {}", e))?;
        for e in dir {
            let e = e.map_err(|e| format!("Read dir: {}", e))?;
            let name = e.file_name().to_string_lossy().to_string();
            if name.ends_with(".app") {
                let bin = dest_dir
                    .join(&name)
                    .join("Contents")
                    .join("MacOS")
                    .join("Google Chrome for Testing");
                if bin.exists() {
                    return Ok(bin);
                }
            }
        }
    }
    #[cfg(target_os = "windows")]
    {
        let dir = std::fs::read_dir(dest_dir).map_err(|e| format!("Read dir: {}", e))?;
        for e in dir {
            let e = e.map_err(|e| format!("Read dir: {}", e))?;
            let name = e.file_name().to_string_lossy().to_string();
            if name.starts_with("chrome-win") {
                let chrome = dest_dir.join(&name).join("chrome.exe");
                if chrome.exists() {
                    return Ok(chrome);
                }
            }
        }
    }

    Err("Chrome binary not found after extract".to_string())
}

/// Ensure the given version is present under download_dir; return path to binary.
/// version_dir is e.g. download_dir.join("145.0.7632.117").
/// If `on_progress` is set, it is passed to download_and_extract for progress reporting.
pub async fn ensure_chrome_binary(
    version_info: &CftVersionInfo,
    download_dir: &Path,
    on_progress: Option<Arc<dyn Fn(CftProgress) + Send + Sync>>,
) -> Result<PathBuf, String> {
    let version_dir = download_dir.join(&version_info.version);
    let existing = find_chrome_in_dir(&version_dir);
    if let Some(p) = existing {
        return Ok(p);
    }
    download_and_extract(&version_info.url, &version_dir, on_progress).await
}

fn find_chrome_in_dir(dir: &Path) -> Option<PathBuf> {
    if !dir.exists() {
        return None;
    }
    #[cfg(target_os = "linux")]
    {
        let d = dir.read_dir().ok()?;
        for e in d {
            let e = e.ok()?;
            let name = e.file_name().to_string_lossy().to_string();
            if name.starts_with("chrome-linux64") {
                let chrome = dir.join(name).join("chrome");
                if chrome.exists() {
                    return Some(chrome);
                }
            }
        }
    }
    #[cfg(target_os = "macos")]
    {
        let d = dir.read_dir().ok()?;
        for e in d {
            let e = e.ok()?;
            let name = e.file_name().to_string_lossy().to_string();
            if name.ends_with(".app") {
                let bin = dir
                    .join(name)
                    .join("Contents")
                    .join("MacOS")
                    .join("Google Chrome for Testing");
                if bin.exists() {
                    return Some(bin);
                }
            }
        }
    }
    #[cfg(target_os = "windows")]
    {
        let d = dir.read_dir().ok()?;
        for e in d {
            let e = e.ok()?;
            let name = e.file_name().to_string_lossy().to_string();
            if name.starts_with("chrome-win") {
                let chrome = dir.join(name).join("chrome.exe");
                if chrome.exists() {
                    return Some(chrome);
                }
            }
        }
    }
    None
}
