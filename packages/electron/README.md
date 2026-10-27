# @glitch-spot/electron

Embed the Glitch Spot annotation overlay into any Electron + TypeScript + React application. Press a hotkey, drag rectangles, type notes — your callback receives a structured `GlitchSnapshot`.

```bash
npm install --save @glitch-spot/electron
```

```typescript
import { app, BrowserWindow } from 'electron'
import { GlitchSpot } from '@glitch-spot/electron'

app.whenReady().then(() => {
  const mainWindow = new BrowserWindow({ /* ... */ })
  mainWindow.loadURL('https://your.app')

  GlitchSpot.init(app, mainWindow, {
    hotkey: 'CommandOrControl+Shift+G',
    onSnapshot: async (snapshot) => {
      // Your delivery: POST to backend, attach to Linear, write to disk, etc.
      console.log('Got snapshot', snapshot.id)
    },
  })
})
```

## Documentation

- **[INTEGRATION.md](./INTEGRATION.md)** — Full integration guide with step-by-step verification, API reference, recipes, and troubleshooting.

## Requirements

- Electron ≥ 28
- React ≥ 18 (only for `<GlitchSpotButton />`)
- macOS (v1 is macOS-only; Windows/Linux untested)

## License

MIT
