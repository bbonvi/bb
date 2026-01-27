import { useState, useEffect } from 'react'

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

export function useResponsiveColumns(): number {
  const [cols, setCols] = useState(() => getResponsiveColumns())
  useEffect(() => {
    const handler = () => setCols(getResponsiveColumns())
    window.addEventListener('resize', handler)
    return () => window.removeEventListener('resize', handler)
  }, [])
  return cols
}

export function getResponsiveColumns(): number {
  if (typeof window === 'undefined') return 4
  const w = window.innerWidth
  if (w < 640) return 1
  if (w < 900) return 2
  if (w < 1200) return 3
  if (w < 1800) return 4
  return 5
}
