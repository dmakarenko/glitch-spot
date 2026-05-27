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
  my_display_index: number;
};

export type WindowModePayload = {
  app_name: string;
  title: string;
  window_id: number;
  window_rect_css: { x: number; y: number; w: number; h: number };
  window_rect_physical: { x: number; y: number; w: number; h: number };
  display_scale_factor: number;
};
