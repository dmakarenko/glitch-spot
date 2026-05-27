// ABOUTME: Overlay window component. Supports live (transparent), frozen (screenshot backdrop), and window modes.
// ABOUTME: Uses the OverlayBridge abstraction to work under both Tauri and Electron.
import { useEffect, useReducer, useRef, useState } from 'react'
import { createBridge } from './bridge'
import { DrawingLayer } from './annotations/DrawingLayer'
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
    };

    let unlistenFrame: (() => void) | null = null;
    let cancelled = false;

    // Frames are only ever produced by the frozen path. Force frozen mode here
    // so the backdrop renders the screenshot even if __glitchSpotPresent was
    // not called (first show before useEffect installs it, or Electron path).
    bridge
      .onFrame((blobUrl) => {
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
      if ((e.metaKey || e.ctrlKey) && e.key === 'Enter') {
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
        {/* dim veil over everything */}
        <div
          style={{ position: 'absolute', inset: 0, background: 'rgba(0,0,0,0.40)' }}
          onClick={() => {
            if (annotations.rects.length === 0) {
              bridge.cancel()
            } else {
              setShowDiscardModal(true)
            }
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
        <DrawingLayer state={annotations} dispatch={dispatch} clipRect={window_rect_css} />
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
