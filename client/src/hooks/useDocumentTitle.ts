import { useEffect } from 'react'
import { useStore } from '@/lib/store'

/**
 * Updates document.title to reflect current state:
 * "bb - {workspace} - {visible}/{total}"
 */
export function useDocumentTitle() {
  const bookmarks = useStore((s) => s.bookmarks)
  const totalCount = useStore((s) => s.totalCount)
  const workspaces = useStore((s) => s.workspaces)
  const activeWorkspaceId = useStore((s) => s.activeWorkspaceId)
  const workspacesAvailable = useStore((s) => s.workspacesAvailable)

  useEffect(() => {
    const visible = bookmarks.length
    const activeWorkspace = workspaces.find((w) => w.id === activeWorkspaceId)
    const workspaceName =
      workspacesAvailable && activeWorkspace ? activeWorkspace.name : 'All'

    document.title = `bb - ${workspaceName} - ${visible}/${totalCount}`
  }, [bookmarks.length, totalCount, workspaces, activeWorkspaceId, workspacesAvailable])
}
