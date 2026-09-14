import type { SearchQuery } from './api'

export interface PinnedNavigationState {
  searchQuery: SearchQuery
  showAll: boolean
  activeWorkspaceId: string | null
}

interface ResolveNavigationStateOptions {
  search: string
  standalone: boolean
  pinnedState: PinnedNavigationState | null
  lastWorkspaceId: string | null
}

export interface InitialNavigationState extends PinnedNavigationState {
  urlWorkspaceName: string | null
}

const PINNED_STATE_KEY = 'bb:pinnedState'

export function isStandaloneApp(): boolean {
  return window.matchMedia('(display-mode: standalone)').matches
    || (navigator as Navigator & { standalone?: boolean }).standalone === true
}

export function loadPinnedNavigationState(): PinnedNavigationState | null {
  const raw = localStorage.getItem(PINNED_STATE_KEY)
  if (!raw) return null

  try {
    return JSON.parse(raw) as PinnedNavigationState
  } catch {
    return null
  }
}

export function savePinnedNavigationState(state: PinnedNavigationState): void {
  localStorage.setItem(PINNED_STATE_KEY, JSON.stringify(state))
}

export function resolveInitialNavigationState({
  search,
  standalone,
  pinnedState,
  lastWorkspaceId,
}: ResolveNavigationStateOptions): InitialNavigationState {
  if (standalone) {
    return {
      searchQuery: pinnedState?.searchQuery ?? {},
      showAll: pinnedState?.showAll ?? false,
      activeWorkspaceId: pinnedState ? pinnedState.activeWorkspaceId : lastWorkspaceId,
      urlWorkspaceName: null,
    }
  }

  const params = new URLSearchParams(search)
  const searchQuery: SearchQuery = {}
  if (params.get('tags')) searchQuery.tags = params.get('tags')!
  if (params.get('title')) searchQuery.title = params.get('title')!
  if (params.get('url')) searchQuery.url = params.get('url')!
  if (params.get('description')) searchQuery.description = params.get('description')!
  if (params.get('query')) searchQuery.query = params.get('query')!
  if (params.get('semantic')) searchQuery.semantic = params.get('semantic')!

  return {
    searchQuery,
    showAll: params.get('all') === '1',
    activeWorkspaceId: lastWorkspaceId,
    urlWorkspaceName: params.get('workspace'),
  }
}
