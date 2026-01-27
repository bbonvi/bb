import { useState, useEffect, type RefObject } from 'react'

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

const MIN_CARD_WIDTH = 300
const GRID_GAP = 16
const MAX_COLUMNS = 8

/** Compute column count from container width */
export function columnsForWidth(width: number): number {
  if (width <= 0) return 1
  // Account for gaps: N columns need (N-1) gaps
  // width >= N * MIN_CARD_WIDTH + (N-1) * GRID_GAP
  // width + GRID_GAP >= N * (MIN_CARD_WIDTH + GRID_GAP)
  const cols = Math.floor((width + GRID_GAP) / (MIN_CARD_WIDTH + GRID_GAP))
  return Math.max(1, Math.min(MAX_COLUMNS, cols))
}

/**
 * Observe a container's width and return the ideal column count.
 * Uses ResizeObserver for accurate container-based measurement.
 */
export function useAutoColumns(ref: RefObject<HTMLElement | null>): number {
  const [cols, setCols] = useState(() => {
    if (ref.current) return columnsForWidth(ref.current.clientWidth)
    if (typeof window !== 'undefined') return columnsForWidth(window.innerWidth)
    return 4
  })

  useEffect(() => {
    const el = ref.current
    if (!el) return

    const ro = new ResizeObserver((entries) => {
      for (const entry of entries) {
        const w = entry.contentBoxSize?.[0]?.inlineSize ?? entry.contentRect.width
        setCols(columnsForWidth(w))
      }
    })
    ro.observe(el)
    return () => ro.disconnect()
  }, [ref])

  return cols
}

// Keep legacy export for fallback/init
export function getResponsiveColumns(): number {
  if (typeof window === 'undefined') return 4
  return columnsForWidth(window.innerWidth)
}
