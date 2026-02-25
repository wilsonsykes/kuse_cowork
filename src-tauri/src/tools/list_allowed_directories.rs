use crate::agent::ToolDefinition;
use crate::tools::path_utils;
use serde_json::json;

pub fn definition() -> ToolDefinition {
    ToolDefinition {
        name: "list_allowed_directories".to_string(),
        description: "List mounted workspace root directories currently allowed for filesystem operations.".to_string(),
        input_schema: json!({
            "type": "object",
            "properties": {}
        }),
    }
}

pub fn execute(_input: &serde_json::Value, project_path: Option<&str>) -> Result<String, String> {
    let mut roots = path_utils::parse_project_roots(project_path);
    if roots.is_empty() {
        if let Ok(default_root) = path_utils::default_local_workspace_root() {
            roots.push(default_root);
        }
    }
    if roots.is_empty() {
        return Err("No allowed directories found".to_string());
    }

    let output = roots
        .into_iter()
        .map(|p| p.to_string_lossy().to_string())
        .collect::<Vec<_>>()
        .join("\n");
    Ok(output)
}
