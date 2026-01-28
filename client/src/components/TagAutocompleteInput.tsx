import { useState, useMemo, useRef, useCallback, useLayoutEffect } from 'react'
import { createPortal } from 'react-dom'

/**
 * A text input with tag autocomplete for comma/space-separated tag values.
 * Suggests tags matching the last token being typed.
 * Uses a portal so the dropdown escapes overflow-hidden containers.
 */
export function TagAutocompleteInput({
  value,
  onChange,
  availableTags,
  placeholder,
  className,
  inputClassName,
}: {
  value: string
  onChange: (v: string) => void
  availableTags: string[]
  placeholder?: string
  className?: string
  inputClassName?: string
}) {
  'use no memo' // Opt out of React Compiler - DOM measurements require refs in effects
  const [showSuggestions, setShowSuggestions] = useState(false)
  const [highlightIdx, setHighlightIdx] = useState(-1)
  const [dropdownPos, setDropdownPos] = useState<{ top: number; left: number; width: number } | null>(null)
  const inputRef = useRef<HTMLInputElement>(null)

  // Parse the current token (last segment after comma/space)
  const { prefix, currentToken } = useMemo(() => {
    const lastDelim = Math.max(value.lastIndexOf(','), value.lastIndexOf(' '))
    if (lastDelim === -1) return { prefix: '', currentToken: value.trim() }
    return {
      prefix: value.slice(0, lastDelim + 1) + (value[lastDelim] === ',' ? ' ' : ''),
      currentToken: value.slice(lastDelim + 1).trim(),
    }
  }, [value])

  // Already-entered tags (to exclude from suggestions)
  const enteredTags = useMemo(() => {
    return new Set(
      value
        .split(/[\s,]+/)
        .map((t) => t.trim().toLowerCase())
        .filter(Boolean),
    )
  }, [value])

  const suggestions = useMemo(() => {
    if (!currentToken) return []
    const lower = currentToken.toLowerCase()
    return availableTags
      .filter((t) => t.toLowerCase().includes(lower) && !enteredTags.has(t.toLowerCase()))
      .slice(0, 8)
  }, [currentToken, availableTags, enteredTags])

  const visible = showSuggestions && suggestions.length > 0

  // Position the portal dropdown relative to the input (DOM measurement requires effect + state)
  useLayoutEffect(() => {
    if (!visible || !inputRef.current) {
      setDropdownPos(null) // eslint-disable-line react-hooks/set-state-in-effect
      return
    }
    const rect = inputRef.current.getBoundingClientRect()
    setDropdownPos({
      top: rect.bottom + 4,
      left: rect.left,
      width: rect.width,
    })
  }, [visible, value])

  const selectSuggestion = useCallback(
    (tag: string) => {
      const newValue = prefix + tag + ', '
      onChange(newValue)
      setShowSuggestions(false)
      setHighlightIdx(-1)
      inputRef.current?.focus()
    },
    [prefix, onChange],
  )

  const handleKeyDown = useCallback(
    (e: React.KeyboardEvent) => {
      if (!visible) return

      if (e.key === 'ArrowDown') {
        e.preventDefault()
        setHighlightIdx((i) => (i + 1) % suggestions.length)
      } else if (e.key === 'ArrowUp') {
        e.preventDefault()
        setHighlightIdx((i) => (i <= 0 ? suggestions.length - 1 : i - 1))
      } else if (e.key === 'Enter' && highlightIdx >= 0) {
        e.preventDefault()
        selectSuggestion(suggestions[highlightIdx])
      } else if (e.key === 'Escape') {
        setShowSuggestions(false)
        setHighlightIdx(-1)
      }
    },
    [visible, suggestions, highlightIdx, selectSuggestion],
  )

  const dropdown =
    visible && dropdownPos
      ? createPortal(
          <div
            style={{
              position: 'fixed',
              top: dropdownPos.top,
              left: dropdownPos.left,
              width: dropdownPos.width,
              minWidth: 160,
            }}
            className="z-[9999] max-h-40 overflow-y-auto rounded-md border border-white/[0.08] bg-surface shadow-lg"
          >
            {suggestions.map((s, i) => (
              <button
                key={s}
                onMouseDown={(e) => {
                  e.preventDefault()
                  selectSuggestion(s)
                }}
                className={`block w-full px-2.5 py-1.5 text-left text-xs transition-colors ${
                  i === highlightIdx
                    ? 'bg-surface-hover text-text'
                    : 'text-text-muted hover:bg-surface-hover hover:text-text'
                }`}
              >
                {s}
              </button>
            ))}
          </div>,
          document.body,
        )
      : null

  return (
    <div className={`relative ${className ?? ''}`}>
      <input
        ref={inputRef}
        type="text"
        value={value}
        onChange={(e) => {
          onChange(e.target.value)
          setShowSuggestions(true)
          setHighlightIdx(-1)
        }}
        onFocus={() => setShowSuggestions(true)}
        onBlur={() => setTimeout(() => setShowSuggestions(false), 150)}
        onKeyDown={handleKeyDown}
        placeholder={placeholder}
        className={inputClassName}
      />
      {dropdown}
    </div>
  )
}
