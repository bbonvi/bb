import { AuthGate } from '@/components/AuthGate'
import { Toolbar } from '@/components/Toolbar'
import { BookmarkGrid } from '@/components/BookmarkGrid'
import { BookmarkList } from '@/components/BookmarkList'
import { BookmarkTable } from '@/components/BookmarkTable'
import { usePolling } from '@/hooks/usePolling'
import { useStore } from '@/lib/store'

function AppShell() {
  usePolling()
  const initialLoadComplete = useStore((s) => s.initialLoadComplete)
  const viewMode = useStore((s) => s.viewMode)

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
