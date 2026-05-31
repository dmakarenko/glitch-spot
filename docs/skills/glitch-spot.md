Surface a Glitch Spot annotation snapshot in the current CC session.

## Arguments

- No args → arm the listener (wait for next snapshot)
- `cancel` → cancel any in-flight wait

The waiter polls the Glitch Spot MCP server directly over HTTP from a single
background shell process. The wait costs zero model turns — the loop blocks in
`curl`, not in an agent, and the session is only re-invoked when a snapshot
arrives, the wait is cancelled, or it times out.

---

## Arm state (no args)

1. **Health check.** Run this via Bash — it resolves the configured MCP port
   (default 7820) and pings the server:

   ```bash
   PORT=$(sed -n '/^\[mcp\]/,/^\[/{s/^[[:space:]]*port[[:space:]]*=[[:space:]]*\([0-9]\{1,\}\).*/\1/p;}' "$HOME/.glitch-spot/config.toml" 2>/dev/null | head -1); PORT=${PORT:-7820}
   curl -s -m 5 -X POST "http://127.0.0.1:${PORT}/mcp" -H 'content-type: application/json' \
     -d '{"jsonrpc":"2.0","id":1,"method":"tools/call","params":{"name":"get_latest_snapshot","arguments":{}}}' \
     | jq -e '.result' >/dev/null && echo HEALTHY || echo UNHEALTHY
   ```

   `HEALTHY` → continue to step 2.

   `UNHEALTHY` (connection refused, timeout, non-JSON, or no `result` field):
   - Run `open -a "Glitch Spot"` via Bash. If the app bundle is not found, try `open /Applications/Glitch\ Spot.app` as a fallback.
   - Output: > Glitch Spot wasn't running — I've launched it. Give it a moment to start, then run `/glitch-spot` again.
   - Stop.

2. **Pick a per-invocation result path.** Run `mktemp -t glitch-spot-snapshot`
   via Bash and note the path it prints — call it `RESULT_PATH`. Each
   `/glitch-spot` run gets its own file so concurrent sessions never clobber each
   other's result. You MUST reference this exact path both when launching the
   waiter (as `OUT`) and when reading the result in the present state.

3. **Launch the background waiter.** Run this via Bash with `run_in_background: true`,
   substituting your `RESULT_PATH` into `OUT` on the first line.
   It long-polls `wait_for_snapshot` (the server caps each call at 45s and returns
   `pending`; the loop retries with `last_seen_id` so a snapshot that lands in the
   gap is never missed) and writes the final result JSON to `OUT`. The `curl -m 60`
   per-call timeout must stay above the server's 45s chunk cap — if the cap ever
   rises past 60s, curl would abort mid-wait and the loop would write a spurious
   error:

   ```bash
   OUT=<RESULT_PATH>   # the mktemp path from step 2
   PORT=$(sed -n '/^\[mcp\]/,/^\[/{s/^[[:space:]]*port[[:space:]]*=[[:space:]]*\([0-9]\{1,\}\).*/\1/p;}' "$HOME/.glitch-spot/config.toml" 2>/dev/null | head -1); PORT=${PORT:-7820}
   ENDPOINT="http://127.0.0.1:${PORT}/mcp"
   rm -f "$OUT"; last=""
   for i in $(seq 1 14); do
     if [ -n "$last" ]; then args=$(printf '{"timeout_seconds":600,"last_seen_id":"%s"}' "$last"); else args='{"timeout_seconds":600}'; fi
     resp=$(curl -s -m 60 -X POST "$ENDPOINT" -H 'content-type: application/json' \
       -d "{\"jsonrpc\":\"2.0\",\"id\":1,\"method\":\"tools/call\",\"params\":{\"name\":\"wait_for_snapshot\",\"arguments\":$args}}")
     inner=$(printf '%s' "$resp" | jq -r '.result.content[0].text // empty' 2>/dev/null)
     if [ -z "$inner" ]; then printf '{"error":"no response from Glitch Spot MCP server"}' > "$OUT"; exit 0; fi
     if [ "$(printf '%s' "$inner" | jq -r '.cancelled // empty')" = "true" ]; then printf '%s' "$inner" > "$OUT"; exit 0; fi
     if [ "$(printf '%s' "$inner" | jq -r '.pending // empty')" = "true" ]; then last=$(printf '%s' "$inner" | jq -r '.last_id // empty'); continue; fi
     printf '%s' "$inner" > "$OUT"; exit 0
   done
   printf '{"cancelled":true,"reason":"timeout"}' > "$OUT"
   ```

4. **Tell the user:**

   > Ready. I'm watching for your next snapshot — go annotate. I'll still be here if you want to talk while you look at the screen.

5. **Stay responsive.** Do not call any other tools. The background command completes when the user saves a snapshot.

---

## Present state (background command completes)

When the background waiter command completes (the harness re-invokes you):

1. Read `RESULT_PATH` — the exact `mktemp` path you generated when arming — and parse it as JSON — call this `result`. This is the raw snapshot object, or a status object.

2. If `result.reason == "timeout"` or (`result.cancelled == true` and `result.reason`): output:
   > Snapshot listener timed out after 10 minutes. Run `/glitch-spot` to start again.
   Stop.

3. If `result.cancelled == true`: output:
   > Snapshot listener was cancelled.
   Stop.

4. If `result.error` is present: output:
   > Glitch Spot returned an error while waiting. Try again.
   Stop.

5. **Read the annotated image** (if available):
   - If `result.images.annotated` is non-null, read that file path.
   - Otherwise, read `result.images.clean`.
   - If the file does not exist on disk, skip the image and work from text only.

6. **Format the summary:**

   ```
   Snapshot {result.id} — {N} annotation(s):
     1. Rect [{coords}] — "{note}"
     2. ...

   Context: {result.context.os.platform} {result.context.os.version}
   ```

7. **Decide action:**

   - **No annotations** (empty array): show the image and ask: > I see the snapshot but no annotations — what would you like me to do?
   - **All notes are specific and actionable** (contain clear instructions like "fix this typo", "change colour to X", "reduce padding", "align left"): announce what you're about to do and start working immediately.
   - **Any note is vague or empty** ("something's wrong", "??", or empty string): show the summary and ask: > What would you like me to do?
   - **Mixed**: start on the specific ones, ask about the vague ones.

---

## Cancel state (args = "cancel")

1. Call `cancel_wait` over HTTP — this unblocks the in-flight `wait_for_snapshot`, so the background waiter writes `{cancelled:true}` and exits:

   ```bash
   PORT=$(sed -n '/^\[mcp\]/,/^\[/{s/^[[:space:]]*port[[:space:]]*=[[:space:]]*\([0-9]\{1,\}\).*/\1/p;}' "$HOME/.glitch-spot/config.toml" 2>/dev/null | head -1); PORT=${PORT:-7820}
   curl -s -m 5 -X POST "http://127.0.0.1:${PORT}/mcp" -H 'content-type: application/json' \
     -d '{"jsonrpc":"2.0","id":1,"method":"tools/call","params":{"name":"cancel_wait","arguments":{}}}' >/dev/null
   ```

2. Output: > Cancelled. Run `/glitch-spot` again when you're ready.

---

## Error handling

| Condition | Response |
|---|---|
| Health check returns `UNHEALTHY` | Try to launch the app via `open -a "Glitch Spot"`, output "wasn't running — I've launched it. Give it a moment, then run `/glitch-spot` again." |
| Image file path in snapshot does not exist on disk | Skip image, present text annotations only. Do not error. |
| Waiter times out (result contains `"reason":"timeout"`) | "Snapshot listener timed out after 10 minutes. Run `/glitch-spot` to start again." |
