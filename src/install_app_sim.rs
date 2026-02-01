use serde::Deserialize;
use std::fs;

use crate::exec::{CommandSpec, Runner};
use crate::session::SessionDefaults;
use crate::simctl::resolve_simulator_id;
use crate::tools::ToolResponse;

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct InstallAppSimParamsInput {
    pub simulator_id: Option<String>,
    pub simulator_name: Option<String>,
    pub app_path: Option<String>,
    pub use_latest_os: Option<bool>,
}

#[derive(Debug, Clone)]
pub struct InstallAppSimParams {
    pub simulator_id: Option<String>,
    pub simulator_name: Option<String>,
    pub app_path: String,
    pub use_latest_os: bool,
}

pub fn install_app_sim_from_input(
    input: InstallAppSimParamsInput,
    defaults: &SessionDefaults,
) -> Result<InstallAppSimParams, String> {
    let simulator_id = normalize_opt(input.simulator_id).or_else(|| defaults.simulator_id.clone());
    let simulator_name =
        normalize_opt(input.simulator_name).or_else(|| defaults.simulator_name.clone());
    let app_path = normalize_opt(input.app_path);
    let use_latest_os = input
        .use_latest_os
        .or(defaults.use_latest_os)
        .unwrap_or(true);

    let app_path = app_path.ok_or_else(|| "appPath is required".to_string())?;

    if simulator_id.is_none() && simulator_name.is_none() {
        return Err("Either simulatorId or simulatorName is required.".to_string());
    }
    if simulator_id.is_some() && simulator_name.is_some() {
        return Err("simulatorId and simulatorName are mutually exclusive.".to_string());
    }

    Ok(InstallAppSimParams {
        simulator_id,
        simulator_name,
        app_path,
        use_latest_os,
    })
}

pub fn execute_install_app_sim(params: InstallAppSimParams, runner: &impl Runner) -> ToolResponse {
    if let Err(err) = validate_app_path(&params.app_path) {
        return ToolResponse::error("Invalid appPath".to_string(), Some(err));
    }

    let simulator_id = match resolve_simulator_id(
        params.simulator_id.clone(),
        params.simulator_name.clone(),
        params.use_latest_os,
        runner,
    ) {
        Ok(id) => id,
        Err(err) => {
            return ToolResponse::error("Failed to resolve simulator".to_string(), Some(err));
        }
    };

    let install_spec = CommandSpec {
        program: "xcrun".to_string(),
        args: vec![
            "simctl".to_string(),
            "install".to_string(),
            simulator_id.clone(),
            params.app_path.clone(),
        ],
        cwd: None,
        env: None,
    };

    let install_output = match runner.run(&install_spec) {
        Ok(output) => output,
        Err(err) => {
            return ToolResponse::error("Install app failed".to_string(), Some(err.to_string()));
        }
    };

    if install_output.exit_code != 0 {
        return ToolResponse::error(
            "Install app failed".to_string(),
            Some(format_command_output(&install_output)),
        );
    }

    let bundle_id = read_bundle_id(&params.app_path, runner).unwrap_or_default();

    let mut lines = Vec::new();
    lines.push(format!(
        "✅ App installed successfully in simulator {}",
        simulator_id
    ));
    lines.push("Next steps:".to_string());
    if !bundle_id.is_empty() {
        lines.push(format!(
            "1. Launch app: launch_app_sim({{ simulatorId: \"{}\", bundleId: \"{}\" }})",
            simulator_id, bundle_id
        ));
    } else {
        lines.push(format!(
            "1. Launch app: launch_app_sim({{ simulatorId: \"{}\", bundleId: \"BUNDLE_ID\" }})",
            simulator_id
        ));
    }

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

fn read_bundle_id(app_path: &str, runner: &impl Runner) -> Option<String> {
    let info_path = format!("{}/Info", app_path.trim_end_matches('/'));
    let spec = CommandSpec {
        program: "defaults".to_string(),
        args: vec![
            "read".to_string(),
            info_path,
            "CFBundleIdentifier".to_string(),
        ],
        cwd: None,
        env: None,
    };
    let output = runner.run(&spec).ok()?;
    if output.exit_code == 0 {
        let bundle_id = output.stdout.trim();
        if !bundle_id.is_empty() {
            return Some(bundle_id.to_string());
        }
    }
    None
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
    fn install_app_sim_requires_app_path() {
        let defaults = SessionDefaults::default();
        let result = install_app_sim_from_input(
            InstallAppSimParamsInput {
                simulator_id: Some("SIM".to_string()),
                simulator_name: None,
                app_path: None,
                use_latest_os: None,
            },
            &defaults,
        );
        assert!(result.is_err());
    }

    #[test]
    fn install_app_sim_reads_bundle_id_when_available() {
        let temp_dir = std::env::temp_dir();
        let app_dir = temp_dir.join("TestApp.app");
        let _ = fs::create_dir_all(&app_dir);

        let runner = MockRunner {
            outputs: RefCell::new(vec![
                CommandOutput {
                    exit_code: 0,
                    terminated_by_signal: false,
                    stdout: String::new(),
                    stderr: String::new(),
                },
                CommandOutput {
                    exit_code: 0,
                    terminated_by_signal: false,
                    stdout: "com.example.Test".to_string(),
                    stderr: String::new(),
                },
            ]),
        };

        let params = InstallAppSimParams {
            simulator_id: Some("SIM".to_string()),
            simulator_name: None,
            app_path: app_dir.to_string_lossy().to_string(),
            use_latest_os: true,
        };

        let response = execute_install_app_sim(params, &runner);
        assert!(response
            .content
            .iter()
            .any(|content| matches!(content, crate::tools::ToolResponseContent::Text { text } if text.contains("com.example.Test"))));

        let _ = fs::remove_dir_all(&app_dir);
    }

    #[test]
    fn validate_app_path_rejects_files() {
        let temp_file = std::env::temp_dir().join("xcbuild-app.txt");
        let _ = fs::write(&temp_file, "test");
        let result = validate_app_path(temp_file.to_string_lossy().as_ref());
        assert!(result.is_err());
        let _ = fs::remove_file(&temp_file);
    }
}
