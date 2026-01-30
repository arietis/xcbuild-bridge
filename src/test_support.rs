use serde::Deserialize;
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use crate::exec::{CommandOutput, CommandSpec, Runner};

#[derive(Debug, Clone)]
pub enum ResultBundleSpec {
    Temp { dir: PathBuf, path: PathBuf },
    External { path: PathBuf },
}

impl ResultBundleSpec {
    pub fn path(&self) -> &Path {
        match self {
            ResultBundleSpec::Temp { path, .. } => path.as_path(),
            ResultBundleSpec::External { path } => path.as_path(),
        }
    }

    pub fn include_in_command(&self) -> bool {
        matches!(self, ResultBundleSpec::Temp { .. })
    }

    pub fn cleanup(&self) {
        if let ResultBundleSpec::Temp { dir, .. } = self {
            let _ = fs::remove_dir_all(dir);
        }
    }
}

pub fn resolve_result_bundle(extra_args: &[String]) -> Result<ResultBundleSpec, String> {
    if let Some(path) = find_result_bundle_path(extra_args) {
        return Ok(ResultBundleSpec::External { path });
    }

    let dir = create_temp_dir("xcbuild-test").map_err(|err| err.to_string())?;
    let path = dir.join("TestResults.xcresult");
    Ok(ResultBundleSpec::Temp { dir, path })
}

pub fn normalize_env(values: Option<HashMap<String, String>>) -> Option<HashMap<String, String>> {
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

pub fn prefix_test_runner_env(values: &HashMap<String, String>) -> HashMap<String, String> {
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

pub fn render_command_output(output: &CommandOutput) -> String {
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

pub fn parse_xcresult_bundle(path: &Path, runner: &impl Runner) -> Result<String, String> {
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
