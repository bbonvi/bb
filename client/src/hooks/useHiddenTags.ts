import { useMemo } from 'react'
import { useStore } from '@/lib/store'
import { useShallow } from 'zustand/react/shallow'

/**
 * Returns the merged set of hidden tags: global `hidden_by_default` from config
 * plus the active workspace's blacklist tags.
 * §6.5: visible tags = all_tags - global_hidden - workspace_blacklist
 */
export function useHiddenTags(): string[] {
  const { globalHidden, activeWorkspaceId, workspaces } = useStore(
    useShallow((s) => ({
      globalHidden: s.config?.hidden_by_default ?? [],
      activeWorkspaceId: s.activeWorkspaceId,
      workspaces: s.workspaces,
    })),
  )

  return useMemo(() => {
    if (!activeWorkspaceId) {
      return globalHidden
    }

    const ws = workspaces.find((w) => w.id === activeWorkspaceId)
    if (!ws || ws.filters.tag_blacklist.length === 0) {
      return globalHidden
    }

    // Merge, dedup
    const set = new Set(globalHidden)
    for (const t of ws.filters.tag_blacklist) set.add(t)
    return Array.from(set)
  }, [globalHidden, activeWorkspaceId, workspaces])
}
