import { useState, useCallback, useMemo, useRef, useEffect } from 'react'
import type { Bookmark } from '@/lib/api'
import { fileUrl, deleteBookmark } from '@/lib/api'
import { useStore } from '@/lib/store'
import { Pencil, Trash2 } from 'lucide-react'

// ─── Thumbnail with styled fallback ────────────────────────────────
export function Thumbnail({
  bookmark,
  className = 'h-36 w-full rounded-t-lg',
}: {
  bookmark: Bookmark
  className?: string
}) {
  const [loaded, setLoaded] = useState(false)
  const [failed, setFailed] = useState(false)
  const hasImage = !!bookmark.image_id && !failed

  return (
    <div className={`relative overflow-hidden ${className}`}>
      {/* Fallback — always rendered, visible until image loads */}
      <div
        className={`absolute inset-0 flex items-center justify-center bg-gradient-to-br from-surface-hover to-surface transition-opacity duration-150 ${
          hasImage && loaded ? 'opacity-0' : 'opacity-100'
        }`}
      >
        <span className="text-3xl text-text-dim select-none">
          {bookmark.title?.[0]?.toUpperCase() || '?'}
        </span>
      </div>
      {/* Image — hidden until loaded */}
      {hasImage && (
        <img
          src={fileUrl(bookmark.image_id!)}
          alt=""
          loading="lazy"
          onLoad={() => setLoaded(true)}
          onError={() => setFailed(true)}
          className={`absolute inset-0 h-full w-full object-cover transition-opacity duration-150 ${
            loaded ? 'opacity-100' : 'opacity-0'
          }`}
        />
      )}
    </div>
  )
}

// ─── Favicon ───────────────────────────────────────────────────────
export function Favicon({
  iconId,
  className = 'h-4 w-4',
}: {
  iconId: string | null
  className?: string
}) {
  const [loaded, setLoaded] = useState(false)
  const [failed, setFailed] = useState(false)
  const hasIcon = !!iconId && !failed
  const inlineStyle = { verticalAlign: '-3px' as const, marginRight: '0.3em' }

  return (
    <span
      className={`relative inline-block shrink-0 ${className}`}
      style={inlineStyle}
    >
      {/* Fallback placeholder — always present, fades out when image loads */}
      <span
        className={`absolute inset-0 rounded-sm bg-surface-hover transition-opacity duration-100 ${
          hasIcon && loaded ? 'opacity-0' : 'opacity-100'
        }`}
      />
      {hasIcon && (
        <img
          src={fileUrl(iconId!)}
          alt=""
          onLoad={() => setLoaded(true)}
          onError={() => setFailed(true)}
          className={`relative h-full w-full rounded-sm object-contain transition-opacity duration-100 ${
            loaded ? 'opacity-100' : 'opacity-0'
          }`}
        />
      )}
    </span>
  )
}

// ─── URL display ───────────────────────────────────────────────────
export function UrlDisplay({ url }: { url: string }) {
  return (
    <a
      href={url}
      target="_blank"
      rel="noopener noreferrer"
      onClick={(e) => e.stopPropagation()}
      className="block truncate font-mono text-[11px] text-text-dim hover:text-text-muted"
    >
      {url}
    </a>
  )
}

// ─── Tags ──────────────────────────────────────────────────────────
export function Tags({
  tags,
  hiddenTags,
}: {
  tags: string[]
  hiddenTags: string[]
}) {
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

export function TagChip({ tag }: { tag: string }) {
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
export function Description({
  text,
  lineClamp = 3,
}: {
  text: string
  lineClamp?: number
}) {
  const [expanded, setExpanded] = useState(false)
  const [clamped, setClamped] = useState(false)
  const ref = useRef<HTMLParagraphElement>(null)

  useEffect(() => {
    const el = ref.current
    if (el) setClamped(el.scrollHeight > el.clientHeight)
  }, [text])

  if (!text) return null

  return (
    <div className="relative">
      <p
        ref={ref}
        className={`text-xs leading-relaxed text-text-muted ${
          !expanded ? 'overflow-hidden' : ''
        }`}
        style={!expanded ? { display: '-webkit-box', WebkitBoxOrient: 'vertical', WebkitLineClamp: lineClamp } : undefined}
      >
        {text}
      </p>
      {clamped && (
        <button
          tabIndex={-1}
          onClick={(e) => {
            e.stopPropagation()
            setExpanded(!expanded)
          }}
          className="mt-0.5 text-[11px] text-hi-muted hover:text-hi"
        >
          {expanded ? 'Show less' : 'Show more'}
        </button>
      )}
    </div>
  )
}

// ─── Card action buttons (hover overlay) ──────────────────────────
export function CardActions({ bookmarkId }: { bookmarkId: number }) {
  const openDetailInEditMode = useStore((s) => s.openDetailInEditMode)
  const setBookmarks = useStore((s) => s.setBookmarks)
  const bookmarks = useStore((s) => s.bookmarks)
  const detailModalId = useStore((s) => s.detailModalId)
  const setDetailModalId = useStore((s) => s.setDetailModalId)
  const [confirmDelete, setConfirmDelete] = useState(false)
  const [deleting, setDeleting] = useState(false)

  const handleEdit = useCallback(
    (e: React.MouseEvent) => {
      e.stopPropagation()
      openDetailInEditMode(bookmarkId)
    },
    [bookmarkId, openDetailInEditMode],
  )

  const handleDeleteClick = useCallback((e: React.MouseEvent) => {
    e.stopPropagation()
    setConfirmDelete(true)
  }, [])

  const handleConfirmDelete = useCallback(
    async (e: React.MouseEvent) => {
      e.stopPropagation()
      setDeleting(true)
      try {
        await deleteBookmark(bookmarkId)
        setBookmarks(bookmarks.filter((b) => b.id !== bookmarkId))
        if (detailModalId === bookmarkId) setDetailModalId(null)
      } catch {
        // Silently fail — next poll will restore if needed
      } finally {
        setDeleting(false)
        setConfirmDelete(false)
      }
    },
    [bookmarkId, bookmarks, detailModalId, setBookmarks, setDetailModalId],
  )

  const handleCancelDelete = useCallback((e: React.MouseEvent) => {
    e.stopPropagation()
    setConfirmDelete(false)
  }, [])

  if (confirmDelete) {
    return (
      <div
        className="absolute inset-0 z-10 flex items-center justify-center gap-2 rounded-lg bg-bg/90 backdrop-blur-sm"
        onClick={(e) => e.stopPropagation()}
      >
        <span className="text-sm text-danger">Delete?</span>
        <button
          tabIndex={-1}
          onClick={handleConfirmDelete}
          disabled={deleting}
          className="rounded bg-danger px-2 py-1 text-xs font-medium text-white transition-colors hover:bg-danger/80 disabled:opacity-50"
        >
          {deleting ? '...' : 'Yes'}
        </button>
        <button
          tabIndex={-1}
          onClick={handleCancelDelete}
          className="rounded bg-surface-hover px-2 py-1 text-xs font-medium text-text-muted transition-colors hover:text-text"
        >
          No
        </button>
      </div>
    )
  }

  return (
    <div className="absolute right-2 top-2 z-10 flex gap-1 opacity-0 transition-opacity group-hover:opacity-100">
      <button
        tabIndex={-1}
        onClick={handleEdit}
        className="rounded bg-bg/80 p-1.5 text-text-muted backdrop-blur-sm transition-colors hover:bg-surface-hover hover:text-text"
        title="Edit"
      >
        <Pencil className="h-3.5 w-3.5" />
      </button>
      <button
        tabIndex={-1}
        onClick={handleDeleteClick}
        className="rounded bg-bg/80 p-1.5 text-text-muted backdrop-blur-sm transition-colors hover:bg-danger/20 hover:text-danger"
        title="Delete"
      >
        <Trash2 className="h-3.5 w-3.5" />
      </button>
    </div>
  )
}

// ─── Empty state ───────────────────────────────────────────────────
export function EmptyState({
  icon,
  title,
  subtitle,
}: {
  icon: string
  title: string
  subtitle: string
}) {
  return (
    <div className="flex h-full flex-col items-center justify-center gap-3 text-center">
      <span className="text-4xl select-none">{icon}</span>
      <h2 className="text-lg font-medium text-text">{title}</h2>
      <p className="text-sm text-text-muted">{subtitle}</p>
    </div>
  )
}
