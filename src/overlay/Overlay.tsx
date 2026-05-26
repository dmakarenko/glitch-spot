// ABOUTME: Overlay window component. Supports live (transparent) and frozen (screenshot backdrop) modes.
// ABOUTME: Uses the OverlayBridge abstraction to work under both Tauri and Electron.
import { useEffect, useReducer, useRef, useState } from 'react'
import { createBridge } from './bridge'
import { DrawingLayer } from './annotations/DrawingLayer'
import { DiscardModal } from './annotations/DiscardModal'
import { initialState, reducer } from './annotations/state'
import { Toolbar } from './Toolbar'
import type { AnnotationPayload } from '../../packages/electron/src/types'

type Mode = 'live' | 'frozen'

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
    ;(window as unknown as { __glitchSpotPresent?: (mode: Mode) => void }).__glitchSpotPresent = (
      newMode,
    ) => {
      if (currentUrlRef.current) URL.revokeObjectURL(currentUrlRef.current)
      currentUrlRef.current = null
      setImageUrl(null)
      setMode(newMode)
      dispatch({ type: 'CLEAR' })
      setShowDiscardModal(false)
    }

    let unlistenFrame: (() => void) | null = null

    bridge.onFrame((blobUrl) => {
      setMode('frozen')
      const img = new window.Image()
      img.onload = () => {
        if (currentUrlRef.current) URL.revokeObjectURL(currentUrlRef.current)
        currentUrlRef.current = blobUrl
        setImageUrl(blobUrl)
      }
      img.onerror = () => URL.revokeObjectURL(blobUrl)
      img.src = blobUrl
    }).then((unlisten) => {
      unlistenFrame = unlisten
    })

    bridge.getFrame().then((blobUrl) => {
      if (blobUrl) {
        setMode('frozen')
        currentUrlRef.current = blobUrl
        setImageUrl(blobUrl)
      }
    })

    return () => {
      unlistenFrame?.()
      if (currentUrlRef.current) URL.revokeObjectURL(currentUrlRef.current)
      delete (window as unknown as { __glitchSpotPresent?: (mode: Mode) => void })
        .__glitchSpotPresent
    }
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
