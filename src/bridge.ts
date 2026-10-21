import { invoke, isTauri } from '@tauri-apps/api/core'
import { listen, type UnlistenFn } from '@tauri-apps/api/event'
import type { HostPush, HostRequest } from './types'

/**
 * Sole channel between the React UI and the Tauri backend.
 *
 * `send` forwards a request to the Rust `dispatch` command; `onPush` subscribes
 * to the single `host-push` event stream the backend emits. Components never
 * touch Tauri APIs directly.
 */

/** True when running inside the Tauri webview rather than a plain browser. */
export function isHosted(): boolean {
  return isTauri()
}

/** Sends a request to the native host. */
export function send(request: HostRequest): void {
  if (!isTauri()) {
    console.debug('[bridge] (browser dev) request', request)
    return
  }

  void invoke('dispatch', { req: request }).catch((error) => {
    console.error('[bridge] dispatch error:', error, request)
  })
}

/**
 * Subscribes to host pushes.
 *
 * `onReady` 在监听器**真正注册完成之后**才调用。
 *
 * 这一点很关键：`listen()` 是异步的，如果调用方在 `onPush()` 之后立刻
 * `send({type:'ready'})`，后端可能在监听器建立之前就把整批初始状态推出去，
 * 这些消息会被静默丢弃——表现就是图片永不显示（`blobsRoot` 始终为 null）、
 * 列表空白。所以把"可以发 ready 了"这件事交给 here 回调。
 */
export function onPush(
  handler: (message: HostPush) => void,
  onReady?: () => void,
): () => void {
  if (!isTauri()) {
    console.debug('[bridge] running in browser, seeding dev data')
    seedDevData(handler)
    onReady?.()
    return () => {}
  }

  let unlisten: UnlistenFn | null = null
  let cancelled = false

  listen<HostPush>('host-push', (event) => {
    handler(event.payload)
  })
    .then((fn) => {
      if (cancelled) {
        fn()
        return
      }
      unlisten = fn
      // 监听器就绪后才通知前端可以请求初始数据
      onReady?.()
    })
    .catch((error) => {
      console.error('[bridge] failed to listen to host-push:', error)
    })

  return () => {
    cancelled = true
    unlisten?.()
  }
}

/** Fills the UI with sample entries so `npm run dev` in a plain browser shows something. */
function seedDevData(handler: (message: HostPush) => void): void {
  const samples = [
    'https://github.com/tauri-apps/tauri',
    'npm run tauri dev',
    'SELECT Id, Type, Content FROM ClipboardItems ORDER BY CreatedAt DESC',
    'The quick brown fox jumps over the lazy dog',
    '好记性不如烂笔头',
    'git rebase --interactive HEAD~3',
    'C:\\Users\\Pyzl\\Documents\\report.pdf',
    '#3b82f6',
  ]

  const items = Array.from({ length: 40 }, (_, index) => {
    const text = samples[index % samples.length]
    const isFile = text.includes('\\')
    return {
      id: index + 1,
      type: isFile ? ('Files' as const) : ('Text' as const),
      content: `[${String(index + 1).padStart(2, '0')}] ${text}`,
      preview: `[${String(index + 1).padStart(2, '0')}] ${text}`,
      hasImage: false,
      createdAtLabel: '2026-10-16 12:00',
      pinned: index === 2 || index === 15,
      tag: index === 2 ? '代码' : index === 5 ? '常用' : null,
      truncated: false,
    }
  }).reverse()

  setTimeout(() => {
    handler({
      type: 'items',
      items,
      isEmpty: false,
      totalCount: items.length,
      allTags: ['代码', '常用'],
      activeTag: null,
    })
    handler({
      type: 'selection',
      selection: { itemId: items[0]?.id ?? null, canPrev: false, canNext: true },
    })
    handler({ type: 'hotkey', label: 'Ctrl + Shift + V' })
  }, 80)
}
