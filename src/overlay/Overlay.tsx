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

// Hidden SVG filters that isolate an image's red channel and its green+blue
// (cyan) channel. Screen-blending a red-only copy over a cyan-only copy
// reconstructs the full-colour image when aligned; offsetting them horizontally
// is the chromatic-aberration split.
function ChromaticFilters() {
  return (
    <svg width="0" height="0" style={{ position: 'absolute' }} aria-hidden>
      <defs>
        <filter id="glitch-red" colorInterpolationFilters="sRGB">
          <feColorMatrix type="matrix" values="1 0 0 0 0  0 0 0 0 0  0 0 0 0 0  0 0 0 1 0" />
        </filter>
        <filter id="glitch-cyan" colorInterpolationFilters="sRGB">
          <feColorMatrix type="matrix" values="0 0 0 0 0  0 1 0 0 0  0 0 1 0 0  0 0 0 1 0" />
        </filter>
      </defs>
    </svg>
  )
}

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
    // Three rapid hard-step shifts (~70ms each), then snap into register.
    const opts: KeyframeAnimationOptions = {
      duration: 210,
      easing: 'steps(1, end)',
      fill: 'forwards',
    }
    redRef.current?.animate(
      [
        { transform: 'translate(11px, -2px)' },
        { transform: 'translate(-7px, 2px)', offset: 0.34 },
        { transform: 'translate(4px, -1px)', offset: 0.67 },
        { transform: 'translate(0, 0)' },
      ],
      opts,
    )
    const cyan = cyanRef.current?.animate(
      [
        { transform: 'translate(-11px, 2px)' },
        { transform: 'translate(7px, -2px)', offset: 0.34 },
        { transform: 'translate(-4px, 1px)', offset: 0.67 },
        { transform: 'translate(0, 0)' },
      ],
      opts,
    )
    if (cyan) cyan.onfinish = () => setSettled(true)
    else setSettled(true)
  }, [])

  if (settled) return <div style={fillLayer(url)} />

  return (
    <>
      {/* opaque base so the screen-blended channels composite to full colour */}
      <div style={{ ...fillLayer(url), background: '#000' }} />
      <div ref={redRef} style={{ ...fillLayer(url), filter: 'url(#glitch-red)', mixBlendMode: 'screen' }} />
      <div ref={cyanRef} style={{ ...fillLayer(url), filter: 'url(#glitch-cyan)', mixBlendMode: 'screen' }} />
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

    // Frames are only ever produced by the frozen path. Force frozen mode here
    // so the backdrop renders the screenshot even if __glitchSpotPresent was
    // not called (first show before useEffect installs it, or Electron path).
    bridge
      .onFrame((blobUrl) => {
        // Don't override window mode — frames only arrive on the frozen path.
        if (modeRef.current === 'window') {
          URL.revokeObjectURL(blobUrl)
          return
        }
        setMode("frozen");
        // Decode off-screen before swapping the visible background so the new
        // image appears atomically rather than after a perceptible decode delay.
        const img = new window.Image();
        img.onload = () => pushFrame(blobUrl);
        img.onerror = () => URL.revokeObjectURL(blobUrl);
        img.src = blobUrl;
      })
      .then((unlisten) => {
        if (cancelled) unlisten();
        else unlistenFrame = unlisten;
      })
      .catch(console.error);

    bridge
      .getFrame()
      .then((blobUrl) => {
        if (blobUrl) {
          if (modeRef.current === 'window') {
            URL.revokeObjectURL(blobUrl)
            return
          }
          setMode("frozen");
          const img = new window.Image();
          img.onload = () => pushFrame(blobUrl);
          img.onerror = () => URL.revokeObjectURL(blobUrl);
          img.src = blobUrl;
        }
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
        {/* dim veil over everything (visual only; the full-screen DrawingLayer
            above it forwards outside-clicks via onOutsideClick) */}
        <div
          style={{ position: 'absolute', inset: 0, background: 'rgba(0,0,0,0.40)', pointerEvents: 'none' }}
        />
        {/* gray-white checker over the window rect */}
        <div
          style={{
            position: 'absolute',
            left: window_rect_css.x,
            top: window_rect_css.y,
            width: window_rect_css.w,
            height: window_rect_css.h,
            backgroundImage: CHECKER_BG,
            backgroundRepeat: 'repeat',
            opacity: 0.12,
            pointerEvents: 'none',
          }}
        />
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
      <ChromaticFilters />
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
