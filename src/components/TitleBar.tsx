import { send } from '../bridge'

interface TitleBarProps {
  isMaximized: () => boolean
  totalCount: () => number
  filteredCount: () => number
  onOpenGuide?: () => void
  onOpenSettings?: () => void
}

/**
 * Frameless-window title bar with the app brand, count badge and window controls.
 *
 * Solid 版本：随状态变化的值全部以 accessor 传入，模板里直接调用。
 * `prop={expr}` 会被编译成细粒度绑定，只更新对应的文本节点，
 * 不会像 React 那样重渲染整个标题栏。
 */
export default function TitleBar(props: TitleBarProps) {
  const isFiltering = () => props.filteredCount() !== props.totalCount()

  const badgeText = () => {
    const total = props.totalCount()
    if (total === 0) return '0 项'
    return isFiltering() ? `${props.filteredCount()} / ${total}` : `${total} 项`
  }

  const badgeTitle = () =>
    isFiltering()
      ? `已筛选 ${props.filteredCount()} 项（共 ${props.totalCount()} 项）`
      : `当前共存储 ${props.totalCount()} 条剪贴板历史`

  return (
    <header
      class="titlebar"
      onMouseDown={(event) => {
        if (event.button === 0 && !(event.target as HTMLElement).closest('.window-control-btn')) {
          send({ type: 'dragMove' })
        }
      }}
      onDblClick={(event) => {
        if (!(event.target as HTMLElement).closest('.window-control-btn')) {
          send({ type: 'toggleMaximize' })
        }
      }}
    >
      <div class="brand">
        <span class="brand-icon">&#xE8A5;</span>
        <span class="brand-title">剪贴板历史</span>
        <span class={["count-badge", { filtering: isFiltering() }]} title={badgeTitle()}>
          {badgeText()}
        </span>
      </div>

      <div class="drag-region" />

      <div class="window-controls">
        <button
          class="window-control-btn help-btn"
          title="使用指南"
          onClick={(event) => {
            event.stopPropagation()
            props.onOpenGuide?.()
          }}
        >
          &#xE897;
        </button>

        <button
          class="window-control-btn settings-btn"
          title="设置与关于"
          onClick={(event) => {
            event.stopPropagation()
            props.onOpenSettings?.()
          }}
        >
          &#xE713;
        </button>

        <button
          class="window-control-btn"
          title="最小化"
          onClick={(event) => {
            event.stopPropagation()
            send({ type: 'minimize' })
          }}
        >
          &#xE921;
        </button>

        <button
          class="window-control-btn"
          title={props.isMaximized() ? '还原' : '最大化'}
          onClick={(event) => {
            event.stopPropagation()
            send({ type: 'toggleMaximize' })
          }}
        >
          {props.isMaximized() ? '\uE923' : '\uE922'}
        </button>

        <button
          class="window-control-btn btn-close"
          title="关闭 (Esc)"
          onClick={(event) => {
            event.stopPropagation()
            send({ type: 'close' })
          }}
        >
          &#xE8BB;
        </button>
      </div>
    </header>
  )
}
