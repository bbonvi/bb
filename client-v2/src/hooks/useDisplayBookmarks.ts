import { useMemo } from 'react'
import { useStore } from '@/lib/store'
import type { Bookmark } from '@/lib/api'

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

export type EmptyReason = 'no-bookmarks' | 'no-query' | 'no-matches' | null

export function useDisplayBookmarks() {
  const bookmarks = useStore((s) => s.bookmarks)
  const shuffle = useStore((s) => s.shuffle)
  const shuffleSeed = useStore((s) => s.shuffleSeed)
  const showAll = useStore((s) => s.showAll)
  const searchQuery = useStore((s) => s.searchQuery)
  const totalCount = useStore((s) => s.totalCount)
  const bookmarksFresh = useStore((s) => s.bookmarksFresh)

  const hasQuery = !!(
    searchQuery.semantic ||
    searchQuery.keyword ||
    searchQuery.tags ||
    searchQuery.title ||
    searchQuery.url ||
    searchQuery.description
  )

  const emptyReason: EmptyReason = useMemo(() => {
    if (totalCount === 0 && bookmarksFresh) return 'no-bookmarks'
    if (!showAll && !hasQuery) return 'no-query'
    if (bookmarks.length === 0 && hasQuery && bookmarksFresh) return 'no-matches'
    return null
  }, [totalCount, showAll, hasQuery, bookmarks.length, bookmarksFresh])

  const displayBookmarks = useMemo(() => {
    if (shuffle && !searchQuery.semantic) return shuffleBookmarks(bookmarks, shuffleSeed)
    // Non-semantic: reverse for newest-first; semantic: relevance-ranked as-is
    if (!searchQuery.semantic) return [...bookmarks].reverse()
    return bookmarks
  }, [bookmarks, shuffle, shuffleSeed, searchQuery.semantic])

  return { displayBookmarks, emptyReason, hasQuery }
}
