import { useEffect, useRef, type RefObject } from 'react'
import { useStore } from '@/lib/store'

export function useScrollResetOnSearch(containerRef: RefObject<HTMLElement | null>, itemCount: number) {
  const searchQuery = useStore((s) => s.searchQuery)
  const bookmarksFresh = useStore((s) => s.bookmarksFresh)
  const queryKey = JSON.stringify(searchQuery)
  const lastQueryKeyRef = useRef(queryKey)
  const pendingResetRef = useRef(false)

  useEffect(() => {
    if (queryKey !== lastQueryKeyRef.current) {
      pendingResetRef.current = true
      lastQueryKeyRef.current = queryKey
    }
  }, [queryKey])

  useEffect(() => {
    if (!pendingResetRef.current) return
    const container = containerRef.current
    if (!container || !bookmarksFresh) return
    container.scrollTo({ top: 0 })
    pendingResetRef.current = false
  }, [containerRef, itemCount, bookmarksFresh])
}
