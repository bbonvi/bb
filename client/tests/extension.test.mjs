import assert from 'node:assert/strict'
import { readFile } from 'node:fs/promises'
import test from 'node:test'
import ts from 'typescript'

async function moduleUrl(path, replacements = {}) {
  let source = await readFile(new URL(path, import.meta.url), 'utf8')
  for (const [from, to] of Object.entries(replacements)) source = source.replace(`'${from}'`, `'${to}'`)
  const { outputText } = ts.transpileModule(source, {
    compilerOptions: { module: ts.ModuleKind.ESNext, target: ts.ScriptTarget.ES2022 },
  })
  return `data:text/javascript;base64,${Buffer.from(outputText).toString('base64')}`
}

const apiModule = await moduleUrl('../src/lib/api.ts')
const bbModule = await moduleUrl('../extension/bb.ts', { '../src/lib/api': apiModule })
const feedbackModule = await moduleUrl('../extension/feedback.ts')
const backgroundModule = await moduleUrl('../extension/background.ts', { './bb': bbModule, './feedback': feedbackModule })
const bb = await import(bbModule)
const manifest = JSON.parse(await readFile(new URL('../extension/manifest.json', import.meta.url), 'utf8'))

function event() {
  const listeners = []
  return { addListener: (listener) => listeners.push(listener), listeners }
}

function deferred() {
  let resolve
  const promise = new Promise((done) => { resolve = done })
  return { promise, resolve }
}

function chromeHarness(connection = { server: 'https://bb.example', token: 'test-token' }, permitted = true) {
  const menus = []
  const badges = []
  const titles = []
  const notifications = []
  const popups = []
  const captures = []
  const terminal = deferred()
  const chrome = {
    storage: {
      local: { get: async () => connection ? { connection } : {} },
      session: {
        set: async ({ capture }) => {
          captures.push(capture)
          if (capture.status !== 'saving') terminal.resolve(capture)
        },
      },
    },
    permissions: { contains: async () => permitted },
    contextMenus: {
      removeAll: async () => { menus.length = 0 },
      create: (menu) => menus.push(menu),
      onClicked: event(),
    },
    action: {
      setBadgeText: async (badge) => badges.push(badge.text),
      setBadgeBackgroundColor: async () => {},
      setTitle: async ({ title }) => titles.push(title),
      openPopup: async (options) => popups.push(options),
    },
    runtime: { onInstalled: event(), onStartup: event(), onMessage: event() },
    scripting: { executeScript: async (options) => { notifications.push(options) } },
    commands: { onCommand: event() },
    tabs: { query: async () => [] },
  }
  return { chrome, menus, badges, titles, notifications, popups, captures, terminal: terminal.promise }
}

let workerNumber = 0
async function startWorker(harness) {
  globalThis.chrome = harness.chrome
  await import(`${backgroundModule}#${workerNumber++}`)
}

const draft = { url: 'https://example.com/article', title: 'An article', tags: 'reading dev/rust, web', description: 'A note' }

function savedBookmark(id, url = draft.url) {
  return { id, url, title: 'Stored title', description: 'Stored note', tags: ['reading'] }
}

test('extension capture and BB integration', async (t) => {
  t.beforeEach(() => {
    t.mock.timers.reset()
    t.mock.timers.enable({ apis: ['setTimeout'] })
  })
  const originalFetch = globalThis.fetch
  const originalChrome = globalThis.chrome
  t.after(() => {
    globalThis.fetch = originalFetch
    if (originalChrome === undefined) delete globalThis.chrome
    else globalThis.chrome = originalChrome
  })

  await t.test('link and page actions select different URLs, including inside a frame', () => {
    const info = {
      menuItemId: 'send-link', linkUrl: 'https://target.example/article', pageUrl: 'https://frame.example',
    }
    const tab = { url: 'https://page.example', title: 'Source page' }
    const link = bb.draftFromContext(info, tab)
    assert.equal(link.url, 'https://target.example/article')
    assert.equal(link.title, '')
    const page = bb.draftFromContext({ ...info, menuItemId: 'send-page' }, tab)
    assert.equal(page.url, 'https://page.example')
    assert.equal(page.title, 'Source page')
  })

  await t.test('server settings preserve proxy paths and reject credentials and non-web URLs', () => {
    assert.deepEqual(bb.connectionFromFields(' https://bb.example/bookmarks/// ', ' token '), {
      server: 'https://bb.example/bookmarks', token: 'token',
    })
    assert.equal(bb.hostPermission('http://localhost:8080/bb'), 'http://localhost/*')
    for (const url of ['chrome://settings', 'javascript:alert(1)', 'https://user:pass@example.com', 'bad-url']) {
      assert.throws(() => bb.httpUrl(url))
    }
    assert.throws(() => bb.connectionFromFields('https://bb.example/?token=secret', ''))
  })

  await t.test('API sends bearer auth, normalizes tags, and enables background metadata', async () => {
    const requests = []
    globalThis.fetch = async (url, options) => {
      requests.push({ url, options })
      return Response.json(savedBookmark(42))
    }
    const bookmark = await bb.createBookmark({ server: 'https://bb.example/bb', token: 'secret' }, draft)
    assert.equal(bookmark.id, 42)
    const { url, options } = requests[0]
    assert.equal(url, 'https://bb.example/bb/api/bookmarks/create')
    assert.equal(options.headers.Authorization, 'Bearer secret')
    assert.equal(options.credentials, 'omit')
    const body = JSON.parse(options.body)
    assert.equal(body.url, draft.url)
    assert.equal(body.tags, 'reading,dev/rust,web')
    assert.equal(body.async_meta, true)
  })

  await t.test('connection test rejects a non-BB server even when it returns JSON', async () => {
    globalThis.fetch = async () => Response.json({ ok: true })
    await assert.rejects(bb.testConnection({ server: 'https://wrong.example', token: '' }))
    globalThis.fetch = async () => Response.json({ task_queue_max_threads: 4 })
    await bb.testConnection({ server: 'https://bb.example', token: '' })
  })

  await t.test('lookup asks BB for an exact URL and loads saved editor fields', async () => {
    const requests = []
    globalThis.fetch = async (url, options) => {
      requests.push({ url, body: JSON.parse(options.body) })
      return Response.json([savedBookmark(8)])
    }
    const bookmark = await bb.findBookmark({ server: 'https://bb.example', token: '' }, draft.url)
    assert.equal(requests[0].url, 'https://bb.example/api/bookmarks/search')
    assert.equal(requests[0].body.url, draft.url)
    assert.equal(requests[0].body.exact, true)
    assert.equal(bookmark.id, 8)
    assert.equal(bb.bookmarkDraft(bookmark).description, 'Stored note')
    assert.equal(bb.bookmarkDraft(bookmark).tags, 'reading')
  })

  await t.test('lookup finds bare-origin bookmarks created outside the browser', async () => {
    const candidates = []
    globalThis.fetch = async (_url, options) => {
      const { url } = JSON.parse(options.body)
      candidates.push(url)
      return Response.json(url === 'https://example.com' ? [savedBookmark(5, url)] : [])
    }
    const result = await bb.findBookmark({ server: 'https://bb.example', token: '' }, 'https://example.com/')
    assert.equal(result.id, 5)
    assert.deepEqual(candidates, ['https://example.com/', 'https://example.com'])
  })

  await t.test('editor updates the original ID and can clear title, description, and tags', async () => {
    const requests = []
    globalThis.fetch = async (url, options) => {
      requests.push({ url, body: JSON.parse(options.body) })
      return Response.json(savedBookmark(8, 'https://changed.example/'))
    }
    await bb.updateBookmark({ server: 'https://bb.example', token: '' }, 8, {
      url: 'https://changed.example/', title: '', description: '', tags: '',
    })
    assert.equal(requests[0].url, 'https://bb.example/api/bookmarks/update')
    assert.equal(requests[0].body.id, 8)
    assert.equal(requests[0].body.url, 'https://changed.example/')
    for (const field of ['title', 'description', 'tags']) assert.equal(requests[0].body[field], '')
  })

  await t.test('failed edits retain their target ID so retry cannot create a new bookmark', async () => {
    const harness = chromeHarness()
    globalThis.fetch = async () => new Response('', { status: 500 })
    await startWorker(harness)
    const response = deferred()
    const changed = { ...draft, url: 'https://changed.example/' }
    harness.chrome.runtime.onMessage.listeners[0]({ type: 'update-bookmark', id: 23, draft: changed, tabId: 4 }, {}, response.resolve)
    const capture = await response.promise
    assert.equal(capture.status, 'error')
    assert.equal(capture.type, 'update-bookmark')
    assert.equal(capture.id, 23)
    assert.equal(capture.draft.url, changed.url)
    let query
    globalThis.fetch = async (_url, options) => {
      query = JSON.parse(options.body)
      return Response.json([savedBookmark(23)])
    }
    const original = await bb.getBookmark({ server: 'https://bb.example', token: '' }, capture.id)
    assert.equal(query.id, 23)
    assert.equal(original.id, 23)
  })

  await t.test('page action remains available on links, selections, media, and inputs', async () => {
    const harness = chromeHarness()
    await startWorker(harness)
    harness.chrome.runtime.onInstalled.listeners[0]()
    await new Promise(setImmediate)
    const page = harness.menus.find((menu) => menu.id === 'send-page')
    for (const context of ['page', 'link', 'selection', 'image', 'editable', 'frame', 'video', 'audio']) {
      assert.ok(page.contexts.includes(context))
    }
    const link = harness.menus.find((menu) => menu.id === 'send-link')
    assert.deepEqual(link.contexts, ['link'])
  })

  await t.test('context save uses the link target and persists success without opening a tab', async () => {
    const harness = chromeHarness()
    const requests = []
    globalThis.fetch = async (_url, options) => {
      requests.push(JSON.parse(options.body))
      return Response.json(savedBookmark(7))
    }
    await startWorker(harness)
    harness.chrome.contextMenus.onClicked.listeners[0]({
      menuItemId: 'send-link', linkUrl: draft.url, pageUrl: 'https://source.example',
    }, { id: 10, windowId: 2, url: 'https://source.example', title: 'Source' })
    const result = await harness.terminal
    await new Promise(setImmediate)
    assert.equal(result.status, 'saved')
    assert.equal(requests[0].url, draft.url)
    assert.equal(requests[0].title, undefined)
    assert.equal(result.tabId, 10)
    assert.ok(harness.badges.includes('✓'))
    assert.equal(harness.popups.length, 0)
    assert.equal(harness.notifications[0].target.tabId, 10)
    assert.equal(harness.notifications[0].args[1], false)
  })

  await t.test('popup submits through the worker and receives the saved bookmark', async () => {
    const harness = chromeHarness()
    globalThis.fetch = async () => Response.json(savedBookmark(23))
    await startWorker(harness)
    const response = deferred()
    const keepAlive = harness.chrome.runtime.onMessage.listeners[0]({
      type: 'save-bookmark', draft, tabId: 4,
    }, {}, response.resolve)
    assert.equal(keepAlive, true)
    const result = await response.promise
    assert.equal(result.status, 'saved')
    assert.equal(result.bookmark.id, 23)
  })

  await t.test('success badge disappears after brief feedback', async () => {
    const harness = chromeHarness()
    globalThis.fetch = async () => Response.json(savedBookmark(23))
    await startWorker(harness)
    const response = deferred()
    harness.chrome.runtime.onMessage.listeners[0]({ type: 'save-bookmark', draft, tabId: 4 }, {}, response.resolve)
    await response.promise
    assert.equal(harness.badges.at(-1), '✓')
    t.mock.timers.tick(3000)
    await new Promise(setImmediate)
    assert.equal(harness.badges.at(-1), '')
    assert.equal(harness.titles.at(-1), 'Send to BB')
  })

  await t.test('older feedback does not clear the badge of a new save', async () => {
    const harness = chromeHarness()
    globalThis.fetch = async () => Response.json(savedBookmark(23))
    await startWorker(harness)
    const first = deferred()
    const send = harness.chrome.runtime.onMessage.listeners[0]
    send({ type: 'save-bookmark', draft, tabId: 4 }, {}, first.resolve)
    await first.promise
    t.mock.timers.tick(2000)
    const request = deferred()
    globalThis.fetch = () => request.promise
    const second = deferred()
    send({ type: 'save-bookmark', draft, tabId: 4 }, {}, second.resolve)
    await new Promise(setImmediate)
    t.mock.timers.tick(1000)
    await new Promise(setImmediate)
    assert.equal(harness.badges.at(-1), '…')
    request.resolve(Response.json(savedBookmark(24)))
    await second.promise
    assert.equal(harness.badges.at(-1), '✓')
    t.mock.timers.tick(3000)
    await new Promise(setImmediate)
    assert.equal(harness.badges.at(-1), '')
  })

  await t.test('a blocked page toast does not turn a successful save into an error', async () => {
    const harness = chromeHarness()
    harness.chrome.scripting.executeScript = async () => { throw new Error('Cannot access this page') }
    globalThis.fetch = async () => Response.json(savedBookmark(23))
    await startWorker(harness)
    const response = deferred()
    harness.chrome.runtime.onMessage.listeners[0]({ type: 'save-bookmark', draft, tabId: 4 }, {}, response.resolve)
    assert.equal((await response.promise).status, 'saved')
  })

  await t.test('saving an existing URL preserves its details and reports success', async () => {
    const harness = chromeHarness()
    const paths = []
    globalThis.fetch = async (url) => {
      paths.push(url)
      return url.endsWith('/search') ? Response.json([savedBookmark(6)]) : new Response('', { status: 409 })
    }
    await startWorker(harness)
    const response = deferred()
    harness.chrome.runtime.onMessage.listeners[0]({ type: 'save-bookmark', draft, tabId: 4 }, {}, response.resolve)
    const result = await response.promise
    assert.equal(result.status, 'saved')
    assert.equal(result.operation, 'existing')
    assert.equal(result.bookmark.title, 'Stored title')
    assert.equal(result.bookmark.description, 'Stored note')
    assert.ok(paths.every((path) => !path.endsWith('/update')))
    assert.equal(harness.popups.length, 0)
  })

  await t.test('popup updates route through the worker and return editable saved fields', async () => {
    const harness = chromeHarness()
    const requests = []
    globalThis.fetch = async (url, options) => {
      requests.push({ url, body: JSON.parse(options.body) })
      return Response.json(savedBookmark(23))
    }
    await startWorker(harness)
    const response = deferred()
    harness.chrome.runtime.onMessage.listeners[0]({ type: 'update-bookmark', id: 23, draft, tabId: 4 }, {}, response.resolve)
    const result = await response.promise
    assert.equal(result.status, 'saved')
    assert.equal(result.operation, 'updated')
    assert.equal(requests[0].body.id, 23)
    assert.ok(requests[0].url.endsWith('/api/bookmarks/update'))
  })

  for (const providedTab of [true, false]) {
    await t.test(`keyboard shortcut saves the current page with ${providedTab ? 'the supplied tab' : 'an active-tab lookup'}`, async () => {
      const harness = chromeHarness()
      const tab = { id: 15, windowId: 3, url: 'https://current.example/page', title: 'Current page' }
      let queries = 0
      harness.chrome.tabs.query = async (query) => {
        assert.equal(query.active, true)
        assert.equal(query.lastFocusedWindow, true)
        queries++
        return [tab]
      }
      const requests = []
      globalThis.fetch = async (_url, options) => {
        requests.push(JSON.parse(options.body))
        return Response.json(savedBookmark(31))
      }
      await startWorker(harness)
      const [command] = Object.keys(manifest.commands)
      assert.equal(typeof command, 'string')
      harness.chrome.commands.onCommand.listeners[0](command, providedTab ? tab : undefined)
      const capture = await harness.terminal
      assert.equal(capture.status, 'saved')
      assert.equal(capture.tabId, tab.id)
      assert.equal(requests[0].url, tab.url)
      assert.equal(requests[0].title, tab.title)
      assert.equal(queries, providedTab ? 0 : 1)
      assert.equal(harness.popups.length, 0)
    })
  }

  await t.test('keyboard shortcut opens setup and preserves the current URL when disconnected', async () => {
    const harness = chromeHarness(null)
    globalThis.fetch = async () => { throw new Error('Unexpected request') }
    await startWorker(harness)
    const [command] = Object.keys(manifest.commands)
    harness.chrome.commands.onCommand.listeners[0](command, { id: 16, windowId: 3, url: draft.url, title: draft.title })
    const capture = await harness.terminal
    await new Promise(setImmediate)
    assert.equal(capture.status, 'needs-connection')
    assert.equal(capture.draft.url, draft.url)
    assert.equal(harness.popups.length, 1)
    assert.equal(harness.popups[0].windowId, 3)
  })

  for (const [name, connection, permitted] of [
    ['missing connection', null, true],
    ['revoked host permission', { server: 'https://bb.example', token: '' }, false],
  ]) {
    await t.test(`${name} preserves the link and opens setup without a network request`, async () => {
      const harness = chromeHarness(connection, permitted)
      let requests = 0
      globalThis.fetch = async () => { requests++; throw new Error('Unexpected request') }
      await startWorker(harness)
      harness.chrome.contextMenus.onClicked.listeners[0]({ menuItemId: 'send-link', linkUrl: draft.url }, { id: 2 })
      const capture = await harness.terminal
      await new Promise(setImmediate)
      assert.equal(capture.status, 'needs-connection')
      assert.equal(capture.draft.url, draft.url)
      assert.equal(requests, 0)
      assert.equal(harness.popups.length, 1)
      assert.ok(harness.badges.includes('!'))
    })
  }

  for (const status of [401, 409, 500]) {
    await t.test(`HTTP ${status} leaves a retryable error instead of reporting success`, async () => {
      const harness = chromeHarness()
      globalThis.fetch = async () => new Response('', { status })
      await startWorker(harness)
      harness.chrome.contextMenus.onClicked.listeners[0]({ menuItemId: 'send-page', pageUrl: draft.url }, { id: 9 })
      const capture = await harness.terminal
      await new Promise(setImmediate)
      assert.equal(capture.status, 'error')
      assert.equal(capture.draft.url, draft.url)
      assert.ok(capture.message.length > 0)
      assert.ok(!capture.message.includes('test-token'))
      assert.ok(harness.badges.includes('!'))
      assert.equal(harness.popups.length, 1)
    })
  }

  await t.test('network failures and invalid success responses are actionable', async () => {
    const connection = { server: 'https://bb.example', token: '' }
    globalThis.fetch = async () => { throw new TypeError('Failed to fetch') }
    await assert.rejects(bb.createBookmark(connection, draft), /Could not reach BB/)
    globalThis.fetch = async () => Response.json({ message: 'not a bookmark' })
    await assert.rejects(bb.createBookmark(connection, draft), /invalid bookmark/)
    globalThis.fetch = async () => new Response('<html>Proxy login</html>')
    await assert.rejects(bb.createBookmark(connection, draft), /server address/)
  })
})
