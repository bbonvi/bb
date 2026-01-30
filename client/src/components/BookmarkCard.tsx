import { memo } from 'react'
import type { Bookmark } from '@/lib/api'
import { useStore } from '@/lib/store'
import { useHiddenTags } from '@/hooks/useHiddenTags'
import { Thumbnail, Favicon, UrlDisplay, Tags, Description, CardActions, FetchingIndicator } from './bookmark-parts'

interface BookmarkCardProps {
  bookmark: Bookmark
}

export const BookmarkCard = memo(function BookmarkCard({ bookmark }: BookmarkCardProps) {
  const setDetailModalId = useStore((s) => s.setDetailModalId)
  const hiddenTags = useHiddenTags()

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
      className={`group relative flex flex-col overflow-hidden rounded-lg border bg-surface transition-[border-color] duration-150 no-underline ${
        bookmark.fetching
          ? 'fetching-glow'
          : 'border-white/[0.06] hover:border-white/[0.15]'
      }`}
    >
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
        <span className="line-clamp-2 text-sm font-medium leading-snug text-text hover:text-hi">
          <Favicon iconId={bookmark.icon_id} />{' '}
          {bookmark.title || bookmark.url}
        </span>

        <UrlDisplay url={bookmark.url} />
        <Tags tags={bookmark.tags} hiddenTags={hiddenTags} />
        <Description text={bookmark.description} />
      </div>
    </a>
  )
})
