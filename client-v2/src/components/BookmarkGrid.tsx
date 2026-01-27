import { useRef, useMemo, useCallback } from 'react'
import { useVirtualizer } from '@tanstack/react-virtual'
import { useStore } from '@/lib/store'
import { BookmarkCard } from './BookmarkCard'
import { EmptyState } from './bookmark-parts'
import { useDisplayBookmarks } from '@/hooks/useDisplayBookmarks'

const ROW_GAP = 16
const ESTIMATED_ROW_HEIGHT = 320

function chunkArray<T>(arr: T[], size: number): T[][] {
  const chunks: T[][] = []
  for (let i = 0; i < arr.length; i += size) {
    chunks.push(arr.slice(i, i + size))
  }
  return chunks
}

export function BookmarkGrid() {
  const parentRef = useRef<HTMLDivElement>(null)
  const columns = useStore((s) => s.columns)
  const setDetailModalId = useStore((s) => s.setDetailModalId)
  const { displayBookmarks, emptyReason } = useDisplayBookmarks()

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

  if (emptyReason) return <ViewEmptyState reason={emptyReason} />

  return (
    <div ref={parentRef} className="h-full overflow-auto p-4">
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

// ─── Shared empty state renderer ───────────────────────────────────
import type { EmptyReason } from '@/hooks/useDisplayBookmarks'

export function ViewEmptyState({ reason }: { reason: EmptyReason }) {
  switch (reason) {
    case 'no-bookmarks':
      return (
        <EmptyState
          icon="📑"
          title="No bookmarks yet"
          subtitle="Add your first bookmark with Ctrl+N"
        />
      )
    case 'no-query':
      return (
        <EmptyState
          icon="🔍"
          title="Search or enable Show All"
          subtitle="Type a search query or toggle Show All to browse"
        />
      )
    case 'no-matches':
      return (
        <EmptyState
          icon="∅"
          title="No matches"
          subtitle="No bookmarks match your search"
        />
      )
    default:
      return null
  }
}
