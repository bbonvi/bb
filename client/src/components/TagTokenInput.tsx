import { useState, useLayoutEffect, useMemo, useCallback, useRef } from 'react'
import { createPortal } from 'react-dom'
import { X } from 'lucide-react'

/**
 * Inline token field for tag editing.
 * Chips render inside the input. Space/Enter/Tab commit tags.
 * Backspace with empty input does staged-delete of last tag.
 * Autocomplete dropdown with first item pre-selected.
 * Dropdown rendered via portal so it escapes overflow-hidden and modals.
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

  const tagsSet = useMemo(() => new Set(tags.map((t) => t.toLowerCase())), [tags])

  const suggestions = useMemo(() => {
    if (!input.trim()) return []
    const lower = input.toLowerCase()
    return availableTags
      .filter((t) => t.toLowerCase().includes(lower) && !tagsSet.has(t.toLowerCase()))
      .slice(0, 8)
  }, [input, availableTags, tagsSet])

  const visible = showSuggestions && suggestions.length > 0

  // Measure container rect and position the portal dropdown
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

  // Reposition on visibility, input change, scroll, resize, and mobile keyboard
  useLayoutEffect(() => {
    if (!visible) {
      setDropdownPos(null) // eslint-disable-line react-hooks/set-state-in-effect
      return
    }
    updateDropdownPos() // eslint-disable-line react-hooks/set-state-in-effect

    const vv = window.visualViewport
    const scrollParents: EventTarget[] = []

    // Walk up to find scrollable ancestors
    let el: HTMLElement | null = containerRef.current
    while (el) {
      if (el.scrollHeight > el.clientHeight || el.scrollWidth > el.clientWidth) {
        scrollParents.push(el)
      }
      el = el.parentElement
    }

    window.addEventListener('scroll', updateDropdownPos, true)
    window.addEventListener('resize', updateDropdownPos)
    vv?.addEventListener('resize', updateDropdownPos)
    vv?.addEventListener('scroll', updateDropdownPos)
    for (const sp of scrollParents) {
      sp.addEventListener('scroll', updateDropdownPos)
    }

    return () => {
      window.removeEventListener('scroll', updateDropdownPos, true)
      window.removeEventListener('resize', updateDropdownPos)
      vv?.removeEventListener('resize', updateDropdownPos)
      vv?.removeEventListener('scroll', updateDropdownPos)
      for (const sp of scrollParents) {
        sp.removeEventListener('scroll', updateDropdownPos)
      }
    }
  }, [visible, updateDropdownPos, input])

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
      {dropdown}
    </div>
  )
}
