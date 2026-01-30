import { useState, useEffect, useMemo, useRef } from 'react'
import { TagTokenInput } from '@/components/TagTokenInput'
import { X, Plus, RefreshCw, LogOut, GripVertical, Save, RotateCcw } from 'lucide-react'
import { useStore } from '@/lib/store'
import { useHiddenTags } from '@/hooks/useHiddenTags'
import { useSettings } from '@/hooks/useSettings'
import {
  createWorkspace as apiCreateWorkspace,
  updateWorkspace as apiUpdateWorkspace,
  deleteWorkspace as apiDeleteWorkspace,
  reorderWorkspaces as apiReorderWorkspaces,
  fetchWorkspaces,
  fetchRules,
  updateRules,
  searchBookmarks,
  searchBookmarksUncached,
} from '@/lib/api'
import type { Workspace, Rule } from '@/lib/api'
import { DeleteButton } from './bookmark-parts'
import { buildWorkspaceQuery } from '@/lib/workspaceFilters'
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

type SettingsView = 'preferences' | 'workspaces' | 'rules'

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
    <div className="fixed inset-0 z-50 flex items-center justify-center bg-black/60 backdrop-blur-sm sm:p-4" onClick={() => setOpen(false)}>
      <div
        className="flex h-full w-full flex-col bg-bg sm:h-[85vh] sm:max-h-[760px] sm:max-w-5xl sm:rounded-xl sm:border sm:border-white/[0.08] sm:shadow-2xl"
        onClick={(e) => e.stopPropagation()}
      >
        {/* Header */}
        <div className="flex items-center justify-between border-b border-white/[0.06] px-4 py-3 sm:px-5">
          <h2 className="text-sm font-semibold text-text">Settings</h2>
          <button
            onClick={() => setOpen(false)}
            className="flex h-7 w-7 items-center justify-center rounded-md text-text-muted transition-colors hover:bg-surface-hover hover:text-text"
          >
            <X className="h-4 w-4" />
          </button>
        </div>

        {/* Body */}
        <div className="flex min-h-0 flex-1 flex-col sm:flex-row">
          {/* Navigation — horizontal tabs on mobile, vertical sidebar on desktop */}
          <div className="flex shrink-0 gap-1 overflow-x-auto border-b border-white/[0.06] px-2 py-2 sm:w-40 sm:flex-col sm:overflow-x-visible sm:border-b-0 sm:border-r">
            <button
              onClick={() => setSelectedView('preferences')}
              className={`flex shrink-0 items-center rounded-md px-2.5 py-1.5 text-xs transition-colors ${
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
                className={`flex shrink-0 items-center rounded-md px-2.5 py-1.5 text-xs transition-colors ${
                  selectedView === 'workspaces'
                    ? 'bg-hi-dim text-text'
                    : 'text-text-muted hover:bg-surface-hover hover:text-text'
                }`}
              >
                Workspaces
              </button>
            )}
            <button
              onClick={() => setSelectedView('rules')}
              className={`flex shrink-0 items-center rounded-md px-2.5 py-1.5 text-xs transition-colors ${
                selectedView === 'rules'
                  ? 'bg-hi-dim text-text'
                  : 'text-text-muted hover:bg-surface-hover hover:text-text'
              }`}
            >
              Rules
            </button>
          </div>

          {/* Main content */}
          <div className="flex min-w-0 flex-1 flex-col overflow-y-auto px-4 py-4 sm:px-5">
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
                refreshWorkspaces={refreshWorkspaces}
              />
            ) : selectedView === 'rules' ? (
              <RulesManager />
            ) : null}
          </div>
        </div>

        {/* Footer */}
        <div className="flex items-center justify-between border-t border-white/[0.06] px-4 py-3 sm:px-5">
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
  refreshWorkspaces,
}: {
  workspaces: Workspace[]
  visibleTags: string[]
  sensors: ReturnType<typeof useSensors>
  saving: boolean
  onDragEnd: (event: DragEndEvent) => void
  onCreate: () => void
  onDelete: (id: string) => void
  refreshWorkspaces: () => Promise<void>
}) {
  const [selectedId, setSelectedId] = useState<string | null>(null)

  // Derive the effective selected workspace, falling back to first if selection invalid
  const selectedWorkspace = useMemo(() => {
    if (workspaces.length === 0) return null
    const found = selectedId ? workspaces.find((ws) => ws.id === selectedId) : null
    return found ?? workspaces[0]
  }, [workspaces, selectedId])

  return (
    <div className="flex min-h-0 flex-1 flex-col gap-3 sm:flex-row sm:gap-4">
      {/* Workspace list */}
      <div className="flex max-h-40 shrink-0 flex-col rounded-lg border border-white/[0.06] bg-surface/30 sm:max-h-none sm:w-44">
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
      <div className="min-h-0 min-w-0 flex-1 overflow-y-auto">
        {selectedWorkspace ? (
          <WorkspaceEditor
            workspace={selectedWorkspace}
            visibleTags={visibleTags}
            refreshWorkspaces={refreshWorkspaces}
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
  refreshWorkspaces,
  onDelete,
}: {
  workspace: Workspace
  visibleTags: string[]
  refreshWorkspaces: () => Promise<void>
  onDelete: () => void
}) {
  const activeWorkspaceId = useStore((s) => s.activeWorkspaceId)
  const setActiveWorkspaceId = useStore((s) => s.setActiveWorkspaceId)
  const isActive = workspace.id === activeWorkspaceId

  const [name, setName] = useState(workspace.name)
  const [query, setQuery] = useState(workspace.filters.query ?? '')
  const [whitelist, setWhitelist] = useState<string[]>(workspace.filters.tag_whitelist ?? [])
  const [blacklist, setBlacklist] = useState<string[]>(workspace.filters.tag_blacklist ?? [])

  // Related tags (separate per list)
  const [whitelistRelated, setWhitelistRelated] = useState<string[]>([])
  const [blacklistRelated, setBlacklistRelated] = useState<string[]>([])
  const [fetchingWhitelistRelated, setFetchingWhitelistRelated] = useState(false)
  const [fetchingBlacklistRelated, setFetchingBlacklistRelated] = useState(false)

  // Reset form when workspace changes
  useEffect(() => {
    setName(workspace.name)
    setQuery(workspace.filters.query ?? '')
    setWhitelist(workspace.filters.tag_whitelist ?? [])
    setBlacklist(workspace.filters.tag_blacklist ?? [])
    setWhitelistRelated([])
    setBlacklistRelated([])
  }, [workspace.id]) // eslint-disable-line react-hooks/exhaustive-deps

  const [saving, setSaving] = useState(false)
  const [saveError, setSaveError] = useState<string | null>(null)

  // Dirty detection
  const isDirty = useMemo(() => {
    const origWhitelist = workspace.filters.tag_whitelist ?? []
    const origBlacklist = workspace.filters.tag_blacklist ?? []
    return (
      name !== workspace.name ||
      query !== (workspace.filters.query ?? '') ||
      JSON.stringify(whitelist) !== JSON.stringify(origWhitelist) ||
      JSON.stringify(blacklist) !== JSON.stringify(origBlacklist)
    )
  }, [name, query, whitelist, blacklist, workspace])

  async function handleSave() {
    setSaving(true)
    setSaveError(null)
    try {
      await apiUpdateWorkspace(workspace.id, {
        name,
        filters: {
          tag_whitelist: whitelist,
          tag_blacklist: blacklist,
          query: query || null,
        },
        view_prefs: workspace.view_prefs,
      })
      await refreshWorkspaces()
    } catch (err) {
      setSaveError(err instanceof Error ? err.message : 'Failed to save workspace')
    } finally {
      setSaving(false)
    }
  }

  function handleReset() {
    setName(workspace.name)
    setQuery(workspace.filters.query ?? '')
    setWhitelist(workspace.filters.tag_whitelist ?? [])
    setBlacklist(workspace.filters.tag_blacklist ?? [])
    setSaveError(null)
  }

  // Bookmark count — use draft filters for live preview
  const draftFilters = useMemo(() => ({
    tag_whitelist: whitelist,
    tag_blacklist: blacklist,
    query: query || null,
  }), [whitelist, blacklist, query])

  const [bookmarkCount, setBookmarkCount] = useState<number | null>(null)
  useEffect(() => {
    let cancelled = false
    const wsKeyword = buildWorkspaceQuery({ ...workspace, filters: draftFilters })
    if (!wsKeyword) {
      setBookmarkCount(null)
      return
    }
    searchBookmarksUncached({ query: wsKeyword }).then((results) => {
      if (!cancelled) setBookmarkCount(results.length)
    }).catch(() => {
      if (!cancelled) setBookmarkCount(null)
    })
    return () => { cancelled = true }
  }, [workspace.id, draftFilters]) // eslint-disable-line react-hooks/exhaustive-deps

  // Filtered autocomplete suggestions (exclude already-added tags)
  const existingTags = useMemo(
    () => new Set([...whitelist, ...blacklist]),
    [whitelist, blacklist],
  )

  const autocompleteTags = useMemo(
    () => visibleTags.filter((t) => !existingTags.has(t)),
    [visibleTags, existingTags],
  )

  async function fetchRelated(
    sourceTags: string[],
    setResult: (tags: string[]) => void,
    setLoading: (v: boolean) => void,
  ) {
    const concrete = sourceTags.filter((t) => !t.includes('*') && !t.includes('?'))
    if (concrete.length === 0) { setResult([]); return }
    setLoading(true)
    try {
      const results = await Promise.all(
        concrete.map((tag) => searchBookmarks({ tags: tag })),
      )
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

  const inputClass = 'h-7 w-full rounded-md border border-white/[0.06] bg-surface px-2 text-xs text-text outline-none transition-colors focus:border-hi-dim'
  const labelClass = 'shrink-0 w-24 text-[11px] text-text-dim pt-1.5'

  function handleKeyDown(e: React.KeyboardEvent) {
    if (e.key === 'Enter' && (e.metaKey || e.ctrlKey) && isDirty && !saving) {
      e.preventDefault()
      handleSave()
    }
  }

  return (
    <div className="flex flex-col gap-0 pb-4" onKeyDown={handleKeyDown}>
      {/* Name + activate + delete — top bar */}
      <div className="flex items-center gap-2 pb-3">
        <input
          type="text"
          value={name}
          onChange={(e) => setName(e.target.value)}
          className="h-7 flex-1 rounded-md border border-transparent bg-transparent px-2 text-sm text-text outline-none transition-colors hover:border-white/[0.06] focus:border-hi-dim focus:bg-surface"
          placeholder="Workspace name"
        />
        {isActive ? (
          <span className="flex h-7 items-center px-2 text-[11px] text-text-dim">Active</span>
        ) : (
          <button
            onClick={() => setActiveWorkspaceId(workspace.id)}
            className="h-7 shrink-0 rounded-md border border-hi/30 px-2.5 text-[11px] font-medium text-hi hover:bg-hi/10"
          >
            Activate
          </button>
        )}
        <DeleteButton onDelete={onDelete} iconClass="h-3.5 w-3.5" className="h-7 w-7 shrink-0" />
      </div>
      {saveError && (
        <div className="mb-2 rounded-md bg-danger/10 px-3 py-2 text-xs text-danger">{saveError}</div>
      )}

      {/* Show bookmarks that... */}
      <div className="rounded-lg border border-white/[0.06] bg-hi-subtle px-4 py-3">
        <div className="mb-2.5 flex items-center gap-2">
          <span className="rounded bg-hi-dim px-1.5 py-0.5 text-[10px] font-bold uppercase tracking-widest text-hi">show</span>
          <span className="text-[11px] text-text-dim">
            bookmarks matching{bookmarkCount !== null && <> &middot; {bookmarkCount} found</>}
          </span>
        </div>
        <div className="flex flex-col gap-2">
          {/* Search query */}
          <div className="flex items-start gap-2">
            <span className={labelClass}>Query</span>
            <input
              type="text"
              value={query}
              onChange={(e) => setQuery(e.target.value)}
              className={`${inputClass} font-mono`}
              placeholder="#tag .title >desc :url and or not"
            />
          </div>

          {/* Must have these tags */}
          <div className="flex items-start gap-2">
            <span className={labelClass}>Has tags</span>
            <div className="min-w-0 flex-1">
              <TagTokenInput
                tags={whitelist}
                onChange={setWhitelist}
                availableTags={autocompleteTags}
                placeholder="Add tag"
              />
              <RelatedTags
                tags={whitelistRelated}
                fetching={fetchingWhitelistRelated}
                disabled={whitelist.length === 0}
                onFetch={() => fetchRelated(whitelist, setWhitelistRelated, setFetchingWhitelistRelated)}
                onAdd={(tag) => { if (tag.trim() && !whitelist.includes(tag.trim())) setWhitelist([...whitelist, tag.trim()]) }}
                setTags={setWhitelistRelated}
              />
            </div>
          </div>

          {/* Must not have these tags */}
          <div className="flex items-start gap-2">
            <span className={labelClass}>Without tags</span>
            <div className="min-w-0 flex-1">
              <TagTokenInput
                tags={blacklist}
                onChange={setBlacklist}
                availableTags={autocompleteTags}
                placeholder="Add tag"
              />
              <RelatedTags
                tags={blacklistRelated}
                fetching={fetchingBlacklistRelated}
                disabled={blacklist.length === 0}
                onFetch={() => fetchRelated(blacklist, setBlacklistRelated, setFetchingBlacklistRelated)}
                onAdd={(tag) => { if (tag.trim() && !blacklist.includes(tag.trim())) setBlacklist([...blacklist, tag.trim()]) }}
                setTags={setBlacklistRelated}
              />
            </div>
          </div>
        </div>
      </div>
      {isDirty && (
        <div className="sticky bottom-0 z-10 mt-3 flex items-center justify-between rounded-md border border-hi/20 bg-hi/[0.06] px-3 py-2 backdrop-blur-sm">
          <span className="text-xs text-text-muted">Unsaved changes</span>
          <div className="flex items-center gap-2">
            <button
              onClick={handleReset}
              disabled={saving}
              className="flex items-center gap-1.5 rounded-md px-2.5 py-1 text-xs font-medium text-text-muted transition-colors hover:bg-surface-hover hover:text-text disabled:opacity-50"
            >
              <RotateCcw className="h-3 w-3" />
              Reset
            </button>
            <button
              onClick={handleSave}
              disabled={saving}
              className="flex items-center gap-1.5 rounded-md bg-hi px-3 py-1 text-xs font-semibold text-bg transition-all hover:bg-hi/85 disabled:opacity-50"
            >
              <Save className="h-3 w-3" />
              {saving ? 'Saving…' : 'Save'}
            </button>
          </div>
        </div>
      )}
    </div>
  )
}

// ─── Rules Manager ────────────────────────────────────────────────

function RulesManager() {
  const [savedRules, setSavedRules] = useState<Rule[]>([])
  const [draftRules, setDraftRules] = useState<Rule[]>([])
  const [selectedIndex, setSelectedIndex] = useState<number | null>(null)
  const [loading, setLoading] = useState(true)
  const [saving, setSaving] = useState(false)
  const [error, setError] = useState<string | null>(null)
  const [resetKey, setResetKey] = useState(0)

  useEffect(() => {
    fetchRules()
      .then((r) => {
        setSavedRules(r)
        setDraftRules(r)
        if (r.length > 0) setSelectedIndex(0)
      })
      .catch((err) => setError(err instanceof Error ? err.message : 'Failed to load rules'))
      .finally(() => setLoading(false))
  }, [])

  // Dirty detection
  const isDirty = useMemo(
    () => JSON.stringify(draftRules) !== JSON.stringify(savedRules),
    [draftRules, savedRules],
  )

  async function handleSave() {
    setSaving(true)
    setError(null)
    try {
      const result = await updateRules(draftRules)
      setSavedRules(result)
      setDraftRules(result)
    } catch (err) {
      setError(err instanceof Error ? err.message : 'Failed to save rules')
    } finally {
      setSaving(false)
    }
  }

  function handleReset() {
    setDraftRules(savedRules)
    setError(null)
    setResetKey((k) => k + 1)
  }

  function handleAdd() {
    const newRule: Rule = { action: { UpdateBookmark: {} } }
    const updated = [...draftRules, newRule]
    setDraftRules(updated)
    setSelectedIndex(updated.length - 1)
  }

  function handleDelete(index: number) {
    const updated = draftRules.filter((_, i) => i !== index)
    setDraftRules(updated)
    if (updated.length === 0) {
      setSelectedIndex(null)
    } else if (selectedIndex !== null && selectedIndex >= updated.length) {
      setSelectedIndex(updated.length - 1)
    }
  }

  function handleUpdateRule(index: number, patch: Partial<Rule>) {
    setDraftRules((prev) => prev.map((r, i) => (i === index ? { ...r, ...patch } : r)))
  }

  function ruleLabel(rule: Rule, index: number): string {
    const label = rule.comment || rule.url || rule.title || rule.query || 'Empty rule'
    return `${index + 1}. ${label}`
  }

  if (loading) {
    return <div className="flex h-full items-center justify-center text-sm text-text-dim">Loading rules...</div>
  }

  const selectedRule = selectedIndex !== null ? draftRules[selectedIndex] : null

  function handleKeyDown(e: React.KeyboardEvent) {
    if (e.key === 'Enter' && (e.metaKey || e.ctrlKey) && isDirty && !saving) {
      e.preventDefault()
      handleSave()
    }
  }

  return (
    <div className="flex min-h-0 flex-1 flex-col gap-3 sm:flex-row sm:gap-4" onKeyDown={handleKeyDown}>
      {/* Rule list */}
      <div className="flex max-h-40 shrink-0 flex-col rounded-lg border border-white/[0.06] bg-surface/30 sm:max-h-none sm:w-44">
        <div className="flex items-center justify-between border-b border-white/[0.06] px-3 py-2">
          <span className="text-[11px] font-medium uppercase tracking-wider text-text-dim">Rules</span>
          <button
            onClick={handleAdd}
            className="flex h-6 w-6 items-center justify-center rounded-md text-text-muted transition-colors hover:bg-surface-hover hover:text-text"
            title="Add rule"
          >
            <Plus className="h-3.5 w-3.5" />
          </button>
        </div>
        <div className="flex-1 overflow-y-auto py-1">
          {draftRules.map((rule, i) => (
            <button
              key={i}
              onClick={() => setSelectedIndex(i)}
              className={`flex w-full items-center truncate px-3 py-1.5 text-left text-xs transition-colors ${
                selectedIndex === i
                  ? 'bg-hi-dim text-text'
                  : 'text-text-muted hover:bg-surface-hover hover:text-text'
              }`}
            >
              {ruleLabel(rule, i)}
            </button>
          ))}
          {draftRules.length === 0 && (
            <div className="px-3 py-4 text-xs text-text-dim">No rules yet</div>
          )}
        </div>
      </div>

      {/* Rule editor */}
      <div className="min-h-0 min-w-0 flex-1 overflow-y-auto">
        {error && (
          <div className="mb-3 rounded-md bg-danger/10 px-3 py-2 text-xs text-danger">{error}</div>
        )}
        {selectedRule && selectedIndex !== null ? (
          <RuleEditor
            key={`${selectedIndex}-${resetKey}`}
            rule={selectedRule}
            onUpdate={(patch) => handleUpdateRule(selectedIndex, patch)}
            onDelete={() => handleDelete(selectedIndex)}
          />
        ) : (
          <div className="flex h-full items-center justify-center text-sm text-text-dim">
            {draftRules.length === 0 ? 'Add a rule to get started' : 'Select a rule'}
          </div>
        )}
        {isDirty && (
          <div className="sticky bottom-0 z-10 mt-3 flex items-center justify-between rounded-md border border-hi/20 bg-hi/[0.06] px-3 py-2 backdrop-blur-sm">
            <span className="text-xs text-text-muted">Unsaved changes</span>
            <div className="flex items-center gap-2">
              <button
                onClick={handleReset}
                disabled={saving}
                className="flex items-center gap-1.5 rounded-md px-2.5 py-1 text-xs font-medium text-text-muted transition-colors hover:bg-surface-hover hover:text-text disabled:opacity-50"
              >
                <RotateCcw className="h-3 w-3" />
                Reset
              </button>
              <button
                onClick={handleSave}
                disabled={saving}
                className="flex items-center gap-1.5 rounded-md bg-hi px-3 py-1 text-xs font-semibold text-bg transition-all hover:bg-hi/85 disabled:opacity-50"
              >
                <Save className="h-3 w-3" />
                {saving ? 'Saving…' : 'Save'}
              </button>
            </div>
          </div>
        )}
      </div>
    </div>
  )
}

// ─── Rule Editor ──────────────────────────────────────────────────

function RuleEditor({
  rule,
  onUpdate,
  onDelete,
}: {
  rule: Rule
  onUpdate: (patch: Partial<Rule>) => void
  onDelete: () => void
}) {
  const tags = useStore((s) => s.tags)
  const hiddenTags = useHiddenTags()

  const [url, setUrl] = useState(rule.url ?? '')
  const [title, setTitle] = useState(rule.title ?? '')
  const [description, setDescription] = useState(rule.description ?? '')
  const [query, setQuery] = useState(rule.query ?? '')
  const [comment, setComment] = useState(rule.comment ?? '')
  const [actionTitle, setActionTitle] = useState(rule.action.UpdateBookmark.title ?? '')
  const [actionDesc, setActionDesc] = useState(rule.action.UpdateBookmark.description ?? '')


  function saveField(patch: Partial<Rule>) {
    onUpdate(patch)
  }

  function saveActionField(field: 'title' | 'description', value: string) {
    onUpdate({
      action: {
        UpdateBookmark: {
          ...rule.action.UpdateBookmark,
          [field]: value || undefined,
        },
      },
    })
  }

  const conditionTags = useMemo(() => rule.tags ?? [], [rule.tags])
  const actionTags = useMemo(() => rule.action.UpdateBookmark.tags ?? [], [rule.action.UpdateBookmark.tags])

  // Visible tags for autocomplete (exclude hidden + already-used)
  const hiddenSet = useMemo(() => new Set(hiddenTags), [hiddenTags])
  const visibleTags = useMemo(() => tags.filter((t) => !hiddenSet.has(t)), [tags, hiddenSet])

  const inputClass = 'h-7 w-full rounded-md border border-white/[0.06] bg-surface px-2 text-xs text-text outline-none transition-colors focus:border-hi-dim'
  const labelClass = 'shrink-0 w-24 text-[11px] text-text-dim pt-1.5'

  return (
    <div className="flex flex-col gap-0 pb-4">
      {/* Comment — subtle top bar */}
      <div className="flex items-center gap-2 pb-3">
        <input
          type="text"
          value={comment}
          onChange={(e) => { setComment(e.target.value); saveField({ comment: e.target.value || undefined }) }}
          className="h-7 flex-1 rounded-md border border-transparent bg-transparent px-2 text-xs text-text-muted outline-none transition-colors hover:border-white/[0.06] focus:border-hi-dim focus:bg-surface"
          placeholder="Add a note..."
        />
        <DeleteButton onDelete={onDelete} iconClass="h-3.5 w-3.5" className="h-7 w-7 shrink-0" />
      </div>

      {/* IF — Conditions */}
      <div className="rounded-t-lg border border-white/[0.06] bg-hi-subtle px-4 py-3">
        <div className="mb-2.5 flex items-center gap-2">
          <span className="rounded bg-hi-dim px-1.5 py-0.5 text-[10px] font-bold uppercase tracking-widest text-hi">if</span>
          <span className="text-[11px] text-text-dim">all conditions match</span>
        </div>
        <div className="flex flex-col gap-2">
          <div className="flex items-start gap-2">
            <span className={labelClass}>Query</span>
            <input
              type="text"
              value={query}
              onChange={(e) => { setQuery(e.target.value); saveField({ query: e.target.value || undefined }) }}
              className={`${inputClass} font-mono`}
              placeholder="#tag .title >desc :url and or not"
            />
          </div>
          <div className="flex items-start gap-2">
            <span className={labelClass}>URL</span>
            <input
              type="text"
              value={url}
              onChange={(e) => { setUrl(e.target.value); saveField({ url: e.target.value || undefined }) }}
              className={inputClass}
              placeholder="substring or r/regex/"
            />
          </div>
          <div className="flex items-start gap-2">
            <span className={labelClass}>Title</span>
            <input
              type="text"
              value={title}
              onChange={(e) => { setTitle(e.target.value); saveField({ title: e.target.value || undefined }) }}
              className={inputClass}
              placeholder="substring or r/regex/"
            />
          </div>
          <div className="flex items-start gap-2">
            <span className={labelClass}>Description</span>
            <input
              type="text"
              value={description}
              onChange={(e) => { setDescription(e.target.value); saveField({ description: e.target.value || undefined }) }}
              className={inputClass}
              placeholder="substring or r/regex/"
            />
          </div>
          <div className="flex items-start gap-2">
            <span className={labelClass}>Has tags</span>
            <div className="min-w-0 flex-1">
              <TagTokenInput
                tags={conditionTags}
                onChange={(tags) => onUpdate({ tags: tags.length > 0 ? tags : undefined })}
                availableTags={visibleTags}
                placeholder="Add tag"
              />
            </div>
          </div>
        </div>
      </div>

      {/* THEN — Actions */}
      <div className="rounded-b-lg border border-t-0 border-white/[0.06] bg-surface/40 px-4 py-3">
        <div className="mb-2.5">
          <span className="rounded bg-surface-active px-1.5 py-0.5 text-[10px] font-bold uppercase tracking-widest text-text-muted">then</span>
        </div>
        <div className="flex flex-col gap-2">
          <div className="flex items-start gap-2">
            <span className={labelClass}>Set title</span>
            <input
              type="text"
              value={actionTitle}
              onChange={(e) => { setActionTitle(e.target.value); saveActionField('title', e.target.value) }}
              className={inputClass}
              placeholder="Override title"
            />
          </div>
          <div className="flex items-start gap-2">
            <span className={labelClass}>Set description</span>
            <input
              type="text"
              value={actionDesc}
              onChange={(e) => { setActionDesc(e.target.value); saveActionField('description', e.target.value) }}
              className={inputClass}
              placeholder="Override description"
            />
          </div>
          <div className="flex items-start gap-2">
            <span className={labelClass}>Add tags</span>
            <div className="min-w-0 flex-1">
              <TagTokenInput
                tags={actionTags}
                onChange={(tags) => onUpdate({
                  action: {
                    UpdateBookmark: {
                      ...rule.action.UpdateBookmark,
                      tags: tags.length > 0 ? tags : undefined,
                    },
                  },
                })}
                availableTags={visibleTags}
                placeholder="Add tag"
              />
            </div>
          </div>
        </div>
      </div>
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
        <TagTokenInput
          tags={settings.globalIgnoredTags}
          onChange={(tags) => updateSettings({ globalIgnoredTags: tags })}
          availableTags={visibleTags}
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
