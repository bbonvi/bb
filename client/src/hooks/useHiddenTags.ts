import { useMemo } from 'react'
import { useStore } from '@/lib/store'

/**
 * Returns the merged set of hidden tags: global `hidden_by_default` from config
 * plus the active workspace's blacklist tags.
 * §6.5: visible tags = all_tags - global_hidden - workspace_blacklist
 */
export function useHiddenTags(): string[] {
  const config = useStore((s) => s.config)
  const activeWorkspaceId = useStore((s) => s.activeWorkspaceId)
  const workspaces = useStore((s) => s.workspaces)

  return useMemo(() => {
    const globalHidden = config?.hidden_by_default ?? []

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
  }, [config?.hidden_by_default, activeWorkspaceId, workspaces])
}
