import { useState, useCallback, useEffect, useMemo, useRef } from 'react'
import {
  Dialog,
  DialogContent,
  DialogTitle,
} from '@/components/ui/dialog'
import { Button } from '@/components/ui/button'
import { useStore } from '@/lib/store'
import { useHiddenTags } from '@/hooks/useHiddenTags'
import { useDisplayBookmarks } from '@/hooks/useDisplayBookmarks'
import { updateBookmark, deleteBookmark, refreshMetadata, toBase64, fileUrl } from '@/lib/api'
import type { Bookmark, MetadataReport } from '@/lib/api'
import { Thumbnail, Favicon, Tags, UrlDisplay, DeleteButton, ImageDropZone, FetchingIndicator } from './bookmark-parts'
import { TagTokenInput } from '@/components/TagTokenInput'
import {
  ChevronLeft,
  ChevronRight,
  Pencil,
  RefreshCw,
  X,
  Check,
  ExternalLink,
} from 'lucide-react'

export default function BookmarkDetailModal() {
  const detailModalId = useStore((s) => s.detailModalId)
  const detailModalEdit = useStore((s) => s.detailModalEdit)
  const setDetailModalId = useStore((s) => s.setDetailModalId)
  const bookmarks = useStore((s) => s.bookmarks)
  const markDirty = useStore((s) => s.markDirty)
  const clearDirty = useStore((s) => s.clearDirty)
  const setBookmarks = useStore((s) => s.setBookmarks)
  const triggerRefetch = useStore((s) => s.triggerRefetch)
  const allTags = useStore((s) => s.tags)
  const pendingFetchReport = useStore((s) => s.pendingFetchReport)
  const setPendingFetchReport = useStore((s) => s.setPendingFetchReport)
  const hiddenTags = useHiddenTags()
  const visibleTags = useMemo(() => {
    const hidden = new Set(hiddenTags)
    return allTags.filter((t) => !hidden.has(t))
  }, [allTags, hiddenTags])

  const { displayBookmarks } = useDisplayBookmarks()

  const [editing, setEditing] = useState(false)
  const [editForm, setEditForm] = useState<EditFormState>({ title: '', description: '', url: '', tags: [] })
  const [saving, setSaving] = useState(false)
  const [refreshing, setRefreshing] = useState(false)
  const [error, setError] = useState<string | null>(null)
  // Pending image files (uploaded on save, not immediately)
  const [pendingCover, setPendingCover] = useState<File | null>(null)
  const [pendingIcon, setPendingIcon] = useState<File | null>(null)
  // Local preview URLs for display
  const [coverPreview, setCoverPreview] = useState<string | null>(null)
  const [iconPreview, setIconPreview] = useState<string | null>(null)
  // Per-bookmark report cache — survives navigation within the modal session
  const reportCache = useRef<Map<number, MetadataReport>>(new Map())
  const [fetchReport, setFetchReport] = useState<MetadataReport | null>(null)

  const setReportForBookmark = useCallback((id: number, report: MetadataReport | null) => {
    if (report) {
      reportCache.current.set(id, report)
    }
    // Only update visible state if we're still viewing that bookmark
    if (useStore.getState().detailModalId === id) {
      setFetchReport(report)
    }
  }, [])

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

  // Stage image for upload (actual upload happens on save)
  const handleCoverUpload = useCallback((file: File) => {
    setPendingCover(file)
    setCoverPreview(URL.createObjectURL(file))
  }, [])

  const handleIconUpload = useCallback((file: File) => {
    setPendingIcon(file)
    setIconPreview(URL.createObjectURL(file))
  }, [])

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

  // Exit edit mode when bookmark starts fetching
  useEffect(() => {
    if (bookmark?.fetching && editing) {
      setEditing(false)
      setError(null)
    }
  }, [bookmark?.fetching, editing])

  // Reset state when modal opens/closes or bookmark changes
  useEffect(() => {
    if (detailModalEdit && bookmark) {
      setEditForm({
        title: bookmark.title,
        description: bookmark.description,
        url: bookmark.url,
        tags: bookmark.tags.filter((t) => !hiddenTags.includes(t)),
      })
      setEditing(true)
    } else {
      setEditing(false)
    }
    setError(null)
    setRefreshing(false)
    setPendingCover(null)
    setPendingIcon(null)
    setCoverPreview(null)
    setIconPreview(null)
    // Consume pending report from create path, restore from cache, or clear
    if (pendingFetchReport && detailModalId !== null) {
      reportCache.current.set(detailModalId, pendingFetchReport)
      setFetchReport(pendingFetchReport)
      setPendingFetchReport(null)
    } else if (detailModalId !== null && reportCache.current.has(detailModalId)) {
      setFetchReport(reportCache.current.get(detailModalId)!)
    } else {
      setFetchReport(null)
    }
    // Clear cache when modal closes
    if (detailModalId === null) {
      reportCache.current.clear()
    }
  }, [detailModalId])

  const startEdit = useCallback(() => {
    if (!bookmark) return
    setEditForm({
      title: bookmark.title,
      description: bookmark.description,
      url: bookmark.url,
      tags: bookmark.tags.filter((t) => !hiddenTags.includes(t)),
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
      // Build update payload with form data + any pending images
      const payload: Parameters<typeof updateBookmark>[0] = {
        id: bookmark.id,
        title: editForm.title,
        description: editForm.description,
        url: editForm.url,
        tags: editForm.tags.join(','),
      }
      // Convert pending images to base64 if present
      if (pendingCover) payload.image_b64 = await toBase64(pendingCover)
      if (pendingIcon) payload.icon_b64 = await toBase64(pendingIcon)

      const updated = await updateBookmark(payload)
      // Update in local bookmarks array
      setBookmarks(bookmarks.map((b) => (b.id === updated.id ? updated : b)))
      setEditing(false)
    } catch (e) {
      setError(e instanceof Error ? e.message : 'Failed to save')
    } finally {
      clearDirty(bookmark.id)
      setSaving(false)
    }
  }, [bookmark, editForm, pendingCover, pendingIcon, bookmarks, markDirty, clearDirty, setBookmarks])

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

  const setFetchingOptimistic = useCallback((id: number, fetching: boolean) => {
    const state = useStore.getState()
    state.setBookmarks(state.bookmarks.map((b) => (b.id === id ? { ...b, fetching } : b)))
  }, [])

  const handleRefreshMetadata = useCallback(async () => {
    if (!bookmark) return
    const targetId = bookmark.id
    setRefreshing(true)
    setFetchingOptimistic(targetId, true)
    markDirty(targetId)
    setError(null)
    try {
      const { report } = await refreshMetadata(targetId, { async_meta: false })
      setReportForBookmark(targetId, report)
      clearDirty(targetId)
      triggerRefetch()
    } catch (e) {
      clearDirty(targetId)
      if (useStore.getState().detailModalId === targetId) {
        setError(e instanceof Error ? e.message : 'Failed to refresh metadata')
      }
    } finally {
      setFetchingOptimistic(targetId, false)
      if (useStore.getState().detailModalId === targetId) {
        setRefreshing(false)
      }
    }
  }, [bookmark, triggerRefetch, setReportForBookmark, setFetchingOptimistic, markDirty, clearDirty])

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
        className="flex h-[min(78vh,780px)] w-full max-w-3xl flex-col gap-0 overflow-hidden bg-surface p-0"
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
                <div className="relative">
                  <ImageDropZone onUpload={handleCoverUpload} label="Upload cover image" className="h-48 w-full sm:h-64">
                    {coverPreview ? (
                      <img src={coverPreview} alt="" className="h-48 w-full object-cover sm:h-64" />
                    ) : (
                      <Thumbnail bookmark={bookmark} className="h-48 w-full sm:h-64" />
                    )}
                  </ImageDropZone>
                  {pendingCover && (
                    <div className="absolute bottom-2 right-2 rounded bg-amber-600/90 px-2 py-1 text-xs font-medium text-white shadow">
                      New image
                    </div>
                  )}
                </div>
              ) : (
                <div className="relative">
                  <Thumbnail bookmark={bookmark} className="h-48 w-full sm:h-64" />
                  {bookmark.fetching && (
                    <div className="absolute inset-0 flex items-center justify-center bg-surface">
                      <FetchingIndicator />
                    </div>
                  )}
                </div>
              )}

              {/* Content */}
              <div className="flex flex-col gap-4 px-4 pt-4 pb-0 sm:px-6 sm:pt-6">
                {error && (
                  <div className="rounded-md bg-danger/10 px-3 py-2 text-xs text-danger">
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
                    availableTags={visibleTags}
                    iconPending={!!pendingIcon}
                  />
                ) : (
                  <ViewContent
                    bookmark={bookmark}
                    hiddenTags={hiddenTags}
                  />
                )}
              </div>

              {fetchReport && (
                <details className="mx-4 mt-4 rounded-md border border-white/[0.06] p-3 text-xs sm:mx-6">
                  <summary className="cursor-pointer font-medium text-text-muted">
                    Fetch Report ({fetchReport.duration_ms}ms, {fetchReport.fetchers.length} fetchers)
                  </summary>
                  <div className="mt-2 space-y-3">
                    {/* Per-fetcher results */}
                    <div>
                      <h4 className="mb-1 font-medium">Fetchers</h4>
                      <table className="w-full text-left">
                        <thead>
                          <tr className="border-b border-white/[0.06]">
                            <th className="py-1 pr-2">Name</th>
                            <th className="py-1 pr-2">Status</th>
                            <th className="py-1 pr-2">Duration</th>
                            <th className="py-1">Fields</th>
                          </tr>
                        </thead>
                        <tbody>
                          {fetchReport.fetchers.map((f, i) => {
                            const fieldEntries = f.fields
                              ? Object.entries(f.fields).filter(([, v]) => v !== null && v !== false)
                              : []
                            return (
                              <FetcherRow key={i} fetcher={f} fieldEntries={fieldEntries} />
                            )
                          })}
                        </tbody>
                      </table>
                    </div>

                    {/* Field decisions */}
                    {fetchReport.field_decisions.length > 0 && (
                      <div>
                        <h4 className="mb-1 font-medium">Field Decisions</h4>
                        <table className="w-full text-left">
                          <thead>
                            <tr className="border-b border-white/[0.06]">
                              <th className="py-1 pr-2">Field</th>
                              <th className="py-1 pr-2">Winner</th>
                              <th className="py-1 pr-2">Reason</th>
                              <th className="py-1">Preview</th>
                            </tr>
                          </thead>
                          <tbody>
                            {fetchReport.field_decisions.map((d, i) => (
                              <tr key={i} className="border-b border-white/[0.03]">
                                <td className="py-1 pr-2">{d.field}</td>
                                <td className="py-1 pr-2 font-medium">{d.winner}</td>
                                <td className="py-1 pr-2 text-text-muted">{d.reason}</td>
                                <td className="max-w-[200px] truncate py-1" title={d.value_preview ?? undefined}>
                                  {d.value_preview ?? '\u2014'}
                                </td>
                              </tr>
                            ))}
                          </tbody>
                        </table>
                      </div>
                    )}

                    {/* Headless fallback */}
                    {fetchReport.headless_fallback && (
                      <div>
                        <h4 className="mb-1 font-medium">Headless Fallback</h4>
                        <p>
                          {fetchReport.headless_fallback.triggered ? 'Triggered' : 'Not triggered'}
                          {' \u2014 '}{fetchReport.headless_fallback.reason}
                          {' \u2014 Status: '}
                          <span className={
                            fetchReport.headless_fallback.status.status === 'Success' ? 'text-green-600' :
                            fetchReport.headless_fallback.status.status === 'Skip' ? 'text-yellow-600' :
                            'text-red-600'
                          }>
                            {fetchReport.headless_fallback.status.status}
                          </span>
                        </p>
                        {fetchReport.headless_fallback.fields_overridden.length > 0 && (
                          <p className="text-text-muted">
                            Overrode: {fetchReport.headless_fallback.fields_overridden.join(', ')}
                          </p>
                        )}
                      </div>
                    )}
                  </div>
                </details>
              )}
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
                      disabled={refreshing || bookmark.fetching}
                      className="rounded p-1.5 text-text-muted transition-colors hover:bg-surface-hover hover:text-text disabled:opacity-30 disabled:hover:bg-transparent"
                      title="Refresh metadata"
                    >
                      <RefreshCw className={`h-4 w-4 ${refreshing || bookmark.fetching ? 'animate-spin' : ''}`} />
                    </button>
                    <button
                      tabIndex={-1}
                      onClick={startEdit}
                      disabled={bookmark.fetching}
                      className="rounded p-1.5 text-text-muted transition-colors hover:bg-surface-hover hover:text-text disabled:opacity-30 disabled:hover:bg-transparent"
                      title={bookmark.fetching ? 'Cannot edit while fetching' : 'Edit'}
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
  tags: string[]
}

function EditForm({
  form,
  onChange,
  bookmark,
  iconPreview,
  onIconUpload,
  availableTags,
  iconPending,
}: {
  form: EditFormState
  onChange: (form: EditFormState) => void
  bookmark: Bookmark
  iconPreview: string | null
  onIconUpload: (file: File) => void
  availableTags: string[]
  iconPending: boolean
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
        <span className={`text-xs ${iconPending ? 'text-amber-400' : 'text-text-dim'}`}>
          {iconPending ? 'New icon' : 'Click or drag to change icon'}
        </span>
      </div>

      <label className="flex flex-col gap-1">
        <span className="text-xs font-medium text-text-muted">Title</span>
        <input
          type="text"
          value={form.title}
          onChange={(e) => update('title', e.target.value)}
          className="h-8 w-full rounded-md border border-white/[0.06] bg-surface px-2.5 text-xs text-text outline-none transition-colors placeholder:text-text-dim focus:border-hi-dim"
        />
      </label>
      <label className="flex flex-col gap-1">
        <span className="text-xs font-medium text-text-muted">URL</span>
        <input
          type="text"
          value={form.url}
          onChange={(e) => update('url', e.target.value)}
          className="h-8 w-full rounded-md border border-white/[0.06] bg-surface px-2.5 text-xs text-text outline-none transition-colors placeholder:text-text-dim focus:border-hi-dim"
        />
      </label>
      <div className="flex flex-col gap-1">
        <span className="text-xs font-medium text-text-muted">Tags</span>
        <TagTokenInput
          tags={form.tags}
          onChange={(tags) => onChange({ ...form, tags })}
          availableTags={availableTags}
          placeholder="Add tag"
        />
      </div>
      <label className="flex flex-col gap-1">
        <span className="text-xs font-medium text-text-muted">Description</span>
        <textarea
          value={form.description}
          onChange={(e) => update('description', e.target.value)}
          rows={4}
          className="resize-none rounded-md border border-white/[0.06] bg-surface px-2.5 py-2 text-xs text-text outline-none transition-colors placeholder:text-text-dim focus:border-hi-dim"
        />
      </label>
    </div>
  )
}

// Fetcher table row with expandable field values in a full-width row below
function FetcherRow({
  fetcher: f,
  fieldEntries,
}: {
  fetcher: import('@/lib/api').FetcherReport
  fieldEntries: [string, unknown][]
}) {
  const [open, setOpen] = useState(false)
  const expandable = fieldEntries.length > 0

  return (
    <>
      <tr
        className={`border-b border-white/[0.03] ${expandable ? 'cursor-pointer hover:bg-white/[0.02]' : ''}`}
        onClick={expandable ? () => setOpen((o) => !o) : undefined}
      >
        <td className="py-1 pr-2">{f.name}</td>
        <td className="py-1 pr-2">
          <span className={
            f.status.status === 'Success' ? 'text-green-600' :
            f.status.status === 'Skip' ? 'text-yellow-600' :
            'text-red-600'
          }>
            {f.status.status}
            {(f.status.status === 'Error' || f.status.status === 'Skip') && f.status.detail && `: ${f.status.detail}`}
          </span>
        </td>
        <td className="py-1 pr-2">{f.duration_ms}ms</td>
        <td className="py-1">
          {expandable
            ? <>{open ? '\u25BC' : '\u25B6'} {fieldEntries.map(([k]) => k).join(', ')}</>
            : '\u2014'}
        </td>
      </tr>
      {open && (
        <tr className="border-b border-white/[0.03] bg-white/[0.01]">
          <td colSpan={4} className="py-1.5 pl-4">
            <dl className="space-y-0.5 text-text-muted">
              {fieldEntries.map(([k, v]) => (
                <div key={k} className="flex gap-1.5">
                  <dt className="shrink-0 font-medium">{k}:</dt>
                  <dd className="break-all">
                    {String(v)}
                  </dd>
                </div>
              ))}
            </dl>
          </td>
        </tr>
      )}
    </>
  )
}

// Icon with proper fallback for edit mode — matches Favicon's plain square pattern
function EditableIcon({ iconId, previewUrl }: { iconId: string | null; previewUrl: string | null }) {
  // Track which iconId/previewUrl combo failed, so changing either resets error
  const [failedKey, setFailedKey] = useState<string | null>(null)
  const currentKey = `${iconId}:${previewUrl}`
  const failed = failedKey === currentKey
  const src = previewUrl ?? (iconId && !failed ? fileUrl(iconId) : null)

  return (
    <div className="relative h-10 w-10 overflow-hidden rounded-md">
      <div className={`absolute inset-0 bg-surface-hover transition-opacity duration-100 ${src ? 'opacity-0' : 'opacity-100'}`} />
      {src && (
        <img
          src={src}
          alt=""
          onError={() => setFailedKey(currentKey)}
          className="absolute inset-0 h-full w-full object-contain"
        />
      )}
    </div>
  )
}
