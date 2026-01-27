import { useState, useEffect, useRef, useCallback } from 'react'

/**
 * Returns [debouncedValue, setLocalValue, localValue, flush].
 * localValue updates immediately (for controlled inputs).
 * debouncedValue updates after `delay` ms of inactivity.
 * flush(v) bypasses debounce — sets both local and debounced instantly.
 * External value changes (from store) sync into localValue when not actively typing.
 */
export function useDebouncedValue(
  externalValue: string,
  delay: number,
): [string, (v: string) => void, string, (v: string) => void] {
  const [local, setLocal] = useState(externalValue)
  const [debounced, setDebounced] = useState(externalValue)
  const timerRef = useRef<ReturnType<typeof setTimeout> | null>(null)
  const dirtyRef = useRef(false)

  function setLocalValue(v: string) {
    dirtyRef.current = true
    setLocal(v)
    if (timerRef.current) clearTimeout(timerRef.current)
    timerRef.current = setTimeout(() => {
      setDebounced(v)
      dirtyRef.current = false
    }, delay)
  }

  const flush = useCallback((v: string) => {
    if (timerRef.current) clearTimeout(timerRef.current)
    dirtyRef.current = false
    setLocal(v)
    setDebounced(v)
  }, [])

  // Sync from external when not actively typing
  useEffect(() => {
    if (!dirtyRef.current) {
      setLocal(externalValue)
      setDebounced(externalValue)
    }
  }, [externalValue])

  // Cleanup
  useEffect(() => {
    return () => {
      if (timerRef.current) clearTimeout(timerRef.current)
    }
  }, [])

  return [debounced, setLocalValue, local, flush]
}
