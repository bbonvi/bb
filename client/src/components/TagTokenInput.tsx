import { useState, useEffect, useMemo, useCallback, useRef } from 'react'
import { X } from 'lucide-react'

/**
 * Inline token field for tag editing.
 * Chips render inside the input. Space/Enter/Tab commit tags.
 * Backspace with empty input does staged-delete of last tag.
 * Autocomplete dropdown with first item pre-selected.
 */
export function TagTokenInput({
  tags,
  onChange,
  availableTags = [],
  placeholder = 'Add tag',
  className,
}: {
  tags: string[]
  onChange: (tags: string[]) => void
  availableTags?: string[]
  placeholder?: string
  className?: string
}) {
  const [input, setInput] = useState('')
  const [showSuggestions, setShowSuggestions] = useState(false)
  const [highlightIdx, setHighlightIdx] = useState(0)
  const [stagedDelete, setStagedDelete] = useState(false)
  const [dropUp, setDropUp] = useState(false)
  const inputRef = useRef<HTMLInputElement>(null)
  const containerRef = useRef<HTMLDivElement>(null)

  const tagsSet = useMemo(() => new Set(tags.map((t) => t.toLowerCase())), [tags])

  const suggestions = useMemo(() => {
    if (!input.trim()) return []
    const lower = input.toLowerCase()
    return availableTags
      .filter((t) => t.toLowerCase().includes(lower) && !tagsSet.has(t.toLowerCase()))
      .slice(0, 8)
  }, [input, availableTags, tagsSet])

  const visible = showSuggestions && suggestions.length > 0

  // Recompute drop direction
  const recomputeDropDirection = useCallback(() => {
    if (!containerRef.current) return
    const rect = containerRef.current.getBoundingClientRect()
    const vh = window.visualViewport?.height ?? window.innerHeight
    setDropUp(vh - rect.bottom < 160)
  }, [])

  useEffect(() => {
    if (!visible) return
    recomputeDropDirection()
    const vv = window.visualViewport
    window.addEventListener('resize', recomputeDropDirection)
    vv?.addEventListener('resize', recomputeDropDirection)
    return () => {
      window.removeEventListener('resize', recomputeDropDirection)
      vv?.removeEventListener('resize', recomputeDropDirection)
    }
  }, [visible, recomputeDropDirection])

  const addTag = useCallback(
    (tag: string) => {
      const trimmed = tag.trim()
      if (!trimmed || tagsSet.has(trimmed.toLowerCase())) return
      onChange([...tags, trimmed])
    },
    [tags, onChange, tagsSet],
  )

  const removeTag = useCallback(
    (tag: string) => {
      onChange(tags.filter((t) => t !== tag))
    },
    [tags, onChange],
  )

  const commitTag = useCallback(
    (raw: string) => {
      const cleaned = raw.replace(/,/g, ' ').trim()
      if (cleaned) {
        for (const t of cleaned.split(/\s+/)) {
          if (t) addTag(t)
        }
      }
      setInput('')
      setShowSuggestions(false)
      setHighlightIdx(0)
    },
    [addTag],
  )

  const handleKeyDown = useCallback(
    (e: React.KeyboardEvent) => {
      if (visible) {
        if (e.key === 'ArrowDown') {
          e.preventDefault()
          setHighlightIdx((i) => (i + 1) % suggestions.length)
          return
        }
        if (e.key === 'ArrowUp') {
          e.preventDefault()
          setHighlightIdx((i) => (i <= 0 ? suggestions.length - 1 : i - 1))
          return
        }
        if (e.key === 'Tab' || e.key === 'Enter') {
          e.preventDefault()
          commitTag(suggestions[highlightIdx] ?? suggestions[0])
          return
        }
        if (e.key === 'Escape') {
          setShowSuggestions(false)
          setHighlightIdx(0)
          return
        }
      }

      if (e.key === 'Enter' && input.trim()) {
        e.preventDefault()
        commitTag(input)
        return
      }

      if (e.key === 'Backspace' && input === '' && tags.length > 0) {
        e.preventDefault()
        if (stagedDelete) {
          removeTag(tags[tags.length - 1])
          setStagedDelete(false)
        } else {
          setStagedDelete(true)
        }
      }
    },
    [visible, suggestions, highlightIdx, input, tags, stagedDelete, commitTag, removeTag],
  )

  const handleChange = useCallback(
    (e: React.ChangeEvent<HTMLInputElement>) => {
      const cleaned = e.target.value.replace(/,/g, ' ')
      if (cleaned.endsWith(' ') && cleaned.trim()) {
        commitTag(cleaned)
      } else {
        setInput(cleaned)
        setShowSuggestions(true)
        setHighlightIdx(0)
        setStagedDelete(false)
      }
    },
    [commitTag],
  )

  return (
    <div ref={containerRef} className={`relative ${className ?? ''}`}>
      <div
        className="flex min-h-[28px] cursor-text flex-wrap items-center gap-1 rounded-md border border-white/[0.06] bg-surface px-1.5 py-1 transition-colors focus-within:border-hi-dim"
        onClick={() => inputRef.current?.focus()}
      >
        {tags.map((tag, i) => (
          <span
            key={tag}
            className={`flex items-center gap-0.5 rounded px-1.5 py-px font-mono text-[11px] transition-colors ${
              stagedDelete && i === tags.length - 1
                ? 'bg-danger/20 text-danger'
                : 'bg-surface-active text-text-muted'
            }`}
          >
            {tag}
            <button
              onMouseDown={(e) => {
                e.preventDefault()
                e.stopPropagation()
                removeTag(tag)
              }}
              className="ml-0.5 text-text-dim transition-colors hover:text-danger"
              tabIndex={-1}
            >
              <X className="h-2.5 w-2.5" />
            </button>
          </span>
        ))}
        <input
          ref={inputRef}
          type="text"
          value={input}
          onChange={handleChange}
          onFocus={() => {
            setShowSuggestions(true)
            setHighlightIdx(0)
          }}
          onBlur={() => {
            setTimeout(() => setShowSuggestions(false), 150)
            setStagedDelete(false)
            if (input.trim()) commitTag(input)
          }}
          onKeyDown={handleKeyDown}
          className="min-w-[60px] flex-1 bg-transparent font-mono text-xs text-text outline-none placeholder:text-text-dim"
          placeholder={tags.length === 0 ? placeholder : ''}
        />
      </div>

      {visible && (
        <div
          className={`absolute left-0 z-50 max-h-32 w-full overflow-y-auto rounded-md border border-white/[0.08] bg-surface shadow-lg ${
            dropUp ? 'bottom-full mb-1' : 'top-full mt-1'
          }`}
        >
          {suggestions.map((s, i) => (
            <button
              key={s}
              onMouseDown={(e) => {
                e.preventDefault()
                commitTag(s)
              }}
              className={`block w-full px-2 py-1 text-left font-mono text-xs transition-colors ${
                i === highlightIdx
                  ? 'bg-surface-hover text-text'
                  : 'text-text-muted hover:bg-surface-hover hover:text-text'
              }`}
            >
              {s}
            </button>
          ))}
        </div>
      )}
    </div>
  )
}
