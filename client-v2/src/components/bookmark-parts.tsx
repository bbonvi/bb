import { useState, useCallback, useMemo, useRef, useEffect } from 'react'
import type { Bookmark } from '@/lib/api'
import { fileUrl, deleteBookmark } from '@/lib/api'
import { useStore } from '@/lib/store'
import { CircleHelp, Pencil, Trash2, Upload } from 'lucide-react'

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

// ─── Double-click confirm button ──────────────────────────────────
// First click arms (shows confirm state). Second click executes.
// Mouse leave disarms.
export function ConfirmButton({
  onConfirm,
  icon,
  armedIcon,
  iconClass = 'h-3.5 w-3.5',
  className = '',
  title = 'Confirm',
  armedTitle = 'Click again to confirm',
  colorClass = 'text-text-muted hover:bg-danger/20 hover:text-danger',
  armedColorClass = 'bg-danger/20 text-danger',
  stopPropagation = false,
  disabled = false,
  children,
  armedChildren,
}: {
  onConfirm: () => void | Promise<void>
  icon?: React.ReactNode
  armedIcon?: React.ReactNode
  iconClass?: string
  className?: string
  title?: string
  armedTitle?: string
  colorClass?: string
  armedColorClass?: string
  stopPropagation?: boolean
  disabled?: boolean
  children?: React.ReactNode
  armedChildren?: React.ReactNode
}) {
  const [armed, setArmed] = useState(false)
  const [busy, setBusy] = useState(false)

  const handleClick = useCallback(
    async (e: React.MouseEvent) => {
      if (stopPropagation) e.stopPropagation()
      if (!armed) {
        setArmed(true)
        return
      }
      setBusy(true)
      try {
        await onConfirm()
      } finally {
        setBusy(false)
        setArmed(false)
      }
    },
    [armed, onConfirm, stopPropagation],
  )

  const defaultIcon = <Trash2 className={iconClass} />
  const defaultArmedIcon = <CircleHelp className={iconClass} />

  // Render both states in a grid overlay so the button keeps the larger width
  const hasSwap = armedChildren != null
  const normalContent = <>{icon ?? defaultIcon}{children}</>
  const armedContent = <>{armedIcon ?? defaultArmedIcon}{armedChildren ?? children}</>

  return (
    <button
      tabIndex={-1}
      onClick={handleClick}
      onMouseLeave={() => setArmed(false)}
      disabled={busy || disabled}
      className={`rounded p-1.5 transition-all duration-200 disabled:opacity-50 ${
        armed ? armedColorClass : colorClass
      } ${className}`}
      title={armed ? armedTitle : title}
    >
      {hasSwap ? (
        <span className="inline-grid [&>*]:col-start-1 [&>*]:row-start-1">
          <span className={`inline-flex items-center transition-opacity duration-150 ${armed ? 'opacity-0' : 'opacity-100'}`}>
            {normalContent}
          </span>
          <span className={`inline-flex items-center transition-opacity duration-150 ${armed ? 'opacity-100' : 'opacity-0'}`}>
            {armedContent}
          </span>
        </span>
      ) : (
        <span className="inline-flex items-center">
          {armed ? armedContent : normalContent}
        </span>
      )}
    </button>
  )
}

// Backward-compatible thin wrapper
export function DeleteButton({
  onDelete,
  iconClass,
  className,
  stopPropagation,
}: {
  onDelete: () => void | Promise<void>
  iconClass?: string
  className?: string
  stopPropagation?: boolean
}) {
  return (
    <ConfirmButton
      onConfirm={onDelete}
      iconClass={iconClass}
      className={className}
      stopPropagation={stopPropagation}
      title="Delete"
      armedTitle="Click again to confirm"
    />
  )
}

// ─── Card action buttons (hover overlay) ──────────────────────────
export function CardActions({ bookmarkId, variant = 'card' }: { bookmarkId: number; variant?: 'card' | 'row' }) {
  const openDetailInEditMode = useStore((s) => s.openDetailInEditMode)
  const setBookmarks = useStore((s) => s.setBookmarks)
  const bookmarks = useStore((s) => s.bookmarks)
  const detailModalId = useStore((s) => s.detailModalId)
  const setDetailModalId = useStore((s) => s.setDetailModalId)

  const handleEdit = useCallback(
    (e: React.MouseEvent) => {
      e.stopPropagation()
      openDetailInEditMode(bookmarkId)
    },
    [bookmarkId, openDetailInEditMode],
  )

  const handleDelete = useCallback(async () => {
    await deleteBookmark(bookmarkId)
    setBookmarks(bookmarks.filter((b) => b.id !== bookmarkId))
    if (detailModalId === bookmarkId) setDetailModalId(null)
  }, [bookmarkId, bookmarks, detailModalId, setBookmarks, setDetailModalId])

  return (
    <div className={`absolute right-2 z-10 flex gap-1 opacity-0 transition-opacity group-hover:opacity-100 ${variant === 'card' ? 'top-2' : 'top-1/2 -translate-y-1/2'}`}>
      <button
        tabIndex={-1}
        onClick={handleEdit}
        className="rounded bg-bg/80 p-1.5 text-text-muted backdrop-blur-sm transition-colors hover:bg-surface-hover hover:text-text"
        title="Edit"
      >
        <Pencil className="h-3.5 w-3.5" />
      </button>
      <DeleteButton
        onDelete={handleDelete}
        stopPropagation
        className="bg-bg/80 backdrop-blur-sm"
      />
    </div>
  )
}

// ─── Image drop zone (edit mode) ──────────────────────────────────
// Reusable component for click-to-upload + drag-and-drop image upload
export function ImageDropZone({
  onUpload,
  children,
  className = '',
  label = 'Upload image',
}: {
  onUpload: (file: File) => void
  children: React.ReactNode
  className?: string
  label?: string
}) {
  const [dragOver, setDragOver] = useState(false)
  const inputRef = useRef<HTMLInputElement>(null)

  const handleDrop = useCallback(
    (e: React.DragEvent) => {
      e.preventDefault()
      e.stopPropagation()
      setDragOver(false)
      const file = e.dataTransfer.files[0]
      if (file && file.type.startsWith('image/')) onUpload(file)
    },
    [onUpload],
  )

  const handleDragOver = useCallback((e: React.DragEvent) => {
    e.preventDefault()
    e.stopPropagation()
    setDragOver(true)
  }, [])

  const handleDragLeave = useCallback((e: React.DragEvent) => {
    e.preventDefault()
    e.stopPropagation()
    setDragOver(false)
  }, [])

  const handleClick = useCallback(() => inputRef.current?.click(), [])

  const handleFileChange = useCallback(
    (e: React.ChangeEvent<HTMLInputElement>) => {
      const file = e.target.files?.[0]
      if (file) onUpload(file)
      e.target.value = '' // reset so same file can be re-selected
    },
    [onUpload],
  )

  return (
    <div
      onClick={handleClick}
      onDrop={handleDrop}
      onDragOver={handleDragOver}
      onDragLeave={handleDragLeave}
      className={`group/drop relative cursor-pointer ${className}`}
      title={label}
    >
      {/* Drag overlay */}
      {dragOver && (
        <div className="absolute inset-0 z-20 flex items-center justify-center rounded bg-accent/20 ring-2 ring-accent">
          <Upload className="h-8 w-8 text-accent" />
        </div>
      )}
      {/* Dim existing content on dragover */}
      <div className={`transition-opacity duration-100 ${dragOver ? 'opacity-50' : 'opacity-100'}`}>
        {children}
      </div>
      {/* Upload hint on hover (not during drag) */}
      {!dragOver && (
        <div className="absolute inset-0 z-10 flex items-center justify-center rounded bg-black/40 opacity-0 transition-opacity duration-100 group-hover/drop:opacity-100">
          <Upload className="h-6 w-6 text-white/70" />
        </div>
      )}
      <input
        ref={inputRef}
        type="file"
        accept="image/*"
        className="hidden"
        onChange={handleFileChange}
      />
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
