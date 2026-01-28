import { useEffect } from 'react'
import { useStore } from '@/lib/store'

/**
 * Updates document.title to reflect current workspace:
 * "bb - {workspace}"
 */
export function useDocumentTitle() {
  const workspaces = useStore((s) => s.workspaces)
  const activeWorkspaceId = useStore((s) => s.activeWorkspaceId)
  const workspacesAvailable = useStore((s) => s.workspacesAvailable)

  useEffect(() => {
    const activeWorkspace = workspaces.find((w) => w.id === activeWorkspaceId)
    const workspaceName =
      workspacesAvailable && activeWorkspace ? activeWorkspace.name : 'All'

    document.title = `bb - ${workspaceName}`
  }, [workspaces, activeWorkspaceId, workspacesAvailable])
}
