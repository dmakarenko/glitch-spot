// ABOUTME: Interaction tests for the picker overlay component.
// ABOUTME: Stubs the Tauri invoke, drives session start + mouse + keyboard events.

// @vitest-environment jsdom

import { describe, expect, it, vi, beforeEach, afterEach } from "vitest";
import { render, fireEvent, act, cleanup } from "@testing-library/react";
import { Picker } from "./Picker";
import type { PickerStartPayload } from "./types";

const invokeMock = vi.fn();
vi.mock("@tauri-apps/api/core", () => ({
  invoke: (cmd: string, args: unknown) => invokeMock(cmd, args),
}));

function makeSession(): PickerStartPayload {
  return {
    my_display_index: 0,
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

  it("highlights the window when cursor moves inside its bounds", () => {
    const { getByTestId, queryByTestId } = render(<Picker />);
    act(() => {
      (window as unknown as { __glitchSpotPickerStart: (p: PickerStartPayload) => void })
        .__glitchSpotPickerStart(makeSession());
    });
    fireEvent.mouseMove(getByTestId("picker-root"), { clientX: 200, clientY: 200 });
    expect(queryByTestId("picker-highlight")).toBeTruthy();
  });

  it("clears the highlight on mouseLeave", () => {
    const { getByTestId, queryByTestId } = render(<Picker />);
    act(() => {
      (window as unknown as { __glitchSpotPickerStart: (p: PickerStartPayload) => void })
        .__glitchSpotPickerStart(makeSession());
    });
    fireEvent.mouseMove(getByTestId("picker-root"), { clientX: 200, clientY: 200 });
    fireEvent.mouseLeave(getByTestId("picker-root"));
    expect(queryByTestId("picker-highlight")).toBeNull();
  });

  it("calls picker_select when clicking on a window", () => {
    const { getByTestId } = render(<Picker />);
    act(() => {
      (window as unknown as { __glitchSpotPickerStart: (p: PickerStartPayload) => void })
        .__glitchSpotPickerStart(makeSession());
    });
    fireEvent.mouseMove(getByTestId("picker-root"), { clientX: 200, clientY: 200 });
    fireEvent.click(getByTestId("picker-root"));
    expect(invokeMock).toHaveBeenCalledWith("picker_select", { windowId: 7 });
  });

  it("calls picker_cancel when clicking on empty desktop", () => {
    const { getByTestId } = render(<Picker />);
    act(() => {
      (window as unknown as { __glitchSpotPickerStart: (p: PickerStartPayload) => void })
        .__glitchSpotPickerStart(makeSession());
    });
    fireEvent.mouseMove(getByTestId("picker-root"), { clientX: 1000, clientY: 800 });
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
