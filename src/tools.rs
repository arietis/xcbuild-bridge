use serde::Serialize;
use serde_json::{Value, json};

use crate::boot_sim::{BootSimParamsInput, boot_sim_from_input, execute_boot_sim};
use crate::build_for_testing_sim::{
    BuildForTestingSimParamsInput, build_for_testing_sim_from_input, execute_build_for_testing_sim,
};
use crate::build_run_sim::{BuildRunSimParamsInput, build_run_sim_from_input, execute_build_run_sim};
use crate::build_sim::{BuildSimParamsInput, build_sim_from_input, execute_build_sim};
use crate::discover_projs::{DiscoverProjsParamsInput, execute_discover_projs};
use crate::discover_xctestrun::{DiscoverXctestrunParamsInput, execute_discover_xctestrun};
use crate::erase_sims::{EraseSimsParamsInput, erase_sims_from_input, execute_erase_sims};
use crate::exec::Runner;
use crate::get_app_bundle_id::{
    GetAppBundleIdParamsInput, execute_get_app_bundle_id, get_app_bundle_id_from_input,
};
use crate::get_sim_app_path::{
    GetSimAppPathParamsInput, execute_get_sim_app_path, get_sim_app_path_from_input,
};
use crate::install_app_sim::{
    InstallAppSimParamsInput, execute_install_app_sim, install_app_sim_from_input,
};
use crate::launch_app_sim::{
    LaunchAppSimParamsInput, execute_launch_app_sim, launch_app_sim_from_input,
};
use crate::log_sessions::LogSessionStore;
use crate::list_schemes::{ListSchemesParamsInput, execute_list_schemes, list_schemes_from_input};
use crate::list_sims::{ListSimsParams, execute_list_sims};
use crate::start_sim_log_cap::{
    StartSimLogCapParamsInput, execute_start_sim_log_cap, start_sim_log_cap_from_input,
};
use crate::stop_sim_log_cap::{
    StopSimLogCapParamsInput, execute_stop_sim_log_cap, stop_sim_log_cap_from_input,
};
use crate::session::{SessionClearDefaultsParams, SessionSetDefaultsParams, SessionStore};
use crate::smoke_sim::{SmokeSimParamsInput, execute_smoke_sim, smoke_sim_from_input};
use crate::test_sim::{TestSimParamsInput, execute_test_sim, test_sim_from_input};
use crate::test_without_building_sim::{
    TestWithoutBuildingSimParamsInput, execute_test_without_building_sim,
    test_without_building_sim_from_input,
};

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ToolDefinition {
    pub name: &'static str,
    pub title: &'static str,
    pub description: &'static str,
    pub input_schema: Value,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub annotations: Option<Value>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ToolResponse {
    pub content: Vec<ToolResponseContent>,
    pub is_error: bool,
}

impl ToolResponse {
    pub fn text(text: String, ok: bool) -> Self {
        ToolResponse {
            content: vec![ToolResponseContent::Text { text }],
            is_error: !ok,
        }
    }

    pub fn error(message: String, details: Option<String>) -> Self {
        let detail_text = details
            .map(|details| format!("\nDetails: {}", details))
            .unwrap_or_default();
        ToolResponse {
            content: vec![ToolResponseContent::Text {
                text: format!("Error: {}{}", message, detail_text),
            }],
            is_error: true,
        }
    }
}

#[derive(Debug, Serialize)]
#[serde(tag = "type")]
pub enum ToolResponseContent {
    #[serde(rename = "text")]
    Text { text: String },
}

pub enum ToolCallError {
    UnknownTool(String),
}

pub fn tool_definitions() -> Vec<ToolDefinition> {
    vec![
        ToolDefinition {
            name: "session-set-defaults",
            title: "Set Session Defaults",
            description: "Set the session defaults needed by many tools. Set all relevant defaults up front.",
            input_schema: json!({
                "type": "object",
                "additionalProperties": false,
                "properties": {
                    "projectPath": { "type": "string" },
                    "workspacePath": { "type": "string" },
                    "scheme": { "type": "string" },
                    "configuration": { "type": "string" },
                    "simulatorName": { "type": "string" },
                    "simulatorId": { "type": "string" },
                    "deviceId": { "type": "string" },
                    "useLatestOS": { "type": "boolean" },
                    "arch": { "type": "string" }
                }
            }),
            annotations: Some(json!({ "destructiveHint": true })),
        },
        ToolDefinition {
            name: "session-show-defaults",
            title: "Show Session Defaults",
            description: "Show current session defaults.",
            input_schema: json!({
                "type": "object",
                "additionalProperties": false
            }),
            annotations: Some(json!({ "readOnlyHint": true })),
        },
        ToolDefinition {
            name: "session-clear-defaults",
            title: "Clear Session Defaults",
            description: "Clear selected or all session defaults.",
            input_schema: json!({
                "type": "object",
                "additionalProperties": false,
                "properties": {
                    "keys": {
                        "type": "array",
                        "items": { "type": "string", "enum": [
                            "projectPath",
                            "workspacePath",
                            "scheme",
                            "configuration",
                            "simulatorName",
                            "simulatorId",
                            "deviceId",
                            "useLatestOS",
                            "arch"
                        ]}
                    },
                    "all": { "type": "boolean" }
                }
            }),
            annotations: Some(json!({ "destructiveHint": true })),
        },
        ToolDefinition {
            name: "list_sims",
            title: "List Simulators",
            description: "Lists available iOS simulators with their UUIDs.",
            input_schema: json!({
                "type": "object",
                "additionalProperties": false,
                "properties": {
                    "availableOnly": { "type": "boolean" }
                }
            }),
            annotations: Some(json!({ "readOnlyHint": true })),
        },
        ToolDefinition {
            name: "discover_projs",
            title: "Discover Projects",
            description: "Scans a directory (defaults to workspace root) to find Xcode project (.xcodeproj) and workspace (.xcworkspace) files.",
            input_schema: json!({
                "type": "object",
                "additionalProperties": false,
                "properties": {
                    "workspaceRoot": { "type": "string" },
                    "scanPath": { "type": "string" },
                    "maxDepth": { "type": "integer", "minimum": 0 }
                },
                "required": ["workspaceRoot"]
            }),
            annotations: Some(json!({ "readOnlyHint": true })),
        },
        ToolDefinition {
            name: "list_schemes",
            title: "List Schemes",
            description: "Lists schemes for a project or workspace.",
            input_schema: json!({
                "type": "object",
                "additionalProperties": false,
                "properties": {
                    "projectPath": { "type": "string" },
                    "workspacePath": { "type": "string" }
                }
            }),
            annotations: Some(json!({ "readOnlyHint": true })),
        },
        ToolDefinition {
            name: "get_app_bundle_id",
            title: "Get App Bundle ID",
            description: "Extracts the bundle identifier from an app bundle (.app).",
            input_schema: json!({
                "type": "object",
                "additionalProperties": false,
                "properties": {
                    "appPath": { "type": "string" }
                },
                "required": ["appPath"]
            }),
            annotations: Some(json!({ "readOnlyHint": true })),
        },
        ToolDefinition {
            name: "get_sim_app_path",
            title: "Get Simulator App Path",
            description: "Retrieves the built app path for an iOS simulator.",
            input_schema: json!({
                "type": "object",
                "additionalProperties": false,
                "properties": {
                    "projectPath": { "type": "string" },
                    "workspacePath": { "type": "string" },
                    "scheme": { "type": "string" },
                    "configuration": { "type": "string" },
                    "simulatorId": { "type": "string" },
                    "simulatorName": { "type": "string" },
                    "useLatestOS": { "type": "boolean" }
                }
            }),
            annotations: Some(json!({ "readOnlyHint": true })),
        },
        ToolDefinition {
            name: "install_app_sim",
            title: "Install App Simulator",
            description: "Installs an app in an iOS simulator.",
            input_schema: json!({
                "type": "object",
                "additionalProperties": false,
                "properties": {
                    "simulatorId": { "type": "string" },
                    "simulatorName": { "type": "string" },
                    "appPath": { "type": "string" },
                    "useLatestOS": { "type": "boolean" }
                },
                "required": ["appPath"]
            }),
            annotations: Some(json!({ "destructiveHint": true })),
        },
        ToolDefinition {
            name: "launch_app_sim",
            title: "Launch App Simulator",
            description: "Launches an app in an iOS simulator.",
            input_schema: json!({
                "type": "object",
                "additionalProperties": false,
                "properties": {
                    "simulatorId": { "type": "string" },
                    "simulatorName": { "type": "string" },
                    "bundleId": { "type": "string" },
                    "args": { "type": "array", "items": { "type": "string" } },
                    "useLatestOS": { "type": "boolean" }
                },
                "required": ["bundleId"]
            }),
            annotations: Some(json!({ "destructiveHint": true })),
        },
        ToolDefinition {
            name: "boot_sim",
            title: "Boot Simulator",
            description: "Boots an iOS simulator and optionally waits for boot completion.",
            input_schema: json!({
                "type": "object",
                "additionalProperties": false,
                "properties": {
                    "simulatorId": { "type": "string" },
                    "simulatorName": { "type": "string" },
                    "useLatestOS": { "type": "boolean" },
                    "waitForBoot": { "type": "boolean" }
                }
            }),
            annotations: Some(json!({ "destructiveHint": true })),
        },
        ToolDefinition {
            name: "erase_sims",
            title: "Erase Simulator",
            description: "Erases a simulator by UDID (or name).",
            input_schema: json!({
                "type": "object",
                "additionalProperties": false,
                "properties": {
                    "simulatorId": { "type": "string" },
                    "simulatorName": { "type": "string" },
                    "useLatestOS": { "type": "boolean" },
                    "shutdownFirst": { "type": "boolean" }
                }
            }),
            annotations: Some(json!({ "destructiveHint": true })),
        },
        ToolDefinition {
            name: "start_sim_log_cap",
            title: "Start Simulator Log Capture",
            description: "Starts capturing logs from a specified simulator. Returns a session ID.",
            input_schema: json!({
                "type": "object",
                "additionalProperties": false,
                "properties": {
                    "simulatorId": { "type": "string" },
                    "simulatorName": { "type": "string" },
                    "bundleId": { "type": "string" },
                    "useLatestOS": { "type": "boolean" }
                },
                "required": ["bundleId"]
            }),
            annotations: Some(json!({ "destructiveHint": true })),
        },
        ToolDefinition {
            name: "stop_sim_log_cap",
            title: "Stop Simulator Log Capture",
            description: "Stops an active simulator log capture session and returns the captured logs.",
            input_schema: json!({
                "type": "object",
                "additionalProperties": false,
                "properties": {
                    "logSessionId": { "type": "string" }
                },
                "required": ["logSessionId"]
            }),
            annotations: Some(json!({ "destructiveHint": true })),
        },
        ToolDefinition {
            name: "build_sim",
            title: "Build Simulator",
            description: "Builds an app for an iOS simulator.",
            input_schema: json!({
                "type": "object",
                "additionalProperties": false,
                "properties": {
                    "derivedDataPath": { "type": "string" },
                    "extraArgs": { "type": "array", "items": { "type": "string" } },
                    "preferXcodebuild": { "type": "boolean" }
                }
            }),
            annotations: Some(json!({ "destructiveHint": true })),
        },
        ToolDefinition {
            name: "build_run_sim",
            title: "Build Run Simulator",
            description: "Builds and runs an app on an iOS simulator.",
            input_schema: json!({
                "type": "object",
                "additionalProperties": false,
                "properties": {
                    "projectPath": { "type": "string" },
                    "workspacePath": { "type": "string" },
                    "scheme": { "type": "string" },
                    "configuration": { "type": "string" },
                    "simulatorId": { "type": "string" },
                    "simulatorName": { "type": "string" },
                    "derivedDataPath": { "type": "string" },
                    "extraArgs": { "type": "array", "items": { "type": "string" } },
                    "useLatestOS": { "type": "boolean" },
                    "preferXcodebuild": { "type": "boolean" },
                    "bootSim": { "type": "boolean" },
                    "waitForBoot": { "type": "boolean" },
                    "bundleId": { "type": "string" },
                    "args": { "type": "array", "items": { "type": "string" } }
                }
            }),
            annotations: Some(json!({ "destructiveHint": true })),
        },
        ToolDefinition {
            name: "build_for_testing_sim",
            title: "Build for Testing (Simulator)",
            description: "Builds test artifacts for an iOS simulator.",
            input_schema: json!({
                "type": "object",
                "additionalProperties": false,
                "properties": {
                    "derivedDataPath": { "type": "string" },
                    "extraArgs": { "type": "array", "items": { "type": "string" } },
                    "testPlan": { "type": "string" }
                }
            }),
            annotations: Some(json!({ "destructiveHint": true })),
        },
        ToolDefinition {
            name: "test_without_building_sim",
            title: "Test Without Building (Simulator)",
            description: "Runs tests for an iOS simulator without rebuilding.",
            input_schema: json!({
                "type": "object",
                "additionalProperties": false,
                "properties": {
                    "derivedDataPath": { "type": "string" },
                    "extraArgs": { "type": "array", "items": { "type": "string" } },
                    "onlyTesting": { "type": "array", "items": { "type": "string" } },
                    "skipTesting": { "type": "array", "items": { "type": "string" } },
                    "testPlan": { "type": "string" },
                    "xctestrun": { "type": "string" },
                    "testRunnerEnv": {
                        "type": "object",
                        "additionalProperties": { "type": "string" }
                    }
                }
            }),
            annotations: Some(json!({ "destructiveHint": true })),
        },
        ToolDefinition {
            name: "discover_xctestrun",
            title: "Discover Xctestrun",
            description: "Finds the best matching .xctestrun file under DerivedData.",
            input_schema: json!({
                "type": "object",
                "additionalProperties": false,
                "properties": {
                    "derivedDataPath": { "type": "string" },
                    "scheme": { "type": "string" },
                    "testPlan": { "type": "string" }
                }
            }),
            annotations: Some(json!({ "readOnlyHint": true })),
        },
        ToolDefinition {
            name: "smoke_sim",
            title: "Smoke Test (Simulator)",
            description: "Runs a smoke test flow (build-for-testing + test-without-building).",
            input_schema: json!({
                "type": "object",
                "additionalProperties": false,
                "properties": {
                    "derivedDataPath": { "type": "string" },
                    "extraArgs": { "type": "array", "items": { "type": "string" } },
                    "onlyTesting": { "type": "array", "items": { "type": "string" } },
                    "skipTesting": { "type": "array", "items": { "type": "string" } },
                    "testPlan": { "type": "string" },
                    "xctestrun": { "type": "string" },
                    "testRunnerEnv": {
                        "type": "object",
                        "additionalProperties": { "type": "string" }
                    },
                    "bootSim": { "type": "boolean" },
                    "waitForBoot": { "type": "boolean" },
                    "skipBuild": { "type": "boolean" }
                }
            }),
            annotations: Some(json!({ "destructiveHint": true })),
        },
        ToolDefinition {
            name: "test_sim",
            title: "Test Simulator",
            description: "Runs tests for an iOS simulator target.",
            input_schema: json!({
                "type": "object",
                "additionalProperties": false,
                "properties": {
                    "derivedDataPath": { "type": "string" },
                    "extraArgs": { "type": "array", "items": { "type": "string" } },
                    "onlyTesting": { "type": "array", "items": { "type": "string" } },
                    "skipTesting": { "type": "array", "items": { "type": "string" } },
                    "testRunnerEnv": {
                        "type": "object",
                        "additionalProperties": { "type": "string" }
                    }
                }
            }),
            annotations: Some(json!({ "destructiveHint": true })),
        },
    ]
}

pub fn call_tool(
    name: &str,
    args: Value,
    session: &mut SessionStore,
    log_sessions: &mut LogSessionStore,
    runner: &impl Runner,
) -> Result<ToolResponse, ToolCallError> {
    let args = if args.is_null() { json!({}) } else { args };

    match name {
        "session-set-defaults" => {
            let params: SessionSetDefaultsParams = match serde_json::from_value(args) {
                Ok(params) => params,
                Err(err) => {
                    return Ok(ToolResponse::error(
                        "Parameter validation failed".to_string(),
                        Some(err.to_string()),
                    ));
                }
            };
            let result = session.set_defaults(params);
            let mut text = format!(
                "Defaults updated:\n{}",
                serde_json::to_string_pretty(&result.updated).unwrap_or_default()
            );
            if !result.notices.is_empty() {
                text.push_str("\nNotices:\n- ");
                text.push_str(&result.notices.join("\n- "));
            }
            Ok(ToolResponse::text(text, true))
        }
        "session-show-defaults" => {
            let text = serde_json::to_string_pretty(&session.get_all()).unwrap_or_default();
            Ok(ToolResponse::text(text, true))
        }
        "session-clear-defaults" => {
            let params: SessionClearDefaultsParams = match serde_json::from_value(args) {
                Ok(params) => params,
                Err(err) => {
                    return Ok(ToolResponse::error(
                        "Parameter validation failed".to_string(),
                        Some(err.to_string()),
                    ));
                }
            };
            if params.all.unwrap_or(false) || params.keys.is_none() {
                session.clear_all();
            } else if let Some(keys) = params.keys {
                session.clear_keys(&keys);
            }
            Ok(ToolResponse::text(
                "Session defaults cleared".to_string(),
                true,
            ))
        }
        "list_sims" => {
            let params: ListSimsParams = match serde_json::from_value(args) {
                Ok(params) => params,
                Err(err) => {
                    return Ok(ToolResponse::error(
                        "Parameter validation failed".to_string(),
                        Some(err.to_string()),
                    ));
                }
            };
            Ok(execute_list_sims(params, runner))
        }
        "discover_projs" => {
            let params: DiscoverProjsParamsInput = match serde_json::from_value(args) {
                Ok(params) => params,
                Err(err) => {
                    return Ok(ToolResponse::error(
                        "Parameter validation failed".to_string(),
                        Some(err.to_string()),
                    ));
                }
            };
            Ok(execute_discover_projs(params))
        }
        "list_schemes" => {
            let params: ListSchemesParamsInput = match serde_json::from_value(args) {
                Ok(params) => params,
                Err(err) => {
                    return Ok(ToolResponse::error(
                        "Parameter validation failed".to_string(),
                        Some(err.to_string()),
                    ));
                }
            };
            let merged = match list_schemes_from_input(params, &session.get_all()) {
                Ok(merged) => merged,
                Err(err) => {
                    return Ok(ToolResponse::error(
                        "Parameter validation failed".to_string(),
                        Some(err),
                    ));
                }
            };
            Ok(execute_list_schemes(merged, runner))
        }
        "get_app_bundle_id" => {
            let params: GetAppBundleIdParamsInput = match serde_json::from_value(args) {
                Ok(params) => params,
                Err(err) => {
                    return Ok(ToolResponse::error(
                        "Parameter validation failed".to_string(),
                        Some(err.to_string()),
                    ));
                }
            };
            let merged = match get_app_bundle_id_from_input(params) {
                Ok(merged) => merged,
                Err(err) => {
                    return Ok(ToolResponse::error(
                        "Parameter validation failed".to_string(),
                        Some(err),
                    ));
                }
            };
            Ok(execute_get_app_bundle_id(merged, runner))
        }
        "get_sim_app_path" => {
            let params: GetSimAppPathParamsInput = match serde_json::from_value(args) {
                Ok(params) => params,
                Err(err) => {
                    return Ok(ToolResponse::error(
                        "Parameter validation failed".to_string(),
                        Some(err.to_string()),
                    ));
                }
            };
            let merged = match get_sim_app_path_from_input(params, &session.get_all()) {
                Ok(merged) => merged,
                Err(err) => {
                    return Ok(ToolResponse::error(
                        "Parameter validation failed".to_string(),
                        Some(err),
                    ));
                }
            };
            Ok(execute_get_sim_app_path(merged, runner))
        }
        "install_app_sim" => {
            let params: InstallAppSimParamsInput = match serde_json::from_value(args) {
                Ok(params) => params,
                Err(err) => {
                    return Ok(ToolResponse::error(
                        "Parameter validation failed".to_string(),
                        Some(err.to_string()),
                    ));
                }
            };
            let merged = match install_app_sim_from_input(params, &session.get_all()) {
                Ok(merged) => merged,
                Err(err) => {
                    return Ok(ToolResponse::error(
                        "Parameter validation failed".to_string(),
                        Some(err),
                    ));
                }
            };
            Ok(execute_install_app_sim(merged, runner))
        }
        "launch_app_sim" => {
            let params: LaunchAppSimParamsInput = match serde_json::from_value(args) {
                Ok(params) => params,
                Err(err) => {
                    return Ok(ToolResponse::error(
                        "Parameter validation failed".to_string(),
                        Some(err.to_string()),
                    ));
                }
            };
            let merged = match launch_app_sim_from_input(params, &session.get_all()) {
                Ok(merged) => merged,
                Err(err) => {
                    return Ok(ToolResponse::error(
                        "Parameter validation failed".to_string(),
                        Some(err),
                    ));
                }
            };
            Ok(execute_launch_app_sim(merged, runner))
        }
        "start_sim_log_cap" => {
            let params: StartSimLogCapParamsInput = match serde_json::from_value(args) {
                Ok(params) => params,
                Err(err) => {
                    return Ok(ToolResponse::error(
                        "Parameter validation failed".to_string(),
                        Some(err.to_string()),
                    ));
                }
            };
            let merged = match start_sim_log_cap_from_input(params, &session.get_all()) {
                Ok(merged) => merged,
                Err(err) => {
                    return Ok(ToolResponse::error(
                        "Parameter validation failed".to_string(),
                        Some(err),
                    ));
                }
            };
            Ok(execute_start_sim_log_cap(merged, log_sessions, runner))
        }
        "stop_sim_log_cap" => {
            let params: StopSimLogCapParamsInput = match serde_json::from_value(args) {
                Ok(params) => params,
                Err(err) => {
                    return Ok(ToolResponse::error(
                        "Parameter validation failed".to_string(),
                        Some(err.to_string()),
                    ));
                }
            };
            let merged = match stop_sim_log_cap_from_input(params) {
                Ok(merged) => merged,
                Err(err) => {
                    return Ok(ToolResponse::error(
                        "Parameter validation failed".to_string(),
                        Some(err),
                    ));
                }
            };
            Ok(execute_stop_sim_log_cap(merged, log_sessions))
        }
        "boot_sim" => {
            let params: BootSimParamsInput = match serde_json::from_value(args) {
                Ok(params) => params,
                Err(err) => {
                    return Ok(ToolResponse::error(
                        "Parameter validation failed".to_string(),
                        Some(err.to_string()),
                    ));
                }
            };
            let merged = match boot_sim_from_input(params, &session.get_all()) {
                Ok(merged) => merged,
                Err(err) => {
                    return Ok(ToolResponse::error(
                        "Parameter validation failed".to_string(),
                        Some(err),
                    ));
                }
            };
            Ok(execute_boot_sim(merged, runner))
        }
        "erase_sims" => {
            let params: EraseSimsParamsInput = match serde_json::from_value(args) {
                Ok(params) => params,
                Err(err) => {
                    return Ok(ToolResponse::error(
                        "Parameter validation failed".to_string(),
                        Some(err.to_string()),
                    ));
                }
            };
            let merged = match erase_sims_from_input(params, &session.get_all()) {
                Ok(merged) => merged,
                Err(err) => {
                    return Ok(ToolResponse::error(
                        "Parameter validation failed".to_string(),
                        Some(err),
                    ));
                }
            };
            Ok(execute_erase_sims(merged, runner))
        }
        "build_sim" => {
            let params: BuildSimParamsInput = match serde_json::from_value(args) {
                Ok(params) => params,
                Err(err) => {
                    return Ok(ToolResponse::error(
                        "Parameter validation failed".to_string(),
                        Some(err.to_string()),
                    ));
                }
            };
            let merged = match build_sim_from_input(params, &session.get_all()) {
                Ok(merged) => merged,
                Err(err) => {
                    return Ok(ToolResponse::error(
                        "Parameter validation failed".to_string(),
                        Some(err),
                    ));
                }
            };
            Ok(execute_build_sim(merged, runner))
        }
        "build_run_sim" => {
            let params: BuildRunSimParamsInput = match serde_json::from_value(args) {
                Ok(params) => params,
                Err(err) => {
                    return Ok(ToolResponse::error(
                        "Parameter validation failed".to_string(),
                        Some(err.to_string()),
                    ));
                }
            };
            let merged = match build_run_sim_from_input(params, &session.get_all()) {
                Ok(merged) => merged,
                Err(err) => {
                    return Ok(ToolResponse::error(
                        "Parameter validation failed".to_string(),
                        Some(err),
                    ));
                }
            };
            Ok(execute_build_run_sim(merged, runner))
        }
        "build_for_testing_sim" => {
            let params: BuildForTestingSimParamsInput = match serde_json::from_value(args) {
                Ok(params) => params,
                Err(err) => {
                    return Ok(ToolResponse::error(
                        "Parameter validation failed".to_string(),
                        Some(err.to_string()),
                    ));
                }
            };
            let merged = match build_for_testing_sim_from_input(params, &session.get_all()) {
                Ok(merged) => merged,
                Err(err) => {
                    return Ok(ToolResponse::error(
                        "Parameter validation failed".to_string(),
                        Some(err),
                    ));
                }
            };
            Ok(execute_build_for_testing_sim(merged, runner))
        }
        "test_without_building_sim" => {
            let params: TestWithoutBuildingSimParamsInput = match serde_json::from_value(args) {
                Ok(params) => params,
                Err(err) => {
                    return Ok(ToolResponse::error(
                        "Parameter validation failed".to_string(),
                        Some(err.to_string()),
                    ));
                }
            };
            let merged = match test_without_building_sim_from_input(params, &session.get_all()) {
                Ok(merged) => merged,
                Err(err) => {
                    return Ok(ToolResponse::error(
                        "Parameter validation failed".to_string(),
                        Some(err),
                    ));
                }
            };
            Ok(execute_test_without_building_sim(merged, runner))
        }
        "discover_xctestrun" => {
            let params: DiscoverXctestrunParamsInput = match serde_json::from_value(args) {
                Ok(params) => params,
                Err(err) => {
                    return Ok(ToolResponse::error(
                        "Parameter validation failed".to_string(),
                        Some(err.to_string()),
                    ));
                }
            };
            Ok(execute_discover_xctestrun(params))
        }
        "smoke_sim" => {
            let params: SmokeSimParamsInput = match serde_json::from_value(args) {
                Ok(params) => params,
                Err(err) => {
                    return Ok(ToolResponse::error(
                        "Parameter validation failed".to_string(),
                        Some(err.to_string()),
                    ));
                }
            };
            let merged = match smoke_sim_from_input(params, &session.get_all()) {
                Ok(merged) => merged,
                Err(err) => {
                    return Ok(ToolResponse::error(
                        "Parameter validation failed".to_string(),
                        Some(err),
                    ));
                }
            };
            Ok(execute_smoke_sim(merged, runner))
        }
        "test_sim" => {
            let params: TestSimParamsInput = match serde_json::from_value(args) {
                Ok(params) => params,
                Err(err) => {
                    return Ok(ToolResponse::error(
                        "Parameter validation failed".to_string(),
                        Some(err.to_string()),
                    ));
                }
            };
            let merged = match test_sim_from_input(params, &session.get_all()) {
                Ok(merged) => merged,
                Err(err) => {
                    return Ok(ToolResponse::error(
                        "Parameter validation failed".to_string(),
                        Some(err),
                    ));
                }
            };
            Ok(execute_test_sim(merged, runner))
        }
        _ => Err(ToolCallError::UnknownTool(name.to_string())),
    }
}

pub fn session_keys() -> Vec<&'static str> {
    vec![
        "projectPath",
        "workspacePath",
        "scheme",
        "configuration",
        "simulatorName",
        "simulatorId",
        "deviceId",
        "useLatestOS",
        "arch",
    ]
}
