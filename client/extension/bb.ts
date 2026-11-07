import { normalizeTags, type Bookmark, type BookmarkCreate, type BookmarkUpdate, type SearchQuery } from '../src/lib/api'

export const CONNECTION_KEY = 'connection'
export const CAPTURE_KEY = 'capture'
export const SAVE_MESSAGE = 'save-bookmark'
export const UPDATE_MESSAGE = 'update-bookmark'
export const MENU_LINK = 'send-link'
export const MENU_PAGE = 'send-page'

export interface Connection {
  server: string
  token: string
}

export interface Draft {
  url: string
  title: string
  tags: string
  description: string
}

export type SavedBookmark = Pick<Bookmark, 'id' | 'url' | 'title' | 'description' | 'tags'>

export type Capture = SaveMessage & (
  | { status: 'saving' }
  | { status: 'saved'; bookmark: SavedBookmark; operation: 'created' | 'updated' | 'existing' }
  | { status: 'error' | 'needs-connection'; message: string }
)

export type SaveMessage = {
  draft: Draft
  tabId: number | undefined
} & ({ type: typeof SAVE_MESSAGE } | { type: typeof UPDATE_MESSAGE; id: number })

export function isSaveMessage(value: unknown): value is SaveMessage {
  if (!value || typeof value !== 'object') return false
  const message = value as Partial<SaveMessage>
  return (message.type === SAVE_MESSAGE
    || (message.type === UPDATE_MESSAGE && typeof message.id === 'number' && Number.isSafeInteger(message.id)))
    && (message.tabId === undefined || Number.isInteger(message.tabId))
    && !!message.draft
    && typeof message.draft.url === 'string'
    && typeof message.draft.title === 'string'
    && typeof message.draft.tags === 'string'
    && typeof message.draft.description === 'string'
}

export function httpUrl(raw: string): string {
  let url: URL
  try {
    url = new URL(raw.trim())
  } catch {
    throw new Error('Enter a full URL starting with http:// or https://.')
  }
  if (url.protocol !== 'http:' && url.protocol !== 'https:') {
    throw new Error('Only http:// and https:// URLs can be saved.')
  }
  if (url.username || url.password) {
    throw new Error('Remove the username and password from the URL.')
  }
  return url.href
}

export function connectionFromFields(server: string, token: string): Connection {
  const url = new URL(httpUrl(server))
  if (url.search || url.hash) {
    throw new Error('Use the BB server address without a query or fragment.')
  }
  return { server: url.href.replace(/\/+$/, ''), token: token.trim() }
}

export function hostPermission(server: string): string {
  const url = new URL(server)
  // Chrome host permissions cover every port on the chosen host.
  return `${url.protocol}//${url.hostname}/*`
}

export async function loadConnection(): Promise<Connection | null> {
  const values = await chrome.storage.local.get(CONNECTION_KEY)
  const value: unknown = values[CONNECTION_KEY]
  if (value === undefined) return null
  if (!value || typeof value !== 'object'
    || !('server' in value) || typeof value.server !== 'string'
    || !('token' in value) || typeof value.token !== 'string') {
    throw new Error('Open connection settings and reconnect to BB.')
  }
  return connectionFromFields(value.server, value.token)
}

export function draftFromContext(
  info: Pick<chrome.contextMenus.OnClickData, 'menuItemId' | 'linkUrl' | 'pageUrl'>,
  tab?: Pick<chrome.tabs.Tab, 'url' | 'title'>,
): Draft {
  switch (info.menuItemId) {
    case MENU_LINK:
      // The source page's title belongs to the page, never to its outgoing link.
      return { url: info.linkUrl ?? '', title: '', tags: '', description: '' }
    case MENU_PAGE:
      return { url: tab?.url ?? info.pageUrl ?? '', title: tab?.title ?? '', tags: '', description: '' }
    default:
      throw new Error('Unknown BB menu action.')
  }
}

export class BBError extends Error {
  readonly status: number

  constructor(status: number, message: string) {
    super(message)
    this.name = 'BBError'
    this.status = status
  }
}

async function request(connection: Connection, path: string, body?: BookmarkCreate | BookmarkUpdate | SearchQuery): Promise<unknown> {
  let response: Response
  try {
    response = await fetch(`${connection.server}/${path}`, {
      method: body ? 'POST' : 'GET',
      headers: {
        Accept: 'application/json',
        ...(body ? { 'Content-Type': 'application/json' } : {}),
        ...(connection.token ? { Authorization: `Bearer ${connection.token}` } : {}),
      },
      body: body ? JSON.stringify(body) : undefined,
      credentials: 'omit',
      redirect: 'error',
      signal: AbortSignal.timeout(15000),
    })
  } catch (error) {
    if (error instanceof DOMException && error.name === 'TimeoutError') {
      throw new Error('BB took too long to respond. Check BB before retrying.')
    }
    throw new Error('Could not reach BB. Check the server address and connection.')
  }

  if (response.status === 401 || response.status === 403) {
    throw new BBError(response.status, 'BB rejected the token. Update it in connection settings.')
  }
  if (response.status === 409) {
    throw new BBError(response.status, 'This URL is already in BB.')
  }
  if (!response.ok) {
    throw new Error(`BB returned HTTP ${response.status}. Try again when the server is ready.`)
  }
  try {
    return await response.json()
  } catch {
    throw new Error('The server did not return a BB response. Check the server address.')
  }
}

export async function testConnection(connection: Connection): Promise<void> {
  const config = await request(connection, 'api/config')
  if (!config || typeof config !== 'object'
    || !('task_queue_max_threads' in config) || typeof config.task_queue_max_threads !== 'number') {
    throw new Error('This address does not point to a BB server.')
  }
}

function parseBookmark(result: unknown): SavedBookmark {
  if (!result || typeof result !== 'object'
    || !('id' in result) || typeof result.id !== 'number' || !Number.isSafeInteger(result.id)
    || !('url' in result) || typeof result.url !== 'string'
    || !('title' in result) || typeof result.title !== 'string'
    || !('description' in result) || typeof result.description !== 'string'
    || !('tags' in result) || !Array.isArray(result.tags) || !result.tags.every((tag: unknown) => typeof tag === 'string')) {
    throw new Error('BB returned an invalid bookmark. Check BB before retrying.')
  }
  return { id: result.id, url: result.url, title: result.title, description: result.description, tags: result.tags }
}

export function bookmarkDraft(bookmark: SavedBookmark): Draft {
  return { url: bookmark.url, title: bookmark.title, description: bookmark.description, tags: bookmark.tags.join(', ') }
}

async function searchBookmark(connection: Connection, query: SearchQuery): Promise<SavedBookmark | null> {
  const results = await request(connection, 'api/bookmarks/search', { ...query, limit: 1 })
  if (!Array.isArray(results)) throw new Error('BB returned an invalid search response.')
  return results.length > 0 ? parseBookmark(results[0]) : null
}

export function getBookmark(connection: Connection, id: number): Promise<SavedBookmark | null> {
  return searchBookmark(connection, { id })
}

export async function findBookmark(connection: Connection, url: string): Promise<SavedBookmark | null> {
  const normalized = httpUrl(url)
  const parsed = new URL(normalized)
  const candidates = [normalized]
  // CLI bookmarks can store a bare origin while browsers always include its root slash.
  if (parsed.pathname === '/' && !parsed.search && !parsed.hash) candidates.push(normalized.slice(0, -1))
  for (const candidate of candidates) {
    const bookmark = await searchBookmark(connection, { url: candidate, exact: true })
    if (bookmark) return bookmark
  }
  return null
}

export async function createBookmark(connection: Connection, draft: Draft): Promise<SavedBookmark> {
  const result = await request(connection, 'api/bookmarks/create', {
    url: httpUrl(draft.url),
    title: draft.title.trim() || undefined,
    tags: normalizeTags(draft.tags) || undefined,
    description: draft.description.trim() || undefined,
    async_meta: true,
  })
  return parseBookmark(result)
}

export async function updateBookmark(connection: Connection, id: number, draft: Draft): Promise<SavedBookmark> {
  return parseBookmark(await request(connection, 'api/bookmarks/update', {
    id,
    url: httpUrl(draft.url),
    title: draft.title,
    description: draft.description,
    tags: normalizeTags(draft.tags),
  }))
}
