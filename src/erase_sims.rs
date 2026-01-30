use serde::Deserialize;

use crate::exec::{CommandSpec, Runner};
use crate::session::SessionDefaults;
use crate::simctl::resolve_simulator_id;
use crate::tools::ToolResponse;

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EraseSimsParamsInput {
    pub simulator_id: Option<String>,
    pub simulator_name: Option<String>,
    pub use_latest_os: Option<bool>,
    pub shutdown_first: Option<bool>,
}

#[derive(Debug, Clone)]
pub struct EraseSimsParams {
    pub simulator_id: Option<String>,
    pub simulator_name: Option<String>,
    pub use_latest_os: bool,
    pub shutdown_first: bool,
}

pub fn erase_sims_from_input(
    input: EraseSimsParamsInput,
    defaults: &SessionDefaults,
) -> Result<EraseSimsParams, String> {
    let simulator_id = normalize_opt(input.simulator_id).or_else(|| defaults.simulator_id.clone());
    let simulator_name =
        normalize_opt(input.simulator_name).or_else(|| defaults.simulator_name.clone());
    let use_latest_os = input
        .use_latest_os
        .or(defaults.use_latest_os)
        .unwrap_or(true);
    let shutdown_first = input.shutdown_first.unwrap_or(false);

    if simulator_id.is_none() && simulator_name.is_none() {
        return Err("Either simulatorId or simulatorName is required.".to_string());
    }
    if simulator_id.is_some() && simulator_name.is_some() {
        return Err("simulatorId and simulatorName are mutually exclusive.".to_string());
    }

    Ok(EraseSimsParams {
        simulator_id,
        simulator_name,
        use_latest_os,
        shutdown_first,
    })
}

pub fn execute_erase_sims(params: EraseSimsParams, runner: &impl Runner) -> ToolResponse {
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

    if params.shutdown_first {
        let shutdown_spec = CommandSpec {
            program: "xcrun".to_string(),
            args: vec![
                "simctl".to_string(),
                "shutdown".to_string(),
                simulator_id.clone(),
            ],
            cwd: None,
            env: None,
        };
        let _ = runner.run(&shutdown_spec);
    }

    let erase_spec = CommandSpec {
        program: "xcrun".to_string(),
        args: vec![
            "simctl".to_string(),
            "erase".to_string(),
            simulator_id.clone(),
        ],
        cwd: None,
        env: None,
    };
    let output = match runner.run(&erase_spec) {
        Ok(output) => output,
        Err(err) => {
            return ToolResponse::error(
                "Failed to erase simulator".to_string(),
                Some(err.to_string()),
            );
        }
    };

    if output.exit_code != 0 {
        let error_text = render_command_output(&output);
        if !params.shutdown_first && is_booted_error(&error_text) {
            return ToolResponse::error(
                "Failed to erase simulator".to_string(),
                Some(format!(
                    "{}\nHint: simulator appears Booted. Re-run erase_sims with shutdownFirst: true.",
                    error_text
                )),
            );
        }
        return ToolResponse::error("Failed to erase simulator".to_string(), Some(error_text));
    }

    ToolResponse::text(
        format!("✅ Successfully erased simulator {}", simulator_id),
        true,
    )
}

fn render_command_output(output: &crate::exec::CommandOutput) -> String {
    let mut text = format!("exit code {}", output.exit_code);
    if !output.stdout.trim().is_empty() {
        text.push_str("\nSTDOUT:\n");
        text.push_str(output.stdout.trim());
    }
    if !output.stderr.trim().is_empty() {
        text.push_str("\nSTDERR:\n");
        text.push_str(output.stderr.trim());
    }
    text
}

fn is_booted_error(text: &str) -> bool {
    let text = text.to_lowercase();
    text.contains("booted") && text.contains("unable")
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
