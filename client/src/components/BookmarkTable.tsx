import { useRef, useMemo, memo } from 'react'
import { useVirtualizer } from '@tanstack/react-virtual'
import { useStore } from '@/lib/store'
import { useHiddenTags } from '@/hooks/useHiddenTags'
import { Favicon, TagChip, CardActions, FetchingIndicator } from './bookmark-parts'
import { ViewEmptyState } from './BookmarkGrid'
import { useDisplayBookmarks } from '@/hooks/useDisplayBookmarks'
import type { Bookmark } from '@/lib/api'

const ROW_HEIGHT = 40

export function BookmarkTable() {
  const parentRef = useRef<HTMLDivElement>(null)
  const isUserLoading = useStore((s) => s.isUserLoading)
  const { displayBookmarks, emptyReason } = useDisplayBookmarks()

  const virtualizer = useVirtualizer({
    count: displayBookmarks.length,
    getScrollElement: () => parentRef.current,
    estimateSize: () => ROW_HEIGHT,
    overscan: 15,
    getItemKey: (index) => displayBookmarks[index]?.id ?? index,
  })

  if (emptyReason) return <ViewEmptyState reason={emptyReason} />

  return (
    <div ref={parentRef} className="h-full overflow-auto">
      {/* Header */}
      <div className="sticky top-0 z-10 flex items-center gap-3 border-b border-white/[0.06] bg-bg px-4 py-2 text-[11px] font-medium uppercase tracking-wider text-text-dim">
        <span className="min-w-0 flex-[3]">Title</span>
        <span className="hidden min-w-0 flex-[2] sm:block">URL</span>
        <span className="hidden min-w-0 flex-[2] md:block">Tags</span>
        <span className="hidden min-w-0 flex-[2] lg:block">Description</span>
      </div>

      <div
        className={`relative w-full transition-opacity duration-150 ${isUserLoading ? 'opacity-40' : ''}`}
        style={{ height: virtualizer.getTotalSize() }}
      >
        {virtualizer.getVirtualItems().map((virtualRow) => {
          const bookmark = displayBookmarks[virtualRow.index]
          return (
            <div
              key={bookmark.id}
              data-index={virtualRow.index}
              className="absolute left-0 top-0 w-full"
              style={{ transform: `translateY(${virtualRow.start}px)` }}
            >
              <TableRow bookmark={bookmark} />
            </div>
          )
        })}
      </div>
    </div>
  )
}

const TableRow = memo(function TableRow({ bookmark }: { bookmark: Bookmark }) {
  const setDetailModalId = useStore((s) => s.setDetailModalId)
  const hiddenTags = useHiddenTags()
  const visibleTags = useMemo(
    () => bookmark.tags.filter((t) => !hiddenTags.includes(t)),
    [bookmark.tags, hiddenTags],
  )

  return (
    <a
      href={bookmark.url}
      target="_blank"
      rel="noopener noreferrer"
      onClick={(e) => {
        if (e.button === 0 && !e.metaKey && !e.ctrlKey && !e.shiftKey) {
          e.preventDefault()
          setDetailModalId(bookmark.id)
        }
      }}
      className={`group relative flex items-center gap-3 border-b border-white/[0.03] px-4 py-2 transition-colors hover:bg-surface-hover no-underline ${
        bookmark.fetching ? 'border-l-2 border-l-hi-dim' : ''
      }`}
    >
      <CardActions bookmarkId={bookmark.id} variant="row" />
      {/* Title */}
      <div className="min-w-0 flex-[3]">
        <span onClick={(e) => e.stopPropagation()} className="block truncate text-sm text-text hover:text-hi cursor-pointer">
          <Favicon iconId={bookmark.icon_id} />{' '}
          {bookmark.title || bookmark.url}
        </span>
        {bookmark.fetching && <FetchingIndicator />}
      </div>

      {/* URL */}
      <div className="hidden min-w-0 flex-[2] sm:block">
        <span onClick={(e) => e.stopPropagation()} className="block truncate font-mono text-[11px] text-text-dim cursor-pointer">
          {bookmark.url}
        </span>
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
    </a>
  )
})
