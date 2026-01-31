use serde::Deserialize;
use std::collections::HashMap;

use crate::build_for_testing_sim::{BuildForTestingSimParams, execute_build_for_testing_sim};
use crate::discover_xctestrun::discover_xctestrun_path;
use crate::session::SessionDefaults;
use crate::simctl::{boot_simulator, resolve_simulator_id};
use crate::test_support::normalize_env;
use crate::test_without_building_sim::{
    TestWithoutBuildingSimParams, execute_test_without_building_sim,
};
use crate::tools::ToolResponse;

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SmokeSimParamsInput {
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
    pub boot_sim: Option<bool>,
    pub wait_for_boot: Option<bool>,
    pub skip_build: Option<bool>,
}

#[derive(Debug, Clone)]
pub struct SmokeSimParams {
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
    pub boot_sim: bool,
    pub wait_for_boot: bool,
    pub skip_build: bool,
}

pub fn smoke_sim_from_input(
    input: SmokeSimParamsInput,
    defaults: &SessionDefaults,
) -> Result<SmokeSimParams, String> {
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
    let boot_sim = input.boot_sim.unwrap_or(true);
    let wait_for_boot = input.wait_for_boot.unwrap_or(true);
    let skip_build = input.skip_build.unwrap_or(false);

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

    if !skip_build && scheme.is_none() {
        return Err("scheme is required when skipBuild is false.".to_string());
    }
    Ok(SmokeSimParams {
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
        boot_sim,
        wait_for_boot,
        skip_build,
    })
}

pub fn execute_smoke_sim(
    params: SmokeSimParams,
    runner: &impl crate::exec::Runner,
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

    if params.boot_sim
        && let Err(err) = boot_simulator(&simulator_id, params.wait_for_boot, runner)
    {
        return ToolResponse::error("Boot simulator operation failed".to_string(), Some(err));
    }

    let mut sections = Vec::new();
    let mut resolved_xctestrun = params.xctestrun.clone();
    let mut discovery_error = None;

    if resolved_xctestrun.is_none() {
        match discover_xctestrun_path(
            params.derived_data_path.as_deref(),
            params.scheme.as_deref(),
            params.test_plan.as_deref(),
        ) {
            Ok(path) => {
                resolved_xctestrun = Some(path.to_string_lossy().to_string());
            }
            Err(err) => {
                discovery_error = Some(err);
            }
        }
    }

    if params.skip_build && params.scheme.is_none() && resolved_xctestrun.is_none() {
        return ToolResponse::error(
            "xctestrun is required when skipBuild is true and scheme is missing.".to_string(),
            discovery_error,
        );
    }

    if !params.skip_build {
        let build_params = BuildForTestingSimParams {
            project_path: params.project_path.clone(),
            workspace_path: params.workspace_path.clone(),
            scheme: params.scheme.clone().unwrap_or_default(),
            configuration: params.configuration.clone(),
            simulator_id: Some(simulator_id.clone()),
            simulator_name: None,
            derived_data_path: params.derived_data_path.clone(),
            extra_args: params.extra_args.clone(),
            use_latest_os: params.use_latest_os,
            test_plan: params.test_plan.clone(),
        };
        let build_response = execute_build_for_testing_sim(build_params, runner);
        sections.push(format_section("build_for_testing_sim", &build_response));
        if build_response.is_error {
            return ToolResponse::text(sections.join("\n\n"), false);
        }
    }

    let test_params = TestWithoutBuildingSimParams {
        project_path: params.project_path.clone(),
        workspace_path: params.workspace_path.clone(),
        scheme: params.scheme.clone(),
        configuration: params.configuration.clone(),
        simulator_id: Some(simulator_id),
        simulator_name: None,
        derived_data_path: params.derived_data_path.clone(),
        extra_args: params.extra_args.clone(),
        use_latest_os: params.use_latest_os,
        only_testing: params.only_testing.clone(),
        skip_testing: params.skip_testing.clone(),
        test_plan: params.test_plan.clone(),
        xctestrun: resolved_xctestrun,
        test_runner_env: params.test_runner_env.clone(),
    };
    let test_response = execute_test_without_building_sim(test_params, runner);
    sections.push(format_section("test_without_building_sim", &test_response));

    ToolResponse::text(sections.join("\n\n"), !test_response.is_error)
}

fn format_section(title: &str, response: &ToolResponse) -> String {
    format!("=== {} ===\n{}", title, response_text(response))
}

fn response_text(response: &ToolResponse) -> String {
    response
        .content
        .iter()
        .map(|content| match content {
            crate::tools::ToolResponseContent::Text { text } => text.as_str(),
        })
        .collect::<Vec<_>>()
        .join("\n")
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
    fn smoke_sim_requires_scheme_when_building() {
        let defaults = SessionDefaults::default();
        let input = SmokeSimParamsInput {
            project_path: Some("App.xcodeproj".to_string()),
            workspace_path: None,
            scheme: None,
            configuration: None,
            simulator_id: Some("SIM-UUID".to_string()),
            simulator_name: None,
            derived_data_path: None,
            extra_args: None,
            use_latest_os: None,
            only_testing: None,
            skip_testing: None,
            test_plan: None,
            xctestrun: None,
            test_runner_env: None,
            boot_sim: None,
            wait_for_boot: None,
            skip_build: Some(false),
        };

        let err = smoke_sim_from_input(input, &defaults).unwrap_err();
        assert!(err.contains("scheme"));
    }

    #[test]
    fn smoke_sim_allows_skip_build_with_xctestrun() {
        let defaults = SessionDefaults::default();
        let input = SmokeSimParamsInput {
            project_path: Some("App.xcodeproj".to_string()),
            workspace_path: None,
            scheme: None,
            configuration: None,
            simulator_id: Some("SIM-UUID".to_string()),
            simulator_name: None,
            derived_data_path: None,
            extra_args: None,
            use_latest_os: None,
            only_testing: None,
            skip_testing: None,
            test_plan: None,
            xctestrun: Some("AppTests.xctestrun".to_string()),
            test_runner_env: None,
            boot_sim: None,
            wait_for_boot: None,
            skip_build: Some(true),
        };

        let params = smoke_sim_from_input(input, &defaults).expect("valid");
        assert!(params.skip_build);
    }

    #[test]
    fn smoke_sim_allows_skip_build_without_xctestrun() {
        let defaults = SessionDefaults::default();
        let input = SmokeSimParamsInput {
            project_path: Some("App.xcodeproj".to_string()),
            workspace_path: None,
            scheme: None,
            configuration: None,
            simulator_id: Some("SIM-UUID".to_string()),
            simulator_name: None,
            derived_data_path: None,
            extra_args: None,
            use_latest_os: None,
            only_testing: None,
            skip_testing: None,
            test_plan: None,
            xctestrun: None,
            test_runner_env: None,
            boot_sim: None,
            wait_for_boot: None,
            skip_build: Some(true),
        };

        let params = smoke_sim_from_input(input, &defaults).expect("valid");
        assert!(params.skip_build);
    }
}
