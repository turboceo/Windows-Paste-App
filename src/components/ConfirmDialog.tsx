import { useEffect, useRef, type ReactNode } from 'react'

interface ConfirmDialogProps {
  open: boolean
  title: string
  confirmLabel: string
  cancelLabel?: string
  children: ReactNode
  onConfirm: () => void
  onCancel: () => void
}

/**
 * 轻量的模态确认框。
 *
 * 用在"导入"这类不可撤销、但影响面又不大的操作上：先把后果说清楚，
 * 再让用户点确认，而不是点一下菜单就直接改数据库。
 */
export default function ConfirmDialog({
  open,
  title,
  confirmLabel,
  cancelLabel = '取消',
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
          <button ref={confirmRef} className="dialog-btn primary" onClick={onConfirm}>
            {confirmLabel}
          </button>
        </div>
      </div>
    </div>
  )
}
