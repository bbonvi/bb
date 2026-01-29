import { useState, useCallback, useEffect } from 'react'
import {
  Dialog,
  DialogContent,
  DialogTitle,
} from '@/components/ui/dialog'
import { Button } from '@/components/ui/button'
import { Input } from '@/components/ui/input'
import { useStore } from '@/lib/store'
import { createBookmark, normalizeTags } from '@/lib/api'
import { Check, X } from 'lucide-react'

interface CreateForm {
  url: string
  title: string
  description: string
  tags: string
  no_meta: boolean
  async_meta: boolean
  no_headless: boolean
}

const emptyForm: CreateForm = {
  url: '',
  title: '',
  description: '',
  tags: '',
  no_meta: false,
  async_meta: true,
  no_headless: false,
}

function isUrl(text: string): boolean {
  try {
    const url = new URL(text)
    return url.protocol === 'http:' || url.protocol === 'https:'
  } catch {
    return false
  }
}

export default function CreateBookmarkModal() {
  const open = useStore((s) => s.createModalOpen)
  const setOpen = useStore((s) => s.setCreateModalOpen)
  const initialUrl = useStore((s) => s.createModalInitialUrl)
  const initialTitle = useStore((s) => s.createModalInitialTitle)
  const initialDescription = useStore((s) => s.createModalInitialDescription)
  const initialTags = useStore((s) => s.createModalInitialTags)
  const openCreateModal = useStore((s) => s.openCreateModal)
  const bookmarks = useStore((s) => s.bookmarks)
  const setBookmarks = useStore((s) => s.setBookmarks)

  const [form, setForm] = useState<CreateForm>(emptyForm)
  const [submitting, setSubmitting] = useState(false)
  const [error, setError] = useState<string | null>(null)

  // Reset form when modal opens, pre-fill fields if provided
  useEffect(() => {
    if (open) {
      setForm({
        ...emptyForm,
        url: initialUrl || '',
        title: initialTitle || '',
        description: initialDescription || '',
        tags: initialTags || '',
      })
      setError(null)
    }
  }, [open, initialUrl, initialTitle, initialDescription, initialTags])

  // Ctrl+N global shortcut
  useEffect(() => {
    const handler = (e: KeyboardEvent) => {
      if ((e.ctrlKey || e.metaKey) && e.key === 'n') {
        e.preventDefault()
        setOpen(true)
      }
    }
    window.addEventListener('keydown', handler)
    return () => window.removeEventListener('keydown', handler)
  }, [setOpen])

  // Paste URL to auto-open create modal
  useEffect(() => {
    const handler = (e: ClipboardEvent) => {
      // Skip if an input/textarea is focused (user is pasting into a field)
      const active = document.activeElement
      if (
        active instanceof HTMLInputElement ||
        active instanceof HTMLTextAreaElement ||
        (active instanceof HTMLElement && active.isContentEditable)
      ) {
        return
      }
      // Skip if modal already open
      if (useStore.getState().createModalOpen) return

      const text = e.clipboardData?.getData('text/plain')?.trim()
      if (text && isUrl(text)) {
        e.preventDefault()
        openCreateModal({ url: text })
      }
    }
    window.addEventListener('paste', handler)
    return () => window.removeEventListener('paste', handler)
  }, [openCreateModal])

  const update = useCallback(
    <K extends keyof CreateForm>(field: K, value: CreateForm[K]) =>
      setForm((prev) => ({ ...prev, [field]: value })),
    [],
  )

  const handleSubmit = useCallback(async () => {
    const url = form.url.trim()
    if (!url) return

    setSubmitting(true)
    setError(null)

    // Optimistic insert with temporary negative ID
    const tempId = -(Date.now())
    const placeholder = {
      id: tempId,
      url,
      title: form.title.trim() || url,
      description: form.description.trim(),
      tags: form.tags
        .split(',')
        .map((t) => t.trim())
        .filter(Boolean),
      image_id: null,
      icon_id: null,
    }
    setBookmarks([placeholder, ...bookmarks])
    setOpen(false)

    try {
      const created = await createBookmark({
        url,
        title: form.title.trim() || undefined,
        description: form.description.trim() || undefined,
        tags: normalizeTags(form.tags) || undefined,
        no_meta: form.no_meta || undefined,
        async_meta: form.async_meta || undefined,
        no_headless: form.no_headless || undefined,
      })
      // Replace placeholder with real bookmark
      setBookmarks(
        useStore
          .getState()
          .bookmarks.map((b) => (b.id === tempId ? created : b)),
      )
    } catch (e) {
      // Remove placeholder on failure, reopen modal with error
      setBookmarks(
        useStore.getState().bookmarks.filter((b) => b.id !== tempId),
      )
      setError(e instanceof Error ? e.message : 'Failed to create bookmark')
      setOpen(true)
    } finally {
      setSubmitting(false)
    }
  }, [form, bookmarks, setBookmarks, setOpen])

  const handleKeyDown = useCallback(
    (e: React.KeyboardEvent) => {
      if (e.key === 'Enter' && !e.shiftKey) {
        e.preventDefault()
        handleSubmit()
      }
    },
    [handleSubmit],
  )

  return (
    <Dialog open={open} onOpenChange={setOpen}>
      <DialogContent
        className="flex w-full max-w-lg flex-col gap-0 overflow-hidden bg-surface p-0"
        showCloseButton={false}
      >
        {/* Header */}
        <div className="flex items-center justify-between border-b border-white/[0.06] px-4 py-3">
          <DialogTitle className="text-sm font-medium text-text">
            New Bookmark
          </DialogTitle>
          <button
            tabIndex={-1}
            onClick={() => setOpen(false)}
            className="rounded p-1 text-text-muted transition-colors hover:bg-surface-hover hover:text-text"
          >
            <X className="h-4 w-4" />
          </button>
        </div>

        {/* Body */}
        <div className="flex flex-col gap-3 p-4 sm:p-6">
          {error && (
            <div className="rounded-md bg-danger/10 px-3 py-2 text-sm text-danger">
              {error}
            </div>
          )}

          <label className="flex flex-col gap-1">
            <span className="text-xs font-medium text-text-muted">
              URL <span className="text-danger">*</span>
            </span>
            <Input
              value={form.url}
              onChange={(e) => update('url', e.target.value)}
              onKeyDown={handleKeyDown}
              placeholder="https://example.com"
              autoFocus
              className="bg-surface-hover"
            />
          </label>

          <label className="flex flex-col gap-1">
            <span className="text-xs font-medium text-text-muted">Title</span>
            <Input
              value={form.title}
              onChange={(e) => update('title', e.target.value)}
              onKeyDown={handleKeyDown}
              placeholder="Optional — fetched from page if empty"
              className="bg-surface-hover"
            />
          </label>

          <label className="flex flex-col gap-1">
            <span className="text-xs font-medium text-text-muted">Tags</span>
            <Input
              value={form.tags}
              onChange={(e) => update('tags', e.target.value)}
              onKeyDown={handleKeyDown}
              placeholder="comma-separated"
              className="bg-surface-hover"
            />
          </label>

          <label className="flex flex-col gap-1">
            <span className="text-xs font-medium text-text-muted">
              Description
            </span>
            <textarea
              value={form.description}
              onChange={(e) => update('description', e.target.value)}
              onKeyDown={(e) => {
                if (e.key === 'Escape') setOpen(false)
              }}
              rows={3}
              placeholder="Optional"
              className="rounded-md border border-border bg-surface-hover px-3 py-2 text-sm text-text outline-none placeholder:text-text-dim focus:ring-1 focus:ring-ring"
            />
          </label>

          {/* Metadata options */}
          <div className="flex flex-wrap items-center gap-x-4 gap-y-1 pt-1">
            <span className="text-[11px] font-medium uppercase tracking-wider text-text-dim">
              Fetch
            </span>
            <MetaToggle
              checked={!form.no_meta}
              onChange={(v) => update('no_meta', !v)}
              label="Metadata"
            />
            <MetaToggle
              checked={form.async_meta}
              onChange={(v) => update('async_meta', v)}
              label="Async"
            />
            <MetaToggle
              checked={!form.no_headless}
              onChange={(v) => update('no_headless', !v)}
              label="Headless"
            />
          </div>
        </div>

        {/* Footer */}
        <div className="flex items-center justify-end gap-2 border-t border-white/[0.06] px-4 py-3 sm:px-6">
          <Button
            variant="ghost"
            size="sm"
            onClick={() => setOpen(false)}
            disabled={submitting}
          >
            Cancel
          </Button>
          <Button
            size="sm"
            onClick={handleSubmit}
            disabled={submitting || !form.url.trim()}
          >
            <Check className="mr-1 h-3.5 w-3.5" />
            {submitting ? 'Creating...' : 'Create'}
          </Button>
        </div>
      </DialogContent>
    </Dialog>
  )
}

function MetaToggle({
  checked,
  onChange,
  label,
}: {
  checked: boolean
  onChange: (v: boolean) => void
  label: string
}) {
  return (
    <label className="flex cursor-pointer items-center gap-1.5 select-none">
      <input
        type="checkbox"
        checked={checked}
        onChange={(e) => onChange(e.target.checked)}
        className="h-3.5 w-3.5 rounded border-white/20 bg-surface-hover accent-hi"
      />
      <span className="text-xs text-text-muted">{label}</span>
    </label>
  )
}
