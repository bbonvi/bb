export function isBookmarkFullyVisible(container: HTMLElement, bookmarkId: number): boolean {
  const target = container.querySelector<HTMLElement>(`[data-bookmark-id="${bookmarkId}"]`)
  if (!target) return false

  const containerRect = container.getBoundingClientRect()
  const targetRect = target.getBoundingClientRect()

  return targetRect.top >= containerRect.top && targetRect.bottom <= containerRect.bottom
}

export function smoothScrollVirtualIndexToCenter(
  _container: HTMLElement,
  scrollToIndex: (index: number, opts?: { align?: 'auto' | 'start' | 'center' | 'end' }) => void,
  index: number,
) {
  scrollToIndex(index, { align: 'center' })
}

export function smoothScrollElementToCenter(container: HTMLElement, element: HTMLElement) {
  const containerRect = container.getBoundingClientRect()
  const elementRect = element.getBoundingClientRect()
  const relativeTop = elementRect.top - containerRect.top + container.scrollTop
  const targetTop = Math.max(0, relativeTop - (container.clientHeight - elementRect.height) / 2)
  container.scrollTop = targetTop
}
