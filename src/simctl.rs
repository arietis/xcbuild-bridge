use serde::Deserialize;
use std::cmp::Ordering;
use std::collections::HashMap;

use crate::exec::{CommandOutput, CommandSpec, Runner};

#[derive(Debug, Clone)]
pub struct SimDevice {
    pub name: String,
    pub udid: String,
    pub state: String,
    pub is_available: bool,
    pub runtime: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct SimctlDevice {
    name: String,
    udid: String,
    state: String,
    is_available: bool,
}

#[derive(Debug, Deserialize)]
struct SimctlList {
    devices: HashMap<String, Vec<SimctlDevice>>,
}

pub fn list_simulators(runner: &impl Runner) -> Result<Vec<SimDevice>, String> {
    let spec = CommandSpec {
        program: "xcrun".to_string(),
        args: vec![
            "simctl".to_string(),
            "list".to_string(),
            "devices".to_string(),
            "--json".to_string(),
        ],
        cwd: None,
        env: None,
    };
    let output = runner.run(&spec).map_err(|err| err.to_string())?;
    if output.exit_code != 0 {
        return Err(render_error("xcrun simctl list failed", &output));
    }

    let list: SimctlList = serde_json::from_str(&output.stdout).map_err(|err| err.to_string())?;
    let mut devices = Vec::new();
    for (runtime, entries) in list.devices {
        for entry in entries {
            devices.push(SimDevice {
                name: entry.name,
                udid: entry.udid,
                state: entry.state,
                is_available: entry.is_available,
                runtime: runtime.clone(),
            });
        }
    }
    Ok(devices)
}

pub fn resolve_simulator_id(
    simulator_id: Option<String>,
    simulator_name: Option<String>,
    use_latest_os: bool,
    runner: &impl Runner,
) -> Result<String, String> {
    if let Some(id) = simulator_id {
        let trimmed = id.trim().to_string();
        if trimmed.is_empty() {
            return Err("simulatorId is empty".to_string());
        }
        return Ok(trimmed);
    }

    let name = simulator_name
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
        .ok_or_else(|| "simulatorId or simulatorName is required".to_string())?;

    let devices = list_simulators(runner)?;
    let mut matches: Vec<SimDevice> = devices
        .into_iter()
        .filter(|device| device.is_available)
        .filter(|device| device.name == name)
        .collect();

    if matches.is_empty() {
        return Err(format!(
            "Simulator named \"{}\" not found. Use list_sims to see available simulators.",
            name
        ));
    }

    if use_latest_os {
        matches.sort_by(|a, b| {
            let runtime_cmp = compare_runtime_versions(&a.runtime, &b.runtime);
            if runtime_cmp == Ordering::Equal {
                state_rank(&a.state).cmp(&state_rank(&b.state))
            } else {
                runtime_cmp
            }
        });
        return Ok(matches.last().expect("matches not empty").udid.clone());
    }

    if let Some(booted) = matches.iter().find(|device| device.state == "Booted") {
        return Ok(booted.udid.clone());
    }

    Ok(matches.first().expect("matches not empty").udid.clone())
}

fn compare_runtime_versions(a: &str, b: &str) -> Ordering {
    let a_parts = parse_runtime_version(a);
    let b_parts = parse_runtime_version(b);
    let max_len = a_parts.len().max(b_parts.len());

    for idx in 0..max_len {
        let a_val = a_parts.get(idx).copied().unwrap_or(0);
        let b_val = b_parts.get(idx).copied().unwrap_or(0);
        match a_val.cmp(&b_val) {
            Ordering::Equal => continue,
            ordering => return ordering,
        }
    }

    Ordering::Equal
}

fn parse_runtime_version(runtime: &str) -> Vec<u32> {
    runtime
        .split(|ch: char| !ch.is_ascii_digit())
        .filter(|chunk| !chunk.is_empty())
        .filter_map(|chunk| chunk.parse::<u32>().ok())
        .collect()
}

fn state_rank(state: &str) -> u8 {
    match state {
        "Booted" => 2,
        "Booting" => 1,
        _ => 0,
    }
}

fn render_error(prefix: &str, output: &CommandOutput) -> String {
    let mut message = format!("{} (code {})", prefix, output.exit_code);
    if !output.stderr.trim().is_empty() {
        message.push_str(": ");
        message.push_str(output.stderr.trim());
    }
    message
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::RefCell;

    struct MockRunner {
        output: RefCell<Option<CommandOutput>>,
    }

    impl Runner for MockRunner {
        fn run(&self, _spec: &CommandSpec) -> std::io::Result<CommandOutput> {
            Ok(self
                .output
                .borrow_mut()
                .take()
                .expect("mock output missing"))
        }
    }

    #[test]
    fn resolve_simulator_id_uses_latest_runtime() {
        let json = r#"
        {
          "devices": {
            "com.apple.CoreSimulator.SimRuntime.iOS-17-0": [
              { "name": "iPhone 15", "udid": "OLD", "state": "Shutdown", "isAvailable": true }
            ],
            "com.apple.CoreSimulator.SimRuntime.iOS-18-2": [
              { "name": "iPhone 15", "udid": "NEW", "state": "Shutdown", "isAvailable": true }
            ]
          }
        }
        "#;
        let runner = MockRunner {
            output: RefCell::new(Some(CommandOutput {
                exit_code: 0,
                terminated_by_signal: false,
                stdout: json.to_string(),
                stderr: String::new(),
            })),
        };

        let result = resolve_simulator_id(None, Some("iPhone 15".to_string()), true, &runner)
            .expect("should resolve");
        assert_eq!(result, "NEW");
    }

    #[test]
    fn resolve_simulator_id_prefers_booted_when_not_latest() {
        let json = r#"
        {
          "devices": {
            "com.apple.CoreSimulator.SimRuntime.iOS-18-0": [
              { "name": "iPhone 15", "udid": "BOOTED", "state": "Booted", "isAvailable": true },
              { "name": "iPhone 15", "udid": "SHUT", "state": "Shutdown", "isAvailable": true }
            ]
          }
        }
        "#;
        let runner = MockRunner {
            output: RefCell::new(Some(CommandOutput {
                exit_code: 0,
                terminated_by_signal: false,
                stdout: json.to_string(),
                stderr: String::new(),
            })),
        };

        let result = resolve_simulator_id(None, Some("iPhone 15".to_string()), false, &runner)
            .expect("should resolve");
        assert_eq!(result, "BOOTED");
    }
}
