import { useState, useEffect, useMemo, useCallback, useRef } from 'react'
import { X, Plus, RefreshCw, LogOut, GripVertical } from 'lucide-react'
import { useStore } from '@/lib/store'
import { useHiddenTags } from '@/hooks/useHiddenTags'
import { useSettings } from '@/hooks/useSettings'
import {
  createWorkspace as apiCreateWorkspace,
  updateWorkspace as apiUpdateWorkspace,
  deleteWorkspace as apiDeleteWorkspace,
  reorderWorkspaces as apiReorderWorkspaces,
  fetchWorkspaces,
  searchBookmarks,
  searchBookmarksUncached,
} from '@/lib/api'
import type { Workspace } from '@/lib/api'
import { DeleteButton } from './bookmark-parts'
import { buildWorkspaceKeyword } from '@/lib/workspaceFilters'
import {
  DndContext,
  closestCenter,
  KeyboardSensor,
  PointerSensor,
  useSensor,
  useSensors,
  type DragEndEvent,
} from '@dnd-kit/core'
import {
  SortableContext,
  verticalListSortingStrategy,
  useSortable,
  arrayMove,
} from '@dnd-kit/sortable'
import { CSS } from '@dnd-kit/utilities'

// ─── Settings Panel (Modal) ──────────────────────────────────────

type SettingsView = 'preferences' | 'workspaces'

export default function SettingsPanel() {
  const open = useStore((s) => s.settingsOpen)
  const setOpen = useStore((s) => s.setSettingsOpen)
  const workspaces = useStore((s) => s.workspaces)
  const setWorkspaces = useStore((s) => s.setWorkspaces)
  const workspacesAvailable = useStore((s) => s.workspacesAvailable)
  const token = useStore((s) => s.token)
  const setToken = useStore((s) => s.setToken)
  const tags = useStore((s) => s.tags)
  const hiddenTags = useHiddenTags()

  const [selectedView, setSelectedView] = useState<SettingsView>('preferences')
  const [saving, setSaving] = useState(false)
  const [error, setError] = useState<string | null>(null)
  const prevWorkspacesRef = useRef<Workspace[]>([])

  // Drag-and-drop sensors
  const sensors = useSensors(
    useSensor(PointerSensor, { activationConstraint: { distance: 5 } }),
    useSensor(KeyboardSensor),
  )

  async function handleDragEnd(event: DragEndEvent) {
    const { active, over } = event
    if (!over || active.id === over.id) return

    const oldIndex = workspaces.findIndex((ws) => ws.id === active.id)
    const newIndex = workspaces.findIndex((ws) => ws.id === over.id)
    if (oldIndex === -1 || newIndex === -1) return

    // Optimistic update
    prevWorkspacesRef.current = workspaces
    const reordered = arrayMove(workspaces, oldIndex, newIndex)
    setWorkspaces(reordered)

    // Persist to backend
    try {
      await apiReorderWorkspaces(reordered.map((ws) => ws.id))
    } catch (err) {
      // Rollback on error
      setWorkspaces(prevWorkspacesRef.current)
      setError(err instanceof Error ? err.message : 'Failed to reorder workspaces')
    }
  }

  // Reset to preferences on close
  useEffect(() => {
    if (!open) {
      setSelectedView('preferences')
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
      await apiCreateWorkspace({ name: 'New workspace' })
      await refreshWorkspaces()
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

  return (
    <div className="fixed inset-0 z-50 flex items-center justify-center bg-black/60 backdrop-blur-sm" onClick={() => setOpen(false)}>
      <div
        className="flex w-full max-w-4xl flex-col rounded-xl border border-white/[0.08] bg-bg shadow-2xl"
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
          {/* Sidebar — simple navigation */}
          <div className="flex w-40 shrink-0 flex-col border-r border-white/[0.06] py-2">
            <button
              onClick={() => setSelectedView('preferences')}
              className={`mx-2 mb-1 flex items-center rounded-md px-2.5 py-1.5 text-xs transition-colors ${
                selectedView === 'preferences'
                  ? 'bg-hi-dim text-text'
                  : 'text-text-muted hover:bg-surface-hover hover:text-text'
              }`}
            >
              Preferences
            </button>
            {workspacesAvailable && (
              <button
                onClick={() => setSelectedView('workspaces')}
                className={`mx-2 flex items-center rounded-md px-2.5 py-1.5 text-xs transition-colors ${
                  selectedView === 'workspaces'
                    ? 'bg-hi-dim text-text'
                    : 'text-text-muted hover:bg-surface-hover hover:text-text'
                }`}
              >
                Workspaces
              </button>
            )}
          </div>

          {/* Main content */}
          <div className="flex min-w-0 flex-1 flex-col overflow-y-auto px-5 py-4">
            {error && (
              <div className="mb-3 rounded-md bg-danger/10 px-3 py-2 text-xs text-danger">{error}</div>
            )}

            {selectedView === 'preferences' ? (
              <GeneralSettings visibleTags={visibleTags} />
            ) : selectedView === 'workspaces' && workspacesAvailable ? (
              <WorkspaceManager
                workspaces={workspaces}
                visibleTags={visibleTags}
                sensors={sensors}
                saving={saving}
                onDragEnd={handleDragEnd}
                onCreate={handleCreate}
                onDelete={handleDelete}
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
              />
            ) : null}
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

// ─── Workspace Manager ────────────────────────────────────────────

function WorkspaceManager({
  workspaces,
  visibleTags,
  sensors,
  saving,
  onDragEnd,
  onCreate,
  onDelete,
  onSave,
}: {
  workspaces: Workspace[]
  visibleTags: string[]
  sensors: ReturnType<typeof useSensors>
  saving: boolean
  onDragEnd: (event: DragEndEvent) => void
  onCreate: () => void
  onDelete: (id: string) => void
  onSave: (ws: Workspace) => Promise<void>
}) {
  const [selectedId, setSelectedId] = useState<string | null>(null)

  // Derive the effective selected workspace, falling back to first if selection invalid
  const selectedWorkspace = useMemo(() => {
    if (workspaces.length === 0) return null
    const found = selectedId ? workspaces.find((ws) => ws.id === selectedId) : null
    return found ?? workspaces[0]
  }, [workspaces, selectedId])

  return (
    <div className="flex min-h-0 flex-1 gap-4">
      {/* Workspace list */}
      <div className="flex w-44 shrink-0 flex-col rounded-lg border border-white/[0.06] bg-surface/30">
        <div className="flex items-center justify-between border-b border-white/[0.06] px-3 py-2">
          <span className="text-[11px] font-medium uppercase tracking-wider text-text-dim">Workspaces</span>
          <button
            onClick={onCreate}
            disabled={saving}
            className="flex h-6 w-6 items-center justify-center rounded-md text-text-muted transition-colors hover:bg-surface-hover hover:text-text disabled:opacity-50"
            title="Create workspace"
          >
            <Plus className="h-3.5 w-3.5" />
          </button>
        </div>
        <div className="flex-1 overflow-y-auto py-1">
          <DndContext
            sensors={sensors}
            collisionDetection={closestCenter}
            onDragEnd={onDragEnd}
          >
            <SortableContext items={workspaces.map((ws) => ws.id)} strategy={verticalListSortingStrategy}>
              {workspaces.map((ws) => (
                <SortableWorkspaceItem
                  key={ws.id}
                  workspace={ws}
                  isSelected={selectedWorkspace?.id === ws.id}
                  onSelect={() => setSelectedId(ws.id)}
                />
              ))}
            </SortableContext>
          </DndContext>
          {workspaces.length === 0 && (
            <div className="px-3 py-4 text-xs text-text-dim">No workspaces yet</div>
          )}
        </div>
      </div>

      {/* Workspace editor */}
      <div className="min-w-0 flex-1">
        {selectedWorkspace ? (
          <WorkspaceEditor
            workspace={selectedWorkspace}
            visibleTags={visibleTags}
            onSave={onSave}
            onDelete={() => onDelete(selectedWorkspace.id)}
          />
        ) : (
          <div className="flex h-full items-center justify-center text-sm text-text-dim">
            Create a workspace to get started
          </div>
        )}
      </div>
    </div>
  )
}

// ─── Sortable Workspace Item ─────────────────────────────────────

function SortableWorkspaceItem({
  workspace,
  isSelected,
  onSelect,
}: {
  workspace: Workspace
  isSelected: boolean
  onSelect: () => void
}) {
  const {
    attributes,
    listeners,
    setNodeRef,
    transform,
    transition,
    isDragging,
  } = useSortable({ id: workspace.id })

  const style = {
    transform: CSS.Transform.toString(transform),
    transition,
    opacity: isDragging ? 0.5 : 1,
  }

  return (
    <div
      ref={setNodeRef}
      style={style}
      className={`flex w-full items-center text-xs transition-colors ${
        isSelected
          ? 'bg-hi-dim text-text'
          : 'text-text-muted hover:bg-surface-hover hover:text-text'
      }`}
    >
      <button
        {...attributes}
        {...listeners}
        className={`flex h-7 w-6 shrink-0 cursor-grab items-center justify-center active:cursor-grabbing ${
          isSelected ? 'text-text' : 'text-text-dim hover:text-text-muted'
        }`}
        title="Drag to reorder"
      >
        <GripVertical className="h-3 w-3" />
      </button>
      <button
        onClick={onSelect}
        className="flex-1 truncate py-1.5 pr-3 text-left"
      >
        {workspace.name}
      </button>
    </div>
  )
}

// ─── Workspace Editor ────────────────────────────────────────────

function WorkspaceEditor({
  workspace,
  visibleTags,
  onSave,
  onDelete,
}: {
  workspace: Workspace
  visibleTags: string[]
  onSave: (ws: Workspace) => Promise<void>
  onDelete: () => void
}) {
  const activeWorkspaceId = useStore((s) => s.activeWorkspaceId)
  const setActiveWorkspaceId = useStore((s) => s.setActiveWorkspaceId)
  const isActive = workspace.id === activeWorkspaceId

  const [name, setName] = useState(workspace.name)
  const [whitelistInput, setWhitelistInput] = useState('')
  const [blacklistInput, setBlacklistInput] = useState('')
  const [keyword, setKeyword] = useState(workspace.filters.keyword ?? '')

  // Related tags (separate per list)
  const [whitelistRelated, setWhitelistRelated] = useState<string[]>([])
  const [blacklistRelated, setBlacklistRelated] = useState<string[]>([])
  const [fetchingWhitelistRelated, setFetchingWhitelistRelated] = useState(false)
  const [fetchingBlacklistRelated, setFetchingBlacklistRelated] = useState(false)

  // Reset form when workspace changes
  useEffect(() => {
    setName(workspace.name)
    setKeyword(workspace.filters.keyword ?? '')
    setWhitelistInput('')
    setBlacklistInput('')
    setWhitelistRelated([])
    setBlacklistRelated([])
  }, [workspace.id]) // eslint-disable-line react-hooks/exhaustive-deps

  // Bookmark count for this workspace
  const [bookmarkCount, setBookmarkCount] = useState<number | null>(null)
  useEffect(() => {
    let cancelled = false
    const wsKeyword = buildWorkspaceKeyword(workspace)
    if (!wsKeyword) {
      setBookmarkCount(null)
      return
    }
    searchBookmarksUncached({ keyword: wsKeyword }).then((results) => {
      if (!cancelled) setBookmarkCount(results.length)
    }).catch(() => {
      if (!cancelled) setBookmarkCount(null)
    })
    return () => { cancelled = true }
  }, [workspace.id, workspace.filters])

  // Filtered autocomplete suggestions (exclude already-added tags)
  const whitelist = useMemo(() => workspace.filters.tag_whitelist ?? [], [workspace.filters.tag_whitelist])
  const blacklist = useMemo(() => workspace.filters.tag_blacklist ?? [], [workspace.filters.tag_blacklist])

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
      keyword: keyword || null,
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

  async function fetchRelated(
    sourceTags: string[],
    setResult: (tags: string[]) => void,
    setLoading: (v: boolean) => void,
  ) {
    const concrete = sourceTags.filter((t) => !t.includes('*') && !t.includes('?'))
    if (concrete.length === 0) { setResult([]); return }
    setLoading(true)
    try {
      // Fetch per-tag to avoid overly restrictive AND queries
      const results = await Promise.all(
        concrete.map((tag) => searchBookmarks({ tags: tag })),
      )
      // Count co-occurrence: tags appearing with more source tags rank higher
      const freq = new Map<string, number>()
      for (const bookmarks of results) {
        const seen = new Set<string>()
        for (const bm of bookmarks) {
          for (const t of bm.tags) {
            if (!existingTags.has(t) && !seen.has(t)) {
              seen.add(t)
              freq.set(t, (freq.get(t) ?? 0) + 1)
            }
          }
        }
      }
      // Sort by frequency desc, then alphabetically
      const sorted = Array.from(freq.entries())
        .sort((a, b) => b[1] - a[1] || a[0].localeCompare(b[0]))
        .map(([tag]) => tag)
      setResult(sorted)
    } catch {
      // ignore
    } finally {
      setLoading(false)
    }
  }

  const handleNameBlur = useCallback(() => {
    if (name !== workspace.name && name.trim()) {
      saveWorkspace()
    }
  }, [name, workspace.name]) // eslint-disable-line react-hooks/exhaustive-deps

  const handleKeywordBlur = useCallback(() => {
    saveWorkspace()
  }, [keyword]) // eslint-disable-line react-hooks/exhaustive-deps

  return (
    <div className="flex flex-col gap-4">
      {/* Name + activate + delete */}
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
        {isActive ? (
          <span className="flex h-8 items-center px-2 text-xs text-text-dim">Active</span>
        ) : (
          <button
            onClick={() => setActiveWorkspaceId(workspace.id)}
            className="h-8 rounded-md border border-hi/30 px-2.5 text-xs font-medium text-hi hover:bg-hi/10"
          >
            Activate
          </button>
        )}
        <DeleteButton onDelete={onDelete} iconClass="h-3.5 w-3.5" className="h-8 w-8" />
      </div>
      {bookmarkCount !== null && (
        <p className="text-xs text-text-dim">
          {bookmarkCount} {bookmarkCount === 1 ? 'bookmark' : 'bookmarks'} matching filters
        </p>
      )}

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
        <RelatedTags
          tags={whitelistRelated}
          fetching={fetchingWhitelistRelated}
          disabled={whitelist.length === 0}
          onFetch={() => fetchRelated(whitelist, setWhitelistRelated, setFetchingWhitelistRelated)}
          onAdd={addWhitelistTag}
          setTags={setWhitelistRelated}
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
        <RelatedTags
          tags={blacklistRelated}
          fetching={fetchingBlacklistRelated}
          disabled={blacklist.length === 0}
          onFetch={() => fetchRelated(blacklist, setBlacklistRelated, setFetchingBlacklistRelated)}
          onAdd={addBlacklistTag}
          setTags={setBlacklistRelated}
        />
      </div>

      {/* Keyword filter */}
      <div>
        <label className="mb-1 block text-[11px] font-medium uppercase tracking-wider text-text-dim">
          Keyword filter
        </label>
        <input
          type="text"
          value={keyword}
          onChange={(e) => setKeyword(e.target.value)}
          onBlur={handleKeywordBlur}
          onKeyDown={(e) => {
            if (e.key === 'Enter') handleKeywordBlur()
          }}
          className="h-7 w-full rounded-md border border-white/[0.06] bg-surface px-2 text-xs font-mono text-text outline-none transition-colors focus:border-hi-dim"
          placeholder="e.g. .tutorial :github.com"
        />
        <p className="mt-1 text-[11px] text-text-dim">
          Keyword search query — combined with tag filters above
        </p>
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
              <button
                onClick={() => onRemove(tag)}
                className="text-text-dim transition-colors hover:text-danger"
              >
                <X className="h-2.5 w-2.5" />
              </button>
              {tag}
            </span>
          ))}
        </div>
      )}
    </div>
  )
}

// ─── Related Tags ───────────────────────────────────────────────

function RelatedTags({
  tags,
  fetching,
  disabled,
  onFetch,
  onAdd,
  setTags,
}: {
  tags: string[]
  fetching: boolean
  disabled: boolean
  onFetch: () => void
  onAdd: (tag: string) => void
  setTags: (tags: string[]) => void
}) {
  function handleAdd(tag: string) {
    onAdd(tag)
    setTags(tags.filter((t) => t !== tag))
  }

  return (
    <div className="mt-1.5">
      <button
        onClick={onFetch}
        disabled={fetching || disabled}
        className="flex items-center gap-1.5 rounded-md px-2 py-1 text-[11px] font-medium text-text-dim transition-colors hover:bg-surface-hover hover:text-text-muted disabled:opacity-40"
      >
        <RefreshCw className={`h-2.5 w-2.5 ${fetching ? 'animate-spin' : ''}`} />
        Related tags
      </button>
      {tags.length > 0 && (
        <div className="mt-1 flex flex-wrap gap-1">
          {tags.map((tag) => (
            <button
              key={tag}
              onClick={() => handleAdd(tag)}
              className="rounded px-1.5 py-0.5 text-[11px] text-text-muted transition-colors hover:bg-surface-hover hover:text-text"
            >
              {tag}
            </button>
          ))}
        </div>
      )}
    </div>
  )
}

// ─── General Settings ─────────────────────────────────────────────

function GeneralSettings({ visibleTags }: { visibleTags: string[] }) {
  const [settings, updateSettings] = useSettings()
  const [ignoredInput, setIgnoredInput] = useState('')

  const existingIgnored = useMemo(
    () => new Set(settings.globalIgnoredTags),
    [settings.globalIgnoredTags],
  )

  const suggestions = useMemo(() => {
    if (!ignoredInput) return []
    const lower = ignoredInput.toLowerCase()
    return visibleTags
      .filter((t) => !existingIgnored.has(t) && t.toLowerCase().includes(lower))
      .slice(0, 8)
  }, [ignoredInput, visibleTags, existingIgnored])

  function addIgnoredTag(tag: string) {
    const trimmed = tag.trim()
    if (!trimmed || existingIgnored.has(trimmed)) return
    updateSettings({
      globalIgnoredTags: [...settings.globalIgnoredTags, trimmed],
    })
    setIgnoredInput('')
  }

  function removeIgnoredTag(tag: string) {
    updateSettings({
      globalIgnoredTags: settings.globalIgnoredTags.filter((t) => t !== tag),
    })
  }

  return (
    <div className="flex flex-col gap-6">
      <h3 className="text-sm font-semibold text-text">Preferences</h3>

      {/* Toggle: Show catch-all workspace */}
      <ToggleSetting
        label="Show catch-all workspace"
        description="Display '---' option in workspace selector to view all bookmarks"
        checked={settings.showCatchAllWorkspace}
        onChange={(v) => updateSettings({ showCatchAllWorkspace: v })}
      />

      {/* Polling intervals */}
      <div>
        <label className="mb-1.5 block text-[11px] font-medium uppercase tracking-wider text-text-dim">
          Polling intervals (seconds)
        </label>
        <p className="mb-2 text-xs text-text-muted">
          How often all data is refreshed in the background
        </p>
        <div className="flex gap-3">
          <div className="flex-1">
            <label className="mb-1 block text-[11px] text-text-dim">Normal</label>
            <input
              type="number"
              step={0.5}
              min={0.5}
              value={settings.pollIntervalNormal / 1000}
              onChange={(e) => updateSettings({ pollIntervalNormal: Math.max(500, Number(e.target.value) * 1000) })}
              className="h-7 w-full rounded-md border border-white/[0.06] bg-surface px-2 text-xs text-text outline-none transition-colors focus:border-hi-dim"
            />
          </div>
          <div className="flex-1">
            <label className="mb-1 block text-[11px] text-text-dim">Busy</label>
            <input
              type="number"
              step={0.5}
              min={0.5}
              value={settings.pollIntervalBusy / 1000}
              onChange={(e) => updateSettings({ pollIntervalBusy: Math.max(500, Number(e.target.value) * 1000) })}
              className="h-7 w-full rounded-md border border-white/[0.06] bg-surface px-2 text-xs text-text outline-none transition-colors focus:border-hi-dim"
            />
          </div>
          <div className="flex-1">
            <label className="mb-1 block text-[11px] text-text-dim">Hidden tab</label>
            <input
              type="number"
              step={0.5}
              min={5}
              value={settings.pollIntervalHidden / 1000}
              onChange={(e) => updateSettings({ pollIntervalHidden: Math.max(5000, Number(e.target.value) * 1000) })}
              className="h-7 w-full rounded-md border border-white/[0.06] bg-surface px-2 text-xs text-text outline-none transition-colors focus:border-hi-dim"
            />
          </div>
        </div>
      </div>

      {/* Global ignored tags */}
      <div>
        <label className="mb-1.5 block text-[11px] font-medium uppercase tracking-wider text-text-dim">
          Globally ignored tags
        </label>
        <p className="mb-2 text-xs text-text-muted">
          Bookmarks with these tags are completely hidden everywhere
        </p>
        <TagListEditor
          tags={settings.globalIgnoredTags}
          input={ignoredInput}
          setInput={setIgnoredInput}
          suggestions={suggestions}
          onAdd={addIgnoredTag}
          onRemove={removeIgnoredTag}
          placeholder="Add tag to ignore globally"
        />
      </div>
    </div>
  )
}

// ─── Toggle Setting ───────────────────────────────────────────────

function ToggleSetting({
  label,
  description,
  checked,
  onChange,
}: {
  label: string
  description: string
  checked: boolean
  onChange: (v: boolean) => void
}) {
  return (
    <label className="flex cursor-pointer items-start gap-3">
      <input
        type="checkbox"
        checked={checked}
        onChange={(e) => onChange(e.target.checked)}
        className="mt-0.5 h-4 w-4 rounded border-white/20 bg-surface accent-hi"
      />
      <div>
        <div className="text-sm text-text">{label}</div>
        <div className="text-xs text-text-muted">{description}</div>
      </div>
    </label>
  )
}
