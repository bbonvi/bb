import { useRef, memo } from 'react'
import { useVirtualizer } from '@tanstack/react-virtual'
import { useStore } from '@/lib/store'
import { useHiddenTags } from '@/hooks/useHiddenTags'
import { Favicon, Thumbnail, UrlDisplay, Tags, Description, CardActions, FetchingIndicator } from './bookmark-parts'
import { ViewEmptyState } from './BookmarkGrid'
import { useDisplayBookmarks } from '@/hooks/useDisplayBookmarks'
import type { Bookmark } from '@/lib/api'

const ESTIMATED_ROW_HEIGHT = 140
const ROW_GAP = 8

export function BookmarkList() {
  const parentRef = useRef<HTMLDivElement>(null)
  const isUserLoading = useStore((s) => s.isUserLoading)
  const { displayBookmarks, emptyReason } = useDisplayBookmarks()

  const virtualizer = useVirtualizer({
    count: displayBookmarks.length,
    getScrollElement: () => parentRef.current,
    estimateSize: () => ESTIMATED_ROW_HEIGHT,
    overscan: 8,
    gap: ROW_GAP,
    getItemKey: (index) => displayBookmarks[index]?.id ?? index,
  })

  if (emptyReason) return <ViewEmptyState reason={emptyReason} />

  return (
    <div ref={parentRef} className="h-full overflow-auto p-4">
      <div
        className={`relative w-full transition-opacity duration-150 ${isUserLoading ? 'opacity-40' : ''}`}
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
              <ListCard bookmark={bookmark} />
            </div>
          )
        })}
      </div>
    </div>
  )
}

// ─── Horizontal card ───────────────────────────────────────────────
const ListCard = memo(function ListCard({ bookmark }: { bookmark: Bookmark }) {
  const setDetailModalId = useStore((s) => s.setDetailModalId)
  const hiddenTags = useHiddenTags()

  return (
    <article
      onClick={() => setDetailModalId(bookmark.id)}
      className={`group relative flex overflow-hidden rounded-lg border bg-surface transition-[border-color] duration-150 cursor-default ${
        bookmark.fetching
          ? 'fetching-glow'
          : 'border-white/[0.06] hover:border-white/[0.15]'
      }`}
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
        {bookmark.fetching && <FetchingIndicator />}
        <Tags tags={bookmark.tags} hiddenTags={hiddenTags} />
        <Description text={bookmark.description} lineClamp={2} />
      </div>
    </article>
  )
})
