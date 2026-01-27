import { useState, useEffect, useMemo, useCallback } from 'react'
import { X, Plus, Trash2, RefreshCw, LogOut } from 'lucide-react'
import { useStore } from '@/lib/store'
import { useHiddenTags } from '@/hooks/useHiddenTags'
import {
  createWorkspace as apiCreateWorkspace,
  updateWorkspace as apiUpdateWorkspace,
  deleteWorkspace as apiDeleteWorkspace,
  fetchWorkspaces,
  searchBookmarks,
} from '@/lib/api'
import type { Workspace, Bookmark } from '@/lib/api'

// ─── Settings Panel (Modal) ──────────────────────────────────────

export function SettingsPanel() {
  const open = useStore((s) => s.settingsOpen)
  const setOpen = useStore((s) => s.setSettingsOpen)
  const workspaces = useStore((s) => s.workspaces)
  const setWorkspaces = useStore((s) => s.setWorkspaces)
  const workspacesAvailable = useStore((s) => s.workspacesAvailable)
  const token = useStore((s) => s.token)
  const setToken = useStore((s) => s.setToken)
  const tags = useStore((s) => s.tags)
  const hiddenTags = useHiddenTags()

  const [selectedId, setSelectedId] = useState<string | null>(null)
  const [saving, setSaving] = useState(false)
  const [error, setError] = useState<string | null>(null)

  // Select first workspace by default
  useEffect(() => {
    if (open && workspaces.length > 0 && !selectedId) {
      setSelectedId(workspaces[0].id)
    }
  }, [open, workspaces, selectedId])

  // Reset on close
  useEffect(() => {
    if (!open) {
      setSelectedId(null)
      setError(null)
    }
  }, [open])

  if (!open) return null

  const hiddenSet = new Set(hiddenTags)
  const visibleTags = tags.filter((t) => !hiddenSet.has(t))

  async function refreshWorkspaces() {
    try {
      const ws = await fetchWorkspaces()
      setWorkspaces(ws)
    } catch {
      // polling will catch up
    }
  }

  async function handleCreate() {
    setSaving(true)
    setError(null)
    try {
      const ws = await apiCreateWorkspace({ name: 'New workspace' })
      await refreshWorkspaces()
      setSelectedId(ws.id)
    } catch (err) {
      setError(err instanceof Error ? err.message : 'Failed to create workspace')
    } finally {
      setSaving(false)
    }
  }

  async function handleDelete(id: string) {
    setSaving(true)
    setError(null)
    try {
      await apiDeleteWorkspace(id)
      await refreshWorkspaces()
      if (selectedId === id) setSelectedId(null)
    } catch (err) {
      setError(err instanceof Error ? err.message : 'Failed to delete workspace')
    } finally {
      setSaving(false)
    }
  }

  function handleLogout() {
    setToken(null)
    window.location.reload()
  }

  const selected = workspaces.find((ws) => ws.id === selectedId)

  return (
    <div className="fixed inset-0 z-50 flex items-center justify-center bg-black/60 backdrop-blur-sm" onClick={() => setOpen(false)}>
      <div
        className="flex w-full max-w-3xl flex-col rounded-xl border border-white/[0.08] bg-bg shadow-2xl"
        style={{ height: 'min(80vh, 680px)' }}
        onClick={(e) => e.stopPropagation()}
      >
        {/* Header */}
        <div className="flex items-center justify-between border-b border-white/[0.06] px-5 py-3">
          <h2 className="text-sm font-semibold text-text">Settings</h2>
          <button
            onClick={() => setOpen(false)}
            className="flex h-7 w-7 items-center justify-center rounded-md text-text-muted transition-colors hover:bg-surface-hover hover:text-text"
          >
            <X className="h-4 w-4" />
          </button>
        </div>

        {/* Body */}
        <div className="flex min-h-0 flex-1">
          {/* Sidebar: workspace list */}
          {workspacesAvailable && (
            <div className="flex w-48 shrink-0 flex-col border-r border-white/[0.06]">
              <div className="flex items-center justify-between px-3 py-2">
                <span className="text-[11px] font-medium uppercase tracking-wider text-text-dim">Workspaces</span>
                <button
                  onClick={handleCreate}
                  disabled={saving}
                  className="flex h-6 w-6 items-center justify-center rounded-md text-text-muted transition-colors hover:bg-surface-hover hover:text-text disabled:opacity-50"
                  title="Create workspace"
                >
                  <Plus className="h-3.5 w-3.5" />
                </button>
              </div>
              <div className="flex-1 overflow-y-auto">
                {workspaces.map((ws) => (
                  <button
                    key={ws.id}
                    onClick={() => setSelectedId(ws.id)}
                    className={`flex w-full items-center justify-between px-3 py-1.5 text-left text-xs transition-colors ${
                      selectedId === ws.id
                        ? 'bg-hi-dim text-text'
                        : 'text-text-muted hover:bg-surface-hover hover:text-text'
                    }`}
                  >
                    <span className="truncate">{ws.name}</span>
                  </button>
                ))}
                {workspaces.length === 0 && (
                  <div className="px-3 py-4 text-xs text-text-dim">No workspaces yet</div>
                )}
              </div>
            </div>
          )}

          {/* Main: workspace editor */}
          <div className="flex min-w-0 flex-1 flex-col overflow-y-auto px-5 py-4">
            {error && (
              <div className="mb-3 rounded-md bg-danger/10 px-3 py-2 text-xs text-danger">{error}</div>
            )}

            {workspacesAvailable && selected ? (
              <WorkspaceEditor
                workspace={selected}
                visibleTags={visibleTags}
                onSave={async (updated) => {
                  setSaving(true)
                  setError(null)
                  try {
                    await apiUpdateWorkspace(updated.id, {
                      name: updated.name,
                      filters: updated.filters,
                      view_prefs: updated.view_prefs,
                    })
                    await refreshWorkspaces()
                  } catch (err) {
                    setError(err instanceof Error ? err.message : 'Failed to save workspace')
                  } finally {
                    setSaving(false)
                  }
                }}
                onDelete={() => handleDelete(selected.id)}
                saving={saving}
              />
            ) : workspacesAvailable ? (
              <div className="flex flex-1 items-center justify-center text-sm text-text-dim">
                Select or create a workspace
              </div>
            ) : null}

            {!workspacesAvailable && (
              <div className="text-sm text-text-dim">
                Workspace management is unavailable (backend does not support workspaces).
              </div>
            )}
          </div>
        </div>

        {/* Footer */}
        <div className="flex items-center justify-between border-t border-white/[0.06] px-5 py-3">
          {token && (
            <button
              onClick={handleLogout}
              className="flex items-center gap-1.5 rounded-md px-3 py-1.5 text-xs font-medium text-danger/70 transition-colors hover:bg-danger/10 hover:text-danger"
            >
              <LogOut className="h-3 w-3" />
              Logout
            </button>
          )}
          <div className="flex-1" />
        </div>
      </div>
    </div>
  )
}

// ─── Workspace Editor ────────────────────────────────────────────

function WorkspaceEditor({
  workspace,
  visibleTags,
  onSave,
  onDelete,
  saving,
}: {
  workspace: Workspace
  visibleTags: string[]
  onSave: (ws: Workspace) => Promise<void>
  onDelete: () => void
  saving: boolean
}) {
  const [name, setName] = useState(workspace.name)
  const [whitelistInput, setWhitelistInput] = useState('')
  const [blacklistInput, setBlacklistInput] = useState('')
  const [urlPattern, setUrlPattern] = useState(workspace.filters.url_pattern ?? '')
  const [titlePattern, setTitlePattern] = useState(workspace.filters.title_pattern ?? '')
  const [descPattern, setDescPattern] = useState(workspace.filters.description_pattern ?? '')
  const [anyFieldPattern, setAnyFieldPattern] = useState(workspace.filters.any_field_pattern ?? '')
  const [deleteConfirm, setDeleteConfirm] = useState(false)

  // Related tags
  const [relatedTags, setRelatedTags] = useState<string[]>([])
  const [fetchingRelated, setFetchingRelated] = useState(false)

  // Reset form when workspace changes
  useEffect(() => {
    setName(workspace.name)
    setUrlPattern(workspace.filters.url_pattern ?? '')
    setTitlePattern(workspace.filters.title_pattern ?? '')
    setDescPattern(workspace.filters.description_pattern ?? '')
    setAnyFieldPattern(workspace.filters.any_field_pattern ?? '')
    setWhitelistInput('')
    setBlacklistInput('')
    setDeleteConfirm(false)
    setRelatedTags([])
  }, [workspace.id]) // eslint-disable-line react-hooks/exhaustive-deps

  // Filtered autocomplete suggestions (exclude already-added tags)
  const whitelist = workspace.filters.tag_whitelist ?? []
  const blacklist = workspace.filters.tag_blacklist ?? []

  const existingTags = useMemo(
    () => new Set([...whitelist, ...blacklist]),
    [whitelist, blacklist],
  )

  const autocompleteTags = useMemo(
    () => visibleTags.filter((t) => !existingTags.has(t)),
    [visibleTags, existingTags],
  )

  const whitelistSuggestions = useMemo(() => {
    if (!whitelistInput) return []
    const lower = whitelistInput.toLowerCase()
    return autocompleteTags.filter((t) => t.toLowerCase().includes(lower)).slice(0, 8)
  }, [whitelistInput, autocompleteTags])

  const blacklistSuggestions = useMemo(() => {
    if (!blacklistInput) return []
    const lower = blacklistInput.toLowerCase()
    return autocompleteTags.filter((t) => t.toLowerCase().includes(lower)).slice(0, 8)
  }, [blacklistInput, autocompleteTags])

  function saveWorkspace(overrides: Partial<Workspace['filters']> = {}) {
    const filters = {
      tag_whitelist: whitelist,
      tag_blacklist: blacklist,
      url_pattern: urlPattern || null,
      title_pattern: titlePattern || null,
      description_pattern: descPattern || null,
      any_field_pattern: anyFieldPattern || null,
      ...overrides,
    }
    onSave({ ...workspace, name, filters })
  }

  function addWhitelistTag(tag: string) {
    const trimmed = tag.trim()
    if (!trimmed || whitelist.includes(trimmed)) return
    saveWorkspace({
      tag_whitelist: [...whitelist, trimmed],
    })
    setWhitelistInput('')
  }

  function removeWhitelistTag(tag: string) {
    saveWorkspace({
      tag_whitelist: whitelist.filter((t) => t !== tag),
    })
  }

  function addBlacklistTag(tag: string) {
    const trimmed = tag.trim()
    if (!trimmed || blacklist.includes(trimmed)) return
    saveWorkspace({
      tag_blacklist: [...blacklist, trimmed],
    })
    setBlacklistInput('')
  }

  function removeBlacklistTag(tag: string) {
    saveWorkspace({
      tag_blacklist: workspace.filters.tag_blacklist.filter((t) => t !== tag),
    })
  }

  async function fetchRelatedTags() {
    if (workspace.filters.tag_whitelist.length === 0) return
    setFetchingRelated(true)
    try {
      // Fetch all bookmarks matching the whitelist tags
      const tags = workspace.filters.tag_whitelist.filter((t) => !t.includes('*') && !t.includes('?'))
      if (tags.length === 0) {
        setRelatedTags([])
        return
      }
      const bookmarks: Bookmark[] = await searchBookmarks({ tags: tags.join(',') })
      // Collect all tags from matched bookmarks, excluding already-known tags
      const relatedSet = new Set<string>()
      for (const bm of bookmarks) {
        for (const t of bm.tags) {
          if (!existingTags.has(t)) relatedSet.add(t)
        }
      }
      const sorted = Array.from(relatedSet).sort((a, b) => a.localeCompare(b))
      setRelatedTags(sorted)
    } catch {
      // ignore
    } finally {
      setFetchingRelated(false)
    }
  }

  const handleNameBlur = useCallback(() => {
    if (name !== workspace.name && name.trim()) {
      saveWorkspace()
    }
  }, [name, workspace.name]) // eslint-disable-line react-hooks/exhaustive-deps

  const handlePatternBlur = useCallback(() => {
    saveWorkspace()
  }, [urlPattern, titlePattern, descPattern, anyFieldPattern]) // eslint-disable-line react-hooks/exhaustive-deps

  return (
    <div className="flex flex-col gap-4">
      {/* Name + delete */}
      <div className="flex items-center gap-2">
        <input
          type="text"
          value={name}
          onChange={(e) => setName(e.target.value)}
          onBlur={handleNameBlur}
          onKeyDown={(e) => {
            if (e.key === 'Enter') handleNameBlur()
          }}
          className="h-8 flex-1 rounded-md border border-white/[0.06] bg-surface px-2.5 text-sm text-text outline-none transition-colors focus:border-hi-dim"
          placeholder="Workspace name"
        />
        {deleteConfirm ? (
          <div className="flex items-center gap-1">
            <button
              onClick={onDelete}
              disabled={saving}
              className="rounded-md px-2 py-1 text-xs font-medium text-danger transition-colors hover:bg-danger/10 disabled:opacity-50"
            >
              Confirm
            </button>
            <button
              onClick={() => setDeleteConfirm(false)}
              className="rounded-md px-2 py-1 text-xs text-text-muted transition-colors hover:text-text"
            >
              Cancel
            </button>
          </div>
        ) : (
          <button
            onClick={() => setDeleteConfirm(true)}
            className="flex h-8 w-8 items-center justify-center rounded-md text-text-muted transition-colors hover:bg-danger/10 hover:text-danger"
            title="Delete workspace"
          >
            <Trash2 className="h-3.5 w-3.5" />
          </button>
        )}
      </div>

      {/* Tag whitelist */}
      <div>
        <label className="mb-1.5 block text-[11px] font-medium uppercase tracking-wider text-text-dim">
          Tag whitelist
        </label>
        <TagListEditor
          tags={workspace.filters.tag_whitelist}
          input={whitelistInput}
          setInput={setWhitelistInput}
          suggestions={whitelistSuggestions}
          onAdd={addWhitelistTag}
          onRemove={removeWhitelistTag}
          placeholder="Add tag pattern (supports glob: dev/*)"
        />
      </div>

      {/* Tag blacklist */}
      <div>
        <label className="mb-1.5 block text-[11px] font-medium uppercase tracking-wider text-text-dim">
          Tag blacklist
        </label>
        <TagListEditor
          tags={workspace.filters.tag_blacklist}
          input={blacklistInput}
          setInput={setBlacklistInput}
          suggestions={blacklistSuggestions}
          onAdd={addBlacklistTag}
          onRemove={removeBlacklistTag}
          placeholder="Add tag to exclude"
        />
      </div>

      {/* Regex pattern filters */}
      <div className="grid grid-cols-2 gap-3">
        <PatternInput label="URL pattern" value={urlPattern} onChange={setUrlPattern} onBlur={handlePatternBlur} placeholder="regex" />
        <PatternInput label="Title pattern" value={titlePattern} onChange={setTitlePattern} onBlur={handlePatternBlur} placeholder="regex" />
        <PatternInput label="Description pattern" value={descPattern} onChange={setDescPattern} onBlur={handlePatternBlur} placeholder="regex" />
        <PatternInput label="Any field" value={anyFieldPattern} onChange={setAnyFieldPattern} onBlur={handlePatternBlur} placeholder="regex" />
      </div>

      {/* Fetch related tags */}
      <div>
        <button
          onClick={fetchRelatedTags}
          disabled={fetchingRelated || workspace.filters.tag_whitelist.length === 0}
          className="flex items-center gap-1.5 rounded-md px-3 py-1.5 text-xs font-medium text-text-muted transition-colors hover:bg-surface-hover hover:text-text disabled:opacity-50"
        >
          <RefreshCw className={`h-3 w-3 ${fetchingRelated ? 'animate-spin' : ''}`} />
          Fetch related tags
        </button>
        {relatedTags.length > 0 && (
          <div className="mt-2 flex flex-wrap gap-1">
            {relatedTags.map((tag) => (
              <button
                key={tag}
                onClick={() => addWhitelistTag(tag)}
                className="rounded px-1.5 py-0.5 text-[11px] text-text-muted transition-colors hover:bg-surface-hover hover:text-text"
              >
                {tag}
              </button>
            ))}
          </div>
        )}
      </div>
    </div>
  )
}

// ─── Tag List Editor ─────────────────────────────────────────────

function TagListEditor({
  tags,
  input,
  setInput,
  suggestions,
  onAdd,
  onRemove,
  placeholder,
}: {
  tags: string[]
  input: string
  setInput: (v: string) => void
  suggestions: string[]
  onAdd: (tag: string) => void
  onRemove: (tag: string) => void
  placeholder: string
}) {
  const [showSuggestions, setShowSuggestions] = useState(false)

  return (
    <div>
      {/* Input + suggestions */}
      <div className="relative">
        <input
          type="text"
          value={input}
          onChange={(e) => setInput(e.target.value)}
          onFocus={() => setShowSuggestions(true)}
          onBlur={() => setTimeout(() => setShowSuggestions(false), 150)}
          onKeyDown={(e) => {
            if (e.key === 'Enter') {
              e.preventDefault()
              onAdd(input)
            }
          }}
          className="h-7 w-full rounded-md border border-white/[0.06] bg-surface px-2 text-xs text-text outline-none transition-colors focus:border-hi-dim"
          placeholder={placeholder}
        />
        {showSuggestions && suggestions.length > 0 && (
          <div className="absolute top-full left-0 z-10 mt-1 max-h-32 w-full overflow-y-auto rounded-md border border-white/[0.08] bg-surface shadow-lg">
            {suggestions.map((s) => (
              <button
                key={s}
                onMouseDown={(e) => {
                  e.preventDefault()
                  onAdd(s)
                }}
                className="block w-full px-2 py-1 text-left text-xs text-text-muted transition-colors hover:bg-surface-hover hover:text-text"
              >
                {s}
              </button>
            ))}
          </div>
        )}
      </div>

      {/* Tag chips */}
      {tags.length > 0 && (
        <div className="mt-1.5 flex flex-wrap gap-1">
          {tags.map((tag) => (
            <span
              key={tag}
              className="flex items-center gap-1 rounded-md bg-surface px-1.5 py-0.5 text-[11px] text-text-muted"
            >
              {tag}
              <button
                onClick={() => onRemove(tag)}
                className="text-text-dim transition-colors hover:text-danger"
              >
                <X className="h-2.5 w-2.5" />
              </button>
            </span>
          ))}
        </div>
      )}
    </div>
  )
}

// ─── Pattern Input ───────────────────────────────────────────────

function PatternInput({
  label,
  value,
  onChange,
  onBlur,
  placeholder,
}: {
  label: string
  value: string
  onChange: (v: string) => void
  onBlur: () => void
  placeholder: string
}) {
  return (
    <div>
      <label className="mb-1 block text-[11px] font-medium uppercase tracking-wider text-text-dim">
        {label}
      </label>
      <input
        type="text"
        value={value}
        onChange={(e) => onChange(e.target.value)}
        onBlur={onBlur}
        onKeyDown={(e) => {
          if (e.key === 'Enter') onBlur()
        }}
        className="h-7 w-full rounded-md border border-white/[0.06] bg-surface px-2 text-xs font-mono text-text outline-none transition-colors focus:border-hi-dim"
        placeholder={placeholder}
      />
    </div>
  )
}
