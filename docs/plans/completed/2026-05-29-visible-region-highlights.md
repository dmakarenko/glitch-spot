# Visible-region window picker highlights — implementation plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Clip the cyan-checker highlight to the hovered window's visible regions (matching macOS Screenshot.app), and drop fully-occluded windows from the pickable set.

**Architecture:** After `window_picker::macos::enumerate()` returns the front-to-back window list, compute each window's visible region by subtracting the bounds of all windows in front. Each window carries a `visible_regions: Vec<WindowBounds>` field through to the frontend, where the React picker renders one highlight rectangle per region instead of one for the full bounds. Hit-test is unchanged — the existing front-to-back topmost-wins behaviour already matches macOS Screenshot.app's selection rules.

**Tech Stack:** Tauri v2 (Rust + WebView), React + TypeScript (Vite). Tests: `cargo test --manifest-path src-tauri/Cargo.toml --lib` (Rust), `npx vitest run <path>` / `npm test` (frontend). No new dependencies.

**Design seed:** `docs/seeds/2026-05-29-visible-region-highlights-auto-seed.md` — read this before starting any task. It is the source of truth for behaviour.

---

## File structure

### New files (Rust)
- `src-tauri/src/window_picker/visible_regions.rs` — pure rect-subtraction algebra and `compute_visible_regions(front_to_back: &[WindowBounds]) -> Vec<Vec<WindowBounds>>`.

### Modified files (Rust)
- `src-tauri/src/window_picker/mod.rs` — declare the new submodule; add `visible_regions: Vec<WindowBounds>` to `PickableWindow`; update the JSON round-trip test.
- `src-tauri/src/window_picker/macos.rs` — call `compute_visible_regions` after filtering; populate the new field; drop windows with empty regions.
- `src-tauri/src/window_picker/filter.rs` — set `visible_regions: vec![]` on construction (populated by the macos layer afterward).
- `src-tauri/src/window_picker/hit_test.rs` — update the `pw()` test helper and fixture deserializer to set `visible_regions: vec![]`.

### Modified files (frontend)
- `src/picker/types.ts` — add `visible_regions: WindowBounds[]` to `PickableWindow`.
- `src/picker/Picker.tsx` — replace single-rect `HighlightRect` with a per-region map; rename to `HighlightRegion` for clarity; keep `data-testid="picker-highlight"` on each rendered region.
- `src/picker/Picker.test.tsx` — extend the fixture with `visible_regions`; switch `queryByTestId("picker-highlight")` to `queryAllByTestId("picker-highlight")`; add a multi-region rendering test.

### New files (smoke doc)
- `docs/initiatives/glitch-spot/2026-05-29-visible-region-highlights-smoke.md` — manual checklist.

---

## Conventions

- **Branch:** `feature-overlay-window-selection` (already created in this worktree). Before any commit, run `git rev-parse --abbrev-ref HEAD` and confirm it prints `feature-overlay-window-selection`. Never let work land on `main`.
- **Test commands:**
  - Rust: `cargo test --manifest-path src-tauri/Cargo.toml --lib -- <filter>` for one file, `cargo test --manifest-path src-tauri/Cargo.toml --lib` for the suite.
  - Frontend: `npx vitest run <path>` for a single test file, `npm test` for the full frontend suite.
- **Commit cadence:** one commit per task. Use the commit message shown at the end of each task. Always use a HEREDOC for multi-line commit messages.
- **No backwards-compat shims.** `visible_regions` is a new required field on `PickableWindow`; update every construction site rather than making it optional.

---

## Task 1: Pure rectangle subtraction algebra

**Files:**
- Create: `src-tauri/src/window_picker/visible_regions.rs`
- Modify: `src-tauri/src/window_picker/mod.rs` (declare the submodule)

- [ ] **Step 1: Declare the submodule**

Edit `src-tauri/src/window_picker/mod.rs` and add to the `pub mod` block (alongside `filter`, `hit_test`, `coords`):

```rust
pub mod visible_regions;
```

- [ ] **Step 2: Write the failing tests**

Create `src-tauri/src/window_picker/visible_regions.rs` with the test module first:

```rust
// ABOUTME: Pure rectangle algebra for computing the visible portion of each
// ABOUTME: window after subtracting every window in front of it.

use super::WindowBounds;

pub fn subtract_rect(base: WindowBounds, sub: WindowBounds) -> Vec<WindowBounds> {
    todo!()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn b(x: i32, y: i32, w: u32, h: u32) -> WindowBounds {
        WindowBounds { x, y, w, h }
    }

    #[test]
    fn no_overlap_returns_base_unchanged() {
        let base = b(0, 0, 100, 100);
        let sub = b(200, 200, 50, 50);
        assert_eq!(subtract_rect(base, sub), vec![base]);
    }

    #[test]
    fn sub_fully_covers_base_returns_empty() {
        let base = b(10, 10, 50, 50);
        let sub = b(0, 0, 200, 200);
        assert_eq!(subtract_rect(base, sub), vec![]);
    }

    #[test]
    fn sub_in_the_middle_yields_four_strips() {
        // base = 0,0,100,100 ; sub = 25,25,50,50 → top/bottom/left/right strips
        let base = b(0, 0, 100, 100);
        let sub = b(25, 25, 50, 50);
        let got = subtract_rect(base, sub);
        // Top strip: y=0..25 spans full width
        assert!(got.contains(&b(0, 0, 100, 25)), "missing top strip in {got:?}");
        // Bottom strip: y=75..100 spans full width
        assert!(got.contains(&b(0, 75, 100, 25)), "missing bottom strip in {got:?}");
        // Left strip: within the intersection's y-band (25..75)
        assert!(got.contains(&b(0, 25, 25, 50)), "missing left strip in {got:?}");
        // Right strip: within the intersection's y-band (25..75)
        assert!(got.contains(&b(75, 25, 25, 50)), "missing right strip in {got:?}");
        assert_eq!(got.len(), 4);
    }

    #[test]
    fn sub_clipping_top_left_corner_yields_two_strips() {
        // base = 50,50,100,100 ; sub = 0,0,80,80 → only bottom and right strips
        let base = b(50, 50, 100, 100);
        let sub = b(0, 0, 80, 80);
        let got = subtract_rect(base, sub);
        // intersection = 50,50..80,80 → bottom strip y=80..150 across full width,
        // right strip x=80..150 within y=50..80
        assert!(got.contains(&b(50, 80, 100, 70)), "missing bottom strip in {got:?}");
        assert!(got.contains(&b(80, 50, 70, 30)), "missing right strip in {got:?}");
        assert_eq!(got.len(), 2);
    }

    #[test]
    fn sub_clipping_only_right_edge_yields_one_strip() {
        // base = 0,0,100,100 ; sub = 80,0,100,100 → left strip only
        let base = b(0, 0, 100, 100);
        let sub = b(80, 0, 100, 100);
        let got = subtract_rect(base, sub);
        assert_eq!(got, vec![b(0, 0, 80, 100)]);
    }

    #[test]
    fn sub_clipping_horizontal_band_yields_top_and_bottom() {
        // base = 0,0,100,100 ; sub = -50,30,300,40 → covers full width 30..70
        let base = b(0, 0, 100, 100);
        let sub = b(-50, 30, 300, 40);
        let got = subtract_rect(base, sub);
        assert!(got.contains(&b(0, 0, 100, 30)), "missing top strip in {got:?}");
        assert!(got.contains(&b(0, 70, 100, 30)), "missing bottom strip in {got:?}");
        assert_eq!(got.len(), 2);
    }

    #[test]
    fn zero_width_sub_does_not_change_base() {
        let base = b(0, 0, 100, 100);
        let sub = b(20, 20, 0, 50);
        assert_eq!(subtract_rect(base, sub), vec![base]);
    }
}
```

- [ ] **Step 3: Run tests to verify they fail**

Run: `cargo test --manifest-path src-tauri/Cargo.toml --lib visible_regions`

Expected: every test panics with `not yet implemented` (the `todo!()` body).

- [ ] **Step 4: Implement `subtract_rect`**

Replace the `todo!()` body in `src-tauri/src/window_picker/visible_regions.rs`:

```rust
/// Return the portion of `base` not covered by `sub`. The result is a list of
/// non-overlapping `WindowBounds` whose union equals `base ∖ sub`. When the two
/// rectangles do not intersect, the returned list is `vec![base]`. When `sub`
/// fully covers `base`, the returned list is empty.
///
/// Strips are produced in this order: top, bottom, left, right — but callers
/// must not rely on the order, only on the union semantics.
pub fn subtract_rect(base: WindowBounds, sub: WindowBounds) -> Vec<WindowBounds> {
    let bx1 = base.x;
    let by1 = base.y;
    let bx2 = base.x + base.w as i32;
    let by2 = base.y + base.h as i32;
    let sx1 = sub.x;
    let sy1 = sub.y;
    let sx2 = sub.x + sub.w as i32;
    let sy2 = sub.y + sub.h as i32;

    // Intersection.
    let ix1 = bx1.max(sx1);
    let iy1 = by1.max(sy1);
    let ix2 = bx2.min(sx2);
    let iy2 = by2.min(sy2);

    if ix1 >= ix2 || iy1 >= iy2 {
        // No overlap — base survives intact.
        return vec![base];
    }

    let mut out = Vec::with_capacity(4);

    // Top strip: full base width, from base.y up to intersection.y.
    if iy1 > by1 {
        out.push(WindowBounds {
            x: bx1,
            y: by1,
            w: base.w,
            h: (iy1 - by1) as u32,
        });
    }
    // Bottom strip: full base width, from intersection.y+h down to base.y+h.
    if iy2 < by2 {
        out.push(WindowBounds {
            x: bx1,
            y: iy2,
            w: base.w,
            h: (by2 - iy2) as u32,
        });
    }
    // Left strip: within the intersection's y-band only.
    if ix1 > bx1 {
        out.push(WindowBounds {
            x: bx1,
            y: iy1,
            w: (ix1 - bx1) as u32,
            h: (iy2 - iy1) as u32,
        });
    }
    // Right strip: within the intersection's y-band only.
    if ix2 < bx2 {
        out.push(WindowBounds {
            x: ix2,
            y: iy1,
            w: (bx2 - ix2) as u32,
            h: (iy2 - iy1) as u32,
        });
    }
    out
}
```

- [ ] **Step 5: Run tests to verify they pass**

Run: `cargo test --manifest-path src-tauri/Cargo.toml --lib visible_regions`

Expected: 6 tests pass.

- [ ] **Step 6: Verify the branch and commit**

Run: `git rev-parse --abbrev-ref HEAD`
Expected: `feature-overlay-window-selection`

Then:

```bash
git add src-tauri/src/window_picker/mod.rs src-tauri/src/window_picker/visible_regions.rs
git commit -m "feat(picker): add pure subtract_rect for visible-region algebra"
```

---

## Task 2: Compute per-window visible regions from a front-to-back list

**Files:**
- Modify: `src-tauri/src/window_picker/visible_regions.rs`

- [ ] **Step 1: Write the failing tests**

Append to the `mod tests` block in `src-tauri/src/window_picker/visible_regions.rs`:

```rust
    #[test]
    fn single_window_has_full_bounds_as_only_region() {
        let only = b(0, 0, 100, 100);
        let regions = compute_visible_regions(&[only]);
        assert_eq!(regions, vec![vec![only]]);
    }

    #[test]
    fn front_window_is_unaffected_by_windows_behind() {
        let front = b(0, 0, 100, 100);
        let back = b(50, 50, 100, 100);
        let regions = compute_visible_regions(&[front, back]);
        // Index 0 (front) is the topmost: nothing in front of it.
        assert_eq!(regions[0], vec![front]);
    }

    #[test]
    fn back_window_loses_overlapping_corner() {
        // Front clips back's top-left corner; back's visible region is L-shaped
        // — two strips.
        let front = b(0, 0, 80, 80);
        let back = b(50, 50, 100, 100);
        let regions = compute_visible_regions(&[front, back]);
        assert_eq!(regions[0], vec![front]);
        // Back: visible = full bounds minus front's intersection.
        // intersection of front+back = 50,50..80,80 → bottom strip + right strip
        assert!(regions[1].contains(&b(50, 80, 100, 70)));
        assert!(regions[1].contains(&b(80, 50, 70, 30)));
        assert_eq!(regions[1].len(), 2);
    }

    #[test]
    fn fully_covered_back_window_has_empty_regions() {
        let front = b(0, 0, 200, 200);
        let back = b(50, 50, 100, 100);
        let regions = compute_visible_regions(&[front, back]);
        assert_eq!(regions[0], vec![front]);
        assert_eq!(regions[1], Vec::<WindowBounds>::new());
    }

    #[test]
    fn three_windows_in_a_stack_progressively_lose_regions() {
        let top = b(0, 0, 100, 100);
        let middle = b(50, 0, 100, 100);
        let bottom = b(25, 50, 200, 100);
        let regions = compute_visible_regions(&[top, middle, bottom]);
        // Top is fully visible.
        assert_eq!(regions[0], vec![top]);
        // Middle: lose its left edge to top. Intersection = 50,0..100,100 →
        // right strip x=100..150 across full height.
        assert_eq!(regions[1], vec![b(100, 0, 50, 100)]);
        // Bottom: lose intersections with top (25..100, 50..100) and middle
        // (50..150, 50..100). Union of the two covers x=25..150 of y=50..100,
        // leaving the right strip and the part below y=100.
        assert!(regions[2].iter().any(|r| r.y >= 100), "missing pieces below middle in {:?}", regions[2]);
    }

    #[test]
    fn empty_input_yields_empty_output() {
        let regions = compute_visible_regions(&[]);
        assert!(regions.is_empty());
    }
```

Also add the function stub above the `#[cfg(test)] mod tests` block:

```rust
/// For each window in the **front-to-back** ordered list, compute the
/// rectangles representing the portion of that window not covered by any
/// window in front of it. Output has the same length and ordering as the
/// input. A window with an empty regions list is fully occluded.
pub fn compute_visible_regions(front_to_back: &[WindowBounds]) -> Vec<Vec<WindowBounds>> {
    todo!()
}
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test --manifest-path src-tauri/Cargo.toml --lib visible_regions::tests`

Expected: the new tests panic with `not yet implemented`; the 6 original `subtract_rect` tests still pass.

- [ ] **Step 3: Implement `compute_visible_regions`**

Replace the `todo!()` body:

```rust
pub fn compute_visible_regions(front_to_back: &[WindowBounds]) -> Vec<Vec<WindowBounds>> {
    let mut out = Vec::with_capacity(front_to_back.len());
    for (k, w) in front_to_back.iter().enumerate() {
        let mut regions = vec![*w];
        for above in &front_to_back[..k] {
            let mut next = Vec::with_capacity(regions.len());
            for r in &regions {
                next.extend(subtract_rect(*r, *above));
            }
            regions = next;
            if regions.is_empty() {
                break;
            }
        }
        out.push(regions);
    }
    out
}
```

- [ ] **Step 4: Run tests to verify they pass**

Run: `cargo test --manifest-path src-tauri/Cargo.toml --lib visible_regions`

Expected: 12 tests pass.

- [ ] **Step 5: Commit**

Run: `git rev-parse --abbrev-ref HEAD`
Expected: `feature-overlay-window-selection`

```bash
git add src-tauri/src/window_picker/visible_regions.rs
git commit -m "feat(picker): compute per-window visible regions from z-ordered list"
```

---

## Task 3: Add `visible_regions` to `PickableWindow` and populate during enumeration

**Files:**
- Modify: `src-tauri/src/window_picker/mod.rs`
- Modify: `src-tauri/src/window_picker/filter.rs`
- Modify: `src-tauri/src/window_picker/hit_test.rs`
- Modify: `src-tauri/src/window_picker/macos.rs`

- [ ] **Step 1: Update the JSON round-trip test in `mod.rs` to require the new field**

In `src-tauri/src/window_picker/mod.rs`, replace the existing
`pickable_window_round_trips_through_json` test with:

```rust
    #[test]
    fn pickable_window_round_trips_through_json() {
        let w = PickableWindow {
            window_id: 42,
            app_name: "Safari".into(),
            title: "Reader".into(),
            bounds: WindowBounds { x: 10, y: 20, w: 800, h: 600 },
            display_index: 1,
            layer: 0,
            visible_regions: vec![
                WindowBounds { x: 10, y: 20, w: 200, h: 600 },
                WindowBounds { x: 400, y: 20, w: 410, h: 600 },
            ],
        };
        let json = serde_json::to_string(&w).unwrap();
        let back: PickableWindow = serde_json::from_str(&json).unwrap();
        assert_eq!(w, back);
    }
```

- [ ] **Step 2: Run the test to verify it fails**

Run: `cargo test --manifest-path src-tauri/Cargo.toml --lib pickable_window_round_trips_through_json`

Expected: compile error — `PickableWindow` has no field `visible_regions`.

- [ ] **Step 3: Add the field to `PickableWindow`**

In `src-tauri/src/window_picker/mod.rs`, extend the struct:

```rust
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct PickableWindow {
    pub window_id: u32,
    pub app_name: String,
    pub title: String,
    pub bounds: WindowBounds,
    pub display_index: usize,
    pub layer: i32,
    pub visible_regions: Vec<WindowBounds>,
}
```

- [ ] **Step 4: Update `filter.rs` to initialise the new field**

In `src-tauri/src/window_picker/filter.rs:44`, change the `.map(...)` closure in `filter_standard_windows` to set the field. The macos layer overwrites it after computing regions, so initialise to empty here:

```rust
        .map(|r| PickableWindow {
            window_id: r.window_id,
            app_name: r.app_name.clone(),
            title: r.title.clone(),
            bounds: r.bounds,
            display_index: r.display_index,
            layer: r.layer,
            visible_regions: Vec::new(),
        })
```

- [ ] **Step 5: Update the `pw()` test helper in `hit_test.rs`**

In `src-tauri/src/window_picker/hit_test.rs`, change the `pw` helper to set `visible_regions: vec![]`:

```rust
    fn pw(id: u32, x: i32, y: i32, w: u32, h: u32) -> PickableWindow {
        PickableWindow {
            window_id: id,
            app_name: format!("App{id}"),
            title: format!("Window {id}"),
            bounds: WindowBounds { x, y, w, h },
            display_index: 0,
            layer: 0,
            visible_regions: vec![],
        }
    }
```

And in the same file, in the `parity_with_shared_fixture` test, update the `.map(|w| PickableWindow { ... })` to include `visible_regions: vec![]`:

```rust
            let ws: Vec<PickableWindow> = case.windows.into_iter().map(|w| PickableWindow {
                window_id: w.window_id,
                app_name: String::new(),
                title: String::new(),
                bounds: w.bounds,
                display_index: 0,
                layer: 0,
                visible_regions: vec![],
            }).collect();
```

- [ ] **Step 6: Compute and populate regions in `macos.rs`**

In `src-tauri/src/window_picker/macos.rs`, update the `enumerate` return to compute regions and drop fully-occluded windows. Add an import for the new module at the top of the file:

```rust
use super::{
    coords::points_to_physical_bounds,
    filter::{filter_standard_windows, RawWindow},
    visible_regions::compute_visible_regions,
    DisplayBounds, PickableWindow,
};
```

Replace the final `Ok(filter_standard_windows(&raws), displays))` line with:

```rust
    let mut filtered = filter_standard_windows(&raws);

    // Compute visible regions over the front-to-back list, then drop windows
    // whose visible region is empty (fully occluded by windows in front).
    let bounds: Vec<_> = filtered.iter().map(|w| w.bounds).collect();
    let regions = compute_visible_regions(&bounds);
    for (w, r) in filtered.iter_mut().zip(regions.into_iter()) {
        w.visible_regions = r;
    }
    filtered.retain(|w| !w.visible_regions.is_empty());

    Ok((filtered, displays))
```

- [ ] **Step 7: Run the full Rust suite**

Run: `cargo test --manifest-path src-tauri/Cargo.toml --lib`

Expected: every test passes. Pay special attention to:
- `window_picker::mod::tests::pickable_window_round_trips_through_json`
- All `window_picker::hit_test::tests` (the `pw()` helper change must not regress them).
- All `window_picker::filter::tests`.

If any test in `lib.rs` constructs `PickableWindow` directly, update it to include `visible_regions: vec![]`. Search with: `rg "PickableWindow \{" src-tauri/src`.

- [ ] **Step 8: Commit**

Run: `git rev-parse --abbrev-ref HEAD`
Expected: `feature-overlay-window-selection`

```bash
git add src-tauri/src/window_picker/mod.rs \
        src-tauri/src/window_picker/filter.rs \
        src-tauri/src/window_picker/hit_test.rs \
        src-tauri/src/window_picker/macos.rs
git commit -m "feat(picker): carry per-window visible_regions through enumeration"
```

---

## Task 4: Extend the TypeScript `PickableWindow` type

**Files:**
- Modify: `src/picker/types.ts`

- [ ] **Step 1: Update the type**

In `src/picker/types.ts`, extend `PickableWindow`:

```typescript
export type PickableWindow = {
  window_id: number;
  app_name: string;
  title: string;
  bounds: WindowBounds;
  display_index: number;
  layer: number;
  visible_regions: WindowBounds[];
};
```

- [ ] **Step 2: Confirm the TypeScript build still type-checks**

Run: `npx tsc --noEmit --project tsconfig.json`

Expected: clean output. If callers in `Picker.tsx` or tests construct `PickableWindow` literals, the build will surface them — note them; Task 5 fixes them. If `tsc` complains about those constructions specifically, that is the expected failure for Task 5.

- [ ] **Step 3: Commit**

Run: `git rev-parse --abbrev-ref HEAD`
Expected: `feature-overlay-window-selection`

```bash
git add src/picker/types.ts
git commit -m "feat(picker): add visible_regions to TS PickableWindow type"
```

---

## Task 5: Render one highlight rectangle per visible region

**Files:**
- Modify: `src/picker/Picker.tsx`
- Modify: `src/picker/Picker.test.tsx`

- [ ] **Step 1: Write the failing tests in `Picker.test.tsx`**

In `src/picker/Picker.test.tsx`:

1. Update `makeSession()` to include `visible_regions` on the window. Default it to one region matching the bounds (the unoccluded case):

```typescript
function makeSession(): PickerStartPayload {
  return {
    picker_origin: { x: 0, y: 0 },
    picker_scale: 1,
    displays: [
      { display_index: 0, x: 0, y: 0, w: 1440, h: 900, scale_factor: 1 },
    ],
    windows: [
      {
        window_id: 7,
        app_name: "Safari",
        title: "Reader",
        bounds: { x: 100, y: 100, w: 400, h: 300 },
        display_index: 0,
        layer: 0,
        visible_regions: [{ x: 100, y: 100, w: 400, h: 300 }],
      },
    ],
  };
}
```

2. The existing test `highlights the window when the Rust poller broadcasts a hit` uses `queryByTestId("picker-highlight")`. Switch it to `queryAllByTestId`:

```typescript
  it("highlights the window when the Rust poller broadcasts a hit", async () => {
    const { queryAllByTestId } = render(<Picker />);
    act(() => {
      (window as unknown as { __glitchSpotPickerStart: (p: PickerStartPayload) => void })
        .__glitchSpotPickerStart(makeSession());
    });
    await Promise.resolve();
    emitHover(7);
    expect(queryAllByTestId("picker-highlight").length).toBe(1);
  });
```

3. Update the "clears the highlight" test the same way:

```typescript
  it("clears the highlight when the poller broadcasts a miss", async () => {
    const { queryAllByTestId } = render(<Picker />);
    act(() => {
      (window as unknown as { __glitchSpotPickerStart: (p: PickerStartPayload) => void })
        .__glitchSpotPickerStart(makeSession());
    });
    await Promise.resolve();
    emitHover(7);
    emitHover(null);
    expect(queryAllByTestId("picker-highlight").length).toBe(0);
  });
```

4. Add a new test that exercises the multi-region case:

```typescript
  it("renders one highlight rect per visible region", async () => {
    const { queryAllByTestId } = render(<Picker />);
    const session: PickerStartPayload = {
      picker_origin: { x: 0, y: 0 },
      picker_scale: 1,
      displays: [
        { display_index: 0, x: 0, y: 0, w: 1440, h: 900, scale_factor: 1 },
      ],
      windows: [
        {
          window_id: 9,
          app_name: "Notes",
          title: "Splits",
          bounds: { x: 0, y: 0, w: 200, h: 200 },
          display_index: 0,
          layer: 0,
          // L-shape: the top-left corner is covered, so the visible region is
          // two strips (bottom + right).
          visible_regions: [
            { x: 0, y: 100, w: 200, h: 100 },
            { x: 100, y: 0, w: 100, h: 100 },
          ],
        },
      ],
    };
    act(() => {
      (window as unknown as { __glitchSpotPickerStart: (p: PickerStartPayload) => void })
        .__glitchSpotPickerStart(session);
    });
    await Promise.resolve();
    emitHover(9);
    expect(queryAllByTestId("picker-highlight").length).toBe(2);
  });
```

- [ ] **Step 2: Run the tests to verify they fail**

Run: `npx vitest run src/picker/Picker.test.tsx`

Expected:
- The multi-region test fails (the current component renders only one rect for the full bounds, not two).
- The `queryAllByTestId` switches still pass against the old single-rect renderer (since `length === 1` already holds), but the assertion will be re-verified once the component changes.

If everything passes already, the test is not strong enough — re-read the multi-region test to confirm it asserts on the count.

- [ ] **Step 3: Refactor `Picker.tsx` to render per-region**

In `src/picker/Picker.tsx`:

1. Replace the lookup-then-render block with a per-region map. Inside the component body, after the existing `const highlight = session.windows.find((w) => w.window_id === hoveredId);` line, change the JSX so each visible region renders its own rectangle. Replace the `{highlight && (...)}` block with:

```tsx
      {highlight?.visible_regions.map((region, i) => (
        <HighlightRegion
          key={i}
          bounds={region}
          pickerOriginX={picker_origin.x}
          pickerOriginY={picker_origin.y}
          scale={picker_scale}
        />
      ))}
```

2. Replace the `HighlightRect` component with a `HighlightRegion` component that takes `bounds: WindowBounds` instead of a full `PickableWindow`. Replace the entire `function HighlightRect(...) { ... }` block with:

```tsx
function HighlightRegion({
  bounds,
  pickerOriginX,
  pickerOriginY,
  scale,
}: {
  bounds: WindowBounds;
  pickerOriginX: number;
  pickerOriginY: number;
  scale: number;
}) {
  // Convert physical-px global bounds → picker-local CSS px.
  const left = (bounds.x - pickerOriginX) / scale;
  const top = (bounds.y - pickerOriginY) / scale;
  const width = bounds.w / scale;
  const height = bounds.h / scale;
  return (
    <div
      data-testid="picker-highlight"
      style={{
        position: "absolute",
        left,
        top,
        width,
        height,
        backgroundImage: CYAN_CHECKER,
        backgroundRepeat: "repeat",
        pointerEvents: "none",
      }}
    />
  );
}
```

3. Update the import line at the top of the file. After the refactor `PickableWindow` is no longer referenced (the only use was in the prop type of the old `HighlightRect`), so the final line should be:

```tsx
import type { PickerStartPayload, WindowBounds } from "./types";
```

If `tsc` or ESLint complains about the change, re-read the surrounding code to confirm `PickableWindow` is genuinely unreferenced before deleting — `session.windows.find(...)` returns it by inference and does not need it imported.

- [ ] **Step 4: Run the tests to verify they pass**

Run: `npx vitest run src/picker/Picker.test.tsx`

Expected: all 7 tests pass, including the new multi-region test.

- [ ] **Step 5: Run the full frontend suite to confirm no regressions**

Run: `npm test`

Expected: clean pass. If `src/picker/hitTest.test.ts` (or any other test) fails because of the type change, the fix is to add `visible_regions: []` to the test fixture (or to use the structural `PickableBounds` subset which doesn't need the field).

- [ ] **Step 6: Commit**

Run: `git rev-parse --abbrev-ref HEAD`
Expected: `feature-overlay-window-selection`

```bash
git add src/picker/Picker.tsx src/picker/Picker.test.tsx
git commit -m "feat(picker): render highlight per visible region instead of full bounds"
```

---

## Task 6: Manual smoke verification

**Files:**
- Create: `docs/initiatives/glitch-spot/2026-05-29-visible-region-highlights-smoke.md`

- [ ] **Step 1: Write the smoke checklist**

Create `docs/initiatives/glitch-spot/2026-05-29-visible-region-highlights-smoke.md` with:

```markdown
# Visible-region highlights — manual smoke

> Confirm the macOS Screenshot.app behaviour landed end-to-end. Run after the
> implementation tasks complete. Each row is one observation; tick when the
> behaviour matches the **expected** column.

## Setup

1. Quit the app if it's running.
2. Start the dev build:
   ```
   PATH="$HOME/.cargo/bin:$PATH" npm run tauri -- dev
   ```
3. Open at least three overlapping windows from different apps (e.g. Safari,
   Notes, Finder). Arrange them so each one is partly covered by another.

## Checks

| # | Action | Expected | Pass? |
|---|---|---|---|
| 1 | Press ⌃⌥⌘J. | Pickers appear on every display; the cyan cursor is visible. | |
| 2 | Hover a window that is fully visible (no occlusion). | Cyan checker covers its full rectangle. | |
| 3 | Hover a window whose **left half** is covered by another window. | Cyan checker only covers the visible right half; the covering window remains unobscured. | |
| 4 | Hover a window whose **top-left corner** is covered by another. | Cyan checker forms an L-shape that wraps the corner. | |
| 5 | Move the cursor across the boundary from covered to visible portion of the back window. | Selection switches between the covering window and the back window as expected — the back window is only highlighted when the cursor is over its visible region. | |
| 6 | Bring a small window fully behind a larger one so it is completely hidden. Re-press ⌃⌥⌘J. | The fully-hidden window is not selectable from anywhere on screen — moving the cursor over its bounds selects whatever is on top. | |
| 7 | Click an L-shaped highlight. | The expected window is selected and the overlay enters annotation mode. | |
| 8 | Press Esc during the picker. | The picker dismisses cleanly; pressing ⌃⌥⌘J again opens a fresh picker. | |
| 9 | Hover the same window on both displays in a multi-monitor setup. | Highlight clipping behaves identically on each display. | |

If any row fails, file the failure mode under a "Findings" section below before
fixing.
```

- [ ] **Step 2: Commit the smoke doc**

Run: `git rev-parse --abbrev-ref HEAD`
Expected: `feature-overlay-window-selection`

```bash
git add docs/initiatives/glitch-spot/2026-05-29-visible-region-highlights-smoke.md
git commit -m "docs(picker): add manual smoke checklist for visible-region highlights"
```

- [ ] **Step 3: Walk the checklist**

Follow `docs/initiatives/glitch-spot/2026-05-29-visible-region-highlights-smoke.md`
row-by-row in a real run. Tick each row. If any row fails, stop, file a
"Findings" section with the failure mode, then return to the relevant earlier
task to fix.

---

## After all tasks pass

1. Confirm `cargo test --manifest-path src-tauri/Cargo.toml --lib` is clean.
2. Confirm `npm test` is clean.
3. Confirm the smoke checklist is fully ticked.
4. Hand back to Dmitry for review.
