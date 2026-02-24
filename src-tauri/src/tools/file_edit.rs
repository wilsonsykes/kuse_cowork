use crate::agent::ToolDefinition;
use crate::tools::path_utils;
use serde_json::json;
use std::fs;
use std::path::Path;

pub fn definition() -> ToolDefinition {
    ToolDefinition {
        name: "edit_file".to_string(),
        description: "Make targeted edits to a file by replacing specific text. The old_string must match exactly (including whitespace and indentation). Use this for small, precise changes.".to_string(),
        input_schema: json!({
            "type": "object",
            "properties": {
                "path": {
                    "type": "string",
                    "description": "The path to the file to edit"
                },
                "old_string": {
                    "type": "string",
                    "description": "The exact text to find and replace (must be unique in the file)"
                },
                "new_string": {
                    "type": "string",
                    "description": "The text to replace it with"
                },
                "replace_all": {
                    "type": "boolean",
                    "description": "If true, replace all occurrences. Default is false (replace first only)"
                }
            },
            "required": ["path", "old_string", "new_string"]
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

    let old_string = input
        .get("old_string")
        .and_then(|v| v.as_str())
        .ok_or("Missing 'old_string' parameter")?;

    let new_string = input
        .get("new_string")
        .and_then(|v| v.as_str())
        .ok_or("Missing 'new_string' parameter")?;

    let replace_all = input
        .get("replace_all")
        .and_then(|v| v.as_bool())
        .unwrap_or(false);

    // Resolve path
    let path = resolve_path(path_str, project_path)?;

    // Check if file exists
    if !path.exists() {
        return Err(format!("File not found: {}", path_str));
    }

    // Read current content
    let content = fs::read_to_string(&path)
        .map_err(|e| format!("Failed to read file: {}", e))?;

    // Check if old_string exists
    let count = content.matches(old_string).count();
    if count == 0 {
        return Err(format!(
            "Could not find the specified text in {}. Make sure the old_string matches exactly, including whitespace.",
            path_str
        ));
    }

    if count > 1 && !replace_all {
        return Err(format!(
            "Found {} occurrences of the text. Use replace_all: true to replace all, or provide more context to make the match unique.",
            count
        ));
    }

    // Perform replacement
    let new_content = if replace_all {
        content.replace(old_string, new_string)
    } else {
        content.replacen(old_string, new_string, 1)
    };

    // Write back
    fs::write(&path, &new_content)
        .map_err(|e| format!("Failed to write file: {}", e))?;

    let replaced_count = if replace_all { count } else { 1 };
    Ok(format!(
        "Successfully replaced {} occurrence(s) in {}",
        replaced_count,
        path.display()
    ))
}

fn resolve_path(path_str: &str, project_path: Option<&str>) -> Result<std::path::PathBuf, String> {
    let path = Path::new(path_str);
    path_utils::resolve_path(path, project_path)
}
