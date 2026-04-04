/// cori-mcp — MCP (Model Context Protocol) server for cori tools
///
/// Reads JSON-RPC 2.0 requests from stdin (newline-delimited) and writes
/// JSON-RPC 2.0 responses to stdout. All logging goes to stderr so that
/// stdout remains clean for the protocol.
use cori_core::tools::ToolRegistry;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};

// ── JSON-RPC types ────────────────────────────────────────────────────────────

#[derive(serde::Deserialize, Debug)]
struct JsonRpcRequest {
    id: Option<serde_json::Value>, // can be null, number, or string
    method: String,
    params: Option<serde_json::Value>,
}

#[derive(serde::Serialize)]
struct JsonRpcResponse {
    jsonrpc: &'static str, // always "2.0"
    id: serde_json::Value,
    #[serde(skip_serializing_if = "Option::is_none")]
    result: Option<serde_json::Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    error: Option<JsonRpcError>,
}

#[derive(serde::Serialize)]
struct JsonRpcError {
    code: i32,
    message: String,
}

// ── Main loop ─────────────────────────────────────────────────────────────────

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // Log to stderr so stdout stays clean for JSON-RPC
    tracing_subscriber::fmt()
        .with_writer(std::io::stderr)
        .with_max_level(tracing::Level::INFO)
        .without_time()
        .init();

    // Set up tool registry
    let mut registry = ToolRegistry::new();
    registry.register(cori_core::tools::bash::BashTool);
    registry.register(cori_core::tools::fs::ReadFileTool);
    registry.register(cori_core::tools::fs::WriteFileTool);
    registry.register(cori_core::tools::fs::GlobTool);
    registry.register(cori_core::tools::fs::GrepTool);
    registry.register(cori_core::tools::edit::EditFileTool);
    // Task tools share a single TaskGraph instance
    let task_graph = std::sync::Arc::new(std::sync::Mutex::new(
        cori_core::planner::TaskGraph::load(".tasks")?,
    ));
    registry.register(cori_core::tools::task::TaskCreateTool::new(
        std::sync::Arc::clone(&task_graph),
    ));
    registry.register(cori_core::tools::task::TaskListTool::new(
        std::sync::Arc::clone(&task_graph),
    ));
    registry.register(cori_core::tools::task::TaskGetTool::new(
        std::sync::Arc::clone(&task_graph),
    ));
    registry.register(cori_core::tools::task::TaskUpdateTool::new(
        std::sync::Arc::clone(&task_graph),
    ));

    let stdin = tokio::io::stdin();
    let mut reader = BufReader::new(stdin).lines();
    let mut stdout = tokio::io::stdout();

    tracing::info!("cori-mcp server started");

    while let Some(line) = reader.next_line().await? {
        let line = line.trim().to_string();
        if line.is_empty() {
            continue;
        }

        tracing::debug!(line = %line, "received");

        let response = match serde_json::from_str::<JsonRpcRequest>(&line) {
            Ok(req) => handle_request(req, &registry).await,
            Err(e) => error_response(
                serde_json::Value::Null,
                -32700, // Parse error
                &format!("Parse error: {e}"),
            ),
        };

        let mut out = serde_json::to_string(&response)?;
        out.push('\n');
        stdout.write_all(out.as_bytes()).await?;
        stdout.flush().await?;
    }

    tracing::info!("cori-mcp server shutting down");
    Ok(())
}

// ── Request handlers ──────────────────────────────────────────────────────────

async fn handle_request(req: JsonRpcRequest, registry: &ToolRegistry) -> JsonRpcResponse {
    let id = req.id.clone().unwrap_or(serde_json::Value::Null);
    tracing::info!(method = %req.method, "handling request");
    match req.method.as_str() {
        "initialize" => handle_initialize(id),
        "tools/list" => handle_tools_list(id, registry),
        "tools/call" => handle_tools_call(id, req.params, registry).await,
        "ping" => success_response(id, serde_json::json!({})),
        _ => error_response(id, -32601, "Method not found"),
    }
}

fn handle_initialize(id: serde_json::Value) -> JsonRpcResponse {
    success_response(
        id,
        serde_json::json!({
            "protocolVersion": "2024-11-05",
            "capabilities": {
                "tools": {}
            },
            "serverInfo": {
                "name": "cori-mcp",
                "version": "0.2.0"
            }
        }),
    )
}

fn handle_tools_list(id: serde_json::Value, registry: &ToolRegistry) -> JsonRpcResponse {
    // Convert cori tool schemas to MCP tool format.
    // Cori schema fields: name, description, input_schema
    // MCP tool format:    name, description, inputSchema
    let tools: Vec<serde_json::Value> = registry
        .all_schemas()
        .iter()
        .map(|schema| {
            serde_json::json!({
                "name": schema["name"],
                "description": schema["description"],
                "inputSchema": schema["input_schema"]
            })
        })
        .collect();
    success_response(id, serde_json::json!({ "tools": tools }))
}

async fn handle_tools_call(
    id: serde_json::Value,
    params: Option<serde_json::Value>,
    registry: &ToolRegistry,
) -> JsonRpcResponse {
    let params = match params {
        Some(p) => p,
        None => return error_response(id, -32602, "Missing params"),
    };
    let name = match params["name"].as_str() {
        Some(n) => n.to_string(),
        None => return error_response(id, -32602, "Missing tool name"),
    };
    let input = params
        .get("arguments")
        .cloned()
        .unwrap_or(serde_json::json!({}));

    tracing::info!(tool = %name, "calling tool");

    let tool_use = cori_core::types::ToolUse {
        id: "mcp-call".to_string(),
        name: name.clone(),
        input,
    };

    match registry.dispatch(&tool_use).await {
        Ok(result) => success_response(
            id,
            serde_json::json!({
                "content": [{
                    "type": "text",
                    "text": result.content
                }]
            }),
        ),
        Err(e) => {
            tracing::error!(tool = %name, error = %e, "tool call failed");
            error_response(id, -32603, &e.to_string())
        }
    }
}

// ── Response helpers ──────────────────────────────────────────────────────────

fn success_response(id: serde_json::Value, result: serde_json::Value) -> JsonRpcResponse {
    JsonRpcResponse {
        jsonrpc: "2.0",
        id,
        result: Some(result),
        error: None,
    }
}

fn error_response(id: serde_json::Value, code: i32, message: &str) -> JsonRpcResponse {
    JsonRpcResponse {
        jsonrpc: "2.0",
        id,
        result: None,
        error: Some(JsonRpcError {
            code,
            message: message.to_string(),
        }),
    }
}
