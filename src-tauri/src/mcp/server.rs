// ABOUTME: MCP JSON-RPC HTTP server exposing Glitch Spot snapshot tools.
// ABOUTME: Runs on localhost:7820; CC registers it as an MCP server.

use std::sync::Arc;
use axum::{extract::State, http::StatusCode, response::Json as AxumJson, routing::post, Router};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

use crate::mcp::store::{SnapshotStore, WaitResult};

// ---------- JSON-RPC envelope types ----------

#[derive(Debug, Deserialize)]
pub struct RpcRequest {
    pub jsonrpc: String,
    pub id: Option<Value>,
    pub method: String,
    pub params: Option<Value>,
}

#[derive(Debug, Serialize)]
pub struct RpcResponse {
    pub jsonrpc: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub id: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub result: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<RpcError>,
}

#[derive(Debug, Serialize)]
pub struct RpcError {
    pub code: i32,
    pub message: String,
}

fn ok(id: Option<Value>, result: Value) -> RpcResponse {
    RpcResponse { jsonrpc: "2.0".into(), id, result: Some(result), error: None }
}

fn err(id: Option<Value>, code: i32, message: impl Into<String>) -> RpcResponse {
    RpcResponse { jsonrpc: "2.0".into(), id, result: None, error: Some(RpcError { code, message: message.into() }) }
}

// ---------- Route handler ----------

async fn handle_mcp(
    State(store): State<Arc<SnapshotStore>>,
    AxumJson(req): AxumJson<RpcRequest>,
) -> (StatusCode, AxumJson<RpcResponse>) {
    let resp = dispatch(store, req).await;
    (StatusCode::OK, AxumJson(resp))
}

async fn dispatch(store: Arc<SnapshotStore>, req: RpcRequest) -> RpcResponse {
    let id = req.id.clone();
    match req.method.as_str() {
        "initialize" => ok(id, json!({
            "protocolVersion": "2024-11-05",
            "serverInfo": { "name": "glitch-spot", "version": "0.4.0" },
            "capabilities": { "tools": {} }
        })),

        "tools/list" => ok(id, json!({
            "tools": [
                {
                    "name": "wait_for_snapshot",
                    "description": "Blocks until the next Glitch Spot annotation is saved. Returns the full snapshot JSON.",
                    "inputSchema": {
                        "type": "object",
                        "properties": {
                            "timeout_seconds": { "type": "integer", "description": "Max seconds to wait. Default 600, max 1800." }
                        }
                    }
                },
                {
                    "name": "get_latest_snapshot",
                    "description": "Returns the most recently saved snapshot, or null if none exist.",
                    "inputSchema": { "type": "object", "properties": {} }
                },
                {
                    "name": "get_snapshot",
                    "description": "Returns a specific snapshot by ID.",
                    "inputSchema": {
                        "type": "object",
                        "required": ["id"],
                        "properties": { "id": { "type": "string" } }
                    }
                },
                {
                    "name": "cancel_wait",
                    "description": "Cancels any in-flight wait_for_snapshot call. No-op if nothing is waiting.",
                    "inputSchema": { "type": "object", "properties": {} }
                }
            ]
        })),

        "tools/call" => {
            let params = req.params.unwrap_or(json!({}));
            let tool_name = params["name"].as_str().unwrap_or("").to_string();
            let args = params["arguments"].clone();
            call_tool(store, id, &tool_name, args).await
        }

        _ => err(id, -32601, format!("method not found: {}", req.method)),
    }
}

async fn call_tool(store: Arc<SnapshotStore>, id: Option<Value>, name: &str, args: Value) -> RpcResponse {
    match name {
        "wait_for_snapshot" => {
            let secs = args["timeout_seconds"]
                .as_u64()
                .unwrap_or(600)
                .min(1800);
            let result = store.wait_for_next(secs).await;
            let body = match result {
                WaitResult::Snapshot(s) => serde_json::to_value(&s).unwrap_or(Value::Null),
                WaitResult::Cancelled => json!({ "cancelled": true }),
                WaitResult::TimedOut => json!({ "cancelled": true, "reason": "timeout" }),
            };
            mcp_text_content(id, body)
        }

        "get_latest_snapshot" => {
            let snap = store.get_latest();
            let body = snap.map(|s| serde_json::to_value(&s).unwrap_or(Value::Null))
                .unwrap_or(Value::Null);
            mcp_text_content(id, body)
        }

        "get_snapshot" => {
            let snap_id = match args["id"].as_str() {
                Some(i) => i.to_string(),
                None => return err(id, -32602, "missing required argument: id"),
            };
            match store.get_snapshot(&snap_id) {
                Some(s) => mcp_text_content(id, serde_json::to_value(&s).unwrap_or(Value::Null)),
                None => err(id, -32602, format!("snapshot not found: {snap_id}")),
            }
        }

        "cancel_wait" => {
            store.cancel();
            ok(id, json!({ "content": [{ "type": "text", "text": "cancelled" }] }))
        }

        _ => err(id, -32602, format!("unknown tool: {name}")),
    }
}

/// Wraps a value as MCP tool content (text type, JSON stringified).
fn mcp_text_content(id: Option<Value>, value: Value) -> RpcResponse {
    ok(id, json!({
        "content": [{ "type": "text", "text": value.to_string() }]
    }))
}

// ---------- Public API ----------

pub fn build_router(store: Arc<SnapshotStore>) -> Router {
    Router::new()
        .route("/mcp", post(handle_mcp))
        .with_state(store)
}

/// Start the MCP server. Binds to 127.0.0.1:7820 and runs forever.
/// Call this with `tauri::async_runtime::spawn(mcp::server::serve(store))`.
pub async fn serve(store: Arc<SnapshotStore>) {
    let app = build_router(store);
    let listener = match tokio::net::TcpListener::bind("127.0.0.1:7820").await {
        Ok(l) => l,
        Err(e) => {
            eprintln!("[glitch-spot mcp] failed to bind port 7820: {e}");
            return;
        }
    };
    if let Err(e) = axum::serve(listener, app).await {
        eprintln!("[glitch-spot mcp] server error: {e}");
    }
}

// ---------- Tests ----------

#[cfg(test)]
mod tests {
    use super::*;
    use axum::body::Body;
    use axum::http::{Method, Request};
    use tower::ServiceExt; // for `oneshot`
    use crate::snapshot::{Context, Images, OsInfo, ScreenInfo, Snapshot};

    fn fake_snapshot(id: &str) -> Snapshot {
        Snapshot {
            id: id.to_string(),
            created_at: "2026-01-01T00:00:00Z".to_string(),
            mode: "frozen".to_string(),
            annotations: vec![],
            context: Context {
                screen: ScreenInfo { width: 1440, height: 900, scale_factor: 2.0, display_index: 0 },
                os: OsInfo { platform: "macos".to_string(), version: "15.0".to_string() },
            },
            images: Images { clean: "/tmp/c.png".to_string(), annotated: None },
        }
    }

    fn make_app() -> (Router, Arc<SnapshotStore>) {
        let store = Arc::new(SnapshotStore::new());
        let app = build_router(store.clone());
        (app, store)
    }

    async fn post_json(app: Router, body: Value) -> Value {
        let req = Request::builder()
            .method(Method::POST)
            .uri("/mcp")
            .header("content-type", "application/json")
            .body(Body::from(body.to_string()))
            .unwrap();
        let response = app.oneshot(req).await.unwrap();
        let bytes = axum::body::to_bytes(response.into_body(), usize::MAX).await.unwrap();
        serde_json::from_slice(&bytes).unwrap()
    }

    #[tokio::test]
    async fn initialize_returns_server_info() {
        let (app, _) = make_app();
        let resp = post_json(app, json!({"jsonrpc":"2.0","id":1,"method":"initialize","params":{}})).await;
        assert_eq!(resp["result"]["serverInfo"]["name"], "glitch-spot");
    }

    #[tokio::test]
    async fn tools_list_returns_four_tools() {
        let (app, _) = make_app();
        let resp = post_json(app, json!({"jsonrpc":"2.0","id":1,"method":"tools/list","params":{}})).await;
        let tools = resp["result"]["tools"].as_array().unwrap();
        assert_eq!(tools.len(), 4);
        let names: Vec<&str> = tools.iter().map(|t| t["name"].as_str().unwrap()).collect();
        assert!(names.contains(&"wait_for_snapshot"));
        assert!(names.contains(&"get_latest_snapshot"));
        assert!(names.contains(&"get_snapshot"));
        assert!(names.contains(&"cancel_wait"));
    }

    #[tokio::test]
    async fn get_latest_snapshot_returns_null_when_empty() {
        let (app, _) = make_app();
        let resp = post_json(app, json!({"jsonrpc":"2.0","id":1,"method":"tools/call","params":{"name":"get_latest_snapshot","arguments":{}}})).await;
        let content = &resp["result"]["content"][0]["text"];
        let parsed: Value = serde_json::from_str(content.as_str().unwrap()).unwrap();
        assert!(parsed.is_null());
    }

    #[tokio::test]
    async fn get_latest_snapshot_returns_snapshot_when_present() {
        let (app, store) = make_app();
        store.publish(fake_snapshot("snap_x01"));
        let resp = post_json(app, json!({"jsonrpc":"2.0","id":1,"method":"tools/call","params":{"name":"get_latest_snapshot","arguments":{}}})).await;
        let content = &resp["result"]["content"][0]["text"];
        let parsed: Value = serde_json::from_str(content.as_str().unwrap()).unwrap();
        assert_eq!(parsed["id"], "snap_x01");
    }

    #[tokio::test]
    async fn get_snapshot_returns_error_for_missing_id() {
        let (app, _) = make_app();
        let resp = post_json(app, json!({"jsonrpc":"2.0","id":1,"method":"tools/call","params":{"name":"get_snapshot","arguments":{"id":"snap_nope"}}})).await;
        assert!(resp["error"].is_object());
    }

    #[tokio::test]
    async fn cancel_wait_is_noop_when_no_waiter() {
        let (app, _) = make_app();
        let resp = post_json(app, json!({"jsonrpc":"2.0","id":1,"method":"tools/call","params":{"name":"cancel_wait","arguments":{}}})).await;
        assert!(resp["result"].is_object());
        assert!(resp["error"].is_null());
    }

    #[tokio::test]
    async fn unknown_method_returns_error() {
        let (app, _) = make_app();
        let resp = post_json(app, json!({"jsonrpc":"2.0","id":1,"method":"nonsense","params":{}})).await;
        assert!(resp["error"].is_object());
    }
}
