// ABOUTME: Tests for the GlitchSpotButton renderer component.
// ABOUTME: Verifies visibility logic: renders only when window.glitchSpot is present.
import { describe, it, expect, beforeEach, afterEach } from 'vitest'
import { render, screen, fireEvent } from '@testing-library/react'
import '@testing-library/jest-dom'
import { GlitchSpotButton } from '../src/GlitchSpotButton'

beforeEach(() => {
  delete (window as unknown as { glitchSpot?: unknown }).glitchSpot
})

afterEach(() => {
  delete (window as unknown as { glitchSpot?: unknown }).glitchSpot
})

describe('GlitchSpotButton', () => {
  it('renders nothing when window.glitchSpot is absent', () => {
    const { container } = render(<GlitchSpotButton position="bottom-right" />)
    expect(container.firstChild).toBeNull()
  })

  it('renders a button when window.glitchSpot is present', () => {
    ;(window as unknown as { glitchSpot: { open: () => void } }).glitchSpot = { open: () => {} }
    render(<GlitchSpotButton position="bottom-right" />)
    expect(screen.getByRole('button')).toBeInTheDocument()
  })

  it('calls window.glitchSpot.open() on click', () => {
    let called = false
    ;(window as unknown as { glitchSpot: { open: () => void } }).glitchSpot = {
      open: () => { called = true },
    }
    render(<GlitchSpotButton position="bottom-right" />)
    fireEvent.click(screen.getByRole('button'))
    expect(called).toBe(true)
  })
})
