// ABOUTME: Overlay window component. Supports live (transparent), frozen (screenshot backdrop), and window modes.
// ABOUTME: Uses the OverlayBridge abstraction to work under both Tauri and Electron.
import { useEffect, useReducer, useRef, useState } from 'react'
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

export function Overlay() {
  const [mode, setMode] = useState<Mode>('live')
  const modeRef = useRef<Mode>('live')
  useEffect(() => { modeRef.current = mode }, [mode])
  const [windowPayload, setWindowPayload] = useState<WindowModePayload | null>(null)
  const [imageUrl, setImageUrl] = useState<string | null>(null)
  const [annotations, dispatch] = useReducer(reducer, initialState)
  const [showDiscardModal, setShowDiscardModal] = useState(false)
  const currentUrlRef = useRef<string | null>(null)

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
    // The Rust side calls __glitchSpotPresent via WebviewWindow::eval before
    // showing the window so the mode/state is reset synchronously before first
    // paint. In the Electron path this is not called — the bridge handles reset.
    ;(window as unknown as { __glitchSpotPresent?: (mode: Mode, payload?: WindowModePayload) => void }).__glitchSpotPresent = (
      newMode,
      payload,
    ) => {
      if (currentUrlRef.current) URL.revokeObjectURL(currentUrlRef.current);
      currentUrlRef.current = null;
      setImageUrl(null);
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
        img.onload = () => {
          if (currentUrlRef.current) URL.revokeObjectURL(currentUrlRef.current);
          currentUrlRef.current = blobUrl;
          setImageUrl(blobUrl);
        };
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
          img.onload = () => {
            if (currentUrlRef.current) URL.revokeObjectURL(currentUrlRef.current);
            currentUrlRef.current = blobUrl;
            setImageUrl(blobUrl);
          };
          img.onerror = () => URL.revokeObjectURL(blobUrl);
          img.src = blobUrl;
        }
      })
      .catch(console.error);

    return () => {
      cancelled = true;
      unlistenFrame?.();
      if (currentUrlRef.current) URL.revokeObjectURL(currentUrlRef.current);
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

  const backdrop =
    mode === 'frozen' && imageUrl ? `center / 100% 100% no-repeat url(${imageUrl})` : 'transparent'

  return (
    <div style={{ position: 'fixed', inset: 0, background: backdrop }}>
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
