// ABOUTME: Floating trigger button rendered inside the host app's React tree.
// ABOUTME: Renders nothing when window.glitchSpot is absent (production / non-debug builds).
import { useState, useEffect } from 'react'

type Position = 'bottom-right' | 'bottom-left' | 'top-right' | 'top-left'

const positionStyles: Record<Position, React.CSSProperties> = {
  'bottom-right': { bottom: 16, right: 16 },
  'bottom-left': { bottom: 16, left: 16 },
  'top-right': { top: 16, right: 16 },
  'top-left': { top: 16, left: 16 },
}

interface Props {
  position?: Position
}

export function GlitchSpotButton({ position = 'bottom-right' }: Props) {
  const [available, setAvailable] = useState(false)

  useEffect(() => {
    setAvailable(typeof (window as unknown as { glitchSpot?: unknown }).glitchSpot !== 'undefined')
  }, [])

  if (!available) return null

  return (
    <button
      type="button"
      onClick={() => (window as unknown as { glitchSpot?: { open(): void } }).glitchSpot?.open()}
      style={{
        position: 'fixed',
        ...positionStyles[position],
        zIndex: 999999,
        width: 40,
        height: 40,
        borderRadius: '50%',
        background: '#f97316',
        border: 'none',
        cursor: 'pointer',
        display: 'flex',
        alignItems: 'center',
        justifyContent: 'center',
        boxShadow: '0 2px 8px rgba(0,0,0,0.3)',
      }}
      title="Report a bug with Glitch Spot"
      aria-label="Report a bug"
    >
      <svg width="18" height="18" viewBox="0 0 24 24" fill="none" stroke="white" strokeWidth="2.5" strokeLinecap="round">
        <circle cx="12" cy="12" r="10" />
        <line x1="12" y1="8" x2="12" y2="12" />
        <line x1="12" y1="16" x2="12.01" y2="16" />
      </svg>
    </button>
  )
}
