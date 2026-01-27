import { useEffect, useCallback } from 'react'
import { useStore } from '@/lib/store'
import { Input } from '@/components/ui/input'
import { Button } from '@/components/ui/button'
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
  { value: 'grid' as const, label: 'Grid', icon: '⊞' },
  { value: 'cards' as const, label: 'Cards', icon: '☰' },
  { value: 'table' as const, label: 'Table', icon: '▤' },
]

function useResponsiveColumns(): number {
  if (typeof window === 'undefined') return 3
  const w = window.innerWidth
  if (w < 640) return 1
  if (w < 1024) return 2
  if (w < 1440) return 3
  if (w < 1920) return 4
  return 5
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

  // Debounced search fields
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

  // Apply debounced values to store
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

  // Save queries to URL params
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

  // Restore queries from URL on mount
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

  // Set responsive column default on mount
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

  const inputClass =
    'h-8 bg-surface border-border text-text placeholder:text-text-muted text-sm focus-visible:ring-1 focus-visible:ring-ring'

  return (
    <TooltipProvider delayDuration={300}>
      <div className="sticky top-0 z-40 flex flex-wrap items-center gap-2 border-b border-border bg-bg/80 px-4 py-3 backdrop-blur-xl">
        {/* Search fields */}
        <div className="flex flex-wrap items-center gap-2">
          {semanticEnabled && (
            <Input
              value={localSemantic}
              onChange={(e) => setLocalSemantic(e.target.value)}
              placeholder="Semantic…"
              className={inputClass + ' w-36'}
            />
          )}
          <Input
            value={localKeyword}
            onChange={(e) => setLocalKeyword(e.target.value)}
            placeholder="Keyword"
            className={inputClass + ' w-28'}
          />
          <Input
            value={localTags}
            onChange={(e) => setLocalTags(e.target.value)}
            placeholder="Tags"
            className={inputClass + ' w-28'}
          />
          <Input
            value={localTitle}
            onChange={(e) => setLocalTitle(e.target.value)}
            placeholder="Title"
            className={inputClass + ' w-24'}
          />
          <Input
            value={localUrl}
            onChange={(e) => setLocalUrl(e.target.value)}
            placeholder="URL"
            className={inputClass + ' w-24'}
          />
          <Input
            value={localDescription}
            onChange={(e) => setLocalDescription(e.target.value)}
            placeholder="Description"
            className={inputClass + ' w-28'}
          />
          {hasSearch && (
            <Button
              variant="ghost"
              size="sm"
              onClick={clearAllFields}
              className="h-8 px-2 text-text-muted hover:text-text"
            >
              ✕
            </Button>
          )}
        </div>

        {/* Counter */}
        <span className="text-sm text-text-muted tabular-nums">
          {matchedCount}/{totalCount}
        </span>

        {/* Spacer */}
        <div className="flex-1" />

        {/* Toggles */}
        <div className="flex items-center gap-3">
          <label className="flex items-center gap-1.5 text-sm text-text-muted">
            <Checkbox
              checked={shuffle}
              onCheckedChange={(v) => setShuffle(!!v)}
              className="h-3.5 w-3.5"
            />
            Shuffle
          </label>
          <label className="flex items-center gap-1.5 text-sm text-text-muted">
            <Checkbox
              checked={showAll}
              onCheckedChange={(v) => setShowAll(!!v)}
              className="h-3.5 w-3.5"
            />
            Show all
          </label>
          <label className="flex items-center gap-1.5 text-sm text-text-muted">
            <Checkbox
              checked={saveQueries}
              onCheckedChange={(v) => setSaveQueries(!!v)}
              className="h-3.5 w-3.5"
            />
            Save queries
          </label>
        </div>

        {/* Column controls */}
        <div className="flex items-center gap-1">
          <Button
            variant="ghost"
            size="sm"
            onClick={() => setColumns(Math.max(1, columns - 1))}
            className="h-7 w-7 p-0 text-text-muted hover:text-text"
          >
            −
          </Button>
          <span className="w-4 text-center text-sm tabular-nums text-text-muted">
            {columns}
          </span>
          <Button
            variant="ghost"
            size="sm"
            onClick={() => setColumns(Math.min(12, columns + 1))}
            className="h-7 w-7 p-0 text-text-muted hover:text-text"
          >
            +
          </Button>
        </div>

        {/* View mode toggle */}
        <div className="flex items-center rounded-md border border-border">
          {VIEW_MODES.map((mode) => (
            <Tooltip key={mode.value}>
              <TooltipTrigger asChild>
                <button
                  onClick={() => setViewMode(mode.value)}
                  className={`px-2 py-1 text-sm transition-colors ${
                    viewMode === mode.value
                      ? 'bg-surface text-text'
                      : 'text-text-muted hover:text-text'
                  } ${mode.value === 'grid' ? 'rounded-l-md' : ''} ${
                    mode.value === 'table' ? 'rounded-r-md' : ''
                  }`}
                >
                  {mode.icon}
                </button>
              </TooltipTrigger>
              <TooltipContent>{mode.label}</TooltipContent>
            </Tooltip>
          ))}
        </div>
      </div>
    </TooltipProvider>
  )
}
