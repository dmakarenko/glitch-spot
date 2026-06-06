# Redacting regions

Let users cover sensitive regions of a capture (passwords, tokens, PII) before saving or sharing. A redaction is a draw-mode-toggled rectangle whose pixels are destructively covered in **both** the clean and annotated images. The cover style is a persisted three-way setting: solid fill, pixelated, or branded pixelated.

## Problem

Every capture escapes the machine through several channels: the clean PNG is written to disk (`{id}_clean.png`), served over the public share tunnel (`/share/:id/:token/clean.png`), exposed via MCP, and the annotated image is copied to the clipboard ("Copy Image"). There is no way to hide sensitive content before it leaves. A redaction that only covered the annotated image would be worthless — the secret would still leak through the clean image.

## Core requirement (the linchpin)

**Both clean and annotated images must be redacted, from the same redacted base.** `snapshot::build::build()` is the *sole producer* of both escaping images:

- `clean_bytes` → `persist::write` → on-disk `{id}_clean.png` → share tunnel + MCP
- `annotated_image` (returned `RgbaImage`) → "Copy Image" clipboard format (`lib.rs` `ClipboardFormat::Image`)

The raw cached `latest_frame` is only shown back to the user in the overlay for annotating; it is never persisted or shared, and window-mode save routes back through `build()`. So redacting inside `build()` covers every downstream consumer automatically. There must be no code path where one image is redacted and the other is not.

## Interaction

A **`Redact` toggle** in the overlay toolbar switches draw mode. While Redact is on, drag-drawing creates a redaction box; while off, a normal annotation. The toggle resets to off on each new overlay present.

Redactions reuse the existing rectangle machinery rather than a parallel system — move / resize / delete / undo all work unchanged. A discriminator distinguishes the two kinds.

## Visual treatment — three styles, persisted setting

`RedactionStyle`: `"solid" | "pixelated" | "branded"`. Default: `"branded"` (truly irreversible **and** on-brand). Stored in `config.toml` under `[snapshot]` as `redaction_style = "branded"`.

| Style | Appearance | Reversible? |
|---|---|---|
| **Solid fill** | Opaque dark box | No — truly safe |
| **Pixelated** | Real averaged mosaic: coarse blocks, one flat averaged colour per block | Practically safe at a coarse block size; **not guaranteed** against a determined attacker on known-charset text (e.g. Depix on monospaced tokens) |
| **Branded pixelated** | Glitch/accent mosaic pattern whose pixels are **independent of the source content** | No — truly safe |

**Security note (must appear in code comments + this doc):** Of the three, **Pixelated** is the only one carrying a reversibility caveat. Solid and Branded destroy the underlying content entirely; Pixelated averages it, which is safe only while blocks are coarse relative to glyphs. The setting lets the user choose with this trade-off documented.

Block-size / pattern parameters scale with `scale_factor` so the cover looks consistent across Retina and non-Retina captures, and Rust and TS implementations must stay visually equivalent.

## Control surface — both toolbar and tray

Both controls read/write the same persisted `Settings` value (single source of truth, so they cannot diverge). The overlay reads the style at present-time; the tray reflects it on menu rebuild. Live cross-sync while both surfaces are open simultaneously is out of scope.

### Toolbar split control (`Toolbar.tsx`)

The `Redact` control is a split button mirroring the existing Save split button: the main button toggles redact mode; the chevron opens a style picker (Solid fill / Pixelated / Branded pixelated) with a checkmark on the active style. Picking a style persists it via `setRedactionStyle` and updates the preview; it does **not** trigger a save (unlike the format picker).

### Tray submenu (`tray.rs`)

A `Redaction style` submenu with three check items, mirroring the `Tray icon` and `Auto-delete snapshots` submenus. Menu ids `redaction:solid` / `redaction:pixelated` / `redaction:branded`; the handler calls `do_set_redaction_style` and `refresh_menu`.

## Data shapes

### Frontend (`src/overlay/annotations/types.ts`)

Add a discriminator to `Rect`:

```ts
type Rect = {
  id: string; x: number; y: number; w: number; h: number; note: string;
  kind: 'annotation' | 'redaction';
}
```

`BEGIN_CREATE` carries the kind. New rects default to `'annotation'`; redact mode creates `'redaction'`.

### IPC payload (`packages/electron/src/types.ts`)

`AnnotationPayload` gains `kind: 'annotation' | 'redaction'`.

### Snapshot JSON (`snapshot/mod.rs`)

Annotations array is unchanged (redactions are **not** annotations — no note, no label). Add a separate coords-only field:

```rust
pub redactions: Vec<RedactionRegion>,   // { x, y, w, h } physical px; #[serde(default)]
```

`#[serde(default)]` keeps older snapshots loadable. `agent_prompt` gains a sentence: "The user redacted N region(s); they are intentionally hidden — do not attempt to infer their contents." Coordinates aren't the secret, so recording them is low-risk and helps the agent. (Minor decision; open to recording nothing if preferred.)

## Divergence points for the single `rects` array (enumerated so none slip)

Reusing one array with a `kind` flag means these are the *only* places annotation and redaction behaviour differ:

1. **Label numbering** is computed over `kind === 'annotation'` rects only — in **both** `DrawingLayer` (`formatLabel`) and Rust `build` (`format_label`). A redaction never consumes a number; annotations stay `01, 02, …` sequential.
2. **DrawingLayer render:** a redaction draws a CSS placeholder reflecting the chosen style's family (solid dark / mosaic / accent-glitch) — no number badge, no `RectangleNote`, no note textarea. Resize handles + delete-X still appear.
3. **`SET_NOTE`** never targets a redaction.
4. **`build()` split:** incoming payload is partitioned into redactions and annotations before rendering.

## Rust changes

### `settings.rs`

- Add `RedactionStyle` enum (`Solid`, `Pixelated`, `Branded`) with `from_name`/`as_str`, mirroring `ClipboardFormat`. Default `Branded`.
- Add `redaction_style: RwLock<RedactionStyle>` field; `current_redaction_style()` / `save_redaction_style(dir, style)`.
- Add `redaction_style: Option<String>` to `SnapshotSection`; wire into `load_from` and `write_config_file`.
- Tests: missing → default `branded`; explicit parse; unknown → default; save round-trip; save preserves other sections.

### New module `snapshot/redact.rs`

Pure, unit-testable cover functions operating on `&mut RgbaImage` over a region:

- `apply_redaction(img, region, style, scale_factor)` dispatches on style.
- `solid_fill` — fill region with an opaque dark colour.
- `pixelate` — for each coarse block in the region, average the block's pixels and fill the block with that flat colour.
- `branded` — fill the region with a generated accent/glitch mosaic whose pattern does **not** read any source pixel.

### `snapshot/build.rs`

`build()` gains a `redaction_style: RedactionStyle` parameter. New flow:

```
incoming rects ──split──> redactions + annotations
        │
        ├─ redactions empty? ──> FAST PATH (byte-identical to today):
        │                         clean_bytes = frame.png.clone()
        │                         annotated = render_annotated_png(&frame.png, …)
        ▼ (redactions present)
   decode clean PNG → RgbaImage
   apply_redaction for each region (chosen style) ──> REDACTED image
        ├─ encode redacted ───────────────────────────> clean_bytes
        └─ clone + render annotations ────────────────> annotated_bytes / annotated_image
```

The **fast path** matters: re-encoding a full Retina frame costs ~1.6s (the reason `annotate.rs` uses a fixed `Up` PNG filter), so saves with no redactions must not pay a second encode. Redaction coords map through the same CSS→physical transform as annotations, including window-mode `crop_origin`.

`BuiltSnapshot` / `Snapshot` carry the new `redactions` field.

### `lib.rs`

- `save_snapshot` reads `state.settings.current_redaction_style()` and passes it to `build()`. (Both clean and annotated already flow from `build`'s outputs — no other change needed for the both-images guarantee.)
- New commands `get_redaction_style() -> String` and `set_redaction_style(style: String) -> Result<(), String>`; register both.
- `do_set_redaction_style` helper shared by the command and the tray handler (mirrors `do_set_tray_appearance`).

### `tray.rs`

`Redaction style` submenu (three check items) + menu-event handler, mirroring the tray-appearance submenu.

## Bridge changes (`src/overlay/bridge.ts`)

Two new `OverlayBridge` methods:

```ts
getRedactionStyle(): Promise<RedactionStyle>
setRedactionStyle(style: RedactionStyle): Promise<void>
```

- `TauriBridge`: delegates to the two new commands.
- `ElectronBridge`: `getRedactionStyle` returns `'branded'`; `setRedactionStyle` persists to the SDK host if it exposes config, else no-op. **Critical:** `ElectronBridge.saveSnapshot` currently passes the **raw** `cleanBase64` to the host (actual bytes, not a path) — it must redact the clean canvas first, then derive the annotated canvas from the redacted clean.

## Electron renderer changes (`src/overlay/annotate.ts`)

- Add a `redactCanvas(ctx, regions, style, scaleFactor)` helper with the three cover styles (canvas equivalents of `redact.rs`; for pixelation use deliberate block-averaging, **not** the nearest-neighbour downscale/upscale trick, which keeps one source pixel per block).
- `renderAnnotated` (or the bridge save path) applies redaction to the clean canvas first; the annotated output is rendered on top of the redacted clean. Export a redacted clean base64 as well, so `saveSnapshot` sends redacted bytes for both images.

## Frontend changes

### `Overlay.tsx`

- New state `redactMode: boolean` (reset to false on present) and `redactionStyle: RedactionStyle` (fetched on mount via `bridge.getRedactionStyle()`).
- `BEGIN_CREATE` dispatched from the drawing layer uses `kind = redactMode ? 'redaction' : 'annotation'`.
- `handleSave` payload maps each rect's `kind` into `AnnotationPayload`.
- `handleStyleChange(style)`: `await bridge.setRedactionStyle(style)` → `setRedactionStyle(style)` (no save).
- Pass `redactMode`, `redactionStyle`, toggle + style-change handlers to `<Toolbar>`; pass `redactionStyle` to `<DrawingLayer>` for the preview.

### `DrawingLayer.tsx`

- Accept `redactionStyle` and render `kind === 'redaction'` rects with the style-family placeholder (no badge / note / number); annotation rects render exactly as today.
- `BEGIN_CREATE` carries the current kind (passed down from `Overlay`).

### `Toolbar.tsx`

- Add the `Redact` split control left of (or beside) the Save split button: toggle button (active state styled) + chevron opening the style picker. Outside-click closes the picker (reuse the existing dropdown pattern).

## Testing

### Rust

- `redact.rs`: each cover fn in isolation — solid fills opaque; pixelate produces flat per-block colour and differs from source; branded fill is independent of source (same output regardless of input pixels).
- `build()` with a redaction: **`clean_bytes != frame.png`** (the anti-footgun regression guard — fails loudly if anyone reverts to `frame.png.clone()`), pixels outside the region unchanged, annotated also covered, label numbering skips redactions.
- `build()` with no redactions: fast path unchanged (clean equals `frame.png`).
- `settings.rs`: the round-trip / preserve-other-sections suite above.

### TS

- Reducer: `kind` carried through `BEGIN_CREATE`; annotation-only label numbering (a redaction between two annotations doesn't renumber them).
- `annotate.ts`: redacted region differs from source for each style; annotated derived from redacted clean.

### Done-gate (blocks calling the feature complete)

Re-grep every clean-image exit point (`frame.png`, `latest_frame`, `clean`, the clipboard `Image` handler) and confirm `build()` is the only producer of any persisted / shared / clipboard image, and that the cached raw `latest_frame` is never written or shared.

## Open questions / risks

- **Pixelated reversibility:** documented caveat (above). If the threat model warrants, default could be forced to a truly-safe style — currently default is `branded`, which is safe.
- **`redactions` in JSON:** recording coords-only is the chosen default; trivially droppable to "record nothing" if preferred.
- **Two mosaic implementations:** Rust (`build`) and Electron (canvas) are independent and must stay visually equivalent; Electron is the sharper leak risk because it ships raw bytes to the host.
- **Live-mode preview:** the in-overlay redaction box is a stylized placeholder (we have no live-screen pixels in the DOM); the real cover is baked at save time. Consistent with annotations previewing as outlines.
