import { useRef, memo, useEffect } from 'react'
import { useVirtualizer } from '@tanstack/react-virtual'
import { useStore } from '@/lib/store'
import { useHiddenTags } from '@/hooks/useHiddenTags'
import { Favicon, Thumbnail, UrlDisplay, Tags, Description, CardActions, FetchingIndicator } from './bookmark-parts'
import { ViewEmptyState } from './BookmarkGrid'
import { useDisplayBookmarks } from '@/hooks/useDisplayBookmarks'
import { useScrollResetOnSearch } from '@/hooks/useScrollResetOnSearch'
import { isBookmarkFullyVisible, smoothScrollElementToCenter, smoothScrollVirtualIndexToCenter } from '@/lib/selectionScroll'
import type { Bookmark } from '@/lib/api'

const ESTIMATED_ROW_HEIGHT = 140
const ROW_GAP = 8

export function BookmarkList() {
  const parentRef = useRef<HTMLDivElement>(null)
  const isUserLoading = useStore((s) => s.isUserLoading)
  const selectedBookmarkId = useStore((s) => s.selectedBookmarkId)
  const { displayBookmarks, emptyReason } = useDisplayBookmarks()
  useScrollResetOnSearch(parentRef, displayBookmarks.length)

  // eslint-disable-next-line react-hooks/incompatible-library
  const virtualizer = useVirtualizer({
    count: displayBookmarks.length,
    getScrollElement: () => parentRef.current,
    estimateSize: () => ESTIMATED_ROW_HEIGHT,
    overscan: 8,
    gap: ROW_GAP,
    getItemKey: (index) => displayBookmarks[index]?.id ?? index,
  })

  const selectedIndex = selectedBookmarkId === null
    ? -1
    : displayBookmarks.findIndex((bookmark) => bookmark.id === selectedBookmarkId)

  useEffect(() => {
    if (selectedIndex < 0) return
    const container = parentRef.current
    if (!container) return
    if (selectedBookmarkId !== null && isBookmarkFullyVisible(container, selectedBookmarkId)) return

    const target = container.querySelector<HTMLElement>(`[data-bookmark-id="${selectedBookmarkId}"]`)
    if (target) {
      smoothScrollElementToCenter(container, target)
      return
    }

    smoothScrollVirtualIndexToCenter(container, virtualizer.scrollToIndex, selectedIndex)
  }, [selectedIndex, selectedBookmarkId, virtualizer])

  if (emptyReason) return <ViewEmptyState reason={emptyReason} />

  return (
    <div ref={parentRef} data-bookmark-viewport="true" tabIndex={0} className="h-full overflow-auto p-4 focus:outline-none">
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
  const setSelectedBookmarkId = useStore((s) => s.setSelectedBookmarkId)
  const selectedBookmarkId = useStore((s) => s.selectedBookmarkId)
  const hiddenTags = useHiddenTags()
  const selected = selectedBookmarkId === bookmark.id

  return (
    <article
      data-bookmark-id={bookmark.id}
      onMouseDownCapture={() => setSelectedBookmarkId(bookmark.id)}
      onClick={() => setDetailModalId(bookmark.id)}
      className={`group relative z-0 flex overflow-visible rounded-lg border bg-surface cursor-default ${
        bookmark.fetching
          ? 'fetching-glow'
          : selected
            ? "z-10 border-hi/55 shadow-[inset_0_0_0_999px_rgba(255,255,255,0.016),0_0_0_1px_rgba(107,138,253,0.12),0_0_96px_18px_rgba(107,138,253,0.085)]"
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
          className="mt-1.5 line-clamp-1 text-[15.5px] font-semibold leading-snug tracking-[-0.01em] text-text hover:text-hi"
        >
          <Favicon iconId={bookmark.icon_id} />{' '}
          {bookmark.title || bookmark.url}
        </a>

        <UrlDisplay url={bookmark.url} selected={selected} />
        {bookmark.fetching && <FetchingIndicator />}
        <Tags tags={bookmark.tags} hiddenTags={hiddenTags} selected={selected} />
        <Description text={bookmark.description} lineClamp={2} selected={selected} />
      </div>
    </article>
  )
})
