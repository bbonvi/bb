import { useState, useLayoutEffect, useMemo, useCallback, useRef } from 'react'
import { createPortal } from 'react-dom'
import { X } from 'lucide-react'

/**
 * Inline token field for tag editing.
 * Chips render inside the input container. The text input floats between
 * chips based on cursorIdx — click a chip or arrow-key to reposition.
 * Space/Enter/Tab commit tags. Backspace does staged-delete of the chip
 * immediately before the cursor. Autocomplete via portal.
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
  'use no memo' // Opt out of React Compiler — DOM measurements require refs in effects
  const [input, setInput] = useState('')
  const [cursorIdx, setCursorIdx] = useState(tags.length)
  const [showSuggestions, setShowSuggestions] = useState(false)
  const [highlightIdx, setHighlightIdx] = useState(0)
  const [stagedDelete, setStagedDelete] = useState(false)
  const [dropdownPos, setDropdownPos] = useState<{
    top: number
    left: number
    width: number
    dropUp: boolean
  } | null>(null)
  const inputRef = useRef<HTMLInputElement>(null)
  const containerRef = useRef<HTMLDivElement>(null)

  // Keep cursorIdx in bounds when tags change externally
  const clampedCursorIdx = Math.min(cursorIdx, tags.length)
  if (clampedCursorIdx !== cursorIdx) setCursorIdx(clampedCursorIdx) // eslint-disable-line react-hooks/set-state-in-render

  const tagsSet = useMemo(() => new Set(tags.map((t) => t.toLowerCase())), [tags])

  const suggestions = useMemo(() => {
    if (!input.trim()) return []
    const lower = input.toLowerCase()
    return availableTags
      .filter((t) => t.toLowerCase().includes(lower) && !tagsSet.has(t.toLowerCase()))
      .slice(0, 8)
  }, [input, availableTags, tagsSet])

  const visible = showSuggestions && suggestions.length > 0

  // ── Portal dropdown positioning ──────────────────────────────────
  const updateDropdownPos = useCallback(() => {
    if (!containerRef.current) {
      setDropdownPos(null)
      return
    }
    const rect = containerRef.current.getBoundingClientRect()
    const vh = window.visualViewport?.height ?? window.innerHeight
    const spaceBelow = vh - rect.bottom
    const dropUp = spaceBelow < 160 && rect.top > spaceBelow
    setDropdownPos({
      top: dropUp ? rect.top : rect.bottom + 2,
      left: rect.left,
      width: rect.width,
      dropUp,
    })
  }, [])

  useLayoutEffect(() => {
    if (!visible) {
      setDropdownPos(null) // eslint-disable-line react-hooks/set-state-in-effect
      return
    }
    updateDropdownPos() // eslint-disable-line react-hooks/set-state-in-effect

    const vv = window.visualViewport
    window.addEventListener('scroll', updateDropdownPos, true)
    window.addEventListener('resize', updateDropdownPos)
    vv?.addEventListener('resize', updateDropdownPos)
    vv?.addEventListener('scroll', updateDropdownPos)

    return () => {
      window.removeEventListener('scroll', updateDropdownPos, true)
      window.removeEventListener('resize', updateDropdownPos)
      vv?.removeEventListener('resize', updateDropdownPos)
      vv?.removeEventListener('scroll', updateDropdownPos)
    }
  }, [visible, updateDropdownPos, input])

  // ── Tag operations (cursor-aware) ────────────────────────────────
  const insertTag = useCallback(
    (tag: string) => {
      const trimmed = tag.trim()
      if (!trimmed || tagsSet.has(trimmed.toLowerCase())) return
      const next = [...tags]
      next.splice(clampedCursorIdx, 0, trimmed)
      onChange(next)
      setCursorIdx(clampedCursorIdx + 1)
    },
    [tags, onChange, tagsSet, clampedCursorIdx],
  )

  const removeAtIdx = useCallback(
    (idx: number) => {
      onChange(tags.filter((_, i) => i !== idx))
      if (idx < clampedCursorIdx) setCursorIdx(clampedCursorIdx - 1)
    },
    [tags, onChange, clampedCursorIdx],
  )

  const commitTag = useCallback(
    (raw: string) => {
      const cleaned = raw.replace(/,/g, ' ').trim()
      if (cleaned) {
        for (const t of cleaned.split(/\s+/)) {
          if (t) insertTag(t)
        }
      }
      setInput('')
      setShowSuggestions(false)
      setHighlightIdx(0)
    },
    [insertTag],
  )

  // ── Keyboard ─────────────────────────────────────────────────────
  const handleKeyDown = useCallback(
    (e: React.KeyboardEvent) => {
      // Autocomplete navigation
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

      // Commit raw text
      if (e.key === 'Enter' && input.trim()) {
        e.preventDefault()
        commitTag(input)
        return
      }

      const caretAtStart = inputRef.current?.selectionStart === 0 && inputRef.current?.selectionEnd === 0

      // Arrow Left at caret 0 with empty input → move cursor left among chips
      if (e.key === 'ArrowLeft' && input === '' && clampedCursorIdx > 0) {
        e.preventDefault()
        setCursorIdx(clampedCursorIdx - 1)
        setStagedDelete(false)
        return
      }
      // Arrow Left at caret 0 with text → move cursor left (let input handle if there's text and caret not at 0)
      if (e.key === 'ArrowLeft' && caretAtStart && input !== '' && clampedCursorIdx > 0) {
        // commit text first, then move
        e.preventDefault()
        commitTag(input)
        setCursorIdx(Math.max(0, clampedCursorIdx)) // commitTag already advanced, step back
        return
      }

      // Arrow Right at end of empty input → move cursor right among chips
      if (e.key === 'ArrowRight' && input === '' && clampedCursorIdx < tags.length) {
        e.preventDefault()
        setCursorIdx(clampedCursorIdx + 1)
        setStagedDelete(false)
        return
      }

      // Backspace → staged delete of chip before cursor
      if (e.key === 'Backspace' && input === '' && clampedCursorIdx > 0) {
        e.preventDefault()
        if (stagedDelete) {
          removeAtIdx(clampedCursorIdx - 1)
          setStagedDelete(false)
        } else {
          setStagedDelete(true)
        }
        return
      }

      // Delete key → remove chip after cursor
      if (e.key === 'Delete' && input === '' && clampedCursorIdx < tags.length) {
        e.preventDefault()
        removeAtIdx(clampedCursorIdx)
      }
    },
    [visible, suggestions, highlightIdx, input, tags, clampedCursorIdx, stagedDelete, commitTag, removeAtIdx],
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

  // ── Click handler: clicking a chip moves cursor to its position ──
  const handleChipClick = useCallback(
    (idx: number) => {
      setCursorIdx(idx)
      setStagedDelete(false)
      inputRef.current?.focus()
    },
    [],
  )

  const handleContainerClick = useCallback(
    (e: React.MouseEvent) => {
      const container = containerRef.current?.querySelector('[data-tag-field]') ?? containerRef.current
      if (!container) return
      const clickX = e.clientX
      // Find the nearest gap between chips by checking each child's midpoint
      const children = Array.from(container.children) as HTMLElement[]
      let bestIdx = tags.length
      let bestDist = Infinity
      for (let i = 0; i < children.length; i++) {
        const rect = children[i].getBoundingClientRect()
        const leftDist = Math.abs(clickX - rect.left)
        const rightDist = Math.abs(clickX - rect.right)
        if (leftDist < bestDist) {
          bestDist = leftDist
          // Map child index back to tag index (input element is in the list)
          bestIdx = i <= clampedCursorIdx ? i : i - 1
        }
        if (rightDist < bestDist) {
          bestDist = rightDist
          bestIdx = i < clampedCursorIdx ? i + 1 : i
        }
      }
      setCursorIdx(Math.max(0, Math.min(tags.length, bestIdx)))
      setStagedDelete(false)
      inputRef.current?.focus()
    },
    [tags.length, clampedCursorIdx],
  )

  // ── Build interleaved chips + input ──────────────────────────────
  const atEnd = clampedCursorIdx >= tags.length
  const inputElement = (
    <input
      key="__input__"
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
      style={atEnd ? undefined : { width: input ? `${input.length + 1}ch` : 0 }}
      className={`bg-transparent font-mono text-xs text-text outline-none placeholder:text-text-dim ${
        atEnd ? 'min-w-[60px] flex-1' : 'flex-none'
      }`}
      placeholder={tags.length === 0 ? placeholder : ''}
    />
  )

  const elements: React.ReactNode[] = []
  for (let i = 0; i <= tags.length; i++) {
    if (i === clampedCursorIdx) {
      elements.push(inputElement)
    }
    if (i < tags.length) {
      const tag = tags[i]
      const isStaged = stagedDelete && i === clampedCursorIdx - 1
      elements.push(
        <span
          key={tag}
          className={`flex cursor-pointer items-center gap-0.5 rounded px-1.5 py-px font-mono text-[11px] transition-colors ${
            isStaged ? 'bg-danger/20 text-danger' : 'bg-surface-active text-text-muted'
          }`}
          onMouseDown={(e) => {
            e.preventDefault()
            handleChipClick(i)
          }}
        >
          {tag}
          <button
            onMouseDown={(e) => {
              e.preventDefault()
              e.stopPropagation()
              removeAtIdx(i)
            }}
            className="ml-0.5 text-text-dim transition-colors hover:text-danger"
            tabIndex={-1}
          >
            <X className="h-2.5 w-2.5" />
          </button>
        </span>,
      )
    }
  }

  // ── Dropdown portal ──────────────────────────────────────────────
  const dropdown =
    visible && dropdownPos
      ? createPortal(
          <div
            style={{
              position: 'fixed',
              top: dropdownPos.dropUp ? undefined : dropdownPos.top,
              bottom: dropdownPos.dropUp
                ? (window.visualViewport?.height ?? window.innerHeight) - dropdownPos.top + 2
                : undefined,
              left: dropdownPos.left,
              width: dropdownPos.width,
              minWidth: 160,
              zIndex: 99999,
            }}
            className="max-h-32 overflow-y-auto rounded-md border border-white/[0.08] bg-surface shadow-lg"
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
          </div>,
          document.body,
        )
      : null

  return (
    <div ref={containerRef} className={className ?? ''}>
      <div
        data-tag-field
        className="flex min-h-[28px] cursor-text flex-wrap items-center gap-1 rounded-md border border-white/[0.06] bg-surface px-1.5 py-1 transition-colors focus-within:border-hi-dim"
        onClick={handleContainerClick}
      >
        {elements}
      </div>
      {dropdown}
    </div>
  )
}
