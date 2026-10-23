import { Show, createEffect } from 'solid-js'

interface OcrTextViewerProps {
  open: boolean
  /** 识别出的文字原文 */
  text: () => string
  /** 图片的尺寸描述，例如 `1920 x 1080 image`，用来给弹窗一个上下文 */
  imageLabel: () => string
  onCopy: (text: string) => void
  onClose: () => void
}

/**
 * 只读查看某张图片的 OCR 识别结果。
 *
 * 存在的理由：之前唯一的出口是「复制提取的文本」，文字直接进剪贴板，
 * 用户想看内容只能到别处粘贴——既不方便，也会顺手覆盖掉剪贴板里原来的东西。
 * 这里先展示，要不要复制由用户决定。
 */
export default function OcrTextViewer(props: OcrTextViewerProps) {
  // Esc 关闭。放在捕获阶段并 stopPropagation：
  // 否则事件会冒泡到 App 的全局键盘处理，把整个面板一起关掉。
  // 两段式 createEffect：compute 跟踪 open，apply 里挂/卸监听器并返回清理函数
  createEffect(
    () => props.open,
    (open) => {
      if (!open) return
      const handler = (event: KeyboardEvent) => {
        if (event.key === 'Escape') {
          event.stopPropagation()
          props.onClose()
        }
      }
      window.addEventListener('keydown', handler, true)
      return () => window.removeEventListener('keydown', handler, true)
    },
  )

  const charCount = () => [...props.text()].length
  const lineCount = () => props.text().split('\n').filter((line) => line.trim()).length

  return (
    <Show when={props.open}>
      <div class="dialog-backdrop" onClick={() => props.onClose()}>
        <div
          class="dialog ocr-viewer"
          role="dialog"
          aria-modal="true"
          aria-label="图片识别文字"
          onClick={(event) => event.stopPropagation()}
        >
          <div class="ocr-viewer-head">
            <div class="ocr-viewer-titles">
              <h2 class="dialog-title ocr-viewer-title">
                <span class="ocr-viewer-glyph">&#xE8C1;</span>
                识别到的文字
              </h2>
              <span class="ocr-viewer-meta">
                {props.imageLabel()} · {lineCount()} 行 · {charCount()} 字符
              </span>
            </div>
            <button class="ocr-viewer-close" title="关闭 (Esc)" onClick={() => props.onClose()}>
              &#xE711;
            </button>
          </div>

          <pre class="ocr-viewer-text">{props.text()}</pre>

          <div class="ocr-viewer-actions">
            <span class="ocr-viewer-hint">识别结果只读，可直接选中部分文字</span>
            <div class="ocr-viewer-buttons">
              <button class="dialog-btn" onClick={() => props.onClose()}>
                关闭
              </button>
              <button
                class="dialog-btn primary"
                onClick={() => {
                  props.onCopy(props.text())
                  props.onClose()
                }}
              >
                复制全部
              </button>
            </div>
          </div>
        </div>
      </div>
    </Show>
  )
}
