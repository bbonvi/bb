import { useState, useEffect, useCallback } from 'react'

export function useIsMobile(): boolean {
  const [mobile, setMobile] = useState(() =>
    typeof window !== 'undefined' ? window.innerWidth < 640 : false,
  )
  useEffect(() => {
    const mq = window.matchMedia('(max-width: 639px)')
    const handler = (e: MediaQueryListEvent) => setMobile(e.matches)
    mq.addEventListener('change', handler)
    return () => mq.removeEventListener('change', handler)
  }, [])
  return mobile
}

export const MIN_CARD_WIDTH = 300
export const GRID_GAP = 16
const MOBILE_MIN_CARD_WIDTH = 140
const MOBILE_GRID_GAP = 8
export const MAX_COLUMNS = 8
export const MAX_GRID_WIDTH = MAX_COLUMNS * MIN_CARD_WIDTH + (MAX_COLUMNS - 1) * GRID_GAP

/** Compute column count from container width */
export function columnsForWidth(width: number, compact = false): number {
  if (width <= 0) return 1
  const minCardWidth = compact ? MOBILE_MIN_CARD_WIDTH : MIN_CARD_WIDTH
  const gap = compact ? MOBILE_GRID_GAP : GRID_GAP
  // Account for gaps: N columns need (N-1) gaps
  // width >= N * MIN_CARD_WIDTH + (N-1) * GRID_GAP
  // width + GRID_GAP >= N * (MIN_CARD_WIDTH + GRID_GAP)
  const cols = Math.floor((width + gap) / (minCardWidth + gap))
  return Math.max(1, Math.min(compact ? 2 : MAX_COLUMNS, cols))
}

/**
 * Observe a container's width and return the ideal column count.
 * Uses ResizeObserver for accurate container-based measurement.
 *
 * Returns [cols, callbackRef] — attach callbackRef to the container element.
 * The callback ref ensures the observer re-attaches whenever the element
 * mounts/unmounts (e.g. grid hidden behind an empty-state early return).
 */
export function useAutoColumns(compact = false): [number, (node: HTMLElement | null) => void] {
  const [cols, setCols] = useState(() =>
    typeof window !== 'undefined' ? columnsForWidth(window.innerWidth, compact) : 4
  )

  const [el, setEl] = useState<HTMLElement | null>(null)

  // Callback ref: called by React whenever the element mounts or unmounts
  const callbackRef = useCallback((node: HTMLElement | null) => {
    setEl(node)
  }, [])

  useEffect(() => {
    if (!el) return

    const ro = new ResizeObserver((entries) => {
      for (const entry of entries) {
        const w = entry.contentBoxSize?.[0]?.inlineSize ?? entry.contentRect.width
        setCols(columnsForWidth(w, compact))
      }
    })
    ro.observe(el)
    return () => ro.disconnect()
  }, [compact, el])

  return [cols, callbackRef]
}

// Keep legacy export for fallback/init
export function getResponsiveColumns(): number {
  if (typeof window === 'undefined') return 4
  return columnsForWidth(window.innerWidth, window.innerWidth < 640)
}
