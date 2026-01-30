import { useState, useEffect, useCallback, useRef, useMemo, memo } from 'react'
import { Plus, Pencil, Trash2, Settings, ChevronDown } from 'lucide-react'
import { useStore } from '@/lib/store'
import { useDebouncedValue } from '@/hooks/useDebouncedValue'
import { useHiddenTags } from '@/hooks/useHiddenTags'
import { useDisplayBookmarks } from '@/hooks/useDisplayBookmarks'
import { useSettings } from '@/hooks/useSettings'
import { TagAutocompleteInput } from '@/components/TagAutocompleteInput'
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

// ─── Main component ────────────────────────────────────────────────
export function Toolbar() {
  // Compute initial filtersOpen: true only if URL has advanced filters
  const [filtersOpen, setFiltersOpen] = useState(() => {
    if (typeof window === 'undefined') return false
    const p = new URLSearchParams(window.location.search)
    return !!(p.get('tags') || p.get('title') || p.get('url') || p.get('description'))
  })
  const searchInputRef = useRef<HTMLInputElement>(null)

  const searchQuery = useStore((s) => s.searchQuery)
  const setSearchQuery = useStore((s) => s.setSearchQuery)
  const totalCount = useStore((s) => s.totalCount)
  const semanticEnabled = useStore((s) => s.semanticEnabled)
  const searchError = useStore((s) => s.searchError)
  const setSearchError = useStore((s) => s.setSearchError)

  const viewMode = useStore((s) => s.viewMode)
  const setViewMode = useStore((s) => s.setViewMode)
  const shuffle = useStore((s) => s.shuffle)
  const setShuffle = useStore((s) => s.setShuffle)
  const showAll = useStore((s) => s.showAll)
  const setShowAll = useStore((s) => s.setShowAll)
  const pinToUrl = useStore((s) => s.pinToUrl)
  const setCreateModalOpen = useStore((s) => s.setCreateModalOpen)
  const setBulkEditOpen = useStore((s) => s.setBulkEditOpen)
  const setBulkDeleteOpen = useStore((s) => s.setBulkDeleteOpen)
  const workspacesAvailable = useStore((s) => s.workspacesAvailable)
  const workspaces = useStore((s) => s.workspaces)
  const activeWorkspaceId = useStore((s) => s.activeWorkspaceId)
  const setActiveWorkspaceId = useStore((s) => s.setActiveWorkspaceId)
  const setSettingsOpen = useStore((s) => s.setSettingsOpen)
  const tags = useStore((s) => s.tags)
  const hiddenTags = useHiddenTags()
  const [settings] = useSettings()
  const autocompleteTags = useMemo(() => {
    const hidden = new Set(hiddenTags)
    return tags.filter((t) => !hidden.has(t))
  }, [tags, hiddenTags])

  // Clear search error when query changes
  const searchQueryRef = useRef(searchQuery)
  useEffect(() => {
    if (searchQueryRef.current !== searchQuery) {
      searchQueryRef.current = searchQuery
      if (searchError) setSearchError(null)
    }
  }, [searchQuery, searchError, setSearchError])

  const SEARCH_DEBOUNCE_MS = 500

  // Primary search — semantic if enabled, query otherwise
  const primaryField = semanticEnabled ? 'semantic' : 'query'
  const primaryExternal = searchQuery[primaryField] ?? ''
  const [debouncedPrimary, setLocalPrimary, localPrimary, flushPrimary] =
    useDebouncedValue(primaryExternal, SEARCH_DEBOUNCE_MS)

  // Advanced filter fields
  const [debouncedTags, setLocalTags, localTags, flushTags] =
    useDebouncedValue(searchQuery.tags ?? '', SEARCH_DEBOUNCE_MS)
  const [debouncedTitle, setLocalTitle, localTitle, flushTitle] =
    useDebouncedValue(searchQuery.title ?? '', SEARCH_DEBOUNCE_MS)
  const [debouncedUrl, setLocalUrl, localUrl, flushUrl] =
    useDebouncedValue(searchQuery.url ?? '', SEARCH_DEBOUNCE_MS)
  const [debouncedDescription, setLocalDescription, localDescription, flushDescription] =
    useDebouncedValue(searchQuery.description ?? '', SEARCH_DEBOUNCE_MS)
  // query field shown in filters when semantic is the primary
  const [debouncedQueryAlt, setLocalQueryAlt, localQueryAlt, flushQueryAlt] =
    useDebouncedValue(searchQuery.query ?? '', SEARCH_DEBOUNCE_MS)

  // Apply debounced values to store
  useEffect(() => {
    const query: SearchQuery = {}
    if (debouncedPrimary) query[primaryField] = debouncedPrimary
    if (semanticEnabled && debouncedQueryAlt) query.query = debouncedQueryAlt
    if (debouncedTags) query.tags = debouncedTags
    if (debouncedTitle) query.title = debouncedTitle
    if (debouncedUrl) query.url = debouncedUrl
    if (debouncedDescription) query.description = debouncedDescription

    // Guard: only update if query actually changed
    const current = useStore.getState().searchQuery
    const keys = new Set([...Object.keys(query), ...Object.keys(current)])
    let changed = false
    for (const k of keys) {
      if (query[k as keyof SearchQuery] !== current[k as keyof SearchQuery]) {
        changed = true
        break
      }
    }
    if (changed) setSearchQuery(query)
  }, [
    debouncedPrimary,
    debouncedQueryAlt,
    debouncedTags,
    debouncedTitle,
    debouncedUrl,
    debouncedDescription,
    primaryField,
    semanticEnabled,
    setSearchQuery,
  ])

  const hasAdvancedFilters = !!debouncedTags || !!debouncedTitle || !!debouncedUrl || !!debouncedDescription || (semanticEnabled && !!debouncedQueryAlt)
  const hasAnySearch = !!debouncedPrimary || hasAdvancedFilters

  const clearAll = useCallback(() => {
    flushPrimary('')
    flushTags('')
    flushTitle('')
    flushUrl('')
    flushDescription('')
    if (semanticEnabled) flushQueryAlt('')
    setSearchQuery({})
    searchInputRef.current?.focus()
  }, [flushPrimary, flushTags, flushTitle, flushUrl, flushDescription, flushQueryAlt, semanticEnabled, setSearchQuery])

  const { displayBookmarks } = useDisplayBookmarks()
  const matchedCount = displayBookmarks.length

  // Filters panel is shown if user toggled it open OR if advanced filters have values
  const showFilters = filtersOpen || hasAdvancedFilters

  return (
    <header className="sticky top-0 z-40 bg-bg">
      {/* ── Search row ── */}
      <div className="flex items-center gap-2 px-2 py-2 sm:gap-3 sm:px-3 sm:py-2.5">
        {/* Logo */}
        <button
          onClick={clearAll}
          className="hidden sm:flex h-9 w-9 shrink-0 items-center justify-center rounded-lg transition-all hover:scale-105 active:scale-95 group -translate-y-0.5"
          title="bb — clear search"
        >
          <img
            src="/logo192.png"
            alt="bb"
            className="h-7 w-7 opacity-70 grayscale-[30%] transition-all group-hover:opacity-100 group-hover:grayscale-0"
          />
        </button>

        {/* Search bar */}
        <div className="relative flex min-w-0 flex-1 items-center sm:max-w-2xl">
          <SearchIcon className="pointer-events-none absolute left-3 text-text-dim" />
          <input
            ref={searchInputRef}
            type="text"
            value={localPrimary}
            onChange={(e) => setLocalPrimary(e.target.value)}
            onKeyDown={(e) => {
              if (e.key === 'Escape') { e.stopPropagation(); searchInputRef.current?.select() }
              else if (e.key === 'Enter') {
                flushPrimary(localPrimary)
                const el = searchInputRef.current
                if (el) {
                  el.classList.remove('search-flash')
                  void el.offsetWidth
                  el.classList.add('search-flash')
                  el.addEventListener('animationend', () => el.classList.remove('search-flash'), { once: true })
                }
              }
            }}
            autoFocus
            placeholder={semanticEnabled ? 'Search semantically…' : 'Search bookmarks…'}
            className="h-9 w-full rounded-lg border border-white/[0.06] bg-surface pl-9 pr-[4.25rem] text-sm text-text placeholder:text-text-dim outline-none transition-colors focus:border-hi-dim focus:bg-surface-hover"
          />
          {localPrimary && (
            <button
              tabIndex={-1}
              onClick={() => { setLocalPrimary(''); searchInputRef.current?.focus() }}
              className="absolute right-9 flex h-6 w-6 items-center justify-center rounded-md text-text-muted hover:text-text transition-colors"
              aria-label="Clear search"
            >
              <XIcon />
            </button>
          )}
          {/* Filter toggle inside search bar */}
          <button
            tabIndex={-1}
            onClick={() => setFiltersOpen(!showFilters)}
            className={`absolute right-1.5 flex h-6 items-center gap-1 rounded-md px-1.5 text-xs transition-colors ${
              showFilters
                ? 'bg-hi-dim text-text'
                : 'text-text-muted hover:text-text'
            }`}
          >
            <FilterIcon className="shrink-0" />
            {hasAdvancedFilters && (
              <span className="font-mono text-[10px]">
                {[debouncedTags, debouncedTitle, debouncedUrl, debouncedDescription, semanticEnabled ? debouncedQueryAlt : ''].filter(Boolean).length}
              </span>
            )}
          </button>
        </div>

        {/* Workspace selector */}
        {workspacesAvailable && (
          <div className="relative shrink-0">
            <select
              value={activeWorkspaceId ?? ''}
              onChange={(e) => {
                const v = e.target.value
                setActiveWorkspaceId(v === '' ? null : v)
              }}
              className="h-7 appearance-none rounded-md border border-white/[0.06] bg-surface pl-2 pr-7 text-xs text-text outline-none transition-colors hover:bg-surface-hover focus:border-hi-dim cursor-pointer"
            >
              {(settings.showCatchAllWorkspace || workspaces.length === 0) && (
                <option value="">---</option>
              )}
              {workspaces.map((ws) => (
                <option key={ws.id} value={ws.id}>{ws.name}</option>
              ))}
            </select>
            <ChevronDown className="pointer-events-none absolute right-1.5 top-1/2 h-3 w-3 -translate-y-1/2 text-text-dim" />
          </div>
        )}

        {/* Counter */}
        <div className="flex items-baseline gap-0.5 font-mono text-xs tabular-nums select-none shrink-0">
          <span className={hasAnySearch || activeWorkspaceId ? 'text-hi' : 'text-text-muted'}>
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

        {/* Settings */}
        <button
          tabIndex={-1}
          onClick={() => setSettingsOpen(true)}
          className="flex h-7 w-7 shrink-0 items-center justify-center rounded-md text-text-muted transition-colors hover:bg-surface-hover hover:text-text"
          title="Settings"
        >
          <Settings className="h-3.5 w-3.5" />
        </button>

      </div>

      {/* ── Search error ── */}
      {searchError && (
        <div className="mx-2 mb-1 rounded-md bg-danger/10 px-3 py-1.5 text-xs text-danger sm:mx-3">
          {searchError}
        </div>
      )}

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
          showFilters ? 'grid-rows-[1fr]' : 'grid-rows-[0fr]'
        }`}
      >
        <div className="overflow-hidden">
          <div className="flex flex-wrap items-center gap-x-3 gap-y-2 border-t border-white/[0.04] px-3 py-2 sm:px-4 sm:py-2.5">
            {/* Advanced fields */}
            {semanticEnabled && (
              <FilterField label="query" value={localQueryAlt} onChange={setLocalQueryAlt} />
            )}
            <TagFilterField label="tags" value={localTags} onChange={setLocalTags} availableTags={autocompleteTags} />
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

// ─── Tag filter field (with autocomplete) ─────────────────────────
const TagFilterField = memo(function TagFilterField({
  label,
  value,
  onChange,
  availableTags,
}: {
  label: string
  value: string
  onChange: (v: string) => void
  availableTags: string[]
}) {
  return (
    <label className="flex items-center gap-1.5 min-w-0 w-[calc(50%-0.75rem)] sm:w-auto">
      <span className="text-[11px] font-medium uppercase tracking-wider text-text-dim select-none shrink-0">
        {label}
      </span>
      <TagAutocompleteInput
        value={value}
        onChange={onChange}
        availableTags={availableTags}
        className="w-full sm:w-28"
        inputClassName={`h-7 w-full rounded-md border bg-transparent px-2 text-sm outline-none transition-colors ${
          value
            ? 'border-hi/20 text-text'
            : 'border-white/[0.06] text-text placeholder:text-text-dim'
        } focus:border-hi-dim focus:bg-surface`}
      />
    </label>
  )
})

// ─── Filter field ──────────────────────────────────────────────────
const FilterField = memo(function FilterField({
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
})

// ─── Pill toggle ───────────────────────────────────────────────────
const PillToggle = memo(function PillToggle({
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
})

const PillButton = memo(function PillButton({ onClick, label }: { onClick: () => void; label: string }) {
  return (
    <button
      tabIndex={-1}
      onClick={onClick}
      className="rounded-full px-2.5 py-1 text-xs font-medium text-text-muted transition-all select-none hover:text-text hover:bg-surface-hover active:bg-hi-dim active:text-text"
    >
      {label}
    </button>
  )
})
