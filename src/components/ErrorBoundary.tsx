import React from 'react'
import { reportFrontendError } from '../bridge'

interface ErrorBoundaryState {
  error: Error | null
}

/**
 * 渲染错误的最后一道防线。
 *
 * 没有它的时候，任何一个组件在渲染中抛错，React 都会卸载整棵树——
 * 用户看到的就是"一块干净底色、什么都没有"的白屏，而且会一直白下去
 * （同一份 DOM 状态反复走到同一条抛错路径）。有了边界，至少能给出
 * 可操作的出口，并把堆栈写进 app.log。
 */
export default class ErrorBoundary extends React.Component<
  { children: React.ReactNode },
  ErrorBoundaryState
> {
  state: ErrorBoundaryState = { error: null }

  static getDerivedStateFromError(error: Error): ErrorBoundaryState {
    return { error }
  }

  componentDidCatch(error: Error, info: React.ErrorInfo): void {
    reportFrontendError(
      error.message,
      `${error.stack ?? ''}\n--- componentStack ---\n${info.componentStack ?? ''}`,
      'react-error-boundary',
    )
  }

  render(): React.ReactNode {
    if (!this.state.error) return this.props.children

    return (
      <div className="crash-screen">
        <div className="crash-box">
          <div className="crash-glyph">&#xE7BA;</div>
          <h2 className="crash-title">界面渲染出错了</h2>
          <p className="crash-desc">
            这一屏已经无法继续显示。你的剪贴板历史没有受影响，可以重新加载界面。
          </p>
          <pre className="crash-detail">{this.state.error.message}</pre>
          <div className="crash-actions">
            <button
              className="dialog-btn primary"
              onClick={() => {
                // 重载整个 WebView，等价于重开面板，但不用重启进程
                location.reload()
              }}
            >
              重新加载界面
            </button>
            <button
              className="dialog-btn"
              onClick={() => {
                this.setState({ error: null })
              }}
            >
              再试一次
            </button>
          </div>
          <p className="crash-hint">错误详情已写入 app.log，可在设置里打开存储目录查看。</p>
        </div>
      </div>
    )
  }
}
