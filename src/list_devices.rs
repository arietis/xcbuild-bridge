use serde::Deserialize;
use serde_json::Value;
use std::fs;
use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

use crate::exec::{CommandSpec, Runner};
use crate::tools::ToolResponse;

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ListDevicesParams {}

#[derive(Debug)]
struct DeviceInfo {
    name: String,
    identifier: String,
    platform: String,
    os_version: Option<String>,
    model: Option<String>,
    connection: Option<String>,
    state: String,
}

pub fn execute_list_devices(_params: ListDevicesParams, runner: &impl Runner) -> ToolResponse {
    match list_devices_via_devicectl(runner) {
        Ok(devices) if !devices.is_empty() => ToolResponse::text(format_devices(devices), true),
        _ => match list_devices_via_xctrace(runner) {
            Ok(output) => ToolResponse::text(output, true),
            Err(err) => ToolResponse::error("Failed to list devices".to_string(), Some(err)),
        },
    }
}

fn list_devices_via_devicectl(runner: &impl Runner) -> Result<Vec<DeviceInfo>, String> {
    let temp_path = temp_json_path("devicectl");
    let spec = CommandSpec {
        program: "xcrun".to_string(),
        args: vec![
            "devicectl".to_string(),
            "list".to_string(),
            "devices".to_string(),
            "--json-output".to_string(),
            temp_path.to_string_lossy().to_string(),
        ],
        cwd: None,
        env: None,
    };
    let output = runner.run(&spec).map_err(|err| err.to_string())?;
    if output.exit_code != 0 {
        let _ = fs::remove_file(&temp_path);
        return Err(format!(
            "devicectl exited with code {}",
            output.exit_code
        ));
    }

    let json = fs::read_to_string(&temp_path).map_err(|err| err.to_string())?;
    let _ = fs::remove_file(&temp_path);

    let value: Value = serde_json::from_str(&json).map_err(|err| err.to_string())?;
    let devices = parse_devicectl_devices(&value);
    Ok(devices)
}

fn list_devices_via_xctrace(runner: &impl Runner) -> Result<String, String> {
    let spec = CommandSpec {
        program: "xcrun".to_string(),
        args: vec!["xctrace".to_string(), "list".to_string(), "devices".to_string()],
        cwd: None,
        env: None,
    };
    let output = runner.run(&spec).map_err(|err| err.to_string())?;
    if output.exit_code != 0 {
        return Err(format!("xctrace exited with code {}", output.exit_code));
    }
    let mut text = String::from("Device listing (xctrace output):\n\n");
    text.push_str(output.stdout.trim());
    text.push_str("\n\nNote: For better device information, upgrade to Xcode 15+ for devicectl JSON.");
    Ok(text)
}

fn parse_devicectl_devices(value: &Value) -> Vec<DeviceInfo> {
    let devices = value
        .get("result")
        .and_then(|result| result.get("devices"))
        .and_then(|devices| devices.as_array())
        .cloned()
        .unwrap_or_default();

    let mut results = Vec::new();
    for device in devices {
        let visibility = device
            .get("visibilityClass")
            .and_then(|v| v.as_str())
            .unwrap_or("");
        if visibility == "Simulator" {
            continue;
        }

        let identifier = device
            .get("identifier")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string();
        if identifier.is_empty() {
            continue;
        }

        let connection_props = device.get("connectionProperties").and_then(|v| v.as_object());
        let pairing_state = connection_props
            .and_then(|props| props.get("pairingState"))
            .and_then(|v| v.as_str())
            .unwrap_or("");
        if pairing_state.is_empty() {
            continue;
        }

        let tunnel_state = connection_props
            .and_then(|props| props.get("tunnelState"))
            .and_then(|v| v.as_str())
            .unwrap_or("");
        let transport = connection_props
            .and_then(|props| props.get("transportType"))
            .and_then(|v| v.as_str())
            .map(|value| value.to_string());

        let device_props = device.get("deviceProperties").and_then(|v| v.as_object());
        let name = device_props
            .and_then(|props| props.get("name"))
            .and_then(|v| v.as_str())
            .unwrap_or("Unknown Device")
            .to_string();
        let platform_id = device_props
            .and_then(|props| props.get("platformIdentifier"))
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_lowercase();
        let os_version = device_props
            .and_then(|props| props.get("osVersionNumber"))
            .and_then(|v| v.as_str())
            .map(|value| value.to_string());
        let model = device_props
            .and_then(|props| props.get("marketingName"))
            .and_then(|v| v.as_str())
            .map(|value| value.to_string());

        let platform = if platform_id.contains("ios") || platform_id.contains("iphone") {
            "iOS"
        } else if platform_id.contains("ipad") {
            "iPadOS"
        } else if platform_id.contains("watch") {
            "watchOS"
        } else if platform_id.contains("tv") || platform_id.contains("apple tv") {
            "tvOS"
        } else if platform_id.contains("vision") {
            "visionOS"
        } else {
            "Unknown"
        };

        let state = if pairing_state == "paired" {
            if tunnel_state == "connected" {
                "Available".to_string()
            } else {
                "Available (WiFi)".to_string()
            }
        } else {
            "Unpaired".to_string()
        };

        results.push(DeviceInfo {
            name,
            identifier,
            platform: platform.to_string(),
            os_version,
            model,
            connection: transport,
            state,
        });
    }

    results
}

fn format_devices(mut devices: Vec<DeviceInfo>) -> String {
    devices.sort_by(|a, b| a.name.cmp(&b.name));
    let mut text = String::from("Connected Devices:\n\n");

    let available: Vec<&DeviceInfo> = devices
        .iter()
        .filter(|d| d.state.starts_with("Available"))
        .collect();
    let unpaired: Vec<&DeviceInfo> = devices.iter().filter(|d| d.state == "Unpaired").collect();

    if available.is_empty() && unpaired.is_empty() {
        text.push_str("No physical Apple devices found.\n");
        return text;
    }

    if !available.is_empty() {
        text.push_str("✅ Available Devices:\n");
        for device in available {
            text.push_str(&format!("\n📱 {}\n", device.name));
            text.push_str(&format!("   UDID: {}\n", device.identifier));
            if let Some(model) = &device.model {
                text.push_str(&format!("   Model: {}\n", model));
            }
            text.push_str(&format!(
                "   Platform: {} {}\n",
                device.platform,
                device.os_version.clone().unwrap_or_default()
            ));
            if let Some(connection) = &device.connection {
                text.push_str(&format!("   Connection: {}\n", connection));
            }
        }
        text.push('\n');
    }

    if !unpaired.is_empty() {
        text.push_str("❌ Unpaired Devices:\n");
        for device in unpaired {
            text.push_str(&format!("- {} ({})\n", device.name, device.identifier));
        }
        text.push('\n');
    }

    text
}

fn temp_json_path(prefix: &str) -> PathBuf {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos();
    let filename = format!("{}-{}.json", prefix, nanos);
    std::env::temp_dir().join(filename)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn parse_devicectl_devices_filters_simulators() {
        let value = json!({
            "result": {
                "devices": [
                    { "visibilityClass": "Simulator", "identifier": "SIM" },
                    {
                        "visibilityClass": "Device",
                        "identifier": "DEV1",
                        "connectionProperties": { "pairingState": "paired", "tunnelState": "connected" },
                        "deviceProperties": { "name": "iPhone", "platformIdentifier": "ios", "osVersionNumber": "17.0" }
                    }
                ]
            }
        });
        let devices = parse_devicectl_devices(&value);
        assert_eq!(devices.len(), 1);
        assert_eq!(devices[0].identifier, "DEV1");
    }
}
