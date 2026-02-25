use crate::agent::ToolDefinition;
use crate::tools::path_utils;
use rust_xlsxwriter::Workbook;
use serde_json::json;
use std::fs;
use std::path::Path;

pub fn definition() -> ToolDefinition {
    ToolDefinition {
        name: "create_xlsx_file".to_string(),
        description: "Create a valid .xlsx Excel file from headers and rows.".to_string(),
        input_schema: json!({
            "type": "object",
            "properties": {
                "path": {
                    "type": "string",
                    "description": "Output file path ending with .xlsx"
                },
                "sheet_name": {
                    "type": "string",
                    "description": "Worksheet name (default: Sheet1)"
                },
                "headers": {
                    "type": "array",
                    "description": "Optional header row (array of strings)",
                    "items": { "type": "string" }
                },
                "rows": {
                    "type": "array",
                    "description": "Data rows as array of arrays",
                    "items": {
                        "type": "array",
                        "items": {
                            "anyOf": [
                                { "type": "string" },
                                { "type": "number" },
                                { "type": "boolean" },
                                { "type": "null" }
                            ]
                        }
                    }
                }
            },
            "required": ["path", "rows"],
            "additionalProperties": false
        }),
    }
}

pub fn execute(input: &serde_json::Value, project_path: Option<&str>) -> Result<String, String> {
    let path_str = input
        .get("path")
        .and_then(|v| v.as_str())
        .ok_or("Missing 'path' parameter")?;

    if !path_str.to_lowercase().ends_with(".xlsx") {
        return Err("Path must end with .xlsx".to_string());
    }

    let sheet_name = input
        .get("sheet_name")
        .and_then(|v| v.as_str())
        .unwrap_or("Sheet1");

    let headers: Vec<String> = input
        .get("headers")
        .and_then(|v| v.as_array())
        .map(|arr| {
            arr.iter()
                .map(|v| v.as_str().unwrap_or_default().to_string())
                .collect()
        })
        .unwrap_or_default();

    let rows = input
        .get("rows")
        .and_then(|v| v.as_array())
        .ok_or("Missing 'rows' parameter; expected an array of arrays")?;

    let path = resolve_path(path_str, project_path)?;
    if let Some(parent) = path.parent() {
        if !parent.exists() {
            fs::create_dir_all(parent).map_err(|e| format!("Failed to create directories: {}", e))?;
        }
    }

    let mut workbook = Workbook::new();
    let worksheet = workbook
        .add_worksheet()
        .set_name(sheet_name)
        .map_err(|e| format!("Invalid sheet name: {}", e))?;

    let mut row_index: u32 = 0;

    if !headers.is_empty() {
        for (col_index, value) in headers.iter().enumerate() {
            worksheet
                .write_string(row_index, col_index as u16, value)
                .map_err(|e| format!("Failed writing header cell: {}", e))?;
        }
        row_index += 1;
    }

    for (ri, row) in rows.iter().enumerate() {
        let cells = row
            .as_array()
            .ok_or_else(|| format!("rows[{}] must be an array", ri))?;

        for (ci, cell) in cells.iter().enumerate() {
            let col = ci as u16;
            match cell {
                serde_json::Value::Null => {}
                serde_json::Value::Bool(v) => {
                    worksheet
                        .write_boolean(row_index, col, *v)
                        .map_err(|e| format!("Failed writing boolean cell: {}", e))?;
                }
                serde_json::Value::Number(v) => {
                    if let Some(num) = v.as_f64() {
                        worksheet
                            .write_number(row_index, col, num)
                            .map_err(|e| format!("Failed writing number cell: {}", e))?;
                    } else {
                        worksheet
                            .write_string(row_index, col, v.to_string())
                            .map_err(|e| format!("Failed writing numeric-string cell: {}", e))?;
                    }
                }
                serde_json::Value::String(v) => {
                    worksheet
                        .write_string(row_index, col, v)
                        .map_err(|e| format!("Failed writing string cell: {}", e))?;
                }
                other => {
                    worksheet
                        .write_string(row_index, col, other.to_string())
                        .map_err(|e| format!("Failed writing JSON cell: {}", e))?;
                }
            }
        }
        row_index += 1;
    }

    workbook
        .save(&path)
        .map_err(|e| format!("Failed to save XLSX file: {}", e))?;

    Ok(format!(
        "Successfully created XLSX file at {} with {} data row(s)",
        path.display(),
        rows.len()
    ))
}

fn resolve_path(path_str: &str, project_path: Option<&str>) -> Result<std::path::PathBuf, String> {
    let path = Path::new(path_str);
    path_utils::resolve_path(path, project_path)
}
