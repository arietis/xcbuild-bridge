use serde::Deserialize;
use std::collections::BTreeMap;

use crate::exec::Runner;
use crate::simctl::{SimDevice, list_simulators};
use crate::tools::ToolResponse;

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ListSimsParams {
    pub available_only: Option<bool>,
}

pub fn execute_list_sims(params: ListSimsParams, runner: &impl Runner) -> ToolResponse {
    let available_only = params.available_only.unwrap_or(true);
    match list_simulators(runner) {
        Ok(devices) => ToolResponse::text(format_simulator_list(&devices, available_only), true),
        Err(err) => ToolResponse::error("Failed to list simulators".to_string(), Some(err)),
    }
}

fn format_simulator_list(devices: &[SimDevice], available_only: bool) -> String {
    let mut grouped: BTreeMap<String, Vec<&SimDevice>> = BTreeMap::new();
    for device in devices {
        if available_only && !device.is_available {
            continue;
        }
        grouped
            .entry(device.runtime.clone())
            .or_default()
            .push(device);
    }

    let mut lines = Vec::new();
    lines.push("Available iOS Simulators:".to_string());
    lines.push(String::new());

    for (runtime, entries) in grouped {
        lines.push(format!("{}:", runtime));
        for device in entries {
            let booted_suffix = if device.state == "Booted" {
                " [Booted]"
            } else {
                ""
            };
            lines.push(format!(
                "- {} ({}){}",
                device.name, device.udid, booted_suffix
            ));
        }
        lines.push(String::new());
    }

    lines.push("Next steps:".to_string());
    lines.push("1. Boot a simulator: boot_sim({ simulatorId: \"UUID\" })".to_string());
    lines.push("2. Run tests: test_sim({})".to_string());

    lines.join("\n")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn format_simulator_list_hides_unavailable_when_requested() {
        let devices = vec![
            SimDevice {
                name: "iPhone 15".to_string(),
                udid: "A".to_string(),
                state: "Booted".to_string(),
                is_available: true,
                runtime: "iOS 18.0".to_string(),
            },
            SimDevice {
                name: "iPhone 15".to_string(),
                udid: "B".to_string(),
                state: "Shutdown".to_string(),
                is_available: false,
                runtime: "iOS 18.0".to_string(),
            },
        ];

        let text = format_simulator_list(&devices, true);
        assert!(text.contains("iOS 18.0"));
        assert!(text.contains("A"));
        assert!(!text.contains("(B)"));
    }
}
