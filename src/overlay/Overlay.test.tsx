// ABOUTME: Rendering tests for the overlay component's window mode.
// ABOUTME: Verifies the dim veil spotlights the selected window region.

// @vitest-environment jsdom

import { describe, expect, it, vi, afterEach, beforeAll, beforeEach } from "vitest";
import { render, act, cleanup, findByText, waitFor } from "@testing-library/react";
import { Overlay } from "./Overlay";
import type { WindowModePayload } from "../picker/types";

const { invokeMock, frameHandlers } = vi.hoisted(() => ({
  invokeMock: vi.fn((..._args: unknown[]) => Promise.resolve(null)),
  // Captures the 'glitch-spot://frame' listener (the bridge's onFrame wrapper)
  // so a test can deliver a frame through the real frame path. The bridge calls
  // listen(name, handler); the image-frame test invokes the captured handler.
  frameHandlers: [] as Array<(e: { payload: unknown }) => void>,
}));

beforeAll(() => {
  // jsdom does not implement the Web Animations API. The frozen-frame shimmer
  // (GlitchShimmer) and chromatic resolve (GlitchImage) call Element.animate on
  // mount; stub it so the components render without throwing. The animation
  // itself is verified by the manual smoke test, not here.
  if (!Element.prototype.animate) {
    Element.prototype.animate = () =>
      ({ cancel() {}, finish() {}, onfinish: null }) as unknown as Animation;
  }
});

vi.mock("@tauri-apps/api/core", () => ({
  invoke: invokeMock,
}));
vi.mock("@tauri-apps/api/event", () => ({
  listen: (name: string, handler: (e: { payload: unknown }) => void) => {
    if (name === "glitch-spot://frame") frameHandlers.push(handler);
    return Promise.resolve(() => {});
  },
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

    const { window_rect_css: r } = payload;
    // Four solid panels frame the clear window rect (top / bottom / left /
    // right). Solid fills rasterise immediately; a 100vmax box-shadow spread
    // rasterised late and flashed undimmed for a frame on show.
    const top = getByTestId("window-dim-top");
    expect(top.style.top).toBe("0px");
    expect(top.style.height).toBe(`${r.y}px`);

    const bottom = getByTestId("window-dim-bottom");
    expect(bottom.style.top).toBe(`${r.y + r.h}px`);

    const left = getByTestId("window-dim-left");
    expect(left.style.top).toBe(`${r.y}px`);
    expect(left.style.width).toBe(`${r.x}px`);
    expect(left.style.height).toBe(`${r.h}px`);

    const right = getByTestId("window-dim-right");
    expect(right.style.top).toBe(`${r.y}px`);
    expect(right.style.left).toBe(`${r.x + r.w}px`);
    expect(right.style.height).toBe(`${r.h}px`);

    for (const panel of [top, bottom, left, right]) {
      // Dim is 40% black; panels must not capture clicks — outside-click-to-
      // cancel relies on the full-screen DrawingLayer underneath catching them.
      expect(panel.style.background).toContain("rgba(0, 0, 0");
      expect(panel.style.pointerEvents).toBe("none");
    }
  });
});

describe("Overlay clipboard format", () => {
  afterEach(() => {
    cleanup();
    invokeMock.mockReset();
    invokeMock.mockImplementation((..._args: unknown[]) => Promise.resolve(null));
  });

  it("passes clipboardFormat prop to Toolbar", async () => {
    // Make getClipboardFormat return 'image' so Toolbar renders "Copy Image"
    invokeMock.mockImplementation((cmd: unknown) => {
      if (cmd === "get_clipboard_format") return Promise.resolve("image" as unknown as null);
      return Promise.resolve(null);
    });

    const { container } = render(<Overlay />);

    // Add a rect so Toolbar becomes visible (Toolbar returns null when rectCount === 0)
    act(() => {
      (
        window as unknown as {
          __glitchSpotPresent: (mode: string) => void;
        }
      ).__glitchSpotPresent("frozen");
    });

    // Draw a rect by dispatching directly via the state — simulate a rect in the
    // DOM by looking for the toolbar after state propagates.
    // We need a rect to make Toolbar render. We dispatch a synthetic present with
    // an initial rect via window mode with initial_rect_css.
    act(() => {
      (
        window as unknown as {
          __glitchSpotPresent: (mode: string, payload: unknown) => void;
        }
      ).__glitchSpotPresent("window", {
        app_name: "Safari",
        title: "Reader",
        window_id: 7,
        window_rect_css: { x: 0, y: 0, w: 800, h: 600 },
        window_rect_physical: { x: 0, y: 0, w: 1600, h: 1200 },
        display_scale_factor: 2,
        initial_rect_css: { x: 10, y: 10, w: 200, h: 100 },
      });
    });

    // Wait for the async getClipboardFormat call to resolve and re-render
    await act(async () => {
      await Promise.resolve();
    });

    // Toolbar is now visible (1 rect); "Copy Image" must appear in the save button
    expect(invokeMock).toHaveBeenCalledWith("get_clipboard_format");
    await findByText(container, /Copy Image/);
  });
});

describe("Overlay handleFormatChange", () => {
  afterEach(() => {
    cleanup();
    invokeMock.mockReset();
    invokeMock.mockImplementation((..._args: unknown[]) => Promise.resolve(null));
  });

  it("persists the format via set_clipboard_format then triggers save_snapshot", async () => {
    // Arrange: get_clipboard_format returns 'json' (the default) on init
    invokeMock.mockImplementation((cmd: unknown) => {
      if (cmd === "get_clipboard_format") return Promise.resolve("json" as unknown as null);
      return Promise.resolve(null);
    });

    const { getByTestId } = render(<Overlay />);

    // Present with an initial rect so Toolbar renders (rectCount > 0)
    act(() => {
      (
        window as unknown as {
          __glitchSpotPresent: (mode: string, payload: unknown) => void;
        }
      ).__glitchSpotPresent("window", {
        app_name: "Safari",
        title: "Reader",
        window_id: 7,
        window_rect_css: { x: 0, y: 0, w: 800, h: 600 },
        window_rect_physical: { x: 0, y: 0, w: 1600, h: 1200 },
        display_scale_factor: 2,
        initial_rect_css: { x: 10, y: 10, w: 200, h: 100 },
      });
    });

    // Wait for async getClipboardFormat to resolve
    await act(async () => {
      await Promise.resolve();
    });

    invokeMock.mockClear();

    // Act: open the format dropdown and select 'image'
    act(() => {
      getByTestId("toolbar-chevron").click();
    });

    await act(async () => {
      getByTestId("toolbar-option-image").click();
      // Flush the awaited setClipboardFormat promise inside handleFormatChange
      await Promise.resolve();
      await Promise.resolve();
    });

    // Assert: set_clipboard_format was called with 'image' before save_snapshot
    const calls = invokeMock.mock.calls.map((c) => c[0]);
    const formatIdx = calls.indexOf("set_clipboard_format");
    const saveIdx = calls.indexOf("save_snapshot");

    expect(formatIdx).toBeGreaterThanOrEqual(0);
    await waitFor(() => {
      expect(invokeMock).toHaveBeenCalledWith("set_clipboard_format", { format: "image" });
    });
    await waitFor(() => {
      expect(invokeMock).toHaveBeenCalledWith("save_snapshot", expect.anything());
    });
    // Ordering: persist must precede save
    expect(formatIdx).toBeLessThan(saveIdx);
  });
});

describe("<Overlay/> image mode", () => {
  afterEach(() => {
    cleanup();
    frameHandlers.length = 0;
  });

  it("image mode renders the dimmed framed branch at the image rect", async () => {
    const { getByTestId } = render(<Overlay />);
    await act(async () => {
      (
        window as unknown as {
          __glitchSpotPresent: (mode: string, payload: unknown) => void;
        }
      ).__glitchSpotPresent("image", {
        image_rect_css: { x: 100, y: 50, w: 400, h: 300 },
      });
    });
    // The window-mode dim panels are reused for image mode; their presence
    // proves the framed branch (not the live/frozen branch) rendered.
    const top = getByTestId("window-dim-top");
    expect(top).toBeTruthy();
    const frame = getByTestId("window-frozen-frame");
    expect(frame.style.left).toBe("100px");
    expect(frame.style.top).toBe("50px");
    expect(frame.style.width).toBe("400px");
    expect(frame.style.height).toBe("300px");
  });

  it("frozen mode does not render the framed (window/image) branch", async () => {
    const { queryByTestId } = render(<Overlay />);
    await act(async () => {
      (
        window as unknown as { __glitchSpotPresent: (mode: string) => void }
      ).__glitchSpotPresent("frozen");
    });
    expect(queryByTestId("window-dim-top")).toBeNull();
    expect(queryByTestId("window-frozen-frame")).toBeNull();
  });

  it("renders a pushed frame into the framed box without flipping out of image mode", async () => {
    // No prior test drives a frame: the listen mock is a no-op and jsdom lacks
    // URL.createObjectURL + a loading Image. Stub the I/O boundary so a frame
    // can flow through the real onFrame → showBlob → pushFrame → GlitchImage
    // path. The blob token is opaque; what's verified is that it reaches the
    // DOM and that showBlob's guard keeps mode 'image' (no flip to frozen).
    const BLOB_TOKEN = "blob:test-frame";
    vi.stubGlobal("requestAnimationFrame", (_cb: FrameRequestCallback) => 0);
    const origCreate = URL.createObjectURL;
    URL.createObjectURL = vi.fn(() => BLOB_TOKEN);
    // showBlob constructs `new window.Image()` and pushes the frame only on
    // onload; jsdom never fires it for a blob URL, so fire it on src assignment.
    const origImage = window.Image;
    class StubImage {
      onload: (() => void) | null = null;
      onerror: (() => void) | null = null;
      set src(_v: string) {
        this.onload?.();
      }
    }
    (window as unknown as { Image: unknown }).Image = StubImage;

    try {
      const { getByTestId, container } = render(<Overlay />);
      await act(async () => {
        (
          window as unknown as {
            __glitchSpotPresent: (mode: string, payload: unknown) => void;
          }
        ).__glitchSpotPresent("image", {
          image_rect_css: { x: 100, y: 50, w: 400, h: 300 },
        });
      });

      // Deliver a frame through the captured 'glitch-spot://frame' handler.
      expect(frameHandlers.length).toBeGreaterThan(0);
      await act(async () => {
        frameHandlers.forEach((h) =>
          h({
            payload: {
              width: 4,
              height: 4,
              scale_factor: 2,
              png: [1, 2, 3, 4],
              display_index: 0,
            },
          }),
        );
        await Promise.resolve();
      });

      // The framed box is still present at the image rect — the showBlob guard
      // held (a regression flipping to frozen would drop this testid).
      const frame = getByTestId("window-frozen-frame");
      expect(frame.style.left).toBe("100px");
      // The decoded frame's blob URL reached the DOM (GlitchImage's channel
      // layers), not the GlitchShimmer placeholder.
      expect(container.innerHTML).toContain(BLOB_TOKEN);
    } finally {
      URL.createObjectURL = origCreate;
      (window as unknown as { Image: unknown }).Image = origImage;
      vi.unstubAllGlobals();
    }
  });
});

describe("<Overlay/> post-paint signals", () => {
  beforeEach(() => {
    // Run rAF synchronously so the post-paint signals are observable here.
    vi.stubGlobal("requestAnimationFrame", (cb: FrameRequestCallback) => {
      cb(0);
      return 0;
    });
  });
  afterEach(() => {
    cleanup();
    vi.unstubAllGlobals();
    invokeMock.mockClear();
  });

  it("nudges the OS cursor after presenting so the crosshair appears without a manual mouse move", () => {
    // The crosshair is a CSS cursor that macOS only redraws on a mouse event.
    render(<Overlay />);
    invokeMock.mockClear(); // ignore the mount-time get_latest_frame probe
    presentWindowMode(makeWindowPayload());
    expect(invokeMock).toHaveBeenCalledWith("refresh_cursor");
  });

  it("reveals the overlay after the window-mode frame paints so show() never flashes the previous display's stale buffer", () => {
    // The overlay is shown at alpha 0 (rendering but invisible), so the window
    // repaints the new display's layout before becoming visible. The frontend
    // signals once that frame has painted; the backend then sets alpha 1.
    render(<Overlay />);
    invokeMock.mockClear(); // ignore the mount-time get_latest_frame probe
    presentWindowMode(makeWindowPayload());
    expect(invokeMock).toHaveBeenCalledWith("reveal_window");
  });

  it("renders a clipped frozen-frame box mirroring the full-screen frozen render", () => {
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
    // Before the frame arrives, the scanline shimmer plays (the cathode-tube
    // stripe), matching the full-screen frozen render.
    expect(container.innerHTML).toContain("repeating-linear-gradient");
    // A faint gray-white checker texture overlays the frozen capture, also
    // mirroring the full-screen frozen render.
    expect(container.innerHTML).toContain("%23999999");
  });
});
