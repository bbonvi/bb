import { useEffect, useState, useCallback, type FormEvent } from 'react'
import { useStore } from '@/lib/store'
import { fetchConfig, configureAuth } from '@/lib/api'

type AuthPhase = 'checking' | 'login' | 'ready'

export function AuthGate({ children }: { children: React.ReactNode }) {
  const token = useStore((s) => s.token)
  const setToken = useStore((s) => s.setToken)
  const setConfig = useStore((s) => s.setConfig)

  const [phase, setPhase] = useState<AuthPhase>('checking')
  const [error, setError] = useState<string | null>(null)

  const onUnauthorized = useCallback(() => {
    const hadToken = !!useStore.getState().token
    useStore.getState().setToken(null)
    if (hadToken) {
      window.location.reload()
    }
  }, [])

  // Configure API auth callbacks once
  useEffect(() => {
    configureAuth(() => useStore.getState().token, onUnauthorized)
  }, [onUnauthorized])

  // Probe auth on mount and when token changes
  useEffect(() => {
    let cancelled = false

    async function probe() {
      setPhase('checking')
      setError(null)

      try {
        // Try fetching config with current token (or no token)
        const config = await fetchConfig()
        if (cancelled) return
        setConfig(config)
        setPhase('ready')
      } catch (err: unknown) {
        if (cancelled) return
        // 401 means auth is enabled and token is invalid/missing
        if (err instanceof Error && 'status' in err && (err as { status: number }).status === 401) {
          if (token) {
            // Token was stored but is invalid — clear it
            setToken(null)
          }
          setPhase('login')
        } else {
          // Network or other error — show error but allow retry
          setError(err instanceof Error ? err.message : 'Failed to connect')
          setPhase('login')
        }
      }
    }

    probe()
    return () => { cancelled = true }
  }, [token, setConfig, setToken])

  if (phase === 'checking') {
    return (
      <div className="flex h-screen items-center justify-center bg-bg">
        <div className="text-text-muted animate-pulse text-lg">Loading…</div>
      </div>
    )
  }

  if (phase === 'login') {
    return <LoginForm error={error} onSubmit={setToken} />
  }

  return <>{children}</>
}

function LoginForm({
  error,
  onSubmit,
}: {
  error: string | null
  onSubmit: (token: string) => void
}) {
  const [value, setValue] = useState('')
  const [submitting, setSubmitting] = useState(false)
  const [localError, setLocalError] = useState<string | null>(null)

  async function handleSubmit(e: FormEvent) {
    e.preventDefault()
    const trimmed = value.trim()
    if (!trimmed) return

    setSubmitting(true)
    setLocalError(null)

    // Validate the token by trying to fetch config with it
    try {
      const res = await fetch('/api/config', {
        headers: {
          Authorization: `Bearer ${trimmed}`,
          Accept: 'application/json',
        },
      })
      if (res.status === 401) {
        setLocalError('Invalid token')
        setSubmitting(false)
        return
      }
      if (!res.ok) {
        setLocalError(`Server error: ${res.status}`)
        setSubmitting(false)
        return
      }
      onSubmit(trimmed)
    } catch {
      setLocalError('Could not connect to server')
      setSubmitting(false)
    }
  }

  const displayError = localError || error

  return (
    <div className="flex h-screen items-center justify-center bg-bg">
      <form
        onSubmit={handleSubmit}
        className="flex w-80 flex-col gap-4 rounded-lg border border-border bg-surface p-6"
      >
        <h1 className="text-center text-xl font-semibold text-text">bb</h1>
        <input
          type="password"
          value={value}
          onChange={(e) => setValue(e.target.value)}
          placeholder="Enter token"
          autoFocus
          disabled={submitting}
          className="rounded border border-border bg-bg px-3 py-2 text-text placeholder:text-text-muted focus:border-primary focus:outline-none"
        />
        {displayError && (
          <p className="text-sm text-danger">{displayError}</p>
        )}
        <button
          type="submit"
          disabled={submitting || !value.trim()}
          className="rounded bg-primary px-4 py-2 text-sm font-medium text-primary-foreground hover:bg-primary/90 disabled:opacity-50"
        >
          {submitting ? 'Checking…' : 'Login'}
        </button>
      </form>
    </div>
  )
}
