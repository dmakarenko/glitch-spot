// ABOUTME: axum router for /share/* read-only routes plus /healthz.
// ABOUTME: Reads from ~/.glitch-spot/snapshots/{id}/; verifies tokens constant-time.

use std::path::PathBuf;
use std::sync::Arc;

use axum::body::Body;
use axum::extract::{Path as AxumPath, State};
use axum::http::{header, StatusCode};
use axum::response::{IntoResponse, Response};
use axum::routing::get;
use axum::Router;

use crate::share::{html, token};
use crate::snapshot::Snapshot;

/// Local port the share server binds and the tunnel is pointed at. Canonical
/// home for the value so the bind, the tray status row, and the tunnel target
/// can't drift apart.
pub const SHARE_PORT: u16 = 7821;

#[derive(Clone)]
struct ShareState {
    base: Arc<PathBuf>,
}

pub fn build_share_router(base: PathBuf) -> Router {
    let state = ShareState {
        base: Arc::new(base),
    };
    Router::new()
        .route("/healthz", get(healthz))
        .route("/share/:id/:token/", get(landing))
        .route("/share/:id/:token/snapshot.json", get(snapshot_json))
        .route("/share/:id/:token/clean.png", get(serve_clean_png))
        .route("/share/:id/:token/annotated.png", get(serve_annotated_png))
        .with_state(state)
}

async fn healthz() -> &'static str {
    "ok"
}

/// Look up the snapshot dir, verify the token, and return its path.
/// Returns None for any failure condition — 404 is the unified response.
fn authorize(state: &ShareState, id: &str, provided_token: &str) -> Option<PathBuf> {
    // Guard against path traversal: snapshot ids are `snap_` + 12 hex chars.
    if !is_safe_id(id) {
        return None;
    }
    let dir = state.base.join(id);
    if !dir.is_dir() {
        return None;
    }
    let stored = token::read_for(&dir).ok().flatten()?;
    if !token::verify_ct(provided_token, &stored) {
        return None;
    }
    Some(dir)
}

fn is_safe_id(id: &str) -> bool {
    // Reject empty, path separators, and dot-sequences that could traverse directories.
    !id.is_empty() && !id.contains('/') && !id.contains('\\') && !id.contains("..")
}

async fn landing(
    State(state): State<ShareState>,
    AxumPath((id, tok)): AxumPath<(String, String)>,
) -> Response {
    let Some(dir) = authorize(&state, &id, &tok) else {
        return StatusCode::NOT_FOUND.into_response();
    };
    let json_bytes = match std::fs::read(dir.join(crate::snapshot::snapshot_json_filename(&id))) {
        Ok(b) => b,
        Err(_) => return StatusCode::NOT_FOUND.into_response(),
    };
    let snap: Snapshot = match serde_json::from_slice(&json_bytes) {
        Ok(s) => s,
        Err(_) => return StatusCode::NOT_FOUND.into_response(),
    };
    let body = html::render_share_page(&snap);
    ([(header::CONTENT_TYPE, "text/html; charset=utf-8")], body).into_response()
}

async fn snapshot_json(
    State(state): State<ShareState>,
    AxumPath((id, tok)): AxumPath<(String, String)>,
) -> Response {
    let Some(dir) = authorize(&state, &id, &tok) else {
        return StatusCode::NOT_FOUND.into_response();
    };
    match std::fs::read(dir.join(crate::snapshot::snapshot_json_filename(&id))) {
        Ok(bytes) => ([(header::CONTENT_TYPE, "application/json")], bytes).into_response(),
        Err(_) => StatusCode::NOT_FOUND.into_response(),
    }
}

async fn serve_png(state: ShareState, id: &str, tok: &str, filename: String) -> Response {
    let Some(dir) = authorize(&state, id, tok) else {
        return StatusCode::NOT_FOUND.into_response();
    };
    match std::fs::read(dir.join(&filename)) {
        Ok(bytes) => ([(header::CONTENT_TYPE, "image/png")], Body::from(bytes)).into_response(),
        Err(_) => StatusCode::NOT_FOUND.into_response(),
    }
}

async fn serve_clean_png(
    State(state): State<ShareState>,
    AxumPath((id, tok)): AxumPath<(String, String)>,
) -> Response {
    let filename = crate::snapshot::clean_png_filename(&id);
    serve_png(state, &id, &tok, filename).await
}

async fn serve_annotated_png(
    State(state): State<ShareState>,
    AxumPath((id, tok)): AxumPath<(String, String)>,
) -> Response {
    let filename = crate::snapshot::annotated_png_filename(&id);
    serve_png(state, &id, &tok, filename).await
}

/// Start the share server. Binds 127.0.0.1:port; logs and returns on bind
/// failure. The shared `bound` flag is set to true on successful bind and
/// cleared if/when serve returns — the tray uses it as the health signal,
/// mirroring the MCP server.
pub async fn serve_share(base: PathBuf, port: u16, bound: Arc<std::sync::atomic::AtomicBool>) {
    use std::sync::atomic::Ordering;
    let app = build_share_router(base);
    let addr = format!("127.0.0.1:{port}");
    let listener = match tokio::net::TcpListener::bind(&addr).await {
        Ok(l) => l,
        Err(e) => {
            eprintln!("[glitch-spot share] failed to bind {addr}: {e}");
            return;
        }
    };
    bound.store(true, Ordering::SeqCst);
    if let Err(e) = axum::serve(listener, app).await {
        eprintln!("[glitch-spot share] server error: {e}");
    }
    bound.store(false, Ordering::SeqCst);
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::body::Body;
    use axum::http::{Method, Request, StatusCode};
    use tower::ServiceExt;

    fn write_fixture(base: &std::path::Path, id: &str, token: &str) -> std::path::PathBuf {
        let dir = base.join(id);
        std::fs::create_dir_all(&dir).unwrap();
        let snapshot_json = serde_json::json!({
            "id": id,
            "created_at": "2026-05-26T12:00:00Z",
            "mode": "frozen",
            "annotations": [{
                "id": "a1", "shape": "rect",
                "coords": { "x": 0.0, "y": 0.0, "w": 10.0, "h": 10.0 },
                "note": "fixture note",
                "label": "01"
            }],
            "context": {
                "screen": { "width": 1440, "height": 900, "scale_factor": 2.0, "display_index": 0 },
                "os": { "platform": "macos", "version": "26.0" }
            },
            "images": { "clean": "x", "annotated": "y" }
        });
        std::fs::write(
            dir.join(crate::snapshot::snapshot_json_filename(id)),
            serde_json::to_vec_pretty(&snapshot_json).unwrap(),
        )
        .unwrap();
        std::fs::write(
            dir.join(crate::snapshot::clean_png_filename(id)),
            b"clean-bytes",
        )
        .unwrap();
        std::fs::write(
            dir.join(crate::snapshot::annotated_png_filename(id)),
            b"annotated-bytes",
        )
        .unwrap();
        crate::share::token::write_for(&dir, token).unwrap();
        dir
    }

    async fn get(app: axum::Router, uri: &str) -> (StatusCode, Vec<u8>, Option<String>) {
        let req = Request::builder()
            .method(Method::GET)
            .uri(uri)
            .body(Body::empty())
            .unwrap();
        let resp = app.oneshot(req).await.unwrap();
        let status = resp.status();
        let ct = resp
            .headers()
            .get("content-type")
            .and_then(|h| h.to_str().ok())
            .map(|s| s.to_string());
        let bytes = axum::body::to_bytes(resp.into_body(), usize::MAX)
            .await
            .unwrap()
            .to_vec();
        (status, bytes, ct)
    }

    #[tokio::test]
    async fn healthz_returns_ok() {
        let temp = tempfile::TempDir::new().unwrap();
        let app = build_share_router(temp.path().to_path_buf());
        let (status, body, _) = get(app, "/healthz").await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(body, b"ok");
    }

    #[tokio::test]
    async fn html_route_returns_html_view() {
        let temp = tempfile::TempDir::new().unwrap();
        write_fixture(temp.path(), "snap_aaa", "tok-aaa");
        let app = build_share_router(temp.path().to_path_buf());
        let (status, body, ct) = get(app, "/share/snap_aaa/tok-aaa/").await;
        assert_eq!(status, StatusCode::OK);
        assert!(ct.unwrap_or_default().starts_with("text/html"));
        let body_str = std::str::from_utf8(&body).unwrap();
        assert!(body_str.contains("fixture note"));
        assert!(body_str.contains(r#"<img src="annotated.png""#));
    }

    #[tokio::test]
    async fn json_route_returns_snapshot_json() {
        let temp = tempfile::TempDir::new().unwrap();
        write_fixture(temp.path(), "snap_bbb", "tok-bbb");
        let app = build_share_router(temp.path().to_path_buf());
        let (status, body, ct) = get(app, "/share/snap_bbb/tok-bbb/snapshot.json").await;
        assert_eq!(status, StatusCode::OK);
        assert!(ct.unwrap_or_default().starts_with("application/json"));
        let val: serde_json::Value = serde_json::from_slice(&body).unwrap();
        assert_eq!(val["id"], "snap_bbb");
    }

    #[tokio::test]
    async fn clean_png_route_returns_image() {
        let temp = tempfile::TempDir::new().unwrap();
        write_fixture(temp.path(), "snap_ccc", "tok-ccc");
        let app = build_share_router(temp.path().to_path_buf());
        let (status, body, ct) = get(app, "/share/snap_ccc/tok-ccc/clean.png").await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(ct.unwrap_or_default(), "image/png");
        assert_eq!(body, b"clean-bytes");
    }

    #[tokio::test]
    async fn annotated_png_route_returns_image() {
        let temp = tempfile::TempDir::new().unwrap();
        write_fixture(temp.path(), "snap_ddd", "tok-ddd");
        let app = build_share_router(temp.path().to_path_buf());
        let (status, body, ct) = get(app, "/share/snap_ddd/tok-ddd/annotated.png").await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(ct.unwrap_or_default(), "image/png");
        assert_eq!(body, b"annotated-bytes");
    }

    #[tokio::test]
    async fn unknown_id_returns_404() {
        let temp = tempfile::TempDir::new().unwrap();
        let app = build_share_router(temp.path().to_path_buf());
        let (status, _, _) = get(app, "/share/snap_nope/anything/snapshot.json").await;
        assert_eq!(status, StatusCode::NOT_FOUND);
    }

    #[tokio::test]
    async fn snapshot_without_token_file_returns_404() {
        let temp = tempfile::TempDir::new().unwrap();
        let dir = temp.path().join("snap_eee");
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(
            dir.join(crate::snapshot::snapshot_json_filename("snap_eee")),
            b"{}",
        )
        .unwrap();
        let app = build_share_router(temp.path().to_path_buf());
        let (status, _, _) = get(app, "/share/snap_eee/whatever/snapshot.json").await;
        assert_eq!(status, StatusCode::NOT_FOUND);
    }

    #[tokio::test]
    async fn wrong_token_returns_404() {
        let temp = tempfile::TempDir::new().unwrap();
        write_fixture(temp.path(), "snap_fff", "real-token");
        let app = build_share_router(temp.path().to_path_buf());
        let (status, _, _) = get(app, "/share/snap_fff/wrong-token/snapshot.json").await;
        assert_eq!(status, StatusCode::NOT_FOUND);
    }

    #[tokio::test]
    async fn valid_token_missing_file_returns_404() {
        let temp = tempfile::TempDir::new().unwrap();
        let dir = write_fixture(temp.path(), "snap_ggg", "tok-ggg");
        std::fs::remove_file(dir.join(crate::snapshot::clean_png_filename("snap_ggg"))).unwrap();
        let app = build_share_router(temp.path().to_path_buf());
        let (status, _, _) = get(app, "/share/snap_ggg/tok-ggg/clean.png").await;
        assert_eq!(status, StatusCode::NOT_FOUND);
    }

    #[tokio::test]
    async fn serve_share_sets_bound_after_successful_bind() {
        use std::sync::atomic::{AtomicBool, Ordering};
        let temp = tempfile::TempDir::new().unwrap();
        let bound = Arc::new(AtomicBool::new(false));
        let bound_clone = bound.clone();

        // port=0 → OS picks a free ephemeral port; we only assert the flag flips.
        let task = tokio::spawn(async move {
            serve_share(temp.path().to_path_buf(), 0, bound_clone).await;
        });

        for _ in 0..50 {
            if bound.load(Ordering::SeqCst) {
                break;
            }
            tokio::time::sleep(tokio::time::Duration::from_millis(10)).await;
        }

        assert!(
            bound.load(Ordering::SeqCst),
            "bound should be set after successful bind"
        );
        task.abort();
    }

    #[tokio::test]
    async fn router_has_no_mcp_route() {
        let temp = tempfile::TempDir::new().unwrap();
        let app = build_share_router(temp.path().to_path_buf());
        let req = Request::builder()
            .method(Method::POST)
            .uri("/mcp")
            .body(Body::from("{}"))
            .unwrap();
        let resp = app.oneshot(req).await.unwrap();
        assert_eq!(
            resp.status(),
            StatusCode::NOT_FOUND,
            "share router must not expose /mcp"
        );
    }
}
