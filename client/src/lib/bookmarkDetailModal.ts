import { useStore } from '@/lib/store'

let pendingEditBookmarkId: number | null = null

/** Opens the bookmark detail modal and requests that it enter local edit mode on open. */
export function openBookmarkDetailInEditMode(id: number): void {
  pendingEditBookmarkId = id
  const state = useStore.getState()
  state.setSelectedBookmarkId(id)
  state.setDetailModalId(id)
}

/** Consumes a one-shot edit request for the currently opened bookmark. */
export function consumePendingDetailEditRequest(id: number | null): boolean {
  if (id === null || pendingEditBookmarkId !== id) return false
  pendingEditBookmarkId = null
  return true
}
