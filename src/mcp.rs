use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::io::{self, BufRead, Write};

use crate::exec::OsRunner;
use crate::session::SessionStore;
use crate::tools::{ToolCallError, call_tool, tool_definitions};

#[derive(Debug, Deserialize)]
pub struct JsonRpcRequest {
    pub jsonrpc: String,
    pub id: Option<Value>,
    pub method: String,
    pub params: Option<Value>,
}

#[derive(Debug, Serialize)]
pub struct JsonRpcResponse {
    pub jsonrpc: &'static str,
    pub id: Value,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub result: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<JsonRpcError>,
}

#[derive(Debug, Serialize)]
pub struct JsonRpcError {
    pub code: i64,
    pub message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub data: Option<Value>,
}

struct ServerState {
    session: SessionStore,
    runner: OsRunner,
    protocol_version: String,
}

pub fn run_stdio_server() -> io::Result<()> {
    let stdin = io::stdin();
    let mut stdout = io::stdout();
    let mut state = ServerState {
        session: SessionStore::new(),
        runner: OsRunner,
        protocol_version: "2024-11-05".to_string(),
    };

    for line in stdin.lock().lines() {
        let line = line?;
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }

        let response = match serde_json::from_str::<JsonRpcRequest>(trimmed) {
            Ok(request) => handle_request(&mut state, request),
            Err(err) => Some(JsonRpcResponse {
                jsonrpc: "2.0",
                id: Value::Null,
                result: None,
                error: Some(JsonRpcError {
                    code: -32700,
                    message: "Parse error".to_string(),
                    data: Some(json!({ "details": err.to_string() })),
                }),
            }),
        };

        if let Some(resp) = response {
            let payload = serde_json::to_string(&resp).unwrap_or_else(|_| "{}".to_string());
            writeln!(stdout, "{}", payload)?;
            stdout.flush()?;
        }
    }

    Ok(())
}

fn handle_request(state: &mut ServerState, request: JsonRpcRequest) -> Option<JsonRpcResponse> {
    if request.id.is_none() {
        handle_notification(state, request);
        return None;
    }

    let id = request.id.clone().unwrap_or(Value::Null);

    match request.method.as_str() {
        "initialize" => Some(handle_initialize(state, id, request.params)),
        "tools/list" => Some(handle_tools_list(id)),
        "tools/call" => Some(handle_tools_call(state, id, request.params)),
        _ => Some(JsonRpcResponse {
            jsonrpc: "2.0",
            id,
            result: None,
            error: Some(JsonRpcError {
                code: -32601,
                message: "Method not found".to_string(),
                data: None,
            }),
        }),
    }
}

fn handle_notification(_state: &mut ServerState, _request: JsonRpcRequest) {}

fn handle_initialize(state: &mut ServerState, id: Value, params: Option<Value>) -> JsonRpcResponse {
    if let Some(Value::Object(map)) = &params
        && let Some(Value::String(protocol_version)) = map.get("protocolVersion")
    {
        state.protocol_version = protocol_version.clone();
    }

    JsonRpcResponse {
        jsonrpc: "2.0",
        id,
        result: Some(json!({
            "protocolVersion": state.protocol_version,
            "capabilities": {
                "tools": { "listChanged": true },
                "resources": { "subscribe": true, "listChanged": true },
                "logging": {}
            },
            "serverInfo": {
                "name": "xcbuild-bridge",
                "version": env!("CARGO_PKG_VERSION")
            }
        })),
        error: None,
    }
}

fn handle_tools_list(id: Value) -> JsonRpcResponse {
    let tools = tool_definitions();
    JsonRpcResponse {
        jsonrpc: "2.0",
        id,
        result: Some(json!({ "tools": tools })),
        error: None,
    }
}

fn handle_tools_call(state: &mut ServerState, id: Value, params: Option<Value>) -> JsonRpcResponse {
    let params = params.unwrap_or_else(|| json!({}));
    let name = params
        .get("name")
        .and_then(|value| value.as_str())
        .unwrap_or_default()
        .to_string();
    let args = params
        .get("arguments")
        .cloned()
        .unwrap_or_else(|| json!({}));

    match call_tool(&name, args, &mut state.session, &state.runner) {
        Ok(tool_response) => JsonRpcResponse {
            jsonrpc: "2.0",
            id,
            result: Some(json!(tool_response)),
            error: None,
        },
        Err(ToolCallError::UnknownTool(_)) => JsonRpcResponse {
            jsonrpc: "2.0",
            id,
            result: None,
            error: Some(JsonRpcError {
                code: -32602,
                message: "Unknown tool".to_string(),
                data: Some(json!({ "name": name })),
            }),
        },
    }
}
