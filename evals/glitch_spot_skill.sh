#!/usr/bin/env bash
# ABOUTME: Automated evals for the /glitch-spot CC skill.
# ABOUTME: Builds the standalone MCP server, runs 4 evals, reports pass/fail.
#
# Usage: bash evals/glitch_spot_skill.sh
# Run from the repository root.
#
# Requirements:
#   - `claude` CLI on PATH (Claude Code)
#   - Rust toolchain on PATH (cargo)
#   - No Glitch Spot app or other process on port 7820

set -uo pipefail

PASS=0
FAIL=0
SERVER_PID=""

# Portable timeout: prefer GNU timeout, fall back to gtimeout (brew install coreutils),
# or run without a timeout if neither is available.
_TIMEOUT_CMD=""
if command -v timeout &>/dev/null; then
    _TIMEOUT_CMD="timeout"
elif command -v gtimeout &>/dev/null; then
    _TIMEOUT_CMD="gtimeout"
fi

run_with_timeout() {
    local secs="$1"; shift
    if [ -n "$_TIMEOUT_CMD" ]; then
        "$_TIMEOUT_CMD" "$secs" "$@"
    else
        echo "  [warn] timeout command not found — running without timeout limit" >&2
        "$@"
    fi
}

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
BINARY="$REPO_ROOT/src-tauri/target/debug/mcp_server"

# ── Helpers ──────────────────────────────────────────────────────────────────

build_server() {
    echo "Building mcp_server..."
    # Source cargo env if cargo is not already on PATH
    if ! command -v cargo &>/dev/null; then
        # shellcheck source=/dev/null
        [ -f "$HOME/.cargo/env" ] && source "$HOME/.cargo/env"
    fi
    (cd "$REPO_ROOT/src-tauri" && cargo build --bin mcp_server) \
        || { echo "ERROR: cargo build failed (see output above)."; exit 1; }
}

start_server() {
    "$BINARY" &
    SERVER_PID=$!

    # Give the binary a moment to bind or crash
    sleep 0.3
    if ! kill -0 "$SERVER_PID" 2>/dev/null; then
        echo "ERROR: mcp_server exited immediately — port 7820 may already be in use"
        exit 1
    fi

    # Poll until ready (up to ~10s of sleep plus curl timeouts)
    for _ in $(seq 1 20); do
        if curl -s --max-time 1 http://127.0.0.1:7820/mcp \
            -H 'Content-Type: application/json' \
            -d '{"jsonrpc":"2.0","id":0,"method":"initialize","params":{}}' 2>/dev/null \
            | grep -q "glitch-spot"; then
            return
        fi
        sleep 0.5
    done
    echo "ERROR: server did not start within 10s"
    exit 1
}

stop_server() {
    if [ -n "$SERVER_PID" ]; then
        kill "$SERVER_PID" 2>/dev/null || true
        wait "$SERVER_PID" 2>/dev/null || true
        SERVER_PID=""
    fi
}

pass() { echo "  ✓ $1"; PASS=$((PASS + 1)); }
fail() { echo "  ✗ $1"; echo "    expected: $2"; echo "    got:      $3"; FAIL=$((FAIL + 1)); }

assert_contains() {
    local name="$1" output="$2" expected="$3"
    if echo "$output" | grep -qi "$expected"; then
        pass "$name"
    else
        fail "$name" "$expected" "$(echo "$output" | head -3)"
    fi
}

# ── Evals ────────────────────────────────────────────────────────────────────

eval_not_running() {
    echo ""
    echo "Eval 1: health check when server is not running"
    stop_server
    # If something else owns port 7820, don't kill it — that could destroy unsaved state
    # in the real Glitch Spot app. Fail cleanly instead.
    if lsof -ti:7820 &>/dev/null 2>&1; then
        fail "CC reports server not running" "(port 7820 free)" "port 7820 is in use by another process — stop it first"
        return
    fi
    sleep 0.1

    local output
    # Assertion text is coupled to prose in docs/skills/glitch-spot.md — update both if skill wording changes
    output=$(run_with_timeout 60 claude --no-session-persistence --print "/glitch-spot" 2>&1)
    assert_contains "CC reports server not running" "$output" "wasn't running"
}

eval_arm() {
    echo ""
    echo "Eval 2: arm state (server running, no snapshot yet)"
    start_server

    local output
    # Assertion text is coupled to prose in docs/skills/glitch-spot.md — update both if skill wording changes
    # Only verifies the arm-state text response. In --print mode the background
    # agent dispatched by the skill does not persist — multi-turn delivery is
    # covered by the manual eval 5 guide below.
    output=$(run_with_timeout 60 claude --no-session-persistence --print "/glitch-spot" 2>&1)
    assert_contains "CC says ready/watching" "$output" "watching"

    stop_server
}

eval_cancel() {
    echo ""
    echo "Eval 3: cancel state"
    start_server

    local output
    # Assertion text is coupled to prose in docs/skills/glitch-spot.md — update both if skill wording changes
    output=$(run_with_timeout 60 claude --no-session-persistence --print "/glitch-spot cancel" 2>&1)
    assert_contains "CC says cancelled" "$output" "Cancelled"

    stop_server
}

eval_inject_endpoint() {
    echo ""
    echo "Eval 4: /test/inject endpoint"
    start_server

    # Verify the store starts empty before inject
    local before_inject
    before_inject=$(curl -s http://127.0.0.1:7820/mcp \
        -H 'Content-Type: application/json' \
        -d '{"jsonrpc":"2.0","id":0,"method":"tools/call","params":{"name":"get_latest_snapshot","arguments":{}}}')
    assert_contains "store empty before inject" "$before_inject" '"text":"null"'

    local snap_json
    snap_json='{"id":"snap_eval_001","created_at":"2026-01-01T00:00:00Z","mode":"frozen","annotations":[{"id":"a1","shape":"rect","coords":{"x":100.0,"y":100.0,"w":200.0,"h":50.0},"note":"make this button blue"}],"context":{"screen":{"width":1440,"height":900,"scale_factor":2.0,"display_index":0},"os":{"platform":"macos","version":"15.4.0"}},"images":{"clean":"/tmp/snap_eval_001.png","annotated":null}}'

    # Inject
    local inject_resp
    inject_resp=$(curl -s http://127.0.0.1:7820/test/inject \
        -H 'Content-Type: application/json' \
        -d "$snap_json")
    assert_contains "inject endpoint returns ok" "$inject_resp" '"ok":true'

    # Verify stored
    local latest_resp
    latest_resp=$(curl -s http://127.0.0.1:7820/mcp \
        -H 'Content-Type: application/json' \
        -d '{"jsonrpc":"2.0","id":1,"method":"tools/call","params":{"name":"get_latest_snapshot","arguments":{}}}')
    assert_contains "injected snapshot retrievable via get_latest_snapshot" "$latest_resp" "snap_eval_001"

    # Verify annotation note is in there
    assert_contains "injected annotation note preserved" "$latest_resp" "make this button blue"

    stop_server
}

# ── Manual eval guide (full chain) ───────────────────────────────────────────

print_manual_guide() {
    cat <<'GUIDE'

─────────────────────────────────────────────────────────────────────────────
Manual eval 5: full snapshot-delivery chain (requires interactive CC session)
─────────────────────────────────────────────────────────────────────────────

This eval cannot be automated because it requires a multi-turn CC session.

Steps:
  1. Start the server:
       src-tauri/target/debug/mcp_server

  2. In a new terminal, open a CC session:
       claude

  3. In CC, arm the listener:
       /glitch-spot
     Expected: "Ready. I'm watching…" (CC stays responsive)

  4. In a third terminal, inject a synthetic snapshot:
       curl -s http://127.0.0.1:7820/test/inject \
         -H 'Content-Type: application/json' \
         -d '{"id":"snap_manual_001","created_at":"2026-01-01T00:00:00Z","mode":"frozen","annotations":[{"id":"a1","shape":"rect","coords":{"x":140.0,"y":310.0,"w":280.0,"h":60.0},"note":"make this button blue"}],"context":{"screen":{"width":1440,"height":900,"scale_factor":2.0,"display_index":0},"os":{"platform":"macos","version":"15.4.0"}},"images":{"clean":"/tmp/clean.png","annotated":null}}'

  5. Back in the CC session, it should wake up and say something like:
       "Snapshot snap_manual_001 — 1 annotation:
          1. Rect [...] — "make this button blue"
       On it — I'll look for a button element and change its colour."

  6. If CC asks "What would you like me to do?" instead of acting, the
     auto-act threshold is correct (the note was judged specific enough but
     CC deferred). Respond with "go" to confirm the flow works.
─────────────────────────────────────────────────────────────────────────────
GUIDE
}

# ── Main ─────────────────────────────────────────────────────────────────────

main() {
    # Verify claude is available
    if ! command -v claude &>/dev/null; then
        echo "ERROR: 'claude' CLI not found on PATH. Install Claude Code first."
        exit 1
    fi

    # Verify glitch-spot MCP server is registered with claude
    if ! claude mcp list 2>/dev/null | grep -q "glitch-spot"; then
        echo "ERROR: glitch-spot MCP server not registered with Claude Code."
        echo "Run: claude mcp add --transport http glitch-spot http://127.0.0.1:7820/mcp --scope user"
        exit 1
    fi

    build_server

    echo ""
    echo "Running /glitch-spot skill evals"
    echo "================================="

    eval_arm
    eval_cancel
    eval_inject_endpoint
    eval_not_running  # Run last — skill may auto-launch the real app on this path

    echo ""
    echo "================================="
    echo "Results: $PASS passed, $FAIL failed"

    print_manual_guide

    [ "$FAIL" -eq 0 ]
}

trap stop_server EXIT INT TERM
main
