use serde::Deserialize;

use crate::exec::{CommandSpec, Runner};
use crate::session::SessionDefaults;
use crate::test_support::render_command_output;
use crate::tools::ToolResponse;

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ListSchemesParamsInput {
    pub project_path: Option<String>,
    pub workspace_path: Option<String>,
}

#[derive(Debug, Clone)]
pub struct ListSchemesParams {
    pub project_path: Option<String>,
    pub workspace_path: Option<String>,
}

pub fn list_schemes_from_input(
    input: ListSchemesParamsInput,
    defaults: &SessionDefaults,
) -> Result<ListSchemesParams, String> {
    let project_path = normalize_opt(input.project_path).or_else(|| defaults.project_path.clone());
    let workspace_path =
        normalize_opt(input.workspace_path).or_else(|| defaults.workspace_path.clone());

    if project_path.is_none() && workspace_path.is_none() {
        return Err("Either projectPath or workspacePath is required.".to_string());
    }
    if project_path.is_some() && workspace_path.is_some() {
        return Err("projectPath and workspacePath are mutually exclusive.".to_string());
    }

    Ok(ListSchemesParams {
        project_path,
        workspace_path,
    })
}

pub fn list_schemes_command(params: &ListSchemesParams) -> CommandSpec {
    let mut args = Vec::new();
    args.push("-list".to_string());

    if let Some(workspace) = &params.workspace_path {
        args.push("-workspace".to_string());
        args.push(workspace.clone());
    } else if let Some(project) = &params.project_path {
        args.push("-project".to_string());
        args.push(project.clone());
    }

    CommandSpec {
        program: "xcodebuild".to_string(),
        args,
        cwd: None,
        env: None,
    }
}

pub fn execute_list_schemes(params: ListSchemesParams, runner: &impl Runner) -> ToolResponse {
    let spec = list_schemes_command(&params);
    match runner.run(&spec) {
        Ok(output) => {
            if output.exit_code != 0 {
                return ToolResponse::text(render_command_output(&output), false);
            }

            let schemes = extract_schemes(&output.stdout);
            if schemes.is_empty() {
                return ToolResponse::error(
                    "No schemes found in output".to_string(),
                    Some(output.stdout),
                );
            }

            let mut lines = Vec::new();
            lines.push("Available schemes:".to_string());
            for scheme in &schemes {
                lines.push(format!("- {}", scheme));
            }

            if let Some(first_scheme) = schemes.first() {
                lines.push(String::new());
                lines.push("Next steps:".to_string());

                let path = params
                    .workspace_path
                    .as_ref()
                    .or(params.project_path.as_ref())
                    .cloned()
                    .unwrap_or_default();
                let key = if params.workspace_path.is_some() {
                    "workspacePath"
                } else {
                    "projectPath"
                };
                lines.push(format!(
                    "1. Build for simulator: build_sim({{{}: \"{}\", scheme: \"{}\", simulatorName: \"iPhone 15\" }})",
                    key, path, first_scheme
                ));
                lines.push(format!(
                    "2. Run tests: test_sim({{{}: \"{}\", scheme: \"{}\", simulatorName: \"iPhone 15\" }})",
                    key, path, first_scheme
                ));
                lines.push(format!(
                    "Hint: Consider saving a default scheme with session-set-defaults {{ scheme: \"{}\" }}",
                    first_scheme
                ));
            }

            ToolResponse::text(lines.join("\n"), true)
        }
        Err(err) => ToolResponse::error("Command failed".to_string(), Some(err.to_string())),
    }
}

fn extract_schemes(stdout: &str) -> Vec<String> {
    let mut schemes = Vec::new();
    let mut in_section = false;

    for line in stdout.lines() {
        let trimmed = line.trim_end();
        if !in_section {
            if trimmed.trim() == "Schemes:" {
                in_section = true;
            }
            continue;
        }

        let entry = trimmed.trim();
        if entry.is_empty() {
            break;
        }

        let starts_with_indent = line.starts_with(' ') || line.starts_with('\t');
        let looks_like_section = !starts_with_indent && entry.ends_with(':');
        if looks_like_section {
            break;
        }

        schemes.push(entry.to_string());
    }

    schemes
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
    fn extract_schemes_parses_block() {
        let stdout = "\
Information about project \"App\":

    Schemes:
        App
        AppTests

    Build Configurations:
        Debug
";
        let schemes = extract_schemes(stdout);
        assert_eq!(schemes, vec!["App".to_string(), "AppTests".to_string()]);
    }

    #[test]
    fn list_schemes_from_input_uses_defaults() {
        let defaults = SessionDefaults {
            project_path: Some("App.xcodeproj".to_string()),
            ..SessionDefaults::default()
        };
        let params = list_schemes_from_input(
            ListSchemesParamsInput {
                project_path: None,
                workspace_path: None,
            },
            &defaults,
        )
        .expect("should resolve");
        assert_eq!(params.project_path, Some("App.xcodeproj".to_string()));
    }
}
