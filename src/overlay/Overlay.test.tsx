// ABOUTME: Rendering tests for the overlay component's window mode.
// ABOUTME: Verifies the dim veil spotlights the selected window region.

// @vitest-environment jsdom

import { describe, expect, it, vi, afterEach } from "vitest";
import { render, act, cleanup } from "@testing-library/react";
import { Overlay } from "./Overlay";
import type { WindowModePayload } from "../picker/types";

vi.mock("@tauri-apps/api/core", () => ({
  invoke: () => Promise.resolve(null),
}));
vi.mock("@tauri-apps/api/event", () => ({
  listen: () => Promise.resolve(() => {}),
}));

function makeWindowPayload(): WindowModePayload {
  return {
    app_name: "Safari",
    title: "Reader",
    window_id: 7,
    window_rect_css: { x: 120, y: 80, w: 400, h: 300 },
    window_rect_physical: { x: 240, y: 160, w: 800, h: 600 },
    display_scale_factor: 2,
  };
}

function presentWindowMode(payload: WindowModePayload) {
  act(() => {
    (
      window as unknown as {
        __glitchSpotPresent: (mode: string, payload?: WindowModePayload) => void;
      }
    ).__glitchSpotPresent("window", payload);
  });
}

describe("<Overlay/> window mode", () => {
  afterEach(() => {
    cleanup();
  });

  it("dims everything except the selected window region", () => {
    const { getByTestId } = render(<Overlay />);
    const payload = makeWindowPayload();
    presentWindowMode(payload);

    const spotlight = getByTestId("window-spotlight");
    const { window_rect_css } = payload;
    expect(spotlight.style.left).toBe(`${window_rect_css.x}px`);
    expect(spotlight.style.top).toBe(`${window_rect_css.y}px`);
    expect(spotlight.style.width).toBe(`${window_rect_css.w}px`);
    expect(spotlight.style.height).toBe(`${window_rect_css.h}px`);
    // The dim lives in the box-shadow spread, leaving the window rect clear.
    expect(spotlight.style.boxShadow).not.toBe("");
    // Must not capture clicks — outside-click-to-cancel relies on the
    // full-screen DrawingLayer underneath catching them.
    expect(spotlight.style.pointerEvents).toBe("none");
  });

  it("renders a clipped frozen-frame box at the window rect and no checker", () => {
    const { getByTestId, container } = render(<Overlay />);
    const payload = makeWindowPayload();
    presentWindowMode(payload);

    const frame = getByTestId("window-frozen-frame");
    const { window_rect_css } = payload;
    expect(frame.style.left).toBe(`${window_rect_css.x}px`);
    expect(frame.style.top).toBe(`${window_rect_css.y}px`);
    expect(frame.style.width).toBe(`${window_rect_css.w}px`);
    expect(frame.style.height).toBe(`${window_rect_css.h}px`);
    // The chromatic shake translates the channel layers ~18px; the box must clip
    // them so no edge gap shows mid-animation.
    expect(frame.style.overflow).toBe("hidden");
    // The gray-white checker placeholder is replaced by the frozen capture.
    expect(container.innerHTML).not.toContain("%23999999");
  });
});
