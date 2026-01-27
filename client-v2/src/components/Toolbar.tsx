import { useEffect, useCallback } from 'react'
import { useStore } from '@/lib/store'
import { Checkbox } from '@/components/ui/checkbox'
import {
  Tooltip,
  TooltipContent,
  TooltipProvider,
  TooltipTrigger,
} from '@/components/ui/tooltip'
import { useDebouncedValue } from '@/hooks/useDebouncedValue'
import type { SearchQuery } from '@/lib/api'

const VIEW_MODES = [
  { value: 'grid' as const, label: 'Grid', icon: GridIcon },
  { value: 'cards' as const, label: 'Cards', icon: CardsIcon },
  { value: 'table' as const, label: 'Table', icon: TableIcon },
]

function GridIcon({ active }: { active?: boolean }) {
  return (
    <svg width="16" height="16" viewBox="0 0 16 16" fill="none">
      <rect x="1" y="1" width="6" height="6" rx="1" className={active ? 'fill-accent' : 'fill-text-muted'} />
      <rect x="9" y="1" width="6" height="6" rx="1" className={active ? 'fill-accent' : 'fill-text-muted'} />
      <rect x="1" y="9" width="6" height="6" rx="1" className={active ? 'fill-accent' : 'fill-text-muted'} />
      <rect x="9" y="9" width="6" height="6" rx="1" className={active ? 'fill-accent' : 'fill-text-muted'} />
    </svg>
  )
}

function CardsIcon({ active }: { active?: boolean }) {
  return (
    <svg width="16" height="16" viewBox="0 0 16 16" fill="none">
      <rect x="1" y="1" width="14" height="4" rx="1" className={active ? 'fill-accent' : 'fill-text-muted'} />
      <rect x="1" y="7" width="14" height="4" rx="1" className={active ? 'fill-accent/60' : 'fill-text-dim'} />
      <rect x="1" y="13" width="14" height="2" rx="1" className={active ? 'fill-accent/30' : 'fill-text-dim/60'} />
    </svg>
  )
}

function TableIcon({ active }: { active?: boolean }) {
  return (
    <svg width="16" height="16" viewBox="0 0 16 16" fill="none">
      {[1, 5, 9, 13].map((y) => (
        <rect key={y} x="1" y={y} width="14" height="2" rx="0.5" className={active ? 'fill-accent' : 'fill-text-muted'} />
      ))}
    </svg>
  )
}

function useResponsiveColumns(): number {
  if (typeof window === 'undefined') return 3
  const w = window.innerWidth
  if (w < 640) return 1
  if (w < 1024) return 2
  if (w < 1440) return 3
  if (w < 1920) return 4
  return 5
}

function Separator() {
  return <div className="mx-1 h-6 w-px bg-white/[0.06]" />
}

export function Toolbar() {
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

  const [debouncedSemantic, setLocalSemantic, localSemantic] =
    useDebouncedValue(searchQuery.semantic ?? '', 500)
  const [debouncedKeyword, setLocalKeyword, localKeyword] =
    useDebouncedValue(searchQuery.keyword ?? '', 300)
  const [debouncedTags, setLocalTags, localTags] =
    useDebouncedValue(searchQuery.tags ?? '', 300)
  const [debouncedTitle, setLocalTitle, localTitle] =
    useDebouncedValue(searchQuery.title ?? '', 300)
  const [debouncedUrl, setLocalUrl, localUrl] =
    useDebouncedValue(searchQuery.url ?? '', 300)
  const [debouncedDescription, setLocalDescription, localDescription] =
    useDebouncedValue(searchQuery.description ?? '', 300)

  useEffect(() => {
    const query: SearchQuery = {}
    if (debouncedSemantic) query.semantic = debouncedSemantic
    if (debouncedKeyword) query.keyword = debouncedKeyword
    if (debouncedTags) query.tags = debouncedTags
    if (debouncedTitle) query.title = debouncedTitle
    if (debouncedUrl) query.url = debouncedUrl
    if (debouncedDescription) query.description = debouncedDescription
    setSearchQuery(query)
  }, [
    debouncedSemantic,
    debouncedKeyword,
    debouncedTags,
    debouncedTitle,
    debouncedUrl,
    debouncedDescription,
    setSearchQuery,
  ])

  useEffect(() => {
    if (!saveQueries) return
    const url = new URL(window.location.href)
    const fields = { tags: debouncedTags, title: debouncedTitle, url: debouncedUrl, description: debouncedDescription }
    for (const [key, val] of Object.entries(fields)) {
      if (val) url.searchParams.set(key, val)
      else url.searchParams.delete(key)
    }
    if (showAll) url.searchParams.set('all', '1')
    else url.searchParams.delete('all')
    window.history.replaceState({}, '', url)
  }, [saveQueries, debouncedTags, debouncedTitle, debouncedUrl, debouncedDescription, showAll])

  useEffect(() => {
    const params = new URLSearchParams(window.location.search)
    const tags = params.get('tags') ?? ''
    const title = params.get('title') ?? ''
    const url = params.get('url') ?? ''
    const description = params.get('description') ?? ''
    const all = params.get('all')
    if (tags) setLocalTags(tags)
    if (title) setLocalTitle(title)
    if (url) setLocalUrl(url)
    if (description) setLocalDescription(description)
    if (all === '1' || all === 'true') setShowAll(true)
  }, []) // eslint-disable-line react-hooks/exhaustive-deps

  useEffect(() => {
    setColumns(useResponsiveColumns())
  }, []) // eslint-disable-line react-hooks/exhaustive-deps

  const hasSearch =
    !!debouncedSemantic ||
    !!debouncedKeyword ||
    !!debouncedTags ||
    !!debouncedTitle ||
    !!debouncedUrl ||
    !!debouncedDescription

  const matchedCount = bookmarks.length

  const clearAllFields = useCallback(() => {
    setLocalSemantic('')
    setLocalKeyword('')
    setLocalTags('')
    setLocalTitle('')
    setLocalUrl('')
    setLocalDescription('')
  }, [setLocalSemantic, setLocalKeyword, setLocalTags, setLocalTitle, setLocalUrl, setLocalDescription])

  return (
    <TooltipProvider delayDuration={300}>
      <header className="sticky top-0 z-40 border-b border-white/[0.06] bg-bg/90 backdrop-blur-xl">
        <div className="flex flex-wrap items-center gap-x-2 gap-y-2 px-4 py-2.5">
          {/* Brand */}
          <span className="font-mono text-sm font-medium tracking-tight text-accent mr-1 select-none">
            bb
          </span>

          <Separator />

          {/* Search fields */}
          <div className="flex flex-wrap items-center gap-1.5">
            {semanticEnabled && (
              <SearchField
                value={localSemantic}
                onChange={setLocalSemantic}
                placeholder="semantic"
                className="w-32"
                accent
              />
            )}
            <SearchField
              value={localKeyword}
              onChange={setLocalKeyword}
              placeholder="keyword"
              className="w-24"
            />
            <SearchField
              value={localTags}
              onChange={setLocalTags}
              placeholder="tags"
              className="w-24"
            />
            <SearchField
              value={localTitle}
              onChange={setLocalTitle}
              placeholder="title"
              className="w-20"
            />
            <SearchField
              value={localUrl}
              onChange={setLocalUrl}
              placeholder="url"
              className="w-20"
            />
            <SearchField
              value={localDescription}
              onChange={setLocalDescription}
              placeholder="desc"
              className="w-20"
            />
            {hasSearch && (
              <button
                onClick={clearAllFields}
                className="ml-0.5 flex h-7 w-7 items-center justify-center rounded text-text-dim transition-colors hover:bg-surface-hover hover:text-text-muted"
              >
                <svg width="12" height="12" viewBox="0 0 12 12" fill="none">
                  <path d="M2 2l8 8M10 2l-8 8" stroke="currentColor" strokeWidth="1.5" strokeLinecap="round" />
                </svg>
              </button>
            )}
          </div>

          <Separator />

          {/* Counter */}
          <span className="font-mono text-xs tabular-nums tracking-wide">
            <span className={hasSearch ? 'text-accent' : 'text-text-muted'}>
              {matchedCount}
            </span>
            <span className="text-text-dim">/</span>
            <span className="text-text-dim">{totalCount}</span>
          </span>

          {/* Spacer */}
          <div className="flex-1" />

          {/* Toggles */}
          <div className="flex items-center gap-2.5">
            <Toggle checked={shuffle} onChange={setShuffle} label="shfl" />
            <Toggle checked={showAll} onChange={setShowAll} label="all" />
            <Toggle checked={saveQueries} onChange={setSaveQueries} label="pin" />
          </div>

          <Separator />

          {/* Column controls */}
          <div className="flex items-center gap-0">
            <button
              onClick={() => setColumns(Math.max(1, columns - 1))}
              className="flex h-7 w-6 items-center justify-center rounded-l text-text-dim transition-colors hover:bg-surface-hover hover:text-text-muted"
            >
              <svg width="10" height="10" viewBox="0 0 10 10"><path d="M2 5h6" stroke="currentColor" strokeWidth="1.5" strokeLinecap="round" /></svg>
            </button>
            <span className="flex h-7 min-w-[1.25rem] items-center justify-center border-x border-white/[0.04] bg-surface/50 px-1 font-mono text-xs tabular-nums text-text-muted">
              {columns}
            </span>
            <button
              onClick={() => setColumns(Math.min(12, columns + 1))}
              className="flex h-7 w-6 items-center justify-center rounded-r text-text-dim transition-colors hover:bg-surface-hover hover:text-text-muted"
            >
              <svg width="10" height="10" viewBox="0 0 10 10"><path d="M5 2v6M2 5h6" stroke="currentColor" strokeWidth="1.5" strokeLinecap="round" /></svg>
            </button>
          </div>

          <Separator />

          {/* View mode toggle */}
          <div className="flex items-center gap-0 rounded-md border border-white/[0.06] bg-surface/40">
            {VIEW_MODES.map((mode) => (
              <Tooltip key={mode.value}>
                <TooltipTrigger asChild>
                  <button
                    onClick={() => setViewMode(mode.value)}
                    className={`flex h-7 w-8 items-center justify-center transition-all ${
                      viewMode === mode.value
                        ? 'bg-surface-active'
                        : 'hover:bg-surface-hover'
                    } ${mode.value === 'grid' ? 'rounded-l-[5px]' : ''} ${
                      mode.value === 'table' ? 'rounded-r-[5px]' : ''
                    }`}
                  >
                    <mode.icon active={viewMode === mode.value} />
                  </button>
                </TooltipTrigger>
                <TooltipContent side="bottom" className="text-xs">
                  {mode.label}
                </TooltipContent>
              </Tooltip>
            ))}
          </div>
        </div>
      </header>
    </TooltipProvider>
  )
}

function SearchField({
  value,
  onChange,
  placeholder,
  className = '',
  accent,
}: {
  value: string
  onChange: (v: string) => void
  placeholder: string
  className?: string
  accent?: boolean
}) {
  const hasValue = value.length > 0
  return (
    <div className={`relative ${className}`}>
      <input
        type="text"
        value={value}
        onChange={(e) => onChange(e.target.value)}
        placeholder={placeholder}
        className={`toolbar-field h-7 w-full px-2 font-mono text-xs ${
          hasValue && accent
            ? 'border-accent-dim/60 text-accent'
            : hasValue
              ? 'border-white/[0.12] text-text'
              : ''
        }`}
      />
    </div>
  )
}

function Toggle({
  checked,
  onChange,
  label,
}: {
  checked: boolean
  onChange: (v: boolean) => void
  label: string
}) {
  return (
    <label className="group flex cursor-pointer items-center gap-1.5 select-none">
      <Checkbox
        checked={checked}
        onCheckedChange={(v) => onChange(!!v)}
        className="h-3 w-3 rounded-[3px] border-text-dim data-[state=checked]:border-accent-muted data-[state=checked]:bg-accent-muted"
      />
      <span className={`font-mono text-[11px] tracking-wide transition-colors ${
        checked ? 'text-text-muted' : 'text-text-dim'
      } group-hover:text-text-muted`}>
        {label}
      </span>
    </label>
  )
}
