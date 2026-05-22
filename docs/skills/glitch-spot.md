Surface a Glitch Spot annotation snapshot in the current CC session.

## Arguments

- No args → arm the listener (wait for next snapshot)
- `cancel` → cancel any in-flight wait

---

## Arm state (no args)

1. **Health check:** call the `get_latest_snapshot` MCP tool (tool is registered under the `glitch-spot` MCP server). If the call fails for any reason — connection refused, tool not registered, timeout, or any error — output exactly:

   > Glitch Spot isn't running. Launch the app, then run `/glitch-spot` again.

   Then stop.

2. **Dispatch background agent:** Dispatch a background Agent (run_in_background: true) named `glitch-spot-waiter` with this exact prompt:

   > Call the `wait_for_snapshot` MCP tool with `timeout_seconds: 600`. When the tool returns, output the raw JSON result on a single line, with no commentary, no markdown, and no code fences. If the tool errors, output `{"error": "<error message>"}` and nothing else.

3. **Tell the user:**

   > Ready. I'm watching for your next snapshot — go annotate. I'll still be here if you want to talk while you look at the screen.

4. **Stay responsive.** Do not call any other tools. The background agent will complete when the user saves a snapshot.

---

## Present state (background agent completes)

When notified that the `glitch-spot-waiter` background agent completed:

1. Parse the agent's output as JSON — call this `envelope`.

2. Extract `envelope.result.content[0].text`, then JSON-parse that string to get `snapshot`. If the result contains `"cancelled": true` at the envelope level OR inside the snapshot text, treat as cancelled.

3. If `snapshot.reason == "timeout"`: output:
   > Snapshot listener timed out after 10 minutes. Run `/glitch-spot` to start again.

   Stop.

4. If `snapshot.cancelled == true`: output:
   > Snapshot listener was cancelled.

   Stop.

5. If `envelope.result.content[0].text` contains an `"error"` key: output:
   > Glitch Spot returned an error while waiting. Try again.

   Stop.

6. **Read the annotated image** (if available):
   - If `snapshot.images.annotated` is non-null, read that file path.
   - Otherwise, read `snapshot.images.clean`.
   - If the file does not exist on disk, skip the image and work from text only.

7. **Format the summary:**

   ```
   Snapshot {id} — {N} annotation(s):
     1. Rect [{coords}] — "{note}"
     2. ...

   Context: {context.os.platform} {context.os.version}
   ```

8. **Decide action:**

   - **No annotations** (empty array): show the image and ask: > I see the snapshot but no annotations — what would you like me to do?
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
| `get_latest_snapshot` fails for any reason | "Glitch Spot isn't running. Launch the app and try again." |
| Image file path in snapshot does not exist on disk | Skip image, present text annotations only. Do not error. |
| Background agent times out (snapshot contains `"reason":"timeout"`) | "Snapshot listener timed out after 10 minutes. Run `/glitch-spot` to start again." |
