import { useRef, useMemo, memo, useEffect } from 'react'
import { useVirtualizer } from '@tanstack/react-virtual'
import { useStore } from '@/lib/store'
import { useHiddenTags } from '@/hooks/useHiddenTags'
import { Favicon, TagChip, CardActions, FetchingIndicator, ArmedDeleteOverlay } from './bookmark-parts'
import { ViewEmptyState } from './BookmarkGrid'
import { useDisplayBookmarks } from '@/hooks/useDisplayBookmarks'
import { useScrollResetOnSearch } from '@/hooks/useScrollResetOnSearch'
import { isBookmarkFullyVisible, smoothScrollElementToCenter, smoothScrollVirtualIndexToCenter } from '@/lib/selectionScroll'
import type { Bookmark } from '@/lib/api'

const ROW_HEIGHT = 40

export function BookmarkTable() {
  const parentRef = useRef<HTMLDivElement>(null)
  const isUserLoading = useStore((s) => s.isUserLoading)
  const selectedBookmarkId = useStore((s) => s.selectedBookmarkId)
  const selectedBookmarkRevealSeq = useStore((s) => s.selectedBookmarkRevealSeq)
  const { displayBookmarks, emptyReason } = useDisplayBookmarks()
  useScrollResetOnSearch(parentRef, displayBookmarks.length)

  // eslint-disable-next-line react-hooks/incompatible-library
  const virtualizer = useVirtualizer({
    count: displayBookmarks.length,
    getScrollElement: () => parentRef.current,
    estimateSize: () => ROW_HEIGHT,
    overscan: 15,
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
  }, [selectedIndex, selectedBookmarkId, selectedBookmarkRevealSeq, virtualizer])

  if (emptyReason) return <ViewEmptyState reason={emptyReason} />

  return (
    <div ref={parentRef} data-bookmark-viewport="true" tabIndex={0} className="h-full overflow-auto focus:outline-none">
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
  const setSelectedBookmarkId = useStore((s) => s.setSelectedBookmarkId)
  const selectedBookmarkId = useStore((s) => s.selectedBookmarkId)
  const armedDeleteBookmarkId = useStore((s) => s.armedDeleteBookmarkId)
  const hiddenTags = useHiddenTags()
  const visibleTags = useMemo(
    () => bookmark.tags.filter((t) => !hiddenTags.includes(t)),
    [bookmark.tags, hiddenTags],
  )
  const selected = selectedBookmarkId === bookmark.id
  const deleteArmed = armedDeleteBookmarkId === bookmark.id

  return (
    <a
      data-bookmark-id={bookmark.id}
      href={bookmark.url}
      target="_blank"
      rel="noopener noreferrer"
      onMouseDownCapture={() => setSelectedBookmarkId(bookmark.id)}
      onClick={(e) => {
        if (e.button === 0 && !e.metaKey && !e.ctrlKey && !e.shiftKey) {
          e.preventDefault()
          setDetailModalId(bookmark.id)
        }
      }}
      className={`group relative z-0 flex items-center gap-3 border-b border-white/[0.03] px-4 py-2 no-underline ${
        bookmark.fetching
          ? 'border-l-2 border-l-hi-dim'
          : deleteArmed
            ? "z-10 border-l-2 border-l-danger shadow-[inset_0_0_0_999px_rgba(239,68,68,0.055)]"
          : selected
            ? "z-10 border-l-2 border-l-hi/65 shadow-[inset_0_0_0_999px_rgba(255,255,255,0.014)]"
            : 'hover:bg-surface-hover'
      }`}
    >
      {deleteArmed && <ArmedDeleteOverlay variant="row" />}
      <CardActions bookmarkId={bookmark.id} variant="row" />
      {/* Title */}
      <div className="min-w-0 flex-[3]">
        <span
          onClick={(e) => e.stopPropagation()}
          className={`mt-1 block truncate cursor-pointer text-[16px] font-semibold tracking-[-0.01em] ${
            selected ? 'text-white hover:text-white' : 'text-text hover:text-hi'
          }`}
        >
          <Favicon iconId={bookmark.icon_id} />{' '}
          {bookmark.title || bookmark.url}
        </span>
        {bookmark.fetching && <FetchingIndicator />}
      </div>

      {/* URL */}
      <div className="hidden min-w-0 flex-[2] sm:block">
        <span
          onClick={(e) => e.stopPropagation()}
          className={`block truncate cursor-pointer font-mono text-[11px] ${
            selected ? 'text-text-muted' : 'text-text-dim'
          }`}
        >
          {bookmark.url}
        </span>
      </div>

      {/* Tags */}
      <div className="hidden min-w-0 flex-[2] md:flex flex-wrap gap-1">
        {visibleTags.slice(0, 3).map((tag) => (
          <TagChip key={tag} tag={tag} selected={selected} />
        ))}
        {visibleTags.length > 3 && (
          <span className={`px-1 text-[11px] ${selected ? 'text-text-muted' : 'text-text-dim'}`}>
            +{visibleTags.length - 3}
          </span>
        )}
      </div>

      {/* Description snippet */}
      <div className="hidden min-w-0 flex-[2] lg:block">
        <span className={`truncate text-xs line-clamp-1 ${selected ? 'text-[#a3a3b2]' : 'text-text-muted'}`}>
          {bookmark.description}
        </span>
      </div>
    </a>
  )
})
