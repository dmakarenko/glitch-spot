# `@glitch-spot/electron` — Integration Guide

> Embed the Glitch Spot annotation overlay into any Electron + TypeScript + React application. Press a hotkey, drag rectangles, type notes, hit `Cmd+Return` — your callback receives a structured `GlitchSnapshot`.

This guide is written so that an autonomous coding agent (or a human following step-by-step) can integrate the SDK into an existing Electron app without ambiguity. Every step has an exact command, an exact code snippet, and an exact expected output.

---

## Audience and assumptions

You are integrating the SDK into:

- An **Electron app** (v28 or newer) on **macOS** (v1 is macOS-only — see [§9 Platform support](#9-platform-support)).
- A **renderer** built with **React 18 or 19** (the `<GlitchSpotButton />` component requires React; the rest of the SDK is React-agnostic).
- A renderer that uses `contextIsolation: true` (the modern Electron default).

You have:

- `node` ≥ 18 and `npm` available on the command line.
- Write access to your app's `main.ts` (or `main.js`), `preload.ts`, and at least one React file.

---

## 1. What the SDK gives you

After integration, the host app gains:

1. A **global hotkey** (configurable; omit to disable) that captures the host's main window and shows the annotation overlay.
2. A **programmatic API**: `gs.open()` from main-process code, `glitchSpot.open()` from renderer code.
3. An **optional `<GlitchSpotButton />`** React component — a 40px floating bug-report button that only renders when the SDK is initialised, so it disappears automatically in non-debug builds.
4. A single **`onSnapshot(snapshot)` callback** that fires with a fully-built `GlitchSnapshot` (annotated PNG + clean PNG + annotations + app metadata). Your host app owns delivery (POST to your backend, write to disk, attach to a Linear issue — your call).

**What the SDK does NOT do:**

- It does not persist snapshots, upload them, or open browser windows.
- It does not show its own UI outside the overlay (no toast, no menu items).
- It does not modify your host app's main window beyond reading its bounds and calling `webContents.capturePage()` on it.

---

## 2. Install

```bash
npm install --save @glitch-spot/electron
```

**Expected:** the package and its prebuilt assets land in `node_modules/@glitch-spot/electron/`. Verify:

```bash
ls node_modules/@glitch-spot/electron/dist/
# Expected output includes:
#   main.cjs  main.mjs  main.d.ts
#   renderer.cjs  renderer.mjs  renderer.d.ts
#   preload.cjs
#   overlay-preload.cjs

ls node_modules/@glitch-spot/electron/overlay/
# Expected output:
#   overlay.html  assets/
```

If `overlay/` is missing, the package was not built before publish — see [§10 Troubleshooting](#10-troubleshooting).

---

## 3. Minimum integration (3 steps)

You can integrate with **only the global hotkey** in three steps. No renderer changes required.

### 3.1 — Initialise from your main process

In your **main process entry file** (commonly `electron/main.ts`, `src/main/index.ts`, or `main.js`):

```typescript
import { app, BrowserWindow } from 'electron'
import { GlitchSpot } from '@glitch-spot/electron'

let mainWindow: BrowserWindow

app.whenReady().then(() => {
  mainWindow = new BrowserWindow({
    width: 1200,
    height: 800,
    webPreferences: {
      contextIsolation: true,
      nodeIntegration: false,
      // ...your existing webPreferences
    },
  })

  mainWindow.loadURL('https://your.app/or/file/url')

  // === Glitch Spot integration ===
  GlitchSpot.init(app, mainWindow, {
    hotkey: 'Ctrl+Shift+G',
    onSnapshot: async (snapshot) => {
      // Replace this with your real delivery — POST to backend, attach to Linear, etc.
      console.log('[glitch-spot] snapshot received:', snapshot.id)
      console.log('  annotations:', snapshot.annotations.length)
      console.log('  app:', snapshot.context.app)
    },
  })
})
```

That's it. Re-launch your app and press `Ctrl+Shift+G`. The overlay should appear over your main window with a screenshot of its current contents.

> **Why pass `app` and `mainWindow`?** `app` lets the SDK read your `app.getName()` and `app.getVersion()` (stamped into every snapshot). `mainWindow` is the capture target.

### 3.2 — (Optional) Add the floating button

Skip this if the hotkey is enough. Otherwise, in any renderer file (after Step 3.3 wires up the preload):

```tsx
import { GlitchSpotButton } from '@glitch-spot/electron/renderer'

function App() {
  return (
    <>
      <YourRealApp />
      <GlitchSpotButton position="bottom-right" />
    </>
  )
}
```

`position` accepts: `'bottom-right' | 'bottom-left' | 'top-right' | 'top-left'` (default `'bottom-right'`).

> **The button auto-hides if the preload isn't set up.** This is intentional — leave it in your tree, and it will simply render nothing in production builds where you choose not to wire the preload.

### 3.3 — (Optional) Wire the preload for renderer triggers

Required **only if** you want to use `<GlitchSpotButton />` OR call `glitchSpot.open()` from your renderer.

**Easiest:** point Electron's `webPreferences.preload` at the SDK's bundled preload script:

```typescript
import { createRequire } from 'node:module'
const require = createRequire(import.meta.url)
const glitchSpotPreloadPath = require.resolve('@glitch-spot/electron/preload')

mainWindow = new BrowserWindow({
  // ...
  webPreferences: {
    preload: glitchSpotPreloadPath,
    contextIsolation: true,
    nodeIntegration: false,
  },
})
```

(For CommonJS main processes, drop the `createRequire` dance and use `require.resolve('@glitch-spot/electron/preload')` directly.)

**If your app already has its own preload script** — and most non-trivial apps do — `require` the SDK's preload subpath from inside it. The module self-executes on load, so you do not need to call `glitchSpotPreload()` manually:

```javascript
// your existing preload.cjs (or .js — Electron preload scripts are CommonJS)
const { contextBridge } = require('electron')

// Your existing contextBridge.exposeInMainWorld(...) calls
contextBridge.exposeInMainWorld('myApp', { /* ... */ })

// Add this line — exposes window.glitchSpot automatically
require('@glitch-spot/electron/preload')
```

> **Don't call `glitchSpotPreload()` after `require`'ing the module** — the module already runs it for you. A second call would throw `An object has already been registered with the name 'glitchSpot'`.

After that, `window.glitchSpot.open()` is callable from the renderer, and `<GlitchSpotButton />` will appear.

---

## 4. Full API reference

### `GlitchSpot.init(app, mainWindow, config)`

Main-process factory. Call once after `app.whenReady()`, after creating your main window.

| Parameter    | Type                | Description                                                                |
| ------------ | ------------------- | -------------------------------------------------------------------------- |
| `app`        | `Electron.App`      | The Electron `app` module.                                                 |
| `mainWindow` | `BrowserWindow`     | The window to capture and overlay. Must be created before this call.       |
| `config`     | `GlitchSpotConfig`  | Configuration object — see below.                                          |

**Returns:** `GlitchSpotInstance` — `{ open(): void; stop(): void }`.

**Throws:** `Error` if `config.onSnapshot` is missing.

### `GlitchSpotConfig`

```typescript
interface GlitchSpotConfig {
  /** Global accelerator string. Omit to disable the hotkey entirely. */
  hotkey?: string

  /** Required. Called with the completed snapshot. May be async. Errors are caught and logged. */
  onSnapshot: (snapshot: GlitchSnapshot) => void | Promise<void>

  /** Optional. Called at capture time; return value merged into snapshot.context.app.metadata. */
  metadata?: () => Record<string, unknown>
}
```

**Hotkey format:** Electron's [`globalShortcut`](https://www.electronjs.org/docs/latest/api/accelerator) syntax — e.g. `'Ctrl+Shift+G'`, `'CommandOrControl+Alt+Shift+B'`. On macOS, `'Ctrl'` is the literal Control key; use `'Cmd'` or `'CommandOrControl'` for ⌘.

**`onSnapshot` errors:** thrown errors are caught and logged via `console.error`; they do not crash the host app or block the overlay from hiding.

**`metadata()` is called synchronously at capture time** — keep it cheap. Use it to inject things like user ID, current route, or feature-flag state into every snapshot:

```typescript
metadata: () => ({
  userId: currentSession?.userId,
  route: window.location.pathname,   // ← only valid if called from renderer; for main, pass via IPC
  buildVersion: process.env.BUILD_SHA,
}),
```

### `GlitchSpotInstance`

```typescript
interface GlitchSpotInstance {
  /** Programmatic trigger — same effect as the hotkey. */
  open(): void

  /** Unregister hotkey, remove IPC handlers, destroy overlay window. Call on app shutdown. */
  stop(): void
}
```

Typical shutdown:

```typescript
const gs = GlitchSpot.init(app, mainWindow, { /* ... */ })

app.on('before-quit', () => {
  gs.stop()
})
```

### `GlitchSnapshot` schema

This is what your `onSnapshot` callback receives.

```typescript
interface GlitchSnapshot {
  id: string                 // "snap_<12 hex chars>"
  created_at: string         // ISO 8601 UTC timestamp
  mode: 'frozen'             // v1 supports frozen mode only
  annotations: Annotation[]  // user-drawn rectangles + notes
  context: {
    screen: {
      width: number          // PHYSICAL pixels (already multiplied by scale_factor)
      height: number
      scale_factor: number   // e.g. 2 on a Retina display
    }
    os: {
      platform: string       // 'darwin' on macOS
      version: string        // e.g. '14.5' (macOS only currently)
    }
    app: {
      name: string           // from electron app.getName()
      version: string        // from electron app.getVersion()
      metadata?: Record<string, unknown>  // from config.metadata()
    }
  }
  images: {
    clean_base64: string      // base64 PNG of the captured window (NO data: prefix)
    annotated_base64: string  // base64 PNG with overlaid rectangles + numbered badges
  }
}

interface Annotation {
  id: string                 // unique per snapshot
  shape: 'rect'              // v1 supports rectangles only
  coords: {                  // PHYSICAL pixels — matches clean_base64 resolution
    x: number
    y: number
    w: number
    h: number
  }
  note: string               // user-typed text (may be empty)
  label: string              // auto-derived display label ("01", "02", ..., "100")
}
```

**Decoding a base64 image** to a Node `Buffer`:

```typescript
const cleanPng = Buffer.from(snapshot.images.clean_base64, 'base64')
require('node:fs').writeFileSync('/tmp/clean.png', cleanPng)
```

**Or to a data URL** for use in an `<img src>`:

```typescript
const dataUrl = `data:image/png;base64,${snapshot.images.clean_base64}`
```

### Renderer exports (`@glitch-spot/electron/renderer`)

```typescript
import { GlitchSpotButton, glitchSpot } from '@glitch-spot/electron/renderer'

// Render the floating button anywhere in your React tree:
<GlitchSpotButton position="top-right" />

// Programmatically trigger from any renderer code:
glitchSpot.open()
```

---

## 5. Common integration recipes

### 5.1 — POST snapshots to your backend

```typescript
GlitchSpot.init(app, mainWindow, {
  hotkey: 'CommandOrControl+Shift+G',
  onSnapshot: async (snapshot) => {
    await fetch('https://api.example.com/bug-reports', {
      method: 'POST',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify({
        snapshot,
        reporter: currentUser.email,
      }),
    })
  },
})
```

### 5.2 — Attach to a Linear issue

```typescript
onSnapshot: async (snapshot) => {
  const issue = await linear.issueCreate({
    title: `[Glitch] ${snapshot.context.app.name} — ${snapshot.annotations[0]?.note ?? 'No note'}`,
    description: `${snapshot.annotations.length} annotation(s)\n\n` +
      snapshot.annotations.map((a, i) => `${i + 1}. ${a.note}`).join('\n'),
  })
  await linear.attachmentCreate({
    issueId: issue.id,
    title: 'annotated.png',
    contentType: 'image/png',
    data: snapshot.images.annotated_base64,
  })
}
```

### 5.3 — Write to disk during dev

```typescript
import { writeFile, mkdir } from 'node:fs/promises'
import { join } from 'node:path'

onSnapshot: async (snapshot) => {
  const dir = join(app.getPath('userData'), 'glitch-snapshots', snapshot.id)
  await mkdir(dir, { recursive: true })
  await writeFile(join(dir, 'snapshot.json'), JSON.stringify(snapshot, null, 2))
  await writeFile(join(dir, 'clean.png'),     Buffer.from(snapshot.images.clean_base64, 'base64'))
  await writeFile(join(dir, 'annotated.png'), Buffer.from(snapshot.images.annotated_base64, 'base64'))
  console.log(`Saved to ${dir}`)
}
```

### 5.4 — Trigger from a custom menu item

```typescript
import { Menu } from 'electron'

const gs = GlitchSpot.init(app, mainWindow, { onSnapshot: /* ... */ })

const menu = Menu.buildFromTemplate([
  {
    label: 'Help',
    submenu: [
      { label: 'Report a bug…', accelerator: 'CmdOrCtrl+/', click: () => gs.open() },
    ],
  },
])
Menu.setApplicationMenu(menu)
```

### 5.5 — Inject host-app context with `metadata`

```typescript
GlitchSpot.init(app, mainWindow, {
  onSnapshot: async (snapshot) => {
    console.log(snapshot.context.app.metadata)
    // → { userId: 'usr_42', route: '/dashboard', tenant: 'acme' }
  },
  metadata: () => ({
    userId: currentSession.userId,
    route: lastKnownRoute,
    tenant: currentSession.tenant,
  }),
})

// Have the renderer forward route changes to the main process via IPC,
// store them in `lastKnownRoute`, and they'll appear in every snapshot.
```

---

## 6. Testing the integration

This section is the **agent-friendly verification flow** — a deterministic sequence of steps to confirm the SDK is wired correctly.

### 6.1 — Smoke test (no UI required)

Verify your main-process integration loads and registers the hotkey before you click anything:

```bash
# In your host app's root:
npm run start  # or whatever launches Electron

# In the same terminal, watch for one of these on stdout:
#   (no warning) → hotkey registered successfully
#   [glitch-spot] Could not register hotkey: <accelerator> → another app owns that hotkey
#   [glitch-spot] onSnapshot is required → you forgot to pass onSnapshot
```

**Pass criteria:** the app launches, no `[glitch-spot]` error or warning appears in the main-process log.

### 6.2 — Hotkey path (end-to-end, manual)

This is the canonical "does the whole pipeline work" test.

| Step | Action                                                        | Expected result                                                                                                |
| ---- | ------------------------------------------------------------- | -------------------------------------------------------------------------------------------------------------- |
| 1    | Launch your app with the SDK initialised.                     | App opens normally; no `[glitch-spot]` warnings in console.                                                     |
| 2    | Press the configured hotkey (e.g. `Ctrl+Shift+G`).             | A transparent overlay appears over your main window, showing a screenshot of its current contents.            |
| 3    | Drag-to-draw a rectangle anywhere on the overlay.              | An orange rectangle (`#EE6633`) appears with a small note input next to it.                                    |
| 4    | Type a short note (e.g. "test annotation"). Click outside.    | The note is committed to the rectangle.                                                                        |
| 5    | Press `Cmd+Return` (macOS) or `Ctrl+Enter`.                    | The overlay closes. Your `onSnapshot` callback fires.                                                          |
| 6    | Inspect your callback output.                                 | `snapshot.id` starts with `snap_`, `snapshot.annotations.length === 1`, both `clean_base64` and `annotated_base64` are non-empty strings. |

**A pass means all six steps succeed.** If any step fails, jump to [§10 Troubleshooting](#10-troubleshooting).

### 6.3 — Programmatic trigger path

Add a temporary button or menu item that calls `gs.open()` directly (skipping the hotkey):

```typescript
// In main process, after init:
setTimeout(() => gs.open(), 5000)  // 5 seconds after launch, trigger automatically
```

**Pass criteria:** overlay appears 5 seconds after launch with no key press. Confirms the IPC-free path works.

### 6.4 — Renderer trigger path (requires preload)

Requires §3.3 done. Open DevTools on your main window, switch to the Console, type:

```javascript
window.glitchSpot.open()
```

**Pass criteria:** overlay appears immediately. If `window.glitchSpot is undefined`, the preload is not wired — re-check §3.3.

### 6.5 — `<GlitchSpotButton />` visibility test

With the preload wired and `<GlitchSpotButton position="bottom-right" />` rendered:

| Test            | Expected result                                                                            |
| --------------- | ------------------------------------------------------------------------------------------ |
| Default state   | A small orange circular button with a bug-report icon is pinned to the bottom-right corner. |
| Click it        | Overlay appears immediately.                                                                |
| Remove the `require('@glitch-spot/electron/preload')` line from your preload and reload. | Button is gone (it renders nothing when `window.glitchSpot` is absent). |

### 6.6 — Snapshot shape regression check

A quick script you can run from your `onSnapshot` to assert the schema:

```typescript
onSnapshot: (snapshot) => {
  const errors: string[] = []
  if (!/^snap_[0-9a-f]{12}$/.test(snapshot.id))                errors.push(`bad id: ${snapshot.id}`)
  if (snapshot.mode !== 'frozen')                              errors.push(`bad mode: ${snapshot.mode}`)
  if (!Array.isArray(snapshot.annotations))                    errors.push('annotations not an array')
  if (typeof snapshot.images.clean_base64 !== 'string')        errors.push('missing clean_base64')
  if (typeof snapshot.images.annotated_base64 !== 'string')    errors.push('missing annotated_base64')
  if (snapshot.images.clean_base64.startsWith('data:'))        errors.push('clean_base64 should be raw base64, no data: prefix')
  if (!snapshot.context.app.name)                              errors.push('missing context.app.name')
  if (typeof snapshot.context.screen.scale_factor !== 'number') errors.push('missing scale_factor')

  if (errors.length) {
    console.error('[glitch-spot integration test] FAIL', errors)
  } else {
    console.log('[glitch-spot integration test] PASS', snapshot.id)
  }
},
```

**Pass criteria:** `[glitch-spot integration test] PASS snap_xxxxxxxxxxxx` appears in the console after every save.

### 6.7 — Coordinate-system sanity check

Decode `clean_base64` to a file and confirm an annotation's coordinates land on the right pixel:

```typescript
onSnapshot: async (snapshot) => {
  const fs = await import('node:fs/promises')
  await fs.writeFile('/tmp/clean.png', Buffer.from(snapshot.images.clean_base64, 'base64'))
  await fs.writeFile('/tmp/annotated.png', Buffer.from(snapshot.images.annotated_base64, 'base64'))
  console.log('annotation[0] coords:', snapshot.annotations[0]?.coords)
  console.log('image dimensions: width', snapshot.context.screen.width, 'height', snapshot.context.screen.height)
  console.log('open /tmp/clean.png and /tmp/annotated.png to compare')
}
```

**Pass criteria:** When you open both PNGs at full size, the orange rectangle in `annotated.png` falls on top of the same UI element that was inside your drag on screen. Coordinates and image dimensions are in **physical pixels** — on a Retina display, a 1440×900 logical window produces 2880×1800 PNGs.

### 6.8 — Cleanup check

Call `gs.stop()` and verify everything is unwound:

```typescript
const gs = GlitchSpot.init(/* ... */)

setTimeout(() => {
  gs.stop()
  console.log('stopped — hotkey should now do nothing')
}, 10000)
```

**Pass criteria:** after 10s, pressing the hotkey does nothing. No `[glitch-spot]` error appears. App can quit cleanly.

---

## 7. What the SDK changes about your app

So you know what to expect when reviewing a PR that adds Glitch Spot:

| Surface area               | What changes                                                                                       |
| -------------------------- | -------------------------------------------------------------------------------------------------- |
| **Main process**           | One `BrowserWindow` is created (the overlay, hidden until triggered). Three `ipcMain` handlers registered: `glitch-spot:trigger`, `glitch-spot:snapshot`, `glitch-spot:cancel`. One global shortcut registered (if `hotkey` is set). Two listeners on your main window: `'resize'` and `'move'`. |
| **Renderer (your window)** | None, unless you opt into the preload. With the preload: `window.glitchSpot` is added. With `<GlitchSpotButton />`: one extra fixed-position 40×40 `<button>` at z-index 999999. |
| **Disk I/O**               | None. The SDK never reads or writes files at runtime. Snapshots are delivered in-memory; persistence is up to your `onSnapshot`. |
| **Network**                | None. The SDK never makes network calls.                                                            |
| **Permissions prompts**    | None. `webContents.capturePage()` captures only renderer content and does not require Screen Recording permission on macOS. |

---

## 8. Security considerations

- **IPC sender validation:** the `glitch-spot:snapshot` handler verifies `event.sender === overlay.webContents` and rejects messages from any other frame. A compromised renderer cannot fabricate snapshots.
- **`contextIsolation: true`** is required on both your main window (if you use the preload) and is hard-coded on the overlay window. `nodeIntegration: false` is hard-coded on the overlay.
- **No outbound network or disk access from the SDK** — see §7.
- **Annotation notes are user-typed text and are passed through verbatim.** If you embed them in HTML, sanitise on the receiving side.
- **PNG payloads come from `webContents.capturePage()`** of your own main window — they can contain whatever is on screen, including secrets the user has open. Treat snapshots as sensitive; never log entire snapshots to an unsecured channel.

---

## 9. Platform support

**v1 is macOS only.** Windows and Linux will work for `webContents.capturePage()` (it's cross-platform), but the SDK has not been smoke-tested on either, and `process.getSystemVersion()` returns an empty string on non-macOS platforms (snapshot.context.os.version will be `''`).

**Electron versions:** `>=28` declared as peer dep. The SDK uses only stable Electron APIs (`BrowserWindow`, `ipcMain`, `globalShortcut`, `screen`, `webContents.capturePage`) — newer versions should work without changes.

**React versions:** `>=18` for `<GlitchSpotButton />`. The component uses `useState` + `useEffect`, no concurrent-rendering primitives.

---

## 10. Troubleshooting

| Symptom                                                                                  | Cause / fix                                                                                                                                                  |
| ---------------------------------------------------------------------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------ |
| `[glitch-spot] Could not register hotkey: <accelerator>`                                  | Another app owns that hotkey. Pick a different one in `config.hotkey`.                                                                                       |
| `[glitch-spot] onSnapshot is required`                                                   | You called `GlitchSpot.init(...)` without an `onSnapshot` function. Add one.                                                                                 |
| Overlay shows a blank window with no screenshot.                                          | `overlay.html` is loading but assets aren't. Verify `node_modules/@glitch-spot/electron/overlay/assets/` is present. Re-install if missing.                  |
| `window.glitchSpot is undefined` in the renderer.                                         | Preload not wired. Re-check §3.3 — either point `webPreferences.preload` at `require.resolve('@glitch-spot/electron/preload')`, or `require('@glitch-spot/electron/preload')` from your own preload. |
| `<GlitchSpotButton />` never appears.                                                    | Same as above — the button hides itself when `window.glitchSpot` is absent. Wire the preload.                                                                |
| `onSnapshot` never fires.                                                                | You closed the overlay without pressing `Cmd+Return`. Esc / clicking outside discards, save requires `Cmd+Return` (or `Ctrl+Enter` on non-mac).             |
| Annotation rectangles in `annotated.png` are positioned wrong.                            | Coordinates are in **physical** pixels and `screen.width` / `screen.height` are too. If you draw onto a CSS-pixel canvas downstream, divide by `scale_factor`. |
| `Rejected snapshot IPC from unexpected sender` warning in main process.                  | A non-overlay frame is sending `glitch-spot:snapshot`. This is the security guard doing its job — investigate why.                                           |
| App slow to quit / hangs on shutdown.                                                    | You're not calling `gs.stop()` on `before-quit`. Add it (see §4 `GlitchSpotInstance`).                                                                       |
| `Error: spawn EBADF` or similar when launching after install.                            | The native Electron version was rebuilt for a different ABI. Run `npx electron-rebuild` or your equivalent.                                                  |

---

## 11. Uninstall

```typescript
// 1. Remove the integration code from your main process.
// 2. Remove <GlitchSpotButton /> and any glitchSpot.open() calls from renderer code.
// 3. Remove require('@glitch-spot/electron/preload') from your preload (if present).
// 4. Uninstall the package:
```

```bash
npm uninstall @glitch-spot/electron
```

There is no persisted state for the SDK to clean up — no files, no settings, no registered handlers outlive the process.

---

## Appendix A — Minimal complete example

A complete `main.ts` that integrates the SDK with all three trigger paths:

```typescript
import { app, BrowserWindow } from 'electron'
import { createRequire } from 'node:module'
import { GlitchSpot } from '@glitch-spot/electron'

const require = createRequire(import.meta.url)

let mainWindow: BrowserWindow
let gs: ReturnType<typeof GlitchSpot.init>

app.whenReady().then(() => {
  mainWindow = new BrowserWindow({
    width: 1200,
    height: 800,
    webPreferences: {
      preload: require.resolve('@glitch-spot/electron/preload'),
      contextIsolation: true,
      nodeIntegration: false,
    },
  })

  mainWindow.loadURL('https://example.com')

  gs = GlitchSpot.init(app, mainWindow, {
    hotkey: 'CommandOrControl+Shift+G',
    metadata: () => ({ env: process.env.NODE_ENV ?? 'production' }),
    onSnapshot: async (snapshot) => {
      console.log('Got snapshot', snapshot.id, 'with', snapshot.annotations.length, 'annotation(s)')
      // your delivery here
    },
  })
})

app.on('before-quit', () => {
  gs?.stop()
})

app.on('window-all-closed', () => {
  if (process.platform !== 'darwin') app.quit()
})
```

The matching renderer:

```tsx
import { GlitchSpotButton } from '@glitch-spot/electron/renderer'

export function App() {
  return (
    <>
      <YourRealUI />
      <GlitchSpotButton position="bottom-right" />
    </>
  )
}
```

That's the whole integration.
