// ABOUTME: Overlay window component. Supports live (transparent), frozen (screenshot backdrop), and window modes.
// ABOUTME: Uses the OverlayBridge abstraction to work under both Tauri and Electron.
import { useEffect, useReducer, useRef, useState } from 'react'
import type { CSSProperties } from 'react'
import { createBridge } from './bridge'
import { DrawingLayer, nextRectId } from './annotations/DrawingLayer'
import { MIN_RECT_DIMENSION } from './annotations/state'
import { DiscardModal } from './annotations/DiscardModal'
import { initialState, reducer } from './annotations/state'
import { Toolbar } from './Toolbar'
import type { AnnotationPayload } from '../../packages/electron/src/types'
import type { WindowModePayload } from '../picker/types'

type Mode = 'live' | 'frozen' | 'window'

const CHECKER_BG =
  "url(\"data:image/svg+xml;utf8,<svg xmlns='http://www.w3.org/2000/svg' width='20' height='20'>" +
  "<rect width='10' height='10' fill='%23999999'/>" +
  "<rect x='10' y='10' width='10' height='10' fill='%23999999'/>" +
  "<rect x='10' y='0' width='10' height='10' fill='%23ffffff'/>" +
  "<rect x='0' y='10' width='10' height='10' fill='%23ffffff'/>" +
  "</svg>\")"

const bridge = createBridge()

const fillLayer = (url: string): CSSProperties => ({
  position: 'absolute',
  inset: 0,
  background: `center / 100% 100% no-repeat url(${url})`,
  pointerEvents: 'none',
})

// A single-channel copy of the screenshot, isolated by multiply-blending the
// image against a pure red or cyan fill (R×red = red channel only, etc.).
// Screen-blending the two copies over black reconstructs full colour when
// aligned; offsetting them is the chromatic split. Uses background-blend-mode
// rather than SVG filters — GPU-composited, so it rasterises immediately
// (SVG filters on a multi-megapixel surface rasterise late and miss a short
// animation).
const channelLayer = (url: string, color: string): CSSProperties => ({
  position: 'absolute',
  inset: 0,
  pointerEvents: 'none',
  backgroundImage: `url(${url})`,
  backgroundColor: color,
  backgroundBlendMode: 'multiply',
  backgroundSize: '100% 100%',
  backgroundRepeat: 'no-repeat',
  backgroundPosition: 'center',
  mixBlendMode: 'screen',
})

// Shown over the grey veil while the screenshot is being captured: faint
// scanlines plus an accent scan bar sweeping top-to-bottom, signalling activity
// for however long the capture takes.
function GlitchShimmer() {
  const barRef = useRef<HTMLDivElement>(null)
  useEffect(() => {
    const anim = barRef.current?.animate(
      [
        { transform: 'translateY(-120%)' },
        { transform: 'translateY(820%)' },
      ],
      { duration: 1100, iterations: Infinity, easing: 'ease-in-out' },
    )
    return () => anim?.cancel()
  }, [])
  return (
    <>
      <div
        style={{
          position: 'absolute',
          inset: 0,
          pointerEvents: 'none',
          background:
            'repeating-linear-gradient(0deg, rgba(255,255,255,0.05) 0 1px, transparent 1px 3px)',
        }}
      />
      <div
        ref={barRef}
        style={{
          position: 'absolute',
          left: 0,
          right: 0,
          height: '12%',
          pointerEvents: 'none',
          background: 'linear-gradient(180deg, transparent, rgba(238,102,51,0.20), transparent)',
        }}
      />
    </>
  )
}

// The captured screenshot, resolved with a chromatic glitch: the red and cyan
// channels do three rapid hard-step shifts then snap into register. The two
// channel copies screen-blend over black to reconstruct full colour once
// aligned. Settles to a single clean layer so no filter/blend cost lingers.
function GlitchImage({ url }: { url: string }) {
  const [settled, setSettled] = useState(false)
  const redRef = useRef<HTMLDivElement>(null)
  const cyanRef = useRef<HTMLDivElement>(null)

  useEffect(() => {
    // A snappy 3-shake: the channels swing right → left → right in quick
    // alternation (~40ms per move) then settle into register.
    const opts: KeyframeAnimationOptions = { duration: 240, easing: 'linear', fill: 'forwards' }
    const startShake = () => {
      redRef.current?.animate(
        [
          { transform: 'translate(0, 0)' },
          { transform: 'translate(18px, -4px)' }, //  right
          { transform: 'translate(0, 0)' },
          { transform: 'translate(-18px, 4px)' }, //  left
          { transform: 'translate(0, 0)' },
          { transform: 'translate(18px, -3px)' }, //  right
          { transform: 'translate(0, 0)' }, //         settle
        ],
        opts,
      )
      const cyan = cyanRef.current?.animate(
        [
          { transform: 'translate(0, 0)' },
          { transform: 'translate(-18px, 4px)' },
          { transform: 'translate(0, 0)' },
          { transform: 'translate(18px, -4px)' },
          { transform: 'translate(0, 0)' },
          { transform: 'translate(-18px, 3px)' },
          { transform: 'translate(0, 0)' },
        ],
        opts,
      )
      if (cyan) cyan.onfinish = () => setSettled(true)
      else setSettled(true)
    }
    // Wait two frames so the (filter-heavy, multi-megapixel) chroma layers have
    // painted before the shake plays — otherwise a short animation finishes and
    // settles before the layers are even visible.
    let raf2 = 0
    const raf1 = requestAnimationFrame(() => {
      raf2 = requestAnimationFrame(startShake)
    })
    return () => {
      cancelAnimationFrame(raf1)
      cancelAnimationFrame(raf2)
    }
  }, [])

  if (settled) return <div style={fillLayer(url)} />

  return (
    <>
      {/* opaque base so the screen-blended channels composite to full colour */}
      <div style={{ ...fillLayer(url), background: '#000' }} />
      <div ref={redRef} style={channelLayer(url, 'rgb(255,0,0)')} />
      <div ref={cyanRef} style={channelLayer(url, 'rgb(0,255,255)')} />
    </>
  )
}

export function Overlay() {
  const [mode, setMode] = useState<Mode>('live')
  const modeRef = useRef<Mode>('live')
  useEffect(() => { modeRef.current = mode }, [mode])
  const [windowPayload, setWindowPayload] = useState<WindowModePayload | null>(null)
  const [imageUrl, setImageUrl] = useState<string | null>(null)
  const imageUrlRef = useRef<string | null>(null)
  const [annotations, dispatch] = useReducer(reducer, initialState)
  const [showDiscardModal, setShowDiscardModal] = useState(false)

  const handleSave = async () => {
    const payload: AnnotationPayload[] = annotations.rects.map((r) => ({
      id: r.id,
      x: r.x,
      y: r.y,
      w: r.w,
      h: r.h,
      note: r.note,
    }))
    try {
      await bridge.saveSnapshot(mode, payload)
      dispatch({ type: 'CLEAR' })
      setShowDiscardModal(false)
    } catch (err) {
      console.error('saveSnapshot failed', err)
    }
  }

  useEffect(() => {
    // Clear the screenshot back to the bare veil (e.g. on a new present).
    const resetFrames = () => {
      if (imageUrlRef.current) URL.revokeObjectURL(imageUrlRef.current);
      imageUrlRef.current = null;
      setImageUrl(null);
    };
    // Show a freshly-decoded frame; it resolves via the chromatic glitch-settle.
    const pushFrame = (url: string) => {
      if (imageUrlRef.current) URL.revokeObjectURL(imageUrlRef.current);
      imageUrlRef.current = url;
      setImageUrl(url);
    };

    // The Rust side calls __glitchSpotPresent via WebviewWindow::eval before
    // showing the window so the mode/state is reset synchronously before first
    // paint. In the Electron path this is not called — the bridge handles reset.
    ;(window as unknown as { __glitchSpotPresent?: (mode: Mode, payload?: WindowModePayload) => void }).__glitchSpotPresent = (
      newMode,
      payload,
    ) => {
      resetFrames();
      setMode(newMode);
      dispatch({ type: "CLEAR" });
      setShowDiscardModal(false);
      setWindowPayload(newMode === 'window' && payload ? payload : null);
      if (newMode === 'window' && payload?.initial_rect_css) {
        const r = payload.initial_rect_css;
        const clip = payload.window_rect_css;
        // Clamp to the selected window's bounds.
        const x = Math.max(clip.x, r.x);
        const y = Math.max(clip.y, r.y);
        const w = Math.min(clip.x + clip.w, r.x + r.w) - x;
        const h = Math.min(clip.y + clip.h, r.y + r.h) - y;
        if (w >= MIN_RECT_DIMENSION && h >= MIN_RECT_DIMENSION) {
          const id = nextRectId();
          dispatch({ type: 'BEGIN_CREATE', id, x, y });
          dispatch({ type: 'POINTER_TO', x: x + w, y: y + h, viewportW: window.innerWidth, viewportH: window.innerHeight });
          dispatch({ type: 'END_GESTURE' });
        }
      }
    };

    let unlistenFrame: (() => void) | null = null;
    let cancelled = false;
    // One-shot guard so the mount-time getFrame() pull doesn't replay the glitch
    // if the listener already delivered the same frame.
    let anyFrameShown = false;

    // Decode off-screen before swapping the visible background so the new image
    // appears atomically.
    const showBlob = (blobUrl: string) => {
      anyFrameShown = true;
      // A pushed frame in window mode is the frozen window capture — keep mode
      // 'window' and let it render into the window rect. Otherwise it's a monitor
      // freeze and we switch to frozen full-screen.
      if (modeRef.current !== 'window') setMode("frozen");
      const img = new window.Image();
      img.onload = () => pushFrame(blobUrl);
      img.onerror = () => URL.revokeObjectURL(blobUrl);
      img.src = blobUrl;
    };

    // Register the frame listener FIRST, then pull the current frame. On a cold
    // first show the capture's emit can land before the webview is listening
    // (we now show the overlay before capturing); pulling after the listener is
    // active recovers that frame instead of losing the first freeze.
    bridge
      .onFrame(showBlob)
      .then((unlisten) => {
        if (cancelled) {
          unlisten()
          return undefined
        }
        unlistenFrame = unlisten
        return bridge.getFrame()
      })
      .then((blobUrl) => {
        if (!blobUrl || cancelled || anyFrameShown) {
          if (blobUrl) URL.revokeObjectURL(blobUrl)
          return
        }
        showBlob(blobUrl)
      })
      .catch(console.error);

    return () => {
      cancelled = true;
      unlistenFrame?.();
      resetFrames();
      delete (window as unknown as { __glitchSpotPresent?: (mode: Mode, payload?: WindowModePayload) => void })
        .__glitchSpotPresent;
    };
  }, [])

  useEffect(() => {
    const onKey = (e: KeyboardEvent) => {
      if (e.key === 'Escape') {
        e.preventDefault()
        if (annotations.rects.length === 0) {
          bridge.cancel()
        } else {
          setShowDiscardModal(true)
        }
        return
      }
      if ((e.metaKey || e.ctrlKey) && e.shiftKey && e.key === 'Enter') {
        e.preventDefault()
        if (annotations.rects.length > 0) handleSave()
        return
      }
      if ((e.metaKey || e.ctrlKey) && e.key.toLowerCase() === 'z') {
        e.preventDefault()
        dispatch({ type: 'UNDO' })
        return
      }
    }
    window.addEventListener('keydown', onKey)
    return () => window.removeEventListener('keydown', onKey)
  }, [annotations.rects, mode])

  if (mode === 'window' && windowPayload) {
    const { window_rect_css } = windowPayload
    return (
      <div style={{ position: 'fixed', inset: 0, background: 'transparent' }}>
        {/* dim everything except the selected window: a transparent rect at the
            window bounds whose huge box-shadow spread dims the surrounding
            screen. Visual only; the full-screen DrawingLayer above forwards
            outside-clicks via onOutsideClick. */}
        <div
          data-testid="window-spotlight"
          style={{
            position: 'absolute',
            left: window_rect_css.x,
            top: window_rect_css.y,
            width: window_rect_css.w,
            height: window_rect_css.h,
            background: 'transparent',
            boxShadow: '0 0 0 100vmax rgba(0,0,0,0.40)',
            pointerEvents: 'none',
          }}
        />
        {/* frozen isolated capture of the selected window, presented above
            everything. Empty until the frame arrives (live window shows through
            the transparent rect); then the chromatic shake resolves it in.
            overflow:hidden clips the ~18px shake to the window bounds. */}
        <div
          data-testid="window-frozen-frame"
          style={{
            position: 'absolute',
            left: window_rect_css.x,
            top: window_rect_css.y,
            width: window_rect_css.w,
            height: window_rect_css.h,
            overflow: 'hidden',
            pointerEvents: 'none',
          }}
        >
          {imageUrl && <GlitchImage key={imageUrl} url={imageUrl} />}
        </div>
        {/* drawing layer constrained to window rect */}
        <DrawingLayer
          state={annotations}
          dispatch={dispatch}
          clipRect={window_rect_css}
          onOutsideClick={() => {
            if (annotations.rects.length === 0) {
              bridge.cancel()
            } else {
              setShowDiscardModal(true)
            }
          }}
        />
        <Toolbar rectCount={annotations.rects.length} onSave={handleSave} />
        {showDiscardModal && (
          <DiscardModal
            onDiscard={() => {
              setShowDiscardModal(false)
              dispatch({ type: 'CLEAR' })
              bridge.cancel()
            }}
            onSave={() => {
              setShowDiscardModal(false)
              handleSave()
            }}
            onCancel={() => setShowDiscardModal(false)}
          />
        )}
      </div>
    )
  }

  // In frozen mode the overlay paints before the screenshot is ready: the base
  // is a dim "freeze engaged" veil shown immediately, a scanline shimmer plays
  // while capturing, then the screenshot resolves with a chromatic glitch.
  const base = mode === 'frozen' ? 'rgba(0, 0, 0, 0.4)' : 'transparent'

  return (
    <div style={{ position: 'fixed', inset: 0, background: base }}>
      {mode === 'frozen' && !imageUrl && <GlitchShimmer />}
      {imageUrl && <GlitchImage key={imageUrl} url={imageUrl} />}
      <div
        style={{
          position: 'absolute',
          inset: 0,
          backgroundImage: CHECKER_BG,
          backgroundRepeat: 'repeat',
          opacity: 0.12,
          pointerEvents: 'none',
        }}
      />
      <DrawingLayer state={annotations} dispatch={dispatch} />
      <Toolbar rectCount={annotations.rects.length} onSave={handleSave} />
      {showDiscardModal && (
        <DiscardModal
          onDiscard={() => {
            setShowDiscardModal(false)
            dispatch({ type: 'CLEAR' })
            bridge.cancel()
          }}
          onSave={() => {
            setShowDiscardModal(false)
            handleSave()
          }}
          onCancel={() => setShowDiscardModal(false)}
        />
      )}
    </div>
  )
}
