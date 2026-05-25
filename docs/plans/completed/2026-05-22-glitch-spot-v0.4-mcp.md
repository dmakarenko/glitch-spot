# Glitch Spot v0.4 — MCP Coding Harness Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use `superpowers:subagent-driven-development` (recommended) or `superpowers:executing-plans` to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.
>
> **Reference spec:** `docs/initiatives/glitch-spot/v0.4-coding-harness-seed.md`
> **Part of initiative:** `docs/initiatives/glitch-spot/INITIATIVE.md`

**Goal:** Add a local MCP server to the Glitch Spot desktop app and a `/glitch-spot` CC skill so that saving an annotation in Glitch Spot automatically surfaces the snapshot in an active Claude Code session, keeping CC responsive while waiting.

**Architecture:** An `axum` HTTP server runs inside the Tauri app process on port 7820, serving a minimal MCP JSON-RPC endpoint. A `SnapshotStore` (tokio watch channel) connects `save_snapshot` (existing Tauri command) to the MCP server — when the user saves an annotation, `save_snapshot` publishes to the store, which unblocks any in-flight `wait_for_snapshot` call. The CC skill dispatches a background subagent that calls `wait_for_snapshot`; the main CC session stays responsive.

**Tech Stack:** Tauri 2, Rust (stable), axum 0.7, tokio 1, serde_json 1 (already in deps). CC skill: plain markdown in `~/.claude/commands/`.

**Prerequisites:** This plan builds on the `v0.2-drawing` branch (snapshot building + clipboard write exist). The skill's "read annotated PNG" step works optimally once `v0.3-persistence` writes images to `~/.glitch-spot/snapshots/`; until then the skill reads from `/tmp` (where `build_and_write` currently writes clean PNGs). No blocking dependency.

---

## File map

**New files:**
- `src-tauri/src/mcp/mod.rs` — module root; re-exports `store` and `server`
- `src-tauri/src/mcp/store.rs` — `SnapshotStore`: in-memory latest snapshot + async wait/cancel
- `src-tauri/src/mcp/server.rs` — axum HTTP server; MCP JSON-RPC dispatcher and tool handlers
- `~/.claude/commands/glitch-spot.md` — `/glitch-spot` CC skill

**Modified files:**
- `src-tauri/Cargo.toml` — add `axum`, `tokio` (with `sync` feature)
- `src-tauri/src/lib.rs` — add `mcp` module, extend `AppState` with `snapshot_store`, spawn server on startup, publish snapshot in `save_snapshot`

---

## Task 1: Add dependencies and MCP module skeleton

**Files:**
- Modify: `src-tauri/Cargo.toml`
- Create: `src-tauri/src/mcp/mod.rs`

- [ ] **Step 1: Add axum and full tokio to Cargo.toml**

Open `src-tauri/Cargo.toml`. In the `[dependencies]` section add:

```toml
axum = "0.7"
tokio = { version = "1", features = ["rt-multi-thread", "macros", "sync", "time"] }
```

(The existing `anyhow = "1"` and `serde_json = "1"` are already present — do not duplicate them.)

- [ ] **Step 2: Create `src-tauri/src/mcp/mod.rs`**

```rust
// ABOUTME: MCP server module: in-memory snapshot store and HTTP JSON-RPC server.
// ABOUTME: Exposes wait_for_snapshot, get_latest_snapshot, get_snapshot, cancel_wait.

pub mod server;
pub mod store;
```

- [ ] **Step 3: Declare the module in lib.rs**

Open `src-tauri/src/lib.rs`. After the existing `mod overlay;` line add:

```rust
pub mod mcp;
```

- [ ] **Step 4: Verify it compiles**

```bash
cd src-tauri && cargo check
```

Expected: no errors. The two new submodule files don't exist yet so you'll get `file not found` errors — create empty placeholder files to pass this check:

```bash
touch src/mcp/store.rs src/mcp/server.rs
```

Then re-run `cargo check`. Expected: PASS (empty files compile).

- [ ] **Step 5: Commit**

```bash
git add src-tauri/Cargo.toml src-tauri/src/mcp/
git commit -m "Add mcp module skeleton and axum/tokio dependencies"
```

---

## Task 2: SnapshotStore

**Files:**
- Create: `src-tauri/src/mcp/store.rs`

### The design

A `tokio::sync::watch` channel broadcasts `Option<StoreEvent>` to any number of concurrent waiters. The `Receiver` pattern is:

1. Call `rx.borrow_and_update()` to mark the current value as "seen".
2. Call `rx.changed().await` — resolves only on the *next* send.
3. Read the new value.

This gives us "wait for the next snapshot, not the current one" semantics without a sentinel ID comparison.

- [ ] **Step 1: Write the failing tests first**

Create `src-tauri/src/mcp/store.rs` with just the test module:

```rust
// ABOUTME: In-memory snapshot store for the MCP server.
// ABOUTME: Broadcasts snapshot-ready and cancel events to waiting tool callers.

use crate::snapshot::Snapshot;
use std::sync::{Arc, Mutex};
use tokio::sync::watch;
use tokio::time::{timeout, Duration};

#[derive(Clone, Debug, PartialEq)]
pub enum StoreEvent {
    Snapshot(Snapshot),
    Cancelled,
}

pub struct SnapshotStore {
    latest: Arc<Mutex<Option<Snapshot>>>,
    tx: watch::Sender<Option<StoreEvent>>,
    rx: watch::Receiver<Option<StoreEvent>>,
}

pub enum WaitResult {
    Snapshot(Snapshot),
    Cancelled,
    TimedOut,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::snapshot::{Annotation, Context, Coords, Images, OsInfo, ScreenInfo, Snapshot};

    fn fake_snapshot(id: &str) -> Snapshot {
        Snapshot {
            id: id.to_string(),
            created_at: "2026-01-01T00:00:00Z".to_string(),
            mode: "frozen".to_string(),
            annotations: vec![Annotation {
                id: "a1".to_string(),
                shape: "rect".to_string(),
                coords: Coords { x: 0.0, y: 0.0, w: 10.0, h: 10.0 },
                note: "fix this".to_string(),
            }],
            context: Context {
                screen: ScreenInfo { width: 1440, height: 900, scale_factor: 2.0, display_index: 0 },
                os: OsInfo { platform: "macos".to_string(), version: "15.4.0".to_string() },
            },
            images: Images { clean: "/tmp/clean.png".to_string(), annotated: None },
        }
    }

    #[tokio::test]
    async fn get_latest_returns_none_initially() {
        let store = SnapshotStore::new();
        assert!(store.get_latest().is_none());
    }

    #[tokio::test]
    async fn publish_updates_latest() {
        let store = SnapshotStore::new();
        let snap = fake_snapshot("snap_001");
        store.publish(snap.clone());
        assert_eq!(store.get_latest().unwrap().id, "snap_001");
    }

    #[tokio::test]
    async fn wait_receives_published_snapshot() {
        let store = Arc::new(SnapshotStore::new());
        let store2 = store.clone();

        let handle = tokio::spawn(async move {
            store2.wait_for_next(5).await
        });

        tokio::time::sleep(Duration::from_millis(50)).await;
        store.publish(fake_snapshot("snap_002"));

        match handle.await.unwrap() {
            WaitResult::Snapshot(s) => assert_eq!(s.id, "snap_002"),
            other => panic!("expected Snapshot, got {:?}", std::mem::discriminant(&other)),
        }
    }

    #[tokio::test]
    async fn wait_returns_cancelled_on_cancel() {
        let store = Arc::new(SnapshotStore::new());
        let store2 = store.clone();

        let handle = tokio::spawn(async move {
            store2.wait_for_next(5).await
        });

        tokio::time::sleep(Duration::from_millis(50)).await;
        store.cancel();

        match handle.await.unwrap() {
            WaitResult::Cancelled => {}
            other => panic!("expected Cancelled, got {:?}", std::mem::discriminant(&other)),
        }
    }

    #[tokio::test]
    async fn wait_times_out() {
        let store = SnapshotStore::new();
        match store.wait_for_next(0).await {
            WaitResult::TimedOut => {}
            other => panic!("expected TimedOut, got {:?}", std::mem::discriminant(&other)),
        }
    }

    #[tokio::test]
    async fn get_snapshot_by_id() {
        let store = SnapshotStore::new();
        store.publish(fake_snapshot("snap_abc"));
        assert!(store.get_snapshot("snap_abc").is_some());
        assert!(store.get_snapshot("snap_xyz").is_none());
    }
}
```

- [ ] **Step 2: Run the tests to confirm they fail**

```bash
cd src-tauri && cargo test mcp::store::tests 2>&1 | head -30
```

Expected: compilation error — `SnapshotStore`, `WaitResult`, `StoreEvent` not defined yet. This confirms the tests are wired correctly.

- [ ] **Step 3: Implement SnapshotStore**

Replace `src-tauri/src/mcp/store.rs` with the full implementation (keep the `tests` module at the bottom unchanged):

```rust
// ABOUTME: In-memory snapshot store for the MCP server.
// ABOUTME: Broadcasts snapshot-ready and cancel events to waiting tool callers.

use crate::snapshot::Snapshot;
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use tokio::sync::watch;
use tokio::time::{timeout, Duration};

#[derive(Clone, Debug, PartialEq)]
pub enum StoreEvent {
    Snapshot(Snapshot),
    Cancelled,
}

pub struct SnapshotStore {
    latest: Arc<Mutex<Option<Snapshot>>>,
    // id -> snapshot, for get_snapshot(id) lookups
    all: Arc<Mutex<HashMap<String, Snapshot>>>,
    tx: watch::Sender<Option<StoreEvent>>,
    rx: watch::Receiver<Option<StoreEvent>>,
}

pub enum WaitResult {
    Snapshot(Snapshot),
    Cancelled,
    TimedOut,
}

impl SnapshotStore {
    pub fn new() -> Self {
        let (tx, rx) = watch::channel(None);
        Self {
            latest: Arc::new(Mutex::new(None)),
            all: Arc::new(Mutex::new(HashMap::new())),
            tx,
            rx,
        }
    }

    pub fn publish(&self, snap: Snapshot) {
        let mut latest = self.latest.lock().unwrap();
        *latest = Some(snap.clone());
        self.all.lock().unwrap().insert(snap.id.clone(), snap.clone());
        // Ignore send error — it means no receivers, which is fine.
        let _ = self.tx.send(Some(StoreEvent::Snapshot(snap)));
    }

    pub fn cancel(&self) {
        let _ = self.tx.send(Some(StoreEvent::Cancelled));
    }

    pub fn get_latest(&self) -> Option<Snapshot> {
        self.latest.lock().unwrap().clone()
    }

    pub fn get_snapshot(&self, id: &str) -> Option<Snapshot> {
        self.all.lock().unwrap().get(id).cloned()
    }

    /// Wait for the next snapshot or cancel signal, up to `timeout_secs`.
    pub async fn wait_for_next(&self, timeout_secs: u64) -> WaitResult {
        let mut rx = self.rx.clone();
        // Mark current value as seen so changed() only fires on the NEXT send.
        let _ = rx.borrow_and_update();

        let wait = async {
            loop {
                if rx.changed().await.is_err() {
                    // Sender dropped — treat as timeout.
                    return WaitResult::TimedOut;
                }
                match rx.borrow().as_ref() {
                    Some(StoreEvent::Snapshot(s)) => return WaitResult::Snapshot(s.clone()),
                    Some(StoreEvent::Cancelled) => return WaitResult::Cancelled,
                    None => continue, // spurious None send; keep waiting
                }
            }
        };

        match timeout(Duration::from_secs(timeout_secs), wait).await {
            Ok(result) => result,
            Err(_) => WaitResult::TimedOut,
        }
    }
}

#[cfg(test)]
mod tests {
    // ... (tests from Step 1 go here verbatim)
    use super::*;
    use crate::snapshot::{Annotation, Context, Coords, Images, OsInfo, ScreenInfo, Snapshot};

    fn fake_snapshot(id: &str) -> Snapshot {
        Snapshot {
            id: id.to_string(),
            created_at: "2026-01-01T00:00:00Z".to_string(),
            mode: "frozen".to_string(),
            annotations: vec![Annotation {
                id: "a1".to_string(),
                shape: "rect".to_string(),
                coords: Coords { x: 0.0, y: 0.0, w: 10.0, h: 10.0 },
                note: "fix this".to_string(),
            }],
            context: Context {
                screen: ScreenInfo { width: 1440, height: 900, scale_factor: 2.0, display_index: 0 },
                os: OsInfo { platform: "macos".to_string(), version: "15.4.0".to_string() },
            },
            images: Images { clean: "/tmp/clean.png".to_string(), annotated: None },
        }
    }

    #[tokio::test]
    async fn get_latest_returns_none_initially() {
        let store = SnapshotStore::new();
        assert!(store.get_latest().is_none());
    }

    #[tokio::test]
    async fn publish_updates_latest() {
        let store = SnapshotStore::new();
        let snap = fake_snapshot("snap_001");
        store.publish(snap.clone());
        assert_eq!(store.get_latest().unwrap().id, "snap_001");
    }

    #[tokio::test]
    async fn wait_receives_published_snapshot() {
        let store = Arc::new(SnapshotStore::new());
        let store2 = store.clone();

        let handle = tokio::spawn(async move {
            store2.wait_for_next(5).await
        });

        tokio::time::sleep(Duration::from_millis(50)).await;
        store.publish(fake_snapshot("snap_002"));

        match handle.await.unwrap() {
            WaitResult::Snapshot(s) => assert_eq!(s.id, "snap_002"),
            other => panic!("expected Snapshot, got {:?}", std::mem::discriminant(&other)),
        }
    }

    #[tokio::test]
    async fn wait_returns_cancelled_on_cancel() {
        let store = Arc::new(SnapshotStore::new());
        let store2 = store.clone();

        let handle = tokio::spawn(async move {
            store2.wait_for_next(5).await
        });

        tokio::time::sleep(Duration::from_millis(50)).await;
        store.cancel();

        match handle.await.unwrap() {
            WaitResult::Cancelled => {}
            other => panic!("expected Cancelled, got {:?}", std::mem::discriminant(&other)),
        }
    }

    #[tokio::test]
    async fn wait_times_out() {
        let store = SnapshotStore::new();
        match store.wait_for_next(0).await {
            WaitResult::TimedOut => {}
            other => panic!("expected TimedOut, got {:?}", std::mem::discriminant(&other)),
        }
    }

    #[tokio::test]
    async fn get_snapshot_by_id() {
        let store = SnapshotStore::new();
        store.publish(fake_snapshot("snap_abc"));
        assert!(store.get_snapshot("snap_abc").is_some());
        assert!(store.get_snapshot("snap_xyz").is_none());
    }
}
```

- [ ] **Step 4: Run the tests and confirm they pass**

```bash
cd src-tauri && cargo test mcp::store::tests -- --nocapture
```

Expected: all 6 tests PASS. If `wait_times_out` is flaky (0-second timeout races), change the timeout to `1` in the test; keep `0` as the value passed in `wait_for_next`.

- [ ] **Step 5: Commit**

```bash
git add src-tauri/src/mcp/store.rs src-tauri/Cargo.toml
git commit -m "Add SnapshotStore with watch-channel wait/cancel semantics"
```

---

## Task 3: MCP HTTP server

**Files:**
- Create: `src-tauri/src/mcp/server.rs`

The server speaks MCP JSON-RPC 2.0 over HTTP. CC sends a `POST /mcp` with a JSON body; the server dispatches on `method` and returns a JSON response. For `tools/call wait_for_snapshot`, the axum handler awaits the store and holds the HTTP connection open.

MCP methods implemented:
- `initialize` → return server info + capabilities
- `tools/list` → return 4 tool definitions
- `tools/call` → dispatch to `wait_for_snapshot`, `get_latest_snapshot`, `get_snapshot`, `cancel_wait`

- [ ] **Step 1: Write the tests**

Create `src-tauri/src/mcp/server.rs` with the test module:

```rust
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

// ---------- Tests ----------

#[cfg(test)]
mod tests {
    use super::*;
    use axum::body::Body;
    use axum::http::{Method, Request};
    use tower::ServiceExt; // for `oneshot`
    use crate::snapshot::{Annotation, Context, Coords, Images, OsInfo, ScreenInfo, Snapshot};

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
        // Should not error
        assert!(resp["error"].is_null());
    }

    #[tokio::test]
    async fn unknown_method_returns_error() {
        let (app, _) = make_app();
        let resp = post_json(app, json!({"jsonrpc":"2.0","id":1,"method":"nonsense","params":{}})).await;
        assert!(resp["error"].is_object());
    }
}
```

- [ ] **Step 2: Run tests to confirm they fail**

```bash
cd src-tauri && cargo test mcp::server::tests 2>&1 | head -20
```

Expected: compilation error — `build_router` not defined yet.

- [ ] **Step 3: Implement the server**

Add the implementation above the `#[cfg(test)]` block:

```rust
// ---------- Route handler ----------

async fn handle_mcp(
    State(store): State<Arc<SnapshotStore>>,
    AxumJson(req): AxumJson<RpcRequest>,
) -> (StatusCode, AxumJson<RpcResponse>) {
    let id = req.id.clone();
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
```

- [ ] **Step 4: Add `tower` as a dev dependency (used in tests)**

In `src-tauri/Cargo.toml` add:

```toml
[dev-dependencies]
tower = { version = "0.4", features = ["util"] }
```

- [ ] **Step 5: Run the tests and confirm they pass**

```bash
cd src-tauri && cargo test mcp::server::tests -- --nocapture
```

Expected: all 7 tests PASS. `wait_for_snapshot` is not directly tested here (it blocks indefinitely on an empty store); that's covered by the integration test in Task 6.

- [ ] **Step 6: Commit**

```bash
git add src-tauri/src/mcp/server.rs src-tauri/Cargo.toml
git commit -m "Add MCP HTTP server with JSON-RPC dispatcher and 4 tool handlers"
```

---

## Task 4: Wire SnapshotStore into AppState and save_snapshot

**Files:**
- Modify: `src-tauri/src/lib.rs`

The `SnapshotStore` needs to be:
1. Created once on app startup and held in `AppState`.
2. Passed to the MCP server (which runs in a background task).
3. Called in `save_snapshot` after building the snapshot.

- [ ] **Step 1: Write a test for the notification path**

Add this to `src-tauri/src/mcp/store.rs` `tests` module (after the existing tests):

```rust
#[tokio::test]
async fn publishing_multiple_snapshots_each_wakes_a_waiter() {
    let store = Arc::new(SnapshotStore::new());

    for i in 0..3u32 {
        let store2 = store.clone();
        let handle = tokio::spawn(async move { store2.wait_for_next(5).await });
        tokio::time::sleep(Duration::from_millis(20)).await;
        store.publish(fake_snapshot(&format!("snap_{i:03}")));
        match handle.await.unwrap() {
            WaitResult::Snapshot(s) => assert_eq!(s.id, format!("snap_{i:03}")),
            _ => panic!("expected snapshot"),
        }
    }
}
```

- [ ] **Step 2: Run the new test to confirm it passes**

```bash
cd src-tauri && cargo test mcp::store::tests::publishing_multiple_snapshots -- --nocapture
```

Expected: PASS. (This verifies the watch channel resets correctly between publishes.)

- [ ] **Step 3: Extend AppState in lib.rs**

Open `src-tauri/src/lib.rs`. Replace the `AppState` struct:

```rust
pub struct AppState {
    pub latest_frame: Mutex<Option<capture::CapturedFrame>>,
    pub snapshot_store: Arc<mcp::store::SnapshotStore>,
}
```

- [ ] **Step 4: Update the save_snapshot command to notify the store**

In `lib.rs`, find the `save_snapshot` command. After the line `let json = serde_json::to_string_pretty(&snap)...` and before the clipboard write, add:

```rust
    // Notify any waiting MCP caller.
    state.snapshot_store.publish(snap.clone());
```

The final `save_snapshot` function should look like:

```rust
#[tauri::command]
async fn save_snapshot(
    app: tauri::AppHandle,
    state: tauri::State<'_, AppState>,
    mode: String,
    annotations: Vec<IncomingAnnotation>,
) -> Result<snapshot::Snapshot, String> {
    let frame = match mode.as_str() {
        "frozen" => state
            .latest_frame
            .lock()
            .ok()
            .and_then(|g| g.clone())
            .ok_or_else(|| "no cached frame for frozen mode".to_string())?,
        "live" => {
            overlay::hide(&app).map_err(|e| e.to_string())?;
            std::thread::sleep(std::time::Duration::from_millis(120));
            capture::capture_active_monitor(&app).map_err(|e| e.to_string())?
        }
        other => return Err(format!("unknown mode: {other}")),
    };

    let os_platform: &str = tauri_plugin_os::platform();
    let os_version: String = tauri_plugin_os::version().to_string();

    let snap =
        snapshot::build::build_and_write(&frame, &mode, annotations, os_platform, &os_version)
            .map_err(|e| e.to_string())?;

    // Notify any waiting MCP caller.
    state.snapshot_store.publish(snap.clone());

    let json = serde_json::to_string_pretty(&snap).map_err(|e| e.to_string())?;
    app.clipboard()
        .write_text(json)
        .map_err(|e| format!("clipboard: {e}"))?;

    overlay::hide(&app).map_err(|e| e.to_string())?;
    Ok(snap)
}
```

- [ ] **Step 5: Update run() to create the store, spawn the MCP server, and pass it to AppState**

Replace the `pub fn run()` function in `lib.rs`:

```rust
pub fn run() {
    let snapshot_store = Arc::new(mcp::store::SnapshotStore::new());
    let store_for_mcp = snapshot_store.clone();

    tauri::async_runtime::spawn(async move {
        mcp::server::serve(store_for_mcp).await;
    });

    tauri::Builder::default()
        .plugin(tauri_plugin_clipboard_manager::init())
        .plugin(tauri_plugin_os::init())
        .plugin(tauri_plugin_global_shortcut::Builder::new().build())
        .manage(AppState {
            latest_frame: Mutex::new(None),
            snapshot_store,
        })
        .setup(|app| {
            hotkey::register(app.handle())?;
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            capture_active_monitor,
            open_overlay_live,
            open_overlay_frozen,
            hide_overlay,
            get_latest_frame,
            save_snapshot,
            spike_capture::spike_capture,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
```

Add `use std::sync::Arc;` to the imports at the top of `lib.rs` (if not already present).

- [ ] **Step 6: Build and verify compilation**

```bash
cd src-tauri && cargo build 2>&1 | tail -20
```

Expected: compiles without errors. Fix any type errors (e.g., `Arc` import, `AppState` field names in tests).

- [ ] **Step 7: Run all mcp tests**

```bash
cd src-tauri && cargo test mcp:: -- --nocapture
```

Expected: all tests PASS.

- [ ] **Step 8: Commit**

```bash
git add src-tauri/src/lib.rs src-tauri/src/mcp/store.rs
git commit -m "Wire SnapshotStore into AppState; save_snapshot notifies MCP waiters"
```

---

## Task 5: Manual smoke test

Before writing the CC skill, verify the MCP server actually works end-to-end with `curl`.

- [ ] **Step 1: Launch the dev app**

```bash
npm run tauri dev
```

Wait for the app window to open (MCP server starts in background).

- [ ] **Step 2: Verify the server is listening**

```bash
curl -s http://localhost:7820/mcp \
  -H 'Content-Type: application/json' \
  -d '{"jsonrpc":"2.0","id":1,"method":"initialize","params":{}}' | python3 -m json.tool
```

Expected output contains `"name": "glitch-spot"`.

- [ ] **Step 3: List tools**

```bash
curl -s http://localhost:7820/mcp \
  -H 'Content-Type: application/json' \
  -d '{"jsonrpc":"2.0","id":2,"method":"tools/list","params":{}}' | python3 -m json.tool
```

Expected: 4 tools listed.

- [ ] **Step 4: Test get_latest_snapshot (returns null)**

```bash
curl -s http://localhost:7820/mcp \
  -H 'Content-Type: application/json' \
  -d '{"jsonrpc":"2.0","id":3,"method":"tools/call","params":{"name":"get_latest_snapshot","arguments":{}}}' | python3 -m json.tool
```

Expected: `"content": [{"type":"text","text":"null"}]`.

- [ ] **Step 5: Test wait_for_snapshot in one shell, save in another**

In one terminal, start a blocking wait (5-second timeout to keep it short):

```bash
curl -s http://localhost:7820/mcp \
  -H 'Content-Type: application/json' \
  -d '{"jsonrpc":"2.0","id":4,"method":"tools/call","params":{"name":"wait_for_snapshot","arguments":{"timeout_seconds":30}}}' &
```

Then in the running app, press the hotkey, draw a rectangle, add a note, and press Cmd+Return. 

Expected: the `curl` command (which was blocking) returns with the snapshot JSON within a second of saving.

- [ ] **Step 6: Test cancel_wait**

In one terminal, start a long wait:

```bash
curl -s http://localhost:7820/mcp \
  -H 'Content-Type: application/json' \
  -d '{"jsonrpc":"2.0","id":5,"method":"tools/call","params":{"name":"wait_for_snapshot","arguments":{"timeout_seconds":600}}}' &
WAIT_PID=$!
```

In a second terminal, cancel it:

```bash
curl -s http://localhost:7820/mcp \
  -H 'Content-Type: application/json' \
  -d '{"jsonrpc":"2.0","id":6,"method":"tools/call","params":{"name":"cancel_wait","arguments":{}}}' | python3 -m json.tool
```

Expected: the blocking `curl` from the first terminal exits immediately with `{"cancelled":true}`.

- [ ] **Step 7: Commit the smoke-test result**

```bash
git commit --allow-empty -m "MCP server smoke test: all curl checks pass"
```

---

## Task 6: `/glitch-spot` CC skill

**Files:**
- Create: `~/.claude/commands/glitch-spot.md`

The skill is a plain markdown instruction file that CC reads when `/glitch-spot` is invoked. It follows three state branches: arm, present, and cancel.

- [ ] **Step 1: Create the skill file**

```bash
cat > ~/.claude/commands/glitch-spot.md << 'SKILL'
Surface a Glitch Spot annotation snapshot in the current CC session.

## Arguments

- No args → arm the listener (wait for next snapshot)
- `cancel` → cancel any in-flight wait

---

## Arm state (no args)

1. **Health check:** call the `get_latest_snapshot` MCP tool (tool is registered under the `glitch-spot` MCP server). If the call fails with a connection error, output exactly:

   > Glitch Spot isn't running. Launch the app, then run `/glitch-spot` again.

   Then stop.

2. **Dispatch background agent:** Dispatch a background Agent (run_in_background: true) named `glitch-spot-waiter` with this exact prompt:

   > Call the `wait_for_snapshot` MCP tool with `timeout_seconds: 600`. When the tool returns, output the raw JSON result exactly as received — nothing else.

3. **Tell the user:**

   > Ready. I'm watching for your next snapshot — go annotate. I'll still be here if you want to talk while you look at the screen.

4. **Stay responsive.** Do not call any other tools. The background agent will complete when the user saves a snapshot.

---

## Present state (background agent completes)

When notified that the `glitch-spot-waiter` background agent completed:

1. Parse the agent result as JSON.

2. If the result contains `"cancelled": true`:
   - Output: > Snapshot listener was cancelled.
   - Stop.

3. **Read the annotated image** (if available):
   - Check `result.images.clean` — this is a local file path.
   - If `result.images.annotated` is non-null, read that path instead.
   - Use the `Read` tool to read the image file. CC will see it visually.
   - If the file does not exist, skip the image and work from text only.

4. **Format the summary:**

   ```
   Snapshot {id} — {N} annotation(s):
     1. Rect [{coords}] — "{note}"
     2. ...

   Context: {context.os.platform} {context.os.version}
   ```

5. **Decide action:**

   - **All notes are specific and actionable** (contain clear instructions like "fix this typo", "change colour to X", "reduce padding", "align left"): announce what you're about to do and start working immediately.
   - **Any note is vague or empty** ("something's wrong", "??", or empty string): show the summary and ask: > What would you like me to do?
   - **Mixed**: start on the specific ones, ask about the vague ones.

---

## Cancel state (args = "cancel")

1. Call the `cancel_wait` MCP tool.
2. Output: > Cancelled. Run `/glitch-spot` again when you're ready.

---

## Error handling

| Condition | Response |
|---|---|
| `get_latest_snapshot` throws connection error | "Glitch Spot isn't running. Launch the app and try again." |
| Image file path in snapshot does not exist on disk | Skip image, present text annotations only. Do not error. |
| Background agent times out (result contains `"reason":"timeout"`) | "Snapshot listener timed out after 10 minutes. Run `/glitch-spot` to start again." |
SKILL
```

- [ ] **Step 2: Verify the file was created**

```bash
cat ~/.claude/commands/glitch-spot.md | head -5
```

Expected: first line is `Surface a Glitch Spot annotation snapshot in the current CC session.`

- [ ] **Step 3: Register Glitch Spot as an MCP server in CC settings**

Open `~/.claude/settings.json` (create it if it doesn't exist). Add the MCP server entry:

```json
{
  "mcpServers": {
    "glitch-spot": {
      "url": "http://localhost:7820/mcp"
    }
  }
}
```

If the file already has content, merge — don't overwrite existing keys.

- [ ] **Step 4: Commit the skill**

The skill file lives outside the repo (`~/.claude/commands/`), so commit a copy in the repo for reference and future team members:

```bash
mkdir -p docs/skills
cp ~/.claude/commands/glitch-spot.md docs/skills/glitch-spot.md
git add docs/skills/glitch-spot.md
git commit -m "Add /glitch-spot CC skill and note MCP server registration"
```

---

## Task 7: End-to-end integration test with CC

Manual verification that the full chain works: CC skill → background agent → Glitch Spot MCP → snapshot → CC presents.

- [ ] **Step 1: Start the app**

```bash
npm run tauri dev
```

- [ ] **Step 2: Open a new CC session in a terminal**

```bash
claude
```

- [ ] **Step 3: Confirm the MCP tools are visible**

In the CC session, ask:
```
What MCP tools do you have available from glitch-spot?
```

Expected: CC lists `wait_for_snapshot`, `get_latest_snapshot`, `get_snapshot`, `cancel_wait`.

If CC says it doesn't see them, the `~/.claude/settings.json` registration is not picked up — restart CC and try again.

- [ ] **Step 4: Arm the listener**

In the CC session: `/glitch-spot`

Expected: CC says "Ready. I'm watching for your next snapshot..."

Confirm the main CC session is still responsive — type something like "what's 2+2?" — CC should answer normally.

- [ ] **Step 5: Take a snapshot**

Press the Glitch Spot hotkey (`Ctrl+Opt+Cmd+G` in v0.2). Draw a rectangle. Type a specific note, e.g. "the button text should be blue". Press Cmd+Return.

Expected: within ~2 seconds, the CC session wakes up, shows the annotated image (or clean image if annotated doesn't exist yet), and either starts working or summarises the annotation.

- [ ] **Step 6: Test the cancel path**

Type `/glitch-spot` in CC to arm again. Then immediately type `/glitch-spot cancel`.

Expected: CC outputs "Cancelled. Run /glitch-spot again when you're ready." without waiting for any snapshot.

- [ ] **Step 7: Final commit**

```bash
git commit --allow-empty -m "End-to-end integration test: CC /glitch-spot skill working"
```

---

## Self-review against spec

| Spec requirement | Task |
|---|---|
| `wait_for_snapshot(timeout_seconds?)` — blocks, returns snapshot JSON | Task 3 (call_tool), Task 4 (store.wait_for_next) |
| `get_latest_snapshot()` — non-blocking, returns null if empty | Task 3, Task 2 (store.get_latest) |
| `get_snapshot(id)` — returns by ID or error | Task 3, Task 2 (store.get_snapshot) |
| `cancel_wait()` — unblocks in-flight wait, returns `{cancelled:true}` | Task 3 (cancel_wait call), Task 2 (store.cancel) |
| `cancel_wait` is a no-op when nothing is waiting | Task 3 test |
| Localhost HTTP on port 7820 | Task 3 (serve function) |
| MCP server starts on app launch | Task 4 (run() spawns serve) |
| save_snapshot notifies the MCP store | Task 4 |
| One-time CC setup via settings.json | Task 6 step 3 |
| `/glitch-spot` skill: health check, arm, dispatch background agent | Task 6 |
| Skill stays responsive (background agent pattern) | Task 6 (run_in_background: true) |
| Present state: read annotated PNG visually | Task 6 present state step 3 |
| Present state: auto-act if notes specific, ask if vague | Task 6 present state step 5 |
| Cancel state: `/glitch-spot cancel` → cancel_wait | Task 6 cancel state |
| Error: Glitch Spot not running | Task 6 arm state step 1 |
| Error: image missing → text-only fallback | Task 6 present state step 3 |
| Error: timeout → user-facing message | Task 6 error table |

**No placeholders or TBDs remain in the plan.**

---

## After this plan

Next stages from the initiative:

- **v1.0-windows-and-relay** — once this is stable, add the Cloudflare relay so `images.annotated` is a real URL (removes the "file might not exist" edge case in the skill).
- **v0.3-persistence** — if not already shipped, the `images.clean` and `images.annotated` fields will point to `~/.glitch-spot/snapshots/{id}/` instead of `/tmp` once that plan lands. The skill needs no change — it reads whatever path is in the snapshot JSON.
