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

const POLL_INTERVAL = 3000

export function usePolling() {
  const timerRef = useRef<ReturnType<typeof setTimeout> | null>(null)
  const visibleRef = useRef(true)
  const lastAppliedSeq = useRef(0)

  useEffect(() => {
    function handleVisibility() {
      visibleRef.current = document.visibilityState === 'visible'
      // Resume polling immediately when tab becomes visible
      if (visibleRef.current) {
        schedulePoll(0)
      }
    }

    document.addEventListener('visibilitychange', handleVisibility)

    async function poll() {
      if (!visibleRef.current) return

      const store = useStore.getState()
      const seq = store.nextPollSequence()

      try {
        // Fire all poll requests in parallel
        const [bookmarks, totalResp, tags, config, taskQueue, semanticStatus] =
          await Promise.all([
            searchBookmarks({}),
            fetchTotal(),
            fetchTags(),
            fetchConfig(),
            fetchTaskQueue(),
            fetchSemanticStatus(),
          ])

        // Stale poll suppression: discard if a newer poll was already applied
        if (seq <= lastAppliedSeq.current) return
        lastAppliedSeq.current = seq

        const state = useStore.getState()

        // Merge bookmarks respecting dirty IDs (§9.1)
        const dirtyIds = state.dirtyIds
        if (dirtyIds.size > 0) {
          const dirtyMap = new Map<number, typeof bookmarks[0]>()
          for (const bm of state.bookmarks) {
            if (dirtyIds.has(bm.id)) {
              dirtyMap.set(bm.id, bm)
            }
          }
          const merged = bookmarks.map((bm) => dirtyMap.get(bm.id) ?? bm)
          // Keep dirty bookmarks that aren't in server response (pending save)
          for (const [id, bm] of dirtyMap) {
            if (!bookmarks.some((b) => b.id === id)) {
              merged.push(bm)
            }
          }
          state.setBookmarks(merged)
        } else {
          state.setBookmarks(bookmarks)
        }

        state.setTotalCount(totalResp.total)
        state.setTags(tags)
        state.setConfig(config)
        state.setTaskQueue(taskQueue)
        state.setSemanticEnabled(semanticStatus.enabled)

        if (!state.initialLoadComplete) {
          state.setInitialLoadComplete(true)
        }
      } catch (err) {
        // On 401 the API client handles redirect; skip other errors silently
        // (poll will retry next cycle)
        if (err instanceof ApiError && err.status === 401) return
      }
    }

    function schedulePoll(delay: number = POLL_INTERVAL) {
      if (timerRef.current) clearTimeout(timerRef.current)
      timerRef.current = setTimeout(async () => {
        await poll()
        if (visibleRef.current) {
          schedulePoll()
        }
      }, delay)
    }

    // Initial poll immediately
    poll().then(() => schedulePoll())

    return () => {
      if (timerRef.current) clearTimeout(timerRef.current)
      document.removeEventListener('visibilitychange', handleVisibility)
    }
  }, [])
}
