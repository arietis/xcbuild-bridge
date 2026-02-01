use serde::Deserialize;
use std::fs;

use crate::exec::{CommandSpec, Runner};
use crate::tools::ToolResponse;

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GetAppBundleIdParamsInput {
    pub app_path: Option<String>,
}

#[derive(Debug, Clone)]
pub struct GetAppBundleIdParams {
    pub app_path: String,
}

pub fn get_app_bundle_id_from_input(
    input: GetAppBundleIdParamsInput,
) -> Result<GetAppBundleIdParams, String> {
    let app_path = normalize_opt(input.app_path).ok_or_else(|| "appPath is required".to_string())?;
    Ok(GetAppBundleIdParams { app_path })
}

pub fn execute_get_app_bundle_id(params: GetAppBundleIdParams, runner: &impl Runner) -> ToolResponse {
    if let Err(err) = validate_app_path(&params.app_path) {
        return ToolResponse::error("Invalid appPath".to_string(), Some(err));
    }

    let bundle_id = match read_bundle_id(&params.app_path, runner) {
        Ok(bundle_id) => bundle_id,
        Err(err) => {
            return ToolResponse::error(
                "Failed to extract bundle ID".to_string(),
                Some(err),
            );
        }
    };

    let mut lines = Vec::new();
    lines.push(format!("✅ Bundle ID: {}", bundle_id));
    lines.push("Next steps:".to_string());
    lines.push("- Simulator: install_app_sim + launch_app_sim".to_string());
    lines.push("- Device: install_app_device + launch_app_device".to_string());

    ToolResponse::text(lines.join("\n"), true)
}

fn validate_app_path(app_path: &str) -> Result<(), String> {
    let metadata = fs::metadata(app_path).map_err(|err| err.to_string())?;
    if metadata.is_dir() {
        Ok(())
    } else {
        Err("appPath is not a directory".to_string())
    }
}

fn read_bundle_id(app_path: &str, runner: &impl Runner) -> Result<String, String> {
    let info_path = format!("{}/Info", app_path.trim_end_matches('/'));
    let defaults_spec = CommandSpec {
        program: "defaults".to_string(),
        args: vec![
            "read".to_string(),
            info_path,
            "CFBundleIdentifier".to_string(),
        ],
        cwd: None,
        env: None,
    };
    let defaults_output = runner.run(&defaults_spec).map_err(|err| err.to_string())?;
    if defaults_output.exit_code == 0 {
        let bundle_id = defaults_output.stdout.trim();
        if !bundle_id.is_empty() {
            return Ok(bundle_id.to_string());
        }
    }

    let plist_path = format!("{}/Info.plist", app_path.trim_end_matches('/'));
    let plist_spec = CommandSpec {
        program: "/usr/libexec/PlistBuddy".to_string(),
        args: vec![
            "-c".to_string(),
            "Print :CFBundleIdentifier".to_string(),
            plist_path,
        ],
        cwd: None,
        env: None,
    };
    let plist_output = runner.run(&plist_spec).map_err(|err| err.to_string())?;
    if plist_output.exit_code == 0 {
        let bundle_id = plist_output.stdout.trim();
        if !bundle_id.is_empty() {
            return Ok(bundle_id.to_string());
        }
    }

    Err(format!(
        "defaults output: {}\nplistbuddy output: {}",
        format_command_output(&defaults_output),
        format_command_output(&plist_output)
    ))
}

fn format_command_output(output: &crate::exec::CommandOutput) -> String {
    let mut message = format!("Command exited with code {}", output.exit_code);
    if !output.stdout.trim().is_empty() {
        message.push_str("\nSTDOUT:\n");
        message.push_str(output.stdout.trim());
    }
    if !output.stderr.trim().is_empty() {
        message.push_str("\nSTDERR:\n");
        message.push_str(output.stderr.trim());
    }
    message
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::exec::{CommandOutput, CommandSpec};
    use std::cell::RefCell;

    struct MockRunner {
        outputs: RefCell<Vec<CommandOutput>>,
    }

    impl Runner for MockRunner {
        fn run(&self, _spec: &CommandSpec) -> std::io::Result<CommandOutput> {
            Ok(self.outputs.borrow_mut().remove(0))
        }
    }

    #[test]
    fn get_app_bundle_id_requires_app_path() {
        let result = get_app_bundle_id_from_input(GetAppBundleIdParamsInput { app_path: None });
        assert!(result.is_err());
    }

    #[test]
    fn read_bundle_id_uses_defaults_when_available() {
        let temp_dir = std::env::temp_dir().join("TestBundle.app");
        let _ = fs::create_dir_all(&temp_dir);

        let runner = MockRunner {
            outputs: RefCell::new(vec![CommandOutput {
                exit_code: 0,
                terminated_by_signal: false,
                stdout: "com.example.App".to_string(),
                stderr: String::new(),
            }]),
        };

        let result = read_bundle_id(temp_dir.to_string_lossy().as_ref(), &runner)
            .expect("bundle id");
        assert_eq!(result, "com.example.App");

        let _ = fs::remove_dir_all(&temp_dir);
    }

    #[test]
    fn read_bundle_id_falls_back_to_plistbuddy() {
        let temp_dir = std::env::temp_dir().join("TestBundleFallback.app");
        let _ = fs::create_dir_all(&temp_dir);

        let runner = MockRunner {
            outputs: RefCell::new(vec![
                CommandOutput {
                    exit_code: 1,
                    terminated_by_signal: false,
                    stdout: String::new(),
                    stderr: "defaults failed".to_string(),
                },
                CommandOutput {
                    exit_code: 0,
                    terminated_by_signal: false,
                    stdout: "com.example.Fallback".to_string(),
                    stderr: String::new(),
                },
            ]),
        };

        let result = read_bundle_id(temp_dir.to_string_lossy().as_ref(), &runner)
            .expect("bundle id");
        assert_eq!(result, "com.example.Fallback");

        let _ = fs::remove_dir_all(&temp_dir);
    }
}
