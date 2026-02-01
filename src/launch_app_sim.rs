use serde::Deserialize;

use crate::exec::{CommandSpec, Runner};
use crate::session::SessionDefaults;
use crate::simctl::resolve_simulator_id;
use crate::tools::ToolResponse;

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LaunchAppSimParamsInput {
    pub simulator_id: Option<String>,
    pub simulator_name: Option<String>,
    pub bundle_id: Option<String>,
    pub args: Option<Vec<String>>,
    pub use_latest_os: Option<bool>,
}

#[derive(Debug, Clone)]
pub struct LaunchAppSimParams {
    pub simulator_id: Option<String>,
    pub simulator_name: Option<String>,
    pub bundle_id: String,
    pub args: Vec<String>,
    pub use_latest_os: bool,
}

pub fn launch_app_sim_from_input(
    input: LaunchAppSimParamsInput,
    defaults: &SessionDefaults,
) -> Result<LaunchAppSimParams, String> {
    let simulator_id = normalize_opt(input.simulator_id).or_else(|| defaults.simulator_id.clone());
    let simulator_name =
        normalize_opt(input.simulator_name).or_else(|| defaults.simulator_name.clone());
    let bundle_id = normalize_opt(input.bundle_id);
    let args = normalize_vec(input.args);
    let use_latest_os = input
        .use_latest_os
        .or(defaults.use_latest_os)
        .unwrap_or(true);

    let bundle_id = bundle_id.ok_or_else(|| "bundleId is required".to_string())?;

    if simulator_id.is_none() && simulator_name.is_none() {
        return Err("Either simulatorId or simulatorName is required.".to_string());
    }
    if simulator_id.is_some() && simulator_name.is_some() {
        return Err("simulatorId and simulatorName are mutually exclusive.".to_string());
    }

    Ok(LaunchAppSimParams {
        simulator_id,
        simulator_name,
        bundle_id,
        args,
        use_latest_os,
    })
}

pub fn execute_launch_app_sim(params: LaunchAppSimParams, runner: &impl Runner) -> ToolResponse {
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

    if let Err(err) = ensure_app_installed(&simulator_id, &params.bundle_id, runner) {
        return ToolResponse::error(
            "App is not installed on the simulator".to_string(),
            Some(err),
        );
    }

    let mut args = vec![
        "simctl".to_string(),
        "launch".to_string(),
        simulator_id.clone(),
        params.bundle_id.clone(),
    ];
    args.extend(params.args.clone());

    let spec = CommandSpec {
        program: "xcrun".to_string(),
        args,
        cwd: None,
        env: None,
    };

    match runner.run(&spec) {
        Ok(output) => {
            if output.exit_code != 0 {
                return ToolResponse::error(
                    "Launch app failed".to_string(),
                    Some(format_command_output(&output)),
                );
            }
            let mut lines = Vec::new();
            lines.push(format!(
                "✅ App launched successfully in simulator {}",
                simulator_id
            ));
            lines.push("Next steps:".to_string());
            lines.push("1. If the simulator UI is not visible, open the Simulator app.".to_string());
            ToolResponse::text(lines.join("\n"), true)
        }
        Err(err) => ToolResponse::error("Command failed".to_string(), Some(err.to_string())),
    }
}

fn ensure_app_installed(
    simulator_id: &str,
    bundle_id: &str,
    runner: &impl Runner,
) -> Result<(), String> {
    let spec = CommandSpec {
        program: "xcrun".to_string(),
        args: vec![
            "simctl".to_string(),
            "get_app_container".to_string(),
            simulator_id.to_string(),
            bundle_id.to_string(),
            "app".to_string(),
        ],
        cwd: None,
        env: None,
    };
    let output = runner.run(&spec).map_err(|err| err.to_string())?;
    if output.exit_code == 0 {
        Ok(())
    } else {
        Err("Use install_app_sim before launching. Workflow: build → install → launch.".to_string())
    }
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

fn normalize_vec(values: Option<Vec<String>>) -> Vec<String> {
    values
        .unwrap_or_default()
        .into_iter()
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn launch_app_sim_requires_bundle_id() {
        let defaults = SessionDefaults::default();
        let result = launch_app_sim_from_input(
            LaunchAppSimParamsInput {
                simulator_id: Some("SIM".to_string()),
                simulator_name: None,
                bundle_id: None,
                args: None,
                use_latest_os: None,
            },
            &defaults,
        );
        assert!(result.is_err());
    }
}
