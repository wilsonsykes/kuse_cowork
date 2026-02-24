use std::path::{Path, PathBuf};

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
        std::env::current_dir()
            .map(|cwd| cwd.join(path))
            .map_err(|e| format!("Failed to get current directory: {}", e))
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
