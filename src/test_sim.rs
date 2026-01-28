use serde::Deserialize;
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use crate::exec::{CommandSpec, Runner};
use crate::session::SessionDefaults;
use crate::tools::ToolResponse;

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TestSimParamsInput {
    pub project_path: Option<String>,
    pub workspace_path: Option<String>,
    pub scheme: Option<String>,
    pub configuration: Option<String>,
    pub simulator_id: Option<String>,
    pub simulator_name: Option<String>,
    pub derived_data_path: Option<String>,
    pub extra_args: Option<Vec<String>>,
    pub use_latest_os: Option<bool>,
    pub only_testing: Option<Vec<String>>,
    pub skip_testing: Option<Vec<String>>,
    pub test_runner_env: Option<HashMap<String, String>>,
}

#[derive(Debug, Clone)]
pub struct TestSimParams {
    pub project_path: Option<String>,
    pub workspace_path: Option<String>,
    pub scheme: String,
    pub configuration: String,
    pub simulator_id: Option<String>,
    pub simulator_name: Option<String>,
    pub derived_data_path: Option<String>,
    pub extra_args: Vec<String>,
    pub use_latest_os: bool,
    pub only_testing: Vec<String>,
    pub skip_testing: Vec<String>,
    pub test_runner_env: Option<HashMap<String, String>>,
}

pub fn test_sim_from_input(
    input: TestSimParamsInput,
    defaults: &SessionDefaults,
) -> Result<TestSimParams, String> {
    let project_path = normalize_opt(input.project_path).or_else(|| defaults.project_path.clone());
    let workspace_path =
        normalize_opt(input.workspace_path).or_else(|| defaults.workspace_path.clone());
    let scheme = normalize_opt(input.scheme).or_else(|| defaults.scheme.clone());
    let configuration = normalize_opt(input.configuration)
        .or_else(|| defaults.configuration.clone())
        .unwrap_or_else(|| "Debug".to_string());
    let simulator_id = normalize_opt(input.simulator_id).or_else(|| defaults.simulator_id.clone());
    let simulator_name =
        normalize_opt(input.simulator_name).or_else(|| defaults.simulator_name.clone());
    let derived_data_path = normalize_opt(input.derived_data_path);
    let extra_args = normalize_vec(input.extra_args);
    let use_latest_os = input
        .use_latest_os
        .or(defaults.use_latest_os)
        .unwrap_or(true);
    let only_testing = normalize_vec(input.only_testing);
    let skip_testing = normalize_vec(input.skip_testing);
    let test_runner_env = normalize_env(input.test_runner_env);

    let scheme = scheme.ok_or_else(|| "scheme is required".to_string())?;

    if project_path.is_none() && workspace_path.is_none() {
        return Err("Either projectPath or workspacePath is required.".to_string());
    }
    if project_path.is_some() && workspace_path.is_some() {
        return Err("projectPath and workspacePath are mutually exclusive.".to_string());
    }
    if simulator_id.is_none() && simulator_name.is_none() {
        return Err("Either simulatorId or simulatorName is required.".to_string());
    }
    if simulator_id.is_some() && simulator_name.is_some() {
        return Err("simulatorId and simulatorName are mutually exclusive.".to_string());
    }

    Ok(TestSimParams {
        project_path,
        workspace_path,
        scheme,
        configuration,
        simulator_id,
        simulator_name,
        derived_data_path,
        extra_args,
        use_latest_os,
        only_testing,
        skip_testing,
        test_runner_env,
    })
}

pub fn test_sim_command(params: &TestSimParams) -> CommandSpec {
    build_test_sim_command(params, None, false)
}

fn build_test_sim_command(
    params: &TestSimParams,
    result_bundle_path: Option<&Path>,
    include_result_bundle: bool,
) -> CommandSpec {
    let mut args = Vec::new();

    if let Some(workspace) = &params.workspace_path {
        args.push("-workspace".to_string());
        args.push(workspace.clone());
    } else if let Some(project) = &params.project_path {
        args.push("-project".to_string());
        args.push(project.clone());
    }

    args.push("-scheme".to_string());
    args.push(params.scheme.clone());

    args.push("-configuration".to_string());
    args.push(params.configuration.clone());

    let destination = if let Some(simulator_id) = &params.simulator_id {
        format!("id={}", simulator_id)
    } else {
        let name = params
            .simulator_name
            .clone()
            .unwrap_or_else(|| "iPhone".to_string());
        if params.use_latest_os {
            format!("platform=iOS Simulator,name={},OS=latest", name)
        } else {
            format!("platform=iOS Simulator,name={}", name)
        }
    };

    args.push("-destination".to_string());
    args.push(destination);

    if let Some(derived_data_path) = &params.derived_data_path {
        args.push("-derivedDataPath".to_string());
        args.push(derived_data_path.clone());
    }

    if include_result_bundle && let Some(result_bundle_path) = result_bundle_path {
        args.push("-resultBundlePath".to_string());
        args.push(result_bundle_path.to_string_lossy().to_string());
    }

    for entry in &params.only_testing {
        args.push("-only-testing".to_string());
        args.push(entry.clone());
    }

    for entry in &params.skip_testing {
        args.push("-skip-testing".to_string());
        args.push(entry.clone());
    }

    args.extend(params.extra_args.clone());
    args.push("test".to_string());

    let env = params.test_runner_env.as_ref().map(prefix_test_runner_env);

    CommandSpec {
        program: "xcodebuild".to_string(),
        args,
        cwd: None,
        env,
    }
}

pub fn execute_test_sim(params: TestSimParams, runner: &impl Runner) -> ToolResponse {
    let result_bundle = match resolve_result_bundle(&params) {
        Ok(bundle) => bundle,
        Err(err) => {
            return ToolResponse::error(
                "Failed to prepare test result bundle".to_string(),
                Some(err),
            );
        }
    };

    let (result_bundle_path, include_result_bundle, temp_dir) = match &result_bundle {
        ResultBundleSpec::Temp { dir, path } => (Some(path.as_path()), true, Some(dir.clone())),
        ResultBundleSpec::External { path } => (Some(path.as_path()), false, None),
    };

    let spec = build_test_sim_command(&params, result_bundle_path, include_result_bundle);
    let output = match runner.run(&spec) {
        Ok(output) => output,
        Err(err) => {
            if let Some(dir) = temp_dir {
                let _ = fs::remove_dir_all(dir);
            }
            return ToolResponse::error("Command failed".to_string(), Some(err.to_string()));
        }
    };

    let mut text = render_command_output(&output);
    if let Some(path) = result_bundle_path
        && let Ok(summary) = parse_xcresult_bundle(path, runner)
    {
        text.push_str("\n\nTest Results Summary:\n");
        text.push_str(&summary);
    }

    if let Some(dir) = temp_dir {
        let _ = fs::remove_dir_all(dir);
    }

    ToolResponse::text(text, output.exit_code == 0)
}

fn normalize_opt(value: Option<String>) -> Option<String> {
    match value {
        Some(v) => {
            let trimmed = v.trim().to_string();
            if trimmed.is_empty() {
                None
            } else {
                Some(trimmed)
            }
        }
        None => None,
    }
}

fn normalize_vec(values: Option<Vec<String>>) -> Vec<String> {
    values
        .unwrap_or_default()
        .into_iter()
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
        .collect()
}

fn normalize_env(values: Option<HashMap<String, String>>) -> Option<HashMap<String, String>> {
    let mut normalized = HashMap::new();
    for (key, value) in values.unwrap_or_default() {
        let key = key.trim();
        let value = value.trim();
        if !key.is_empty() && !value.is_empty() {
            normalized.insert(key.to_string(), value.to_string());
        }
    }
    if normalized.is_empty() {
        None
    } else {
        Some(normalized)
    }
}

fn prefix_test_runner_env(values: &HashMap<String, String>) -> HashMap<String, String> {
    values
        .iter()
        .map(|(key, value)| {
            if key.starts_with("TEST_RUNNER_") {
                (key.clone(), value.clone())
            } else {
                (format!("TEST_RUNNER_{}", key), value.clone())
            }
        })
        .collect()
}

enum ResultBundleSpec {
    Temp { dir: PathBuf, path: PathBuf },
    External { path: PathBuf },
}

fn resolve_result_bundle(params: &TestSimParams) -> Result<ResultBundleSpec, String> {
    if let Some(path) = find_result_bundle_path(&params.extra_args) {
        return Ok(ResultBundleSpec::External { path });
    }

    let dir = create_temp_dir("xcbuild-test").map_err(|err| err.to_string())?;
    let path = dir.join("TestResults.xcresult");
    Ok(ResultBundleSpec::Temp { dir, path })
}

fn find_result_bundle_path(extra_args: &[String]) -> Option<PathBuf> {
    let mut iter = extra_args.iter();
    while let Some(arg) = iter.next() {
        if arg == "-resultBundlePath"
            && let Some(path) = iter.next()
        {
            return Some(PathBuf::from(path));
        }
    }
    None
}

fn create_temp_dir(prefix: &str) -> std::io::Result<PathBuf> {
    let pid = std::process::id();
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos();
    let dir = std::env::temp_dir().join(format!("{}-{}-{}", prefix, pid, nanos));
    fs::create_dir(&dir)?;
    Ok(dir)
}

fn render_command_output(output: &crate::exec::CommandOutput) -> String {
    let mut text = format!("xcodebuild exited with code {}", output.exit_code);
    if !output.stdout.is_empty() {
        text.push_str("\n\nSTDOUT:\n");
        text.push_str(&output.stdout);
    }
    if !output.stderr.is_empty() {
        text.push_str("\n\nSTDERR:\n");
        text.push_str(&output.stderr);
    }
    text
}

fn parse_xcresult_bundle(path: &Path, runner: &impl Runner) -> Result<String, String> {
    fs::metadata(path).map_err(|err| err.to_string())?;
    let spec = CommandSpec {
        program: "xcrun".to_string(),
        args: vec![
            "xcresulttool".to_string(),
            "get".to_string(),
            "test-results".to_string(),
            "summary".to_string(),
            "--path".to_string(),
            path.to_string_lossy().to_string(),
        ],
        cwd: None,
        env: None,
    };

    let output = runner.run(&spec).map_err(|err| err.to_string())?;
    if output.exit_code != 0 {
        return Err(format!(
            "xcresulttool exited with code {}",
            output.exit_code
        ));
    }

    let summary: TestSummary =
        serde_json::from_str(&output.stdout).map_err(|err| err.to_string())?;
    Ok(format_test_summary(&summary))
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct TestSummary {
    title: Option<String>,
    result: Option<String>,
    total_test_count: Option<u64>,
    passed_tests: Option<u64>,
    failed_tests: Option<u64>,
    skipped_tests: Option<u64>,
    expected_failures: Option<u64>,
    environment_description: Option<String>,
    devices_and_configurations: Option<Vec<TestDeviceConfig>>,
    test_failures: Option<Vec<TestFailure>>,
    top_insights: Option<Vec<TestInsight>>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct TestDeviceConfig {
    device: Option<TestDevice>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct TestDevice {
    device_name: Option<String>,
    platform: Option<String>,
    os_version: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct TestFailure {
    test_name: Option<String>,
    target_name: Option<String>,
    failure_text: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct TestInsight {
    impact: Option<String>,
    text: Option<String>,
}

fn format_test_summary(summary: &TestSummary) -> String {
    let mut lines = Vec::new();
    lines.push(format!(
        "Test Summary: {}",
        summary.title.as_deref().unwrap_or("Unknown")
    ));
    lines.push(format!(
        "Overall Result: {}",
        summary.result.as_deref().unwrap_or("Unknown")
    ));
    lines.push(String::new());
    lines.push("Test Counts:".to_string());
    lines.push(format!(
        "  Total: {}",
        summary.total_test_count.unwrap_or(0)
    ));
    lines.push(format!("  Passed: {}", summary.passed_tests.unwrap_or(0)));
    lines.push(format!("  Failed: {}", summary.failed_tests.unwrap_or(0)));
    lines.push(format!("  Skipped: {}", summary.skipped_tests.unwrap_or(0)));
    lines.push(format!(
        "  Expected Failures: {}",
        summary.expected_failures.unwrap_or(0)
    ));
    lines.push(String::new());

    if let Some(description) = &summary.environment_description {
        lines.push(format!("Environment: {}", description));
        lines.push(String::new());
    }

    if let Some(devices) = &summary.devices_and_configurations
        && let Some(device) = devices.iter().find_map(|entry| entry.device.as_ref())
    {
        lines.push(format!(
            "Device: {} ({} {})",
            device.device_name.as_deref().unwrap_or("Unknown"),
            device.platform.as_deref().unwrap_or("Unknown"),
            device.os_version.as_deref().unwrap_or("Unknown")
        ));
        lines.push(String::new());
    }

    if let Some(failures) = &summary.test_failures
        && !failures.is_empty()
    {
        lines.push("Test Failures:".to_string());
        for (index, failure) in failures.iter().enumerate() {
            lines.push(format!(
                "  {}. {} ({})",
                index + 1,
                failure.test_name.as_deref().unwrap_or("Unknown Test"),
                failure.target_name.as_deref().unwrap_or("Unknown Target")
            ));
            if let Some(text) = &failure.failure_text {
                lines.push(format!("     {}", text));
            }
        }
        lines.push(String::new());
    }

    if let Some(insights) = &summary.top_insights
        && !insights.is_empty()
    {
        lines.push("Insights:".to_string());
        for (index, insight) in insights.iter().enumerate() {
            lines.push(format!(
                "  {}. [{}] {}",
                index + 1,
                insight.impact.as_deref().unwrap_or("Unknown"),
                insight.text.as_deref().unwrap_or("No description")
            ));
        }
    }

    lines.join("\n")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn build_test_sim_command_includes_result_bundle_path() {
        let params = TestSimParams {
            project_path: Some("App.xcodeproj".to_string()),
            workspace_path: None,
            scheme: "App".to_string(),
            configuration: "Debug".to_string(),
            simulator_id: Some("SIM-UUID".to_string()),
            simulator_name: None,
            derived_data_path: None,
            extra_args: Vec::new(),
            use_latest_os: true,
            only_testing: Vec::new(),
            skip_testing: Vec::new(),
            test_runner_env: None,
        };

        let path = PathBuf::from("/tmp/TestResults.xcresult");
        let spec = build_test_sim_command(&params, Some(&path), true);
        assert!(spec.args.iter().any(|arg| arg == "-resultBundlePath"));
        assert!(
            spec.args
                .iter()
                .any(|arg| arg == "/tmp/TestResults.xcresult")
        );
    }
}
