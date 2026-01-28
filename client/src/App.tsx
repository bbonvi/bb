import { useEffect } from 'react'
import { AuthGate } from '@/components/AuthGate'
import { Toolbar } from '@/components/Toolbar'
import { BookmarkGrid } from '@/components/BookmarkGrid'
import { BookmarkList } from '@/components/BookmarkList'
import { BookmarkTable } from '@/components/BookmarkTable'
import { BookmarkDetailModal } from '@/components/BookmarkDetailModal'
import { CreateBookmarkModal } from '@/components/CreateBookmarkModal'
import { BulkEditModal, BulkDeleteModal } from '@/components/BulkOperationsModal'
import { SettingsPanel } from '@/components/SettingsPanel'
import { usePolling } from '@/hooks/usePolling'
import { useDocumentTitle } from '@/hooks/useDocumentTitle'
import { useStore } from '@/lib/store'

function AppShell() {
  usePolling()
  useDocumentTitle()
  const initialLoadComplete = useStore((s) => s.initialLoadComplete)
  const viewMode = useStore((s) => s.viewMode)
  const bulkEditOpen = useStore((s) => s.bulkEditOpen)
  const setBulkEditOpen = useStore((s) => s.setBulkEditOpen)
  const bulkDeleteOpen = useStore((s) => s.bulkDeleteOpen)
  const setBulkDeleteOpen = useStore((s) => s.setBulkDeleteOpen)
  const setCreateModalOpen = useStore((s) => s.setCreateModalOpen)

  // Handle ?action=create URL parameter
  useEffect(() => {
    const params = new URLSearchParams(window.location.search)
    if (params.get('action') === 'create') {
      setCreateModalOpen(true)
      // Clean up URL
      params.delete('action')
      const newUrl = params.toString()
        ? `${window.location.pathname}?${params}`
        : window.location.pathname
      window.history.replaceState({}, '', newUrl)
    }
  }, [setCreateModalOpen])

  if (!initialLoadComplete) {
    return (
      <div className="flex h-screen items-center justify-center bg-bg">
        <div className="text-text-muted animate-pulse text-lg">Loading…</div>
      </div>
    )
  }

  return (
    <div className="flex h-screen flex-col bg-bg">
      <Toolbar />
      <main className="min-h-0 flex-1">
        {viewMode === 'grid' && <BookmarkGrid />}
        {viewMode === 'cards' && <BookmarkList />}
        {viewMode === 'table' && <BookmarkTable />}
      </main>
      <BookmarkDetailModal />
      <CreateBookmarkModal />
      <BulkEditModal open={bulkEditOpen} onOpenChange={setBulkEditOpen} />
      <BulkDeleteModal open={bulkDeleteOpen} onOpenChange={setBulkDeleteOpen} />
      <SettingsPanel />
    </div>
  )
}

export default function App() {
  return (
    <AuthGate>
      <AppShell />
    </AuthGate>
  )
}
