# Window-Mode Freeze Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

> Part of initiative: docs/initiatives/glitch-spot/INITIATIVE.md
> Design seed: docs/seeds/2026-06-02-window-mode-freeze-seed.md

**Goal:** In window mode, present a frozen, isolated screenshot of the selected window (above everything) and annotate on it, instead of annotating on the live window seen through the transparent overlay.

**Architecture:** On window pick, the overlay is shown first (dim surround + transparent rect — live window shows through the gap), then the backend captures the isolated window (`capture_window`, single-window filter), caches it in `state.latest_frame`, and pushes it to the overlay, which resolves it into the window rect with the existing chromatic-shake. Save reuses the cached frame (no recapture), exactly like monitor freeze. One capture total instead of two.

**Tech Stack:** Tauri v2 (Rust), React + TypeScript (inline styles), Vitest, screencapturekit.

**Task order rationale:** Each task leaves the app working. Backend capture+push lands first (save still recaptures redundantly — works), then the frontend renders the frame, then the save path is switched to reuse the cached frame (drops the redundant recapture).

---

### Task 1: Show-first capture + cache + push in `picker_select`

**Files:**
- Modify: `src-tauri/src/lib.rs` (`picker_select`, lines ~536-604)

This is backend IPC wiring around `tauri::AppHandle` + a real screen capture. It is **not** unit-testable without mocking `AppHandle` (which the codebase never does) — it is verified by `cargo build`, `cargo clippy`, and the manual window-mode smoke test in Task 6. No red-green step here.

- [ ] **Step 1: Reorder to show-first, then capture/cache/push**

In `picker_select`, the tail currently reads:

```rust
    state
        .picker_state
        .try_select_window(picker_session::AnnotatingSession {
            window_id,
            window: window.clone(),
            display,
        })?;

    overlay::show_window_mode(&app, window.display_index, &payload).map_err(|e| e.to_string())?;
    Ok(())
}
```

Replace that tail with:

```rust
    state
        .picker_state
        .try_select_window(picker_session::AnnotatingSession {
            window_id,
            window: window.clone(),
            display,
        })?;

    // Show the overlay first (dim surround + transparent rect — the live window
    // shows through the brief gap), then capture the isolated window so the user
    // never sees a blank pause while the grab/encode runs. capture_window filters
    // with .with_window(), so the visible overlay does not pollute the grab.
    overlay::show_window_mode(&app, window.display_index, &payload).map_err(|e| e.to_string())?;

    let mut frame = match capture::capture_window(&app, window_id) {
        Ok(f) => f,
        Err(e) => {
            use tauri_plugin_notification::NotificationExt;
            let _ = app
                .notification()
                .builder()
                .title("Glitch Spot")
                .body("Selected window closed — annotations discarded")
                .show();
            eprintln!("[glitch-spot] window capture failed: {e}");
            overlay::hide(&app).map_err(|err| err.to_string())?;
            state.picker_state.reset_to_idle();
            return Err(format!("window capture failed: {e}"));
        }
    };
    frame.display_index = window.display_index;

    // Cache for save reuse (get_latest_frame / save_snapshot read this) and push
    // to the already-visible overlay. push_frame runs after show_window_mode's
    // eval('window'), so the frontend resetFrames() cannot wipe the pushed frame.
    if let Ok(mut guard) = state.latest_frame.lock() {
        *guard = Some(frame.clone());
    }
    overlay::push_frame(&app, &frame).map_err(|e| e.to_string())?;
    Ok(())
}
```

- [ ] **Step 2: Verify it builds and lints clean**

Run: `PATH="$HOME/.cargo/bin:$PATH" cargo build --manifest-path src-tauri/Cargo.toml`
Expected: builds with no errors.

Run: `PATH="$HOME/.cargo/bin:$PATH" cargo clippy --manifest-path src-tauri/Cargo.toml -- -D warnings`
Expected: no warnings. (If `reset_to_idle` is named differently, match the existing call already used in the old `save_snapshot` window branch — it used `state.picker_state.reset_to_idle();`.)

- [ ] **Step 3: Verify the existing Rust suite still passes**

Run: `PATH="$HOME/.cargo/bin:$PATH" cargo test --manifest-path src-tauri/Cargo.toml`
Expected: all tests pass (188 baseline), no regressions.

- [ ] **Step 4: Commit**

```bash
git add src-tauri/src/lib.rs
git commit -m "feat(window-mode): capture + cache + push frozen window frame on pick"
```

---

### Task 2: Render the frozen window frame in the overlay (frontend)

**Files:**
- Modify: `src/overlay/Overlay.tsx` (`showBlob` ~240-251; window-mode render ~311-376)
- Test: `src/overlay/Overlay.test.tsx`

jsdom does not fire `img.onload` on `img.src=` (verified), so the actual image render + chromatic resolve is smoke-verified (Task 6), not unit-tested. The TDD step asserts the statically-renderable seam: the frozen-frame container box exists at the window rect with `overflow: hidden`, and the old checker `<div>` is gone.

- [ ] **Step 1: Write the failing test**

Add to `src/overlay/Overlay.test.tsx` inside the `describe("<Overlay/> window mode", ...)` block:

```tsx
  it("renders a clipped frozen-frame box at the window rect and no checker", () => {
    const { getByTestId, container } = render(<Overlay />);
    const payload = makeWindowPayload();
    presentWindowMode(payload);

    const frame = getByTestId("window-frozen-frame");
    const { window_rect_css } = payload;
    expect(frame.style.left).toBe(`${window_rect_css.x}px`);
    expect(frame.style.top).toBe(`${window_rect_css.y}px`);
    expect(frame.style.width).toBe(`${window_rect_css.w}px`);
    expect(frame.style.height).toBe(`${window_rect_css.h}px`);
    // The chromatic shake translates the channel layers ~18px; the box must clip
    // them so no edge gap shows mid-animation.
    expect(frame.style.overflow).toBe("hidden");
    // The gray-white checker placeholder is replaced by the frozen capture.
    expect(container.innerHTML).not.toContain("%23999999");
  });
```

- [ ] **Step 2: Run test to verify it fails**

Run: `PATH="$HOME/.cargo/bin:$PATH" npx vitest run src/overlay/Overlay.test.tsx`
Expected: FAIL — `Unable to find an element by: [data-testid="window-frozen-frame"]` (and the checker assertion would also fail, since the checker is still rendered).

- [ ] **Step 3: Let window-mode frames through `showBlob`**

In `Overlay.tsx`, `showBlob` currently early-returns for window mode:

```tsx
    const showBlob = (blobUrl: string) => {
      if (modeRef.current === 'window') {
        URL.revokeObjectURL(blobUrl)
        return
      }
      anyFrameShown = true;
      setMode("frozen");
      const img = new window.Image();
      img.onload = () => pushFrame(blobUrl);
      img.onerror = () => URL.revokeObjectURL(blobUrl);
      img.src = blobUrl;
    };
```

Replace it with (frames now flow to `imageUrl` in window mode too; only flip to frozen when not in window mode):

```tsx
    const showBlob = (blobUrl: string) => {
      anyFrameShown = true;
      // A pushed frame in window mode is the frozen window capture — keep mode
      // 'window' and let it render into the window rect. Otherwise it's a monitor
      // freeze and we switch to frozen full-screen.
      if (modeRef.current !== 'window') setMode("frozen");
      const img = new window.Image();
      img.onload = () => pushFrame(blobUrl);
      img.onerror = () => URL.revokeObjectURL(blobUrl);
      img.src = blobUrl;
    };
```

- [ ] **Step 4: Replace the checker div with the frozen-frame box**

In the window-mode render branch, replace the checker `<div>` block:

```tsx
        {/* gray-white checker over the window rect */}
        <div
          style={{
            position: 'absolute',
            left: window_rect_css.x,
            top: window_rect_css.y,
            width: window_rect_css.w,
            height: window_rect_css.h,
            backgroundImage: CHECKER_BG,
            backgroundRepeat: 'repeat',
            opacity: 0.12,
            pointerEvents: 'none',
          }}
        />
```

with:

```tsx
        {/* frozen isolated capture of the selected window, presented above
            everything. Empty until the frame arrives (live window shows through
            the transparent rect); then the chromatic shake resolves it in.
            overflow:hidden clips the ~18px shake to the window bounds. */}
        <div
          data-testid="window-frozen-frame"
          style={{
            position: 'absolute',
            left: window_rect_css.x,
            top: window_rect_css.y,
            width: window_rect_css.w,
            height: window_rect_css.h,
            overflow: 'hidden',
            pointerEvents: 'none',
          }}
        >
          {imageUrl && <GlitchImage key={imageUrl} url={imageUrl} />}
        </div>
```

- [ ] **Step 5: Run test to verify it passes**

Run: `PATH="$HOME/.cargo/bin:$PATH" npx vitest run src/overlay/Overlay.test.tsx`
Expected: PASS — both the new test and the existing spotlight test.

- [ ] **Step 6: Verify the full vitest suite + typecheck**

Run: `PATH="$HOME/.cargo/bin:$PATH" npx vitest run`
Expected: all tests pass (115 baseline + 1 new).

Run: `PATH="$HOME/.cargo/bin:$PATH" npx tsc --noEmit`
Expected: no type errors. (`CHECKER_BG` is still referenced by the frozen full-screen render, so it stays imported/defined — do not remove it.)

- [ ] **Step 7: Commit**

```bash
git add src/overlay/Overlay.tsx src/overlay/Overlay.test.tsx
git commit -m "feat(window-mode): render frozen window capture in place of checker"
```

---

### Task 3: Extract a tested frame-resolution helper for window save

**Files:**
- Modify: `src-tauri/src/lib.rs` (add `resolve_window_save_frame` fn near `save_snapshot`; add tests to the existing `#[cfg(test)] mod tests`)

This isolates the window-save frame policy (reuse cached frame, assign `display_index`, compute crop origin) into a pure function that is unit-testable with real data — no `AppHandle`, no mocks.

- [ ] **Step 1: Write the failing tests**

Add to the existing `#[cfg(test)] mod tests` block in `src-tauri/src/lib.rs`:

```rust
    use crate::window_picker::{DisplayBounds, PickableWindow, WindowBounds};

    fn annotating_session() -> picker_session::AnnotatingSession {
        picker_session::AnnotatingSession {
            window_id: 7,
            window: PickableWindow {
                window_id: 7,
                app_name: "Safari".into(),
                title: "Reader".into(),
                bounds: WindowBounds { x: 240, y: 160, w: 800, h: 600 },
                display_index: 1,
                layer: 0,
                visible_regions: vec![],
            },
            display: DisplayBounds {
                display_index: 1, x: 200, y: 100, w: 2560, h: 1414, scale_factor: 2.0,
            },
        }
    }

    fn sample_frame() -> capture::CapturedFrame {
        capture::CapturedFrame {
            width: 800, height: 600, scale_factor: 2.0, png: vec![1, 2, 3], display_index: 0,
        }
    }

    #[test]
    fn window_save_reuses_cached_frame_and_computes_crop() {
        let session = annotating_session();
        let cached = sample_frame();
        let (frame, crop) =
            resolve_window_save_frame(&session, Some(cached.clone())).unwrap();
        // Same pixels — no recapture.
        assert_eq!(frame.png, cached.png);
        // display_index tracks the window's display.
        assert_eq!(frame.display_index, 1);
        // Crop origin is the window's offset within its display, in physical px.
        assert_eq!(crop, (40.0, 60.0));
    }

    #[test]
    fn window_save_errors_when_no_cached_frame() {
        let session = annotating_session();
        let err = resolve_window_save_frame(&session, None).unwrap_err();
        assert!(err.contains("no cached frame"), "got: {err}");
    }
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `PATH="$HOME/.cargo/bin:$PATH" cargo test --manifest-path src-tauri/Cargo.toml resolve_window_save_frame`
Expected: FAIL to compile — `cannot find function resolve_window_save_frame`.

- [ ] **Step 3: Implement the helper**

Add near `save_snapshot` in `src-tauri/src/lib.rs` (above the `#[tauri::command]` fn):

```rust
/// Resolve the frame to save for window mode. Window mode is frozen: the frame
/// captured at pick time (cached in `latest_frame`) is what the user annotated
/// on, so reuse it rather than recapturing — correct (no window drift) and fast.
/// Returns the frame (with `display_index` set to the window's display) and the
/// crop origin (window offset within its display, in physical pixels).
fn resolve_window_save_frame(
    session: &picker_session::AnnotatingSession,
    latest_frame: Option<capture::CapturedFrame>,
) -> Result<(capture::CapturedFrame, (f32, f32)), String> {
    let mut frame = latest_frame.ok_or_else(|| "no cached frame for window mode".to_string())?;
    frame.display_index = session.window.display_index;
    let crop_x = (session.window.bounds.x - session.display.x) as f32;
    let crop_y = (session.window.bounds.y - session.display.y) as f32;
    Ok((frame, (crop_x, crop_y)))
}
```

- [ ] **Step 4: Run tests to verify they pass**

Run: `PATH="$HOME/.cargo/bin:$PATH" cargo test --manifest-path src-tauri/Cargo.toml resolve_window_save_frame`
Expected: PASS (2 tests).

- [ ] **Step 5: Commit**

```bash
git add src-tauri/src/lib.rs
git commit -m "feat(window-mode): add tested frame-resolution helper for window save"
```

---

### Task 4: Switch `save_snapshot("window")` to reuse the cached frame

**Files:**
- Modify: `src-tauri/src/lib.rs` (`save_snapshot` `"window"` branch, lines ~109-134)

Wiring change verified by the suite staying green (the resolution logic is tested in Task 3) + manual smoke (Task 6). No new red-green step.

- [ ] **Step 1: Replace the recapture block with the helper**

The current `"window"` branch reads:

```rust
        "window" => {
            let session = match state.picker_state.current() {
                picker_session::RunMode::Annotating { session } => session,
                other => return Err(format!("save_snapshot('window') while in {:?}", other)),
            };
            overlay::hide(&app).map_err(|e| e.to_string())?;
            std::thread::sleep(std::time::Duration::from_millis(120));
            let frame_result = capture::capture_window(&app, session.window_id);
            let mut frame = match frame_result {
                Ok(f) => f,
                Err(e) => {
                    use tauri_plugin_notification::NotificationExt;
                    let _ = app.notification()
                        .builder()
                        .title("Glitch Spot")
                        .body("Selected window closed — annotations discarded")
                        .show();
                    eprintln!("[glitch-spot] window capture failed: {e}");
                    state.picker_state.reset_to_idle();
                    return Err(format!("window capture failed: {e}"));
                }
            };
            frame.display_index = session.window.display_index;
            let crop_x = (session.window.bounds.x - session.display.x) as f32;
            let crop_y = (session.window.bounds.y - session.display.y) as f32;
            (frame, Some((crop_x, crop_y)))
        }
```

Replace it with (the window is already frozen at pick time, so reuse the cached frame — no hide, no sleep, no recapture, no window-closed path):

```rust
        "window" => {
            let session = match state.picker_state.current() {
                picker_session::RunMode::Annotating { session } => session,
                other => return Err(format!("save_snapshot('window') while in {:?}", other)),
            };
            let cached = state.latest_frame.lock().ok().and_then(|g| g.clone());
            let (frame, crop) = resolve_window_save_frame(&session, cached)?;
            overlay::hide(&app).map_err(|e| e.to_string())?;
            (frame, Some(crop))
        }
```

(`overlay::hide` stays — it dismisses the overlay after a successful save, matching the other branches. The 120ms sleep is gone because we no longer capture after hiding.)

- [ ] **Step 2: Verify it builds, lints, and the suite passes**

Run: `PATH="$HOME/.cargo/bin:$PATH" cargo build --manifest-path src-tauri/Cargo.toml`
Expected: builds clean. If `capture::capture_window` is now unused, the compiler will warn — it is still used by `picker_select` (Task 1), so no unused-import cleanup is expected. Confirm no new warnings.

Run: `PATH="$HOME/.cargo/bin:$PATH" cargo clippy --manifest-path src-tauri/Cargo.toml -- -D warnings`
Expected: no warnings.

Run: `PATH="$HOME/.cargo/bin:$PATH" cargo test --manifest-path src-tauri/Cargo.toml`
Expected: all tests pass.

- [ ] **Step 3: Commit**

```bash
git add src-tauri/src/lib.rs
git commit -m "feat(window-mode): reuse frozen frame at save instead of recapturing"
```

---

### Task 5: Update the window-mode smoke checklist for the behavior change

**Files:**
- Modify: `docs/initiatives/glitch-spot/2026-05-27-window-mode-smoke.md`

- [ ] **Step 1: Update the single-display and error-path expectations**

In the "Single display" section, change:

```markdown
- [ ] Click on a window enters annotation mode with dim veil + checker over the window
```

to:

```markdown
- [ ] Click on a window enters annotation mode with dim veil; the live window shows briefly, then the frozen isolated capture resolves in with the chromatic shake
- [ ] The frozen capture shows only the selected window (no overlapping windows), presented above everything
```

In the "Error paths" section, change:

```markdown
- [ ] Close selected window mid-annotation → notification fires, no snapshot written
- [ ] Minimise selected window → same path
```

to:

```markdown
- [ ] Close selected window after it is frozen → save still produces the frozen capture (no recapture, no discard)
- [ ] Close/minimise the window the instant you click it (capture fails at pick) → notification fires, overlay dismisses, no annotation mode
```

- [ ] **Step 2: Commit**

```bash
git add docs/initiatives/glitch-spot/2026-05-27-window-mode-smoke.md
git commit -m "docs(smoke): window-mode freeze behavior changes"
```

---

### Task 6: Full verification + manual smoke

**Files:** none (verification only)

- [ ] **Step 1: Full automated suites green**

Run: `PATH="$HOME/.cargo/bin:$PATH" cargo test --manifest-path src-tauri/Cargo.toml`
Expected: all Rust tests pass (190: 188 baseline + 2 new helper tests).

Run: `PATH="$HOME/.cargo/bin:$PATH" npx vitest run`
Expected: all vitest pass (116: 115 baseline + 1 new).

Run: `PATH="$HOME/.cargo/bin:$PATH" npx tsc --noEmit`
Expected: no type errors.

Run: `PATH="$HOME/.cargo/bin:$PATH" cargo clippy --manifest-path src-tauri/Cargo.toml -- -D warnings`
Expected: no warnings.

- [ ] **Step 2: Manual window-mode smoke (real hardware)**

Launch: `PATH="$HOME/.cargo/bin:$PATH" npm run tauri -- dev`

Verify against `docs/initiatives/glitch-spot/2026-05-27-window-mode-smoke.md`:
- Press `Ctrl+Opt+Cmd+J`, hover to highlight a window, click it.
- Expected: dim surround appears immediately; the live window shows for a beat; the frozen isolated capture of just that window resolves in with the chromatic shake. No grey blank gap.
- Open a window that is partially covered by another window, pick the covered one → the frozen capture shows the full selected window with no overlap.
- Draw a rectangle (clipped to the window), `Cmd+Shift+Return` to save → annotated PNG is window-sized, rectangle positioned correctly, clipboard JSON has `mode: "window"` + `context.window`.
- Multi-display: pick a window on the secondary display → frozen capture + dim position correctly.
- Save responsiveness: clicking Save no longer pauses ~120ms+recapture before dismissing.

- [ ] **Step 3: Report results**

State exact pass/fail counts from Step 1 and the smoke observations from Step 2. Do not claim success without the actual output.

---

## Notes for the implementer

- **Do not remove `CHECKER_BG`** from `Overlay.tsx` — the frozen full-screen render (monitor freeze) still uses it as a backdrop.
- **Do not touch the monitor-freeze (`'frozen'`) or live (`'live'`) paths** — only the window-mode render branch and the shared `showBlob`.
- **Electron bridge is out of scope** — window mode is Tauri/macOS only; `ElectronBridge` is unaffected.
- The chromatic shake (`GlitchImage`) translates channel layers ~18px; on a very small selected window this may look degenerate but is clipped by `overflow: hidden`. Acceptable for v1; note any oddity in the smoke report.
