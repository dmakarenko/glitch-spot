// ABOUTME: Standalone MCP server binary for evals and manual testing.
// ABOUTME: Starts the MCP server on 127.0.0.1:7820 with an extra /test/inject endpoint.

use axum::{extract::State, routing::post, Json, Router};
use glitch_spot::mcp::{server::build_router, store::SnapshotStore};
use glitch_spot::snapshot::Snapshot;
use serde_json::{json, Value};
use std::sync::Arc;

async fn inject(
    State(store): State<Arc<SnapshotStore>>,
    Json(snap): Json<Snapshot>,
) -> Json<Value> {
    store.publish(snap);
    Json(json!({ "ok": true }))
}

#[tokio::main]
async fn main() {
    let store = Arc::new(SnapshotStore::new());

    let inject_router = Router::new()
        .route("/test/inject", post(inject))
        .with_state(store.clone());

    let app = build_router(store).merge(inject_router);

    let listener = tokio::net::TcpListener::bind("127.0.0.1:7820")
        .await
        .expect("failed to bind 127.0.0.1:7820 — kill any running Glitch Spot instance first");

    eprintln!("[glitch-spot mcp] listening on 127.0.0.1:7820 (inject endpoint enabled)");
    axum::serve(listener, app).await.expect("server error");
}
