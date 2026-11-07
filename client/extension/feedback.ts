// executeScript serializes this function, so everything it uses must live inside it.
export function pageToast(message: string, failed: boolean, duration: number) {
  const id = 'bb-extension-toast'
  document.getElementById(id)?.remove()
  const host = document.createElement('div')
  host.id = id
  host.style.cssText = 'all:initial!important;position:fixed!important;inset:18px 18px auto auto!important;margin:0!important;padding:0!important;border:0!important;background:transparent!important;z-index:2147483647!important;max-width:calc(100vw - 36px)!important;'
  const shadow = host.attachShadow({ mode: 'closed' })
  const style = document.createElement('style')
  style.textContent = `
    .toast { display:flex; align-items:center; gap:10px; max-width:320px; box-sizing:border-box;
      padding:12px 14px; border:1px solid #ffffff20; border-radius:10px;
      background:#1c1c21; color:#e8e8ed; box-shadow:0 6px 28px #0004;
      font:13px/1.45 'Avenir Next','Segoe UI',sans-serif; }
    .mark { color:${failed ? '#ffa4ad' : '#9ad7bc'}; font-size:16px; }
    .message { overflow-wrap:anywhere; }
    button { flex-shrink:0; margin-left:6px; padding:0 2px; border:0; background:transparent;
      color:#9494a3; cursor:pointer; font:18px/1 sans-serif; }
    button:hover { color:#fff; } button:focus-visible { outline:2px solid #6b8afd; outline-offset:3px; }
  `
  const toast = document.createElement('div')
  toast.className = 'toast'
  toast.setAttribute('role', 'status')
  toast.setAttribute('aria-live', 'polite')
  const mark = document.createElement('span')
  mark.className = 'mark'
  mark.textContent = failed ? '!' : '✓'
  mark.setAttribute('aria-hidden', 'true')
  const text = document.createElement('span')
  text.className = 'message'
  text.textContent = message
  const close = document.createElement('button')
  close.type = 'button'
  close.textContent = '×'
  close.setAttribute('aria-label', 'Dismiss BB notification')
  close.addEventListener('click', () => host.remove())
  toast.append(mark, text, close)
  shadow.append(style, toast)
  document.documentElement.append(host)
  setTimeout(() => host.remove(), duration)
}
