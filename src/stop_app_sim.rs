use serde::Deserialize;

use crate::exec::{CommandSpec, Runner};
use crate::session::SessionDefaults;
use crate::simctl::resolve_simulator_id;
use crate::tools::ToolResponse;

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StopAppSimParamsInput {
    pub simulator_id: Option<String>,
    pub simulator_name: Option<String>,
    pub bundle_id: Option<String>,
    pub use_latest_os: Option<bool>,
}

#[derive(Debug, Clone)]
pub struct StopAppSimParams {
    pub simulator_id: Option<String>,
    pub simulator_name: Option<String>,
    pub bundle_id: String,
    pub use_latest_os: bool,
}

pub fn stop_app_sim_from_input(
    input: StopAppSimParamsInput,
    defaults: &SessionDefaults,
) -> Result<StopAppSimParams, String> {
    let simulator_id = normalize_opt(input.simulator_id).or_else(|| defaults.simulator_id.clone());
    let simulator_name =
        normalize_opt(input.simulator_name).or_else(|| defaults.simulator_name.clone());
    let bundle_id = normalize_opt(input.bundle_id);
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

    Ok(StopAppSimParams {
        simulator_id,
        simulator_name,
        bundle_id,
        use_latest_os,
    })
}

pub fn execute_stop_app_sim(params: StopAppSimParams, runner: &impl Runner) -> ToolResponse {
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

    let spec = CommandSpec {
        program: "xcrun".to_string(),
        args: vec![
            "simctl".to_string(),
            "terminate".to_string(),
            simulator_id.clone(),
            params.bundle_id.clone(),
        ],
        cwd: None,
        env: None,
    };

    match runner.run(&spec) {
        Ok(output) => {
            if output.exit_code != 0 {
                return ToolResponse::error(
                    "Stop app failed".to_string(),
                    Some(format_command_output(&output)),
                );
            }
            ToolResponse::text(
                format!(
                    "✅ App {} stopped successfully in simulator {}",
                    params.bundle_id, simulator_id
                ),
                true,
            )
        }
        Err(err) => ToolResponse::error("Command failed".to_string(), Some(err.to_string())),
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn stop_app_sim_requires_bundle_id() {
        let defaults = SessionDefaults::default();
        let result = stop_app_sim_from_input(
            StopAppSimParamsInput {
                simulator_id: Some("SIM".to_string()),
                simulator_name: None,
                bundle_id: None,
                use_latest_os: None,
            },
            &defaults,
        );
        assert!(result.is_err());
    }
}
