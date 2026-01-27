import { useState, useCallback, useEffect, useMemo } from 'react'
import {
  Dialog,
  DialogContent,
  DialogTitle,
} from '@/components/ui/dialog'
import { Button } from '@/components/ui/button'
import { Input } from '@/components/ui/input'
import { useStore } from '@/lib/store'
import { useDisplayBookmarks } from '@/hooks/useDisplayBookmarks'
import { updateBookmark, deleteBookmark, refreshMetadata, normalizeTags, toBase64, fileUrl } from '@/lib/api'
import type { Bookmark } from '@/lib/api'
import { Thumbnail, Favicon, Tags, UrlDisplay, DeleteButton, ImageDropZone } from './bookmark-parts'
import {
  ChevronLeft,
  ChevronRight,
  Pencil,
  RefreshCw,
  X,
  Check,
  ExternalLink,
} from 'lucide-react'

export function BookmarkDetailModal() {
  const detailModalId = useStore((s) => s.detailModalId)
  const detailModalEdit = useStore((s) => s.detailModalEdit)
  const setDetailModalId = useStore((s) => s.setDetailModalId)
  const bookmarks = useStore((s) => s.bookmarks)
  const config = useStore((s) => s.config)
  const markDirty = useStore((s) => s.markDirty)
  const clearDirty = useStore((s) => s.clearDirty)
  const setBookmarks = useStore((s) => s.setBookmarks)
  const hiddenTags = config?.hidden_by_default ?? []

  const { displayBookmarks } = useDisplayBookmarks()

  const [editing, setEditing] = useState(false)
  const [editForm, setEditForm] = useState({ title: '', description: '', url: '', tags: '' })
  const [saving, setSaving] = useState(false)
  const [refreshing, setRefreshing] = useState(false)
  const [error, setError] = useState<string | null>(null)
  // Local preview URLs for optimistic display after upload
  const [coverPreview, setCoverPreview] = useState<string | null>(null)
  const [iconPreview, setIconPreview] = useState<string | null>(null)

  // Find the bookmark by ID from the full bookmarks array (not display — may be reversed/shuffled)
  const bookmark = useMemo(
    () => bookmarks.find((b) => b.id === detailModalId) ?? null,
    [bookmarks, detailModalId],
  )

  // Current index in display order for prev/next navigation
  const currentIndex = useMemo(
    () => (detailModalId !== null ? displayBookmarks.findIndex((b) => b.id === detailModalId) : -1),
    [displayBookmarks, detailModalId],
  )

  const uploadImage = useCallback(async (file: File, field: 'image_b64' | 'icon_b64') => {
    if (!bookmark) return
    markDirty(bookmark.id)
    const previewUrl = URL.createObjectURL(file)
    if (field === 'image_b64') setCoverPreview(previewUrl)
    else setIconPreview(previewUrl)
    try {
      const b64 = await toBase64(file)
      const updated = await updateBookmark({ id: bookmark.id, [field]: b64 })
      setBookmarks(bookmarks.map((b) => (b.id === updated.id ? updated : b)))
    } catch (e) {
      setError(e instanceof Error ? e.message : 'Failed to upload image')
      if (field === 'image_b64') setCoverPreview(null)
      else setIconPreview(null)
    } finally {
      clearDirty(bookmark.id)
    }
  }, [bookmark, bookmarks, markDirty, clearDirty, setBookmarks])

  const handleCoverUpload = useCallback((file: File) => uploadImage(file, 'image_b64'), [uploadImage])
  const handleIconUpload = useCallback((file: File) => uploadImage(file, 'icon_b64'), [uploadImage])

  // Clipboard paste for cover image (global, edit mode only)
  useEffect(() => {
    if (!editing || detailModalId === null) return
    const handler = (e: ClipboardEvent) => {
      const items = e.clipboardData?.items
      if (!items) return
      for (const item of items) {
        if (item.type.startsWith('image/')) {
          e.preventDefault()
          const file = item.getAsFile()
          if (file) handleCoverUpload(file)
          return
        }
      }
    }
    window.addEventListener('paste', handler)
    return () => window.removeEventListener('paste', handler)
  }, [editing, detailModalId, handleCoverUpload])

  const canPrev = currentIndex > 0
  const canNext = currentIndex >= 0 && currentIndex < displayBookmarks.length - 1

  const navigate = useCallback(
    (direction: -1 | 1) => {
      const nextIdx = currentIndex + direction
      if (nextIdx >= 0 && nextIdx < displayBookmarks.length) {
        setEditing(false)
        setError(null)
        setDetailModalId(displayBookmarks[nextIdx].id)
      }
    },
    [currentIndex, displayBookmarks, setDetailModalId],
  )

  // Reset state when modal opens/closes or bookmark changes
  useEffect(() => {
    if (detailModalEdit && bookmark) {
      setEditForm({
        title: bookmark.title,
        description: bookmark.description,
        url: bookmark.url,
        tags: bookmark.tags.filter((t) => !hiddenTags.includes(t)).join(', '),
      })
      setEditing(true)
    } else {
      setEditing(false)
    }
    setError(null)
    setCoverPreview(null)
    setIconPreview(null)
  }, [detailModalId])

  const startEdit = useCallback(() => {
    if (!bookmark) return
    setEditForm({
      title: bookmark.title,
      description: bookmark.description,
      url: bookmark.url,
      tags: bookmark.tags.filter((t) => !hiddenTags.includes(t)).join(', '),
    })
    setEditing(true)
    setError(null)
  }, [bookmark, hiddenTags])

  const cancelEdit = useCallback(() => {
    setEditing(false)
    setError(null)
  }, [])

  const saveEdit = useCallback(async () => {
    if (!bookmark) return
    setSaving(true)
    setError(null)
    markDirty(bookmark.id)
    try {
      const updated = await updateBookmark({
        id: bookmark.id,
        title: editForm.title,
        description: editForm.description,
        url: editForm.url,
        tags: normalizeTags(editForm.tags),
      })
      // Update in local bookmarks array
      setBookmarks(bookmarks.map((b) => (b.id === updated.id ? updated : b)))
      setEditing(false)
    } catch (e) {
      setError(e instanceof Error ? e.message : 'Failed to save')
    } finally {
      clearDirty(bookmark.id)
      setSaving(false)
    }
  }, [bookmark, editForm, bookmarks, markDirty, clearDirty, setBookmarks])

  const handleDelete = useCallback(async () => {
    if (!bookmark) return
    setError(null)
    try {
      await deleteBookmark(bookmark.id)
      setBookmarks(bookmarks.filter((b) => b.id !== bookmark.id))
      setDetailModalId(null)
    } catch (e) {
      setError(e instanceof Error ? e.message : 'Failed to delete')
    }
  }, [bookmark, bookmarks, setBookmarks, setDetailModalId])

  const handleRefreshMetadata = useCallback(async () => {
    if (!bookmark) return
    setRefreshing(true)
    setError(null)
    try {
      await refreshMetadata(bookmark.id, { async_meta: true })
    } catch (e) {
      setError(e instanceof Error ? e.message : 'Failed to refresh metadata')
    } finally {
      setRefreshing(false)
    }
  }, [bookmark])

  // Keyboard: arrows for nav, Ctrl+Enter to save, Escape to discard edit first
  useEffect(() => {
    if (detailModalId === null) return
    const handler = (e: KeyboardEvent) => {
      if (editing) {
        if (e.key === 'Enter' && (e.ctrlKey || e.metaKey) && !saving) {
          e.preventDefault()
          saveEdit()
        }
        if (e.key === 'Escape') {
          e.preventDefault()
          e.stopPropagation()
          cancelEdit()
        }
        return
      }
      if (e.key === 'ArrowLeft' && canPrev) navigate(-1)
      if (e.key === 'ArrowRight' && canNext) navigate(1)
    }
    window.addEventListener('keydown', handler, true)
    return () => window.removeEventListener('keydown', handler, true)
  }, [detailModalId, editing, saving, canPrev, canNext, navigate, saveEdit, cancelEdit])

  const open = detailModalId !== null && bookmark !== null

  return (
    <Dialog open={open} onOpenChange={(isOpen) => { if (!isOpen) setDetailModalId(null) }}>
      <DialogContent
        className="flex h-[min(78vh,780px)] w-full max-w-2xl flex-col gap-0 overflow-hidden bg-surface p-0"
        showCloseButton={false}
      >
        {bookmark && (
          <>
            {/* Header with nav + close */}
            <div className="flex items-center justify-between border-b border-white/[0.06] px-4 py-3">
              <div className="flex items-center gap-1">
                <button
                  tabIndex={-1}
                  onClick={() => navigate(-1)}
                  disabled={!canPrev}
                  className="rounded p-1 text-text-muted transition-colors hover:bg-surface-hover hover:text-text disabled:opacity-25 disabled:hover:bg-transparent"
                  title="Previous (Left arrow)"
                >
                  <ChevronLeft className="h-4 w-4" />
                </button>
                <button
                  tabIndex={-1}
                  onClick={() => navigate(1)}
                  disabled={!canNext}
                  className="rounded p-1 text-text-muted transition-colors hover:bg-surface-hover hover:text-text disabled:opacity-25 disabled:hover:bg-transparent"
                  title="Next (Right arrow)"
                >
                  <ChevronRight className="h-4 w-4" />
                </button>
                {currentIndex >= 0 && (
                  <span className="ml-2 font-mono text-[11px] text-text-dim">
                    {currentIndex + 1}/{displayBookmarks.length}
                  </span>
                )}
              </div>
              <DialogTitle className="sr-only">Bookmark Details</DialogTitle>
              <button
                tabIndex={-1}
                onClick={() => setDetailModalId(null)}
                className="rounded p-1 text-text-muted transition-colors hover:bg-surface-hover hover:text-text"
              >
                <X className="h-4 w-4" />
              </button>
            </div>

            {/* Scrollable body */}
            <div className="flex-1 overflow-y-auto">
              {/* Thumbnail — wraps in drop zone during edit mode */}
              {editing ? (
                <ImageDropZone onUpload={handleCoverUpload} label="Upload cover image" className="h-48 w-full sm:h-64">
                  {coverPreview ? (
                    <img src={coverPreview} alt="" className="h-48 w-full object-cover sm:h-64" />
                  ) : (
                    <Thumbnail bookmark={bookmark} className="h-48 w-full sm:h-64" />
                  )}
                </ImageDropZone>
              ) : (
                <Thumbnail bookmark={bookmark} className="h-48 w-full sm:h-64" />
              )}

              {/* Content */}
              <div className="flex flex-col gap-4 px-4 pt-4 pb-0 sm:px-6 sm:pt-6">
                {error && (
                  <div className="rounded-md bg-danger/10 px-3 py-2 text-sm text-danger">
                    {error}
                  </div>
                )}

                {editing ? (
                  <EditForm
                    form={editForm}
                    onChange={setEditForm}
                    bookmark={bookmark}
                    iconPreview={iconPreview}
                    onIconUpload={handleIconUpload}
                  />
                ) : (
                  <ViewContent
                    bookmark={bookmark}
                    hiddenTags={hiddenTags}
                  />
                )}
              </div>
            </div>

            {/* Footer actions */}
            <div className="flex items-center justify-between border-t border-white/[0.06] px-4 py-3 sm:px-6">
              <div className="flex items-center gap-2">
                <DeleteButton onDelete={handleDelete} iconClass="h-4 w-4" />
              </div>
              <div className="flex items-center gap-2">
                {editing ? (
                  <>
                    <Button variant="ghost" size="sm" onClick={cancelEdit} disabled={saving}>
                      Cancel
                    </Button>
                    <Button size="sm" onClick={saveEdit} disabled={saving}>
                      <Check className="mr-1 h-3.5 w-3.5" />
                      {saving ? 'Saving...' : 'Save'}
                    </Button>
                  </>
                ) : (
                  <>
                    <button
                      tabIndex={-1}
                      onClick={handleRefreshMetadata}
                      disabled={refreshing}
                      className="rounded p-1.5 text-text-muted transition-colors hover:bg-surface-hover hover:text-text disabled:opacity-50"
                      title="Refresh metadata"
                    >
                      <RefreshCw className={`h-4 w-4 ${refreshing ? 'animate-spin' : ''}`} />
                    </button>
                    <button
                      tabIndex={-1}
                      onClick={startEdit}
                      className="rounded p-1.5 text-text-muted transition-colors hover:bg-surface-hover hover:text-text"
                      title="Edit"
                    >
                      <Pencil className="h-4 w-4" />
                    </button>
                    <a
                      href={bookmark.url}
                      target="_blank"
                      rel="noopener noreferrer"
                      className="rounded p-1.5 text-text-muted transition-colors hover:bg-surface-hover hover:text-text"
                      title="Open URL"
                    >
                      <ExternalLink className="h-4 w-4" />
                    </a>
                  </>
                )}
              </div>
            </div>
          </>
        )}
      </DialogContent>
    </Dialog>
  )
}

// ─── View mode ────────────────────────────────────────────────────

function ViewContent({
  bookmark,
  hiddenTags,
}: {
  bookmark: Bookmark
  hiddenTags: string[]
}) {
  return (
    <div className="flex flex-col gap-3">
      <h2 className="break-words text-lg font-medium leading-snug text-text">
        <Favicon iconId={bookmark.icon_id} className="h-5 w-5" />{' '}
        <a
          href={bookmark.url}
          target="_blank"
          rel="noopener noreferrer"
          className="hover:text-hi"
        >
          {bookmark.title || bookmark.url}
        </a>
      </h2>

      {/* URL */}
      <UrlDisplay url={bookmark.url} />

      {/* Tags */}
      <Tags tags={bookmark.tags} hiddenTags={hiddenTags} />

      {/* Description */}
      {bookmark.description && (
        <p className="whitespace-pre-wrap text-sm leading-relaxed text-text-muted">
          {bookmark.description}
        </p>
      )}
    </div>
  )
}

// ─── Edit mode ────────────────────────────────────────────────────

interface EditFormState {
  title: string
  description: string
  url: string
  tags: string
}

function EditForm({
  form,
  onChange,
  bookmark,
  iconPreview,
  onIconUpload,
}: {
  form: EditFormState
  onChange: (form: EditFormState) => void
  bookmark: Bookmark
  iconPreview: string | null
  onIconUpload: (file: File) => void
}) {
  const update = (field: keyof EditFormState, value: string) =>
    onChange({ ...form, [field]: value })

  return (
    <div className="flex flex-col gap-3">
      {/* Icon upload */}
      <div className="flex items-center gap-3">
        <ImageDropZone onUpload={onIconUpload} label="Upload icon" className="h-10 w-10 shrink-0 rounded-md">
          <EditableIcon iconId={bookmark.icon_id} previewUrl={iconPreview} />
        </ImageDropZone>
        <span className="text-xs text-text-dim">Click or drag to change icon</span>
      </div>

      <label className="flex flex-col gap-1">
        <span className="text-xs font-medium text-text-muted">Title</span>
        <Input
          value={form.title}
          onChange={(e) => update('title', e.target.value)}
          className="bg-surface-hover"
        />
      </label>
      <label className="flex flex-col gap-1">
        <span className="text-xs font-medium text-text-muted">URL</span>
        <Input
          value={form.url}
          onChange={(e) => update('url', e.target.value)}
          className="bg-surface-hover"
        />
      </label>
      <label className="flex flex-col gap-1">
        <span className="text-xs font-medium text-text-muted">Tags</span>
        <Input
          value={form.tags}
          onChange={(e) => update('tags', e.target.value)}
          placeholder="comma or space separated"
          className="bg-surface-hover"
        />
      </label>
      <label className="flex flex-col gap-1">
        <span className="text-xs font-medium text-text-muted">Description</span>
        <textarea
          value={form.description}
          onChange={(e) => update('description', e.target.value)}
          rows={4}
          className="resize-none rounded-md border border-border bg-surface-hover px-3 py-2 text-sm text-text outline-none focus:ring-1 focus:ring-ring"
        />
      </label>
    </div>
  )
}

// Icon with proper fallback for edit mode — matches Favicon's plain square pattern
function EditableIcon({ iconId, previewUrl }: { iconId: string | null; previewUrl: string | null }) {
  const [failed, setFailed] = useState(false)
  const src = previewUrl ?? (iconId && !failed ? fileUrl(iconId) : null)

  // Reset error state when icon changes
  useEffect(() => { setFailed(false) }, [iconId, previewUrl])

  return (
    <div className="relative h-10 w-10 overflow-hidden rounded-md">
      <div className={`absolute inset-0 bg-surface-hover transition-opacity duration-100 ${src ? 'opacity-0' : 'opacity-100'}`} />
      {src && (
        <img
          src={src}
          alt=""
          onError={() => setFailed(true)}
          className="absolute inset-0 h-full w-full object-contain"
        />
      )}
    </div>
  )
}
