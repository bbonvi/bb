import type { Bookmark } from '@/lib/api'
import { useStore } from '@/lib/store'
import { useHiddenTags } from '@/hooks/useHiddenTags'
import { Thumbnail, Favicon, UrlDisplay, Tags, Description, CardActions } from './bookmark-parts'

interface BookmarkCardProps {
  bookmark: Bookmark
}

export function BookmarkCard({ bookmark }: BookmarkCardProps) {
  const setDetailModalId = useStore((s) => s.setDetailModalId)
  const hiddenTags = useHiddenTags()

  return (
    <article
      onClick={() => setDetailModalId(bookmark.id)}
      className="group relative flex flex-col overflow-hidden rounded-lg border border-white/[0.06] bg-surface transition-[border-color] duration-150 hover:border-white/[0.15] cursor-default"
    >
      <CardActions bookmarkId={bookmark.id} />
      <Thumbnail bookmark={bookmark} className="h-36 w-full rounded-t-lg" />
      <div className="flex flex-col gap-1.5 p-3">
        <a
          href={bookmark.url}
          target="_blank"
          rel="noopener noreferrer"
          onClick={(e) => e.stopPropagation()}
          className="line-clamp-2 text-sm font-medium leading-snug text-text hover:text-hi"
        >
          <Favicon iconId={bookmark.icon_id} />{' '}
          {bookmark.title || bookmark.url}
        </a>

        <UrlDisplay url={bookmark.url} />
        <Tags tags={bookmark.tags} hiddenTags={hiddenTags} />
        <Description text={bookmark.description} />
      </div>
    </article>
  )
}
