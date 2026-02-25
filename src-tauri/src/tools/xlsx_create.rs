use crate::agent::ToolDefinition;
use crate::tools::path_utils;
use regex::Regex;
use rust_xlsxwriter::Workbook;
use serde_json::json;
use std::fs;
use std::io::Read;
use std::path::Path;
use zip::ZipArchive;

pub fn definition() -> ToolDefinition {
    ToolDefinition {
        name: "create_xlsx_file".to_string(),
        description: "Create simple or complex .xlsx workbooks in one call (multi-sheet, formulas, widths, freeze panes, filters, row heights).".to_string(),
        input_schema: json!({
            "type": "object",
            "properties": {
                "path": {
                    "type": "string",
                    "description": "Output file path ending with .xlsx"
                },
                "workbook": {
                    "type": "object",
                    "description": "Advanced workbook payload. If present, overrides top-level sheet_name/headers/rows.",
                    "properties": {
                        "sheets": {
                            "type": "array",
                            "items": {
                                "type": "object",
                                "properties": {
                                    "name": { "type": "string" },
                                    "headers": { "type": "array", "items": { "type": "string" } },
                                    "rows": {
                                        "type": "array",
                                        "items": {
                                            "type": "array",
                                            "items": {
                                                "anyOf": [
                                                    { "type": "string" },
                                                    { "type": "number" },
                                                    { "type": "boolean" },
                                                    { "type": "null" },
                                                    {
                                                        "type": "object",
                                                        "properties": {
                                                            "value": {
                                                                "anyOf": [
                                                                    { "type": "string" },
                                                                    { "type": "number" },
                                                                    { "type": "boolean" },
                                                                    { "type": "null" }
                                                                ]
                                                            },
                                                            "formula": { "type": "string" }
                                                        },
                                                        "additionalProperties": true
                                                    }
                                                ]
                                            }
                                        }
                                    },
                                    "column_widths": { "type": "array", "items": { "type": "number" } },
                                    "row_heights": { "type": "array", "items": { "type": "number" } },
                                    "freeze_panes": {
                                        "type": "object",
                                        "properties": {
                                            "row": { "type": "integer" },
                                            "col": { "type": "integer" }
                                        }
                                    },
                                    "autofilter": {
                                        "type": "object",
                                        "properties": {
                                            "from_row": { "type": "integer" },
                                            "from_col": { "type": "integer" },
                                            "to_row": { "type": "integer" },
                                            "to_col": { "type": "integer" }
                                        }
                                    }
                                },
                                "required": ["name", "rows"]
                            }
                        }
                    },
                    "required": ["sheets"]
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
                },
                "strict": {
                    "type": "boolean",
                    "description": "If true (default), verify workbook structure after write and fail on mismatch."
                }
            },
            "required": ["path"],
            "additionalProperties": false
        }),
    }
}

pub fn execute(input: &serde_json::Value, project_path: Option<&str>) -> Result<String, String> {
    let strict = input.get("strict").and_then(|v| v.as_bool()).unwrap_or(true);
    let path_str = input
        .get("path")
        .and_then(|v| v.as_str())
        .ok_or("Missing 'path' parameter")?;

    if !path_str.to_lowercase().ends_with(".xlsx") {
        return Err("Path must end with .xlsx".to_string());
    }

    let path = resolve_path(path_str, project_path)?;
    if let Some(parent) = path.parent() {
        if !parent.exists() {
            fs::create_dir_all(parent).map_err(|e| format!("Failed to create directories: {}", e))?;
        }
    }

    let mut workbook = Workbook::new();
    let used_complex_payload = input.get("workbook").is_some();
    if let Some(workbook_payload) = input.get("workbook") {
        write_complex_workbook(&mut workbook, workbook_payload)?;
    } else {
        write_simple_sheet(&mut workbook, input)?;
    }

    workbook
        .save(&path)
        .map_err(|e| format!("Failed to save XLSX file: {}", e))?;

    if strict {
        verify_written_workbook(&path, input, used_complex_payload)?;
    }

    Ok(format!(
        "Successfully created XLSX file at {} (verified)",
        path.display()
    ))
}

fn resolve_path(path_str: &str, project_path: Option<&str>) -> Result<std::path::PathBuf, String> {
    let path = Path::new(path_str);
    path_utils::resolve_path_for_write(path, project_path)
}

fn write_simple_sheet(workbook: &mut Workbook, input: &serde_json::Value) -> Result<(), String> {
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
        .ok_or("Missing 'rows' parameter; expected an array of arrays. For complex workbook creation, use 'workbook.sheets'.")?;

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
            write_cell(worksheet, row_index, ci as u16, cell)?;
        }
        row_index += 1;
    }

    Ok(())
}

fn verify_written_workbook(
    path: &Path,
    input: &serde_json::Value,
    used_complex_payload: bool,
) -> Result<(), String> {
    let file = fs::File::open(path)
        .map_err(|e| format!("Failed to reopen written XLSX for verification: {}", e))?;
    let mut archive = ZipArchive::new(file)
        .map_err(|e| format!("Written file is not a valid XLSX/ZIP archive: {}", e))?;

    let workbook_xml = read_zip_entry_string(&mut archive, "xl/workbook.xml")?;
    let workbook_rels = read_zip_entry_string(&mut archive, "xl/_rels/workbook.xml.rels")?;

    let sheets = parse_sheets_and_targets(&workbook_xml, &workbook_rels)?;
    if sheets.is_empty() {
        return Err("Workbook verification failed: no worksheets found".to_string());
    }

    if used_complex_payload {
        verify_complex_payload(&mut archive, input, &sheets)?;
    } else {
        verify_simple_payload(&mut archive, input, &sheets)?;
    }

    Ok(())
}

fn read_zip_entry_string<R: Read + std::io::Seek>(
    archive: &mut ZipArchive<R>,
    name: &str,
) -> Result<String, String> {
    let mut file = archive
        .by_name(name)
        .map_err(|e| format!("Missing '{}' in XLSX: {}", name, e))?;
    let mut out = String::new();
    file.read_to_string(&mut out)
        .map_err(|e| format!("Failed reading '{}' in XLSX: {}", name, e))?;
    Ok(out)
}

fn parse_sheets_and_targets(
    workbook_xml: &str,
    workbook_rels_xml: &str,
) -> Result<Vec<(String, String)>, String> {
    let sheet_re = Regex::new(r#"<sheet\b[^>]*\bname="([^"]+)"[^>]*\br:id="([^"]+)""#)
        .map_err(|e| format!("Regex error: {}", e))?;
    let rel_re = Regex::new(r#"<Relationship\b[^>]*\bId="([^"]+)"[^>]*\bTarget="([^"]+)""#)
        .map_err(|e| format!("Regex error: {}", e))?;

    let mut rel_map = std::collections::HashMap::new();
    for caps in rel_re.captures_iter(workbook_rels_xml) {
        rel_map.insert(caps[1].to_string(), caps[2].to_string());
    }

    let mut out = Vec::new();
    for caps in sheet_re.captures_iter(workbook_xml) {
        let name = caps[1].to_string();
        let rid = caps[2].to_string();
        if let Some(target) = rel_map.get(&rid) {
            let clean = target.trim_start_matches('/');
            let target_path = if clean.starts_with("xl/") {
                clean.to_string()
            } else {
                format!("xl/{}", clean)
            };
            out.push((name, target_path));
        }
    }
    Ok(out)
}

fn count_xml_rows(sheet_xml: &str) -> usize {
    sheet_xml.matches("<row ").count() + sheet_xml.matches("<row>").count()
}

fn count_xml_formulas(sheet_xml: &str) -> usize {
    sheet_xml.matches("<f>").count() + sheet_xml.matches("<f ").count()
}

fn verify_simple_payload<R: Read + std::io::Seek>(
    archive: &mut ZipArchive<R>,
    input: &serde_json::Value,
    sheets: &[(String, String)],
) -> Result<(), String> {
    let expected_name = input
        .get("sheet_name")
        .and_then(|v| v.as_str())
        .unwrap_or("Sheet1");
    let (_, target) = sheets
        .iter()
        .find(|(name, _)| name == expected_name)
        .ok_or_else(|| format!("Workbook verification failed: sheet '{}' not found", expected_name))?;
    let sheet_xml = read_zip_entry_string(archive, target)?;

    let rows_len = input
        .get("rows")
        .and_then(|v| v.as_array())
        .map(|a| a.len())
        .unwrap_or(0);
    let has_headers = input
        .get("headers")
        .and_then(|v| v.as_array())
        .map(|a| !a.is_empty())
        .unwrap_or(false);
    let expected_rows = rows_len + usize::from(has_headers);
    let actual_rows = count_xml_rows(&sheet_xml);
    if actual_rows < expected_rows {
        return Err(format!(
            "Workbook verification failed: expected at least {} row(s) in '{}', found {}",
            expected_rows, expected_name, actual_rows
        ));
    }
    Ok(())
}

fn count_expected_formulas(rows: &[serde_json::Value]) -> usize {
    let mut count = 0usize;
    for row in rows {
        if let Some(cells) = row.as_array() {
            for cell in cells {
                if cell
                    .as_object()
                    .and_then(|o| o.get("formula"))
                    .and_then(|f| f.as_str())
                    .is_some()
                {
                    count += 1;
                }
            }
        }
    }
    count
}

fn verify_complex_payload<R: Read + std::io::Seek>(
    archive: &mut ZipArchive<R>,
    input: &serde_json::Value,
    sheets: &[(String, String)],
) -> Result<(), String> {
    let expected_sheets = input
        .get("workbook")
        .and_then(|w| w.get("sheets"))
        .and_then(|v| v.as_array())
        .ok_or("Workbook verification failed: workbook.sheets missing")?;

    for expected in expected_sheets {
        let name = expected
            .get("name")
            .and_then(|v| v.as_str())
            .ok_or("Workbook verification failed: sheet missing name")?;

        let (_, target) = sheets
            .iter()
            .find(|(actual_name, _)| actual_name == name)
            .ok_or_else(|| format!("Workbook verification failed: sheet '{}' not found", name))?;

        let sheet_xml = read_zip_entry_string(archive, target)?;

        let rows = expected
            .get("rows")
            .and_then(|v| v.as_array())
            .ok_or_else(|| format!("Workbook verification failed: rows missing for '{}'", name))?;
        let has_headers = expected
            .get("headers")
            .and_then(|v| v.as_array())
            .map(|a| !a.is_empty())
            .unwrap_or(false);
        let expected_rows = rows.len() + usize::from(has_headers);
        let actual_rows = count_xml_rows(&sheet_xml);
        if actual_rows < expected_rows {
            return Err(format!(
                "Workbook verification failed for '{}': expected at least {} row(s), found {}",
                name, expected_rows, actual_rows
            ));
        }

        let expected_formulas = count_expected_formulas(rows);
        if expected_formulas > 0 {
            let actual_formulas = count_xml_formulas(&sheet_xml);
            if actual_formulas < expected_formulas {
                return Err(format!(
                    "Workbook verification failed for '{}': expected at least {} formula cell(s), found {}",
                    name, expected_formulas, actual_formulas
                ));
            }
        }

        if expected.get("freeze_panes").is_some() && !sheet_xml.contains("<pane") {
            return Err(format!(
                "Workbook verification failed for '{}': freeze panes were requested but not found",
                name
            ));
        }

        if expected.get("autofilter").is_some() && !sheet_xml.contains("<autoFilter") {
            return Err(format!(
                "Workbook verification failed for '{}': auto filter was requested but not found",
                name
            ));
        }
    }

    Ok(())
}

fn write_complex_workbook(workbook: &mut Workbook, payload: &serde_json::Value) -> Result<(), String> {
    let sheets = payload
        .get("sheets")
        .and_then(|v| v.as_array())
        .ok_or("workbook.sheets must be an array")?;

    if sheets.is_empty() {
        return Err("workbook.sheets cannot be empty".to_string());
    }

    for (si, sheet) in sheets.iter().enumerate() {
        let name = sheet
            .get("name")
            .and_then(|v| v.as_str())
            .ok_or_else(|| format!("workbook.sheets[{}].name is required", si))?;

        let rows = sheet
            .get("rows")
            .and_then(|v| v.as_array())
            .ok_or_else(|| format!("workbook.sheets[{}].rows is required", si))?;

        let headers: Vec<String> = sheet
            .get("headers")
            .and_then(|v| v.as_array())
            .map(|arr| {
                arr.iter()
                    .map(|v| v.as_str().unwrap_or_default().to_string())
                    .collect()
            })
            .unwrap_or_default();

        let worksheet = workbook
            .add_worksheet()
            .set_name(name)
            .map_err(|e| format!("Invalid sheet name '{}': {}", name, e))?;

        if let Some(widths) = sheet.get("column_widths").and_then(|v| v.as_array()) {
            for (col, width) in widths.iter().enumerate() {
                if let Some(w) = width.as_f64() {
                    worksheet
                        .set_column_width(col as u16, w)
                        .map_err(|e| format!("Failed setting column width on '{}': {}", name, e))?;
                }
            }
        }

        if let Some(heights) = sheet.get("row_heights").and_then(|v| v.as_array()) {
            for (row, height) in heights.iter().enumerate() {
                if let Some(h) = height.as_f64() {
                    worksheet
                        .set_row_height(row as u32, h)
                        .map_err(|e| format!("Failed setting row height on '{}': {}", name, e))?;
                }
            }
        }

        if let Some(freeze) = sheet.get("freeze_panes").and_then(|v| v.as_object()) {
            let row = freeze.get("row").and_then(|v| v.as_u64()).unwrap_or(0) as u32;
            let col = freeze.get("col").and_then(|v| v.as_u64()).unwrap_or(0) as u16;
            worksheet
                .set_freeze_panes(row, col)
                .map_err(|e| format!("Failed setting freeze panes on '{}': {}", name, e))?;
        }

        let mut row_index: u32 = 0;
        if !headers.is_empty() {
            for (col_index, value) in headers.iter().enumerate() {
                worksheet
                    .write_string(row_index, col_index as u16, value)
                    .map_err(|e| format!("Failed writing header cell on '{}': {}", name, e))?;
            }
            row_index += 1;
        }

        for (ri, row) in rows.iter().enumerate() {
            let cells = row
                .as_array()
                .ok_or_else(|| format!("workbook.sheets[{}].rows[{}] must be an array", si, ri))?;
            for (ci, cell) in cells.iter().enumerate() {
                write_cell(worksheet, row_index, ci as u16, cell)?;
            }
            row_index += 1;
        }

        if let Some(filter) = sheet.get("autofilter").and_then(|v| v.as_object()) {
            let from_row = filter.get("from_row").and_then(|v| v.as_u64()).unwrap_or(0) as u32;
            let from_col = filter.get("from_col").and_then(|v| v.as_u64()).unwrap_or(0) as u16;
            let to_row = filter
                .get("to_row")
                .and_then(|v| v.as_u64())
                .unwrap_or(row_index.saturating_sub(1) as u64) as u32;
            let to_col = filter
                .get("to_col")
                .and_then(|v| v.as_u64())
                .unwrap_or(0) as u16;
            worksheet
                .autofilter(from_row, from_col, to_row, to_col)
                .map_err(|e| format!("Failed setting autofilter on '{}': {}", name, e))?;
        }
    }

    Ok(())
}

fn write_cell(
    worksheet: &mut rust_xlsxwriter::Worksheet,
    row: u32,
    col: u16,
    cell: &serde_json::Value,
) -> Result<(), String> {
    if let Some(obj) = cell.as_object() {
        if let Some(formula) = obj.get("formula").and_then(|v| v.as_str()) {
            let fx = if formula.starts_with('=') {
                formula.to_string()
            } else {
                format!("={}", formula)
            };
            worksheet
                .write_formula(row, col, fx.as_str())
                .map(|_| ())
                .map_err(|e| format!("Failed writing formula cell: {}", e))?;
            return Ok(());
        }
        if let Some(value) = obj.get("value") {
            return write_cell(worksheet, row, col, value);
        }
    }

    match cell {
        serde_json::Value::Null => Ok(()),
        serde_json::Value::Bool(v) => worksheet
            .write_boolean(row, col, *v)
            .map(|_| ())
            .map_err(|e| format!("Failed writing boolean cell: {}", e)),
        serde_json::Value::Number(v) => {
            if let Some(num) = v.as_f64() {
                worksheet
                    .write_number(row, col, num)
                    .map(|_| ())
                    .map_err(|e| format!("Failed writing number cell: {}", e))
            } else {
                worksheet
                    .write_string(row, col, v.to_string())
                    .map(|_| ())
                    .map_err(|e| format!("Failed writing numeric-string cell: {}", e))
            }
        }
        serde_json::Value::String(v) => worksheet
            .write_string(row, col, v)
            .map(|_| ())
            .map_err(|e| format!("Failed writing string cell: {}", e)),
        other => worksheet
            .write_string(row, col, other.to_string())
            .map(|_| ())
            .map_err(|e| format!("Failed writing JSON cell: {}", e)),
    }
}
