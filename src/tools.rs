use serde::Serialize;
use serde_json::{Value, json};

use crate::boot_sim::{BootSimParamsInput, boot_sim_from_input, execute_boot_sim};
use crate::build_for_testing_sim::{
    BuildForTestingSimParamsInput, build_for_testing_sim_from_input, execute_build_for_testing_sim,
};
use crate::build_sim::{BuildSimParamsInput, build_sim_from_input, execute_build_sim};
use crate::discover_xctestrun::{DiscoverXctestrunParamsInput, execute_discover_xctestrun};
use crate::erase_sims::{EraseSimsParamsInput, erase_sims_from_input, execute_erase_sims};
use crate::exec::Runner;
use crate::list_sims::{ListSimsParams, execute_list_sims};
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
