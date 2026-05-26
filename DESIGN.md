# Glitch Spot — Design System

## Philosophy

Glitch Spot's visual identity comes from its icon: a dark circle with RGB chromatic aberration — the red and cyan channel split that characterises a digital glitch. The UI extends this into the product itself.

**The core rule:** the _annotation frames_ (rectangles drawn on screen) are what "glitch". They are the product's signature artifact. Interactive controls (buttons, delete badges) stay crisp and unambiguous — they need to be read instantly under pressure.

Dark mode is the default and only mode.

---

## Colour Tokens

| Token | Value | Role |
|---|---|---|
| `--bg` | `#0c0c0c` | Overlay and window background |
| `--surface` | `#1c1c1e` | Modal / raised surface |
| `--surface-toolbar` | `rgba(18,18,18,0.94)` | Toolbar pill background |
| `--accent` | `#ee6633` | Primary action (Save button, rect border) |
| `--delete` | `#ff3b30` | Destructive action (× badge) |
| `--danger-text` | `#ff453a` | Discard button text |
| `--danger-border` | `rgba(255,69,58,0.35)` | Discard button border |
| `--text-primary` | `#e0e0e0` | Body text |
| `--text-secondary` | `#888888` | Dimmed / helper text |
| `--border-subtle` | `rgba(255,255,255,0.07)` | Hairline borders on dark surfaces |
| `--glitch-red` | `rgba(255,0,60,0.4)` | Chromatic split — red channel |
| `--glitch-cyan` | `rgba(0,245,255,0.4)` | Chromatic split — cyan channel |

---

## The Chromatic Split Effect

The chromatic aberration is a `box-shadow` with two horizontal offsets — one red-shifted left, one cyan-shifted right. This mirrors the icon's RGB split.

```css
box-shadow:
  -2px 0 0 rgba(255, 0, 60, 0.4),
   2px 0 0 rgba(0, 245, 255, 0.4);
```

**Applied to outer edges of containers** (via a `::before` pseudo-element wrapping the element), not to the element's own `box-shadow` — so internal layout is unaffected.

```css
.glitch-frame::before {
  content: '';
  position: absolute;
  inset: -1.5px;
  border-radius: inherit; /* match the container's radius + 1px */
  box-shadow:
    -2px 0 0 rgba(255, 0, 60, 0.45),
     2px 0 0 rgba(0, 245, 255, 0.45);
  pointer-events: none;
}
```

---

## Components

### Annotation Rectangle

The product's core visual artifact. Glitch treatment is strongest here.

- **Border:** `1.5px solid rgba(238, 102, 51, 0.9)` (accent, slightly transparent)
- **Fill:** `rgba(238, 102, 51, 0.05)` (near-invisible tint)
- **Chromatic split:** `box-shadow: -2px 0 0 rgba(255,0,60,0.4), 2px 0 0 rgba(0,245,255,0.4)` on the rect element directly
- **Delete badge (×):** Crisp `#ff3b30` circle, `18×18px`, `border-radius: 50%`. **No split.** Clean `box-shadow: 0 1px 3px rgba(0,0,0,0.4)` only.

### Toolbar Pill

Top-center floating pill, shown only when ≥1 annotation exists.

- **Background:** `rgba(18,18,18,0.94)` with `backdrop-filter: blur(8px)`
- **Border:** `1px solid rgba(255,255,255,0.07)`
- **Chromatic split:** Applied via `::before` pseudo-element on the wrapping div
- **Shadow:** `0 4px 20px rgba(0,0,0,0.6)`
- **Save button:** Crisp `#ee6633` background, white text, `border-radius: 4px`. **No split.**

### Discard Modal

Confirmation dialog triggered by Esc with unsaved annotations.

- **Backdrop:** `rgba(0,0,0,0.6)`
- **Dialog background:** `#1c1c1e`
- **Border:** `1px solid rgba(255,255,255,0.07)`
- **Chromatic split:** Applied via `::before` pseudo-element on the wrapping div
- **Shadow:** `0 16px 48px rgba(0,0,0,0.7)`
- **Cancel button:** `rgba(255,255,255,0.07)` bg, `#ccc` text
- **Discard button:** Transparent bg, `#ff453a` text, `rgba(255,69,58,0.35)` border. **No split.**
- **Save button:** Crisp `#ee6633`. **No split.**

### Note Textarea

Anchored below (or above) a rectangle.

- **Background:** `#161616`
- **Border:** `1px solid rgba(238,102,51,0.28)`
- **Border-radius:** `5px`
- **Text:** `#aaaaaa`
- **Placeholder:** `rgba(238,102,51,0.35)` — accent colour at low opacity
- **Shadow:** `0 2px 10px rgba(0,0,0,0.5)`
- **Font:** `13px/1.4 system-ui`
- No chromatic split — it's a text input, not a frame.

---

## What Gets the Glitch Treatment

| Element | Chromatic split? | Reasoning |
|---|---|---|
| Annotation rectangle | ✅ Yes | The product artifact — this is what glitches |
| Toolbar pill | ✅ Yes | A "surfaced" container floating over the overlay |
| Discard modal | ✅ Yes | A "surfaced" container interrupting the session |
| Save button | ❌ No | Interactive control — must read instantly |
| Delete × badge | ❌ No | Interactive control — must read instantly |
| Note textarea | ❌ No | Text input — split would hurt legibility |
| Modal action buttons | ❌ No | Interactive controls |

**Rule of thumb:** if it is a _frame_ or _container_ that surfaces over the overlay, it gets the chromatic split. If it is something the user _clicks_ or _types into_, it does not.

---

## Global Styles

```css
html, body, #root { margin: 0; padding: 0; height: 100%; }
body { background: #0c0c0c; color: #e0e0e0; }
```

The overlay background in live mode is `transparent` (user sees the real desktop). The dark body colour only shows during development or if the backdrop fails to load.
