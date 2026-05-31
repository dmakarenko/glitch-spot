# Visible-region highlights — manual smoke

> Confirm the macOS Screenshot.app behaviour landed end-to-end. Run after the
> implementation tasks complete. Each row is one observation; tick when the
> behaviour matches the **expected** column.

## Setup

1. Quit the app if it's running.
2. Start the dev build:
   ```
   PATH="$HOME/.cargo/bin:$PATH" npm run tauri -- dev
   ```
3. Open at least three overlapping windows from different apps (e.g. Safari,
   Notes, Finder). Arrange them so each one is partly covered by another.

## Checks

| # | Action | Expected | Pass? |
|---|---|---|---|
| 1 | Press ⌃⌥⌘J. | Pickers appear on every display; the cyan cursor is visible. | |
| 2 | Hover a window that is fully visible (no occlusion). | Cyan checker covers its full rectangle. | |
| 3 | Hover a window whose **left half** is covered by another window. | Cyan checker only covers the visible right half; the covering window remains unobscured. | |
| 4 | Hover a window whose **top-left corner** is covered by another. | Cyan checker forms an L-shape that wraps the corner. | |
| 5 | Move the cursor across the boundary from covered to visible portion of the back window. | Selection switches between the covering window and the back window as expected — the back window is only highlighted when the cursor is over its visible region. | |
| 6 | Bring a small window fully behind a larger one so it is completely hidden. Re-press ⌃⌥⌘J. | The fully-hidden window is not selectable from anywhere on screen — moving the cursor over its bounds selects whatever is on top. | |
| 7 | Click an L-shaped highlight. | The expected window is selected and the overlay enters annotation mode. | |
| 8 | Press Esc during the picker. | The picker dismisses cleanly; pressing ⌃⌥⌘J again opens a fresh picker. | |
| 9 | Hover the same window on both displays in a multi-monitor setup. | Highlight clipping behaves identically on each display. | |

If any row fails, file the failure mode under a "Findings" section below before
fixing.
