import { useState, useEffect, useRef } from 'react'

/**
 * Returns [debouncedValue, setLocalValue, localValue].
 * localValue updates immediately (for controlled inputs).
 * debouncedValue updates after `delay` ms of inactivity.
 * External value changes (from store) sync into localValue when not actively typing.
 */
export function useDebouncedValue(
  externalValue: string,
  delay: number,
): [string, (v: string) => void, string] {
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

  return [debounced, setLocalValue, local]
}
