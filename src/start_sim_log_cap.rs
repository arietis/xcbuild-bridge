use serde::Deserialize;
use std::fs::OpenOptions;
use std::path::PathBuf;
use std::process::{Command, Stdio};
use std::time::{SystemTime, UNIX_EPOCH};

use crate::exec::Runner;
use crate::log_sessions::LogSessionStore;
use crate::session::SessionDefaults;
use crate::simctl::resolve_simulator_id;
use crate::tools::ToolResponse;

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StartSimLogCapParamsInput {
    pub simulator_id: Option<String>,
    pub simulator_name: Option<String>,
    pub bundle_id: Option<String>,
    pub use_latest_os: Option<bool>,
}

#[derive(Debug, Clone)]
pub struct StartSimLogCapParams {
    pub simulator_id: Option<String>,
    pub simulator_name: Option<String>,
    pub bundle_id: String,
    pub use_latest_os: bool,
}

pub fn start_sim_log_cap_from_input(
    input: StartSimLogCapParamsInput,
    defaults: &SessionDefaults,
) -> Result<StartSimLogCapParams, String> {
    let simulator_id = normalize_opt(input.simulator_id).or_else(|| defaults.simulator_id.clone());
    let simulator_name =
        normalize_opt(input.simulator_name).or_else(|| defaults.simulator_name.clone());
    let bundle_id = normalize_opt(input.bundle_id)
        .ok_or_else(|| "bundleId is required".to_string())?;
    let use_latest_os = input
        .use_latest_os
        .or(defaults.use_latest_os)
        .unwrap_or(true);

    if simulator_id.is_none() && simulator_name.is_none() {
        return Err("Either simulatorId or simulatorName is required.".to_string());
    }
    if simulator_id.is_some() && simulator_name.is_some() {
        return Err("simulatorId and simulatorName are mutually exclusive.".to_string());
    }

    Ok(StartSimLogCapParams {
        simulator_id,
        simulator_name,
        bundle_id,
        use_latest_os,
    })
}

pub fn execute_start_sim_log_cap(
    params: StartSimLogCapParams,
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

    let log_path = match create_log_file_path() {
        Ok(path) => path,
        Err(err) => {
            return ToolResponse::error("Failed to create log file".to_string(), Some(err));
        }
    };

    let file = match OpenOptions::new()
        .create(true)
        .write(true)
        .truncate(true)
        .open(&log_path)
    {
        Ok(file) => file,
        Err(err) => {
            return ToolResponse::error("Failed to open log file".to_string(), Some(err.to_string()));
        }
    };

    let stderr = match file.try_clone() {
        Ok(stderr) => stderr,
        Err(err) => {
            return ToolResponse::error(
                "Failed to prepare log file".to_string(),
                Some(err.to_string()),
            );
        }
    };

    let predicate = format!("subsystem == \"{}\"", params.bundle_id);

    let mut cmd = Command::new("xcrun");
    cmd.args([
        "simctl",
        "spawn",
        &simulator_id,
        "log",
        "stream",
        "--style",
        "syslog",
        "--predicate",
        &predicate,
    ])
    .stdin(Stdio::null())
    .stdout(Stdio::from(file))
    .stderr(Stdio::from(stderr));

    let child = match cmd.spawn() {
        Ok(child) => child,
        Err(err) => {
            return ToolResponse::error(
                "Failed to start log capture".to_string(),
                Some(err.to_string()),
            );
        }
    };

    let session_id = sessions.insert(
        child,
        log_path,
        simulator_id.clone(),
        params.bundle_id.clone(),
    );

    let mut lines = Vec::new();
    lines.push(format!(
        "Log capture started successfully. Session ID: {}.",
        session_id
    ));
    lines.push("Next steps:".to_string());
    lines.push(format!(
        "1. Interact with your app on simulator {}.",
        simulator_id
    ));
    lines.push(format!(
        "2. Stop and retrieve logs: stop_sim_log_cap({{ logSessionId: \"{}\" }})",
        session_id
    ));

    ToolResponse::text(lines.join("\n"), true)
}

fn create_log_file_path() -> Result<PathBuf, String> {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|err| err.to_string())?
        .as_nanos();
    let filename = format!("xcbuild-sim-log-{}-{}.log", std::process::id(), nanos);
    Ok(std::env::temp_dir().join(filename))
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
