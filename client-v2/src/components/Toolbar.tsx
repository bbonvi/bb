import { useState, useEffect, useCallback, useRef } from 'react'
import { Plus, Pencil, Trash2 } from 'lucide-react'
import { useStore } from '@/lib/store'
import { useDebouncedValue } from '@/hooks/useDebouncedValue'
import { useIsMobile } from '@/hooks/useResponsive'
import type { SearchQuery } from '@/lib/api'

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
  const isMobile = useIsMobile()
  const [filtersOpen, setFiltersOpen] = useState(!isMobile)
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
  const pinToUrl = useStore((s) => s.pinToUrl)
  const setCreateModalOpen = useStore((s) => s.setCreateModalOpen)
  const setBulkEditOpen = useStore((s) => s.setBulkEditOpen)
  const setBulkDeleteOpen = useStore((s) => s.setBulkDeleteOpen)

  // Primary search — semantic if enabled, keyword otherwise
  const primaryDelay = semanticEnabled ? 500 : 300
  const primaryField = semanticEnabled ? 'semantic' : 'keyword'
  const primaryExternal = searchQuery[primaryField] ?? ''
  const [debouncedPrimary, setLocalPrimary, localPrimary, flushPrimary] =
    useDebouncedValue(primaryExternal, primaryDelay)

  // Advanced filter fields
  const [debouncedTags, setLocalTags, localTags, flushTags] =
    useDebouncedValue(searchQuery.tags ?? '', 300)
  const [debouncedTitle, setLocalTitle, localTitle, flushTitle] =
    useDebouncedValue(searchQuery.title ?? '', 300)
  const [debouncedUrl, setLocalUrl, localUrl, flushUrl] =
    useDebouncedValue(searchQuery.url ?? '', 300)
  const [debouncedDescription, setLocalDescription, localDescription, flushDescription] =
    useDebouncedValue(searchQuery.description ?? '', 300)
  // keyword field shown in filters when semantic is the primary
  const [debouncedKeywordAlt, setLocalKeywordAlt, localKeywordAlt, flushKeywordAlt] =
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

  // Open filters panel if URL had advanced filters (values already in store from init)
  useEffect(() => {
    const p = new URLSearchParams(window.location.search)
    if (p.get('tags') || p.get('title') || p.get('url') || p.get('description')) {
      setFiltersOpen(true)
    }
  }, []) // eslint-disable-line react-hooks/exhaustive-deps

  // Column auto-sync now handled by BookmarkGrid via useAutoColumns + ResizeObserver

  const hasAdvancedFilters = !!debouncedTags || !!debouncedTitle || !!debouncedUrl || !!debouncedDescription || (semanticEnabled && !!debouncedKeywordAlt)
  const hasAnySearch = !!debouncedPrimary || hasAdvancedFilters

  const clearAll = useCallback(() => {
    flushPrimary('')
    flushTags('')
    flushTitle('')
    flushUrl('')
    flushDescription('')
    if (semanticEnabled) flushKeywordAlt('')
    setSearchQuery({})
    searchInputRef.current?.focus()
  }, [flushPrimary, flushTags, flushTitle, flushUrl, flushDescription, flushKeywordAlt, semanticEnabled, setSearchQuery])

  const matchedCount = bookmarks.length

  // Auto-open filters if advanced filters have values
  useEffect(() => {
    if (hasAdvancedFilters && !filtersOpen) setFiltersOpen(true)
  }, [hasAdvancedFilters]) // eslint-disable-line react-hooks/exhaustive-deps

  return (
    <header className="sticky top-0 z-40 bg-bg/95 backdrop-blur-md">
      {/* ── Search row ── */}
      <div className="flex items-center gap-2 px-3 py-2 sm:gap-3 sm:px-4 sm:py-2.5">
        {/* Search bar */}
        <div className="relative flex min-w-0 flex-1 items-center sm:max-w-2xl">
          <SearchIcon className="pointer-events-none absolute left-3 text-text-dim" />
          <input
            ref={searchInputRef}
            type="text"
            value={localPrimary}
            onChange={(e) => setLocalPrimary(e.target.value)}
            autoFocus
            placeholder={semanticEnabled ? 'Search semantically…' : 'Search bookmarks…'}
            className="h-9 w-full rounded-lg border border-white/[0.06] bg-surface pl-9 pr-10 text-sm text-text placeholder:text-text-dim outline-none transition-colors focus:border-hi-dim focus:bg-surface-hover"
          />
          {/* Filter toggle inside search bar */}
          <button
            tabIndex={-1}
            onClick={() => setFiltersOpen(!filtersOpen)}
            className={`absolute right-1.5 flex h-6 items-center gap-1 rounded-md px-1.5 text-xs transition-colors ${
              filtersOpen || hasAdvancedFilters
                ? 'bg-hi-dim text-text'
                : 'text-text-muted hover:text-text'
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
          <span className={hasAnySearch ? 'text-hi' : 'text-text-muted'}>
            {matchedCount}
          </span>
          <span className="text-text-dim">/</span>
          <span className="text-text-dim">{totalCount}</span>
        </div>

        {/* Clear all */}
        {hasAnySearch && (
          <button
            tabIndex={-1}
            onClick={clearAll}
            className="flex h-7 w-7 shrink-0 items-center justify-center rounded-md text-text-muted transition-colors hover:bg-surface-hover hover:text-text"
          >
            <XIcon />
          </button>
        )}

        {/* New bookmark */}
        <button
          tabIndex={-1}
          onClick={() => setCreateModalOpen(true)}
          className="flex h-7 shrink-0 items-center gap-1 rounded-md px-2 text-xs font-medium text-text-muted transition-colors hover:bg-surface-hover hover:text-text"
          title="New bookmark (Ctrl+N)"
        >
          <Plus className="h-3.5 w-3.5" />
          <span className="hidden sm:inline">New</span>
        </button>

        {/* Bulk actions — visible when non-semantic search active with results */}
        {hasAnySearch && matchedCount > 0 && (
          <>
            <div className="hidden h-5 w-px bg-white/[0.06] sm:block shrink-0" />
            {searchQuery.semantic ? (
              <span className="hidden sm:block text-[10px] text-text-dim shrink-0" title="Bulk operations are not available during semantic search">
                Bulk N/A
              </span>
            ) : (
              <div className="hidden sm:flex items-center gap-1 shrink-0">
                <button
                  tabIndex={-1}
                  onClick={() => setBulkEditOpen(true)}
                  className="flex h-7 items-center gap-1 rounded-md px-2 text-xs font-medium text-text-muted transition-colors hover:bg-surface-hover hover:text-text"
                  title="Bulk edit matched bookmarks"
                >
                  <Pencil className="h-3 w-3" />
                  <span>Edit</span>
                </button>
                <button
                  tabIndex={-1}
                  onClick={() => setBulkDeleteOpen(true)}
                  className="flex h-7 items-center gap-1 rounded-md px-2 text-xs font-medium text-danger/70 transition-colors hover:bg-danger/10 hover:text-danger"
                  title="Bulk delete matched bookmarks"
                >
                  <Trash2 className="h-3 w-3" />
                  <span>Delete</span>
                </button>
              </div>
            )}
          </>
        )}

        {/* Divider — hidden on mobile */}
        <div className="hidden h-5 w-px bg-white/[0.06] sm:block shrink-0" />

        {/* View mode — hidden on mobile, shown in controls row */}
        <div className="hidden sm:flex items-center rounded-lg bg-surface p-0.5 shrink-0">
          {(['grid', 'cards', 'table'] as const).map((mode) => (
            <button
              key={mode}
              tabIndex={-1}
              onClick={() => setViewMode(mode)}
              className={`rounded-md px-2.5 py-1 text-xs font-medium transition-all ${
                viewMode === mode
                  ? 'bg-hi-dim text-text'
                  : 'text-text-muted hover:text-text'
              }`}
            >
              {mode === 'grid' ? 'Grid' : mode === 'cards' ? 'List' : 'Table'}
            </button>
          ))}
        </div>

        {/* Column stepper — hidden on mobile and non-grid views */}
        <div className={`items-center rounded-lg bg-surface shrink-0 ${viewMode === 'grid' ? 'hidden sm:flex' : 'hidden'}`}>
          <button
            tabIndex={-1}
            onClick={() => setColumns(Math.max(1, columns - 1))}
            className="flex h-7 w-7 items-center justify-center rounded-l-lg text-text-muted transition-colors hover:bg-surface-hover hover:text-text"
          >
            <MinusIcon />
          </button>
          <span className="flex h-7 min-w-[1.5rem] items-center justify-center border-x border-white/[0.04] font-mono text-xs tabular-nums text-text-muted">
            {columns}
          </span>
          <button
            tabIndex={-1}
            onClick={() => setColumns(Math.min(12, columns + 1))}
            className="flex h-7 w-7 items-center justify-center rounded-r-lg text-text-muted transition-colors hover:bg-surface-hover hover:text-text"
          >
            <PlusIcon />
          </button>
        </div>
      </div>

      {/* ── Mobile controls row ── */}
      <div className="flex items-center gap-2 border-t border-white/[0.04] px-3 py-1.5 sm:hidden">
        {/* View mode */}
        <div className="flex items-center rounded-lg bg-surface p-0.5">
          {(['grid', 'cards', 'table'] as const).map((mode) => (
            <button
              key={mode}
              tabIndex={-1}
              onClick={() => setViewMode(mode)}
              className={`rounded-md px-2 py-1 text-xs font-medium transition-all ${
                viewMode === mode
                  ? 'bg-hi-dim text-text'
                  : 'text-text-muted hover:text-text'
              }`}
            >
              {mode === 'grid' ? 'Grid' : mode === 'cards' ? 'List' : 'Table'}
            </button>
          ))}
        </div>

        <div className="flex-1" />

        {/* Toggles on mobile */}
        <div className="flex items-center gap-1.5">
          <PillToggle active={shuffle} onClick={() => setShuffle(!shuffle)} label="Shfl" />
          <PillToggle active={showAll} onClick={() => setShowAll(!showAll)} label="All" />
          <PillButton onClick={pinToUrl} label="Pin" />
        </div>
      </div>

      {/* ── Expandable filters ── */}
      <div
        className={`grid transition-[grid-template-rows] duration-200 ease-out ${
          filtersOpen ? 'grid-rows-[1fr]' : 'grid-rows-[0fr]'
        }`}
      >
        <div className="overflow-hidden">
          <div className="flex flex-wrap items-center gap-x-3 gap-y-2 border-t border-white/[0.04] px-3 py-2 sm:px-4 sm:py-2.5">
            {/* Advanced fields */}
            {semanticEnabled && (
              <FilterField label="keyword" value={localKeywordAlt} onChange={setLocalKeywordAlt} />
            )}
            <FilterField label="tags" value={localTags} onChange={setLocalTags} />
            <FilterField label="title" value={localTitle} onChange={setLocalTitle} />
            <FilterField label="url" value={localUrl} onChange={setLocalUrl} />
            <FilterField label="description" value={localDescription} onChange={setLocalDescription} />

            {/* Toggles — desktop only (mobile has them in controls row) */}
            <div className="ml-auto hidden sm:flex items-center gap-3">
              <PillToggle active={shuffle} onClick={() => setShuffle(!shuffle)} label="Shuffle" />
              <PillToggle active={showAll} onClick={() => setShowAll(!showAll)} label="Show all" />
              <PillButton onClick={pinToUrl} label="Pin" />
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
    <label className="flex items-center gap-1.5 min-w-0 w-[calc(50%-0.75rem)] sm:w-auto">
      <span className="text-[11px] font-medium uppercase tracking-wider text-text-dim select-none shrink-0">
        {label}
      </span>
      <input
        type="text"
        value={value}
        onChange={(e) => onChange(e.target.value)}
        className={`h-7 w-full sm:w-28 rounded-md border bg-transparent px-2 text-sm outline-none transition-colors ${
          value
            ? 'border-hi/20 text-text'
            : 'border-white/[0.06] text-text placeholder:text-text-dim'
        } focus:border-hi-dim focus:bg-surface`}
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
      tabIndex={-1}
      onClick={onClick}
      className={`rounded-full px-2.5 py-1 text-xs font-medium transition-all select-none ${
        active
          ? 'bg-hi-dim text-text'
          : 'text-text-muted hover:text-text hover:bg-surface-hover'
      }`}
    >
      {label}
    </button>
  )
}

function PillButton({ onClick, label }: { onClick: () => void; label: string }) {
  return (
    <button
      tabIndex={-1}
      onClick={onClick}
      className="rounded-full px-2.5 py-1 text-xs font-medium text-text-muted transition-all select-none hover:text-text hover:bg-surface-hover active:bg-hi-dim active:text-text"
    >
      {label}
    </button>
  )
}
