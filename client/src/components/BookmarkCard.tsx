import { memo } from 'react'
import type { Bookmark } from '@/lib/api'
import { useStore } from '@/lib/store'
import { useHiddenTags } from '@/hooks/useHiddenTags'
import { Thumbnail, Favicon, UrlDisplay, Tags, Description, CardActions, FetchingIndicator, ArmedDeleteOverlay } from './bookmark-parts'

interface BookmarkCardProps {
  bookmark: Bookmark
}

export const BookmarkCard = memo(function BookmarkCard({ bookmark }: BookmarkCardProps) {
  const setDetailModalId = useStore((s) => s.setDetailModalId)
  const setSelectedBookmarkId = useStore((s) => s.setSelectedBookmarkId)
  const selectedBookmarkId = useStore((s) => s.selectedBookmarkId)
  const armedDeleteBookmarkId = useStore((s) => s.armedDeleteBookmarkId)
  const hiddenTags = useHiddenTags()
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
      className={`group relative z-0 flex flex-col overflow-visible rounded-lg border bg-surface no-underline ${
        bookmark.fetching
          ? 'fetching-glow'
          : deleteArmed
            ? "z-10 border-danger/55 shadow-[inset_0_0_0_999px_rgba(239,68,68,0.06),0_0_0_1px_rgba(239,68,68,0.18)]"
          : selected
            ? "z-10 border-hi/55 shadow-[inset_0_0_0_999px_rgba(255,255,255,0.016),0_0_0_1px_rgba(107,138,253,0.12)]"
            : 'border-white/[0.06] hover:border-white/[0.15]'
      }`}
    >
      {deleteArmed && <ArmedDeleteOverlay />}
      <CardActions bookmarkId={bookmark.id} />
      <div className="relative">
        <Thumbnail bookmark={bookmark} className="h-36 w-full rounded-t-lg" />
        {bookmark.fetching && (
          <div className="absolute inset-0 flex items-center justify-center rounded-t-lg bg-surface">
            <FetchingIndicator />
          </div>
        )}
      </div>
      <div className="flex flex-col gap-1.5 p-3">
        <span
          onClick={(e) => e.stopPropagation()}
          className={`mt-1.5 line-clamp-2 cursor-pointer text-[16px] font-semibold leading-snug tracking-[-0.01em] ${
            selected ? 'text-white hover:text-white' : 'text-text hover:text-hi'
          }`}
        >
          <Favicon iconId={bookmark.icon_id} />{' '}
          {bookmark.title || bookmark.url}
        </span>

        <UrlDisplay url={bookmark.url} selected={selected} />
        <Tags tags={bookmark.tags} hiddenTags={hiddenTags} selected={selected} />
        <Description text={bookmark.description} selected={selected} />
      </div>
    </a>
  )
})
