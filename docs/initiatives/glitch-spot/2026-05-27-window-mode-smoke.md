# Window-mode smoke checklist

Captured from Task 24 of `docs/plans/2026-05-27-per-window-selection.md`.

## Single display

- [ ] ⌃⌥⌘J shows the chromatic crosshair cursor
- [ ] Hovering different windows updates the cyan highlight
- [ ] Esc dismisses the picker
- [ ] Click empty desktop dismisses the picker
- [ ] Click on a window enters annotation mode with dim veil + checker over the window
- [ ] Cannot draw rectangle outside the window
- [ ] Cmd+Enter saves; JSON in clipboard has `mode: "window"` + `context.window`
- [ ] Annotated PNG is window-sized and rectangle is in the correct position

## Multi display

- [ ] Highlight follows cursor across display boundaries
- [ ] Can pick a window on the non-primary display
- [ ] Annotation overlay positions correctly on the secondary display

## Error paths

- [ ] Close selected window mid-annotation → notification fires, no snapshot written
- [ ] Minimise selected window → same path
- [ ] Resize selected window before save → save still produces a coherent PNG; rect clamped if needed

## Permission / startup

- [ ] First launch shows the macOS Screen Recording prompt (if not already granted)
- [ ] After granting + relaunch, ⌃⌥⌘J works
