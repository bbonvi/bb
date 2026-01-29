import { useEffect, useRef, useCallback } from 'react'
import { useStore } from '@/lib/store'
import {
  searchBookmarks,
  fetchTotal,
  fetchTags,
  fetchConfig,
  fetchTaskQueue,
  fetchSemanticStatus,
  fetchWorkspaces,
  clearEtagCache,
  ApiError,
} from '@/lib/api'
import type { Bookmark, SearchQuery, Workspace } from '@/lib/api'
import { mergeWorkspaceQuery } from '@/lib/workspaceFilters'
import { getSettings } from '@/hooks/useSettings'

function hasActiveTasks(): boolean {
  const queue = useStore.getState().taskQueue.queue
  return queue.some((t) => t.status === 'Pending' || t.status === 'InProgress')
}

function getAuxInterval(visible: boolean): number {
  const s = getSettings()
  if (!visible) return s.pollIntervalHidden
  return hasActiveTasks() ? s.pollIntervalBusy : s.pollIntervalNormal
}

function mergeWithDirty(
  incoming: Bookmark[],
  current: Bookmark[],
  dirtyIds: Set<number>,
): Bookmark[] {
  if (dirtyIds.size === 0) return incoming
  const dirtyMap = new Map<number, Bookmark>()
  for (const bm of current) {
    if (dirtyIds.has(bm.id)) dirtyMap.set(bm.id, bm)
  }
  const merged = incoming.map((bm) => dirtyMap.get(bm.id) ?? bm)
  const incomingIds = new Set(incoming.map((b) => b.id))
  for (const [id, bm] of dirtyMap) {
    if (!incomingIds.has(id)) merged.push(bm)
  }
  return merged
}

function isQueryEmpty(q: SearchQuery): boolean {
  return !q.semantic && !q.keyword && !q.tags && !q.title && !q.url && !q.description
}

export function usePolling() {
  const auxTimerRef = useRef<ReturnType<typeof setTimeout> | null>(null)
  const visibleRef = useRef(true)
  const lastAppliedSeq = useRef(0)

  // Fetch bookmarks (event-driven, not on a timer)
  const fetchBookmarks = useCallback(async () => {
    const store = useStore.getState()
    const seq = store.nextPollSequence()
    const { searchQuery, showAll, activeWorkspaceId, workspaces } = store

    const activeWorkspace = activeWorkspaceId
      ? workspaces.find((w: Workspace) => w.id === activeWorkspaceId)
      : null
    const effectiveQuery = activeWorkspace
      ? mergeWorkspaceQuery(searchQuery, activeWorkspace)
      : searchQuery

    // Defer until workspaces loaded on first mount
    const awaitingWorkspaces = !store.initialLoadComplete
      && (activeWorkspaceId !== null || store.urlWorkspaceName !== null)
      && workspaces.length === 0
    if (awaitingWorkspaces) return

    const hasWorkspace = activeWorkspaceId !== null
    const shouldFetchBookmarks = showAll || !isQueryEmpty(searchQuery) || hasWorkspace

    store.setIsLoading(true)
    try {
      if (shouldFetchBookmarks) {
        const bookmarks = await searchBookmarks(effectiveQuery)
        if (seq <= lastAppliedSeq.current) return
        lastAppliedSeq.current = seq
        const state = useStore.getState()
        state.setBookmarks(mergeWithDirty(bookmarks, state.bookmarks, state.dirtyIds))
      } else {
        if (seq <= lastAppliedSeq.current) return
        lastAppliedSeq.current = seq
        useStore.getState().setBookmarks([])
      }

      const state = useStore.getState()
      if (!state.initialLoadComplete) {
        state.setInitialLoadComplete(true)
      }
    } catch (err) {
      if (err instanceof ApiError && err.status === 401) return
    } finally {
      const s = useStore.getState()
      s.setIsLoading(false)
      s.setIsUserLoading(false)
    }
  }, [])

  // Fetch auxiliary data (tags, total, config, task queue, semantic status, workspaces)
  const fetchAuxiliary = useCallback(async () => {
    if (!visibleRef.current) return

    try {
      const [totalResp, tags, config, taskQueue, semanticStatus, workspacesResult] =
        await Promise.all([
          fetchTotal(),
          fetchTags(),
          fetchConfig(),
          fetchTaskQueue(),
          fetchSemanticStatus(),
          fetchWorkspaces().then(
            (ws) => ({ available: true as const, workspaces: ws }),
            (err) => {
              if (err instanceof ApiError && err.status === 404) {
                return { available: false as const, workspaces: [] as Workspace[] }
              }
              throw err
            },
          ),
        ])

      const state = useStore.getState()
      state.setTotalCount(totalResp.total)
      state.setTags(tags)
      state.setConfig(config)
      state.setTaskQueue(taskQueue)
      state.setSemanticEnabled(semanticStatus.enabled)
      state.setWorkspacesAvailable(workspacesResult.available)
      state.setWorkspaces(workspacesResult.workspaces)
    } catch (err) {
      if (err instanceof ApiError && err.status === 401) return
    }
  }, [])

  const lastAuxFetchRef = useRef(0)

  useEffect(() => {
    // --- Visibility change: refetch on tab refocus with debounce ---
    function handleVisibility() {
      visibleRef.current = document.visibilityState === 'visible'
      if (visibleRef.current) {
        const elapsed = Date.now() - lastAuxFetchRef.current
        const minGap = getSettings().pollIntervalNormal
        if (elapsed >= minGap) {
          fetchBookmarks()
          fetchAuxiliary()
        }
        // Always restart polling when tab regains focus
        scheduleAuxPoll()
      }
    }
    document.addEventListener('visibilitychange', handleVisibility)

    // --- Initial load ---
    // Fetch auxiliary first (to get workspaces), then bookmarks
    fetchAuxiliary().then(() => fetchBookmarks())

    // --- Auxiliary poll on dynamic interval ---
    function scheduleAuxPoll() {
      if (auxTimerRef.current) clearTimeout(auxTimerRef.current)
      const interval = getAuxInterval(visibleRef.current)
      auxTimerRef.current = setTimeout(async () => {
        await fetchAuxiliary()
        lastAuxFetchRef.current = Date.now()

        // Always refetch bookmarks on each cycle — ETags make this
        // cheap (304 when unchanged), and it keeps data fresh when
        // external changes occur (task completions, other clients).
        fetchBookmarks()

        scheduleAuxPoll()
      }, interval)
    }
    scheduleAuxPoll()

    // --- Subscribe to store changes for event-driven bookmark fetch ---
    let prevQuery = useStore.getState().searchQuery
    let prevShowAll = useStore.getState().showAll
    let prevActiveWorkspaceId = useStore.getState().activeWorkspaceId
    let prevRefetchTrigger = useStore.getState().refetchTrigger

    const unsub = useStore.subscribe((state) => {
      const queryChanged = state.searchQuery !== prevQuery
      const showAllChanged = state.showAll !== prevShowAll
      const workspaceChanged = state.activeWorkspaceId !== prevActiveWorkspaceId
      const triggerChanged = state.refetchTrigger !== prevRefetchTrigger

      if (queryChanged || showAllChanged || workspaceChanged) {
        // Query/filter changed — clear ETag to get fresh data
        clearEtagCache('bookmarks')
      }

      if (queryChanged || showAllChanged || workspaceChanged || triggerChanged) {
        fetchBookmarks()
      }

      prevQuery = state.searchQuery
      prevShowAll = state.showAll
      prevActiveWorkspaceId = state.activeWorkspaceId
      prevRefetchTrigger = state.refetchTrigger
    })

    return () => {
      if (auxTimerRef.current) clearTimeout(auxTimerRef.current)
      document.removeEventListener('visibilitychange', handleVisibility)
      unsub()
    }
  }, [fetchBookmarks, fetchAuxiliary])
}
