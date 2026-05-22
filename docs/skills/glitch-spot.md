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
