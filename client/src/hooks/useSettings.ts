import { useCallback, useSyncExternalStore } from 'react'

export interface GlobalSettings {
  showCatchAllWorkspace: boolean
  globalIgnoredTags: string[]
}

const DEFAULTS: GlobalSettings = {
  showCatchAllWorkspace: true,
  globalIgnoredTags: [],
}

const STORAGE_KEY = 'bb:settings'

// Shared state for cross-component reactivity
let currentSettings: GlobalSettings = loadSettings()
const listeners = new Set<() => void>()

function loadSettings(): GlobalSettings {
  try {
    const raw = localStorage.getItem(STORAGE_KEY)
    return raw ? { ...DEFAULTS, ...JSON.parse(raw) } : DEFAULTS
  } catch {
    return DEFAULTS
  }
}

function saveSettings(settings: GlobalSettings) {
  currentSettings = settings
  localStorage.setItem(STORAGE_KEY, JSON.stringify(settings))
  listeners.forEach((fn) => fn())
}

function subscribe(callback: () => void) {
  listeners.add(callback)
  return () => listeners.delete(callback)
}

function getSnapshot() {
  return currentSettings
}

/**
 * Hook for global app settings persisted to localStorage.
 * Uses useSyncExternalStore for cross-component reactivity.
 */
export function useSettings() {
  const settings = useSyncExternalStore(subscribe, getSnapshot, getSnapshot)

  const update = useCallback((patch: Partial<GlobalSettings>) => {
    saveSettings({ ...currentSettings, ...patch })
  }, [])

  return [settings, update] as const
}

/**
 * Get current settings synchronously (for non-React contexts).
 */
export function getSettings(): GlobalSettings {
  return currentSettings
}
