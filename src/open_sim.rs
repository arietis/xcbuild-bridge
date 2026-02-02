use serde::Deserialize;

use crate::exec::{CommandSpec, Runner};
use crate::tools::ToolResponse;

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct OpenSimParamsInput {}

pub fn execute_open_sim(runner: &impl Runner) -> ToolResponse {
    let spec = CommandSpec {
        program: "open".to_string(),
        args: vec!["-a".to_string(), "Simulator".to_string()],
        cwd: None,
        env: None,
    };

    match runner.run(&spec) {
        Ok(output) => {
            let ok = output.exit_code == 0;
            let mut text = if ok {
                "Simulator app opened successfully.".to_string()
            } else {
                format!("Open simulator operation failed (code {}).", output.exit_code)
            };
            if !output.stderr.trim().is_empty() {
                text.push_str("\n\nSTDERR:\n");
                text.push_str(output.stderr.trim());
            }
            ToolResponse::text(text, ok)
        }
        Err(err) => ToolResponse::error("Command failed".to_string(), Some(err.to_string())),
    }
}
