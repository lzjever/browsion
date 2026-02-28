use serde::{Deserialize, Serialize};
use std::env;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct McpToolInfo {
    pub id: String,
    pub name: String,
    pub config_path: String,
    pub found: bool,
    pub scope: ToolScope,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ToolScope {
    Global,
    ProjectScoped,
}

#[tauri::command]
pub async fn detect_mcp_tools() -> Result<Vec<McpToolInfo>, String> {
    let home = dirs::home_dir().unwrap_or_else(|| PathBuf::from("."));

    let cursor_path = home.join(".cursor").join("mcp.json");
    let claude_path = home.join(".claude.json");
    let codex_path = home.join(".codex").join("config.toml");
    let windsurf_path = home
        .join(".codeium")
        .join("windsurf")
        .join("mcp_config.json");
    let zed_path = get_zed_path(&home);

    let tools = vec![
        McpToolInfo {
            id: "cursor".into(),
            name: "Cursor".into(),
            found: cursor_path.exists(),
            config_path: cursor_path.to_string_lossy().into(),
            scope: ToolScope::Global,
        },
        McpToolInfo {
            id: "claude_code".into(),
            name: "Claude Code".into(),
            found: claude_path.exists(),
            config_path: claude_path.to_string_lossy().into(),
            scope: ToolScope::Global,
        },
        McpToolInfo {
            id: "codex".into(),
            name: "Codex CLI".into(),
            found: codex_path.exists(),
            config_path: codex_path.to_string_lossy().into(),
            scope: ToolScope::Global,
        },
        McpToolInfo {
            id: "windsurf".into(),
            name: "Windsurf".into(),
            found: windsurf_path.exists(),
            config_path: windsurf_path.to_string_lossy().into(),
            scope: ToolScope::Global,
        },
        McpToolInfo {
            id: "zed".into(),
            name: "Zed".into(),
            found: zed_path.exists(),
            config_path: zed_path.to_string_lossy().into(),
            scope: ToolScope::Global,
        },
        McpToolInfo {
            id: "continue_vscode".into(),
            name: "Continue".into(),
            found: false,
            config_path: ".continue/mcpServers/browsion.json (in your project)".into(),
            scope: ToolScope::ProjectScoped,
        },
        McpToolInfo {
            id: "openclaw".into(),
            name: "OpenClaw".into(),
            found: false,
            config_path: "openclaw.json (in your project)".into(),
            scope: ToolScope::ProjectScoped,
        },
    ];

    Ok(tools)
}

fn get_zed_path(home: &Path) -> PathBuf {
    #[cfg(target_os = "macos")]
    {
        home.join("Library")
            .join("Application Support")
            .join("Zed")
            .join("settings.json")
    }
    #[cfg(windows)]
    {
        dirs::config_dir()
            .unwrap_or_else(|| home.join("AppData").join("Roaming"))
            .join("Zed")
            .join("settings.json")
    }
    #[cfg(not(any(target_os = "macos", windows)))]
    {
        home.join(".config").join("zed").join("settings.json")
    }
}

// ---------------------------------------------------------------------------
// Safety helpers
// ---------------------------------------------------------------------------

/// Atomic write: write to a temp file first, then rename.
/// Prevents partial writes from corrupting an existing config file.
fn atomic_write(path: &Path, content: &str) -> io::Result<()> {
    let parent = path
        .parent()
        .ok_or_else(|| io::Error::other("path has no parent directory"))?;
    let file_name = path
        .file_name()
        .ok_or_else(|| io::Error::other("path has no file name"))?
        .to_string_lossy();
    let tmp_path = parent.join(format!(".browsion_{}.tmp", file_name));

    fs::write(&tmp_path, content)?;

    // Windows: rename fails if destination already exists — remove it first.
    #[cfg(windows)]
    if path.exists() {
        fs::remove_file(path)?;
    }

    fs::rename(&tmp_path, path)
        .inspect_err(|_| {
            let _ = fs::remove_file(&tmp_path); // clean up on failure
        })
        ?;

    Ok(())
}

/// Strip `//` line comments and `/* */` block comments from a JSONC string.
/// Needed for Zed's settings.json, which is JSONC (serde_json cannot parse it).
/// Correctly handles comments inside string literals.
fn strip_jsonc_comments(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut chars = s.chars().peekable();
    let mut in_string = false;
    let mut escape = false;

    while let Some(c) = chars.next() {
        if escape {
            out.push(c);
            escape = false;
            continue;
        }
        if in_string {
            if c == '\\' {
                escape = true;
                out.push(c);
            } else if c == '"' {
                in_string = false;
                out.push(c);
            } else {
                out.push(c);
            }
            continue;
        }
        match c {
            '"' => {
                in_string = true;
                out.push(c);
            }
            '/' if chars.peek() == Some(&'/') => {
                // Line comment — skip to end of line, preserve the newline
                chars.next();
                for nc in chars.by_ref() {
                    if nc == '\n' {
                        out.push('\n');
                        break;
                    }
                }
            }
            '/' if chars.peek() == Some(&'*') => {
                // Block comment — skip until */
                chars.next();
                loop {
                    match chars.next() {
                        Some('*') if chars.peek() == Some(&'/') => {
                            chars.next();
                            break;
                        }
                        Some('\n') => out.push('\n'), // preserve line numbers
                        None => break,
                        _ => {}
                    }
                }
            }
            _ => out.push(c),
        }
    }
    out
}

/// Parse JSON from a string that may contain JSONC comments.
/// Returns an error with a helpful message if parsing fails.
fn parse_json_file(content: &str, path: &Path) -> io::Result<serde_json::Value> {
    let stripped = strip_jsonc_comments(content);
    serde_json::from_str(&stripped).map_err(|e| {
        io::Error::new(
            io::ErrorKind::InvalidData,
            format!(
                "Failed to parse {}: {}. The file may have invalid JSON.",
                path.display(),
                e
            ),
        )
    })
}

/// Read an existing JSON file (JSONC-aware), or return `{}` if it does not exist.
/// Validates that the root is a JSON object.
fn read_json_object(path: &Path) -> io::Result<serde_json::Value> {
    let value = if path.exists() {
        let content = fs::read_to_string(path)?;
        parse_json_file(&content, path)?
    } else {
        serde_json::Value::Object(serde_json::Map::new())
    };

    if !value.is_object() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            format!(
                "{} contains invalid JSON (root is not an object).",
                path.display()
            ),
        ));
    }
    Ok(value)
}

// ---------------------------------------------------------------------------
// Server entry builder
// ---------------------------------------------------------------------------

fn build_server_entry(binary_path: &str, api_port: u16, api_key: Option<&str>) -> serde_json::Value {
    let mut env = serde_json::json!({
        "BROWSION_API_PORT": api_port.to_string(),
    });
    if let Some(key) = api_key {
        if !key.is_empty() {
            env["BROWSION_API_KEY"] = serde_json::Value::String(key.to_string());
        }
    }
    serde_json::json!({
        "command": binary_path,
        "env": env,
    })
}

// ---------------------------------------------------------------------------
// Per-tool write helpers
// ---------------------------------------------------------------------------

/// Used by: cursor, claude_code, windsurf, openclaw
/// Format: `{ "mcpServers": { "browsion": { ... } } }`
fn write_json_mcpservers_to_path(
    path: &Path,
    binary_path: &str,
    api_port: u16,
    api_key: Option<&str>,
) -> io::Result<()> {
    let mut value = read_json_object(path)?;
    // Ensure mcpServers key exists as an object before inserting
    if !value["mcpServers"].is_object() {
        value["mcpServers"] = serde_json::Value::Object(serde_json::Map::new());
    }
    value["mcpServers"]["browsion"] = build_server_entry(binary_path, api_port, api_key);
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    let json = serde_json::to_string_pretty(&value)
        .map_err(io::Error::other)?;
    atomic_write(path, &json)
}

/// Used by: zed
/// Format: `{ "context_servers": { "browsion": { "command": { "path": ..., "env": {...} } } } }`
/// Zed's settings.json is JSONC — comments are stripped before parsing.
fn write_json_zed_to_path(
    path: &Path,
    binary_path: &str,
    api_port: u16,
    api_key: Option<&str>,
) -> io::Result<()> {
    let mut value = read_json_object(path)?;

    let mut env = serde_json::json!({
        "BROWSION_API_PORT": api_port.to_string(),
    });
    if let Some(key) = api_key {
        if !key.is_empty() {
            env["BROWSION_API_KEY"] = serde_json::Value::String(key.to_string());
        }
    }
    // Ensure context_servers key exists as an object
    if !value["context_servers"].is_object() {
        value["context_servers"] = serde_json::Value::Object(serde_json::Map::new());
    }
    value["context_servers"]["browsion"] = serde_json::json!({
        "command": {
            "path": binary_path,
            "env": env,
        }
    });
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    // NOTE: writing back as plain JSON; comments from the original file are lost.
    // This is unavoidable without a JSONC-preserving serializer.
    let json = serde_json::to_string_pretty(&value)
        .map_err(io::Error::other)?;
    atomic_write(path, &json)
}

/// Used by: codex
/// Format: TOML `[mcp_servers.browsion]` with `command` and `[mcp_servers.browsion.env]`
fn write_toml_codex_to_path(
    path: &Path,
    binary_path: &str,
    api_port: u16,
    api_key: Option<&str>,
) -> io::Result<()> {
    let mut table: toml::Table = if path.exists() {
        let content = fs::read_to_string(path)?;
        toml::from_str(&content).map_err(|e| {
            io::Error::new(
                io::ErrorKind::InvalidData,
                format!("Failed to parse {}: {}", path.display(), e),
            )
        })?
    } else {
        toml::Table::new()
    };

    let mcp_servers = table
        .entry("mcp_servers")
        .or_insert_with(|| toml::Value::Table(toml::Table::new()))
        .as_table_mut()
        .ok_or_else(|| {
            io::Error::new(io::ErrorKind::InvalidData, "mcp_servers is not a TOML table")
        })?;

    let mut browsion = toml::Table::new();
    browsion.insert(
        "command".to_string(),
        toml::Value::String(binary_path.to_string()),
    );

    let mut env = toml::Table::new();
    env.insert(
        "BROWSION_API_PORT".to_string(),
        toml::Value::String(api_port.to_string()),
    );
    if let Some(key) = api_key {
        if !key.is_empty() {
            env.insert(
                "BROWSION_API_KEY".to_string(),
                toml::Value::String(key.to_string()),
            );
        }
    }
    browsion.insert("env".to_string(), toml::Value::Table(env));
    mcp_servers.insert("browsion".to_string(), toml::Value::Table(browsion));

    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    let toml_str =
        toml::to_string_pretty(&table).map_err(io::Error::other)?;
    atomic_write(path, &toml_str)
}

/// Used by: continue_vscode
/// Each MCP server gets its own file in `.continue/mcpServers/`.
/// Format: `{ "name": "browsion", "command": "...", "env": {...} }`
/// Merges with existing file to preserve any extra user fields.
fn write_json_continue_to_path(
    path: &Path,
    binary_path: &str,
    api_port: u16,
    api_key: Option<&str>,
) -> io::Result<()> {
    // Read and merge with existing file if present, so extra user fields are preserved.
    let mut value = if path.exists() {
        read_json_object(path)?
    } else {
        serde_json::Value::Object(serde_json::Map::new())
    };

    let mut env = serde_json::json!({
        "BROWSION_API_PORT": api_port.to_string(),
    });
    if let Some(key) = api_key {
        if !key.is_empty() {
            env["BROWSION_API_KEY"] = serde_json::Value::String(key.to_string());
        }
    }
    value["name"] = serde_json::Value::String("browsion".to_string());
    value["command"] = serde_json::Value::String(binary_path.to_string());
    value["env"] = env;

    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    let json = serde_json::to_string_pretty(&value)
        .map_err(io::Error::other)?;
    atomic_write(path, &json)
}

// ---------------------------------------------------------------------------
// Tauri commands
// ---------------------------------------------------------------------------

#[tauri::command]
pub async fn write_browsion_to_tool(
    tool_id: String,
    binary_path: String,
    project_dir: Option<String>,
    api_port: u16,
    api_key: Option<String>,
) -> Result<String, String> {
    let home = dirs::home_dir().unwrap_or_else(|| PathBuf::from("."));
    let api_key_ref = api_key.as_deref();

    let path = match tool_id.as_str() {
        "cursor" => {
            let p = home.join(".cursor").join("mcp.json");
            write_json_mcpservers_to_path(&p, &binary_path, api_port, api_key_ref)
                .map_err(|e| e.to_string())?;
            p
        }
        "claude_code" => {
            let p = home.join(".claude.json");
            write_json_mcpservers_to_path(&p, &binary_path, api_port, api_key_ref)
                .map_err(|e| e.to_string())?;
            p
        }
        "codex" => {
            let p = home.join(".codex").join("config.toml");
            write_toml_codex_to_path(&p, &binary_path, api_port, api_key_ref)
                .map_err(|e| e.to_string())?;
            p
        }
        "windsurf" => {
            let p = home
                .join(".codeium")
                .join("windsurf")
                .join("mcp_config.json");
            write_json_mcpservers_to_path(&p, &binary_path, api_port, api_key_ref)
                .map_err(|e| e.to_string())?;
            p
        }
        "zed" => {
            let p = get_zed_path(&home);
            write_json_zed_to_path(&p, &binary_path, api_port, api_key_ref)
                .map_err(|e| e.to_string())?;
            p
        }
        "continue_vscode" => {
            let dir = project_dir
                .ok_or_else(|| "project_dir required for continue_vscode".to_string())?;
            let p = PathBuf::from(&dir)
                .join(".continue")
                .join("mcpServers")
                .join("browsion.json");
            write_json_continue_to_path(&p, &binary_path, api_port, api_key_ref)
                .map_err(|e| e.to_string())?;
            p
        }
        "openclaw" => {
            let dir =
                project_dir.ok_or_else(|| "project_dir required for openclaw".to_string())?;
            let p = PathBuf::from(&dir).join("openclaw.json");
            write_json_mcpservers_to_path(&p, &binary_path, api_port, api_key_ref)
                .map_err(|e| e.to_string())?;
            p
        }
        _ => return Err(format!("Unknown tool_id: {}", tool_id)),
    };

    Ok(path.to_string_lossy().to_string())
}

#[tauri::command]
pub async fn find_mcp_binary() -> Option<String> {
    let exe = env::current_exe().ok()?;
    let exe_dir = exe.parent()?;

    let bin_name = if cfg!(windows) {
        "browsion-mcp.exe"
    } else {
        "browsion-mcp"
    };

    // 1. Same directory as current executable (production / installed)
    let candidate = exe_dir.join(bin_name);
    if candidate.exists() {
        return Some(candidate.to_string_lossy().to_string());
    }

    // 2. macOS app bundle: ../Resources/browsion-mcp
    let candidate = exe_dir.join("..").join("Resources").join(bin_name);
    if candidate.exists() {
        return candidate
            .canonicalize()
            .ok()
            .map(|p| p.to_string_lossy().to_string());
    }

    // 3. Dev mode: walk up from exe dir looking for target/release/browsion-mcp
    //    e.g. exe is at src-tauri/target/debug/browsion,
    //    walks: target/debug → target → src-tauri → finds src-tauri/target/release/browsion-mcp
    let mut dir = exe_dir.to_path_buf();
    for _ in 0..8 {
        let candidate = dir.join("target").join("release").join(bin_name);
        if candidate.exists() {
            return Some(candidate.to_string_lossy().to_string());
        }
        if !dir.pop() {
            break;
        }
    }

    None
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    /// Create a unique temporary directory for this test run.
    /// Returns the path; caller is responsible for cleanup (or let OS clean /tmp).
    fn make_temp_dir(label: &str) -> PathBuf {
        let nanos = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .subsec_nanos();
        let dir = std::env::temp_dir().join(format!("browsion_test_{}_{}", label, nanos));
        fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn test_strip_jsonc_line_comment() {
        let input = "{ \"a\": 1 // comment\n}";
        let out = strip_jsonc_comments(input);
        let v: serde_json::Value = serde_json::from_str(&out).unwrap();
        assert_eq!(v["a"], 1);
    }

    #[test]
    fn test_strip_jsonc_block_comment() {
        let input = r#"{ /* block */ "a": 1 }"#;
        let out = strip_jsonc_comments(input);
        let v: serde_json::Value = serde_json::from_str(&out).unwrap();
        assert_eq!(v["a"], 1);
    }

    #[test]
    fn test_strip_jsonc_comment_inside_string_preserved() {
        let input = r#"{ "url": "http://example.com" }"#;
        let out = strip_jsonc_comments(input);
        let v: serde_json::Value = serde_json::from_str(&out).unwrap();
        assert_eq!(v["url"], "http://example.com");
    }

    #[test]
    fn test_mcpservers_creates_new_file() {
        let dir = make_temp_dir("mcpservers_creates");
        let p = dir.join("mcp.json");
        write_json_mcpservers_to_path(&p, "/usr/bin/browsion-mcp", 38472, None).unwrap();
        let v: serde_json::Value =
            serde_json::from_str(&fs::read_to_string(&p).unwrap()).unwrap();
        assert_eq!(v["mcpServers"]["browsion"]["command"], "/usr/bin/browsion-mcp");
        assert_eq!(v["mcpServers"]["browsion"]["env"]["BROWSION_API_PORT"], "38472");
        assert!(v["mcpServers"]["browsion"]["env"]["BROWSION_API_KEY"].is_null());
    }

    #[test]
    fn test_mcpservers_merges_existing_content() {
        let dir = make_temp_dir("mcpservers_merges");
        let p = dir.join("mcp.json");
        fs::write(
            &p,
            r#"{ "mcpServers": { "other": { "command": "other-binary" } } }"#,
        )
        .unwrap();
        write_json_mcpservers_to_path(&p, "/bin/browsion-mcp", 38472, None).unwrap();
        let v: serde_json::Value =
            serde_json::from_str(&fs::read_to_string(&p).unwrap()).unwrap();
        assert_eq!(v["mcpServers"]["other"]["command"], "other-binary");
        assert_eq!(v["mcpServers"]["browsion"]["command"], "/bin/browsion-mcp");
    }

    #[test]
    fn test_mcpservers_with_api_key() {
        let dir = make_temp_dir("mcpservers_apikey");
        let p = dir.join("mcp.json");
        write_json_mcpservers_to_path(&p, "/bin/browsion-mcp", 38472, Some("sk-abc123")).unwrap();
        let v: serde_json::Value =
            serde_json::from_str(&fs::read_to_string(&p).unwrap()).unwrap();
        assert_eq!(v["mcpServers"]["browsion"]["env"]["BROWSION_API_KEY"], "sk-abc123");
    }

    #[test]
    fn test_mcpservers_preserves_top_level_fields() {
        let dir = make_temp_dir("mcpservers_toplevel");
        let p = dir.join("claude.json");
        fs::write(
            &p,
            r#"{ "numStartups": 42, "theme": "dark", "mcpServers": {} }"#,
        )
        .unwrap();
        write_json_mcpservers_to_path(&p, "/bin/browsion-mcp", 38472, None).unwrap();
        let v: serde_json::Value =
            serde_json::from_str(&fs::read_to_string(&p).unwrap()).unwrap();
        assert_eq!(v["numStartups"], 42);
        assert_eq!(v["theme"], "dark");
        assert_eq!(v["mcpServers"]["browsion"]["command"], "/bin/browsion-mcp");
    }

    #[test]
    fn test_mcpservers_updates_existing_browsion_entry() {
        let dir = make_temp_dir("mcpservers_update");
        let p = dir.join("mcp.json");
        fs::write(
            &p,
            r#"{ "mcpServers": { "browsion": { "command": "old-path" } } }"#,
        )
        .unwrap();
        write_json_mcpservers_to_path(&p, "/new/browsion-mcp", 9999, None).unwrap();
        let v: serde_json::Value =
            serde_json::from_str(&fs::read_to_string(&p).unwrap()).unwrap();
        assert_eq!(v["mcpServers"]["browsion"]["command"], "/new/browsion-mcp");
        assert_eq!(v["mcpServers"]["browsion"]["env"]["BROWSION_API_PORT"], "9999");
    }

    #[test]
    fn test_mcpservers_invalid_json_returns_error() {
        let dir = make_temp_dir("mcpservers_invalid");
        let p = dir.join("mcp.json");
        let bad = "{ this is not valid JSON }";
        fs::write(&p, bad).unwrap();
        let result = write_json_mcpservers_to_path(&p, "/bin/browsion-mcp", 38472, None);
        assert!(result.is_err());
        // Original file must be untouched (atomic write protects it)
        assert_eq!(fs::read_to_string(&p).unwrap(), bad);
    }

    #[test]
    fn test_mcpservers_non_object_root_returns_error() {
        let dir = make_temp_dir("mcpservers_null_root");
        let p = dir.join("mcp.json");
        fs::write(&p, "null").unwrap();
        let result = write_json_mcpservers_to_path(&p, "/bin/browsion-mcp", 38472, None);
        assert!(result.is_err());
        // Original file must be untouched
        assert_eq!(fs::read_to_string(&p).unwrap(), "null");
    }

    #[test]
    fn test_zed_jsonc_with_comments() {
        let dir = make_temp_dir("zed_jsonc");
        let p = dir.join("settings.json");
        fs::write(
            &p,
            "// Zed settings\n{\n  \"theme\": \"One Dark\", // my theme\n  /* editor config */\n  \"vim_mode\": true\n}",
        )
        .unwrap();
        write_json_zed_to_path(&p, "/bin/browsion-mcp", 38472, None).unwrap();
        let v: serde_json::Value =
            serde_json::from_str(&fs::read_to_string(&p).unwrap()).unwrap();
        assert_eq!(v["theme"], "One Dark");
        assert_eq!(v["vim_mode"], true);
        assert_eq!(
            v["context_servers"]["browsion"]["command"]["path"],
            "/bin/browsion-mcp"
        );
    }

    #[test]
    fn test_zed_merges_existing_context_servers() {
        let dir = make_temp_dir("zed_merge");
        let p = dir.join("settings.json");
        fs::write(
            &p,
            r#"{ "context_servers": { "other-server": { "command": { "path": "/bin/other" } } } }"#,
        )
        .unwrap();
        write_json_zed_to_path(&p, "/bin/browsion-mcp", 38472, None).unwrap();
        let v: serde_json::Value =
            serde_json::from_str(&fs::read_to_string(&p).unwrap()).unwrap();
        assert_eq!(
            v["context_servers"]["other-server"]["command"]["path"],
            "/bin/other"
        );
        assert_eq!(
            v["context_servers"]["browsion"]["command"]["path"],
            "/bin/browsion-mcp"
        );
    }

    #[test]
    fn test_codex_toml_creates_new_file() {
        let dir = make_temp_dir("codex_creates");
        let p = dir.join("config.toml");
        write_toml_codex_to_path(&p, "/bin/browsion-mcp", 38472, None).unwrap();
        let content = fs::read_to_string(&p).unwrap();
        let t: toml::Table = toml::from_str(&content).unwrap();
        assert_eq!(
            t["mcp_servers"]["browsion"]["command"].as_str().unwrap(),
            "/bin/browsion-mcp"
        );
        assert_eq!(
            t["mcp_servers"]["browsion"]["env"]["BROWSION_API_PORT"]
                .as_str()
                .unwrap(),
            "38472"
        );
    }

    #[test]
    fn test_codex_toml_merges_existing() {
        let dir = make_temp_dir("codex_merges");
        let p = dir.join("config.toml");
        fs::write(&p, "[mcp_servers.other]\ncommand = \"other-bin\"\n").unwrap();
        write_toml_codex_to_path(&p, "/bin/browsion-mcp", 38472, None).unwrap();
        let content = fs::read_to_string(&p).unwrap();
        let t: toml::Table = toml::from_str(&content).unwrap();
        assert_eq!(
            t["mcp_servers"]["other"]["command"].as_str().unwrap(),
            "other-bin"
        );
        assert_eq!(
            t["mcp_servers"]["browsion"]["command"].as_str().unwrap(),
            "/bin/browsion-mcp"
        );
    }

    #[test]
    fn test_continue_creates_new_file() {
        let dir = make_temp_dir("continue_creates");
        let p = dir.join("browsion.json");
        write_json_continue_to_path(&p, "/bin/browsion-mcp", 38472, None).unwrap();
        let v: serde_json::Value =
            serde_json::from_str(&fs::read_to_string(&p).unwrap()).unwrap();
        assert_eq!(v["name"], "browsion");
        assert_eq!(v["command"], "/bin/browsion-mcp");
        assert_eq!(v["env"]["BROWSION_API_PORT"], "38472");
    }

    #[test]
    fn test_continue_merges_existing_extra_fields() {
        let dir = make_temp_dir("continue_merges");
        let p = dir.join("browsion.json");
        fs::write(&p, r#"{ "name": "browsion", "command": "old", "timeout": 30 }"#).unwrap();
        write_json_continue_to_path(&p, "/new/browsion-mcp", 38472, None).unwrap();
        let v: serde_json::Value =
            serde_json::from_str(&fs::read_to_string(&p).unwrap()).unwrap();
        assert_eq!(v["command"], "/new/browsion-mcp");
        assert_eq!(v["timeout"], 30); // extra field preserved
    }

    #[test]
    fn test_atomic_write_no_tmp_left_on_success() {
        let dir = make_temp_dir("atomic_write");
        let p = dir.join("out.json");
        atomic_write(&p, "{}").unwrap();
        assert!(p.exists());
        let tmp_count = fs::read_dir(&dir)
            .unwrap()
            .filter(|e| {
                e.as_ref()
                    .unwrap()
                    .file_name()
                    .to_string_lossy()
                    .ends_with(".tmp")
            })
            .count();
        assert_eq!(tmp_count, 0);
    }
}
