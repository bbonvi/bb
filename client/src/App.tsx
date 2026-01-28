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
  const openCreateWithUrlAndTitle = useStore((s) => s.openCreateWithUrlAndTitle)

  // Handle ?action=create and share target URL parameters
  useEffect(() => {
    const params = new URLSearchParams(window.location.search)
    const action = params.get('action')
    const sharedUrl = params.get('url')
    const sharedTitle = params.get('title')
    const sharedText = params.get('text')

    if (action === 'create' || sharedUrl) {
      // Use sharedText as fallback URL if no explicit url param
      const url = sharedUrl || (sharedText?.match(/https?:\/\/\S+/)?.[0] ?? '')
      openCreateWithUrlAndTitle(url, sharedTitle || '')

      // Clean URL
      params.delete('action')
      params.delete('url')
      params.delete('title')
      params.delete('text')
      const newUrl = params.toString()
        ? `${window.location.pathname}?${params}`
        : window.location.pathname
      window.history.replaceState({}, '', newUrl)
    }
  }, [openCreateWithUrlAndTitle])

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
