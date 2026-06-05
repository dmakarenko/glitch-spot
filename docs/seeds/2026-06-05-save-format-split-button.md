# Save format split button

Replace the single Save button in the overlay toolbar with a persistent-default split button that lets users choose whether saving copies JSON, an annotated image, or a formatted prompt to the clipboard.

## Problem

The Save action always copies a JSON snapshot to the clipboard. There is no way to copy the annotated screenshot image directly, and no affordance for a future "copy as prompt" action.

## Solution

A split button: the **left half** executes the stored default immediately; the **right chevron** opens a small dropdown to pick a different format, which both updates the stored default and triggers a save. The keyboard shortcut ⌘⇧↩ always uses the stored default.

## UX

- Button label tracks the current default: `Save & Copy JSON`, `Save & Copy Image`, `Save & Copy Prompt`
- Dropdown shows a checkmark next to the active option
- Picking from the dropdown: update stored default → save immediately
- Dropdown closes on outside-click or Escape
- "Copy as Prompt" item is rendered but disabled until the feature is implemented (third option, future work)

## Data shape

`ClipboardFormat`: `"json" | "image" | "prompt"`. Default: `"json"` (preserves existing behaviour). Stored in `config.toml` under `[snapshot]` as `clipboard_format = "json"`.

## Rust changes (`settings.rs`)

- Add `ClipboardFormat` enum (`Json`, `Image`, `Prompt`) with `from_name`/`as_str`, mirroring `SnapshotRetention`
- Add `clipboard_format: RwLock<ClipboardFormat>` field to `Settings`
- Add `current_clipboard_format()` and `save_clipboard_format(dir, format)` methods
- Add `clipboard_format: Option<String>` to `SnapshotSection`
- Update `load_from` and `write_config_file` to include the new field

## Rust changes (`lib.rs`)

- `save_snapshot` reads `state.settings.current_clipboard_format()` and switches clipboard behaviour:
  - `Json` → existing `write_text(json)` path
  - `Image` → write annotated PNG bytes to clipboard via `write_image` (verify `tauri_plugin_clipboard_manager` supports this; fall back to AppKit if not)
  - `Prompt` → reserved, falls through to `write_text(json)` until implemented
- New command `get_clipboard_format() -> String` — returns the current stored value
- New command `set_clipboard_format(format: String) -> Result<(), String>` — validates, updates in-memory + disk; both commands registered in `tauri::Builder`

## Bridge changes (`bridge.ts`)

Two new methods on `OverlayBridge`:

```ts
getClipboardFormat(): Promise<ClipboardFormat>
setClipboardFormat(format: ClipboardFormat): Promise<void>
```

- `TauriBridge`: delegates to the two new Tauri commands
- `ElectronBridge`: `getClipboardFormat` returns `"json"`; `setClipboardFormat` is a no-op (Electron always copies image natively; format selection doesn't apply)

## Frontend changes (`Overlay.tsx`)

- New state: `const [clipboardFormat, setClipboardFormat] = useState<ClipboardFormat>('json')`
- On mount (inside the existing `useEffect`): `bridge.getClipboardFormat().then(setClipboardFormat)`
- `handleFormatChange(format: ClipboardFormat)`:
  1. `await bridge.setClipboardFormat(format)`
  2. `setClipboardFormat(format)`
  3. `handleSave()`
- Pass `clipboardFormat` and `onChangeFormat={handleFormatChange}` to `<Toolbar>`

## Frontend changes (`Toolbar.tsx`)

New props:
```ts
clipboardFormat: ClipboardFormat
onChangeFormat: (f: ClipboardFormat) => void
```

New local state: `dropdownOpen: boolean`.

Render:
- Left half: `<button onClick={onSave}>Save & Copy {label}</button>`
- Right half: `<button onClick={toggleDropdown}>▾</button>` — same orange background, separated by a 1px semi-transparent divider
- Dropdown: absolutely positioned below the chevron, dark background matching toolbar, three items with checkmark on active one; Prompt item disabled (greyed, `pointer-events: none`)
- Outside-click handler closes dropdown (attach to `document` on open, remove on close)

## Open questions / risks

- **`write_image` availability**: `tauri_plugin_clipboard_manager` may only expose `write_text`. Verify before implementing the Image path; if absent, use AppKit `NSPasteboard` directly via `objc2`.
- **Prompt format**: the third option's exact template is not designed yet — the item will be rendered but disabled until a follow-up spec is written.
