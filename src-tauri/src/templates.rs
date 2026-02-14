use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;

/// Template metadata extracted from markdown frontmatter
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TemplateInfo {
    /// Filename (without .md extension)
    pub id: String,
    /// Template name (from frontmatter or filename)
    pub name: String,
    /// Template content (the markdown body)
    pub content: String,
    /// Start URL (from frontmatter, optional)
    pub start_url: Option<String>,
    /// Whether to use headless mode
    pub headless: bool,
    /// Last modified timestamp
    pub modified_at: u64,
}

/// Get the templates directory path
pub fn get_templates_dir() -> PathBuf {
    let home = dirs::home_dir().unwrap_or_else(|| PathBuf::from("."));
    home.join(".config").join("browsion").join("templates")
}

/// Ensure the templates directory exists
pub fn ensure_templates_dir() -> Result<PathBuf, String> {
    let dir = get_templates_dir();
    if !dir.exists() {
        fs::create_dir_all(&dir)
            .map_err(|e| format!("Failed to create templates directory: {}", e))?;
    }
    Ok(dir)
}

/// List all template files
pub fn list_templates() -> Result<Vec<TemplateInfo>, String> {
    let dir = ensure_templates_dir()?;

    let mut templates = Vec::new();

    let entries =
        fs::read_dir(&dir).map_err(|e| format!("Failed to read templates directory: {}", e))?;

    for entry in entries.flatten() {
        let path = entry.path();
        if path.extension().map(|e| e == "md").unwrap_or(false) {
            if let Ok(info) = read_template_file(&path) {
                templates.push(info);
            }
        }
    }

    // Sort by name
    templates.sort_by(|a, b| a.name.cmp(&b.name));

    Ok(templates)
}

/// Read a template file
pub fn read_template_file(path: &PathBuf) -> Result<TemplateInfo, String> {
    let content =
        fs::read_to_string(path).map_err(|e| format!("Failed to read template: {}", e))?;

    let metadata = fs::metadata(path).ok();
    let modified_at = metadata
        .and_then(|m| m.modified().ok())
        .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
        .map(|d| d.as_secs())
        .unwrap_or(0);

    let filename = path
        .file_stem()
        .and_then(|n| n.to_str())
        .unwrap_or("unknown")
        .to_string();

    // Parse frontmatter if present
    let (name, start_url, headless, body) = parse_frontmatter(&content, &filename);

    Ok(TemplateInfo {
        id: filename,
        name,
        content: body,
        start_url,
        headless,
        modified_at,
    })
}

/// Parse YAML frontmatter from markdown content
fn parse_frontmatter(content: &str, filename: &str) -> (String, Option<String>, bool, String) {
    let mut name = filename.to_string();
    let mut start_url = None;
    let mut headless = false;
    let mut body = content.to_string();

    // Check for frontmatter
    if let Some(stripped) = content.strip_prefix("---\n") {
        if let Some(end_idx) = stripped.find("\n---\n") {
            let frontmatter = &stripped[..end_idx];
            body = stripped[end_idx + 5..].to_string();

            // Parse frontmatter lines
            for line in frontmatter.lines() {
                let line = line.trim();
                if let Some(value) = line.strip_prefix("name:") {
                    name = value.trim().to_string();
                } else if let Some(value) = line.strip_prefix("start_url:") {
                    start_url = Some(value.trim().to_string());
                } else if let Some(value) = line.strip_prefix("headless:") {
                    headless = value.trim().to_lowercase() == "true";
                }
            }
        }
    }

    (name, start_url, headless, body)
}

/// Get a single template by ID (filename without extension)
pub fn get_template(id: &str) -> Result<TemplateInfo, String> {
    let dir = ensure_templates_dir()?;
    let path = dir.join(format!("{}.md", id));

    if !path.exists() {
        return Err(format!("Template '{}' not found", id));
    }

    read_template_file(&path)
}

/// Create or update a template
pub fn save_template(
    id: &str,
    name: &str,
    content: &str,
    start_url: Option<&str>,
    headless: bool,
) -> Result<(), String> {
    let dir = ensure_templates_dir()?;

    // Sanitize ID for filename
    let safe_id: String = id
        .chars()
        .map(|c| {
            if c.is_alphanumeric() || c == '-' || c == '_' {
                c
            } else {
                '_'
            }
        })
        .collect();

    let path = dir.join(format!("{}.md", safe_id));

    // Build file content with frontmatter
    let mut file_content = String::new();
    file_content.push_str("---\n");
    file_content.push_str(&format!("name: {}\n", name));

    if let Some(url) = start_url {
        file_content.push_str(&format!("start_url: {}\n", url));
    }

    file_content.push_str(&format!("headless: {}\n", headless));
    file_content.push_str("---\n\n");
    file_content.push_str(content);

    fs::write(&path, file_content).map_err(|e| format!("Failed to write template: {}", e))?;

    Ok(())
}

/// Delete a template
pub fn delete_template(id: &str) -> Result<(), String> {
    let dir = ensure_templates_dir()?;
    let path = dir.join(format!("{}.md", id));

    if !path.exists() {
        return Err(format!("Template '{}' not found", id));
    }

    fs::remove_file(&path).map_err(|e| format!("Failed to delete template: {}", e))?;

    Ok(())
}

/// Open the templates directory in the system file manager
pub fn open_templates_dir() -> Result<(), String> {
    let dir = ensure_templates_dir()?;

    #[cfg(target_os = "windows")]
    {
        std::process::Command::new("explorer")
            .arg(&dir)
            .spawn()
            .map_err(|e| format!("Failed to open directory: {}", e))?;
    }

    #[cfg(target_os = "macos")]
    {
        std::process::Command::new("open")
            .arg(&dir)
            .spawn()
            .map_err(|e| format!("Failed to open directory: {}", e))?;
    }

    #[cfg(target_os = "linux")]
    {
        std::process::Command::new("xdg-open")
            .arg(&dir)
            .spawn()
            .map_err(|e| format!("Failed to open directory: {}", e))?;
    }

    Ok(())
}

/// Create default templates if none exist
pub fn create_default_templates() -> Result<(), String> {
    let templates = list_templates()?;
    if !templates.is_empty() {
        return Ok(());
    }

    // Create a sample template
    save_template(
        "example-task",
        "Example Task",
        "Navigate to a website and perform an action.\n\nFor example:\n- Go to example.com\n- Find the search box\n- Type your query\n- Press Enter",
        Some("https://example.com"),
        false,
    )?;

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_frontmatter_with_metadata() {
        let content = r#"---
name: Test Template
start_url: https://example.com
headless: true
---

This is the task content.
It can span multiple lines."#;

        let (name, start_url, headless, body) = parse_frontmatter(content, "default");

        assert_eq!(name, "Test Template");
        assert_eq!(start_url, Some("https://example.com".to_string()));
        assert!(headless);
        assert!(body.contains("This is the task content"));
    }

    #[test]
    fn test_parse_frontmatter_minimal() {
        let content = r#"---
name: Simple Template
---

Just a simple task."#;

        let (name, start_url, headless, body) = parse_frontmatter(content, "default");

        assert_eq!(name, "Simple Template");
        assert_eq!(start_url, None);
        assert!(!headless);
        assert!(body.contains("Just a simple task"));
    }

    #[test]
    fn test_parse_frontmatter_no_frontmatter() {
        let content = "This is just plain content without frontmatter.";

        let (name, start_url, headless, body) = parse_frontmatter(content, "filename");

        assert_eq!(name, "filename");
        assert_eq!(start_url, None);
        assert!(!headless);
        assert_eq!(body, content);
    }

    #[test]
    fn test_parse_frontmatter_headless_values() {
        // Test true
        let content = r#"---
name: Test
headless: true
---

Content."#;
        let (_, _, headless, _) = parse_frontmatter(content, "default");
        assert!(headless);

        // Test false
        let content = r#"---
name: Test
headless: false
---

Content."#;
        let (_, _, headless, _) = parse_frontmatter(content, "default");
        assert!(!headless);
    }

    #[test]
    fn test_template_info_serialization() {
        let info = TemplateInfo {
            id: "test-template".to_string(),
            name: "Test Template".to_string(),
            content: "Test content".to_string(),
            start_url: Some("https://example.com".to_string()),
            headless: true,
            modified_at: 1234567890,
        };

        // Verify it can be serialized
        let json = serde_json::to_string(&info).unwrap();
        assert!(json.contains("test-template"));
        assert!(json.contains("Test Template"));

        // Verify it can be deserialized
        let decoded: TemplateInfo = serde_json::from_str(&json).unwrap();
        assert_eq!(decoded.id, info.id);
        assert_eq!(decoded.name, info.name);
        assert_eq!(decoded.content, info.content);
    }

    #[test]
    fn test_sanitize_id() {
        // Test that save_template sanitizes the ID
        // This is implicitly tested through the safe_id creation in save_template
        let special_chars = "test template!@#$%";
        let safe: String = special_chars
            .chars()
            .map(|c| {
                if c.is_alphanumeric() || c == '-' || c == '_' {
                    c
                } else {
                    '_'
                }
            })
            .collect();
        assert_eq!(safe, "test_template_____");
    }
}
