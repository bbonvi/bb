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
  status: number
  code: string

  constructor(status: number, code: string, message: string) {
    super(message)
    this.name = 'ApiError'
    this.status = status
    this.code = code
  }
}

// --- Base64 file conversion ---
// Reads a File as base64, strips the data: prefix per §13.3
export function toBase64(file: File): Promise<string> {
  return new Promise((resolve, reject) => {
    const reader = new FileReader()
    reader.onload = () => {
      const result = reader.result as string
      const idx = result.indexOf(',')
      resolve(idx >= 0 ? result.slice(idx + 1) : result)
    }
    reader.onerror = () => reject(new Error('Failed to read file'))
    reader.readAsDataURL(file)
  })
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

// --- ETag conditional fetch for search endpoint ---

const etagCache = new Map<string, string>()
const responseCache = new Map<string, unknown>()

async function fetchApiWithEtag<T>(
  path: string,
  cacheKey: string,
  options: { method?: string; body?: unknown; signal?: AbortSignal } = {},
): Promise<T> {
  const headers: Record<string, string> = {
    'Content-Type': 'application/json',
    Accept: 'application/json',
  }

  const token = _getToken()
  if (token) {
    headers['Authorization'] = `Bearer ${token}`
  }

  const cachedEtag = etagCache.get(cacheKey)
  if (cachedEtag) {
    headers['If-None-Match'] = cachedEtag
  }

  const res = await fetch(path, {
    method: options.method ?? 'GET',
    headers,
    body: options.body !== undefined ? JSON.stringify(options.body) : undefined,
    signal: options.signal,
  })

  if (res.status === 401) {
    _onUnauthorized()
    throw new ApiError(401, 'Unauthorized', 'Unauthorized')
  }

  if (res.status === 304) {
    const cached = responseCache.get(cacheKey)
    if (cached !== undefined) return cached as T
    // Fallback: cache miss after 304 — clear and refetch without ETag
    etagCache.delete(cacheKey)
    return fetchApiWithEtag(path, cacheKey, options)
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

  const text = await res.text()
  if (!text) return undefined as T
  const data = JSON.parse(text) as T

  // Store ETag and response
  const newEtag = res.headers.get('etag')
  if (newEtag) {
    etagCache.set(cacheKey, newEtag)
    responseCache.set(cacheKey, data)
  }

  return data
}

export function clearEtagCache(prefix?: string) {
  if (prefix) {
    for (const key of etagCache.keys()) {
      if (key.startsWith(prefix)) {
        etagCache.delete(key)
        responseCache.delete(key)
      }
    }
  } else {
    etagCache.clear()
    responseCache.clear()
  }
}

// --- File URL helper (for <img src>) ---

export function fileUrl(fileId: string): string {
  const token = _getToken()
  const base = `/api/file/${fileId}`
  return token ? `${base}?token=${encodeURIComponent(token)}` : base
}

// --- Endpoint functions ---

export function searchBookmarks(query: SearchQuery = {}, signal?: AbortSignal): Promise<Bookmark[]> {
  return fetchApiWithEtag('/api/bookmarks/search', 'bookmarks-search', { method: 'POST', body: query, signal })
}

export function searchBookmarksUncached(query: SearchQuery = {}): Promise<Bookmark[]> {
  return fetchApi('/api/bookmarks/search', { method: 'POST', body: query })
}

export async function createBookmark(data: BookmarkCreate): Promise<Bookmark> {
  const result = await fetchApi<Bookmark>('/api/bookmarks/create', { method: 'POST', body: data })
  clearEtagCache('bookmarks')
  return result
}

export async function updateBookmark(data: BookmarkUpdate): Promise<Bookmark> {
  const result = await fetchApi<Bookmark>('/api/bookmarks/update', { method: 'POST', body: data })
  clearEtagCache('bookmarks')
  return result
}

export async function deleteBookmark(id: number): Promise<void> {
  await fetchApi<void>('/api/bookmarks/delete', { method: 'POST', body: { id } })
  clearEtagCache('bookmarks')
}

export async function searchUpdateBookmarks(
  query: BulkSearchQuery,
  update: BulkUpdate,
): Promise<number> {
  const result = await fetchApi<number>('/api/bookmarks/search_update', {
    method: 'POST',
    body: { query, update },
  })
  clearEtagCache('bookmarks')
  return result
}

export async function searchDeleteBookmarks(query: BulkSearchQuery): Promise<number> {
  const result = await fetchApi<number>('/api/bookmarks/search_delete', {
    method: 'POST',
    body: query,
  })
  clearEtagCache('bookmarks')
  return result
}

export function fetchTotal(): Promise<{ total: number }> {
  return fetchApi('/api/bookmarks/total', { method: 'POST', body: {} })
}

export function fetchTags(): Promise<string[]> {
  return fetchApi('/api/bookmarks/tags', { method: 'POST', body: {} })
}

export async function refreshMetadata(
  id: number,
  opts?: { async_meta?: boolean; no_headless?: boolean },
): Promise<void> {
  await fetchApi<void>('/api/bookmarks/refresh_metadata', {
    method: 'POST',
    body: { id, ...opts },
  })
  clearEtagCache('bookmarks')
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

// --- Workspace types ---

export interface WorkspaceFilters {
  tag_whitelist: string[]
  tag_blacklist: string[]
  keyword: string | null
}

export interface ViewPrefs {
  mode: 'grid' | 'cards' | 'table' | null
  columns: number | null
}

export interface Workspace {
  id: string
  name: string
  filters: WorkspaceFilters
  view_prefs: ViewPrefs
}

export interface WorkspaceCreate {
  name: string
  filters?: Partial<WorkspaceFilters>
  view_prefs?: Partial<ViewPrefs>
}

export interface WorkspaceUpdate {
  name?: string
  filters?: Partial<WorkspaceFilters>
  view_prefs?: Partial<ViewPrefs>
}

// --- Workspace endpoints ---

export function fetchWorkspaces(): Promise<Workspace[]> {
  return fetchApi('/api/workspaces')
}

export function createWorkspace(data: WorkspaceCreate): Promise<Workspace> {
  return fetchApi('/api/workspaces', { method: 'POST', body: data })
}

export function updateWorkspace(id: string, data: WorkspaceUpdate): Promise<Workspace> {
  return fetchApi(`/api/workspaces/${encodeURIComponent(id)}`, { method: 'PUT', body: data })
}

export function deleteWorkspace(id: string): Promise<void> {
  return fetchApi(`/api/workspaces/${encodeURIComponent(id)}`, { method: 'DELETE' })
}

export function reorderWorkspaces(ids: string[]): Promise<void> {
  return fetchApi('/api/workspaces/reorder', { method: 'POST', body: { ids } })
}
