import {
  CAPTURE_KEY, CONNECTION_KEY, SAVE_MESSAGE, UPDATE_MESSAGE, bookmarkDraft,
  connectionFromFields, findBookmark, getBookmark, hostPermission, httpUrl, loadConnection, testConnection,
  type Capture, type Connection, type Draft, type SavedBookmark, type SaveMessage,
} from './bb'

const element = <T extends HTMLElement>(id: string) => {
  const node = document.getElementById(id)
  if (!node) throw new Error(`Missing popup element: ${id}`)
  return node as T
}

const settingsToggle = element<HTMLButtonElement>('settings-toggle')
const connectionPanel = element<HTMLElement>('connection-panel')
const connectionForm = element<HTMLFormElement>('connection-form')
const connectionFields = element<HTMLFieldSetElement>('connection-fields')
const server = element<HTMLInputElement>('server')
const token = element<HTMLInputElement>('token')
const connectButton = element<HTMLButtonElement>('connect')
const connectionStatus = element<HTMLParagraphElement>('connection-status')
const bookmarkForm = element<HTMLFormElement>('bookmark-form')
const bookmarkFields = element<HTMLFieldSetElement>('bookmark-fields')
const heading = element<HTMLHeadingElement>('bookmark-heading')
const editorLink = element<HTMLAnchorElement>('editor-link')
const url = element<HTMLTextAreaElement>('url')
const title = element<HTMLInputElement>('title')
const description = element<HTMLTextAreaElement>('description')
const tags = element<HTMLInputElement>('tags')
const saveLabel = element<HTMLSpanElement>('save-label')
const saveButton = element<HTMLButtonElement>('save')
const saveStatus = element<HTMLParagraphElement>('save-status')
const serverLabel = element<HTMLSpanElement>('server-label')
const openBB = element<HTMLAnchorElement>('open-bb')

type Editor = { kind: 'create' | 'loading' }
  | { kind: 'unavailable'; targetId: number | undefined }
  | { kind: 'edit'; bookmark: SavedBookmark }
let editor: Editor = { kind: 'loading' }
let connection: Connection | null = null
let tabId: number | undefined
let busy = false

function showStatus(node: HTMLElement, message: string) {
  node.textContent = message
  node.hidden = !message
}

function showSettings(show: boolean) {
  connectionPanel.hidden = !show
  bookmarkForm.hidden = show
  settingsToggle.setAttribute('aria-expanded', String(show))
  if (show) server.focus()
}

function fields(): Draft {
  return { url: url.value, title: title.value, description: description.value, tags: tags.value }
}

function fillDraft(draft: Draft) {
  url.value = draft.url
  title.value = draft.title
  description.value = draft.description
  tags.value = draft.tags
}

function hasChanges() {
  if (editor.kind !== 'edit') return true
  const original = bookmarkDraft(editor.bookmark)
  const current = fields()
  return (Object.keys(original) as (keyof Draft)[]).some((key) => original[key] !== current[key])
}

function renderEditor() {
  const editing = editor.kind === 'edit'
  heading.textContent = editing ? 'Edit bookmark' : 'Send to BB'
  editorLink.hidden = !editing
  if (editor.kind === 'edit' && connection) {
    const target = new URL(`${connection.server}/`)
    target.searchParams.set('query', `=${editor.bookmark.id}`)
    editorLink.href = target.href
  }
  bookmarkFields.disabled = !connection || busy || editor.kind === 'loading'
  saveButton.disabled = editing && !hasChanges()
  saveLabel.textContent = busy ? 'Saving…'
    : editor.kind === 'loading' ? 'Checking BB…'
    : editor.kind === 'unavailable' ? 'Retry lookup'
    : editing ? 'Save changes' : 'Save bookmark'
}

function renderConnection() {
  server.value = connection?.server ?? 'http://localhost:8080'
  token.value = connection?.token ?? ''
  serverLabel.textContent = connection ? connection.server.replace(/^https?:\/\//, '') : 'Not connected'
  serverLabel.title = connection?.server ?? ''
  openBB.href = connection?.server ?? '#'
  renderEditor()
}

async function loadEditor(targetId?: number) {
  if (!connection) return
  if (!url.value) {
    editor = { kind: 'create' }
    renderEditor()
    showStatus(saveStatus, 'This page cannot be saved. Paste an http:// or https:// URL.')
    return
  }
  editor = { kind: 'loading' }
  const draft = fields()
  renderEditor()
  showStatus(saveStatus, '')
  try {
    const bookmark = targetId === undefined
      ? await findBookmark(connection, url.value)
      : await getBookmark(connection, targetId)
    if (targetId !== undefined && !bookmark) throw new Error('This bookmark no longer exists in BB.')
    editor = bookmark ? { kind: 'edit', bookmark } : { kind: 'create' }
    if (bookmark) fillDraft(targetId === undefined ? bookmarkDraft(bookmark) : draft)
  } catch (error) {
    editor = { kind: 'unavailable', targetId }
    showStatus(saveStatus, error instanceof Error ? error.message : 'Could not check this URL in BB.')
  }
  renderEditor()
}

function applyCapture(capture: Capture) {
  busy = capture.status === 'saving'
  if (capture.status === 'saved') {
    editor = { kind: 'edit', bookmark: capture.bookmark }
    fillDraft(bookmarkDraft(capture.bookmark))
  }
  if (capture.status === 'error' || capture.status === 'needs-connection') showStatus(saveStatus, capture.message)
  else showStatus(saveStatus, '')
  if (capture.status === 'needs-connection') showSettings(true)
  renderEditor()
}

settingsToggle.addEventListener('click', () => showSettings(connectionPanel.hidden))
openBB.addEventListener('click', (event) => {
  event.preventDefault()
  if (connection) void chrome.tabs.create({ url: connection.server })
  else showSettings(true)
})
editorLink.addEventListener('click', (event) => {
  event.preventDefault()
  void chrome.tabs.create({ url: editorLink.href })
})

connectionForm.addEventListener('submit', async (event) => {
  event.preventDefault()
  connectionFields.disabled = true
  connectButton.textContent = 'Connecting…'
  showStatus(connectionStatus, '')
  try {
    const next = connectionFromFields(server.value, token.value)
    // Request in the submit gesture; awaiting first loses the gesture in Chrome.
    const granted = await chrome.permissions.request({ origins: [hostPermission(next.server)] })
    if (!granted) throw new Error('Allow access to your BB server to connect.')
    await testConnection(next)
    await chrome.storage.local.set({ [CONNECTION_KEY]: next })
    connection = next
    renderConnection()
    showSettings(false)
    await loadEditor()
  } catch (error) {
    showStatus(connectionStatus, error instanceof Error ? error.message : 'Could not connect to BB.')
  } finally {
    connectionFields.disabled = false
    connectButton.textContent = 'Connect'
  }
})

bookmarkForm.addEventListener('submit', async (event) => {
  event.preventDefault()
  if (busy || bookmarkFields.disabled || saveButton.disabled || !connection) return
  if (editor.kind === 'unavailable') {
    await loadEditor(editor.targetId)
    return
  }
  try {
    const draft = { ...fields(), url: httpUrl(url.value) }
    const message: SaveMessage = editor.kind === 'edit'
      ? { type: UPDATE_MESSAGE, id: editor.bookmark.id, draft, tabId }
      : { type: SAVE_MESSAGE, draft, tabId }
    applyCapture({ ...message, status: 'saving' })
    const result: Capture = await chrome.runtime.sendMessage(message)
    if (busy) applyCapture(result)
  } catch (error) {
    busy = false
    renderEditor()
    showStatus(saveStatus, error instanceof Error ? error.message : 'Could not save to BB.')
  }
})

url.addEventListener('input', () => {
  if (editor.kind === 'create') title.value = ''
})
bookmarkForm.addEventListener('input', () => {
  renderEditor()
  showStatus(saveStatus, '')
})

document.addEventListener('keydown', (event) => {
  if ((event.metaKey || event.ctrlKey) && event.key === 'Enter') {
    event.preventDefault()
    if (!connectionPanel.hidden && !connectionFields.disabled) connectionForm.requestSubmit()
    else if (!bookmarkFields.disabled && !saveButton.disabled) bookmarkForm.requestSubmit()
  }
})

chrome.storage.onChanged.addListener((changes, area) => {
  const capture = changes[CAPTURE_KEY]?.newValue as Capture | undefined
  if (area !== 'session' || !capture || capture.tabId !== tabId || capture.draft.url !== url.value.trim()) return
  // A quick-save running elsewhere must not replace unsaved edits in this popup.
  if (!busy && editor.kind === 'edit' && hasChanges()) return
  applyCapture(capture)
})

async function initialize() {
  const [tab] = await chrome.tabs.query({ active: true, currentWindow: true })
  tabId = tab?.id
  if (tab?.url?.startsWith('http://') || tab?.url?.startsWith('https://')) {
    url.value = tab.url
    title.value = tab.title ?? ''
  }

  const values = await chrome.storage.session.get<{ [CAPTURE_KEY]?: Capture }>(CAPTURE_KEY)
  const capture = values[CAPTURE_KEY]
  const pending = capture && capture.tabId === tabId && capture.status !== 'saved' ? capture : null
  if (pending) fillDraft(pending.draft)
  connection = await loadConnection()
  renderConnection()
  const permitted = connection && await chrome.permissions.contains({ origins: [hostPermission(connection.server)] })
  showSettings(!permitted)
  if (permitted) await loadEditor(pending?.type === UPDATE_MESSAGE ? pending.id : undefined)
  if (pending) {
    const latest = (await chrome.storage.session.get<{ [CAPTURE_KEY]?: Capture }>(CAPTURE_KEY))[CAPTURE_KEY]
    if (latest && latest.tabId === tabId && latest.draft.url === pending.draft.url) applyCapture(latest)
  }
  if (!navigator.platform.toLowerCase().includes('mac')) element('shortcut').textContent = 'Ctrl ↵'
}

void initialize().catch((error: unknown) => {
  renderConnection()
  showSettings(true)
  showStatus(connectionStatus, error instanceof Error ? error.message : 'Could not load extension settings.')
})
