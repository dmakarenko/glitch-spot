// ABOUTME: Tests for the Toolbar split button — save action, format dropdown, and default tracking.
// @vitest-environment jsdom

import { describe, it, expect, vi, afterEach } from 'vitest'
import { render, fireEvent, cleanup } from '@testing-library/react'
import { Toolbar } from './Toolbar'

afterEach(cleanup)

describe('Toolbar', () => {
  it('renders nothing when rectCount is 0', () => {
    const { container } = render(
      <Toolbar rectCount={0} clipboardFormat="json" onSave={vi.fn()} onChangeFormat={vi.fn()} />
    )
    expect(container.firstChild).toBeNull()
  })

  it('renders the save button with the current format label', () => {
    const { getByText } = render(
      <Toolbar rectCount={2} clipboardFormat="json" onSave={vi.fn()} onChangeFormat={vi.fn()} />
    )
    expect(getByText(/Copy JSON/)).toBeTruthy()
  })

  it('renders image label when format is image', () => {
    const { getByText } = render(
      <Toolbar rectCount={1} clipboardFormat="image" onSave={vi.fn()} onChangeFormat={vi.fn()} />
    )
    expect(getByText(/Copy Image/)).toBeTruthy()
  })

  it('calls onSave when left button is clicked', () => {
    const onSave = vi.fn()
    const { getByTestId } = render(
      <Toolbar rectCount={1} clipboardFormat="json" onSave={onSave} onChangeFormat={vi.fn()} />
    )
    fireEvent.click(getByTestId('toolbar-save-btn'))
    expect(onSave).toHaveBeenCalledTimes(1)
  })

  it('opens dropdown when chevron is clicked', () => {
    const { getByTestId, queryByTestId } = render(
      <Toolbar rectCount={1} clipboardFormat="json" onSave={vi.fn()} onChangeFormat={vi.fn()} />
    )
    expect(queryByTestId('toolbar-dropdown')).toBeNull()
    fireEvent.click(getByTestId('toolbar-chevron'))
    expect(queryByTestId('toolbar-dropdown')).toBeTruthy()
  })

  it('calls onChangeFormat with the selected format and closes dropdown', () => {
    const onChangeFormat = vi.fn()
    const { getByTestId, queryByTestId } = render(
      <Toolbar rectCount={1} clipboardFormat="json" onSave={vi.fn()} onChangeFormat={onChangeFormat} />
    )
    fireEvent.click(getByTestId('toolbar-chevron'))
    fireEvent.click(getByTestId('toolbar-option-image'))
    expect(onChangeFormat).toHaveBeenCalledWith('image')
    expect(queryByTestId('toolbar-dropdown')).toBeNull()
  })

  it('shows a checkmark on the active format option', () => {
    const { getByTestId } = render(
      <Toolbar rectCount={1} clipboardFormat="image" onSave={vi.fn()} onChangeFormat={vi.fn()} />
    )
    fireEvent.click(getByTestId('toolbar-chevron'))
    expect(getByTestId('toolbar-option-image').dataset.active).toBe('true')
    expect(getByTestId('toolbar-option-json').dataset.active).toBe('false')
  })

  it('prompt option is rendered but disabled', () => {
    const onChangeFormat = vi.fn()
    const { getByTestId } = render(
      <Toolbar rectCount={1} clipboardFormat="json" onSave={vi.fn()} onChangeFormat={onChangeFormat} />
    )
    fireEvent.click(getByTestId('toolbar-chevron'))
    const promptOption = getByTestId('toolbar-option-prompt')
    expect(promptOption.dataset.disabled).toBe('true')
    fireEvent.click(promptOption)
    expect(onChangeFormat).not.toHaveBeenCalled()
  })
})
