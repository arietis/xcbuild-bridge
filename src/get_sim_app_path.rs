use serde::Deserialize;

use crate::exec::{CommandSpec, Runner};
use crate::session::SessionDefaults;
use crate::test_support::render_command_output;
use crate::tools::ToolResponse;

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GetSimAppPathParamsInput {
    pub project_path: Option<String>,
    pub workspace_path: Option<String>,
    pub scheme: Option<String>,
    pub configuration: Option<String>,
    pub simulator_id: Option<String>,
    pub simulator_name: Option<String>,
    pub use_latest_os: Option<bool>,
}

#[derive(Debug, Clone)]
pub struct GetSimAppPathParams {
    pub project_path: Option<String>,
    pub workspace_path: Option<String>,
    pub scheme: String,
    pub configuration: String,
    pub simulator_id: Option<String>,
    pub simulator_name: Option<String>,
    pub use_latest_os: bool,
}

pub fn get_sim_app_path_from_input(
    input: GetSimAppPathParamsInput,
    defaults: &SessionDefaults,
) -> Result<GetSimAppPathParams, String> {
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
    let use_latest_os = input
        .use_latest_os
        .or(defaults.use_latest_os)
        .unwrap_or(true);

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

    Ok(GetSimAppPathParams {
        project_path,
        workspace_path,
        scheme,
        configuration,
        simulator_id,
        simulator_name,
        use_latest_os,
    })
}

pub fn get_sim_app_path_command(params: &GetSimAppPathParams) -> CommandSpec {
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

    CommandSpec {
        program: "xcodebuild".to_string(),
        args,
        cwd: None,
        env: None,
    }
}

pub fn execute_get_sim_app_path(params: GetSimAppPathParams, runner: &impl Runner) -> ToolResponse {
    let spec = get_sim_app_path_command(&params);
    match runner.run(&spec) {
        Ok(output) => {
            if output.exit_code != 0 {
                return ToolResponse::text(render_command_output(&output), false);
            }

            let app_path = match extract_app_path(&output.stdout) {
                Some(path) => path,
                None => {
                    return ToolResponse::error(
                        "Failed to extract app path from build settings".to_string(),
                        Some("Make sure the app has been built first.".to_string()),
                    );
                }
            };

            let mut lines = Vec::new();
            lines.push(format!("✅ App path retrieved successfully: {}", app_path));
            lines.push("Next steps:".to_string());
            lines.push(format!(
                "1. Install app: install_app_sim({{ simulatorId: \"SIMULATOR_UUID\", appPath: \"{}\" }})",
                app_path
            ));
            lines.push(
                "2. Launch app: launch_app_sim({ simulatorId: \"SIMULATOR_UUID\", bundleId: \"BUNDLE_ID\" })"
                    .to_string(),
            );
            ToolResponse::text(lines.join("\n"), true)
        }
        Err(err) => ToolResponse::error("Command failed".to_string(), Some(err.to_string())),
    }
}

fn extract_app_path(stdout: &str) -> Option<String> {
    let built_products_dir = extract_setting(stdout, "BUILT_PRODUCTS_DIR")?;
    let full_product_name = extract_setting(stdout, "FULL_PRODUCT_NAME")?;
    Some(format!("{}/{}", built_products_dir, full_product_name))
}

fn extract_setting(stdout: &str, key: &str) -> Option<String> {
    for line in stdout.lines() {
        let trimmed = line.trim();
        if trimmed.starts_with(key) {
            if let Some((_, value)) = trimmed.split_once('=') {
                let value = value.trim();
                if !value.is_empty() {
                    return Some(value.to_string());
                }
            }
        }
    }
    None
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn extract_app_path_finds_settings() {
        let stdout = "\
Build settings for action build and target App:
    BUILT_PRODUCTS_DIR = /tmp/DerivedData/Build/Products/Debug-iphonesimulator
    FULL_PRODUCT_NAME = App.app
";
        let path = extract_app_path(stdout).expect("path");
        assert_eq!(
            path,
            "/tmp/DerivedData/Build/Products/Debug-iphonesimulator/App.app"
        );
    }
}
