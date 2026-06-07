// ABOUTME: Tests for the Toolbar split button — save action, format dropdown, and default tracking.
// ABOUTME: Also covers the Redact split control: toggle, style picker, and always-visible behavior.
// @vitest-environment jsdom

import { describe, it, expect, vi, afterEach } from 'vitest'
import { render, fireEvent, cleanup } from '@testing-library/react'
import { Toolbar } from './Toolbar'

afterEach(cleanup)

const defaultProps = {
  clipboardFormat: 'json' as const,
  onSave: vi.fn(),
  onChangeFormat: vi.fn(),
  redactMode: false,
  redactionStyle: 'branded' as const,
  onToggleRedact: vi.fn(),
  onChangeRedactionStyle: vi.fn(),
}

describe('Toolbar', () => {
  it('shows the redact toggle at rectCount=0 and hides the save button', () => {
    const { getByTestId, queryByTestId } = render(
      <Toolbar {...defaultProps} rectCount={0} />
    )
    expect(getByTestId('toolbar-redact-toggle')).toBeTruthy()
    expect(queryByTestId('toolbar-save-btn')).toBeNull()
  })

  it('renders the save button with the current format label', () => {
    const { getByText } = render(
      <Toolbar {...defaultProps} rectCount={2} clipboardFormat="json" />
    )
    expect(getByText(/Copy JSON/)).toBeTruthy()
  })

  it('renders image label when format is image', () => {
    const { getByText } = render(
      <Toolbar {...defaultProps} rectCount={1} clipboardFormat="image" />
    )
    expect(getByText(/Copy Image/)).toBeTruthy()
  })

  it('calls onSave when left button is clicked', () => {
    const onSave = vi.fn()
    const { getByTestId } = render(
      <Toolbar {...defaultProps} rectCount={1} onSave={onSave} />
    )
    fireEvent.click(getByTestId('toolbar-save-btn'))
    expect(onSave).toHaveBeenCalledTimes(1)
  })

  it('opens dropdown when chevron is clicked', () => {
    const { getByTestId, queryByTestId } = render(
      <Toolbar {...defaultProps} rectCount={1} />
    )
    expect(queryByTestId('toolbar-dropdown')).toBeNull()
    fireEvent.click(getByTestId('toolbar-chevron'))
    expect(queryByTestId('toolbar-dropdown')).toBeTruthy()
  })

  it('calls onChangeFormat with the selected format and closes dropdown', () => {
    const onChangeFormat = vi.fn()
    const { getByTestId, queryByTestId } = render(
      <Toolbar {...defaultProps} rectCount={1} onChangeFormat={onChangeFormat} />
    )
    fireEvent.click(getByTestId('toolbar-chevron'))
    fireEvent.click(getByTestId('toolbar-option-image'))
    expect(onChangeFormat).toHaveBeenCalledWith('image')
    expect(queryByTestId('toolbar-dropdown')).toBeNull()
  })

  it('shows a checkmark on the active format option', () => {
    const { getByTestId } = render(
      <Toolbar {...defaultProps} rectCount={1} clipboardFormat="image" />
    )
    fireEvent.click(getByTestId('toolbar-chevron'))
    expect(getByTestId('toolbar-option-image').dataset.active).toBe('true')
    expect(getByTestId('toolbar-option-json').dataset.active).toBe('false')
  })

  it('prompt option is rendered and enabled', () => {
    const onChangeFormat = vi.fn()
    const { getByTestId } = render(
      <Toolbar {...defaultProps} rectCount={1} onChangeFormat={onChangeFormat} />
    )
    fireEvent.click(getByTestId('toolbar-chevron'))
    const promptOption = getByTestId('toolbar-option-prompt')
    expect(promptOption.dataset.disabled).toBe('false')
    fireEvent.click(promptOption)
    expect(onChangeFormat).toHaveBeenCalledWith('prompt')
  })

  describe('Redact split control', () => {
    it('calls onToggleRedact when the toggle button is clicked', () => {
      const onToggleRedact = vi.fn()
      const { getByTestId } = render(
        <Toolbar {...defaultProps} rectCount={0} onToggleRedact={onToggleRedact} />
      )
      fireEvent.click(getByTestId('toolbar-redact-toggle'))
      expect(onToggleRedact).toHaveBeenCalledTimes(1)
    })

    it('shows the redact dropdown when chevron is clicked', () => {
      const { getByTestId, queryByTestId } = render(
        <Toolbar {...defaultProps} rectCount={0} />
      )
      expect(queryByTestId('toolbar-redact-dropdown')).toBeNull()
      fireEvent.click(getByTestId('toolbar-redact-chevron'))
      expect(queryByTestId('toolbar-redact-dropdown')).toBeTruthy()
    })

    it('calls onChangeRedactionStyle with the selected style and does NOT call onSave', () => {
      const onChangeRedactionStyle = vi.fn()
      const onSave = vi.fn()
      const { getByTestId, queryByTestId } = render(
        <Toolbar {...defaultProps} rectCount={0} onChangeRedactionStyle={onChangeRedactionStyle} onSave={onSave} />
      )
      fireEvent.click(getByTestId('toolbar-redact-chevron'))
      fireEvent.click(getByTestId('toolbar-redact-option-pixelated'))
      expect(onChangeRedactionStyle).toHaveBeenCalledWith('pixelated')
      expect(onSave).not.toHaveBeenCalled()
      expect(queryByTestId('toolbar-redact-dropdown')).toBeNull()
    })

    it('marks the active redaction style option', () => {
      const { getByTestId } = render(
        <Toolbar {...defaultProps} rectCount={0} redactionStyle="solid" />
      )
      fireEvent.click(getByTestId('toolbar-redact-chevron'))
      expect(getByTestId('toolbar-redact-option-solid').dataset.active).toBe('true')
      expect(getByTestId('toolbar-redact-option-pixelated').dataset.active).toBe('false')
      expect(getByTestId('toolbar-redact-option-branded').dataset.active).toBe('false')
    })

    it('reflects redactMode=true with orange background on toggle button', () => {
      const { getByTestId } = render(
        <Toolbar {...defaultProps} rectCount={0} redactMode={true} />
      )
      const toggle = getByTestId('toolbar-redact-toggle')
      expect((toggle as HTMLElement).style.background).toBe('rgb(238, 102, 51)')
    })
  })
})
