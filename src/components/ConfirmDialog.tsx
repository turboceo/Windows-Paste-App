import { useEffect, useRef, type ReactNode } from 'react'

interface ConfirmDialogProps {
  open: boolean
  title: string
  confirmLabel: string
  cancelLabel?: string
  /**
   * 破坏性操作（删除、清空）。
   *
   * 主按钮会变红。这不只是装饰：蓝色主按钮的语义是"继续/确认"，
   * 而删除是不可撤销的，颜色要先把后果说出来。所有删除类确认框都用它，
   * 保证同样的危险操作在不同入口看起来一致。
   */
  danger?: boolean
  children: ReactNode
  onConfirm: () => void
  onCancel: () => void
}

/**
 * 轻量的模态确认框。
 *
 * 用在"导入 / 清空 / 删除分类"这类不可撤销、但影响面又不大的操作上：
 * 先把后果说清楚，再让用户点确认，而不是点一下菜单就直接改数据库。
 *
 * 破坏性操作用 `danger`，主按钮会变红。三个入口（清空历史、删除分类）
 * 共用同一个标记，保证同样的危险操作在任何地方看起来都一致。
 */
export default function ConfirmDialog({
  open,
  title,
  confirmLabel,
  cancelLabel = '取消',
  danger = false,
  children,
  onConfirm,
  onCancel,
}: ConfirmDialogProps) {
  const confirmRef = useRef<HTMLButtonElement | null>(null)

  useEffect(() => {
    if (!open) return
    // 打开时把焦点放到确认按钮，键盘可以直接回车
    confirmRef.current?.focus()
  }, [open])

  if (!open) return null

  return (
    <div className="dialog-backdrop" onMouseDown={onCancel}>
      <div
        className="dialog"
        role="dialog"
        aria-modal="true"
        aria-label={title}
        onMouseDown={(event) => event.stopPropagation()}
      >
        <h2 className="dialog-title">{title}</h2>
        <div className="dialog-body">{children}</div>

        <div className="dialog-actions">
          <button className="dialog-btn" onClick={onCancel}>
            {cancelLabel}
          </button>
          <button
            ref={confirmRef}
            className={danger ? 'dialog-btn primary danger' : 'dialog-btn primary'}
            onClick={onConfirm}
          >
            {confirmLabel}
          </button>
        </div>
      </div>
    </div>
  )
}
