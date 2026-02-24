use crate::agent::ToolDefinition;
use crate::tools::path_utils;
use serde_json::json;
use std::fs;
use std::path::Path;

pub fn definition() -> ToolDefinition {
    ToolDefinition {
        name: "write_file".to_string(),
        description: "Write content to a file. Creates the file if it doesn't exist, or overwrites if it does. Use for creating new files or complete rewrites.".to_string(),
        input_schema: json!({
            "type": "object",
            "properties": {
                "path": {
                    "type": "string",
                    "description": "The path to the file to write"
                },
                "content": {
                    "type": "string",
                    "description": "The content to write to the file"
                }
            },
            "required": ["path", "content"]
        }),
    }
}

pub fn execute(
    input: &serde_json::Value,
    project_path: Option<&str>,
) -> Result<String, String> {
    let path_str = input
        .get("path")
        .and_then(|v| v.as_str())
        .ok_or("Missing 'path' parameter")?;

    let content = input
        .get("content")
        .and_then(|v| v.as_str())
        .ok_or("Missing 'content' parameter")?;

    // Resolve path
    let path = resolve_path(path_str, project_path)?;

    // Create parent directories if needed
    if let Some(parent) = path.parent() {
        if !parent.exists() {
            fs::create_dir_all(parent)
                .map_err(|e| format!("Failed to create directories: {}", e))?;
        }
    }

    // Write file
    fs::write(&path, content)
        .map_err(|e| format!("Failed to write file: {}", e))?;

    let line_count = content.lines().count();
    Ok(format!(
        "Successfully wrote {} lines to {}",
        line_count,
        path.display()
    ))
}

fn resolve_path(path_str: &str, project_path: Option<&str>) -> Result<std::path::PathBuf, String> {
    let path = Path::new(path_str);
    path_utils::resolve_path(path, project_path)
}
