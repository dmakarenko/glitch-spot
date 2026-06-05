# Save Format Split Button Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Replace the single Save button in the overlay toolbar with a persistent-default split button that lets users choose between copying JSON, an annotated image, or a formatted prompt to the clipboard on save.

**Architecture:** The clipboard format preference is stored in `config.toml` under `[snapshot].clipboard_format` and read by `save_snapshot` at save time — no parameter threading needed. The split button (left half = run default, right chevron = pick new default + save) lives in `Toolbar.tsx`; `Overlay.tsx` loads and owns the format state, passing it down as props.

**Tech Stack:** Rust (settings.rs, lib.rs, tauri-plugin-clipboard-manager), TypeScript/React (Vitest + @testing-library/react)

---

### Task 1: ClipboardFormat in settings.rs

**Files:**
- Modify: `src-tauri/src/settings.rs`

- [ ] **Step 1: Write failing tests**

Add at the bottom of the `#[cfg(test)] mod tests` block in `src-tauri/src/settings.rs`:

```rust
#[test]
fn missing_clipboard_format_yields_json_default() {
    let temp = tempfile::TempDir::new().unwrap();
    let settings = Settings::load_from(temp.path()).unwrap();
    assert_eq!(settings.current_clipboard_format(), ClipboardFormat::Json);
}

#[test]
fn explicit_clipboard_format_image_is_parsed() {
    let temp = tempfile::TempDir::new().unwrap();
    std::fs::write(
        temp.path().join("config.toml"),
        b"[snapshot]\nclipboard_format = \"image\"\n",
    )
    .unwrap();
    let settings = Settings::load_from(temp.path()).unwrap();
    assert_eq!(settings.current_clipboard_format(), ClipboardFormat::Image);
}

#[test]
fn unknown_clipboard_format_falls_back_to_json() {
    let temp = tempfile::TempDir::new().unwrap();
    std::fs::write(
        temp.path().join("config.toml"),
        b"[snapshot]\nclipboard_format = \"bogus\"\n",
    )
    .unwrap();
    let settings = Settings::load_from(temp.path()).unwrap();
    assert_eq!(settings.current_clipboard_format(), ClipboardFormat::Json);
}

#[test]
fn save_clipboard_format_roundtrip() {
    let temp = tempfile::TempDir::new().unwrap();
    let settings = Settings::load_from(temp.path()).unwrap();
    settings.save_clipboard_format(temp.path(), ClipboardFormat::Image).unwrap();

    let reloaded = Settings::load_from(temp.path()).unwrap();
    assert_eq!(reloaded.current_clipboard_format(), ClipboardFormat::Image);
}

#[test]
fn save_clipboard_format_preserves_other_sections() {
    let temp = tempfile::TempDir::new().unwrap();
    let settings = Settings::load_from(temp.path()).unwrap();
    settings.save_provider(temp.path(), ProviderKind::Ngrok).unwrap();
    settings.save_tray_appearance(temp.path(), false, false).unwrap();
    settings.save_clipboard_format(temp.path(), ClipboardFormat::Image).unwrap();

    let reloaded = Settings::load_from(temp.path()).unwrap();
    assert_eq!(reloaded.current_provider(), ProviderKind::Ngrok, "provider preserved");
    assert_eq!(reloaded.current_tray(), (false, false), "tray preserved");
    assert_eq!(reloaded.current_clipboard_format(), ClipboardFormat::Image, "format applied");
}
```

- [ ] **Step 2: Run tests to confirm they fail**

```bash
cd src-tauri && cargo test clipboard_format 2>&1 | tail -20
```

Expected: compile error — `ClipboardFormat` not yet defined.

- [ ] **Step 3: Add ClipboardFormat enum**

In `src-tauri/src/settings.rs`, add after the `SnapshotRetention` block (around line 61):

```rust
/// Which format is copied to the clipboard after a successful save.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ClipboardFormat {
    #[default]
    Json,
    Image,
    Prompt,
}

impl ClipboardFormat {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Json    => "json",
            Self::Image   => "image",
            Self::Prompt  => "prompt",
        }
    }

    pub fn from_name(s: &str) -> Option<Self> {
        match s {
            "json"   => Some(Self::Json),
            "image"  => Some(Self::Image),
            "prompt" => Some(Self::Prompt),
            _        => None,
        }
    }
}
```

- [ ] **Step 4: Add clipboard_format field to SnapshotSection**

Change the `SnapshotSection` struct:

```rust
#[derive(Debug, Serialize, Deserialize, Default)]
struct SnapshotSection {
    #[serde(default)]
    retention: Option<String>,
    #[serde(default)]
    clipboard_format: Option<String>,
}
```

- [ ] **Step 5: Add clipboard_format field to Settings**

Add the field to the `Settings` struct:

```rust
pub struct Settings {
    provider: RwLock<ProviderKind>,
    mcp_port: u16,
    tray_animated: Arc<AtomicBool>,
    tray_coloured: Arc<AtomicBool>,
    snapshot_retention: RwLock<SnapshotRetention>,
    clipboard_format: RwLock<ClipboardFormat>,
}
```

- [ ] **Step 6: Wire into load_from**

Add the parse block to `load_from`, alongside the `snapshot_retention` parse (around line 163):

```rust
let clipboard_format = parsed
    .get("snapshot")
    .and_then(|s| s.get("clipboard_format"))
    .and_then(|v| v.as_str())
    .and_then(ClipboardFormat::from_name)
    .unwrap_or_default();
```

And add the field to the `Ok(Settings { … })` constructor at the end of `load_from`:

```rust
Ok(Settings {
    provider: RwLock::new(provider),
    mcp_port,
    tray_animated: Arc::new(AtomicBool::new(tray_animated)),
    tray_coloured: Arc::new(AtomicBool::new(tray_coloured)),
    snapshot_retention: RwLock::new(snapshot_retention),
    clipboard_format: RwLock::new(clipboard_format),
})
```

- [ ] **Step 7: Add accessor and save method**

Add alongside `current_retention` / `save_retention`:

```rust
pub fn current_clipboard_format(&self) -> ClipboardFormat {
    *self.clipboard_format.read().expect("settings lock poisoned")
}

pub fn save_clipboard_format(&self, dir: &Path, format: ClipboardFormat) -> io::Result<()> {
    {
        let mut w = self.clipboard_format.write().expect("settings lock poisoned");
        *w = format;
    }
    self.write_config_file(dir)
}
```

- [ ] **Step 8: Include in write_config_file**

Update `write_config_file` to include the new field in `SnapshotSection`:

```rust
snapshot: SnapshotSection {
    retention: Some(self.current_retention().as_str().to_string()),
    clipboard_format: Some(self.current_clipboard_format().as_str().to_string()),
},
```

- [ ] **Step 9: Run tests**

```bash
cd src-tauri && cargo test clipboard_format 2>&1 | tail -20
```

Expected: all 5 new tests pass, plus all pre-existing settings tests still pass.

```bash
cd src-tauri && cargo test 2>&1 | tail -5
```

Expected: `test result: ok. N passed; 0 failed`.

- [ ] **Step 10: Commit**

```bash
git add src-tauri/src/settings.rs
git commit -m "feat(settings): add ClipboardFormat preference to config.toml"
```

---

### Task 2: Tauri commands + save_snapshot clipboard switch

**Files:**
- Modify: `src-tauri/src/lib.rs`

- [ ] **Step 1: Write failing test for the settings commands**

There are no unit-testable integration tests for Tauri commands themselves (they require a live app handle), but the settings behaviour is already fully tested in Task 1. This task focuses on the compile-time contract. The manual test plan at the end of the step covers runtime verification.

Skip to Step 2.

- [ ] **Step 2: Add get_clipboard_format command**

In `src-tauri/src/lib.rs`, add alongside `set_tray_appearance` (around line 740):

```rust
#[tauri::command]
fn get_clipboard_format(state: tauri::State<'_, AppState>) -> String {
    state.settings.current_clipboard_format().as_str().to_string()
}

#[tauri::command]
fn set_clipboard_format(
    state: tauri::State<'_, AppState>,
    format: String,
) -> Result<(), String> {
    let fmt = settings::ClipboardFormat::from_name(&format)
        .ok_or_else(|| format!("unknown clipboard format: {format}"))?;
    state
        .settings
        .save_clipboard_format(&state.config_dir, fmt)
        .map_err(|e| e.to_string())
}
```

- [ ] **Step 3: Register both commands**

In `generate_handler!` (around line 1069), add the two new commands:

```rust
.invoke_handler(tauri::generate_handler![
    capture_active_monitor,
    open_overlay_live,
    open_overlay_frozen,
    hide_overlay,
    refresh_cursor,
    reveal_window,
    get_latest_frame,
    save_snapshot,
    spike_capture::spike_capture,
    share_latest,
    current_tunnel_state,
    set_tunnel_provider,
    picker_select,
    picker_cancel,
    set_tray_appearance,
    get_clipboard_format,
    set_clipboard_format,
])
```

- [ ] **Step 4: Update save_snapshot to switch clipboard behavior**

Replace the clipboard write block inside `save_snapshot` (currently lines 232–235):

```rust
// Before (remove this):
let json = serde_json::to_string_pretty(&snap).map_err(|e| e.to_string())?;
app.clipboard()
    .write_text(json)
    .map_err(|e| format!("clipboard: {e}"))?;
```

Replace with:

```rust
match state.settings.current_clipboard_format() {
    settings::ClipboardFormat::Json | settings::ClipboardFormat::Prompt => {
        // Prompt is reserved; falls through to JSON until implemented.
        let json = serde_json::to_string_pretty(&snap).map_err(|e| e.to_string())?;
        app.clipboard()
            .write_text(json)
            .map_err(|e| format!("clipboard: {e}"))?;
    }
    settings::ClipboardFormat::Image => {
        let rgba = image::load_from_memory_with_format(
            &built.annotated_bytes,
            image::ImageFormat::Png,
        )
        .map_err(|e| format!("image decode: {e}"))?
        .to_rgba8();
        let (w, h) = (rgba.width(), rgba.height());
        let tauri_img = tauri::image::Image::new_owned(rgba.into_vec(), w, h);
        app.clipboard()
            .write_image(&tauri_img)
            .map_err(|e| format!("clipboard image: {e}"))?;
    }
}
```

- [ ] **Step 5: Verify the project compiles cleanly**

```bash
cd src-tauri && cargo build 2>&1 | grep -E "^error" | head -20
```

Expected: no output (zero errors).

- [ ] **Step 6: Commit**

```bash
git add src-tauri/src/lib.rs
git commit -m "feat(save): switch clipboard format based on persisted setting"
```

---

### Task 3: Bridge interface + ClipboardFormat type

**Files:**
- Modify: `src/overlay/bridge.ts`
- Modify: `src/overlay/bridge.test.ts`

- [ ] **Step 1: Write failing tests**

Add to `src/overlay/bridge.test.ts`:

```ts
describe('TauriBridge clipboard format', () => {
  it('exposes getClipboardFormat', () => {
    const { createBridge } = require('./bridge')
    const bridge = createBridge()
    expect(typeof bridge.getClipboardFormat).toBe('function')
  })

  it('exposes setClipboardFormat', () => {
    const { createBridge } = require('./bridge')
    const bridge = createBridge()
    expect(typeof bridge.setClipboardFormat).toBe('function')
  })
})

describe('ElectronBridge clipboard format', () => {
  it('getClipboardFormat resolves to json', async () => {
    window.glitchSpotBridge = {
      onFrame: vi.fn(), offFrame: vi.fn(), saveSnapshot: vi.fn(), cancel: vi.fn(),
    }
    const { createBridge } = await import('./bridge')
    const bridge = createBridge()
    await expect(bridge.getClipboardFormat()).resolves.toBe('json')
  })

  it('setClipboardFormat resolves without throwing', async () => {
    window.glitchSpotBridge = {
      onFrame: vi.fn(), offFrame: vi.fn(), saveSnapshot: vi.fn(), cancel: vi.fn(),
    }
    const { createBridge } = await import('./bridge')
    const bridge = createBridge()
    await expect(bridge.setClipboardFormat('image')).resolves.toBeUndefined()
  })
})
```

- [ ] **Step 2: Run tests to confirm they fail**

```bash
npm test -- --reporter=verbose src/overlay/bridge.test.ts 2>&1 | tail -20
```

Expected: FAIL — `getClipboardFormat is not a function`.

- [ ] **Step 3: Add ClipboardFormat type and extend OverlayBridge**

In `src/overlay/bridge.ts`, add the type export after the imports:

```ts
export type ClipboardFormat = 'json' | 'image' | 'prompt'
```

Add two new methods to the `OverlayBridge` interface:

```ts
export interface OverlayBridge {
  onFrame(cb: (blobUrl: string) => void): Promise<() => void>
  getFrame(): Promise<string | null>
  saveSnapshot(mode: string, annotations: AnnotationPayload[]): Promise<void>
  cancel(): void
  refreshCursor(): void
  revealOverlay(): void
  getClipboardFormat(): Promise<ClipboardFormat>
  setClipboardFormat(format: ClipboardFormat): Promise<void>
}
```

- [ ] **Step 4: Implement in TauriBridge**

Add the two methods to the `TauriBridge` class:

```ts
async getClipboardFormat(): Promise<ClipboardFormat> {
  return invoke<string>('get_clipboard_format') as Promise<ClipboardFormat>
}
async setClipboardFormat(format: ClipboardFormat): Promise<void> {
  await invoke('set_clipboard_format', { format })
}
```

- [ ] **Step 5: Implement in ElectronBridge**

Add the two methods to the `ElectronBridge` class:

```ts
async getClipboardFormat(): Promise<ClipboardFormat> {
  return 'json'
}
async setClipboardFormat(_format: ClipboardFormat): Promise<void> {}
```

- [ ] **Step 6: Run tests**

```bash
npm test -- --reporter=verbose src/overlay/bridge.test.ts 2>&1 | tail -20
```

Expected: all bridge tests pass.

- [ ] **Step 7: Commit**

```bash
git add src/overlay/bridge.ts src/overlay/bridge.test.ts
git commit -m "feat(bridge): add getClipboardFormat / setClipboardFormat methods"
```

---

### Task 4: Toolbar split button

**Files:**
- Modify: `src/overlay/Toolbar.tsx`
- Create: `src/overlay/Toolbar.test.tsx`

- [ ] **Step 1: Write failing tests**

Create `src/overlay/Toolbar.test.tsx`:

```tsx
// ABOUTME: Tests for the Toolbar split button — save action, format dropdown, and default tracking.
// @vitest-environment jsdom

import { describe, it, expect, vi, afterEach } from 'vitest'
import { render, fireEvent, cleanup } from '@testing-library/react'
import { Toolbar } from './Toolbar'

afterEach(cleanup)

describe('Toolbar', () => {
  it('renders nothing when rectCount is 0', () => {
    const { container } = render(
      <Toolbar rectCount={0} clipboardFormat="json" onSave={vi.fn()} onChangeFormat={vi.fn()} />
    )
    expect(container.firstChild).toBeNull()
  })

  it('renders the save button with the current format label', () => {
    const { getByText } = render(
      <Toolbar rectCount={2} clipboardFormat="json" onSave={vi.fn()} onChangeFormat={vi.fn()} />
    )
    expect(getByText(/Copy JSON/)).toBeTruthy()
  })

  it('renders image label when format is image', () => {
    const { getByText } = render(
      <Toolbar rectCount={1} clipboardFormat="image" onSave={vi.fn()} onChangeFormat={vi.fn()} />
    )
    expect(getByText(/Copy Image/)).toBeTruthy()
  })

  it('calls onSave when left button is clicked', () => {
    const onSave = vi.fn()
    const { getByTestId } = render(
      <Toolbar rectCount={1} clipboardFormat="json" onSave={onSave} onChangeFormat={vi.fn()} />
    )
    fireEvent.click(getByTestId('toolbar-save-btn'))
    expect(onSave).toHaveBeenCalledTimes(1)
  })

  it('opens dropdown when chevron is clicked', () => {
    const { getByTestId, queryByTestId } = render(
      <Toolbar rectCount={1} clipboardFormat="json" onSave={vi.fn()} onChangeFormat={vi.fn()} />
    )
    expect(queryByTestId('toolbar-dropdown')).toBeNull()
    fireEvent.click(getByTestId('toolbar-chevron'))
    expect(queryByTestId('toolbar-dropdown')).toBeTruthy()
  })

  it('calls onChangeFormat with the selected format and closes dropdown', () => {
    const onChangeFormat = vi.fn()
    const { getByTestId, queryByTestId } = render(
      <Toolbar rectCount={1} clipboardFormat="json" onSave={vi.fn()} onChangeFormat={onChangeFormat} />
    )
    fireEvent.click(getByTestId('toolbar-chevron'))
    fireEvent.click(getByTestId('toolbar-option-image'))
    expect(onChangeFormat).toHaveBeenCalledWith('image')
    expect(queryByTestId('toolbar-dropdown')).toBeNull()
  })

  it('shows a checkmark on the active format option', () => {
    const { getByTestId } = render(
      <Toolbar rectCount={1} clipboardFormat="image" onSave={vi.fn()} onChangeFormat={vi.fn()} />
    )
    fireEvent.click(getByTestId('toolbar-chevron'))
    expect(getByTestId('toolbar-option-image').dataset.active).toBe('true')
    expect(getByTestId('toolbar-option-json').dataset.active).toBe('false')
  })

  it('prompt option is rendered but disabled', () => {
    const onChangeFormat = vi.fn()
    const { getByTestId } = render(
      <Toolbar rectCount={1} clipboardFormat="json" onSave={vi.fn()} onChangeFormat={onChangeFormat} />
    )
    fireEvent.click(getByTestId('toolbar-chevron'))
    const promptOption = getByTestId('toolbar-option-prompt')
    expect(promptOption.dataset.disabled).toBe('true')
    fireEvent.click(promptOption)
    expect(onChangeFormat).not.toHaveBeenCalled()
  })
})
```

- [ ] **Step 2: Run tests to confirm they fail**

```bash
npm test -- --reporter=verbose src/overlay/Toolbar.test.tsx 2>&1 | tail -20
```

Expected: FAIL — `onChangeFormat is not a function` / props do not exist on Toolbar.

- [ ] **Step 3: Rewrite Toolbar.tsx**

Replace the full contents of `src/overlay/Toolbar.tsx`:

```tsx
// ABOUTME: Top-center floating toolbar for the overlay session.
// ABOUTME: Shows the rect count and a split Save button with a persistent clipboard-format picker.

import { useState, useEffect, useRef } from 'react'
import type { ClipboardFormat } from './bridge'

const FORMAT_LABELS: Record<ClipboardFormat, string> = {
  json:   'Copy JSON',
  image:  'Copy Image',
  prompt: 'Copy Prompt',
}

type Props = {
  rectCount: number
  clipboardFormat: ClipboardFormat
  onSave: () => void
  onChangeFormat: (format: ClipboardFormat) => void
}

export function Toolbar({ rectCount, clipboardFormat, onSave, onChangeFormat }: Props) {
  const [dropdownOpen, setDropdownOpen] = useState(false)
  const dropdownRef = useRef<HTMLDivElement>(null)

  useEffect(() => {
    if (!dropdownOpen) return
    const close = (e: MouseEvent) => {
      if (dropdownRef.current && !dropdownRef.current.contains(e.target as Node)) {
        setDropdownOpen(false)
      }
    }
    document.addEventListener('mousedown', close)
    return () => document.removeEventListener('mousedown', close)
  }, [dropdownOpen])

  if (rectCount === 0) return null

  const handlePickFormat = (format: ClipboardFormat, disabled: boolean) => {
    if (disabled) return
    setDropdownOpen(false)
    onChangeFormat(format)
  }

  const options: { format: ClipboardFormat; disabled: boolean }[] = [
    { format: 'json',   disabled: false },
    { format: 'image',  disabled: false },
    { format: 'prompt', disabled: true },
  ]

  return (
    <div
      className="glitch-frame"
      style={{
        position: 'fixed',
        top: 12,
        left: '50%',
        transform: 'translateX(-50%)',
        borderRadius: 9,
        zIndex: 10,
      }}
    >
      <div
        onMouseDown={(e) => e.stopPropagation()}
        onClick={(e) => e.stopPropagation()}
        style={{
          display: 'flex',
          alignItems: 'center',
          gap: 12,
          padding: '8px 12px',
          background: 'rgba(18,18,18,0.94)',
          color: '#e0e0e0',
          font: '13px/1 system-ui',
          borderRadius: 8,
          border: '1px solid rgba(255,255,255,0.07)',
          boxShadow: '0 4px 20px rgba(0,0,0,0.6)',
          backdropFilter: 'blur(8px)',
          WebkitBackdropFilter: 'blur(8px)',
          pointerEvents: 'auto',
          position: 'relative',
        }}
      >
        <span>{rectCount} rectangle{rectCount === 1 ? '' : 's'}</span>

        {/* Split button */}
        <div ref={dropdownRef} style={{ position: 'relative', display: 'flex', borderRadius: 4, overflow: 'visible' }}>
          {/* Left: run default */}
          <button
            data-testid="toolbar-save-btn"
            type="button"
            onClick={onSave}
            style={{
              padding: '6px 12px',
              background: '#ee6633',
              color: 'white',
              border: 'none',
              borderRight: '1px solid rgba(255,255,255,0.20)',
              borderRadius: '4px 0 0 4px',
              fontWeight: 600,
              cursor: 'pointer',
              whiteSpace: 'nowrap',
            }}
          >
            Save &amp; {FORMAT_LABELS[clipboardFormat]} (⌘⇧↩)
          </button>

          {/* Right: chevron */}
          <button
            data-testid="toolbar-chevron"
            type="button"
            onClick={() => setDropdownOpen((o) => !o)}
            style={{
              padding: '6px 8px',
              background: '#ee6633',
              color: 'white',
              border: 'none',
              borderRadius: '0 4px 4px 0',
              cursor: 'pointer',
              fontSize: 10,
            }}
          >
            {dropdownOpen ? '▴' : '▾'}
          </button>

          {/* Dropdown */}
          {dropdownOpen && (
            <div
              data-testid="toolbar-dropdown"
              style={{
                position: 'absolute',
                top: 'calc(100% + 6px)',
                right: 0,
                background: '#1e1e1e',
                border: '1px solid rgba(255,255,255,0.10)',
                borderRadius: 6,
                boxShadow: '0 8px 24px rgba(0,0,0,0.7)',
                minWidth: 190,
                overflow: 'hidden',
                zIndex: 100,
              }}
            >
              <div style={{
                padding: '8px 12px',
                font: '12px/1.4 system-ui',
                color: '#666',
                borderBottom: '1px solid rgba(255,255,255,0.06)',
              }}>
                Save &amp; copy to clipboard
              </div>
              {options.map(({ format, disabled }) => {
                const active = format === clipboardFormat
                return (
                  <div
                    key={format}
                    data-testid={`toolbar-option-${format}`}
                    data-active={String(active)}
                    data-disabled={String(disabled)}
                    onClick={() => handlePickFormat(format, disabled)}
                    style={{
                      padding: '9px 12px',
                      font: '13px/1 system-ui',
                      color: disabled ? '#555' : active ? '#fff' : '#e0e0e0',
                      background: active ? '#ee6633' : 'transparent',
                      cursor: disabled ? 'default' : 'pointer',
                      display: 'flex',
                      alignItems: 'center',
                      gap: 8,
                    }}
                  >
                    <span style={{ fontSize: 11, opacity: active ? 0.8 : 0 }}>✓</span>
                    {FORMAT_LABELS[format]}{disabled ? ' (soon)' : ''}
                  </div>
                )
              })}
            </div>
          )}
        </div>
      </div>
    </div>
  )
}
```

- [ ] **Step 4: Run tests**

```bash
npm test -- --reporter=verbose src/overlay/Toolbar.test.tsx 2>&1 | tail -20
```

Expected: all Toolbar tests pass.

- [ ] **Step 5: Commit**

```bash
git add src/overlay/Toolbar.tsx src/overlay/Toolbar.test.tsx
git commit -m "feat(toolbar): split button with persistent clipboard-format picker"
```

---

### Task 5: Overlay wiring

**Files:**
- Modify: `src/overlay/Overlay.tsx`
- Modify: `src/overlay/Overlay.test.tsx`

- [ ] **Step 1: Write failing tests**

Open `src/overlay/Overlay.test.tsx`. Add the following tests (inspect the existing file first to find the right `describe` block to extend, or append a new one):

```tsx
// Add near the top with other imports:
import type { ClipboardFormat } from './bridge'

// Add a new describe block:
describe('Overlay clipboard format', () => {
  it('passes clipboardFormat prop to Toolbar', async () => {
    // Mock bridge so getClipboardFormat resolves to 'image'
    vi.mock('./bridge', async (importOriginal) => {
      const actual = await importOriginal<typeof import('./bridge')>()
      return {
        ...actual,
        createBridge: () => ({
          onFrame: () => Promise.resolve(() => {}),
          getFrame: () => Promise.resolve(null),
          saveSnapshot: vi.fn(),
          cancel: vi.fn(),
          refreshCursor: vi.fn(),
          revealOverlay: vi.fn(),
          getClipboardFormat: () => Promise.resolve('image' as ClipboardFormat),
          setClipboardFormat: vi.fn(),
        }),
      }
    })

    const { findByText } = render(<Overlay />)
    // Wait for the async getClipboardFormat to resolve and re-render
    await findByText(/Copy Image/)
  })
})
```

> **Note:** If `Overlay.test.tsx` already mocks `bridge`, extend the existing mock to include `getClipboardFormat` and `setClipboardFormat` on every mock bridge object in the file — the Overlay will throw if the bridge it receives is missing those methods.

- [ ] **Step 2: Run tests to confirm they fail**

```bash
npm test -- --reporter=verbose src/overlay/Overlay.test.tsx 2>&1 | tail -30
```

Expected: FAIL or type error — `Toolbar` missing `clipboardFormat` / `onChangeFormat` props.

- [ ] **Step 3: Update Overlay.tsx**

Add the import at the top of `src/overlay/Overlay.tsx`:

```ts
import type { ClipboardFormat } from './bridge'
```

Add state inside the `Overlay` function component, alongside existing state declarations:

```ts
const [clipboardFormat, setClipboardFormat] = useState<ClipboardFormat>('json')
```

Inside the existing mount `useEffect` (the one that calls `bridge.onFrame`), add a one-shot load of the stored preference. Add this at the top of that effect, before the `bridge.onFrame(showBlob).then(…)` chain:

```ts
bridge.getClipboardFormat().then(setClipboardFormat).catch(console.error)
```

Add `handleFormatChange` alongside `handleSave`:

```ts
const handleFormatChange = async (format: ClipboardFormat) => {
  try {
    await bridge.setClipboardFormat(format)
    setClipboardFormat(format)
    handleSave()
  } catch (err) {
    console.error('setClipboardFormat failed', err)
  }
}
```

Update both `<Toolbar>` usages (one in window mode, one in the default render) to pass the new props:

```tsx
<Toolbar
  rectCount={annotations.rects.length}
  clipboardFormat={clipboardFormat}
  onSave={handleSave}
  onChangeFormat={handleFormatChange}
/>
```

- [ ] **Step 4: Run tests**

```bash
npm test -- --reporter=verbose src/overlay/Overlay.test.tsx 2>&1 | tail -30
```

Expected: all Overlay tests pass (both new and pre-existing).

- [ ] **Step 5: Run the full test suite**

```bash
npm test 2>&1 | tail -10
```

Expected: `N tests passed | 0 failed`.

- [ ] **Step 6: Commit**

```bash
git add src/overlay/Overlay.tsx src/overlay/Overlay.test.tsx
git commit -m "feat(overlay): load and pass clipboard format to Toolbar"
```

---

## Self-Review

**Spec coverage:**
- ✅ Split button (left = default, right = chevron) — Task 4
- ✅ Preference persisted in config.toml — Task 1
- ✅ `save_snapshot` switches clipboard behavior — Task 2
- ✅ Image format: PNG→RGBA→tauri::image::Image — Task 2
- ✅ Prompt renders disabled, falls through to JSON — Tasks 2 + 4
- ✅ ⌘⇧↩ uses stored default (unchanged, reads from settings) — no code change needed, covered by existing keyboard handler
- ✅ ElectronBridge stubs return `"json"` / no-op — Task 3
- ✅ `get_clipboard_format` + `set_clipboard_format` Tauri commands — Task 2

**No placeholders:** All steps contain exact code. No TBDs.

**Type consistency:** `ClipboardFormat` exported from `bridge.ts`, imported by `Toolbar.tsx` and `Overlay.tsx`. `FORMAT_LABELS` in `Toolbar.tsx` is `Record<ClipboardFormat, string>` — all three variants present. `data-testid` names used in tests match those in the implementation.
