use serde::Deserialize;
use std::collections::HashMap;
use std::path::Path;

use crate::exec::{CommandSpec, Runner};
use crate::session::SessionDefaults;
use crate::test_support::{
    normalize_env, parse_xcresult_bundle, prefix_test_runner_env, render_command_output,
    resolve_result_bundle,
};
use crate::tools::ToolResponse;

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TestWithoutBuildingSimParamsInput {
    pub project_path: Option<String>,
    pub workspace_path: Option<String>,
    pub scheme: Option<String>,
    pub configuration: Option<String>,
    pub simulator_id: Option<String>,
    pub simulator_name: Option<String>,
    pub derived_data_path: Option<String>,
    pub extra_args: Option<Vec<String>>,
    pub use_latest_os: Option<bool>,
    pub only_testing: Option<Vec<String>>,
    pub skip_testing: Option<Vec<String>>,
    pub test_plan: Option<String>,
    pub xctestrun: Option<String>,
    pub test_runner_env: Option<HashMap<String, String>>,
}

#[derive(Debug, Clone)]
pub struct TestWithoutBuildingSimParams {
    pub project_path: Option<String>,
    pub workspace_path: Option<String>,
    pub scheme: Option<String>,
    pub configuration: String,
    pub simulator_id: Option<String>,
    pub simulator_name: Option<String>,
    pub derived_data_path: Option<String>,
    pub extra_args: Vec<String>,
    pub use_latest_os: bool,
    pub only_testing: Vec<String>,
    pub skip_testing: Vec<String>,
    pub test_plan: Option<String>,
    pub xctestrun: Option<String>,
    pub test_runner_env: Option<HashMap<String, String>>,
}

pub fn test_without_building_sim_from_input(
    input: TestWithoutBuildingSimParamsInput,
    defaults: &SessionDefaults,
) -> Result<TestWithoutBuildingSimParams, String> {
    let project_path = normalize_opt(input.project_path).or_else(|| defaults.project_path.clone());
    let workspace_path =
        normalize_opt(input.workspace_path).or_else(|| defaults.workspace_path.clone());
    let scheme = normalize_opt(input.scheme).or_else(|| defaults.scheme.clone());
    let configuration = normalize_opt(input.configuration)
        .or_else(|| defaults.configuration.clone())
        .unwrap_or_else(|| "Debug".to_string());
    let simulator_id = normalize_opt(input.simulator_id).or_else(|| defaults.simulator_id.clone());
    let simulator_name =
        normalize_opt(input.simulator_name).or_else(|| defaults.simulator_name.clone());
    let derived_data_path = normalize_opt(input.derived_data_path);
    let extra_args = normalize_vec(input.extra_args);
    let use_latest_os = input
        .use_latest_os
        .or(defaults.use_latest_os)
        .unwrap_or(true);
    let only_testing = normalize_vec(input.only_testing);
    let skip_testing = normalize_vec(input.skip_testing);
    let test_plan = normalize_opt(input.test_plan);
    let xctestrun = normalize_opt(input.xctestrun);
    let test_runner_env = normalize_env(input.test_runner_env);

    if scheme.is_none() && xctestrun.is_none() {
        return Err("scheme is required unless xctestrun is provided.".to_string());
    }

    if project_path.is_none() && workspace_path.is_none() {
        return Err("Either projectPath or workspacePath is required.".to_string());
    }
    if project_path.is_some() && workspace_path.is_some() {
        return Err("projectPath and workspacePath are mutually exclusive.".to_string());
    }
    if simulator_id.is_none() && simulator_name.is_none() {
        return Err("Either simulatorId or simulatorName is required.".to_string());
    }
    if simulator_id.is_some() && simulator_name.is_some() {
        return Err("simulatorId and simulatorName are mutually exclusive.".to_string());
    }

    Ok(TestWithoutBuildingSimParams {
        project_path,
        workspace_path,
        scheme,
        configuration,
        simulator_id,
        simulator_name,
        derived_data_path,
        extra_args,
        use_latest_os,
        only_testing,
        skip_testing,
        test_plan,
        xctestrun,
        test_runner_env,
    })
}

pub fn test_without_building_sim_command(
    params: &TestWithoutBuildingSimParams,
    result_bundle_path: Option<&Path>,
    include_result_bundle: bool,
) -> CommandSpec {
    let mut args = Vec::new();

    if let Some(workspace) = &params.workspace_path {
        args.push("-workspace".to_string());
        args.push(workspace.clone());
    } else if let Some(project) = &params.project_path {
        args.push("-project".to_string());
        args.push(project.clone());
    }

    if let Some(scheme) = &params.scheme {
        args.push("-scheme".to_string());
        args.push(scheme.clone());
    }

    args.push("-configuration".to_string());
    args.push(params.configuration.clone());

    let destination = if let Some(simulator_id) = &params.simulator_id {
        format!("id={}", simulator_id)
    } else {
        let name = params
            .simulator_name
            .clone()
            .unwrap_or_else(|| "iPhone".to_string());
        if params.use_latest_os {
            format!("platform=iOS Simulator,name={},OS=latest", name)
        } else {
            format!("platform=iOS Simulator,name={}", name)
        }
    };

    args.push("-destination".to_string());
    args.push(destination);

    if let Some(derived_data_path) = &params.derived_data_path {
        args.push("-derivedDataPath".to_string());
        args.push(derived_data_path.clone());
    }

    if let Some(test_plan) = &params.test_plan {
        args.push("-testPlan".to_string());
        args.push(test_plan.clone());
    }

    if let Some(xctestrun) = &params.xctestrun {
        args.push("-xctestrun".to_string());
        args.push(xctestrun.clone());
    }

    if include_result_bundle && let Some(result_bundle_path) = result_bundle_path {
        args.push("-resultBundlePath".to_string());
        args.push(result_bundle_path.to_string_lossy().to_string());
    }

    for entry in &params.only_testing {
        args.push("-only-testing".to_string());
        args.push(entry.clone());
    }

    for entry in &params.skip_testing {
        args.push("-skip-testing".to_string());
        args.push(entry.clone());
    }

    args.extend(params.extra_args.clone());
    args.push("test-without-building".to_string());

    let env = params.test_runner_env.as_ref().map(prefix_test_runner_env);

    CommandSpec {
        program: "xcodebuild".to_string(),
        args,
        cwd: None,
        env,
    }
}

pub fn execute_test_without_building_sim(
    params: TestWithoutBuildingSimParams,
    runner: &impl Runner,
) -> ToolResponse {
    let result_bundle = match resolve_result_bundle(&params.extra_args) {
        Ok(bundle) => bundle,
        Err(err) => {
            return ToolResponse::error(
                "Failed to prepare test result bundle".to_string(),
                Some(err),
            );
        }
    };

    let result_bundle_path = result_bundle.path();
    let include_result_bundle = result_bundle.include_in_command();

    let spec =
        test_without_building_sim_command(&params, Some(result_bundle_path), include_result_bundle);
    let output = match runner.run(&spec) {
        Ok(output) => output,
        Err(err) => {
            result_bundle.cleanup();
            return ToolResponse::error("Command failed".to_string(), Some(err.to_string()));
        }
    };

    let mut text = render_command_output(&output);
    if let Ok(summary) = parse_xcresult_bundle(result_bundle_path, runner) {
        text.push_str("\n\nTest Results Summary:\n");
        text.push_str(&summary);
    }

    result_bundle.cleanup();

    ToolResponse::text(text, output.exit_code == 0)
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
    fn test_without_building_command_includes_xctestrun() {
        let params = TestWithoutBuildingSimParams {
            project_path: Some("App.xcodeproj".to_string()),
            workspace_path: None,
            scheme: Some("App".to_string()),
            configuration: "Debug".to_string(),
            simulator_id: Some("SIM-UUID".to_string()),
            simulator_name: None,
            derived_data_path: None,
            extra_args: Vec::new(),
            use_latest_os: true,
            only_testing: Vec::new(),
            skip_testing: Vec::new(),
            test_plan: None,
            xctestrun: Some("AppTests.xctestrun".to_string()),
            test_runner_env: None,
        };

        let spec = test_without_building_sim_command(&params, None, false);
        assert!(spec.args.iter().any(|arg| arg == "-xctestrun"));
        assert!(spec.args.iter().any(|arg| arg == "AppTests.xctestrun"));
        assert_eq!(spec.args.last(), Some(&"test-without-building".to_string()));
    }
}
