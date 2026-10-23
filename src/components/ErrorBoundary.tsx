import { createSignal } from 'solid-js'
import type { JSX } from '@solidjs/web'
import { reportFrontendError } from '../bridge'

interface ErrorBoundaryProps {
  children: JSX.Element
}

/**
 * 渲染错误的兜底界面。
 *
 * 与 React 不同，Solid 没有"捕获子树后续更新错误"的边界机制：
 * 细粒度更新里的异常只会让那一处绑定失效，不会卸载整棵树。
 * 真正需要兜住的是**首次渲染**抛错——那会中断挂载，界面停在半成品。
 *
 * 所以这里直接 try/catch 包住子树（Solid 组件就是普通函数调用），
 * 捕获同步异常后渲染可操作的恢复界面，并把堆栈写进 app.log。
 * 运行期错误由 `installGlobalErrorHandlers` 负责记录（见 bridge.ts）。
 */
export default function ErrorBoundary(props: ErrorBoundaryProps) {
  const [error, setError] = createSignal<{ message: string } | null>(null)

  const render = (): JSX.Element => {
    // 已进入错误态：不再触碰子树，避免反复抛错
    const current = error()
    if (current) {
      return <CrashScreen message={current.message} onReset={() => setError(null)} />
    }

    try {
      return props.children
    } catch (err) {
      const e = err as Error
      const message = e?.message ?? String(err)
      reportFrontendError(message, e?.stack, 'solid-render')
      return <CrashScreen message={message} onReset={() => setError(null)} />
    }
  }

  return <>{render()}</>
}

function CrashScreen(props: { message: string; onReset: () => void }) {
  return (
    <div class="crash-screen">
      <div class="crash-box">
        <div class="crash-glyph">&#xE7BA;</div>
        <h2 class="crash-title">界面渲染出错了</h2>
        <p class="crash-desc">
          这一屏已经无法继续显示。你的剪贴板历史没有受影响，可以重新加载界面。
        </p>
        <pre class="crash-detail">{props.message}</pre>
        <div class="crash-actions">
          <button
            class="dialog-btn primary"
            onClick={() => {
              // 重载整个 WebView，等价于重开面板，但不用重启进程
              location.reload()
            }}
          >
            重新加载界面
          </button>
          <button class="dialog-btn" onClick={() => props.onReset()}>
            再试一次
          </button>
        </div>
        <p class="crash-hint">错误详情已写入 app.log，可在设置里打开存储目录查看。</p>
      </div>
    </div>
  )
}
