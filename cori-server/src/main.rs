/// cori-server — HTTP API server exposing the Cori agent as a REST service.
///
/// Endpoints:
///   POST /v1/chat         — Single-turn agent invocation (non-streaming)
///   POST /v1/chat/stream  — Streaming agent run via SSE
///   GET  /v1/tools        — List available tools
///   GET  /v1/health       — Health check
use std::convert::Infallible;
use std::sync::Arc;

use axum::{
    extract::{Json, State},
    response::{
        sse::{Event, Sse},
        IntoResponse,
    },
    routing::{get, post},
    Router,
};
use futures_util::stream::Stream;
use serde::{Deserialize, Serialize};
use tokio_stream::wrappers::UnboundedReceiverStream;

// ── AppState ──────────────────────────────────────────────────────────────────

/// Shared state: tool schemas (for listing) + provider config (for per-request
/// agent construction). We create a fresh AgentLoop per request.
struct AppState {
    tool_schemas: Vec<serde_json::Value>,
    api_key: String,
    base_url: String,
    model: String,
}

// ── Request / Response types ──────────────────────────────────────────────────

#[derive(Deserialize)]
struct ChatRequest {
    message: String,
    #[allow(dead_code)]
    session_id: Option<String>,
}

#[derive(Serialize)]
struct ChatResponse {
    response: String,
    usage: UsageInfo,
}

#[derive(Serialize)]
struct UsageInfo {
    input_tokens: u32,
    output_tokens: u32,
}

#[derive(Serialize)]
struct ToolInfo {
    name: String,
    description: String,
}

#[derive(Serialize)]
struct ToolsResponse {
    tools: Vec<ToolInfo>,
}

#[derive(Serialize)]
struct HealthResponse {
    status: &'static str,
    version: &'static str,
}

// ── Helper: build a fresh ToolRegistry per request ───────────────────────────

fn build_registry() -> cori_core::tools::ToolRegistry {
    use cori_core::{
        planner::TaskGraph,
        tools::{
            bash::BashTool,
            edit::EditFileTool,
            fs::{GlobTool, GrepTool, ReadFileTool, WriteFileTool},
            subagent::SubagentTool,
            task::{TaskCreateTool, TaskGetTool, TaskListTool, TaskUpdateTool},
            ToolRegistry,
        },
    };
    use std::sync::{Arc, Mutex};

    let tasks_dir = std::env::var("CORI_TASKS_DIR").unwrap_or_else(|_| ".tasks".into());
    let graph = Arc::new(Mutex::new(
        TaskGraph::load(&tasks_dir).expect("failed to load TaskGraph"),
    ));

    let mut registry = ToolRegistry::new();
    registry.register(BashTool);
    registry.register(ReadFileTool);
    registry.register(WriteFileTool);
    registry.register(GlobTool);
    registry.register(GrepTool);
    registry.register(EditFileTool);
    registry.register(SubagentTool);
    registry.register(TaskCreateTool::new(Arc::clone(&graph)));
    registry.register(TaskGetTool::new(Arc::clone(&graph)));
    registry.register(TaskListTool::new(Arc::clone(&graph)));
    registry.register(TaskUpdateTool::new(Arc::clone(&graph)));

    registry
}

/// Build a `ClaudeLlm` from state's stored config + given tool schemas.
fn build_llm(
    state: &AppState,
    tool_schemas: Vec<serde_json::Value>,
) -> anyhow::Result<cori_core::claude::ClaudeLlm> {
    use cori_core::claude::ClaudeLlm;

    // Set env vars from stored state so from_env() picks them up.
    // This is safe for a single-process server where these don't change.
    std::env::set_var("ANTHROPIC_API_KEY", &state.api_key);
    std::env::set_var("ANTHROPIC_BASE_URL", &state.base_url);
    std::env::set_var("ANTHROPIC_MODEL", &state.model);

    ClaudeLlm::from_env(tool_schemas)
}

// ── Handlers ──────────────────────────────────────────────────────────────────

/// POST /v1/chat — single-turn, waits for the full agent response.
async fn handle_chat(
    State(state): State<Arc<AppState>>,
    Json(req): Json<ChatRequest>,
) -> impl IntoResponse {
    use cori_core::loop_::AgentLoop;

    let registry = build_registry();
    let llm = match build_llm(&state, registry.all_schemas()) {
        Ok(l) => l,
        Err(e) => {
            let body = serde_json::json!({"error": e.to_string()});
            return (axum::http::StatusCode::INTERNAL_SERVER_ERROR, Json(body)).into_response();
        }
    };
    let mut agent = AgentLoop::new(llm, registry);

    match agent.run(&req.message).await {
        Ok(response) => {
            // AgentLoop::run returns only the final text; usage is not
            // propagated through the public API at this layer, so we report
            // zeros (acceptable for a first version).
            let body = ChatResponse {
                response,
                usage: UsageInfo {
                    input_tokens: 0,
                    output_tokens: 0,
                },
            };
            (axum::http::StatusCode::OK, Json(body)).into_response()
        }
        Err(e) => {
            tracing::error!("agent error: {e:#}");
            let body = serde_json::json!({"error": e.to_string()});
            (axum::http::StatusCode::INTERNAL_SERVER_ERROR, Json(body)).into_response()
        }
    }
}

/// POST /v1/chat/stream — streaming agent run delivered as SSE.
///
/// Each agent "turn" produces SSE events:
///   event: text_delta   data: {"text": "..."}
///   event: tool_call    data: {"name": "bash", "input": {...}}
///   event: tool_result  data: {"name": "bash", "output": "..."}
///   event: done         data: {"usage": {"input_tokens": 0, "output_tokens": 0}}
///   event: error        data: {"error": "..."}
async fn handle_chat_stream(
    State(state): State<Arc<AppState>>,
    Json(req): Json<ChatRequest>,
) -> Sse<impl Stream<Item = Result<Event, Infallible>>> {
    use cori_core::loop_::AgentLoop;
    use tokio::sync::mpsc;

    let (tx, rx) = mpsc::unbounded_channel::<Result<Event, Infallible>>();

    // Spawn the agent in a background task; send SSE events through the channel.
    tokio::spawn(async move {
        let registry = build_registry();
        let llm = match build_llm(&state, registry.all_schemas()) {
            Ok(l) => l,
            Err(e) => {
                let err_event = Event::default()
                    .event("error")
                    .data(serde_json::json!({"error": e.to_string()}).to_string());
                let _ = tx.send(Ok(err_event));
                return;
            }
        };

        let tx_text = tx.clone();
        let tx_done = tx.clone();
        let tx_err = tx.clone();

        let instrumented = InstrumentedExecutor {
            inner: registry,
            tx: tx.clone(),
        };

        let mut agent = AgentLoop::new(llm, instrumented);
        let mut messages = vec![cori_core::types::Message::user(&req.message)];

        // run_turn_streaming streams text tokens via on_text callback.
        let result = agent
            .run_turn_streaming(&mut messages, move |chunk| {
                let event = Event::default()
                    .event("text_delta")
                    .data(serde_json::json!({"text": chunk}).to_string());
                let _ = tx_text.send(Ok(event));
            })
            .await;

        match result {
            Ok(_text) => {
                let done_event = Event::default()
                    .event("done")
                    .data(
                        serde_json::json!({
                            "usage": {"input_tokens": 0, "output_tokens": 0}
                        })
                        .to_string(),
                    );
                let _ = tx_done.send(Ok(done_event));
            }
            Err(e) => {
                tracing::error!("stream agent error: {e:#}");
                let err_event = Event::default()
                    .event("error")
                    .data(serde_json::json!({"error": e.to_string()}).to_string());
                let _ = tx_err.send(Ok(err_event));
            }
        }
        // tx is dropped here; the channel closes and the stream ends.
    });

    Sse::new(UnboundedReceiverStream::new(rx))
        .keep_alive(axum::response::sse::KeepAlive::default())
}

/// GET /v1/tools — list all registered tools with name + description.
async fn handle_tools(State(state): State<Arc<AppState>>) -> impl IntoResponse {
    let tools: Vec<ToolInfo> = state
        .tool_schemas
        .iter()
        .filter_map(|schema| {
            let name = schema["name"].as_str()?.to_owned();
            let description = schema["description"].as_str().unwrap_or("").to_owned();
            Some(ToolInfo { name, description })
        })
        .collect();
    Json(ToolsResponse { tools })
}

/// GET /v1/health — simple liveness probe.
async fn handle_health() -> impl IntoResponse {
    Json(HealthResponse {
        status: "ok",
        version: env!("CARGO_PKG_VERSION"),
    })
}

// ── InstrumentedExecutor ──────────────────────────────────────────────────────

/// Wraps ToolRegistry to emit SSE events for every tool call and result.
struct InstrumentedExecutor {
    inner: cori_core::tools::ToolRegistry,
    tx: tokio::sync::mpsc::UnboundedSender<Result<Event, Infallible>>,
}

impl cori_core::loop_::ToolExecutor for InstrumentedExecutor {
    async fn execute(
        &self,
        call: &cori_core::types::ToolUse,
    ) -> Result<cori_core::types::ToolResult, anyhow::Error> {
        // Emit tool_call event
        let call_event = Event::default()
            .event("tool_call")
            .data(
                serde_json::json!({
                    "name": call.name,
                    "input": call.input
                })
                .to_string(),
            );
        let _ = self.tx.send(Ok(call_event));

        let result = self.inner.dispatch(call).await?;

        // Emit tool_result event
        let result_event = Event::default()
            .event("tool_result")
            .data(
                serde_json::json!({
                    "name": call.name,
                    "output": result.content
                })
                .to_string(),
            );
        let _ = self.tx.send(Ok(result_event));

        Ok(result)
    }
}

// ── main ──────────────────────────────────────────────────────────────────────

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt()
        .with_max_level(tracing::Level::INFO)
        .without_time()
        .init();

    let config = cori_core::config::ProviderConfig::from_env()?;
    let port = std::env::var("CORI_PORT")
        .unwrap_or_else(|_| "3001".into())
        .parse::<u16>()
        .unwrap_or(3001);

    // Build a registry just to capture the tool schemas for /v1/tools.
    let tool_schemas = build_registry().all_schemas();

    let state = Arc::new(AppState {
        tool_schemas,
        api_key: config.api_key.clone(),
        base_url: config.base_url.clone(),
        model: config.model.clone(),
    });

    let app = Router::new()
        .route("/v1/chat", post(handle_chat))
        .route("/v1/chat/stream", post(handle_chat_stream))
        .route("/v1/tools", get(handle_tools))
        .route("/v1/health", get(handle_health))
        .with_state(state);

    let addr = format!("0.0.0.0:{port}");
    tracing::info!("cori-server listening on {addr}");
    let listener = tokio::net::TcpListener::bind(&addr).await?;
    axum::serve(listener, app).await?;
    Ok(())
}
