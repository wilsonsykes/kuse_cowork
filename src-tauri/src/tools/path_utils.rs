use std::path::{Path, PathBuf};

pub fn default_local_workspace_root() -> Result<PathBuf, String> {
    // Preferred: writable "workspace" folder beside the running executable.
    if let Ok(exe_path) = std::env::current_exe() {
        if let Some(exe_dir) = exe_path.parent() {
            let candidate = exe_dir.join("workspace");
            if std::fs::create_dir_all(&candidate).is_ok() {
                return Ok(candidate);
            }
        }
    }

    // Fallback: per-user app data workspace.
    if let Some(data_dir) = dirs::data_dir() {
        let candidate = data_dir.join("kuse-cowork").join("workspace");
        std::fs::create_dir_all(&candidate)
            .map_err(|e| format!("Failed to create fallback workspace directory: {}", e))?;
        return Ok(candidate);
    }

    Err("Failed to determine a default local workspace directory".to_string())
}

pub fn parse_project_roots(project_path: Option<&str>) -> Vec<PathBuf> {
    project_path
        .unwrap_or("")
        .split(',')
        .map(|p| p.trim())
        .filter(|p| !p.is_empty())
        .map(PathBuf::from)
        .collect()
}

pub fn resolve_path(path: &Path, project_path: Option<&str>) -> Result<PathBuf, String> {
    let roots = parse_project_roots(project_path);

    if path.is_absolute() {
        if !roots.is_empty() && !is_within_roots(path, &roots) {
            if let Some(rewritten) = rewrite_outside_absolute_path(path, &roots) {
                return Ok(rewritten);
            }
            return Err(format!(
                "Path is outside mounted folder(s): {}. Allowed roots: {}",
                path.display(),
                format_roots(&roots)
            ));
        }
        return Ok(path.to_path_buf());
    }

    if let Some(root) = roots.first() {
        Ok(root.join(path))
    } else {
        default_local_workspace_root().map(|root| root.join(path))
    }
}

pub fn resolve_path_for_write(path: &Path, project_path: Option<&str>) -> Result<PathBuf, String> {
    let roots = parse_project_roots(project_path);

    if path.is_absolute() {
        if !roots.is_empty() && !is_within_roots(path, &roots) {
            return Err(format!(
                "Write path is outside mounted folder(s): {}. Allowed roots: {}",
                path.display(),
                format_roots(&roots)
            ));
        }
        return Ok(path.to_path_buf());
    }

    if let Some(root) = roots.first() {
        Ok(root.join(path))
    } else {
        default_local_workspace_root().map(|root| root.join(path))
    }
}

fn is_within_roots(path: &Path, roots: &[PathBuf]) -> bool {
    roots.iter().any(|root| path.starts_with(root))
}

fn format_roots(roots: &[PathBuf]) -> String {
    roots
        .iter()
        .map(|p| p.display().to_string())
        .collect::<Vec<_>>()
        .join(", ")
}

fn rewrite_outside_absolute_path(path: &Path, roots: &[PathBuf]) -> Option<PathBuf> {
    let file_name = path.file_name()?;

    // If one mounted root is configured, map by filename to that root.
    if roots.len() == 1 {
        return Some(roots[0].join(file_name));
    }

    // For multiple roots, prefer one where the filename already exists.
    for root in roots {
        let candidate = root.join(file_name);
        if candidate.exists() {
            return Some(candidate);
        }
    }

    None
}
