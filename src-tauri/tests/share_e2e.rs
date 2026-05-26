// ABOUTME: End-to-end integration test for the share flow using MockTunnelProvider.
// ABOUTME: Writes a real snapshot to disk, then hits the share Router via tower::oneshot.

use std::sync::Arc;

use axum::body::Body;
use axum::http::{Method, Request, StatusCode};
use glitch_spot::share::{router::build_share_router, token, tunnel::mock::MockTunnelProvider, tunnel::TunnelManager};
use tower::ServiceExt;

fn write_real_snapshot(base: &std::path::Path, id: &str) -> std::path::PathBuf {
    let dir = base.join(id);
    std::fs::create_dir_all(&dir).unwrap();
    let snap = serde_json::json!({
        "id": id,
        "created_at": "2026-05-26T12:00:00Z",
        "mode": "frozen",
        "annotations": [{
            "id": "a1", "shape": "rect",
            "coords": { "x": 0.0, "y": 0.0, "w": 10.0, "h": 10.0 },
            "note": "the cancel button is misaligned by 3 px"
        }],
        "context": {
            "screen": { "width": 1440, "height": 900, "scale_factor": 2.0, "display_index": 0 },
            "os": { "platform": "macos", "version": "26.0" }
        },
        "images": { "clean": "x", "annotated": "y" }
    });
    std::fs::write(dir.join("snapshot.json"), serde_json::to_vec_pretty(&snap).unwrap()).unwrap();
    std::fs::write(dir.join("clean.png"), b"\x89PNG-fixture-clean").unwrap();
    std::fs::write(dir.join("annotated.png"), b"\x89PNG-fixture-annotated").unwrap();
    dir
}

#[tokio::test]
async fn share_flow_yields_html_view_with_note() {
    let temp = tempfile::TempDir::new().unwrap();
    let snap_id = "snap_abcdef012345";
    let snap_dir = write_real_snapshot(temp.path(), snap_id);

    // Mock the tunnel.
    let mgr = Arc::new(TunnelManager::new());
    let provider = Arc::new(MockTunnelProvider::with_url("https://e2e.trycloudflare.com"));
    let tunnel_url = mgr.clone().start_tunnel(provider, 7821).await.unwrap();
    assert_eq!(tunnel_url, "https://e2e.trycloudflare.com");

    // Generate token.
    let tok = token::generate();
    token::write_for(&snap_dir, &tok).unwrap();

    // Hit the share router directly (no real network).
    let app = build_share_router(temp.path().to_path_buf());
    let landing_path = format!("/share/{snap_id}/{tok}/");
    let req = Request::builder().method(Method::GET).uri(&landing_path).body(Body::empty()).unwrap();
    let resp = app.oneshot(req).await.unwrap();

    assert_eq!(resp.status(), StatusCode::OK);
    let body_bytes = axum::body::to_bytes(resp.into_body(), usize::MAX).await.unwrap();
    let body = std::str::from_utf8(&body_bytes).unwrap();
    assert!(body.contains("the cancel button is misaligned by 3 px"));
    assert!(body.contains(r#"<img src="annotated.png""#));

    mgr.shutdown().await;
}
