use serde::Deserialize;

use crate::exec::{CommandSpec, Runner};
use crate::session::SessionDefaults;
use crate::test_support::render_command_output;
use crate::tools::ToolResponse;

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BuildForTestingSimParamsInput {
    pub project_path: Option<String>,
    pub workspace_path: Option<String>,
    pub scheme: Option<String>,
    pub configuration: Option<String>,
    pub simulator_id: Option<String>,
    pub simulator_name: Option<String>,
    pub derived_data_path: Option<String>,
    pub extra_args: Option<Vec<String>>,
    pub use_latest_os: Option<bool>,
    pub test_plan: Option<String>,
}

#[derive(Debug, Clone)]
pub struct BuildForTestingSimParams {
    pub project_path: Option<String>,
    pub workspace_path: Option<String>,
    pub scheme: String,
    pub configuration: String,
    pub simulator_id: Option<String>,
    pub simulator_name: Option<String>,
    pub derived_data_path: Option<String>,
    pub extra_args: Vec<String>,
    pub use_latest_os: bool,
    pub test_plan: Option<String>,
}

pub fn build_for_testing_sim_from_input(
    input: BuildForTestingSimParamsInput,
    defaults: &SessionDefaults,
) -> Result<BuildForTestingSimParams, String> {
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
    let test_plan = normalize_opt(input.test_plan);

    let scheme = scheme.ok_or_else(|| "scheme is required".to_string())?;

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

    Ok(BuildForTestingSimParams {
        project_path,
        workspace_path,
        scheme,
        configuration,
        simulator_id,
        simulator_name,
        derived_data_path,
        extra_args,
        use_latest_os,
        test_plan,
    })
}

pub fn build_for_testing_sim_command(params: &BuildForTestingSimParams) -> CommandSpec {
    let mut args = Vec::new();

    if let Some(workspace) = &params.workspace_path {
        args.push("-workspace".to_string());
        args.push(workspace.clone());
    } else if let Some(project) = &params.project_path {
        args.push("-project".to_string());
        args.push(project.clone());
    }

    args.push("-scheme".to_string());
    args.push(params.scheme.clone());

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

    args.extend(params.extra_args.clone());
    args.push("build-for-testing".to_string());

    CommandSpec {
        program: "xcodebuild".to_string(),
        args,
        cwd: None,
        env: None,
    }
}

pub fn execute_build_for_testing_sim(
    params: BuildForTestingSimParams,
    runner: &impl Runner,
) -> ToolResponse {
    let spec = build_for_testing_sim_command(&params);
    match runner.run(&spec) {
        Ok(output) => ToolResponse::text(render_command_output(&output), output.exit_code == 0),
        Err(err) => ToolResponse::error("Command failed".to_string(), Some(err.to_string())),
    }
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
    fn build_for_testing_command_adds_action() {
        let params = BuildForTestingSimParams {
            project_path: Some("App.xcodeproj".to_string()),
            workspace_path: None,
            scheme: "App".to_string(),
            configuration: "Debug".to_string(),
            simulator_id: Some("SIM-UUID".to_string()),
            simulator_name: None,
            derived_data_path: None,
            extra_args: Vec::new(),
            use_latest_os: true,
            test_plan: None,
        };

        let spec = build_for_testing_sim_command(&params);
        assert_eq!(spec.args.last(), Some(&"build-for-testing".to_string()));
    }
}
