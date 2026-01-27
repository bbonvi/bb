import { useRef, useMemo, useCallback } from 'react'
import { useVirtualizer } from '@tanstack/react-virtual'
import { useStore } from '@/lib/store'
import { BookmarkCard } from './BookmarkCard'
import type { Bookmark } from '@/lib/api'

const ROW_GAP = 16
const ESTIMATED_ROW_HEIGHT = 320

function chunkArray<T>(arr: T[], size: number): T[][] {
  const chunks: T[][] = []
  for (let i = 0; i < arr.length; i += size) {
    chunks.push(arr.slice(i, i + size))
  }
  return chunks
}

// Deterministic shuffle using seed + bookmark ID
function shuffleBookmarks(bookmarks: Bookmark[], seed: number): Bookmark[] {
  const arr = [...bookmarks]
  for (let i = arr.length - 1; i > 0; i--) {
    // Simple hash: combine seed with bookmark id
    const h = ((seed * 2654435761) ^ (arr[i].id * 2246822519)) >>> 0
    const j = h % (i + 1)
    ;[arr[i], arr[j]] = [arr[j], arr[i]]
  }
  return arr
}

export function BookmarkGrid() {
  const parentRef = useRef<HTMLDivElement>(null)

  const bookmarks = useStore((s) => s.bookmarks)
  const columns = useStore((s) => s.columns)
  const shuffle = useStore((s) => s.shuffle)
  const shuffleSeed = useStore((s) => s.shuffleSeed)
  const showAll = useStore((s) => s.showAll)
  const searchQuery = useStore((s) => s.searchQuery)
  const totalCount = useStore((s) => s.totalCount)
  const setDetailModalId = useStore((s) => s.setDetailModalId)

  // Determine display state
  const hasQuery = !!(
    searchQuery.semantic ||
    searchQuery.keyword ||
    searchQuery.tags ||
    searchQuery.title ||
    searchQuery.url ||
    searchQuery.description
  )

  const displayBookmarks = useMemo(() => {
    if (shuffle && !searchQuery.semantic) return shuffleBookmarks(bookmarks, shuffleSeed)
    // Semantic results are relevance-ranked; non-semantic are ID-ascending — reverse for newest-first
    if (!searchQuery.semantic) return [...bookmarks].reverse()
    return bookmarks
  }, [bookmarks, shuffle, shuffleSeed, searchQuery.semantic])

  const rows = useMemo(
    () => chunkArray(displayBookmarks, columns),
    [displayBookmarks, columns],
  )

  const virtualizer = useVirtualizer({
    count: rows.length,
    getScrollElement: () => parentRef.current,
    estimateSize: () => ESTIMATED_ROW_HEIGHT,
    overscan: 3,
    gap: ROW_GAP,
  })

  const handleCardClick = useCallback(
    (id: number) => setDetailModalId(id),
    [setDetailModalId],
  )

  // Empty states per §16.2
  if (totalCount === 0) {
    return (
      <EmptyState
        icon="📑"
        title="No bookmarks yet"
        subtitle="Add your first bookmark with Ctrl+N"
      />
    )
  }

  if (!showAll && !hasQuery) {
    return (
      <EmptyState
        icon="🔍"
        title="Search or enable Show All"
        subtitle="Type a search query or toggle Show All to browse"
      />
    )
  }

  if (bookmarks.length === 0 && hasQuery) {
    return (
      <EmptyState
        icon="∅"
        title="No matches"
        subtitle="No bookmarks match your search"
      />
    )
  }

  return (
    <div ref={parentRef} className="h-full overflow-auto px-4 pb-4">
      <div
        className="relative w-full"
        style={{ height: virtualizer.getTotalSize() }}
      >
        {virtualizer.getVirtualItems().map((virtualRow) => {
          const row = rows[virtualRow.index]
          return (
            <div
              key={virtualRow.index}
              ref={virtualizer.measureElement}
              data-index={virtualRow.index}
              className="absolute left-0 top-0 w-full"
              style={{
                transform: `translateY(${virtualRow.start}px)`,
              }}
            >
              <div
                className="grid gap-4"
                style={{
                  gridTemplateColumns: `repeat(${columns}, minmax(0, 1fr))`,
                }}
              >
                {row.map((bookmark) => (
                  <BookmarkCard
                    key={bookmark.id}
                    bookmark={bookmark}
                    onClick={() => handleCardClick(bookmark.id)}
                  />
                ))}
              </div>
            </div>
          )
        })}
      </div>
    </div>
  )
}

// ─── Empty state ───────────────────────────────────────────────────
function EmptyState({
  icon,
  title,
  subtitle,
}: {
  icon: string
  title: string
  subtitle: string
}) {
  return (
    <div className="flex h-full flex-col items-center justify-center gap-3 text-center">
      <span className="text-4xl select-none">{icon}</span>
      <h2 className="text-lg font-medium text-text">{title}</h2>
      <p className="text-sm text-text-muted">{subtitle}</p>
    </div>
  )
}
