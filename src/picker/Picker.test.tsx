// ABOUTME: Interaction tests for the picker overlay component.
// ABOUTME: Stubs the Tauri invoke + event listen, drives session start, hover
// ABOUTME: events from the simulated Rust poller, and keyboard events.

// @vitest-environment jsdom

import { describe, expect, it, vi, beforeEach, afterEach } from "vitest";
import { render, fireEvent, act, cleanup } from "@testing-library/react";
import { Picker } from "./Picker";
import type { PickerStartPayload } from "./types";

const invokeMock = vi.fn();
vi.mock("@tauri-apps/api/core", () => ({
  invoke: (cmd: string, args: unknown) => invokeMock(cmd, args),
}));

// Capture the `picker://hover` listener so tests can drive hover state by
// simulating events from the Rust-side cursor poller.
let hoverListener:
  | ((event: { payload: { window_id: number | null } }) => void)
  | undefined;
vi.mock("@tauri-apps/api/event", () => ({
  listen: (
    eventName: string,
    handler: (event: { payload: { window_id: number | null } }) => void,
  ) => {
    if (eventName === "picker://hover") {
      hoverListener = handler;
    }
    return Promise.resolve(() => {
      if (eventName === "picker://hover") hoverListener = undefined;
    });
  },
}));

function emitHover(window_id: number | null) {
  if (!hoverListener) throw new Error("picker://hover listener not registered");
  act(() => hoverListener!({ payload: { window_id } }));
}

function makeSession(): PickerStartPayload {
  return {
    picker_origin: { x: 0, y: 0 },
    picker_scale: 1,
    displays: [
      { display_index: 0, x: 0, y: 0, w: 1440, h: 900, scale_factor: 1 },
    ],
    windows: [
      {
        window_id: 7,
        app_name: "Safari",
        title: "Reader",
        bounds: { x: 100, y: 100, w: 400, h: 300 },
        display_index: 0,
        layer: 0,
      },
    ],
  };
}

describe("<Picker/>", () => {
  beforeEach(() => {
    invokeMock.mockReset();
  });

  afterEach(() => {
    cleanup();
  });

  it("renders nothing before the session payload arrives", () => {
    const { queryByTestId } = render(<Picker />);
    expect(queryByTestId("picker-root")).toBeNull();
  });

  it("renders the picker root once __glitchSpotPickerStart is invoked", () => {
    const { getByTestId } = render(<Picker />);
    act(() => {
      (window as unknown as { __glitchSpotPickerStart: (p: PickerStartPayload) => void })
        .__glitchSpotPickerStart(makeSession());
    });
    expect(getByTestId("picker-root")).toBeTruthy();
  });

  it("highlights the window when the Rust poller broadcasts a hit", async () => {
    const { getByTestId, queryByTestId } = render(<Picker />);
    act(() => {
      (window as unknown as { __glitchSpotPickerStart: (p: PickerStartPayload) => void })
        .__glitchSpotPickerStart(makeSession());
    });
    // The session-active useEffect registers the listener asynchronously;
    // flush the microtask so `emitHover` finds it.
    await Promise.resolve();
    emitHover(7);
    expect(queryByTestId("picker-highlight")).toBeTruthy();
  });

  it("clears the highlight when the poller broadcasts a miss", async () => {
    const { getByTestId, queryByTestId } = render(<Picker />);
    act(() => {
      (window as unknown as { __glitchSpotPickerStart: (p: PickerStartPayload) => void })
        .__glitchSpotPickerStart(makeSession());
    });
    await Promise.resolve();
    emitHover(7);
    emitHover(null);
    expect(queryByTestId("picker-highlight")).toBeNull();
  });

  it("calls picker_select when clicking on a hovered window", async () => {
    const { getByTestId } = render(<Picker />);
    act(() => {
      (window as unknown as { __glitchSpotPickerStart: (p: PickerStartPayload) => void })
        .__glitchSpotPickerStart(makeSession());
    });
    await Promise.resolve();
    emitHover(7);
    fireEvent.click(getByTestId("picker-root"));
    expect(invokeMock).toHaveBeenCalledWith("picker_select", { windowId: 7 });
  });

  it("calls picker_cancel when clicking on empty desktop", async () => {
    const { getByTestId } = render(<Picker />);
    act(() => {
      (window as unknown as { __glitchSpotPickerStart: (p: PickerStartPayload) => void })
        .__glitchSpotPickerStart(makeSession());
    });
    await Promise.resolve();
    emitHover(null);
    fireEvent.click(getByTestId("picker-root"));
    expect(invokeMock).toHaveBeenCalledWith("picker_cancel", undefined);
  });

  it("calls picker_cancel on Escape", () => {
    render(<Picker />);
    act(() => {
      (window as unknown as { __glitchSpotPickerStart: (p: PickerStartPayload) => void })
        .__glitchSpotPickerStart(makeSession());
    });
    fireEvent.keyDown(window, { key: "Escape" });
    expect(invokeMock).toHaveBeenCalledWith("picker_cancel", undefined);
  });
});
