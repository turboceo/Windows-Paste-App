export interface ToastState {
  visible: boolean
  message: string
}

interface ToastProps {
  toast: ToastState
}

/**
 * 底部轻量提示。
 *
 * 存在的意义是替换掉"静默失败"：导出成功、导入失败、热键被占用
 * 这类结果必须有个可见的回执，否则用户只会觉得软件坏了。
 */
export default function Toast({ toast }: ToastProps) {
  if (!toast.visible || !toast.message) return null

  return (
    <div className="toast" role="status" aria-live="polite">
      {toast.message}
    </div>
  )
}
