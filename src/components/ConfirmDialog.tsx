import { Show, createEffect } from 'solid-js'
import type { JSX } from '@solidjs/web'

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
  children: JSX.Element
  onConfirm: () => void
  onCancel: () => void
}

/**
 * 轻量的模态确认框。
 *
 * 用在"导入 / 清空 / 删除分类"这类不可撤销、但影响面又不大的操作上：
 * 先把后果说清楚，再让用户点确认，而不是点一下菜单就直接改数据库。
 */
export default function ConfirmDialog(props: ConfirmDialogProps) {
  let confirmRef: HTMLButtonElement | undefined

  // 打开时把焦点放到确认按钮，键盘可以直接回车。
  // 两段式 createEffect：compute 跟踪 open，apply 里做副作用。
  createEffect(
    () => props.open,
    (open) => {
      if (!open) return
      // 等 DOM 插入后再聚焦，否则 ref 还是 undefined
      queueMicrotask(() => confirmRef?.focus())
    },
  )

  return (
    <Show when={props.open}>
      <div class="dialog-backdrop" onMouseDown={() => props.onCancel()}>
        <div
          class="dialog"
          role="dialog"
          aria-modal="true"
          aria-label={props.title}
          onMouseDown={(event) => event.stopPropagation()}
        >
          <h2 class="dialog-title">{props.title}</h2>
          <div class="dialog-body">{props.children}</div>

          <div class="dialog-actions">
            <button class="dialog-btn" onClick={() => props.onCancel()}>
              {props.cancelLabel ?? '取消'}
            </button>
            <button
              ref={confirmRef}
              class={["dialog-btn", "primary", { danger: props.danger }]}
              onClick={() => props.onConfirm()}
            >
              {props.confirmLabel}
            </button>
          </div>
        </div>
      </div>
    </Show>
  )
}
