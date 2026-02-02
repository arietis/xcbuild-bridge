use serde::Deserialize;
use crate::app_bundle::{
    extract_app_path_from_build_settings, read_bundle_id as read_bundle_id_from_app,
    validate_app_path,
};
use crate::build_sim::{BuildSimParams, build_sim_command};
use crate::exec::{CommandSpec, Runner};
use crate::session::SessionDefaults;
use crate::simctl::{boot_simulator, resolve_simulator_id};
use crate::test_support::render_command_output;
use crate::tools::ToolResponse;

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BuildRunSimParamsInput {
    pub project_path: Option<String>,
    pub workspace_path: Option<String>,
    pub scheme: Option<String>,
    pub configuration: Option<String>,
    pub simulator_id: Option<String>,
    pub simulator_name: Option<String>,
    pub derived_data_path: Option<String>,
    pub extra_args: Option<Vec<String>>,
    pub use_latest_os: Option<bool>,
    pub prefer_xcodebuild: Option<bool>,
    pub boot_sim: Option<bool>,
    pub wait_for_boot: Option<bool>,
    pub bundle_id: Option<String>,
    pub args: Option<Vec<String>>,
}

#[derive(Debug, Clone)]
pub struct BuildRunSimParams {
    pub project_path: Option<String>,
    pub workspace_path: Option<String>,
    pub scheme: String,
    pub configuration: String,
    pub simulator_id: Option<String>,
    pub simulator_name: Option<String>,
    pub derived_data_path: Option<String>,
    pub extra_args: Vec<String>,
    pub use_latest_os: bool,
    pub prefer_xcodebuild: bool,
    pub boot_sim: bool,
    pub wait_for_boot: bool,
    pub bundle_id: Option<String>,
    pub args: Vec<String>,
}

pub fn build_run_sim_from_input(
    input: BuildRunSimParamsInput,
    defaults: &SessionDefaults,
) -> Result<BuildRunSimParams, String> {
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
    let prefer_xcodebuild = input.prefer_xcodebuild.unwrap_or(false);
    let boot_sim = input.boot_sim.unwrap_or(true);
    let wait_for_boot = input.wait_for_boot.unwrap_or(true);
    let bundle_id = normalize_opt(input.bundle_id);
    let args = normalize_vec(input.args);

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

    Ok(BuildRunSimParams {
        project_path,
        workspace_path,
        scheme,
        configuration,
        simulator_id,
        simulator_name,
        derived_data_path,
        extra_args,
        use_latest_os,
        prefer_xcodebuild,
        boot_sim,
        wait_for_boot,
        bundle_id,
        args,
    })
}

pub fn execute_build_run_sim(params: BuildRunSimParams, runner: &impl Runner) -> ToolResponse {
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

    if params.boot_sim {
        if let Err(err) = boot_simulator(&simulator_id, params.wait_for_boot, runner) {
            return ToolResponse::error("Boot simulator operation failed".to_string(), Some(err));
        }
    }

    let build_params = BuildSimParams {
        project_path: params.project_path.clone(),
        workspace_path: params.workspace_path.clone(),
        scheme: params.scheme.clone(),
        configuration: params.configuration.clone(),
        simulator_id: Some(simulator_id.clone()),
        simulator_name: None,
        derived_data_path: params.derived_data_path.clone(),
        extra_args: params.extra_args.clone(),
        use_latest_os: params.use_latest_os,
        prefer_xcodebuild: params.prefer_xcodebuild,
    };

    let build_spec = build_sim_command(&build_params);
    let build_output = match runner.run(&build_spec) {
        Ok(output) => output,
        Err(err) => {
            return ToolResponse::error("Build failed".to_string(), Some(err.to_string()));
        }
    };
    if build_output.exit_code != 0 {
        return ToolResponse::text(render_command_output(&build_output), false);
    }

    let app_path = match fetch_app_path(&params, &simulator_id, runner) {
        Ok(path) => path,
        Err(err) => {
            return ToolResponse::error("Failed to get app path".to_string(), Some(err));
        }
    };

    if let Err(err) = validate_app_path(&app_path) {
        return ToolResponse::error("Invalid app path".to_string(), Some(err));
    }

    if let Err(err) = install_app(&simulator_id, &app_path, runner) {
        return ToolResponse::error("Install app failed".to_string(), Some(err));
    }

    let bundle_id = match params.bundle_id.clone() {
        Some(bundle_id) => bundle_id,
        None => match read_bundle_id_from_app(&app_path, runner) {
            Ok(bundle_id) => bundle_id,
            Err(err) => {
                return ToolResponse::error(
                    "Failed to extract bundle ID".to_string(),
                    Some(err),
                );
            }
        },
    };

    if let Err(err) = launch_app(&simulator_id, &bundle_id, &params.args, runner) {
        return ToolResponse::error("Launch app failed".to_string(), Some(err));
    }

    let mut lines = Vec::new();
    lines.push(format!(
        "✅ Build and run succeeded on simulator {}",
        simulator_id
    ));
    lines.push(format!("App path: {}", app_path));
    lines.push(format!("Bundle ID: {}", bundle_id));
    lines.push("Next steps:".to_string());
    lines.push("1. If the simulator UI is hidden, open the Simulator app.".to_string());

    ToolResponse::text(lines.join("\n"), true)
}

fn fetch_app_path(
    params: &BuildRunSimParams,
    simulator_id: &str,
    runner: &impl Runner,
) -> Result<String, String> {
    let mut args = Vec::new();
    args.push("-showBuildSettings".to_string());

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
    args.push("-destination".to_string());
    args.push(format!("id={}", simulator_id));

    if let Some(derived_data_path) = &params.derived_data_path {
        args.push("-derivedDataPath".to_string());
        args.push(derived_data_path.clone());
    }

    args.extend(params.extra_args.clone());

    let spec = CommandSpec {
        program: "xcodebuild".to_string(),
        args,
        cwd: None,
        env: None,
    };
    let output = runner.run(&spec).map_err(|err| err.to_string())?;
    if output.exit_code != 0 {
        return Err(render_command_output(&output));
    }

    extract_app_path_from_build_settings(&output.stdout)
        .ok_or_else(|| "Failed to extract app path from build settings".to_string())
}

fn install_app(simulator_id: &str, app_path: &str, runner: &impl Runner) -> Result<(), String> {
    let spec = CommandSpec {
        program: "xcrun".to_string(),
        args: vec![
            "simctl".to_string(),
            "install".to_string(),
            simulator_id.to_string(),
            app_path.to_string(),
        ],
        cwd: None,
        env: None,
    };
    let output = runner.run(&spec).map_err(|err| err.to_string())?;
    if output.exit_code != 0 {
        return Err(render_command_output(&output));
    }
    Ok(())
}


fn launch_app(
    simulator_id: &str,
    bundle_id: &str,
    args: &[String],
    runner: &impl Runner,
) -> Result<(), String> {
    let mut command_args = vec![
        "simctl".to_string(),
        "launch".to_string(),
        simulator_id.to_string(),
        bundle_id.to_string(),
    ];
    command_args.extend(args.iter().cloned());
    let spec = CommandSpec {
        program: "xcrun".to_string(),
        args: command_args,
        cwd: None,
        env: None,
    };
    let output = runner.run(&spec).map_err(|err| err.to_string())?;
    if output.exit_code != 0 {
        return Err(render_command_output(&output));
    }
    Ok(())
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
    use crate::app_bundle::extract_app_path_from_build_settings;

    #[test]
    fn extract_app_path_finds_settings() {
        let stdout = "\
Build settings for action build and target App:
    BUILT_PRODUCTS_DIR = /tmp/DerivedData/Build/Products/Debug-iphonesimulator
    FULL_PRODUCT_NAME = App.app
";
        let path = extract_app_path_from_build_settings(stdout).expect("path");
        assert_eq!(
            path,
            "/tmp/DerivedData/Build/Products/Debug-iphonesimulator/App.app"
        );
    }

    #[test]
    fn build_run_sim_requires_scheme() {
        let defaults = SessionDefaults::default();
        let result = build_run_sim_from_input(
            BuildRunSimParamsInput {
                project_path: Some("App.xcodeproj".to_string()),
                workspace_path: None,
                scheme: None,
                configuration: None,
                simulator_id: Some("SIM".to_string()),
                simulator_name: None,
                derived_data_path: None,
                extra_args: None,
                use_latest_os: None,
                prefer_xcodebuild: None,
                boot_sim: None,
                wait_for_boot: None,
                bundle_id: None,
                args: None,
            },
            &defaults,
        );
        assert!(result.is_err());
    }
}
