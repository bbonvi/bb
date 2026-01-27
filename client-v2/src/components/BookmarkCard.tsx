import { useState, useCallback, useMemo } from 'react'
import type { Bookmark } from '@/lib/api'
import { fileUrl } from '@/lib/api'
import { useStore } from '@/lib/store'

// ─── Thumbnail with styled fallback ────────────────────────────────
function Thumbnail({ bookmark }: { bookmark: Bookmark }) {
  const [failed, setFailed] = useState(false)

  if (!bookmark.image_id || failed) {
    return (
      <div className="flex h-36 w-full items-center justify-center rounded-t-lg bg-gradient-to-br from-surface-hover to-surface">
        <span className="text-3xl text-text-dim select-none">
          {bookmark.title?.[0]?.toUpperCase() || '?'}
        </span>
      </div>
    )
  }

  return (
    <img
      src={fileUrl(bookmark.image_id)}
      alt=""
      loading="lazy"
      onError={() => setFailed(true)}
      className="h-36 w-full rounded-t-lg object-cover"
    />
  )
}

// ─── Favicon ───────────────────────────────────────────────────────
function Favicon({ iconId }: { iconId: string | null }) {
  const [failed, setFailed] = useState(false)

  if (!iconId || failed) {
    return <div className="h-4 w-4 shrink-0 rounded-sm bg-surface-hover" />
  }

  return (
    <img
      src={fileUrl(iconId)}
      alt=""
      onError={() => setFailed(true)}
      className="h-4 w-4 shrink-0 rounded-sm object-contain"
    />
  )
}

// ─── URL display ───────────────────────────────────────────────────
function UrlDisplay({ url }: { url: string }) {
  const display = useMemo(() => {
    try {
      const u = new URL(url)
      return u.hostname + (u.pathname !== '/' ? u.pathname : '')
    } catch {
      return url
    }
  }, [url])

  return (
    <span className="truncate font-mono text-[11px] text-text-dim">
      {display}
    </span>
  )
}

// ─── Tags ──────────────────────────────────────────────────────────
function Tags({ tags, hiddenTags }: { tags: string[]; hiddenTags: string[] }) {
  const visible = useMemo(
    () => tags.filter((t) => !hiddenTags.includes(t)),
    [tags, hiddenTags],
  )

  if (visible.length === 0) return null

  return (
    <div className="flex flex-wrap gap-1">
      {visible.map((tag) => (
        <TagChip key={tag} tag={tag} />
      ))}
    </div>
  )
}

function TagChip({ tag }: { tag: string }) {
  const setSearchQuery = useStore((s) => s.setSearchQuery)
  const searchQuery = useStore((s) => s.searchQuery)

  const handleClick = useCallback(
    (e: React.MouseEvent) => {
      e.stopPropagation()
      const current = searchQuery.tags ?? ''
      const tagList = current ? current.split(',').map((t) => t.trim()) : []
      if (!tagList.includes(tag)) {
        setSearchQuery({ ...searchQuery, tags: [...tagList, tag].join(',') })
      }
    },
    [tag, searchQuery, setSearchQuery],
  )

  return (
    <button
      tabIndex={-1}
      onClick={handleClick}
      className="rounded-md bg-surface-hover px-1.5 py-0.5 font-mono text-[11px] text-text-muted transition-colors hover:bg-surface-active hover:text-text"
    >
      #{tag}
    </button>
  )
}

// ─── Description with expand ───────────────────────────────────────
function Description({ text }: { text: string }) {
  const [expanded, setExpanded] = useState(false)
  const isLong = text.length > 150

  if (!text) return null

  return (
    <div className="relative">
      <p
        className={`text-xs leading-relaxed text-text-muted ${
          !expanded && isLong ? 'line-clamp-3' : ''
        }`}
      >
        {text}
      </p>
      {isLong && (
        <button
          tabIndex={-1}
          onClick={(e) => {
            e.stopPropagation()
            setExpanded(!expanded)
          }}
          className="mt-0.5 text-[11px] text-accent-muted hover:text-accent"
        >
          {expanded ? 'Show less' : 'Show more'}
        </button>
      )}
    </div>
  )
}

// ─── Main card component ───────────────────────────────────────────
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
      className="group flex cursor-pointer flex-col overflow-hidden rounded-lg border border-white/[0.06] bg-surface transition-colors hover:border-white/[0.1] hover:bg-surface-hover"
    >
      <Thumbnail bookmark={bookmark} />
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
              className="line-clamp-2 text-sm font-medium leading-snug text-text hover:text-accent"
            >
              {bookmark.title || bookmark.url}
            </a>
          </div>
        </div>

        {/* URL */}
        <UrlDisplay url={bookmark.url} />

        {/* Tags */}
        <Tags tags={bookmark.tags} hiddenTags={hiddenTags} />

        {/* Description */}
        <Description text={bookmark.description} />
      </div>
    </article>
  )
}
