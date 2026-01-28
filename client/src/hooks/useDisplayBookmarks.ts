import { useMemo, useState } from 'react'
import { useStore } from '@/lib/store'
import type { Bookmark, Workspace } from '@/lib/api'
import { applyWorkspaceFilter } from '@/lib/workspaceFilters'

// Deterministic shuffle using seed + bookmark ID (Knuth multiplicative hash)
function shuffleBookmarks(bookmarks: Bookmark[], seed: number): Bookmark[] {
  const arr = [...bookmarks]
  for (let i = arr.length - 1; i > 0; i--) {
    const h = ((seed * 2654435761) ^ (arr[i].id * 2246822519)) >>> 0
    const j = h % (i + 1)
    ;[arr[i], arr[j]] = [arr[j], arr[i]]
  }
  return arr
}

export type EmptyReason = 'loading' | 'no-bookmarks' | 'no-query' | 'no-matches' | null

export function useDisplayBookmarks() {
  const bookmarks = useStore((s) => s.bookmarks)
  const shuffle = useStore((s) => s.shuffle)
  const shuffleSeed = useStore((s) => s.shuffleSeed)
  const showAll = useStore((s) => s.showAll)
  const searchQuery = useStore((s) => s.searchQuery)
  const totalCount = useStore((s) => s.totalCount)
  const bookmarksFresh = useStore((s) => s.bookmarksFresh)
  const initialLoadComplete = useStore((s) => s.initialLoadComplete)
  const activeWorkspaceId = useStore((s) => s.activeWorkspaceId)
  const workspaces = useStore((s) => s.workspaces)

  const hasQuery = !!(
    searchQuery.semantic ||
    searchQuery.keyword ||
    searchQuery.tags ||
    searchQuery.title ||
    searchQuery.url ||
    searchQuery.description
  )

  const hasWorkspace = activeWorkspaceId !== null

  const emptyReason: EmptyReason = useMemo(() => {
    if (!initialLoadComplete) return 'loading'
    if (totalCount === 0 && bookmarksFresh) return 'no-bookmarks'
    if (!showAll && !hasQuery && !hasWorkspace) return 'no-query'
    if (bookmarks.length === 0 && (hasQuery || hasWorkspace) && bookmarksFresh) return 'no-matches'
    return null
  }, [initialLoadComplete, totalCount, showAll, hasQuery, hasWorkspace, bookmarks.length, bookmarksFresh])

  // Apply client-side workspace filters (glob patterns, regex, blacklist)
  const workspaceFiltered = useMemo(() => {
    if (!bookmarksFresh) return null
    if (!activeWorkspaceId) return bookmarks

    const ws = workspaces.find((w: Workspace) => w.id === activeWorkspaceId)
    if (!ws) return bookmarks

    return applyWorkspaceFilter(bookmarks, ws)
  }, [bookmarks, activeWorkspaceId, workspaces, bookmarksFresh])

  const freshDisplay = useMemo(() => {
    if (!bookmarksFresh || workspaceFiltered === null) return null
    if (shuffle && !searchQuery.semantic) return shuffleBookmarks(workspaceFiltered, shuffleSeed)
    // Non-semantic: reverse for newest-first; semantic: relevance-ranked as-is
    if (!searchQuery.semantic) return [...workspaceFiltered].reverse()
    return workspaceFiltered
  }, [workspaceFiltered, shuffle, shuffleSeed, searchQuery.semantic, bookmarksFresh])

  // Cache last fresh result to avoid flashing stale data with new ordering
  // Uses "update state during render" pattern (React-supported for derived state)
  const [cached, setCached] = useState<Bookmark[]>([])
  if (freshDisplay !== null && freshDisplay !== cached) {
    setCached(freshDisplay)
  }
  const displayBookmarks = freshDisplay ?? cached

  return { displayBookmarks, emptyReason, hasQuery }
}
