// ABOUTME: Top-center floating toolbar for the overlay session.
// ABOUTME: Shows the rect count and a split Save button with a persistent clipboard-format picker.

import { useState, useEffect, useRef } from 'react'
import type { ClipboardFormat } from './bridge'

const FORMAT_LABELS: Record<ClipboardFormat, string> = {
  json:   'Copy JSON',
  image:  'Copy Image',
  prompt: 'Copy Prompt',
}

type Props = {
  rectCount: number
  clipboardFormat: ClipboardFormat
  onSave: () => void
  onChangeFormat: (format: ClipboardFormat) => void
}

export function Toolbar({ rectCount, clipboardFormat, onSave, onChangeFormat }: Props) {
  const [dropdownOpen, setDropdownOpen] = useState(false)
  const dropdownRef = useRef<HTMLDivElement>(null)

  useEffect(() => {
    if (!dropdownOpen) return
    const close = (e: MouseEvent) => {
      if (dropdownRef.current && !dropdownRef.current.contains(e.target as Node)) {
        setDropdownOpen(false)
      }
    }
    document.addEventListener('mousedown', close)
    return () => document.removeEventListener('mousedown', close)
  }, [dropdownOpen])

  if (rectCount === 0) return null

  const handlePickFormat = (format: ClipboardFormat, disabled: boolean) => {
    if (disabled) return
    setDropdownOpen(false)
    onChangeFormat(format)
  }

  const options: { format: ClipboardFormat; disabled: boolean }[] = [
    { format: 'json',   disabled: false },
    { format: 'image',  disabled: false },
    { format: 'prompt', disabled: true },
  ]

  return (
    <div
      className="glitch-frame"
      style={{
        position: 'fixed',
        top: 12,
        left: '50%',
        transform: 'translateX(-50%)',
        borderRadius: 9,
        zIndex: 10,
      }}
    >
      <div
        onMouseDown={(e) => e.stopPropagation()}
        onClick={(e) => e.stopPropagation()}
        style={{
          display: 'flex',
          alignItems: 'center',
          gap: 12,
          padding: '8px 12px',
          background: 'rgba(18,18,18,0.94)',
          color: '#e0e0e0',
          font: '13px/1 system-ui',
          borderRadius: 8,
          border: '1px solid rgba(255,255,255,0.07)',
          boxShadow: '0 4px 20px rgba(0,0,0,0.6)',
          backdropFilter: 'blur(8px)',
          WebkitBackdropFilter: 'blur(8px)',
          pointerEvents: 'auto',
          position: 'relative',
        }}
      >
        <span>{rectCount} rectangle{rectCount === 1 ? '' : 's'}</span>

        {/* Split button */}
        <div ref={dropdownRef} style={{ position: 'relative', display: 'flex', borderRadius: 4, overflow: 'visible' }}>
          {/* Left: run default */}
          <button
            data-testid="toolbar-save-btn"
            type="button"
            onClick={onSave}
            style={{
              padding: '6px 12px',
              background: '#ee6633',
              color: 'white',
              border: 'none',
              borderRight: '1px solid rgba(255,255,255,0.20)',
              borderRadius: '4px 0 0 4px',
              fontWeight: 600,
              cursor: 'pointer',
              whiteSpace: 'nowrap',
            }}
          >
            Save &amp; {FORMAT_LABELS[clipboardFormat]} (⌘⇧↩)
          </button>

          {/* Right: chevron */}
          <button
            data-testid="toolbar-chevron"
            type="button"
            onClick={() => setDropdownOpen((o) => !o)}
            style={{
              padding: '6px 8px',
              background: '#ee6633',
              color: 'white',
              border: 'none',
              borderRadius: '0 4px 4px 0',
              cursor: 'pointer',
              fontSize: 10,
            }}
          >
            {dropdownOpen ? '▴' : '▾'}
          </button>

          {/* Dropdown */}
          {dropdownOpen && (
            <div
              data-testid="toolbar-dropdown"
              style={{
                position: 'absolute',
                top: 'calc(100% + 6px)',
                right: 0,
                background: '#1e1e1e',
                border: '1px solid rgba(255,255,255,0.10)',
                borderRadius: 6,
                boxShadow: '0 8px 24px rgba(0,0,0,0.7)',
                minWidth: 190,
                overflow: 'hidden',
                zIndex: 100,
              }}
            >
              <div style={{
                padding: '8px 12px',
                font: '12px/1.4 system-ui',
                color: '#666',
                borderBottom: '1px solid rgba(255,255,255,0.06)',
              }}>
                Save &amp; copy to clipboard
              </div>
              {options.map(({ format, disabled }) => {
                const active = format === clipboardFormat
                return (
                  <div
                    key={format}
                    data-testid={`toolbar-option-${format}`}
                    data-active={String(active)}
                    data-disabled={String(disabled)}
                    onClick={() => handlePickFormat(format, disabled)}
                    style={{
                      padding: '9px 12px',
                      font: '13px/1 system-ui',
                      color: disabled ? '#555' : active ? '#fff' : '#e0e0e0',
                      background: active ? '#ee6633' : 'transparent',
                      cursor: disabled ? 'default' : 'pointer',
                      display: 'flex',
                      alignItems: 'center',
                      gap: 8,
                    }}
                  >
                    <span style={{ fontSize: 11, opacity: active ? 0.8 : 0 }}>✓</span>
                    {FORMAT_LABELS[format]}{disabled ? ' (soon)' : ''}
                  </div>
                )
              })}
            </div>
          )}
        </div>
      </div>
    </div>
  )
}
