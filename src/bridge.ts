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

/**
 * 把前端异常送到后端日志。
 *
 * 前端跑在 WebView 里，`console.error` 只有打开 DevTools 才看得到；
 * 白屏时连界面都没了，用户能提供的线索等于零。所以这里把错误
 * 直接落到 `%LOCALAPPDATA%\WindowsPaste\app.log`，事后才有据可查。
 *
 * 刻意**不**用 `send()`：那条路径失败时会把错误再 `console.error` 一遍，
 * 在错误处理里递归调用自己。这里直接 invoke 并吞掉所有异常。
 */
export function reportFrontendError(message: string, stack?: string, source?: string): void {
  try {
    console.error(`[frontend] ${source ?? ''} ${message}`, stack ?? '')
  } catch {
    // console 本身不可用时也就算了
  }

  if (!isTauri()) return
  try {
    void invoke('dispatch', {
      req: {
        type: 'reportFrontendError',
        message: String(message).slice(0, 2000),
        stack: String(stack ?? '').slice(0, 4000),
        source: String(source ?? '').slice(0, 200),
      } satisfies HostRequest,
    }).catch(() => {})
  } catch {
    // 投递失败不能再抛，否则就成了错误处理里的错误
  }
}

/**
 * 全局兜底：未捕获错误与未处理的 Promise 拒绝。
 *
 * 在 `main.tsx` 里调用一次。它不负责恢复界面（那是 ErrorBoundary 的事），
 * 只负责留下痕迹。
 */
export function installGlobalErrorHandlers(): void {
  window.addEventListener('error', (event) => {
    // 资源加载失败（img/script）也走 error，但 event.error 为空，另行标注
    const err = event.error as Error | undefined
    reportFrontendError(
      err?.message ?? event.message ?? 'unknown error',
      err?.stack,
      event.filename ? `window.error @${event.filename}:${event.lineno}` : 'window.error',
    )
  })

  window.addEventListener('unhandledrejection', (event) => {
    const reason = event.reason
    const message =
      reason instanceof Error ? reason.message : typeof reason === 'string' ? reason : JSON.stringify(reason)
    reportFrontendError(
      message ?? 'unhandled rejection',
      reason instanceof Error ? reason.stack : undefined,
      'unhandledrejection',
    )
  })
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
      // 相对时间要能分出层次，这里按序号往回退几小时
      createdAt: new Date(Date.now() - (index + 1) * 37 * 60 * 1000).toISOString(),
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
    handler({
      type: 'paths',
      blobsRoot: 'C:\\Users\\Pyzl\\AppData\\Local\\WindowsPaste\\blobs',
      dataRoot: 'C:\\Users\\Pyzl\\AppData\\Local\\WindowsPaste',
      version: '0.2.0',
    })
  }, 80)
}
