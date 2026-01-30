import { useRef, useMemo, useEffect, useCallback, memo } from 'react'
import { useVirtualizer } from '@tanstack/react-virtual'
import { useStore } from '@/lib/store'
import { BookmarkCard } from './BookmarkCard'
import { EmptyState } from './bookmark-parts'
import { useDisplayBookmarks } from '@/hooks/useDisplayBookmarks'
import { useAutoColumns, MAX_GRID_WIDTH } from '@/hooks/useResponsive'

const ROW_GAP = 16
const ESTIMATED_ROW_HEIGHT = 330

function chunkArray<T>(arr: T[], size: number): T[][] {
  const chunks: T[][] = []
  for (let i = 0; i < arr.length; i += size) {
    chunks.push(arr.slice(i, i + size))
  }
  return chunks
}

export function BookmarkGrid() {
  const parentRef = useRef<HTMLDivElement | null>(null)
  const columns = useStore((s) => s.columns)
  const setColumns = useStore((s) => s.setColumns)
  const isUserLoading = useStore((s) => s.isUserLoading)
  const { displayBookmarks, emptyReason } = useDisplayBookmarks()

  // Auto-compute columns from container width, with scroll preservation
  const [autoCols, colsRef] = useAutoColumns()

  // Merge callback ref with parentRef so both track the same element
  const setRefs = useCallback((node: HTMLDivElement | null) => {
    parentRef.current = node
    colsRef(node)
  }, [colsRef])
  const scrollTargetRef = useRef<number | null>(null)

  useEffect(() => {
    if (autoCols === columns) return

    // Capture first visible bookmark BEFORE column change
    const scrollEl = parentRef.current
    if (scrollEl) {
      const scrollTop = scrollEl.scrollTop
      const virtualItems = virtualizer.getVirtualItems()
      if (virtualItems.length > 0) {
        const firstVisible = virtualItems.find((item) => item.start >= scrollTop)
          ?? virtualItems[0]
        scrollTargetRef.current = firstVisible.index * columns
      }
    }

    setColumns(autoCols)
  }, [autoCols]) // eslint-disable-line react-hooks/exhaustive-deps

  const rows = useMemo(
    () => chunkArray(displayBookmarks, columns),
    [displayBookmarks, columns],
  )

  const virtualizer = useVirtualizer({
    count: rows.length,
    getScrollElement: () => parentRef.current,
    estimateSize: () => ESTIMATED_ROW_HEIGHT,
    overscan: 8,
    gap: ROW_GAP,
    getItemKey: (index) => rows[index]?.[0]?.id ?? index, // stable key from first bookmark in row
  })

  // Scroll to preserved bookmark after layout settles
  useEffect(() => {
    const target = scrollTargetRef.current
    if (target === null) return
    scrollTargetRef.current = null

    const newRow = Math.floor(target / columns)
    virtualizer.scrollToIndex(newRow, { align: 'start' })
  }, [columns, virtualizer])

  // Re-measure visible rows after bookmark data changes (new bookmark, metadata fetch, etc.)
  // measureElement must be called on actual DOM nodes — measure() alone only clears the cache
  // without re-reading element heights, leaving rows stuck at estimateSize.
  useEffect(() => {
    const container = parentRef.current
    if (!container) return
    requestAnimationFrame(() => {
      container.querySelectorAll<HTMLElement>('[data-index]').forEach((node) => {
        virtualizer.measureElement(node)
      })
    })
  }, [rows, virtualizer])

  if (emptyReason) return <ViewEmptyState reason={emptyReason} />

  return (
    <div ref={setRefs} className="h-full overflow-auto p-4">
      <div
        className={`relative mx-auto w-full transition-opacity duration-150 ${isUserLoading ? 'opacity-40' : ''}`}
        style={{ height: virtualizer.getTotalSize(), maxWidth: MAX_GRID_WIDTH }}
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
                  <BookmarkCard key={bookmark.id} bookmark={bookmark} />
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

export const ViewEmptyState = memo(function ViewEmptyState({ reason }: { reason: EmptyReason }) {
  switch (reason) {
    case 'loading':
      return (
        <div className="flex h-full items-center justify-center">
          <div className="h-12 w-12 animate-spin rounded-full border-4 border-surface-active border-t-hi" />
        </div>
      )
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
})
