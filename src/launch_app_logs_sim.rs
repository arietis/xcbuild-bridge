use serde::Deserialize;
use std::fs;

use crate::exec::{CommandSpec, Runner};
use crate::log_sessions::LogSessionStore;
use crate::session::SessionDefaults;
use crate::simctl::resolve_simulator_id;
use crate::start_sim_log_cap::{start_log_capture_session, SubsystemFilter};
use crate::tools::ToolResponse;

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LaunchAppLogsSimParamsInput {
    pub simulator_id: Option<String>,
    pub simulator_name: Option<String>,
    pub bundle_id: Option<String>,
    pub args: Option<Vec<String>>,
    pub capture_console: Option<bool>,
    pub use_latest_os: Option<bool>,
}

#[derive(Debug, Clone)]
pub struct LaunchAppLogsSimParams {
    pub simulator_id: Option<String>,
    pub simulator_name: Option<String>,
    pub bundle_id: String,
    pub args: Vec<String>,
    pub capture_console: bool,
    pub use_latest_os: bool,
}

pub fn launch_app_logs_sim_from_input(
    input: LaunchAppLogsSimParamsInput,
    defaults: &SessionDefaults,
) -> Result<LaunchAppLogsSimParams, String> {
    let simulator_id = normalize_opt(input.simulator_id).or_else(|| defaults.simulator_id.clone());
    let simulator_name =
        normalize_opt(input.simulator_name).or_else(|| defaults.simulator_name.clone());
    let bundle_id = normalize_opt(input.bundle_id);
    let args = normalize_vec(input.args);
    let capture_console = input.capture_console.unwrap_or(false);
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

    Ok(LaunchAppLogsSimParams {
        simulator_id,
        simulator_name,
        bundle_id,
        args,
        capture_console,
        use_latest_os,
    })
}

pub fn execute_launch_app_logs_sim(
    params: LaunchAppLogsSimParams,
    sessions: &mut LogSessionStore,
    runner: &impl Runner,
) -> ToolResponse {
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

    let start = match start_log_capture_session(
        &simulator_id,
        &params.bundle_id,
        params.capture_console,
        &SubsystemFilter::App,
        &params.args,
        sessions,
    ) {
        Ok(start) => start,
        Err(err) => {
            return ToolResponse::error("Failed to start log capture".to_string(), Some(err));
        }
    };

    if !params.capture_console {
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
                    cleanup_log_capture(&start.session_id, sessions);
                    return ToolResponse::error(
                        "Launch app failed".to_string(),
                        Some(format_command_output(&output)),
                    );
                }
            }
            Err(err) => {
                cleanup_log_capture(&start.session_id, sessions);
                return ToolResponse::error("Command failed".to_string(), Some(err.to_string()));
            }
        }
    }

    let mut lines = Vec::new();
    lines.push(format!(
        "App launched with log capture. Session ID: {}",
        start.session_id
    ));
    if start.capture_console {
        lines.push("Note: App was relaunched to capture console output.".to_string());
    }
    lines.push(format!(
        "Stop logs: stop_sim_log_cap({{ logSessionId: \"{}\" }})",
        start.session_id
    ));

    ToolResponse::text(lines.join("\n"), true)
}

fn cleanup_log_capture(session_id: &str, sessions: &mut LogSessionStore) {
    let mut session = match sessions.remove(session_id) {
        Some(session) => session,
        None => return,
    };

    for child in session.processes.iter_mut() {
        let _ = child.kill();
        let _ = child.wait();
    }

    let _ = fs::remove_file(&session.file_path);
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
    fn launch_app_logs_sim_requires_bundle_id() {
        let defaults = SessionDefaults::default();
        let result = launch_app_logs_sim_from_input(
            LaunchAppLogsSimParamsInput {
                simulator_id: Some("SIM".to_string()),
                simulator_name: None,
                bundle_id: None,
                args: None,
                capture_console: None,
                use_latest_os: None,
            },
            &defaults,
        );
        assert!(result.is_err());
    }
}
