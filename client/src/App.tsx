import { useEffect, lazy, Suspense } from 'react'
import { AuthGate } from '@/components/AuthGate'
import { Toolbar } from '@/components/Toolbar'
import { BookmarkGrid } from '@/components/BookmarkGrid'
import { BookmarkList } from '@/components/BookmarkList'
import { BookmarkTable } from '@/components/BookmarkTable'
import { usePolling } from '@/hooks/usePolling'
import { useDocumentTitle } from '@/hooks/useDocumentTitle'
import { useStore } from '@/lib/store'
import { useShallow } from 'zustand/react/shallow'

// Lazy load modals — not needed on initial render
const BookmarkDetailModal = lazy(() => import('@/components/BookmarkDetailModal'))
const CreateBookmarkModal = lazy(() => import('@/components/CreateBookmarkModal'))
const BulkOperationsModals = lazy(() =>
  import('@/components/BulkOperationsModal').then((m) => ({
    default: m.BulkOperationsModals,
  })),
)
const SettingsPanel = lazy(() => import('@/components/SettingsPanel'))

function AppShell() {
  usePolling()
  useDocumentTitle()

  // Single subscription for all needed state
  const {
    viewMode,
    bulkEditOpen,
    setBulkEditOpen,
    bulkDeleteOpen,
    setBulkDeleteOpen,
    openCreateModal,
  } = useStore(
    useShallow((s) => ({
      viewMode: s.viewMode,
      bulkEditOpen: s.bulkEditOpen,
      setBulkEditOpen: s.setBulkEditOpen,
      bulkDeleteOpen: s.bulkDeleteOpen,
      setBulkDeleteOpen: s.setBulkDeleteOpen,
      openCreateModal: s.openCreateModal,
    })),
  )

  // Handle ?action=create and share target URL parameters
  useEffect(() => {
    const params = new URLSearchParams(window.location.search)
    const action = params.get('action')
    const sharedUrl = params.get('url')
    const sharedTitle = params.get('title')
    const sharedText = params.get('text')

    if (action === 'create') {
      // Use sharedText as fallback URL if no explicit url param
      const url = sharedUrl || (sharedText?.match(/https?:\/\/\S+/)?.[0] ?? '')
      openCreateModal({
        url,
        title: sharedTitle || '',
        description: params.get('description') || '',
        tags: params.get('tags') || '',
      })

      // Clean URL
      params.delete('action')
      params.delete('url')
      params.delete('title')
      params.delete('text')
      params.delete('description')
      params.delete('tags')
      const newUrl = params.toString()
        ? `${window.location.pathname}?${params}`
        : window.location.pathname
      window.history.replaceState({}, '', newUrl)
    }
  }, [openCreateModal])

  return (
    <div className="flex h-screen flex-col bg-bg">
      <Toolbar />
      <main className="min-h-0 flex-1">
        {viewMode === 'grid' && <BookmarkGrid />}
        {viewMode === 'cards' && <BookmarkList />}
        {viewMode === 'table' && <BookmarkTable />}
      </main>
      {/* Lazy-loaded modals with Suspense — no fallback needed, they render nothing when closed */}
      <Suspense fallback={null}>
        <BookmarkDetailModal />
        <CreateBookmarkModal />
        <BulkOperationsModals
          bulkEditOpen={bulkEditOpen}
          setBulkEditOpen={setBulkEditOpen}
          bulkDeleteOpen={bulkDeleteOpen}
          setBulkDeleteOpen={setBulkDeleteOpen}
        />
        <SettingsPanel />
      </Suspense>
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
