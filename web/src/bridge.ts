import type { HostPush, HostRequest } from './types'

/**
 * Sole channel between the Vue UI and the .NET host.
 *
 * The host injects a `hostBridge` object via AddHostObjectToScript; replies come
 * back through `window.__pasteReceive(json)`. Keeping both directions in one
 * module means components never touch WebView2 APIs directly.
 */

declare global {
  interface Window {
    chrome?: {
      webview?: {
        postMessage: (message: unknown) => void
        addEventListener: (
          type: 'message',
          handler: (event: { data: unknown }) => void,
        ) => void
      }
    }
  }
}

type PushHandler = (message: HostPush) => void

const handlers = new Set<PushHandler>()
let devSeeded = false

/** True when running inside the WebView2 host rather than a plain browser. */
export function isHosted(): boolean {
  return typeof window.chrome?.webview?.postMessage === 'function'
}

/** Sends a request to the native host. */
export function send(request: HostRequest): void {
  if (!isHosted()) {
    if (import.meta.env.DEV) {
      // eslint-disable-next-line no-console
      console.debug('[bridge] (dev) request', request)
    }
    return
  }

  window.chrome!.webview!.postMessage(request)
}

/** Subscribes to host pushes; returns an unsubscribe function. */
export function onPush(handler: PushHandler): () => void {
  handlers.add(handler)
  ensureListener()
  return () => handlers.delete(handler)
}

let listenerAttached = false

function ensureListener(): void {
  if (listenerAttached) {
    return
  }

  listenerAttached = true

  if (isHosted()) {
    // WebView2 delivers PostWebMessageAsJson payloads on the webview 'message'
    // event, with the parsed object already on event.data.
    window.chrome!.webview!.addEventListener('message', (event) => {
      dispatch(event.data)
    })
    return
  }

  if (import.meta.env.DEV) {
    seedDevData()
  }
}

function dispatch(raw: unknown): void {
  try {
    const message = (
      typeof raw === 'string' ? JSON.parse(raw) : raw
    ) as HostPush

    handlers.forEach((handler) => handler(message))
  } catch (error) {
    // eslint-disable-next-line no-console
    console.error('[bridge] malformed host message', error, raw)
  }
}

/** Fills the UI with sample entries so `npm run dev` shows something useful. */
function seedDevData(): void {
  if (devSeeded) {
    return
  }

  devSeeded = true

  const samples = [
    'https://github.com/dotnet/wpf',
    'dotnet build -c Release -r win-x64 --self-contained true',
    'SELECT Id, Type, Content FROM ClipboardItems ORDER BY CreatedAt DESC',
    'The quick brown fox jumps over the lazy dog',
    '好记性不如烂笔头',
    'git rebase --interactive HEAD~3',
    'C:\\Users\\Pyzl\\Documents\\report.pdf',
  ]

  const items = Array.from({ length: 40 }, (_, index) => {
    const text = samples[index % samples.length]
    return {
      id: index + 1,
      type: 'Text' as const,
      content: `[${String(index + 1).padStart(2, '0')}] ${text}`,
      preview: `[${String(index + 1).padStart(2, '0')}] ${text}`,
      imageUrl: null,
      createdAtLabel: '2026-10-14 12:00',
      pinned: index === 2 || index === 15,
    }
  }).reverse()

  setTimeout(() => {
    dispatch({ type: 'items', items, isEmpty: false, totalCount: items.length })
    dispatch({
      type: 'selection',
      itemId: items[0]?.id ?? null,
      canPrev: false,
      canNext: true,
    })
    dispatch({ type: 'hotkey', label: 'Ctrl + Shift + V' })
  }, 80)
}
