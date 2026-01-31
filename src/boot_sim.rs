use serde::Deserialize;

use crate::exec::Runner;
use crate::session::SessionDefaults;
use crate::simctl::{boot_simulator, resolve_simulator_id};
use crate::tools::ToolResponse;

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BootSimParamsInput {
    pub simulator_id: Option<String>,
    pub simulator_name: Option<String>,
    pub use_latest_os: Option<bool>,
    pub wait_for_boot: Option<bool>,
}

#[derive(Debug, Clone)]
pub struct BootSimParams {
    pub simulator_id: Option<String>,
    pub simulator_name: Option<String>,
    pub use_latest_os: bool,
    pub wait_for_boot: bool,
}

pub fn boot_sim_from_input(
    input: BootSimParamsInput,
    defaults: &SessionDefaults,
) -> Result<BootSimParams, String> {
    let simulator_id = normalize_opt(input.simulator_id).or_else(|| defaults.simulator_id.clone());
    let simulator_name =
        normalize_opt(input.simulator_name).or_else(|| defaults.simulator_name.clone());
    let use_latest_os = input
        .use_latest_os
        .or(defaults.use_latest_os)
        .unwrap_or(true);
    let wait_for_boot = input.wait_for_boot.unwrap_or(true);

    if simulator_id.is_none() && simulator_name.is_none() {
        return Err("Either simulatorId or simulatorName is required.".to_string());
    }
    if simulator_id.is_some() && simulator_name.is_some() {
        return Err("simulatorId and simulatorName are mutually exclusive.".to_string());
    }

    Ok(BootSimParams {
        simulator_id,
        simulator_name,
        use_latest_os,
        wait_for_boot,
    })
}

pub fn execute_boot_sim(params: BootSimParams, runner: &impl Runner) -> ToolResponse {
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

    if let Err(err) = boot_simulator(&simulator_id, params.wait_for_boot, runner) {
        return ToolResponse::error("Boot simulator operation failed".to_string(), Some(err));
    }

    let mut text = format!("✅ Simulator {} booted.", simulator_id);
    if !params.wait_for_boot {
        text.push_str(" (bootstatus skipped)");
    }
    text.push_str("\nNext steps:\n1. Run tests: test_sim({})");
    ToolResponse::text(text, true)
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
