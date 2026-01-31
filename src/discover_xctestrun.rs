use serde::Deserialize;
use std::cmp::Ordering;
use std::fs;
use std::path::{Path, PathBuf};
use std::time::{Duration, UNIX_EPOCH};

use crate::tools::ToolResponse;

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DiscoverXctestrunParamsInput {
    pub derived_data_path: Option<String>,
    pub scheme: Option<String>,
    pub test_plan: Option<String>,
}

pub fn execute_discover_xctestrun(params: DiscoverXctestrunParamsInput) -> ToolResponse {
    match discover_xctestrun_path(
        params.derived_data_path.as_deref(),
        params.scheme.as_deref(),
        params.test_plan.as_deref(),
    ) {
        Ok(path) => ToolResponse::text(path.to_string_lossy().to_string(), true),
        Err(err) => ToolResponse::error("Failed to discover xctestrun".to_string(), Some(err)),
    }
}

pub fn discover_xctestrun_path(
    derived_data_path: Option<&str>,
    scheme: Option<&str>,
    test_plan: Option<&str>,
) -> Result<PathBuf, String> {
    let root = match derived_data_path {
        Some(path) => PathBuf::from(path),
        None => default_derived_data_dir()?,
    };
    if !root.exists() {
        return Err(format!(
            "DerivedData path does not exist: {}",
            root.display()
        ));
    }

    let candidates = find_xctestrun_candidates(&root, scheme, test_plan)?;
    candidates
        .first()
        .map(|candidate| candidate.path.clone())
        .ok_or_else(|| format!("No .xctestrun found under {}", root.display()))
}

fn default_derived_data_dir() -> Result<PathBuf, String> {
    let home = std::env::var_os("HOME")
        .ok_or_else(|| "HOME environment variable is not set".to_string())?;
    Ok(PathBuf::from(home).join("Library/Developer/Xcode/DerivedData"))
}

#[derive(Debug)]
struct Candidate {
    path: PathBuf,
    score: u32,
    modified: Duration,
}

fn find_xctestrun_candidates(
    root: &Path,
    scheme: Option<&str>,
    test_plan: Option<&str>,
) -> Result<Vec<Candidate>, String> {
    let mut results = Vec::new();
    let mut stack = vec![root.to_path_buf()];
    let scheme_lower = scheme.map(|value| value.to_lowercase());
    let plan_lower = test_plan.map(|value| value.to_lowercase());

    while let Some(path) = stack.pop() {
        let entries = fs::read_dir(&path).map_err(|err| err.to_string())?;
        for entry in entries {
            let entry = entry.map_err(|err| err.to_string())?;
            let entry_path = entry.path();
            let metadata = entry.metadata().map_err(|err| err.to_string())?;
            if metadata.is_dir() {
                stack.push(entry_path);
                continue;
            }

            if entry_path.extension().and_then(|ext| ext.to_str()) != Some("xctestrun") {
                continue;
            }

            let name = entry_path
                .file_name()
                .and_then(|value| value.to_str())
                .unwrap_or_default()
                .to_lowercase();
            let full_path = entry_path.to_string_lossy().to_lowercase();
            let score = score_match(
                &name,
                &full_path,
                scheme_lower.as_deref(),
                plan_lower.as_deref(),
            );
            let modified = metadata
                .modified()
                .ok()
                .and_then(|value| value.duration_since(UNIX_EPOCH).ok())
                .unwrap_or_else(|| Duration::from_secs(0));

            results.push(Candidate {
                path: entry_path,
                score,
                modified,
            });
        }
    }

    results.sort_by(compare_candidates);
    Ok(results)
}

fn score_match(name: &str, full_path: &str, scheme: Option<&str>, test_plan: Option<&str>) -> u32 {
    let mut score = 0;
    if let Some(scheme) = scheme
        && (name.contains(scheme) || full_path.contains(scheme))
    {
        score += 2;
    }
    if let Some(plan) = test_plan
        && (name.contains(plan) || full_path.contains(plan))
    {
        score += 1;
    }
    score
}

fn compare_candidates(a: &Candidate, b: &Candidate) -> Ordering {
    match b.score.cmp(&a.score) {
        Ordering::Equal => b.modified.cmp(&a.modified),
        ordering => ordering,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs::{File, create_dir_all};
    use std::thread;
    use std::time::{Duration as StdDuration, SystemTime, UNIX_EPOCH};

    fn create_temp_dir(prefix: &str) -> PathBuf {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos();
        let dir = std::env::temp_dir().join(format!("{}-{}", prefix, nanos));
        create_dir_all(&dir).expect("create temp dir");
        dir
    }

    #[test]
    fn discover_prefers_scheme_match() {
        let root = create_temp_dir("xctestrun-test");
        let a = root.join("OtherTests.xctestrun");
        let b = root.join("AppTests.xctestrun");
        File::create(&a).expect("create file a");
        thread::sleep(StdDuration::from_millis(5));
        File::create(&b).expect("create file b");

        let found = discover_xctestrun_path(Some(root.to_str().unwrap()), Some("App"), None)
            .expect("should discover");
        assert_eq!(found.file_name().unwrap(), b.file_name().unwrap());
    }
}
