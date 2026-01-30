import { useMemo } from 'react'
import { useStore } from '@/lib/store'
import { useShallow } from 'zustand/react/shallow'
import { useSettings } from '@/hooks/useSettings'

const EMPTY_TAGS: string[] = []

/**
 * Returns the merged set of hidden tags:
 * - global `hidden_by_default` from server config
 * - user's global ignored tags from localStorage settings
 * - active workspace's blacklist tags
 */
export function useHiddenTags(): string[] {
  const { globalHidden, activeWorkspaceId, workspaces } = useStore(
    useShallow((s) => ({
      globalHidden: s.config?.hidden_by_default ?? EMPTY_TAGS,
      activeWorkspaceId: s.activeWorkspaceId,
      workspaces: s.workspaces,
    })),
  )
  const [settings] = useSettings()

  return useMemo(() => {
    const set = new Set(globalHidden)

    // Add user's global ignored tags
    for (const t of settings.globalIgnoredTags) set.add(t)

    // Add workspace blacklist if active
    if (activeWorkspaceId) {
      const ws = workspaces.find((w) => w.id === activeWorkspaceId)
      if (ws) {
        for (const t of ws.filters.tag_blacklist) set.add(t)
      }
    }

    return Array.from(set)
  }, [globalHidden, activeWorkspaceId, workspaces, settings.globalIgnoredTags])
}
