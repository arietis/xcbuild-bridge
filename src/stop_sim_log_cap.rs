use serde::Deserialize;
use std::fs;

use crate::log_sessions::LogSessionStore;
use crate::tools::ToolResponse;

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StopSimLogCapParamsInput {
    pub log_session_id: Option<String>,
}

#[derive(Debug, Clone)]
pub struct StopSimLogCapParams {
    pub log_session_id: String,
}

pub fn stop_sim_log_cap_from_input(
    input: StopSimLogCapParamsInput,
) -> Result<StopSimLogCapParams, String> {
    let log_session_id = normalize_opt(input.log_session_id)
        .ok_or_else(|| "logSessionId is required".to_string())?;
    Ok(StopSimLogCapParams { log_session_id })
}

pub fn execute_stop_sim_log_cap(
    params: StopSimLogCapParams,
    sessions: &mut LogSessionStore,
) -> ToolResponse {
    let mut session = match sessions.remove(&params.log_session_id) {
        Some(session) => session,
        None => {
            return ToolResponse::error(
                "Log session not found".to_string(),
                Some(params.log_session_id),
            );
        }
    };

    let _ = session.child.kill();
    let _ = session.child.wait();

    let log_content = match fs::read_to_string(&session.file_path) {
        Ok(content) => content,
        Err(err) => {
            return ToolResponse::error(
                "Failed to read log file".to_string(),
                Some(err.to_string()),
            );
        }
    };

    let _ = fs::remove_file(&session.file_path);

    let mut lines = Vec::new();
    if log_content.trim().is_empty() {
        lines.push(format!(
            "Log capture session {} stopped. No logs captured.",
            params.log_session_id
        ));
    } else {
        lines.push(format!(
            "Log capture session {} stopped. Log content follows:\n\n{}",
            params.log_session_id, log_content
        ));
    }

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
