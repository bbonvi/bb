import { useMemo, useState } from 'react'
import { useStore } from '@/lib/store'
import { useShallow } from 'zustand/react/shallow'
import type { Bookmark } from '@/lib/api'
import { useSettings } from '@/hooks/useSettings'

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
  // Single subscription with shallow equality — avoids 9 separate subscriptions
  const {
    bookmarks,
    shuffle,
    shuffleSeed,
    showAll,
    searchQuery,
    totalCount,
    bookmarksFresh,
    initialLoadComplete,
    activeWorkspaceId,
  } = useStore(
    useShallow((s) => ({
      bookmarks: s.bookmarks,
      shuffle: s.shuffle,
      shuffleSeed: s.shuffleSeed,
      showAll: s.showAll,
      searchQuery: s.searchQuery,
      totalCount: s.totalCount,
      bookmarksFresh: s.bookmarksFresh,
      initialLoadComplete: s.initialLoadComplete,
      activeWorkspaceId: s.activeWorkspaceId,
    })),
  )

  const [settings] = useSettings()

  const hasQuery = !!(
    searchQuery.semantic ||
    searchQuery.query ||
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

  // Filter out bookmarks with globally ignored tags (before workspace filter)
  const globalFiltered = useMemo(() => {
    if (!bookmarksFresh) return null
    if (settings.globalIgnoredTags.length === 0) return bookmarks

    const ignoreSet = new Set(settings.globalIgnoredTags)
    return bookmarks.filter((b) => !b.tags.some((t) => ignoreSet.has(t)))
  }, [bookmarks, settings.globalIgnoredTags, bookmarksFresh])

  const freshDisplay = useMemo(() => {
    if (!bookmarksFresh || globalFiltered === null) return null
    if (shuffle && !searchQuery.semantic) return shuffleBookmarks(globalFiltered, shuffleSeed)
    // Non-semantic: reverse for newest-first; semantic: relevance-ranked as-is
    if (!searchQuery.semantic) return [...globalFiltered].reverse()
    return globalFiltered
  }, [globalFiltered, shuffle, shuffleSeed, searchQuery.semantic, bookmarksFresh])

  // Cache last fresh result to avoid flashing stale data with new ordering
  // Uses "update state during render" pattern (React-supported for derived state)
  const [cached, setCached] = useState<Bookmark[]>([])
  if (freshDisplay !== null && freshDisplay !== cached) {
    setCached(freshDisplay)
  }
  const displayBookmarks = freshDisplay ?? cached

  return { displayBookmarks, emptyReason, hasQuery }
}
