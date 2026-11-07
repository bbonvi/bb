import {
  BBError, CAPTURE_KEY, MENU_LINK, MENU_PAGE, SAVE_MESSAGE, UPDATE_MESSAGE,
  createBookmark, draftFromContext, findBookmark, hostPermission, httpUrl,
  isSaveMessage, loadConnection, updateBookmark,
  type Capture, type Draft, type SaveMessage,
} from './bb'
import { pageToast } from './feedback'

let badgeTimer: ReturnType<typeof setTimeout> | undefined

async function resetBadge() {
  await Promise.all([
    chrome.action.setBadgeText({ text: '' }),
    chrome.action.setTitle({ title: 'Send to BB' }),
  ])
}

async function installMenus() {
  await chrome.contextMenus.removeAll()
  chrome.contextMenus.create({
    id: MENU_LINK,
    title: 'Send link to BB',
    contexts: ['link'],
    targetUrlPatterns: ['http://*/*', 'https://*/*'],
  })
  chrome.contextMenus.create({
    id: MENU_PAGE,
    title: 'Send current page to BB',
    contexts: ['page', 'frame', 'selection', 'link', 'editable', 'image', 'video', 'audio'],
    documentUrlPatterns: ['http://*/*', 'https://*/*'],
  })
}

chrome.runtime.onInstalled.addListener(() => {
  void Promise.all([installMenus(), resetBadge()]).catch(console.error)
})
chrome.runtime.onStartup.addListener(() => { void resetBadge().catch(console.error) })

async function publish(capture: Capture): Promise<Capture> {
  clearTimeout(badgeTimer)
  const failed = capture.status === 'error' || capture.status === 'needs-connection'
  await Promise.all([
    chrome.storage.session.set({ [CAPTURE_KEY]: capture }),
    chrome.action.setBadgeText({ text: failed ? '!' : capture.status === 'saved' ? '✓' : '…' }),
    chrome.action.setBadgeBackgroundColor({ color: failed ? '#a63d48' : '#405ec6' }),
    chrome.action.setTitle({ title: failed ? capture.message : capture.status === 'saved' ? 'Saved to BB' : 'Saving to BB…' }),
  ])
  if (capture.status !== 'saving') {
    const duration = failed ? 5000 : 2500
    badgeTimer = setTimeout(() => { void resetBadge().catch(console.error) }, duration)
    if (capture.tabId !== undefined) {
      const message = capture.status === 'saved'
        ? capture.operation === 'existing' ? 'Already in BB' : capture.operation === 'updated' ? 'Changes saved to BB' : 'Saved to BB'
        : capture.message
      try {
        await chrome.scripting.executeScript({
          target: { tabId: capture.tabId },
          func: pageToast,
          args: [message, failed, duration],
        })
      } catch {
        // Restricted pages and tabs that navigated away still get the toolbar feedback.
      }
    }
  }
  return capture
}

async function save(message: SaveMessage): Promise<Capture> {
  const { draft } = message
  let result: Capture
  try {
    httpUrl(draft.url)
    const connection = await loadConnection()
    if (!connection || !await chrome.permissions.contains({ origins: [hostPermission(connection.server)] })) {
      return await publish({
        ...message, status: 'needs-connection',
        message: 'Connect to your BB server to save this URL.',
      })
    }
    await publish({ ...message, status: 'saving' })
    if (message.type === UPDATE_MESSAGE) {
      const bookmark = await updateBookmark(connection, message.id, draft)
      result = { ...message, status: 'saved', bookmark, operation: 'updated' }
    } else {
      try {
        const bookmark = await createBookmark(connection, draft)
        result = { ...message, status: 'saved', bookmark, operation: 'created' }
      } catch (error) {
        if (!(error instanceof BBError) || error.status !== 409) throw error
        const bookmark = await findBookmark(connection, draft.url)
        if (!bookmark) throw error
        result = { ...message, status: 'saved', bookmark, operation: 'existing' }
      }
    }
  } catch (error) {
    result = {
      ...message, status: 'error',
      message: error instanceof Error ? error.message : 'Could not save to BB.',
    }
  }
  return publish(result)
}

async function saveFromBrowser(draft: Draft, tab: chrome.tabs.Tab | undefined) {
  const result = await save({ type: SAVE_MESSAGE, draft, tabId: tab?.id })
  if (result.status === 'error' || result.status === 'needs-connection') {
    try {
      await chrome.action.openPopup({ windowId: tab?.windowId })
    } catch {
      // If Chrome cannot open a popup, the badge and stored capture keep the error visible.
    }
  }
}

chrome.contextMenus.onClicked.addListener((info, tab) => {
  if (info.menuItemId !== MENU_LINK && info.menuItemId !== MENU_PAGE) return
  void saveFromBrowser(draftFromContext(info, tab), tab).catch(console.error)
})

async function sendCurrentPage(tab?: chrome.tabs.Tab) {
  const currentTab = tab ?? (await chrome.tabs.query({ active: true, lastFocusedWindow: true }))[0]
  if (!currentTab) return
  await saveFromBrowser(draftFromContext({ menuItemId: MENU_PAGE }, currentTab), currentTab)
}

chrome.commands.onCommand.addListener((command, tab) => {
  if (command !== MENU_PAGE) return
  void sendCurrentPage(tab).catch(console.error)
})

chrome.runtime.onMessage.addListener((message: unknown, _sender, sendResponse: (capture: Capture) => void) => {
  if (!isSaveMessage(message)) return
  // Keep the request in the service worker so closing the popup does not cancel a save.
  void save(message).then(sendResponse).catch(console.error)
  return true
})
