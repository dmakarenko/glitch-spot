# Glitch Spot — Project Guidelines

## UI Design

All visual decisions for this project are governed by **[DESIGN.md](./DESIGN.md)**.

Read it before touching any component. Key points:

- Dark mode only — `#0c0c0c` background, `#e0e0e0` text
- Accent colour is `#ee6633` (orange-red)
- The chromatic aberration split (`-2px red / +2px cyan box-shadow`) goes on **container frames only** — annotation rectangles, the toolbar pill, and the discard modal
- Buttons and interactive controls are **always crisp** — no split effects on anything the user clicks or types into
- Use a `::before` pseudo-element wrapper to apply the split so it sits outside the element's layout

## Stack

- Tauri v2 (Rust backend + WebView frontend)
- React + TypeScript (Vite)
- All UI is inline styles — no CSS framework, no CSS modules
- Tests: Vitest

## Running the app

```bash
PATH="$HOME/.cargo/bin:$PATH" npm run tauri:dev
```

`cargo` must be on the path explicitly — npm scripts don't inherit the shell's `~/.cargo/bin`. The command starts the Vite dev server and the Tauri/Rust backend together; the app appears in the macOS menu bar.

To test the **unhealthy/broken animation**, hold port 7820 before launching so MCP can't bind:

```bash
python3 -c "import socket,time; s=socket.socket(); s.bind(('127.0.0.1',7820)); s.listen(1); time.sleep(300)" &
PATH="$HOME/.cargo/bin:$PATH" npm run tauri:dev
```

Kill the background `python3` process when done — the icon recovers to cyan within ~1s.

## Plans

Plans live in `docs/plans/`. Completed plans move to `docs/plans/completed/`.
