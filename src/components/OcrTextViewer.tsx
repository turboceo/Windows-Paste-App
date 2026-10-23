import { useEffect } from 'react'

interface OcrTextViewerProps {
  open: boolean
  /** 识别出的文字原文 */
  text: string
  /** 图片的尺寸描述，例如 `1920 x 1080 image`，用来给弹窗一个上下文 */
  imageLabel: string
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
export default function OcrTextViewer({
  open,
  text,
  imageLabel,
  onCopy,
  onClose,
}: OcrTextViewerProps) {
  useEffect(() => {
    if (!open) return
    function handleKeyDown(e: KeyboardEvent) {
      if (e.key === 'Escape') {
        // 阻止冒泡：不然 Esc 会顺手把整个面板关掉
        e.stopPropagation()
        onClose()
      }
    }
    window.addEventListener('keydown', handleKeyDown, true)
    return () => window.removeEventListener('keydown', handleKeyDown, true)
  }, [open, onClose])

  if (!open) return null

  const charCount = [...text].length
  const lineCount = text.split('\n').filter((line) => line.trim()).length

  return (
    <div className="dialog-backdrop" onClick={onClose}>
      <div
        className="dialog ocr-viewer"
        role="dialog"
        aria-modal="true"
        aria-label="图片识别文字"
        onClick={(event) => event.stopPropagation()}
      >
        <div className="ocr-viewer-head">
          <div className="ocr-viewer-titles">
            <h2 className="dialog-title ocr-viewer-title">
              <span className="ocr-viewer-glyph">&#xE8C1;</span>
              识别到的文字
            </h2>
            <span className="ocr-viewer-meta">
              {imageLabel} · {lineCount} 行 · {charCount} 字符
            </span>
          </div>
          <button className="ocr-viewer-close" title="关闭 (Esc)" onClick={onClose}>
            &#xE711;
          </button>
        </div>

        <pre className="ocr-viewer-text">{text}</pre>

        <div className="ocr-viewer-actions">
          <span className="ocr-viewer-hint">识别结果只读，可直接选中部分文字</span>
          <div className="ocr-viewer-buttons">
            <button className="dialog-btn" onClick={onClose}>
              关闭
            </button>
            <button
              className="dialog-btn primary"
              onClick={() => {
                onCopy(text)
                onClose()
              }}
            >
              复制全部
            </button>
          </div>
        </div>
      </div>
    </div>
  )
}
