use serde::Deserialize;
use crate::app_bundle::{read_bundle_id, validate_app_path};
use crate::exec::Runner;
use crate::tools::ToolResponse;

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GetAppBundleIdParamsInput {
    pub app_path: Option<String>,
}

#[derive(Debug, Clone)]
pub struct GetAppBundleIdParams {
    pub app_path: String,
}

pub fn get_app_bundle_id_from_input(
    input: GetAppBundleIdParamsInput,
) -> Result<GetAppBundleIdParams, String> {
    let app_path = normalize_opt(input.app_path).ok_or_else(|| "appPath is required".to_string())?;
    Ok(GetAppBundleIdParams { app_path })
}

pub fn execute_get_app_bundle_id(params: GetAppBundleIdParams, runner: &impl Runner) -> ToolResponse {
    if let Err(err) = validate_app_path(&params.app_path) {
        return ToolResponse::error("Invalid appPath".to_string(), Some(err));
    }

    let bundle_id = match read_bundle_id(&params.app_path, runner) {
        Ok(bundle_id) => bundle_id,
        Err(err) => {
            return ToolResponse::error(
                "Failed to extract bundle ID".to_string(),
                Some(err),
            );
        }
    };

    let mut lines = Vec::new();
    lines.push(format!("✅ Bundle ID: {}", bundle_id));
    lines.push("Next steps:".to_string());
    lines.push("- Simulator: install_app_sim + launch_app_sim".to_string());
    lines.push("- Device: install_app_device + launch_app_device".to_string());

    ToolResponse::text(lines.join("\n"), true)
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
    use crate::exec::{CommandOutput, CommandSpec};
    use std::cell::RefCell;

    struct MockRunner {
        outputs: RefCell<Vec<CommandOutput>>,
    }

    impl Runner for MockRunner {
        fn run(&self, _spec: &CommandSpec) -> std::io::Result<CommandOutput> {
            Ok(self.outputs.borrow_mut().remove(0))
        }
    }

    #[test]
    fn get_app_bundle_id_requires_app_path() {
        let result = get_app_bundle_id_from_input(GetAppBundleIdParamsInput { app_path: None });
        assert!(result.is_err());
    }

    #[test]
    fn read_bundle_id_uses_defaults_when_available() {
        let runner = MockRunner {
            outputs: RefCell::new(vec![CommandOutput {
                exit_code: 0,
                terminated_by_signal: false,
                stdout: "com.example.App".to_string(),
                stderr: String::new(),
            }]),
        };

        let result = read_bundle_id("/tmp/Test.app", &runner).expect("bundle id");
        assert_eq!(result, "com.example.App");
    }

    #[test]
    fn read_bundle_id_falls_back_to_plistbuddy() {
        let runner = MockRunner {
            outputs: RefCell::new(vec![
                CommandOutput {
                    exit_code: 1,
                    terminated_by_signal: false,
                    stdout: String::new(),
                    stderr: "defaults failed".to_string(),
                },
                CommandOutput {
                    exit_code: 0,
                    terminated_by_signal: false,
                    stdout: "com.example.Fallback".to_string(),
                    stderr: String::new(),
                },
            ]),
        };

        let result = read_bundle_id("/tmp/Test.app", &runner).expect("bundle id");
        assert_eq!(result, "com.example.Fallback");
    }
}
