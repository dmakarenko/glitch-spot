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
        visible_regions: [{ x: 100, y: 100, w: 400, h: 300 }],
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

  it("reveals its window after the session paints so re-show never flashes the previous display's stale checker", () => {
    // The picker pool windows are reused across displays and shown at alpha 0
    // (rendering but invisible) so each repaints the new session's blank/hover
    // layout before becoming visible. The picker signals once that frame has
    // painted; the backend then fades the calling window in to alpha 1.
    const raf = vi
      .spyOn(window, "requestAnimationFrame")
      .mockImplementation((cb: FrameRequestCallback) => {
        cb(0);
        return 0;
      });
    try {
      render(<Picker />);
      act(() => {
        (window as unknown as { __glitchSpotPickerStart: (p: PickerStartPayload) => void })
          .__glitchSpotPickerStart(makeSession());
      });
      expect(invokeMock).toHaveBeenCalledWith("reveal_window", undefined);
    } finally {
      raf.mockRestore();
    }
  });

  it("highlights the window when the Rust poller broadcasts a hit", async () => {
    const { queryAllByTestId } = render(<Picker />);
    act(() => {
      (window as unknown as { __glitchSpotPickerStart: (p: PickerStartPayload) => void })
        .__glitchSpotPickerStart(makeSession());
    });
    // The session-active useEffect registers the listener asynchronously;
    // flush the microtask so `emitHover` finds it.
    await Promise.resolve();
    emitHover(7);
    expect(queryAllByTestId("picker-highlight").length).toBe(1);
  });

  it("clears the highlight when the poller broadcasts a miss", async () => {
    const { queryAllByTestId } = render(<Picker />);
    act(() => {
      (window as unknown as { __glitchSpotPickerStart: (p: PickerStartPayload) => void })
        .__glitchSpotPickerStart(makeSession());
    });
    await Promise.resolve();
    emitHover(7);
    emitHover(null);
    expect(queryAllByTestId("picker-highlight").length).toBe(0);
  });

  it("does not call picker_select on mousedown — waits for mouseup", async () => {
    const { getByTestId } = render(<Picker />);
    act(() => {
      (window as unknown as { __glitchSpotPickerStart: (p: PickerStartPayload) => void })
        .__glitchSpotPickerStart(makeSession());
    });
    await Promise.resolve();
    emitHover(7);
    fireEvent.mouseDown(getByTestId("picker-root"), { clientX: 100, clientY: 100, button: 0 });
    expect(invokeMock).not.toHaveBeenCalledWith("picker_select", expect.anything());
  });

  it("calls picker_select on mouseup with initial_rect when drag is large enough", async () => {
    const { getByTestId } = render(<Picker />);
    act(() => {
      (window as unknown as { __glitchSpotPickerStart: (p: PickerStartPayload) => void })
        .__glitchSpotPickerStart(makeSession());
    });
    await Promise.resolve();
    emitHover(7);
    const root = getByTestId("picker-root");
    // picker_origin=(0,0), picker_scale=1 → physical px = CSS px
    fireEvent.mouseDown(root, { clientX: 100, clientY: 100, button: 0 });
    fireEvent.mouseMove(root, { clientX: 200, clientY: 200, buttons: 1 });
    fireEvent.mouseUp(root);
    expect(invokeMock).toHaveBeenCalledWith("picker_select", {
      windowId: 7,
      initialRectX: 100,
      initialRectY: 100,
      initialRectW: 100,
      initialRectH: 100,
    });
  });

  it("calls picker_select on mouseup without initial_rect when drag is too small", async () => {
    const { getByTestId } = render(<Picker />);
    act(() => {
      (window as unknown as { __glitchSpotPickerStart: (p: PickerStartPayload) => void })
        .__glitchSpotPickerStart(makeSession());
    });
    await Promise.resolve();
    emitHover(7);
    const root = getByTestId("picker-root");
    fireEvent.mouseDown(root, { clientX: 100, clientY: 100, button: 0 });
    fireEvent.mouseUp(root); // no movement — zero-size rect
    expect(invokeMock).toHaveBeenCalledWith("picker_select", { windowId: 7 });
  });

  it("converts initial_rect to global physical pixels using picker_origin and picker_scale", async () => {
    // Use a window whose bounds are fully inside the picker's CSS viewport so
    // clamping doesn't change the rect and the conversion math can be checked cleanly.
    // picker_origin={100,200}, picker_scale=2 → picker CSS origin = global (100,200).
    // Window at global {x:200,y:400,w:600,h:400} → CSS {x:50,y:100,w:300,h:200}.
    const scaledSession: PickerStartPayload = {
      picker_origin: { x: 100, y: 200 },
      picker_scale: 2,
      displays: [{ display_index: 0, x: 100, y: 200, w: 1440, h: 900, scale_factor: 2 }],
      windows: [{
        window_id: 7, app_name: "Safari", title: "Reader",
        bounds: { x: 200, y: 400, w: 600, h: 400 },
        display_index: 0, layer: 0,
        visible_regions: [{ x: 200, y: 400, w: 600, h: 400 }],
      }],
    };
    const { getByTestId } = render(<Picker />);
    act(() => {
      (window as unknown as { __glitchSpotPickerStart: (p: PickerStartPayload) => void })
        .__glitchSpotPickerStart(scaledSession);
    });
    await Promise.resolve();
    emitHover(7);
    const root = getByTestId("picker-root");
    // Drag from CSS (100,150) to (200,250) — fully inside the window CSS bounds (50–350, 100–300)
    fireEvent.mouseDown(root, { clientX: 100, clientY: 150, button: 0 });
    fireEvent.mouseMove(root, { clientX: 200, clientY: 250, buttons: 1 });
    fireEvent.mouseUp(root);
    // rect CSS: x=100,y=150,w=100,h=100 → physical: x=100+100*2=300, y=200+150*2=500, w=200, h=200
    expect(invokeMock).toHaveBeenCalledWith("picker_select", {
      windowId: 7,
      initialRectX: 300,
      initialRectY: 500,
      initialRectW: 200,
      initialRectH: 200,
    });
  });

  it("calls picker_cancel on mousedown on empty desktop", async () => {
    const { getByTestId } = render(<Picker />);
    act(() => {
      (window as unknown as { __glitchSpotPickerStart: (p: PickerStartPayload) => void })
        .__glitchSpotPickerStart(makeSession());
    });
    await Promise.resolve();
    emitHover(null);
    fireEvent.mouseDown(getByTestId("picker-root"), { button: 0 });
    expect(invokeMock).toHaveBeenCalledWith("picker_cancel", undefined);
  });

  it("commits via a document-level mouseup (drag released outside the picker div)", async () => {
    const { getByTestId } = render(<Picker />);
    act(() => {
      (window as unknown as { __glitchSpotPickerStart: (p: PickerStartPayload) => void })
        .__glitchSpotPickerStart(makeSession());
    });
    await Promise.resolve();
    emitHover(7);
    const root = getByTestId("picker-root");
    fireEvent.mouseDown(root, { clientX: 150, clientY: 150, button: 0 });
    fireEvent.mouseMove(root, { clientX: 300, clientY: 300, buttons: 1 });
    // Simulate button released outside the div — dispatched on document directly.
    act(() => { document.dispatchEvent(new MouseEvent("mouseup", { bubbles: true })); });
    expect(invokeMock).toHaveBeenCalledWith("picker_select", expect.objectContaining({ windowId: 7 }));
  });

  it("clamps the committed rect to the selected window bounds", async () => {
    // makeSession window: bounds {x:100, y:100, w:400, h:300} → CSS {x:100,y:100,w:400,h:300}
    const { getByTestId } = render(<Picker />);
    act(() => {
      (window as unknown as { __glitchSpotPickerStart: (p: PickerStartPayload) => void })
        .__glitchSpotPickerStart(makeSession());
    });
    await Promise.resolve();
    emitHover(7);
    const root = getByTestId("picker-root");
    // Drag from inside the window out beyond its right/bottom edges
    fireEvent.mouseDown(root, { clientX: 150, clientY: 150, button: 0 });
    fireEvent.mouseMove(root, { clientX: 800, clientY: 800, buttons: 1 });
    fireEvent.mouseUp(root);
    // Raw rect: x=150,y=150,w=650,h=650. Clamped to window (100–500 x, 100–400 y):
    //   x=150, y=150, w=500-150=350, h=400-150=250
    expect(invokeMock).toHaveBeenCalledWith("picker_select", {
      windowId: 7,
      initialRectX: 150,
      initialRectY: 150,
      initialRectW: 350,
      initialRectH: 250,
    });
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

  it("renders one highlight rect per visible region", async () => {
    const { queryAllByTestId } = render(<Picker />);
    const session: PickerStartPayload = {
      picker_origin: { x: 0, y: 0 },
      picker_scale: 1,
      displays: [
        { display_index: 0, x: 0, y: 0, w: 1440, h: 900, scale_factor: 1 },
      ],
      windows: [
        {
          window_id: 9,
          app_name: "Notes",
          title: "Splits",
          bounds: { x: 0, y: 0, w: 200, h: 200 },
          display_index: 0,
          layer: 0,
          // L-shape: the top-left corner is covered, so the visible region is
          // two strips (bottom + right).
          visible_regions: [
            { x: 0, y: 100, w: 200, h: 100 },
            { x: 100, y: 0, w: 100, h: 100 },
          ],
        },
      ],
    };
    act(() => {
      (window as unknown as { __glitchSpotPickerStart: (p: PickerStartPayload) => void })
        .__glitchSpotPickerStart(session);
    });
    await Promise.resolve();
    emitHover(9);
    expect(queryAllByTestId("picker-highlight").length).toBe(2);
  });
});
