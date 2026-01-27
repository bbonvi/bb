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
  const [cols, setCols] = useState(() => getColumns())
  useEffect(() => {
    const handler = () => setCols(getColumns())
    window.addEventListener('resize', handler)
    return () => window.removeEventListener('resize', handler)
  }, [])
  return cols
}

function getColumns(): number {
  if (typeof window === 'undefined') return 3
  const w = window.innerWidth
  if (w < 640) return 1
  if (w < 1024) return 2
  if (w < 1440) return 3
  if (w < 1920) return 4
  return 5
}
