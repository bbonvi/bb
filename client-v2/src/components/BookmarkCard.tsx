import type { Bookmark } from '@/lib/api'
import { useStore } from '@/lib/store'
import { Thumbnail, Favicon, UrlDisplay, Tags, Description } from './bookmark-parts'

interface BookmarkCardProps {
  bookmark: Bookmark
  onClick?: () => void
}

export function BookmarkCard({ bookmark, onClick }: BookmarkCardProps) {
  const config = useStore((s) => s.config)
  const hiddenTags = config?.hidden_by_default ?? []

  return (
    <article
      onClick={onClick}
      className="group flex flex-col overflow-hidden rounded-lg border border-white/[0.06] bg-surface transition-[border-color] duration-150 hover:border-white/[0.12] cursor-default"
    >
      <Thumbnail bookmark={bookmark} className="h-36 w-full rounded-t-lg" />
      <div className="flex flex-col gap-1.5 p-3">
        {/* Title row with favicon */}
        <div className="flex items-start gap-2">
          <Favicon iconId={bookmark.icon_id} />
          <div className="min-w-0 flex-1">
            <a
              href={bookmark.url}
              target="_blank"
              rel="noopener noreferrer"
              onClick={(e) => e.stopPropagation()}
              className="line-clamp-2 text-sm font-medium leading-snug text-text hover:text-hi"
            >
              {bookmark.title || bookmark.url}
            </a>
          </div>
        </div>

        <UrlDisplay url={bookmark.url} />
        <Tags tags={bookmark.tags} hiddenTags={hiddenTags} />
        <Description text={bookmark.description} />
      </div>
    </article>
  )
}
