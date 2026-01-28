import { create } from 'zustand'
import { getResponsiveColumns } from '@/hooks/useResponsive'
import type {
  Bookmark,
  SearchQuery,
  TaskQueue,
  Config,
  Workspace,
} from './api'

export interface AppState {
  // Auth
  token: string | null
  setToken: (token: string | null) => void

  // Bookmarks
  bookmarks: Bookmark[]
  totalCount: number
  tags: string[]
  setBookmarks: (bookmarks: Bookmark[]) => void
  setTotalCount: (count: number) => void
  setTags: (tags: string[]) => void

  // Config
  config: Config | null
  setConfig: (config: Config) => void

  // Search
  searchQuery: SearchQuery
  setSearchQuery: (query: SearchQuery) => void
  clearSearch: () => void

  // UI
  viewMode: 'grid' | 'cards' | 'table'
  columns: number
  shuffle: boolean
  showAll: boolean
  setViewMode: (mode: 'grid' | 'cards' | 'table') => void
  setColumns: (columns: number) => void
  setShuffle: (shuffle: boolean) => void
  setShowAll: (showAll: boolean) => void
  pinToUrl: () => void

  // Detail modal
  detailModalId: number | null
  detailModalEdit: boolean
  setDetailModalId: (id: number | null) => void
  openDetailInEditMode: (id: number) => void

  // Create modal
  createModalOpen: boolean
  createModalInitialUrl: string
  createModalInitialTitle: string
  setCreateModalOpen: (open: boolean) => void
  openCreateWithUrl: (url: string) => void
  openCreateWithUrlAndTitle: (url: string, title: string) => void

  // Bulk operations
  bulkEditOpen: boolean
  bulkDeleteOpen: boolean
  setBulkEditOpen: (open: boolean) => void
  setBulkDeleteOpen: (open: boolean) => void

  // Settings
  settingsOpen: boolean
  setSettingsOpen: (open: boolean) => void

  // Workspace
  workspaces: Workspace[]
  activeWorkspaceId: string | null
  urlWorkspaceName: string | null // from ?workspace= URL param; resolved to ID when workspaces load
  workspacesAvailable: boolean // feature detection: false if 404 from /api/workspaces
  setWorkspaces: (workspaces: Workspace[]) => void
  setActiveWorkspaceId: (id: string | null) => void
  setWorkspacesAvailable: (available: boolean) => void

  // Task queue
  taskQueue: TaskQueue
  setTaskQueue: (queue: TaskQueue) => void

  // Semantic
  semanticEnabled: boolean
  setSemanticEnabled: (enabled: boolean) => void

  // Optimistic updates (§9.1, §9.2)
  dirtyIds: Set<number>
  markDirty: (id: number) => void
  clearDirty: (id: number) => void

  // Stale poll suppression (§9.1)
  pollSequence: number
  nextPollSequence: () => number

  // Shuffle seed (§8) — generated once per session
  shuffleSeed: number

  // Loading state
  initialLoadComplete: boolean
  setInitialLoadComplete: (done: boolean) => void
  bookmarksFresh: boolean
  isLoading: boolean
  setIsLoading: (loading: boolean) => void
  isUserLoading: boolean
  setIsUserLoading: (loading: boolean) => void
}

// Re-export Workspace from api for consumers that import from store
export type { Workspace } from './api'

const emptySearchQuery: SearchQuery = {}

function searchQueryFromUrl(): SearchQuery {
  const p = new URLSearchParams(window.location.search)
  const q: SearchQuery = {}
  if (p.get('tags')) q.tags = p.get('tags')!
  if (p.get('title')) q.title = p.get('title')!
  if (p.get('url')) q.url = p.get('url')!
  if (p.get('description')) q.description = p.get('description')!
  if (p.get('keyword')) q.keyword = p.get('keyword')!
  if (p.get('semantic')) q.semantic = p.get('semantic')!
  return q
}

export const useStore = create<AppState>()((set, get) => ({
  // Auth
  token: localStorage.getItem('bb_token'),
  setToken: (token) => {
    if (token) {
      localStorage.setItem('bb_token', token)
    } else {
      localStorage.removeItem('bb_token')
    }
    set({ token })
  },

  // Bookmarks
  bookmarks: [],
  totalCount: 0,
  tags: [],
  setBookmarks: (bookmarks) => set({ bookmarks, bookmarksFresh: true }),
  setTotalCount: (totalCount) => set({ totalCount }),
  setTags: (tags) => set({ tags }),

  // Config
  config: null,
  setConfig: (config) => set({ config }),

  // Search
  searchQuery: searchQueryFromUrl(),
  setSearchQuery: (searchQuery) => {
    const current = get().searchQuery
    const changed = JSON.stringify(current) !== JSON.stringify(searchQuery)
    set({ searchQuery, bookmarksFresh: false, ...(changed && { isUserLoading: true }) })
  },
  clearSearch: () => {
    const changed = JSON.stringify(get().searchQuery) !== JSON.stringify(emptySearchQuery)
    set({ searchQuery: emptySearchQuery, bookmarksFresh: false, ...(changed && { isUserLoading: true }) })
  },

  // UI
  viewMode: (localStorage.getItem('bb_view_mode') as 'grid' | 'cards' | 'table') || 'grid',
  columns: getResponsiveColumns(),
  shuffle: false,
  showAll: new URLSearchParams(window.location.search).get('all') === '1',
  setViewMode: (viewMode) => {
    localStorage.setItem('bb_view_mode', viewMode)
    set({ viewMode })
  },
  setColumns: (columns) => set({ columns }),
  setShuffle: (shuffle) => set({ shuffle }),
  setShowAll: (showAll) => {
    const changed = get().showAll !== showAll
    set({ showAll, bookmarksFresh: false, ...(changed && { isUserLoading: true }) })
  },
  pinToUrl: () => {
    const { searchQuery, showAll, activeWorkspaceId, workspaces } = get()
    const url = new URL(window.location.href)
    const fields: Record<string, string | undefined> = {
      tags: searchQuery.tags,
      title: searchQuery.title,
      url: searchQuery.url,
      description: searchQuery.description,
      keyword: searchQuery.keyword,
      semantic: searchQuery.semantic,
    }
    for (const [key, val] of Object.entries(fields)) {
      if (val) url.searchParams.set(key, val)
      else url.searchParams.delete(key)
    }
    if (showAll) url.searchParams.set('all', '1')
    else url.searchParams.delete('all')
    // Include workspace name in URL
    const activeWorkspace = workspaces.find((w) => w.id === activeWorkspaceId)
    if (activeWorkspace) url.searchParams.set('workspace', activeWorkspace.name)
    else url.searchParams.delete('workspace')
    window.history.replaceState({}, '', url)
  },

  // Detail modal
  detailModalId: null,
  detailModalEdit: false,
  setDetailModalId: (detailModalId) => set({ detailModalId, detailModalEdit: false }),
  openDetailInEditMode: (id) => set({ detailModalId: id, detailModalEdit: true }),

  // Create modal
  createModalOpen: false,
  createModalInitialUrl: '',
  createModalInitialTitle: '',
  setCreateModalOpen: (createModalOpen) =>
    set({ createModalOpen, createModalInitialUrl: '', createModalInitialTitle: '' }),
  openCreateWithUrl: (url) =>
    set({ createModalOpen: true, createModalInitialUrl: url, createModalInitialTitle: '' }),
  openCreateWithUrlAndTitle: (url, title) =>
    set({ createModalOpen: true, createModalInitialUrl: url, createModalInitialTitle: title }),

  // Bulk operations
  bulkEditOpen: false,
  bulkDeleteOpen: false,
  setBulkEditOpen: (bulkEditOpen) => set({ bulkEditOpen }),
  setBulkDeleteOpen: (bulkDeleteOpen) => set({ bulkDeleteOpen }),

  // Settings
  settingsOpen: false,
  setSettingsOpen: (settingsOpen) => set({ settingsOpen }),

  // Workspace
  workspaces: [],
  activeWorkspaceId: localStorage.getItem('bb:activeWorkspaceId') ?? null,
  urlWorkspaceName: new URLSearchParams(window.location.search).get('workspace'),
  workspacesAvailable: false,
  setWorkspaces: (workspaces) => {
    const { urlWorkspaceName, activeWorkspaceId } = get()
    const updates: Partial<AppState> = { workspaces }

    // URL workspace param takes precedence over localStorage (applied once, then cleared)
    if (urlWorkspaceName) {
      const match = workspaces.find((w) => w.name === urlWorkspaceName)
      if (match) {
        localStorage.setItem('bb:activeWorkspaceId', match.id)
        set({ workspaces, activeWorkspaceId: match.id, urlWorkspaceName: null })
        return
      }
      // No match found — clear URL param
      updates.urlWorkspaceName = null
    }

    // Clear activeWorkspaceId if it no longer exists in workspaces
    if (activeWorkspaceId && !workspaces.some((w) => w.id === activeWorkspaceId)) {
      localStorage.removeItem('bb:activeWorkspaceId')
      updates.activeWorkspaceId = null
    }

    set(updates)
  },
  setActiveWorkspaceId: (activeWorkspaceId) => {
    const changed = get().activeWorkspaceId !== activeWorkspaceId
    if (activeWorkspaceId) {
      localStorage.setItem('bb:activeWorkspaceId', activeWorkspaceId)
    } else {
      localStorage.removeItem('bb:activeWorkspaceId')
    }
    set({ activeWorkspaceId, ...(changed && { isUserLoading: true }) })
  },
  setWorkspacesAvailable: (workspacesAvailable) => set({ workspacesAvailable }),

  // Task queue
  taskQueue: { queue: [], now: 0 },
  setTaskQueue: (taskQueue) => set({ taskQueue }),

  // Semantic
  semanticEnabled: false,
  setSemanticEnabled: (semanticEnabled) => set({ semanticEnabled }),

  // Optimistic updates
  dirtyIds: new Set(),
  markDirty: (id) =>
    set((state) => {
      const next = new Set(state.dirtyIds)
      next.add(id)
      return { dirtyIds: next }
    }),
  clearDirty: (id) =>
    set((state) => {
      const next = new Set(state.dirtyIds)
      next.delete(id)
      return { dirtyIds: next }
    }),

  // Stale poll suppression
  pollSequence: 0,
  nextPollSequence: () => {
    const seq = get().pollSequence + 1
    set({ pollSequence: seq })
    return seq
  },

  // Shuffle seed
  shuffleSeed: Math.floor(Math.random() * 2147483647),

  // Loading
  initialLoadComplete: false,
  setInitialLoadComplete: (initialLoadComplete) => set({ initialLoadComplete }),
  bookmarksFresh: true,
  isLoading: false,
  setIsLoading: (isLoading) => set({ isLoading }),
  isUserLoading: false,
  setIsUserLoading: (isUserLoading) => set({ isUserLoading }),
}))
