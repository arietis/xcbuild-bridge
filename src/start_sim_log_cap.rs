use serde::Deserialize;
use std::collections::BTreeSet;
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
    pub capture_console: Option<bool>,
    pub subsystem_filter: Option<SubsystemFilterInput>,
    pub use_latest_os: Option<bool>,
}

#[derive(Debug, Clone)]
pub struct StartSimLogCapParams {
    pub simulator_id: Option<String>,
    pub simulator_name: Option<String>,
    pub bundle_id: String,
    pub capture_console: bool,
    pub subsystem_filter: SubsystemFilter,
    pub use_latest_os: bool,
}

pub(crate) struct LogCaptureStart {
    pub session_id: String,
    pub simulator_id: String,
    pub capture_console: bool,
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
    let capture_console = input.capture_console.unwrap_or(false);
    let subsystem_filter = normalize_subsystem_filter(input.subsystem_filter)?;
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
        capture_console,
        subsystem_filter,
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

    let start = match start_log_capture_session(
        &simulator_id,
        &params.bundle_id,
        params.capture_console,
        &params.subsystem_filter,
        &[],
        sessions,
    ) {
        Ok(start) => start,
        Err(err) => {
            return ToolResponse::error("Failed to start log capture".to_string(), Some(err));
        }
    };

    let mut lines = Vec::new();
    lines.push(format!(
        "Log capture started successfully. Session ID: {}.",
        start.session_id
    ));
    if start.capture_console {
        lines.push("Note: App was relaunched to capture console output.".to_string());
    }
    lines.push("Next steps:".to_string());
    lines.push(format!(
        "1. Interact with your app on simulator {}.",
        start.simulator_id
    ));
    lines.push(format!(
        "2. Stop and retrieve logs: stop_sim_log_cap({{ logSessionId: \"{}\" }})",
        start.session_id
    ));

    ToolResponse::text(lines.join("\n"), true)
}

pub(crate) fn start_log_capture_session(
    simulator_id: &str,
    bundle_id: &str,
    capture_console: bool,
    subsystem_filter: &SubsystemFilter,
    launch_args: &[String],
    sessions: &mut LogSessionStore,
) -> Result<LogCaptureStart, String> {
    let log_path = create_log_file_path()?;

    let file = OpenOptions::new()
        .create(true)
        .write(true)
        .truncate(true)
        .open(&log_path)
        .map_err(|err| err.to_string())?;
    drop(file);

    let predicate = build_log_predicate(bundle_id, subsystem_filter);
    let mut processes = Vec::new();

    if capture_console {
        let stdout = open_log_append(&log_path)?;
        let stderr = open_log_append(&log_path)?;

        let mut cmd = Command::new("xcrun");
        cmd.args([
            "simctl",
            "launch",
            "--console-pty",
            "--terminate-running-process",
            simulator_id,
            bundle_id,
        ])
        .args(launch_args)
        .stdin(Stdio::null())
        .stdout(Stdio::from(stdout))
        .stderr(Stdio::from(stderr));

        let child = cmd
            .spawn()
            .map_err(|err| format!("Failed to start console log capture: {}", err))?;
        processes.push(child);
    }

    let stdout = open_log_append(&log_path).map_err(|err| {
        terminate_processes(&mut processes);
        err
    })?;
    let stderr = open_log_append(&log_path).map_err(|err| {
        terminate_processes(&mut processes);
        err
    })?;

    let mut cmd = Command::new("xcrun");
    cmd.args([
        "simctl",
        "spawn",
        simulator_id,
        "log",
        "stream",
        "--level=debug",
        "--style",
        "syslog",
    ])
    .stdin(Stdio::null())
    .stdout(Stdio::from(stdout))
    .stderr(Stdio::from(stderr));

    if let Some(predicate) = predicate {
        cmd.args(["--predicate", &predicate]);
    }

    let child = cmd
        .spawn()
        .map_err(|err| format!("Failed to start log capture: {}", err))?;
    processes.push(child);

    let session_id = sessions.insert(
        processes,
        log_path,
        simulator_id.to_string(),
        bundle_id.to_string(),
    );

    Ok(LogCaptureStart {
        session_id,
        simulator_id: simulator_id.to_string(),
        capture_console,
    })
}

fn create_log_file_path() -> Result<PathBuf, String> {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|err| err.to_string())?
        .as_nanos();
    let filename = format!("xcbuild-sim-log-{}-{}.log", std::process::id(), nanos);
    Ok(std::env::temp_dir().join(filename))
}

fn open_log_append(path: &PathBuf) -> Result<std::fs::File, String> {
    OpenOptions::new()
        .create(true)
        .append(true)
        .open(path)
        .map_err(|err| err.to_string())
}

fn terminate_processes(processes: &mut [std::process::Child]) {
    for child in processes {
        let _ = child.kill();
        let _ = child.wait();
    }
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum SubsystemFilterName {
    App,
    All,
    Swiftui,
}

#[derive(Debug, Deserialize)]
#[serde(untagged)]
pub enum SubsystemFilterInput {
    Named(SubsystemFilterName),
    List(Vec<String>),
}

#[derive(Debug, Clone)]
pub enum SubsystemFilter {
    App,
    All,
    Swiftui,
    Custom(Vec<String>),
}

fn normalize_subsystem_filter(
    input: Option<SubsystemFilterInput>,
) -> Result<SubsystemFilter, String> {
    match input.unwrap_or(SubsystemFilterInput::Named(SubsystemFilterName::App)) {
        SubsystemFilterInput::Named(name) => Ok(match name {
            SubsystemFilterName::App => SubsystemFilter::App,
            SubsystemFilterName::All => SubsystemFilter::All,
            SubsystemFilterName::Swiftui => SubsystemFilter::Swiftui,
        }),
        SubsystemFilterInput::List(list) => {
            let mut cleaned: Vec<String> = list
                .into_iter()
                .map(|value| value.trim().to_string())
                .filter(|value| !value.is_empty())
                .collect();
            cleaned.sort();
            cleaned.dedup();
            if cleaned.is_empty() {
                return Err("subsystemFilter list cannot be empty".to_string());
            }
            Ok(SubsystemFilter::Custom(cleaned))
        }
    }
}

fn build_log_predicate(bundle_id: &str, filter: &SubsystemFilter) -> Option<String> {
    match filter {
        SubsystemFilter::All => None,
        SubsystemFilter::App => Some(format!("subsystem == \"{}\"", bundle_id)),
        SubsystemFilter::Swiftui => Some(format!(
            "subsystem == \"{}\" OR subsystem == \"com.apple.SwiftUI\"",
            bundle_id
        )),
        SubsystemFilter::Custom(subsystems) => {
            let mut set = BTreeSet::new();
            set.insert(bundle_id.to_string());
            for entry in subsystems {
                set.insert(entry.clone());
            }
            let predicates: Vec<String> = set
                .into_iter()
                .map(|entry| format!("subsystem == \"{}\"", entry))
                .collect();
            Some(predicates.join(" OR "))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn build_log_predicate_app() {
        let predicate = build_log_predicate("com.example.app", &SubsystemFilter::App)
            .expect("predicate");
        assert_eq!(predicate, "subsystem == \"com.example.app\"");
    }

    #[test]
    fn build_log_predicate_all_returns_none() {
        let predicate = build_log_predicate("com.example.app", &SubsystemFilter::All);
        assert!(predicate.is_none());
    }

    #[test]
    fn build_log_predicate_custom_includes_bundle() {
        let filter = SubsystemFilter::Custom(vec!["com.example.custom".to_string()]);
        let predicate = build_log_predicate("com.example.app", &filter).expect("predicate");
        assert!(predicate.contains("com.example.app"));
        assert!(predicate.contains("com.example.custom"));
    }
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
