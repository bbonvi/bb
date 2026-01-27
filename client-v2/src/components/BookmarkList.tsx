import { useRef, useCallback } from 'react'
import { useVirtualizer } from '@tanstack/react-virtual'
import { useStore } from '@/lib/store'
import { useHiddenTags } from '@/hooks/useHiddenTags'
import { Favicon, Thumbnail, UrlDisplay, Tags, Description, CardActions } from './bookmark-parts'
import { ViewEmptyState } from './BookmarkGrid'
import { useDisplayBookmarks } from '@/hooks/useDisplayBookmarks'
import type { Bookmark } from '@/lib/api'

const ESTIMATED_ROW_HEIGHT = 140
const ROW_GAP = 8

export function BookmarkList() {
  const parentRef = useRef<HTMLDivElement>(null)
  const setDetailModalId = useStore((s) => s.setDetailModalId)
  const { displayBookmarks, emptyReason } = useDisplayBookmarks()

  const virtualizer = useVirtualizer({
    count: displayBookmarks.length,
    getScrollElement: () => parentRef.current,
    estimateSize: () => ESTIMATED_ROW_HEIGHT,
    overscan: 5,
    gap: ROW_GAP,
  })

  const handleClick = useCallback(
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
          const bookmark = displayBookmarks[virtualRow.index]
          return (
            <div
              key={bookmark.id}
              ref={virtualizer.measureElement}
              data-index={virtualRow.index}
              className="absolute left-0 top-0 w-full"
              style={{ transform: `translateY(${virtualRow.start}px)` }}
            >
              <ListCard
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

// ─── Horizontal card ───────────────────────────────────────────────
function ListCard({
  bookmark,
  onClick,
}: {
  bookmark: Bookmark
  onClick: () => void
}) {
  const hiddenTags = useHiddenTags()

  return (
    <article
      onClick={onClick}
      className="group relative flex overflow-hidden rounded-lg border border-white/[0.06] bg-surface transition-[border-color] duration-150 hover:border-white/[0.15] cursor-default"
    >
      <CardActions bookmarkId={bookmark.id} />
      {/* Thumbnail on the left */}
      <div className="hidden shrink-0 sm:block">
        <Thumbnail
          bookmark={bookmark}
          className="h-full w-40 rounded-l-lg rounded-r-none"
        />
      </div>

      {/* Content on the right */}
      <div className="flex min-w-0 flex-1 flex-col gap-1.5 p-3">
        <a
          href={bookmark.url}
          target="_blank"
          rel="noopener noreferrer"
          onClick={(e) => e.stopPropagation()}
          className="line-clamp-1 text-sm font-medium leading-snug text-text hover:text-hi"
        >
          <Favicon iconId={bookmark.icon_id} />{' '}
          {bookmark.title || bookmark.url}
        </a>

        <UrlDisplay url={bookmark.url} />
        <Tags tags={bookmark.tags} hiddenTags={hiddenTags} />
        <Description text={bookmark.description} lineClamp={2} />
      </div>
    </article>
  )
}
