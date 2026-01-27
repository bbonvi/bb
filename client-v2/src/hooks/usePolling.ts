import { useEffect, useRef } from 'react'
import { useStore } from '@/lib/store'
import {
  searchBookmarks,
  fetchTotal,
  fetchTags,
  fetchConfig,
  fetchTaskQueue,
  fetchSemanticStatus,
  fetchWorkspaces,
  ApiError,
} from '@/lib/api'
import type { Bookmark, SearchQuery, Workspace } from '@/lib/api'
import { injectWorkspaceFilters } from '@/lib/workspaceFilters'

const POLL_INTERVAL = 3000

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
  for (const [id, bm] of dirtyMap) {
    if (!incoming.some((b) => b.id === id)) merged.push(bm)
  }
  return merged
}

function isQueryEmpty(q: SearchQuery): boolean {
  return !q.semantic && !q.keyword && !q.tags && !q.title && !q.url && !q.description
}

export function usePolling() {
  const timerRef = useRef<ReturnType<typeof setTimeout> | null>(null)
  const visibleRef = useRef(true)
  const lastAppliedSeq = useRef(0)

  // Subscribe to searchQuery + showAll changes for immediate fetch
  const prevQueryRef = useRef<{ query: SearchQuery; showAll: boolean; activeWorkspaceId: string | null } | null>(null)

  useEffect(() => {
    function handleVisibility() {
      visibleRef.current = document.visibilityState === 'visible'
      if (visibleRef.current) schedulePoll(0)
    }

    document.addEventListener('visibilitychange', handleVisibility)

    async function poll() {
      if (!visibleRef.current) return

      const store = useStore.getState()
      const seq = store.nextPollSequence()
      const { searchQuery, showAll, activeWorkspaceId, workspaces } = store

      // Inject workspace filters into the search query for server-side filtering
      const activeWorkspace = activeWorkspaceId && activeWorkspaceId !== '__uncategorized__'
        ? workspaces.find((w: Workspace) => w.id === activeWorkspaceId)
        : null
      const effectiveQuery = activeWorkspace
        ? injectWorkspaceFilters(searchQuery, activeWorkspace)
        : searchQuery

      // On first load with a stored workspace, defer bookmark fetch until
      // workspaces are loaded so filters can be applied correctly
      const awaitingWorkspaces = !store.initialLoadComplete
        && activeWorkspaceId !== null
        && workspaces.length === 0

      // When a workspace is active, always fetch bookmarks (workspace implies filtering)
      const hasWorkspace = activeWorkspaceId !== null
      // Skip bookmark fetch when show-all OFF + no query + no workspace
      const shouldFetchBookmarks = !awaitingWorkspaces && (showAll || !isQueryEmpty(searchQuery) || hasWorkspace)

      try {
        const [bookmarks, totalResp, tags, config, taskQueue, semanticStatus, workspacesResult] =
          await Promise.all([
            shouldFetchBookmarks ? searchBookmarks(effectiveQuery) : Promise.resolve(null),
            fetchTotal(),
            fetchTags(),
            fetchConfig(),
            fetchTaskQueue(),
            fetchSemanticStatus(),
            fetchWorkspaces().then(
              (ws) => ({ available: true as const, workspaces: ws }),
              (err) => {
                if (err instanceof ApiError && err.status === 404) {
                  return { available: false as const, workspaces: [] }
                }
                throw err
              },
            ),
          ])

        // Stale poll suppression
        if (seq <= lastAppliedSeq.current) return
        lastAppliedSeq.current = seq

        const state = useStore.getState()

        if (bookmarks !== null) {
          state.setBookmarks(mergeWithDirty(bookmarks, state.bookmarks, state.dirtyIds))
        } else {
          // show-all OFF + no query → clear display
          state.setBookmarks([])
        }

        state.setTotalCount(totalResp.total)
        state.setTags(tags)
        state.setConfig(config)
        state.setTaskQueue(taskQueue)
        state.setSemanticEnabled(semanticStatus.enabled)
        state.setWorkspacesAvailable(workspacesResult.available)
        state.setWorkspaces(workspacesResult.workspaces)

        if (!state.initialLoadComplete) {
          state.setInitialLoadComplete(true)
          // Bookmarks were deferred until workspaces loaded — fetch now
          if (awaitingWorkspaces) {
            poll()
          }
        }
      } catch (err) {
        if (err instanceof ApiError && err.status === 401) return
        // Silently continue polling on other errors
      }
    }

    function schedulePoll(delay: number = POLL_INTERVAL) {
      if (timerRef.current) clearTimeout(timerRef.current)
      timerRef.current = setTimeout(async () => {
        await poll()
        if (visibleRef.current) schedulePoll()
      }, delay)
    }

    poll().then(() => schedulePoll())

    // Subscribe to store changes for immediate fetch on query change
    const unsub = useStore.subscribe((state) => {
      const current = { query: state.searchQuery, showAll: state.showAll, activeWorkspaceId: state.activeWorkspaceId }
      const prev = prevQueryRef.current
      if (prev && (prev.query !== current.query || prev.showAll !== current.showAll || prev.activeWorkspaceId !== current.activeWorkspaceId)) {
        // Immediate fetch — reset poll timer
        schedulePoll(0)
      }
      prevQueryRef.current = current
    })

    return () => {
      if (timerRef.current) clearTimeout(timerRef.current)
      document.removeEventListener('visibilitychange', handleVisibility)
      unsub()
    }
  }, [])
}
