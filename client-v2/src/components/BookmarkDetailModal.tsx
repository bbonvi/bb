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
import { updateBookmark, deleteBookmark, refreshMetadata } from '@/lib/api'
import type { Bookmark } from '@/lib/api'
import { Thumbnail, Favicon, Tags, UrlDisplay } from './bookmark-parts'
import {
  ChevronLeft,
  ChevronRight,
  Pencil,
  Trash2,
  RefreshCw,
  X,
  Check,
  ExternalLink,
} from 'lucide-react'

export function BookmarkDetailModal() {
  const detailModalId = useStore((s) => s.detailModalId)
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
  const [deleting, setDeleting] = useState(false)
  const [confirmDelete, setConfirmDelete] = useState(false)
  const [refreshing, setRefreshing] = useState(false)
  const [error, setError] = useState<string | null>(null)

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

  const canPrev = currentIndex > 0
  const canNext = currentIndex >= 0 && currentIndex < displayBookmarks.length - 1

  const navigate = useCallback(
    (direction: -1 | 1) => {
      const nextIdx = currentIndex + direction
      if (nextIdx >= 0 && nextIdx < displayBookmarks.length) {
        setEditing(false)
        setConfirmDelete(false)
        setError(null)
        setDetailModalId(displayBookmarks[nextIdx].id)
      }
    },
    [currentIndex, displayBookmarks, setDetailModalId],
  )

  // Keyboard: left/right arrows for prev/next
  useEffect(() => {
    if (detailModalId === null) return
    const handler = (e: KeyboardEvent) => {
      if (editing) return // don't navigate while editing
      if (e.key === 'ArrowLeft' && canPrev) navigate(-1)
      if (e.key === 'ArrowRight' && canNext) navigate(1)
    }
    window.addEventListener('keydown', handler)
    return () => window.removeEventListener('keydown', handler)
  }, [detailModalId, editing, canPrev, canNext, navigate])

  // Reset state when modal opens/closes or bookmark changes
  useEffect(() => {
    setEditing(false)
    setConfirmDelete(false)
    setError(null)
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
        tags: editForm.tags,
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
    setDeleting(true)
    setError(null)
    try {
      await deleteBookmark(bookmark.id)
      setBookmarks(bookmarks.filter((b) => b.id !== bookmark.id))
      setDetailModalId(null)
    } catch (e) {
      setError(e instanceof Error ? e.message : 'Failed to delete')
    } finally {
      setDeleting(false)
      setConfirmDelete(false)
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

  const open = detailModalId !== null && bookmark !== null

  return (
    <Dialog open={open} onOpenChange={(isOpen) => { if (!isOpen) setDetailModalId(null) }}>
      <DialogContent
        className="flex max-h-[90vh] w-full max-w-2xl flex-col gap-0 overflow-hidden bg-surface p-0"
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
              {/* Thumbnail */}
              <Thumbnail
                bookmark={bookmark}
                className="h-48 w-full sm:h-64"
              />

              {/* Content */}
              <div className="flex flex-col gap-4 p-4 sm:p-6">
                {error && (
                  <div className="rounded-md bg-danger/10 px-3 py-2 text-sm text-danger">
                    {error}
                  </div>
                )}

                {editing ? (
                  <EditForm
                    form={editForm}
                    onChange={setEditForm}
                    onSave={saveEdit}
                    onCancel={cancelEdit}
                    saving={saving}
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
            {!editing && (
              <div className="flex items-center justify-between border-t border-white/[0.06] px-4 py-3 sm:px-6">
                <div className="flex items-center gap-2">
                  {confirmDelete ? (
                    <>
                      <span className="text-sm text-danger">Delete this bookmark?</span>
                      <Button
                        variant="destructive"
                        size="sm"
                        onClick={handleDelete}
                        disabled={deleting}
                      >
                        {deleting ? 'Deleting...' : 'Confirm'}
                      </Button>
                      <Button
                        variant="ghost"
                        size="sm"
                        onClick={() => setConfirmDelete(false)}
                      >
                        Cancel
                      </Button>
                    </>
                  ) : (
                    <button
                      tabIndex={-1}
                      onClick={() => setConfirmDelete(true)}
                      className="rounded p-1.5 text-text-muted transition-colors hover:bg-danger/10 hover:text-danger"
                      title="Delete"
                    >
                      <Trash2 className="h-4 w-4" />
                    </button>
                  )}
                </div>
                <div className="flex items-center gap-2">
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
                </div>
              </div>
            )}
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
      {/* Title + favicon */}
      <div className="flex items-start gap-3">
        <Favicon iconId={bookmark.icon_id} />
        <h2 className="text-lg font-medium leading-snug text-text">
          {bookmark.title || bookmark.url}
        </h2>
      </div>

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
  onSave,
  onCancel,
  saving,
}: {
  form: EditFormState
  onChange: (form: EditFormState) => void
  onSave: () => void
  onCancel: () => void
  saving: boolean
}) {
  const update = (field: keyof EditFormState, value: string) =>
    onChange({ ...form, [field]: value })

  return (
    <div className="flex flex-col gap-3">
      <label className="flex flex-col gap-1">
        <span className="text-xs font-medium text-text-muted">Title</span>
        <Input
          value={form.title}
          onChange={(e) => update('title', e.target.value)}
          onKeyDown={(e) => { if (e.key === 'Escape') onCancel() }}
          className="bg-surface-hover"
        />
      </label>
      <label className="flex flex-col gap-1">
        <span className="text-xs font-medium text-text-muted">URL</span>
        <Input
          value={form.url}
          onChange={(e) => update('url', e.target.value)}
          onKeyDown={(e) => { if (e.key === 'Escape') onCancel() }}
          className="bg-surface-hover"
        />
      </label>
      <label className="flex flex-col gap-1">
        <span className="text-xs font-medium text-text-muted">Tags</span>
        <Input
          value={form.tags}
          onChange={(e) => update('tags', e.target.value)}
          onKeyDown={(e) => { if (e.key === 'Escape') onCancel() }}
          placeholder="comma-separated"
          className="bg-surface-hover"
        />
      </label>
      <label className="flex flex-col gap-1">
        <span className="text-xs font-medium text-text-muted">Description</span>
        <textarea
          value={form.description}
          onChange={(e) => update('description', e.target.value)}
          onKeyDown={(e) => { if (e.key === 'Escape') onCancel() }}
          rows={4}
          className="rounded-md border border-border bg-surface-hover px-3 py-2 text-sm text-text outline-none focus:ring-1 focus:ring-ring"
        />
      </label>
      <div className="flex items-center justify-end gap-2">
        <Button variant="ghost" size="sm" onClick={onCancel} disabled={saving}>
          <X className="mr-1 h-3.5 w-3.5" />
          Cancel
        </Button>
        <Button size="sm" onClick={onSave} disabled={saving}>
          <Check className="mr-1 h-3.5 w-3.5" />
          {saving ? 'Saving...' : 'Save'}
        </Button>
      </div>
    </div>
  )
}
