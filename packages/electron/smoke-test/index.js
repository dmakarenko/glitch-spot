// ABOUTME: Headless smoke test for the @glitch-spot/electron SDK.
// ABOUTME: Run manually with: npx electron smoke-test/index.js (from packages/electron/ after npm run build)
//
// Usage: cd packages/electron && npx electron smoke-test/index.js
const { app, BrowserWindow } = require('electron')
const path = require('path')

app.whenReady().then(async () => {
  const win = new BrowserWindow({
    width: 1200,
    height: 800,
    webPreferences: { nodeIntegration: false, contextIsolation: true },
  })

  await win.loadURL('data:text/html,<h1>Glitch Spot Smoke Test</h1><p>Press Ctrl+Shift+G</p>')

  // Require the built SDK (CJS)
  const { GlitchSpot } = require('./dist/main.cjs')

  const gs = GlitchSpot.init(app, win, {
    hotkey: 'Ctrl+Shift+G',
    onSnapshot: (snap) => {
      console.log('[smoke-test] snapshot received:', snap.id)
      console.log('  annotations:', snap.annotations.length)
      console.log('  clean_base64 length:', snap.images.clean_base64.length)
      console.log('  annotated_base64 length:', snap.images.annotated_base64.length)
      console.log('  app context:', snap.context.app)
      console.log('[smoke-test] PASS')
      app.quit()
    },
  })

  console.log('[smoke-test] SDK initialised. Press Ctrl+Shift+G, draw a rectangle, press Cmd+Return.')
  console.log('[smoke-test] gs.open() is:', typeof gs.open)
})
