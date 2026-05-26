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

## Plans

Plans live in `docs/plans/`. Completed plans move to `docs/plans/completed/`.
