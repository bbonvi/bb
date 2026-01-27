import { useEffect, useRef } from 'react'
import { useStore } from '@/lib/store'
import {
  searchBookmarks,
  fetchTotal,
  fetchTags,
  fetchConfig,
  fetchTaskQueue,
  fetchSemanticStatus,
  ApiError,
} from '@/lib/api'
import type { Bookmark } from '@/lib/api'

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

export function usePolling() {
  const timerRef = useRef<ReturnType<typeof setTimeout> | null>(null)
  const visibleRef = useRef(true)
  const lastAppliedSeq = useRef(0)

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
      const semanticQuery = store.searchQuery.semantic

      try {
        // Build parallel requests
        const requests: [
          Promise<Bookmark[]>,
          Promise<{ total: number }>,
          Promise<string[]>,
          Promise<ReturnType<typeof fetchConfig>>,
          Promise<ReturnType<typeof fetchTaskQueue>>,
          Promise<ReturnType<typeof fetchSemanticStatus>>,
          Promise<Bookmark[]> | null,
        ] = [
          searchBookmarks({}),
          fetchTotal(),
          fetchTags(),
          fetchConfig(),
          fetchTaskQueue(),
          fetchSemanticStatus(),
          // Dual-poll: if semantic search active, also fetch ranked results
          semanticQuery
            ? searchBookmarks({ semantic: semanticQuery, threshold: store.searchQuery.threshold })
            : null,
        ]

        const [bookmarks, totalResp, tags, config, taskQueue, semanticStatus] =
          await Promise.all(requests.slice(0, 6) as [
            Promise<Bookmark[]>,
            Promise<{ total: number }>,
            Promise<string[]>,
            Promise<ReturnType<typeof fetchConfig>>,
            Promise<ReturnType<typeof fetchTaskQueue>>,
            Promise<ReturnType<typeof fetchSemanticStatus>>,
          ])

        const semanticResults = requests[6] ? await requests[6] : null

        // Stale poll suppression
        if (seq <= lastAppliedSeq.current) return
        lastAppliedSeq.current = seq

        const state = useStore.getState()
        const dirtyIds = state.dirtyIds

        state.setBookmarks(mergeWithDirty(bookmarks, state.bookmarks, dirtyIds))
        state.setSemanticResults(
          semanticResults
            ? mergeWithDirty(semanticResults, state.semanticResults ?? [], dirtyIds)
            : null,
        )
        state.setTotalCount(totalResp.total)
        state.setTags(tags)
        state.setConfig(config)
        state.setTaskQueue(taskQueue)
        state.setSemanticEnabled(semanticStatus.enabled)

        if (!state.initialLoadComplete) {
          state.setInitialLoadComplete(true)
        }
      } catch (err) {
        if (err instanceof ApiError && err.status === 401) return
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

    return () => {
      if (timerRef.current) clearTimeout(timerRef.current)
      document.removeEventListener('visibilitychange', handleVisibility)
    }
  }, [])
}
