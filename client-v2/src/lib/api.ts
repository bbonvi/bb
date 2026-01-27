// API client for bb backend
// All endpoints typed per specs §13

// --- Types ---

export interface Bookmark {
  id: number
  title: string
  description: string
  tags: string[]
  url: string
  image_id: string | null
  icon_id: string | null
}

export interface SearchQuery {
  id?: number
  url?: string
  title?: string
  description?: string
  tags?: string       // comma-separated for search endpoint
  keyword?: string
  semantic?: string
  threshold?: number
  exact?: boolean
  limit?: number
  offset?: number
}

export interface BookmarkCreate {
  url: string
  title?: string
  description?: string
  tags?: string       // comma-separated
  async_meta?: boolean
  no_meta?: boolean
  no_headless?: boolean
}

export interface BookmarkUpdate {
  id: number
  title?: string
  description?: string
  tags?: string       // comma-separated (replaces all)
  append_tags?: string
  remove_tags?: string
  url?: string
  image_b64?: string  // raw base64, no data: prefix
  icon_b64?: string
}

// search_update uses domain structs directly — tags are JSON arrays, not comma-separated
export interface BulkSearchQuery {
  id?: number
  url?: string
  title?: string
  description?: string
  tags?: string[]     // JSON array
  keyword?: string
  semantic?: string
  threshold?: number
  exact?: boolean
  limit?: number
}

export interface BulkUpdate {
  title?: string
  description?: string
  tags?: string[]        // replace all (JSON array)
  append_tags?: string[] // append (JSON array)
  remove_tags?: string[] // remove (JSON array)
  url?: string
}

export interface Config {
  task_queue_max_threads: number
  hidden_by_default: string[]
  rules: Rule[]
  semantic_search?: {
    enabled: boolean
    model: string
    default_threshold: number
    embedding_parallelism: string
    download_timeout_secs: number
    semantic_weight: number
  }
}

export interface Rule {
  url?: string
  title?: string
  description?: string
  tags?: string[]
  comment?: string
  action: { update: { title?: string; description?: string; tags?: string[] } }
}

export interface TaskDump {
  id: string
  task: {
    FetchMetadata?: {
      bmark_id: number
      opts: { no_https_upgrade: boolean; meta_opts: { no_headless: boolean } }
    }
    Shutdown?: Record<string, never>
  }
  status: 'Interrupted' | 'Pending' | 'InProgress' | 'Done' | { Error: string }
}

export interface TaskQueue {
  queue: TaskDump[]
  now: number
}

export interface SemanticStatus {
  enabled: boolean
  model: string
  indexed_count: number
  total_bookmarks: number
}

// --- API Error ---

export class ApiError extends Error {
  constructor(
    public status: number,
    public code: string,
    message: string,
  ) {
    super(message)
    this.name = 'ApiError'
  }
}

// --- Tag normalization ---
// Accepts comma-separated, space-separated, or mixed. Returns comma-separated, no empty entries.
export function normalizeTags(raw: string): string {
  return raw
    .split(/[\s,]+/)
    .map((t) => t.trim())
    .filter(Boolean)
    .join(',')
}

// --- Token management (injected via callbacks) ---

let _getToken: () => string | null = () => null
let _onUnauthorized: () => void = () => {}

export function configureAuth(
  getToken: () => string | null,
  onUnauthorized: () => void,
) {
  _getToken = getToken
  _onUnauthorized = onUnauthorized
}

// --- Core fetch helper ---

async function fetchApi<T>(
  path: string,
  options: { method?: string; body?: unknown } = {},
): Promise<T> {
  const headers: Record<string, string> = {
    'Content-Type': 'application/json',
    Accept: 'application/json',
  }

  const token = _getToken()
  if (token) {
    headers['Authorization'] = `Bearer ${token}`
  }

  const res = await fetch(path, {
    method: options.method ?? 'GET',
    headers,
    body: options.body !== undefined ? JSON.stringify(options.body) : undefined,
  })

  if (res.status === 401) {
    _onUnauthorized()
    throw new ApiError(401, 'Unauthorized', 'Unauthorized')
  }

  if (!res.ok) {
    let code = 'UNKNOWN'
    let message = res.statusText
    try {
      const body = await res.json()
      if (body.error) code = body.error
      if (body.message) message = body.message
    } catch {
      // non-JSON error body
    }
    throw new ApiError(res.status, code, message)
  }

  // Some endpoints return empty body (delete, refresh_metadata)
  const text = await res.text()
  if (!text) return undefined as T
  return JSON.parse(text) as T
}

// --- File URL helper (for <img src>) ---

export function fileUrl(fileId: string): string {
  const token = _getToken()
  const base = `/api/file/${fileId}`
  return token ? `${base}?token=${encodeURIComponent(token)}` : base
}

// --- Endpoint functions ---

export function searchBookmarks(query: SearchQuery = {}): Promise<Bookmark[]> {
  return fetchApi('/api/bookmarks/search', { method: 'POST', body: query })
}

export function createBookmark(data: BookmarkCreate): Promise<Bookmark> {
  return fetchApi('/api/bookmarks/create', { method: 'POST', body: data })
}

export function updateBookmark(data: BookmarkUpdate): Promise<Bookmark> {
  return fetchApi('/api/bookmarks/update', { method: 'POST', body: data })
}

export function deleteBookmark(id: number): Promise<void> {
  return fetchApi('/api/bookmarks/delete', { method: 'POST', body: { id } })
}

export function searchUpdateBookmarks(
  query: BulkSearchQuery,
  update: BulkUpdate,
): Promise<number> {
  return fetchApi('/api/bookmarks/search_update', {
    method: 'POST',
    body: { query, update },
  })
}

export function searchDeleteBookmarks(query: BulkSearchQuery): Promise<number> {
  return fetchApi('/api/bookmarks/search_delete', {
    method: 'POST',
    body: query,
  })
}

export function fetchTotal(): Promise<{ total: number }> {
  return fetchApi('/api/bookmarks/total', { method: 'POST', body: {} })
}

export function fetchTags(): Promise<string[]> {
  return fetchApi('/api/bookmarks/tags', { method: 'POST', body: {} })
}

export function refreshMetadata(
  id: number,
  opts?: { async_meta?: boolean; no_headless?: boolean },
): Promise<void> {
  return fetchApi('/api/bookmarks/refresh_metadata', {
    method: 'POST',
    body: { id, ...opts },
  })
}

export function fetchConfig(): Promise<Config> {
  return fetchApi('/api/config')
}

export function fetchTaskQueue(): Promise<TaskQueue> {
  return fetchApi('/api/task_queue')
}

export function fetchSemanticStatus(): Promise<SemanticStatus> {
  return fetchApi('/api/semantic/status')
}
