import type { Bookmark } from '@/lib/api'

const VIEWPORT_SELECTOR = '[data-bookmark-viewport="true"]'
const BOOKMARK_SELECTOR = '[data-bookmark-id]'

export function getBookmarkViewport(): HTMLElement | null {
  return document.querySelector<HTMLElement>(VIEWPORT_SELECTOR)
}

export function focusBookmarkViewport() {
  getBookmarkViewport()?.focus()
}

export function getFirstVisibleBookmarkId(container: HTMLElement | null): number | null {
  if (!container) return null

  const containerRect = container.getBoundingClientRect()
  const candidates = Array.from(container.querySelectorAll<HTMLElement>(BOOKMARK_SELECTOR))
    .map((node) => {
      const rect = node.getBoundingClientRect()
      const id = Number(node.dataset.bookmarkId)
      return { node, rect, id }
    })
    .filter(({ rect, id }) =>
      Number.isFinite(id)
      && rect.bottom > containerRect.top
      && rect.top < containerRect.bottom,
    )
    .sort((a, b) => (a.rect.top - b.rect.top) || (a.rect.left - b.rect.left))

  return candidates[0]?.id ?? null
}

export function getVisibleBookmarkIds(container: HTMLElement | null): number[] {
  if (!container) return []

  const containerRect = container.getBoundingClientRect()
  return Array.from(container.querySelectorAll<HTMLElement>(BOOKMARK_SELECTOR))
    .map((node) => {
      const rect = node.getBoundingClientRect()
      const id = Number(node.dataset.bookmarkId)
      return { rect, id }
    })
    .filter(({ rect, id }) =>
      Number.isFinite(id)
      && rect.bottom > containerRect.top
      && rect.top < containerRect.bottom,
    )
    .sort((a, b) => (a.rect.top - b.rect.top) || (a.rect.left - b.rect.left))
    .map(({ id }) => id)
}

export function isEditableTarget(target: EventTarget | null): boolean {
  if (!(target instanceof HTMLElement)) return false
  if (target.closest('[data-workspace-select="true"]')) return false
  return !!target.closest('input, textarea, select, [contenteditable="true"], [contenteditable=""], [role="textbox"], [role="combobox"]')
}

export function isSearchInputTarget(target: EventTarget | null): target is HTMLElement {
  return target instanceof HTMLElement && !!target.closest('[data-search-input="true"]')
}

export function getNextBookmarkIdAfterDelete(bookmarks: Bookmark[], deletedId: number): number | null {
  const index = bookmarks.findIndex((bookmark) => bookmark.id === deletedId)
  if (index === -1) return null
  return bookmarks[index + 1]?.id ?? bookmarks[index - 1]?.id ?? null
}

export function cycleViewMode(mode: 'grid' | 'cards' | 'table'): 'grid' | 'cards' | 'table' {
  if (mode === 'grid') return 'cards'
  if (mode === 'cards') return 'table'
  return 'grid'
}

export function getGridColumn(index: number, columns: number): number {
  return index % columns
}

export function getGridColumnHomeId(bookmarks: Bookmark[], columns: number, currentIndex: number): number | null {
  if (bookmarks.length === 0) return null
  const column = getGridColumn(currentIndex, columns)
  return bookmarks[Math.min(column, bookmarks.length - 1)]?.id ?? null
}

export function getGridColumnEndId(bookmarks: Bookmark[], columns: number, currentIndex: number): number | null {
  if (bookmarks.length === 0) return null
  const column = getGridColumn(currentIndex, columns)
  const rowCount = Math.ceil(bookmarks.length / columns)
  for (let row = rowCount - 1; row >= 0; row -= 1) {
    const index = row * columns + column
    if (index < bookmarks.length) return bookmarks[index].id
  }
  return bookmarks[bookmarks.length - 1]?.id ?? null
}

export function getViewportEdgeId(
  visibleIds: number[],
  bookmarks: Bookmark[],
  columns: number,
  currentIndex: number,
  edge: 'start' | 'end',
): number | null {
  if (visibleIds.length === 0) return null

  const visibleSet = new Set(visibleIds)
  const currentColumn = getGridColumn(currentIndex, columns)
  const candidates = bookmarks.filter((bookmark, index) =>
    visibleSet.has(bookmark.id) && getGridColumn(index, columns) === currentColumn,
  )

  if (candidates.length > 0) {
    return edge === 'start' ? candidates[0].id : candidates[candidates.length - 1].id
  }

  return edge === 'start' ? visibleIds[0] : visibleIds[visibleIds.length - 1]
}
