use serde::Deserialize;
use std::fs;
use std::path::{Component, Path, PathBuf};

use crate::tools::ToolResponse;

const DEFAULT_MAX_DEPTH: usize = 5;
const SKIPPED_DIRS: [&str; 5] = ["build", "DerivedData", "Pods", ".git", "node_modules"];

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DiscoverProjsParamsInput {
    pub workspace_root: Option<String>,
    pub scan_path: Option<String>,
    pub max_depth: Option<u32>,
}

#[derive(Debug, Default)]
struct DiscoverResults {
    projects: Vec<String>,
    workspaces: Vec<String>,
}

pub fn execute_discover_projs(params: DiscoverProjsParamsInput) -> ToolResponse {
    let workspace_root = match normalize_opt(params.workspace_root) {
        Some(value) => value,
        None => {
            return ToolResponse::error(
                "Parameter validation failed".to_string(),
                Some("workspaceRoot is required".to_string()),
            )
        }
    };

    let scan_path = normalize_opt(params.scan_path).unwrap_or_else(|| ".".to_string());
    let max_depth = params
        .max_depth
        .map(|value| value as usize)
        .unwrap_or(DEFAULT_MAX_DEPTH);

    let workspace_root_path = PathBuf::from(&workspace_root);
    let workspace_root_norm = normalize_path(&workspace_root_path);

    if let Err(err) = ensure_directory(&workspace_root_norm) {
        return ToolResponse::error(
            "Failed to access workspace root".to_string(),
            Some(err),
        );
    }

    let requested_scan_path = workspace_root_path.join(&scan_path);
    let mut scan_path_norm = normalize_path(&requested_scan_path);
    if !scan_path_norm.starts_with(&workspace_root_norm) {
        scan_path_norm = workspace_root_norm.clone();
    }

    if let Err(err) = ensure_directory(&scan_path_norm) {
        return ToolResponse::error(
            format!(
                "Failed to access scan path: {}",
                scan_path_norm.to_string_lossy()
            ),
            Some(err),
        );
    }

    let mut results = DiscoverResults::default();
    find_projects_recursive(
        &scan_path_norm,
        &workspace_root_norm,
        0,
        max_depth,
        &mut results,
    );

    results.projects.sort();
    results.workspaces.sort();

    let mut lines = Vec::new();
    lines.push(format!(
        "Discovery finished. Found {} projects and {} workspaces.",
        results.projects.len(),
        results.workspaces.len()
    ));

    if !results.projects.is_empty() {
        lines.push(String::new());
        lines.push(format!(
            "Projects found:\n - {}",
            results.projects.join("\n - ")
        ));
    }

    if !results.workspaces.is_empty() {
        lines.push(String::new());
        lines.push(format!(
            "Workspaces found:\n - {}",
            results.workspaces.join("\n - ")
        ));
    }

    if !results.projects.is_empty() || !results.workspaces.is_empty() {
        lines.push(String::new());
        lines.push(
            "Hint: Save a default with session-set-defaults { projectPath: '...' } or { workspacePath: '...' }."
                .to_string(),
        );
    }

    ToolResponse::text(lines.join("\n"), true)
}

fn find_projects_recursive(
    current_dir: &Path,
    workspace_root: &Path,
    depth: usize,
    max_depth: usize,
    results: &mut DiscoverResults,
) {
    if depth >= max_depth {
        return;
    }

    let entries = match fs::read_dir(current_dir) {
        Ok(entries) => entries,
        Err(_) => return,
    };

    for entry in entries.flatten() {
        let file_type = match entry.file_type() {
            Ok(file_type) => file_type,
            Err(_) => continue,
        };

        if file_type.is_symlink() {
            continue;
        }

        let entry_path = entry.path();
        if !entry_path.starts_with(workspace_root) {
            continue;
        }

        if file_type.is_dir() {
            let name = entry.file_name();
            let name = name.to_string_lossy();

            if SKIPPED_DIRS.contains(&name.as_ref()) {
                continue;
            }

            if name.ends_with(".xcodeproj") {
                results.projects.push(entry_path.to_string_lossy().to_string());
                continue;
            }

            if name.ends_with(".xcworkspace") {
                results
                    .workspaces
                    .push(entry_path.to_string_lossy().to_string());
                continue;
            }

            find_projects_recursive(
                &entry_path,
                workspace_root,
                depth + 1,
                max_depth,
                results,
            );
        }
    }
}

fn ensure_directory(path: &Path) -> Result<(), String> {
    let metadata = fs::metadata(path).map_err(|err| err.to_string())?;
    if metadata.is_dir() {
        Ok(())
    } else {
        Err("Path is not a directory".to_string())
    }
}

fn normalize_opt(value: Option<String>) -> Option<String> {
    match value {
        Some(value) => {
            let trimmed = value.trim().to_string();
            if trimmed.is_empty() {
                None
            } else {
                Some(trimmed)
            }
        }
        None => None,
    }
}

fn normalize_path(path: &Path) -> PathBuf {
    let mut normalized = PathBuf::new();
    for component in path.components() {
        match component {
            Component::Prefix(prefix) => normalized.push(prefix.as_os_str()),
            Component::RootDir => normalized.push(Path::new("/")),
            Component::CurDir => {}
            Component::ParentDir => {
                normalized.pop();
            }
            Component::Normal(part) => normalized.push(part),
        }
    }
    if normalized.as_os_str().is_empty() {
        PathBuf::from(".")
    } else {
        normalized
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs::{self, File};
    use std::time::{SystemTime, UNIX_EPOCH};

    fn create_temp_dir(prefix: &str) -> PathBuf {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos();
        let dir = std::env::temp_dir().join(format!("{}-{}", prefix, nanos));
        fs::create_dir_all(&dir).expect("create temp dir");
        dir
    }

    #[test]
    fn discover_projects_skips_known_dirs() {
        let root = create_temp_dir("xcbuild-discovery");
        let project_dir = root.join("App.xcodeproj");
        let workspace_dir = root.join("App.xcworkspace");
        let nested_dir = root.join("Nested");
        let nested_project = nested_dir.join("Nested.xcodeproj");
        let skipped_dir = root.join("node_modules");
        let skipped_project = skipped_dir.join("Skip.xcodeproj");

        fs::create_dir_all(&project_dir).unwrap();
        fs::create_dir_all(&workspace_dir).unwrap();
        fs::create_dir_all(&nested_project).unwrap();
        fs::create_dir_all(&skipped_project).unwrap();
        File::create(root.join("Nested").join("dummy.txt")).unwrap();

        let mut results = DiscoverResults::default();
        find_projects_recursive(&root, &root, 0, 5, &mut results);

        assert!(results.projects.iter().any(|p| p.ends_with("App.xcodeproj")));
        assert!(results
            .projects
            .iter()
            .any(|p| p.ends_with("Nested.xcodeproj")));
        assert!(results
            .workspaces
            .iter()
            .any(|p| p.ends_with("App.xcworkspace")));
        assert!(!results
            .projects
            .iter()
            .any(|p| p.ends_with("Skip.xcodeproj")));

        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn discover_projects_respects_depth() {
        let root = create_temp_dir("xcbuild-discovery-depth");
        let nested_dir = root.join("Nested");
        let nested_project = nested_dir.join("Nested.xcodeproj");
        fs::create_dir_all(&nested_project).unwrap();

        let mut results = DiscoverResults::default();
        find_projects_recursive(&root, &root, 0, 1, &mut results);

        assert!(results.projects.is_empty());

        let _ = fs::remove_dir_all(&root);
    }
}
