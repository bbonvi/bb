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

function getPollInterval(visible: boolean): number {
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
  return !q.semantic && !q.query && !q.tags && !q.title && !q.url && !q.description
}

export function usePolling() {
  const pollTimerRef = useRef<ReturnType<typeof setTimeout> | null>(null)
  const visibleRef = useRef(true)
  const lastAppliedSeq = useRef(0)
  const abortRef = useRef<AbortController | null>(null)

  // Single fetch pathway for bookmarks. Only called from the store subscription.
  const fetchBookmarks = useCallback(async () => {
    // Abort any in-flight request to prevent stale responses from
    // polluting the ETag/response cache
    abortRef.current?.abort()
    const controller = new AbortController()
    abortRef.current = controller

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

    const shouldFetchBookmarks = showAll || !isQueryEmpty(searchQuery)

    store.setIsLoading(true)
    try {
      if (shouldFetchBookmarks) {
        const bookmarks = await searchBookmarks(effectiveQuery, controller.signal)
        if (seq <= lastAppliedSeq.current) return
        lastAppliedSeq.current = seq
        const state = useStore.getState()
        state.setBookmarks(mergeWithDirty(bookmarks, state.bookmarks, state.dirtyIds))
      } else {
        if (seq <= lastAppliedSeq.current) return
        lastAppliedSeq.current = seq
        useStore.getState().setBookmarks([])
      }

      useStore.getState().setSearchError(null)

      const state = useStore.getState()
      if (!state.initialLoadComplete) {
        state.setInitialLoadComplete(true)
      }
    } catch (err) {
      if (controller.signal.aborted) return
      if (err instanceof ApiError && err.status === 401) return
      if (err instanceof ApiError && err.code === 'INVALID_QUERY') {
        useStore.getState().setSearchError(err.message)
        return
      }
    } finally {
      // Don't clear loading if this request was aborted — the replacement owns it
      if (!controller.signal.aborted) {
        const s = useStore.getState()
        s.setIsLoading(false)
        s.setIsUserLoading(false)
      }
    }
  }, [])

  // Fetch metadata (tags, total, config, task queue, semantic status, workspaces)
  const fetchMetadata = useCallback(async () => {
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

  const lastPollRef = useRef(0)

  useEffect(() => {
    // --- Store subscription: sole entry point for bookmark fetches ---
    // All triggers (query change, workspace switch, poll tick, visibility)
    // funnel through store state changes which this subscription observes.
    // Microtask batching coalesces rapid consecutive changes into one fetch.
    let prevQuery = useStore.getState().searchQuery
    let prevShowAll = useStore.getState().showAll
    let prevActiveWorkspaceId = useStore.getState().activeWorkspaceId
    let prevRefetchTrigger = useStore.getState().refetchTrigger
    let fetchQueued = false

    const unsub = useStore.subscribe((state) => {
      const queryChanged = state.searchQuery !== prevQuery
      const showAllChanged = state.showAll !== prevShowAll
      const workspaceChanged = state.activeWorkspaceId !== prevActiveWorkspaceId
      const triggerChanged = state.refetchTrigger !== prevRefetchTrigger

      if (queryChanged || showAllChanged || workspaceChanged) {
        clearEtagCache('bookmarks')
      }

      prevQuery = state.searchQuery
      prevShowAll = state.showAll
      prevActiveWorkspaceId = state.activeWorkspaceId
      prevRefetchTrigger = state.refetchTrigger

      if ((queryChanged || showAllChanged || workspaceChanged || triggerChanged) && !fetchQueued) {
        fetchQueued = true
        queueMicrotask(() => {
          fetchQueued = false
          fetchBookmarks()
        })
      }
    })

    // --- Visibility change ---
    function handleVisibility() {
      visibleRef.current = document.visibilityState === 'visible'
      if (visibleRef.current) {
        const elapsed = Date.now() - lastPollRef.current
        const minGap = getSettings().pollIntervalNormal
        if (elapsed >= minGap) {
          fetchMetadata()
          // Bookmark fetch goes through the subscription via triggerRefetch
          useStore.getState().triggerRefetch()
        }
        schedulePoll()
      }
    }
    document.addEventListener('visibilitychange', handleVisibility)

    // --- Initial load ---
    fetchMetadata().then(() => {
      // After metadata (workspaces) are loaded, trigger bookmark fetch
      // through the subscription to maintain single pathway
      useStore.getState().triggerRefetch()
    })

    // --- Poll on dynamic interval ---
    function schedulePoll() {
      if (pollTimerRef.current) clearTimeout(pollTimerRef.current)
      const interval = getPollInterval(visibleRef.current)
      pollTimerRef.current = setTimeout(async () => {
        await fetchMetadata()
        lastPollRef.current = Date.now()

        // Bookmark fetch via subscription — coalesces with any
        // metadata-triggered subscription fetch (e.g. workspace list change)
        useStore.getState().triggerRefetch()

        schedulePoll()
      }, interval)
    }
    schedulePoll()

    return () => {
      if (pollTimerRef.current) clearTimeout(pollTimerRef.current)
      document.removeEventListener('visibilitychange', handleVisibility)
      unsub()
    }
  }, [fetchBookmarks, fetchMetadata])
}
