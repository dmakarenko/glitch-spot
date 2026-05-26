# Design Polish Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Apply the Glitch Spot dark design system (from `DESIGN.md`) to all overlay UI components.

**Architecture:** All UI is React with inline styles — no CSS framework. The one exception is the `.glitch-frame` pseudo-element technique, which requires a CSS class. That class lives in `overlay.html`'s inline `<style>` block (the only CSS file actually loaded by the overlay). All component changes are pure inline-style updates.

**Tech Stack:** React + TypeScript, Vite, Tauri v2. Tests: Vitest (`npm test`).

---

## File Map

| File | Change |
|---|---|
| `overlay.html` | Add `.glitch-frame` + `.glitch-frame::before` CSS class |
| `src/overlay/Toolbar.tsx` | Wrap in `.glitch-frame` div; dark pill styles; `#ee6633` Save button |
| `src/overlay/annotations/DrawingLayer.tsx` | Orange rect border + chromatic `box-shadow`; crisp `#ff3b30` delete badge |
| `src/overlay/annotations/RectangleNote.tsx` | Dark bg `#161616`; orange border; muted text |
| `src/overlay/annotations/DiscardModal.tsx` | Dark `#1c1c1e` surface; `.glitch-frame` wrapper; updated buttons |

`src/styles/globals.css` is **not imported** by the overlay — leave it untouched.

---

### Task 1: Add `.glitch-frame` CSS class to `overlay.html`

The chromatic split on container edges uses a `::before` pseudo-element — this cannot be done with inline styles. The only CSS loaded by the overlay lives in `overlay.html`'s inline `<style>` block.

**Files:**
- Modify: `overlay.html`

- [ ] **Step 1: Verify existing tests pass**

```bash
npm test
```

Expected: all tests in `src/overlay/annotations/state.test.ts` pass.

- [ ] **Step 2: Add `.glitch-frame` CSS**

Open `overlay.html`. The current `<style>` block is:

```html
<style>
  html, body, #root { margin: 0; padding: 0; height: 100%; background: transparent; }
</style>
```

Replace it with:

```html
<style>
  html, body, #root { margin: 0; padding: 0; height: 100%; background: transparent; }
  .glitch-frame { position: relative; }
  .glitch-frame::before {
    content: '';
    position: absolute;
    inset: -1.5px;
    border-radius: inherit;
    box-shadow: -2px 0 0 rgba(255, 0, 60, 0.45), 2px 0 0 rgba(0, 245, 255, 0.45);
    pointer-events: none;
  }
</style>
```

- [ ] **Step 3: Verify tests still pass**

```bash
npm test
```

Expected: same pass count as Step 1.

- [ ] **Step 4: Commit**

```bash
git add overlay.html
git commit -m "style: add glitch-frame CSS class for chromatic split effect"
```

---

### Task 2: Update Toolbar

**Files:**
- Modify: `src/overlay/Toolbar.tsx`

The toolbar pill gets the `.glitch-frame` wrapper (chromatic edge glow) and the Save button changes from Apple blue `#0a84ff` to accent orange `#ee6633`. The wrapper `div` needs `borderRadius: 9` so `border-radius: inherit` on the `::before` picks up the right value (the pill's `borderRadius: 8` plus the 1.5 px inset).

- [ ] **Step 1: Rewrite `Toolbar.tsx`**

Replace the entire file content:

```tsx
// ABOUTME: Top-center floating toolbar for the overlay session.
// ABOUTME: Shows the rect count and a Save button. Hides until at least one rect exists.

type Props = {
  rectCount: number;
  onSave: () => void;
};

export function Toolbar({ rectCount, onSave }: Props) {
  if (rectCount === 0) return null;
  return (
    <div
      className="glitch-frame"
      style={{
        position: "fixed",
        top: 12,
        left: "50%",
        transform: "translateX(-50%)",
        borderRadius: 9,
        zIndex: 10,
      }}
    >
      <div
        onMouseDown={(e) => e.stopPropagation()}
        onClick={(e) => e.stopPropagation()}
        style={{
          display: "flex",
          alignItems: "center",
          gap: 12,
          padding: "8px 12px",
          background: "rgba(18,18,18,0.94)",
          color: "#e0e0e0",
          font: "13px/1 system-ui",
          borderRadius: 8,
          border: "1px solid rgba(255,255,255,0.07)",
          boxShadow: "0 4px 20px rgba(0,0,0,0.6)",
          backdropFilter: "blur(8px)",
          WebkitBackdropFilter: "blur(8px)",
          pointerEvents: "auto",
        }}
      >
        <span>{rectCount} rectangle{rectCount === 1 ? "" : "s"}</span>
        <button
          type="button"
          onClick={onSave}
          style={{
            padding: "6px 12px",
            background: "#ee6633",
            color: "white",
            border: "none",
            borderRadius: 4,
            fontWeight: 600,
            cursor: "pointer",
          }}
        >
          Save (⌘↩)
        </button>
      </div>
    </div>
  );
}
```

- [ ] **Step 2: Run tests**

```bash
npm test
```

Expected: all tests pass (no toolbar unit tests exist; this confirms nothing else broke).

- [ ] **Step 3: Commit**

```bash
git add src/overlay/Toolbar.tsx
git commit -m "style: dark toolbar pill with chromatic glitch frame and orange Save button"
```

---

### Task 3: Update annotation rectangles in `DrawingLayer`

**Files:**
- Modify: `src/overlay/annotations/DrawingLayer.tsx`

Rectangles get the chromatic split directly via `box-shadow` (they are absolutely-positioned, so box-shadow doesn't affect layout — no wrapper needed). The delete `×` badge stays crisp `#ff3b30` with no split.

- [ ] **Step 1: Update `DrawingLayer.tsx`**

Change the two constants at the top of the file:

```tsx
// Before:
const RECT_BORDER = "2px solid #ff3b30";
const RECT_FILL = "rgba(255, 59, 48, 0.08)";

// After:
const RECT_BORDER = "1.5px solid rgba(238, 102, 51, 0.9)";
const RECT_FILL = "rgba(238, 102, 51, 0.05)";
```

Then update the rect `div` style inside the `.map()` — add the `boxShadow` property:

```tsx
style={{
  position: "absolute",
  left: r.x,
  top: r.y,
  width: r.w,
  height: r.h,
  border: RECT_BORDER,
  background: RECT_FILL,
  boxSizing: "border-box",
  pointerEvents: "none",
  boxShadow: "-2px 0 0 rgba(255, 0, 60, 0.4), 2px 0 0 rgba(0, 245, 255, 0.4)",
}}
```

The delete badge style already uses `#ff3b30` — leave it completely unchanged. Do not add any box-shadow or text-shadow to it.

- [ ] **Step 2: Run tests**

```bash
npm test
```

Expected: all tests pass.

- [ ] **Step 3: Commit**

```bash
git add src/overlay/annotations/DrawingLayer.tsx
git commit -m "style: orange rect borders with chromatic split; crisp delete badge"
```

---

### Task 4: Update note textarea in `RectangleNote`

**Files:**
- Modify: `src/overlay/annotations/RectangleNote.tsx`

Note goes dark: `#161616` background, orange-tinted border, muted text. The placeholder colour is set via inline style using the vendor-prefixed colour property — this works in WebKit (the Tauri webview engine).

- [ ] **Step 1: Update `RectangleNote.tsx`**

Replace the `<textarea>` style object:

```tsx
// Before:
style={{
  position: "absolute",
  top,
  left,
  width: NOTE_WIDTH,
  minHeight: NOTE_MIN_HEIGHT,
  padding: "6px 8px",
  font: "13px/1.4 system-ui",
  background: "rgba(255,255,255,0.96)",
  color: "#111",
  border: "1px solid rgba(0,0,0,0.18)",
  borderRadius: 6,
  boxShadow: "0 4px 16px rgba(0,0,0,0.18)",
  outline: "none",
  resize: "none",
  userSelect: "text",
  WebkitUserSelect: "text",
}}

// After:
style={{
  position: "absolute",
  top,
  left,
  width: NOTE_WIDTH,
  minHeight: NOTE_MIN_HEIGHT,
  padding: "6px 8px",
  font: "13px/1.4 system-ui",
  background: "#161616",
  color: "#aaaaaa",
  border: "1px solid rgba(238, 102, 51, 0.28)",
  borderRadius: 6,
  boxShadow: "0 2px 10px rgba(0,0,0,0.5)",
  outline: "none",
  resize: "none",
  userSelect: "text",
  WebkitUserSelect: "text",
}}
```

- [ ] **Step 2: Run tests**

```bash
npm test
```

Expected: all tests pass.

- [ ] **Step 3: Commit**

```bash
git add src/overlay/annotations/RectangleNote.tsx
git commit -m "style: dark note textarea with orange accent border"
```

---

### Task 5: Update `DiscardModal`

**Files:**
- Modify: `src/overlay/annotations/DiscardModal.tsx`

Modal gets the `.glitch-frame` wrapper (same as toolbar), dark `#1c1c1e` surface, updated button palette. The backdrop darkens from `0.35` to `0.6` opacity. The `btn()` helper is updated to cover all three button variants.

- [ ] **Step 1: Rewrite `DiscardModal.tsx`**

Replace the entire file content:

```tsx
// ABOUTME: Confirm modal shown when Esc is pressed with at least one rectangle drawn.
// ABOUTME: Lets the user discard the session or save it.

type Props = {
  onDiscard: () => void;
  onSave: () => void;
  onCancel: () => void;
};

export function DiscardModal({ onDiscard, onSave, onCancel }: Props) {
  return (
    <div
      onMouseDown={(e) => {
        if (e.target === e.currentTarget) onCancel();
      }}
      style={{
        position: "fixed",
        inset: 0,
        background: "rgba(0,0,0,0.6)",
        display: "flex",
        alignItems: "center",
        justifyContent: "center",
        zIndex: 100,
      }}
    >
      <div
        className="glitch-frame"
        style={{ borderRadius: 11 }}
      >
        <div
          style={{
            background: "#1c1c1e",
            color: "#e5e5e7",
            minWidth: 320,
            padding: "20px 22px",
            borderRadius: 10,
            border: "1px solid rgba(255,255,255,0.07)",
            boxShadow: "0 16px 48px rgba(0,0,0,0.7)",
            font: "14px/1.5 system-ui",
          }}
        >
          <h2 style={{ margin: "0 0 8px", font: "600 16px/1.3 system-ui" }}>
            Discard annotations?
          </h2>
          <p style={{ margin: "0 0 16px", color: "#888" }}>
            You have unsaved rectangles. Save them to the clipboard, or discard?
          </p>
          <div style={{ display: "flex", justifyContent: "flex-end", gap: 8 }}>
            <button type="button" onClick={onCancel} style={btnCancel()}>
              Cancel
            </button>
            <button type="button" onClick={onDiscard} style={btnDiscard()}>
              Discard
            </button>
            <button type="button" onClick={onSave} style={btnSave()}>
              Save (⌘↩)
            </button>
          </div>
        </div>
      </div>
    </div>
  );
}

function btnCancel(): React.CSSProperties {
  return {
    padding: "6px 12px",
    background: "rgba(255,255,255,0.07)",
    color: "#ccc",
    border: "none",
    borderRadius: 4,
    cursor: "pointer",
    font: "13px/1 system-ui",
  };
}

function btnDiscard(): React.CSSProperties {
  return {
    padding: "6px 12px",
    background: "transparent",
    color: "#ff453a",
    border: "1px solid rgba(255,69,58,0.35)",
    borderRadius: 4,
    cursor: "pointer",
    font: "13px/1 system-ui",
  };
}

function btnSave(): React.CSSProperties {
  return {
    padding: "6px 12px",
    background: "#ee6633",
    color: "white",
    border: "none",
    borderRadius: 4,
    cursor: "pointer",
    font: "13px/1 system-ui",
  };
}
```

- [ ] **Step 2: Run tests**

```bash
npm test
```

Expected: all tests pass.

- [ ] **Step 3: Commit**

```bash
git add src/overlay/annotations/DiscardModal.tsx
git commit -m "style: dark modal surface with chromatic glitch frame and updated button palette"
```

---

## Completion Checklist

After all tasks are done:

- [ ] All tests pass: `npm test`
- [ ] Build succeeds: `npm run build`
- [ ] Manual visual check: run `npm run dev` and open the overlay — verify toolbar, rects, notes, and modal all match `DESIGN.md`
- [ ] No chromatic split visible on any button or text input
