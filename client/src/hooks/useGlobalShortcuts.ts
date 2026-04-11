import { useCallback, useEffect, useRef } from 'react'
import { deleteBookmark } from '@/lib/api'
import { useDisplayBookmarks } from '@/hooks/useDisplayBookmarks'
import { useStore } from '@/lib/store'
import { openBookmarkDetailInEditMode } from '@/lib/bookmarkDetailModal'
import {
  cycleViewMode,
  focusBookmarkViewport,
  getBookmarkViewport,
  getFirstVisibleBookmarkId,
  getGridColumnEndId,
  getGridColumnHomeId,
  getNextBookmarkIdAfterDelete,
  getVisibleBookmarkIds,
  getViewportEdgeId,
  isEditableTarget,
  isSearchInputTarget,
} from '@/lib/bookmarkSelection'

const ARM_TIMEOUT_MS = 1200

function isNavigationKey(key: string): boolean {
  return ['ArrowUp', 'ArrowDown', 'ArrowLeft', 'ArrowRight', 'h', 'j', 'k', 'l'].includes(key)
}

function isViewportJumpKey(key: string): boolean {
  return key === 'Home' || key === 'End' || key === 'PageUp' || key === 'PageDown'
}

export function useGlobalShortcuts() {
  const { displayBookmarks } = useDisplayBookmarks()
  const deleteArmTimeoutRef = useRef<number | null>(null)
  const initializedSelectionRef = useRef(false)
  const searchQuery = useStore((s) => s.searchQuery)

  const clearDeleteArm = useCallback(() => {
    if (deleteArmTimeoutRef.current !== null) {
      window.clearTimeout(deleteArmTimeoutRef.current)
      deleteArmTimeoutRef.current = null
    }
    useStore.getState().setArmedDeleteBookmarkId(null)
  }, [])

  const armDelete = useCallback((bookmarkId: number) => {
    clearDeleteArm()
    useStore.getState().setArmedDeleteBookmarkId(bookmarkId)
    deleteArmTimeoutRef.current = window.setTimeout(() => {
      deleteArmTimeoutRef.current = null
      useStore.getState().setArmedDeleteBookmarkId(null)
    }, ARM_TIMEOUT_MS)
  }, [clearDeleteArm])

  useEffect(() => {
    const state = useStore.getState()
    if (state.detailModalId !== null) return
    if (state.selectedBookmarkId === null) return
    if (displayBookmarks.some((bookmark) => bookmark.id === state.selectedBookmarkId)) return
    state.setSelectedBookmarkId(null)
    state.setArmedDeleteBookmarkId(null)
  }, [armDelete, clearDeleteArm, displayBookmarks])

  useEffect(() => {
    const state = useStore.getState()
    state.setSelectedBookmarkId(null)
    clearDeleteArm()
  }, [clearDeleteArm, searchQuery])

  useEffect(() => clearDeleteArm, [clearDeleteArm])

  useEffect(() => {
    if (initializedSelectionRef.current) return
    if (displayBookmarks.length === 0) return

    const state = useStore.getState()
    if (state.detailModalId !== null || state.selectedBookmarkId !== null) {
      initializedSelectionRef.current = true
      return
    }

    const assignInitialSelection = () => {
      const firstVisibleId = getFirstVisibleBookmarkId(getBookmarkViewport()) ?? displayBookmarks[0].id
      useStore.getState().setSelectedBookmarkId(firstVisibleId)
      initializedSelectionRef.current = true
    }

    requestAnimationFrame(assignInitialSelection)
  }, [armDelete, clearDeleteArm, displayBookmarks])

  useEffect(() => {
    const handler = async (e: KeyboardEvent) => {
      const hasModifiers = e.metaKey || e.ctrlKey || e.altKey
      const state = useStore.getState()
      const {
        detailModalId,
        createModalOpen,
        bulkEditOpen,
        bulkDeleteOpen,
        settingsOpen,
      } = state

      if (detailModalId !== null || createModalOpen || bulkEditOpen || bulkDeleteOpen || settingsOpen) {
        return
      }

      if (hasModifiers) {
        return
      }

      if (isEditableTarget(e.target)) {
        return
      }

      if (e.key === 'i' || e.key === '/') {
        e.preventDefault()
        document.querySelector<HTMLInputElement>('[data-main-search-input="true"]')?.focus()
        return
      }

      if (e.key === ',') {
        e.preventDefault()
        state.setSettingsOpen(true)
        return
      }

      if (e.key === 'w' && state.workspacesAvailable) {
        e.preventDefault()
        const select = document.querySelector<HTMLSelectElement>('[data-workspace-select="true"]')
        select?.focus()
        select?.showPicker?.()
        return
      }

      if (e.key === 's') {
        e.preventDefault()
        document.querySelector<HTMLButtonElement>('[data-filter-toggle="true"]')?.click()
        return
      }

      if (e.key === 'p') {
        e.preventDefault()
        state.pinToUrl()
        return
      }

      if (e.key === 'a') {
        e.preventDefault()
        state.setShowAll(!state.showAll)
        return
      }

      if (e.key === 'v') {
        e.preventDefault()
        state.setViewMode(cycleViewMode(state.viewMode))
        return
      }

      if (isNavigationKey(e.key)) {
        if (displayBookmarks.length === 0) return
        e.preventDefault()

        const currentIndex = displayBookmarks.findIndex((bookmark) => bookmark.id === state.selectedBookmarkId)
        if (currentIndex === -1) {
          const firstVisibleId = getFirstVisibleBookmarkId(getBookmarkViewport()) ?? displayBookmarks[0].id
          state.setSelectedBookmarkId(firstVisibleId)
          return
        }

        const delta = getNavigationDelta(e.key, state.viewMode, state.columns)
        const nextIndex = currentIndex + delta
        if (nextIndex < 0 || nextIndex >= displayBookmarks.length) {
          return
        }
        state.setSelectedBookmarkId(displayBookmarks[nextIndex].id)
        return
      }

      if (isViewportJumpKey(e.key)) {
        const viewport = getBookmarkViewport()
        if (!viewport || displayBookmarks.length === 0) return

        const currentIndex = displayBookmarks.findIndex((bookmark) => bookmark.id === state.selectedBookmarkId)
        const fallbackIndex = currentIndex >= 0 ? currentIndex : 0

        if (e.key === 'Home') {
          e.preventDefault()
          viewport.scrollTop = 0
          const nextId = state.viewMode === 'grid'
            ? getGridColumnHomeId(displayBookmarks, state.columns, fallbackIndex)
            : displayBookmarks[0]?.id ?? null
          state.setSelectedBookmarkId(nextId)
          return
        }

        if (e.key === 'End') {
          e.preventDefault()
          viewport.scrollTop = viewport.scrollHeight
          const nextId = state.viewMode === 'grid'
            ? getGridColumnEndId(displayBookmarks, state.columns, fallbackIndex)
            : displayBookmarks[displayBookmarks.length - 1]?.id ?? null
          state.setSelectedBookmarkId(nextId)
          return
        }

        e.preventDefault()
        const direction = e.key === 'PageUp' ? -1 : 1
        viewport.scrollTop += direction * viewport.clientHeight

        requestAnimationFrame(() => {
          const visibleIds = getVisibleBookmarkIds(viewport)
          const nextId = state.viewMode === 'grid'
            ? getViewportEdgeId(
                visibleIds,
                displayBookmarks,
                state.columns,
                fallbackIndex,
                direction < 0 ? 'start' : 'end',
              )
            : direction < 0
              ? visibleIds[0] ?? null
              : visibleIds[visibleIds.length - 1] ?? null
          useStore.getState().setSelectedBookmarkId(nextId)
        })
        return
      }

      if (e.key === 'e' && state.selectedBookmarkId !== null) {
        e.preventDefault()
        state.revealSelectedBookmark()
        openBookmarkDetailInEditMode(state.selectedBookmarkId)
        return
      }

      if (e.key === 'Enter' && state.selectedBookmarkId !== null) {
        e.preventDefault()
        state.revealSelectedBookmark()
        state.setDetailModalId(state.selectedBookmarkId)
        return
      }

      if (e.key === 'o' && state.selectedBookmarkId !== null) {
        e.preventDefault()
        state.revealSelectedBookmark()
        const bookmark = displayBookmarks.find((item) => item.id === state.selectedBookmarkId)
        if (bookmark) window.open(bookmark.url, '_blank', 'noopener,noreferrer')
        return
      }

      if (e.key === 'd' && state.selectedBookmarkId !== null) {
        e.preventDefault()
        state.revealSelectedBookmark()
        const armed = state.armedDeleteBookmarkId === state.selectedBookmarkId
        if (!armed) {
          armDelete(state.selectedBookmarkId)
          return
        }

        const selectedId = state.selectedBookmarkId
        const nextSelectedId = getNextBookmarkIdAfterDelete(displayBookmarks, selectedId)
        await deleteBookmark(selectedId)
        const current = useStore.getState().bookmarks
        useStore.getState().setBookmarks(current.filter((bookmark) => bookmark.id !== selectedId))
        useStore.getState().setSelectedBookmarkId(nextSelectedId)
        clearDeleteArm()
        return
      }

      if (e.key === 'x' || e.key === 'Escape') {
        if (state.armedDeleteBookmarkId !== null) {
          e.preventDefault()
          clearDeleteArm()
        }
      }
    }

    const inputHandler = (e: KeyboardEvent) => {
      if (e.metaKey || e.ctrlKey || e.altKey) return
      if (!isSearchInputTarget(e.target)) return
      if (e.key !== 'Escape' && e.key !== 'Enter') return
      const target = e.target
      if (!(target instanceof HTMLElement)) return
      e.preventDefault()
      target.blur()
      focusBookmarkViewport()
      const state = useStore.getState()
      if (state.detailModalId !== null || state.selectedBookmarkId !== null || displayBookmarks.length === 0) return
      const firstVisibleId = getFirstVisibleBookmarkId(getBookmarkViewport()) ?? displayBookmarks[0].id
      state.setSelectedBookmarkId(firstVisibleId)
    }

    window.addEventListener('keydown', handler, true)
    window.addEventListener('keydown', inputHandler, true)
    return () => {
      window.removeEventListener('keydown', handler, true)
      window.removeEventListener('keydown', inputHandler, true)
    }
  }, [armDelete, clearDeleteArm, displayBookmarks])
}

function getNavigationDelta(key: string, viewMode: 'grid' | 'cards' | 'table', columns: number): number {
  switch (key) {
    case 'ArrowLeft':
    case 'h':
      return -1
    case 'ArrowRight':
    case 'l':
      return 1
    case 'ArrowUp':
    case 'k':
      return viewMode === 'grid' ? -columns : -1
    case 'ArrowDown':
    case 'j':
      return viewMode === 'grid' ? columns : 1
    default:
      return 0
  }
}
