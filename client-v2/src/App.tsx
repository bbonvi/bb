import { AuthGate } from '@/components/AuthGate'
import { usePolling } from '@/hooks/usePolling'
import { useStore } from '@/lib/store'

function AppShell() {
  usePolling()
  const initialLoadComplete = useStore((s) => s.initialLoadComplete)
  const totalCount = useStore((s) => s.totalCount)
  const bookmarks = useStore((s) => s.bookmarks)

  if (!initialLoadComplete) {
    return (
      <div className="flex h-screen items-center justify-center bg-bg">
        <div className="text-text-muted animate-pulse text-lg">Loading…</div>
      </div>
    )
  }

  return (
    <div className="flex h-screen flex-col bg-bg">
      <div className="flex items-center justify-center p-4 text-text-muted">
        bb — {bookmarks.length}/{totalCount} bookmarks
      </div>
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
