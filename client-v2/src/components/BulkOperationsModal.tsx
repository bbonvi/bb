import { useState, useCallback } from 'react'
import {
  Dialog,
  DialogContent,
  DialogTitle,
} from '@/components/ui/dialog'
import { Button } from '@/components/ui/button'
import { Input } from '@/components/ui/input'
import { useStore } from '@/lib/store'
import {
  searchUpdateBookmarks,
  searchDeleteBookmarks,
  type SearchQuery,
  type BulkSearchQuery,
} from '@/lib/api'
import { X, Pencil, Trash2, AlertTriangle } from 'lucide-react'

// Convert store SearchQuery (comma-separated tags) to BulkSearchQuery (JSON array tags)
function toBulkQuery(q: SearchQuery): BulkSearchQuery {
  const bulk: BulkSearchQuery = {}
  if (q.id) bulk.id = q.id
  if (q.url) bulk.url = q.url
  if (q.title) bulk.title = q.title
  if (q.description) bulk.description = q.description
  if (q.tags) bulk.tags = q.tags.split(',').map((t) => t.trim()).filter(Boolean)
  if (q.keyword) bulk.keyword = q.keyword
  if (q.semantic) bulk.semantic = q.semantic
  if (q.threshold != null) bulk.threshold = q.threshold
  if (q.exact) bulk.exact = q.exact
  if (q.limit != null) bulk.limit = q.limit
  return bulk
}

// ─── Bookmark preview list (shared between edit/delete) ───────────
function BookmarkPreview({ count }: { count: number }) {
  const bookmarks = useStore((s) => s.bookmarks)
  const maxPreview = 50

  return (
    <div className="flex flex-col gap-2">
      <div className="flex items-center gap-2 text-sm">
        <AlertTriangle className="h-4 w-4 text-amber-400 shrink-0" />
        <span className="text-text">
          This will affect <strong className="text-amber-400 font-mono">{count}</strong> bookmark{count !== 1 ? 's' : ''} matching the current search
        </span>
      </div>
      <div className="max-h-40 overflow-y-auto rounded-md border border-white/[0.06] bg-bg p-2">
        {[...bookmarks].reverse().slice(0, maxPreview).map((b) => (
          <div key={b.id} className="flex items-center gap-2 py-1 text-xs">
            <span className="truncate text-text-muted" title={b.url}>
              {b.title || b.url}
            </span>
          </div>
        ))}
        {bookmarks.length > maxPreview && (
          <div className="py-1 text-xs text-text-dim">
            …and {bookmarks.length - maxPreview} more
          </div>
        )}
      </div>
    </div>
  )
}

// ─── Bulk Edit Modal ──────────────────────────────────────────────
type TagMode = 'append' | 'remove' | 'replace'

export function BulkEditModal({
  open,
  onOpenChange,
}: {
  open: boolean
  onOpenChange: (open: boolean) => void
}) {
  const bookmarks = useStore((s) => s.bookmarks)
  const searchQuery = useStore((s) => s.searchQuery)

  const [tagMode, setTagMode] = useState<TagMode>('append')
  const [tagsInput, setTagsInput] = useState('')
  const [submitting, setSubmitting] = useState(false)
  const [error, setError] = useState<string | null>(null)
  const [result, setResult] = useState<number | null>(null)

  const count = bookmarks.length

  const reset = useCallback(() => {
    setTagMode('append')
    setTagsInput('')
    setError(null)
    setResult(null)
  }, [])

  const handleSubmit = useCallback(async () => {
    const tags = tagsInput.split(/[\s,]+/).map((t) => t.trim()).filter(Boolean)
    if (tags.length === 0) return

    setSubmitting(true)
    setError(null)

    try {
      const query = toBulkQuery(searchQuery)
      const update =
        tagMode === 'append'
          ? { append_tags: tags }
          : tagMode === 'remove'
            ? { remove_tags: tags }
            : { tags }

      const affected = await searchUpdateBookmarks(query, update)
      setResult(affected)
    } catch (e) {
      setError(e instanceof Error ? e.message : 'Bulk update failed')
    } finally {
      setSubmitting(false)
    }
  }, [tagsInput, tagMode, searchQuery])

  const close = useCallback(() => {
    onOpenChange(false)
    // Reset after animation
    setTimeout(reset, 200)
  }, [onOpenChange, reset])

  return (
    <Dialog open={open} onOpenChange={(v) => { if (!v) close() }}>
      <DialogContent
        className="flex w-full max-w-lg flex-col gap-0 overflow-hidden bg-surface p-0"
        showCloseButton={false}
      >
        {/* Header */}
        <div className="flex items-center justify-between border-b border-white/[0.06] px-4 py-3">
          <DialogTitle className="flex items-center gap-2 text-sm font-medium text-text">
            <Pencil className="h-4 w-4 text-hi" />
            Bulk Edit
          </DialogTitle>
          <button
            tabIndex={-1}
            onClick={close}
            className="rounded p-1 text-text-muted transition-colors hover:bg-surface-hover hover:text-text"
          >
            <X className="h-4 w-4" />
          </button>
        </div>

        {/* Body */}
        <div className="flex flex-col gap-4 p-4 sm:p-6">
          {result !== null ? (
            <div className="rounded-md bg-green-500/10 px-3 py-2 text-sm text-green-400">
              Updated <strong className="font-mono">{result}</strong> bookmark{result !== 1 ? 's' : ''}.
            </div>
          ) : (
            <>
              {error && (
                <div className="rounded-md bg-danger/10 px-3 py-2 text-sm text-danger">
                  {error}
                </div>
              )}

              <BookmarkPreview count={count} />

              {/* Tag operation */}
              <div className="flex flex-col gap-2">
                <span className="text-xs font-medium text-text-muted">Tag operation</span>
                <div className="flex items-center rounded-lg bg-bg p-0.5">
                  {(['append', 'remove', 'replace'] as const).map((mode) => (
                    <button
                      key={mode}
                      tabIndex={-1}
                      onClick={() => setTagMode(mode)}
                      className={`flex-1 rounded-md px-3 py-1.5 text-xs font-medium transition-all ${
                        tagMode === mode
                          ? 'bg-hi-dim text-text'
                          : 'text-text-muted hover:text-text'
                      }`}
                    >
                      {mode === 'append' ? 'Add tags' : mode === 'remove' ? 'Remove tags' : 'Replace tags'}
                    </button>
                  ))}
                </div>
                <Input
                  value={tagsInput}
                  onChange={(e) => setTagsInput(e.target.value)}
                  placeholder="tag1, tag2, tag3"
                  autoFocus
                  className="bg-surface-hover"
                  onKeyDown={(e) => {
                    if (e.key === 'Enter' && !e.shiftKey) {
                      e.preventDefault()
                      handleSubmit()
                    }
                  }}
                />
                <span className="text-[11px] text-text-dim">
                  {tagMode === 'append' && 'These tags will be added to all matched bookmarks.'}
                  {tagMode === 'remove' && 'These tags will be removed from all matched bookmarks.'}
                  {tagMode === 'replace' && 'All existing tags will be replaced with these tags.'}
                </span>
              </div>
            </>
          )}
        </div>

        {/* Footer */}
        <div className="flex items-center justify-end gap-2 border-t border-white/[0.06] px-4 py-3 sm:px-6">
          <Button variant="ghost" size="sm" onClick={close} disabled={submitting}>
            {result !== null ? 'Close' : 'Cancel'}
          </Button>
          {result === null && (
            <Button
              size="sm"
              onClick={handleSubmit}
              disabled={submitting || !tagsInput.trim()}
            >
              <Pencil className="mr-1 h-3.5 w-3.5" />
              {submitting ? 'Updating...' : `Update ${count} bookmark${count !== 1 ? 's' : ''}`}
            </Button>
          )}
        </div>
      </DialogContent>
    </Dialog>
  )
}

// ─── Bulk Delete Modal ────────────────────────────────────────────
export function BulkDeleteModal({
  open,
  onOpenChange,
}: {
  open: boolean
  onOpenChange: (open: boolean) => void
}) {
  const bookmarks = useStore((s) => s.bookmarks)
  const searchQuery = useStore((s) => s.searchQuery)

  const [submitting, setSubmitting] = useState(false)
  const [error, setError] = useState<string | null>(null)
  const [result, setResult] = useState<number | null>(null)

  const count = bookmarks.length

  const reset = useCallback(() => {
    setError(null)
    setResult(null)
  }, [])

  const handleDelete = useCallback(async () => {
    setSubmitting(true)
    setError(null)

    try {
      const query = toBulkQuery(searchQuery)
      const deleted = await searchDeleteBookmarks(query)
      setResult(deleted)
    } catch (e) {
      setError(e instanceof Error ? e.message : 'Bulk delete failed')
    } finally {
      setSubmitting(false)
    }
  }, [searchQuery])

  const close = useCallback(() => {
    onOpenChange(false)
    setTimeout(reset, 200)
  }, [onOpenChange, reset])

  return (
    <Dialog open={open} onOpenChange={(v) => { if (!v) close() }}>
      <DialogContent
        className="flex w-full max-w-lg flex-col gap-0 overflow-hidden bg-surface p-0"
        showCloseButton={false}
      >
        {/* Header */}
        <div className="flex items-center justify-between border-b border-white/[0.06] px-4 py-3">
          <DialogTitle className="flex items-center gap-2 text-sm font-medium text-text">
            <Trash2 className="h-4 w-4 text-danger" />
            Bulk Delete
          </DialogTitle>
          <button
            tabIndex={-1}
            onClick={close}
            className="rounded p-1 text-text-muted transition-colors hover:bg-surface-hover hover:text-text"
          >
            <X className="h-4 w-4" />
          </button>
        </div>

        {/* Body */}
        <div className="flex flex-col gap-4 p-4 sm:p-6">
          {result !== null ? (
            <div className="rounded-md bg-green-500/10 px-3 py-2 text-sm text-green-400">
              Deleted <strong className="font-mono">{result}</strong> bookmark{result !== 1 ? 's' : ''}.
            </div>
          ) : (
            <>
              {error && (
                <div className="rounded-md bg-danger/10 px-3 py-2 text-sm text-danger">
                  {error}
                </div>
              )}

              <BookmarkPreview count={count} />

              <div className="rounded-md bg-danger/10 px-3 py-2 text-sm text-danger">
                This action cannot be undone. All matched bookmarks will be permanently deleted.
              </div>
            </>
          )}
        </div>

        {/* Footer */}
        <div className="flex items-center justify-end gap-2 border-t border-white/[0.06] px-4 py-3 sm:px-6">
          <Button variant="ghost" size="sm" onClick={close} disabled={submitting}>
            {result !== null ? 'Close' : 'Cancel'}
          </Button>
          {result === null && (
            <Button
              size="sm"
              variant="destructive"
              onClick={handleDelete}
              disabled={submitting || count === 0}
            >
              <Trash2 className="mr-1 h-3.5 w-3.5" />
              {submitting ? 'Deleting...' : `Delete ${count} bookmark${count !== 1 ? 's' : ''}`}
            </Button>
          )}
        </div>
      </DialogContent>
    </Dialog>
  )
}
