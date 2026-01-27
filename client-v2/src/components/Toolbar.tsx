import { useState, useEffect, useCallback, useRef } from 'react'
import { useStore } from '@/lib/store'
import { useDebouncedValue } from '@/hooks/useDebouncedValue'
import type { SearchQuery } from '@/lib/api'

// ─── Responsive columns ────────────────────────────────────────────
function useResponsiveColumns(): number {
  if (typeof window === 'undefined') return 3
  const w = window.innerWidth
  if (w < 640) return 1
  if (w < 1024) return 2
  if (w < 1440) return 3
  if (w < 1920) return 4
  return 5
}

// ─── Icons (inline SVG) ────────────────────────────────────────────
function SearchIcon({ className = '' }: { className?: string }) {
  return (
    <svg className={className} width="16" height="16" viewBox="0 0 16 16" fill="none" stroke="currentColor" strokeWidth="1.5" strokeLinecap="round">
      <circle cx="7" cy="7" r="4.5" />
      <path d="M10.5 10.5L14 14" />
    </svg>
  )
}

function FilterIcon({ className = '' }: { className?: string }) {
  return (
    <svg className={className} width="16" height="16" viewBox="0 0 16 16" fill="none" stroke="currentColor" strokeWidth="1.5" strokeLinecap="round">
      <path d="M2 4h12M4 8h8M6 12h4" />
    </svg>
  )
}

function XIcon({ className = '' }: { className?: string }) {
  return (
    <svg className={className} width="14" height="14" viewBox="0 0 14 14" fill="none" stroke="currentColor" strokeWidth="1.5" strokeLinecap="round">
      <path d="M3 3l8 8M11 3l-8 8" />
    </svg>
  )
}

function MinusIcon() {
  return (
    <svg width="14" height="14" viewBox="0 0 14 14" fill="none" stroke="currentColor" strokeWidth="1.5" strokeLinecap="round">
      <path d="M3 7h8" />
    </svg>
  )
}

function PlusIcon() {
  return (
    <svg width="14" height="14" viewBox="0 0 14 14" fill="none" stroke="currentColor" strokeWidth="1.5" strokeLinecap="round">
      <path d="M7 3v8M3 7h8" />
    </svg>
  )
}

// ─── Main component ────────────────────────────────────────────────
export function Toolbar() {
  const [filtersOpen, setFiltersOpen] = useState(false)
  const searchInputRef = useRef<HTMLInputElement>(null)

  const searchQuery = useStore((s) => s.searchQuery)
  const setSearchQuery = useStore((s) => s.setSearchQuery)
  const bookmarks = useStore((s) => s.bookmarks)
  const totalCount = useStore((s) => s.totalCount)
  const semanticEnabled = useStore((s) => s.semanticEnabled)

  const viewMode = useStore((s) => s.viewMode)
  const setViewMode = useStore((s) => s.setViewMode)
  const columns = useStore((s) => s.columns)
  const setColumns = useStore((s) => s.setColumns)
  const shuffle = useStore((s) => s.shuffle)
  const setShuffle = useStore((s) => s.setShuffle)
  const showAll = useStore((s) => s.showAll)
  const setShowAll = useStore((s) => s.setShowAll)
  const saveQueries = useStore((s) => s.saveQueries)
  const setSaveQueries = useStore((s) => s.setSaveQueries)

  // Primary search — semantic if enabled, keyword otherwise
  const primaryDelay = semanticEnabled ? 500 : 300
  const primaryField = semanticEnabled ? 'semantic' : 'keyword'
  const primaryExternal = searchQuery[primaryField] ?? ''
  const [debouncedPrimary, setLocalPrimary, localPrimary] =
    useDebouncedValue(primaryExternal, primaryDelay)

  // Advanced filter fields
  const [debouncedTags, setLocalTags, localTags] =
    useDebouncedValue(searchQuery.tags ?? '', 300)
  const [debouncedTitle, setLocalTitle, localTitle] =
    useDebouncedValue(searchQuery.title ?? '', 300)
  const [debouncedUrl, setLocalUrl, localUrl] =
    useDebouncedValue(searchQuery.url ?? '', 300)
  const [debouncedDescription, setLocalDescription, localDescription] =
    useDebouncedValue(searchQuery.description ?? '', 300)
  // keyword field shown in filters when semantic is the primary
  const [debouncedKeywordAlt, setLocalKeywordAlt, localKeywordAlt] =
    useDebouncedValue(searchQuery.keyword ?? '', 300)

  // Apply debounced values to store
  useEffect(() => {
    const query: SearchQuery = {}
    if (debouncedPrimary) query[primaryField] = debouncedPrimary
    if (semanticEnabled && debouncedKeywordAlt) query.keyword = debouncedKeywordAlt
    if (debouncedTags) query.tags = debouncedTags
    if (debouncedTitle) query.title = debouncedTitle
    if (debouncedUrl) query.url = debouncedUrl
    if (debouncedDescription) query.description = debouncedDescription
    setSearchQuery(query)
  }, [
    debouncedPrimary,
    debouncedKeywordAlt,
    debouncedTags,
    debouncedTitle,
    debouncedUrl,
    debouncedDescription,
    primaryField,
    semanticEnabled,
    setSearchQuery,
  ])

  // URL param sync
  useEffect(() => {
    if (!saveQueries) return
    const url = new URL(window.location.href)
    const fields: Record<string, string> = {
      tags: debouncedTags,
      title: debouncedTitle,
      url: debouncedUrl,
      description: debouncedDescription,
    }
    for (const [key, val] of Object.entries(fields)) {
      if (val) url.searchParams.set(key, val)
      else url.searchParams.delete(key)
    }
    if (showAll) url.searchParams.set('all', '1')
    else url.searchParams.delete('all')
    window.history.replaceState({}, '', url)
  }, [saveQueries, debouncedTags, debouncedTitle, debouncedUrl, debouncedDescription, showAll])

  // Restore from URL on mount
  useEffect(() => {
    const params = new URLSearchParams(window.location.search)
    const t = params.get('tags') ?? ''
    const ti = params.get('title') ?? ''
    const u = params.get('url') ?? ''
    const d = params.get('description') ?? ''
    const all = params.get('all')
    if (t) { setLocalTags(t); setFiltersOpen(true) }
    if (ti) { setLocalTitle(ti); setFiltersOpen(true) }
    if (u) { setLocalUrl(u); setFiltersOpen(true) }
    if (d) { setLocalDescription(d); setFiltersOpen(true) }
    if (all === '1' || all === 'true') setShowAll(true)
  }, []) // eslint-disable-line react-hooks/exhaustive-deps

  // Responsive columns on mount
  useEffect(() => {
    setColumns(useResponsiveColumns())
  }, []) // eslint-disable-line react-hooks/exhaustive-deps

  const hasAdvancedFilters = !!debouncedTags || !!debouncedTitle || !!debouncedUrl || !!debouncedDescription || (semanticEnabled && !!debouncedKeywordAlt)
  const hasAnySearch = !!debouncedPrimary || hasAdvancedFilters

  const clearAll = useCallback(() => {
    setLocalPrimary('')
    setLocalTags('')
    setLocalTitle('')
    setLocalUrl('')
    setLocalDescription('')
    if (semanticEnabled) setLocalKeywordAlt('')
    searchInputRef.current?.focus()
  }, [setLocalPrimary, setLocalTags, setLocalTitle, setLocalUrl, setLocalDescription, setLocalKeywordAlt, semanticEnabled])

  const matchedCount = bookmarks.length

  // Auto-open filters if advanced filters have values
  useEffect(() => {
    if (hasAdvancedFilters && !filtersOpen) setFiltersOpen(true)
  }, [hasAdvancedFilters]) // eslint-disable-line react-hooks/exhaustive-deps

  return (
    <header className="sticky top-0 z-40 bg-bg/95 backdrop-blur-md">
      {/* ── Row 1: Main bar ── */}
      <div className="flex items-center gap-3 px-4 py-2.5">
        {/* Search bar */}
        <div className="relative flex min-w-0 flex-1 items-center max-w-2xl">
          <SearchIcon className="pointer-events-none absolute left-3 text-text-dim" />
          <input
            ref={searchInputRef}
            type="text"
            value={localPrimary}
            onChange={(e) => setLocalPrimary(e.target.value)}
            placeholder={semanticEnabled ? 'Search semantically…' : 'Search bookmarks…'}
            className="h-9 w-full rounded-lg border border-white/[0.06] bg-surface pl-9 pr-10 text-sm text-text placeholder:text-text-dim outline-none transition-colors focus:border-accent-dim focus:bg-surface-hover"
          />
          {/* Filter toggle inside search bar */}
          <button
            onClick={() => setFiltersOpen(!filtersOpen)}
            className={`absolute right-1.5 flex h-6 items-center gap-1 rounded-md px-1.5 text-xs transition-colors ${
              filtersOpen || hasAdvancedFilters
                ? 'bg-accent-subtle text-accent'
                : 'text-text-dim hover:text-text-muted'
            }`}
          >
            <FilterIcon className="shrink-0" />
            {hasAdvancedFilters && (
              <span className="font-mono text-[10px]">
                {[debouncedTags, debouncedTitle, debouncedUrl, debouncedDescription, semanticEnabled ? debouncedKeywordAlt : ''].filter(Boolean).length}
              </span>
            )}
          </button>
        </div>

        {/* Counter */}
        <div className="flex items-baseline gap-0.5 font-mono text-xs tabular-nums select-none shrink-0">
          <span className={hasAnySearch ? 'text-accent' : 'text-text-muted'}>
            {matchedCount}
          </span>
          <span className="text-text-dim">/</span>
          <span className="text-text-dim">{totalCount}</span>
        </div>

        {/* Clear all */}
        {hasAnySearch && (
          <button
            onClick={clearAll}
            className="flex h-7 w-7 shrink-0 items-center justify-center rounded-md text-text-dim transition-colors hover:bg-surface-hover hover:text-text-muted"
          >
            <XIcon />
          </button>
        )}

        {/* Divider */}
        <div className="h-5 w-px bg-white/[0.06] shrink-0" />

        {/* View mode */}
        <div className="flex items-center rounded-lg bg-surface p-0.5 shrink-0">
          {(['grid', 'cards', 'table'] as const).map((mode) => (
            <button
              key={mode}
              onClick={() => setViewMode(mode)}
              className={`rounded-md px-2.5 py-1 text-xs font-medium transition-all ${
                viewMode === mode
                  ? 'bg-surface-active text-text shadow-sm'
                  : 'text-text-dim hover:text-text-muted'
              }`}
            >
              {mode === 'grid' ? 'Grid' : mode === 'cards' ? 'List' : 'Table'}
            </button>
          ))}
        </div>

        {/* Column stepper */}
        <div className="flex items-center rounded-lg bg-surface shrink-0">
          <button
            onClick={() => setColumns(Math.max(1, columns - 1))}
            className="flex h-7 w-7 items-center justify-center rounded-l-lg text-text-dim transition-colors hover:bg-surface-hover hover:text-text-muted"
          >
            <MinusIcon />
          </button>
          <span className="flex h-7 min-w-[1.5rem] items-center justify-center border-x border-white/[0.04] font-mono text-xs tabular-nums text-text-muted">
            {columns}
          </span>
          <button
            onClick={() => setColumns(Math.min(12, columns + 1))}
            className="flex h-7 w-7 items-center justify-center rounded-r-lg text-text-dim transition-colors hover:bg-surface-hover hover:text-text-muted"
          >
            <PlusIcon />
          </button>
        </div>
      </div>

      {/* ── Row 2: Expandable filters ── */}
      <div
        className={`grid transition-[grid-template-rows] duration-200 ease-out ${
          filtersOpen ? 'grid-rows-[1fr]' : 'grid-rows-[0fr]'
        }`}
      >
        <div className="overflow-hidden">
          <div className="flex flex-wrap items-center gap-x-3 gap-y-2 border-t border-white/[0.04] px-4 py-2.5">
            {/* Advanced fields */}
            {semanticEnabled && (
              <FilterField label="keyword" value={localKeywordAlt} onChange={setLocalKeywordAlt} />
            )}
            <FilterField label="tags" value={localTags} onChange={setLocalTags} />
            <FilterField label="title" value={localTitle} onChange={setLocalTitle} />
            <FilterField label="url" value={localUrl} onChange={setLocalUrl} />
            <FilterField label="description" value={localDescription} onChange={setLocalDescription} />

            {/* Toggles */}
            <div className="ml-auto flex items-center gap-3">
              <PillToggle active={shuffle} onClick={() => setShuffle(!shuffle)} label="Shuffle" />
              <PillToggle active={showAll} onClick={() => setShowAll(!showAll)} label="Show all" />
              <PillToggle active={saveQueries} onClick={() => setSaveQueries(!saveQueries)} label="Pin" />
            </div>
          </div>
        </div>
      </div>

      {/* Bottom edge */}
      <div className="h-px bg-white/[0.06]" />
    </header>
  )
}

// ─── Filter field ──────────────────────────────────────────────────
function FilterField({
  label,
  value,
  onChange,
}: {
  label: string
  value: string
  onChange: (v: string) => void
}) {
  return (
    <label className="flex items-center gap-1.5">
      <span className="text-[11px] font-medium uppercase tracking-wider text-text-dim select-none">
        {label}
      </span>
      <input
        type="text"
        value={value}
        onChange={(e) => onChange(e.target.value)}
        className={`h-7 w-28 rounded-md border bg-transparent px-2 text-sm outline-none transition-colors ${
          value
            ? 'border-accent/20 text-text'
            : 'border-white/[0.06] text-text placeholder:text-text-dim'
        } focus:border-accent-dim focus:bg-surface`}
      />
    </label>
  )
}

// ─── Pill toggle ───────────────────────────────────────────────────
function PillToggle({
  active,
  onClick,
  label,
}: {
  active: boolean
  onClick: () => void
  label: string
}) {
  return (
    <button
      onClick={onClick}
      className={`rounded-full px-2.5 py-0.5 text-xs font-medium transition-all select-none ${
        active
          ? 'bg-accent-subtle text-accent'
          : 'text-text-dim hover:text-text-muted'
      }`}
    >
      {label}
    </button>
  )
}
