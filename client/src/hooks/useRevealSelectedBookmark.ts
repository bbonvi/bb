import { useEffect, useRef, type RefObject } from 'react'
import { useStore } from '@/lib/store'
import { isBookmarkFullyVisible, smoothScrollElementToCenter, smoothScrollVirtualIndexToCenter } from '@/lib/selectionScroll'

type ScrollToIndex = (index: number, opts?: { align?: 'auto' | 'start' | 'center' | 'end' }) => void

/**
 * Keeps keyboard-driven bookmark selection visible without treating passive list
 * refreshes as a request to jump to the highlighted bookmark.
 */
export function useRevealSelectedBookmark(
  containerRef: RefObject<HTMLElement | null>,
  selectedVirtualIndex: number,
  scrollToIndex: ScrollToIndex,
) {
  const selectedBookmarkId = useStore((s) => s.selectedBookmarkId)
  const selectedBookmarkRevealSeq = useStore((s) => s.selectedBookmarkRevealSeq)
  const lastRevealRef = useRef({
    selectedBookmarkId,
    selectedBookmarkRevealSeq,
  })

  useEffect(() => {
    const lastReveal = lastRevealRef.current
    const selectionChanged = selectedBookmarkId !== lastReveal.selectedBookmarkId
    const revealRequested = selectedBookmarkRevealSeq !== lastReveal.selectedBookmarkRevealSeq
    lastRevealRef.current = { selectedBookmarkId, selectedBookmarkRevealSeq }

    if (!selectionChanged && !revealRequested) return
    if (selectedBookmarkId === null || selectedVirtualIndex < 0) return

    const container = containerRef.current
    if (!container) return
    if (isBookmarkFullyVisible(container, selectedBookmarkId)) return

    const target = container.querySelector<HTMLElement>(`[data-bookmark-id="${selectedBookmarkId}"]`)
    if (target) {
      smoothScrollElementToCenter(container, target)
      return
    }

    smoothScrollVirtualIndexToCenter(container, scrollToIndex, selectedVirtualIndex)
  }, [containerRef, scrollToIndex, selectedBookmarkId, selectedBookmarkRevealSeq, selectedVirtualIndex])
}
