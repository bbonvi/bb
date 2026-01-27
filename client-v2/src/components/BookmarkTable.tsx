import { useRef, useCallback, useMemo } from 'react'
import { useVirtualizer } from '@tanstack/react-virtual'
import { useStore } from '@/lib/store'
import { Favicon, TagChip } from './bookmark-parts'
import { ViewEmptyState } from './BookmarkGrid'
import { useDisplayBookmarks } from '@/hooks/useDisplayBookmarks'
import type { Bookmark } from '@/lib/api'

const ROW_HEIGHT = 40

export function BookmarkTable() {
  const parentRef = useRef<HTMLDivElement>(null)
  const setDetailModalId = useStore((s) => s.setDetailModalId)
  const { displayBookmarks, emptyReason } = useDisplayBookmarks()

  const virtualizer = useVirtualizer({
    count: displayBookmarks.length,
    getScrollElement: () => parentRef.current,
    estimateSize: () => ROW_HEIGHT,
    overscan: 10,
  })

  const handleClick = useCallback(
    (id: number) => setDetailModalId(id),
    [setDetailModalId],
  )

  if (emptyReason) return <ViewEmptyState reason={emptyReason} />

  return (
    <div ref={parentRef} className="h-full overflow-auto">
      {/* Header */}
      <div className="sticky top-0 z-10 flex items-center gap-3 border-b border-white/[0.06] bg-bg/95 px-4 py-2 text-[11px] font-medium uppercase tracking-wider text-text-dim backdrop-blur-sm">
        <span className="w-5" />
        <span className="min-w-0 flex-[3]">Title</span>
        <span className="hidden min-w-0 flex-[2] sm:block">URL</span>
        <span className="hidden min-w-0 flex-[2] md:block">Tags</span>
        <span className="hidden min-w-0 flex-[2] lg:block">Description</span>
      </div>

      <div
        className="relative w-full"
        style={{ height: virtualizer.getTotalSize() }}
      >
        {virtualizer.getVirtualItems().map((virtualRow) => {
          const bookmark = displayBookmarks[virtualRow.index]
          return (
            <div
              key={bookmark.id}
              ref={virtualizer.measureElement}
              data-index={virtualRow.index}
              className="absolute left-0 top-0 w-full"
              style={{ transform: `translateY(${virtualRow.start}px)` }}
            >
              <TableRow
                bookmark={bookmark}
                onClick={() => handleClick(bookmark.id)}
              />
            </div>
          )
        })}
      </div>
    </div>
  )
}

function TableRow({
  bookmark,
  onClick,
}: {
  bookmark: Bookmark
  onClick: () => void
}) {
  const config = useStore((s) => s.config)
  const hiddenTags = config?.hidden_by_default ?? []
  const visibleTags = useMemo(
    () => bookmark.tags.filter((t) => !hiddenTags.includes(t)),
    [bookmark.tags, hiddenTags],
  )

  return (
    <div
      onClick={onClick}
      className="flex items-center gap-3 border-b border-white/[0.03] px-4 py-2 transition-colors hover:bg-surface-hover"
    >
      <Favicon iconId={bookmark.icon_id} />

      {/* Title */}
      <div className="min-w-0 flex-[3]">
        <a
          href={bookmark.url}
          target="_blank"
          rel="noopener noreferrer"
          onClick={(e) => e.stopPropagation()}
          className="truncate text-sm text-text hover:text-hi"
        >
          {bookmark.title || bookmark.url}
        </a>
      </div>

      {/* URL */}
      <div className="hidden min-w-0 flex-[2] sm:block">
        <a
          href={bookmark.url}
          target="_blank"
          rel="noopener noreferrer"
          onClick={(e) => e.stopPropagation()}
          className="block truncate font-mono text-[11px] text-text-dim hover:text-text-muted"
        >
          {bookmark.url}
        </a>
      </div>

      {/* Tags */}
      <div className="hidden min-w-0 flex-[2] md:flex flex-wrap gap-1">
        {visibleTags.slice(0, 3).map((tag) => (
          <TagChip key={tag} tag={tag} />
        ))}
        {visibleTags.length > 3 && (
          <span className="px-1 text-[11px] text-text-dim">
            +{visibleTags.length - 3}
          </span>
        )}
      </div>

      {/* Description snippet */}
      <div className="hidden min-w-0 flex-[2] lg:block">
        <span className="truncate text-xs text-text-muted line-clamp-1">
          {bookmark.description}
        </span>
      </div>
    </div>
  )
}
