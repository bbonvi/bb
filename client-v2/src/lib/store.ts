import { create } from 'zustand'
import { getResponsiveColumns } from '@/hooks/useResponsive'
import type {
  Bookmark,
  SearchQuery,
  TaskQueue,
  Config,
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
  setCreateModalOpen: (open: boolean) => void
  openCreateWithUrl: (url: string) => void

  // Settings
  settingsOpen: boolean
  setSettingsOpen: (open: boolean) => void

  // Workspace
  workspaces: Workspace[]
  activeWorkspaceId: string | null
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
}

export interface Workspace {
  id: string
  name: string
  filters: {
    tag_whitelist: string[]
    tag_blacklist: string[]
    url_pattern: string | null
    title_pattern: string | null
    description_pattern: string | null
    any_field_pattern: string | null
  }
  view_prefs: {
    mode: 'grid' | 'cards' | 'table' | null
    columns: number | null
  }
}

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
  setSearchQuery: (searchQuery) => set({ searchQuery, bookmarksFresh: false }),
  clearSearch: () => set({ searchQuery: emptySearchQuery, bookmarksFresh: false }),

  // UI
  viewMode: (localStorage.getItem('bb_view_mode') as 'grid' | 'cards' | 'table') || 'grid',
  columns: Number(localStorage.getItem('bb_columns')) || getResponsiveColumns(),
  shuffle: false,
  showAll: new URLSearchParams(window.location.search).get('all') === '1',
  setViewMode: (viewMode) => {
    localStorage.setItem('bb_view_mode', viewMode)
    set({ viewMode })
  },
  setColumns: (columns) => {
    localStorage.setItem('bb_columns', String(columns))
    set({ columns })
  },
  setShuffle: (shuffle) => set({ shuffle }),
  setShowAll: (showAll) => set({ showAll, bookmarksFresh: false }),
  pinToUrl: () => {
    const { searchQuery, showAll } = get()
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
  setCreateModalOpen: (createModalOpen) => set({ createModalOpen, createModalInitialUrl: '' }),
  openCreateWithUrl: (url) => set({ createModalOpen: true, createModalInitialUrl: url }),

  // Settings
  settingsOpen: false,
  setSettingsOpen: (settingsOpen) => set({ settingsOpen }),

  // Workspace
  workspaces: [],
  activeWorkspaceId: null,
  workspacesAvailable: false,
  setWorkspaces: (workspaces) => set({ workspaces }),
  setActiveWorkspaceId: (activeWorkspaceId) => set({ activeWorkspaceId }),
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
}))
