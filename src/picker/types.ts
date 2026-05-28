// ABOUTME: TS mirrors of the Rust picker payloads sent over Tauri eval / events.
// ABOUTME: Field names match Rust serde (snake_case) — keep in sync with window_picker/mod.rs.

export type WindowBounds = { x: number; y: number; w: number; h: number };

export type PickableWindow = {
  window_id: number;
  app_name: string;
  title: string;
  bounds: WindowBounds;
  display_index: number;
  layer: number;
};

export type DisplayBounds = {
  display_index: number;
  x: number;
  y: number;
  w: number;
  h: number;
  scale_factor: number;
};

export type PickerStartPayload = {
  windows: PickableWindow[];
  displays: DisplayBounds[];
  // Global physical-px origin of the single picker window. Cursor px inside
  // the picker map back to global physical px by `picker_origin + clientX * picker_scale`.
  picker_origin: { x: number; y: number };
  // Scale factor used to convert CSS px (picker window) ↔ global physical px.
  // Assumes uniform display scale across all displays.
  picker_scale: number;
};

export type WindowModePayload = {
  app_name: string;
  title: string;
  window_id: number;
  window_rect_css: { x: number; y: number; w: number; h: number };
  window_rect_physical: { x: number; y: number; w: number; h: number };
  display_scale_factor: number;
};
