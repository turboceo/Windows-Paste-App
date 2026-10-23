import { Show } from 'solid-js'

interface ToolBarProps {
  searchText: () => string
  canPrev: () => boolean
  canNext: () => boolean
  hotkeyLabel: () => string
  /** 历史总条数，用于在空库时禁用"清空" */
  totalCount: () => number
  onSearchChange: (value: string) => void
  onCompositionStart: () => void
  onCompositionEnd: (value: string) => void
  onPrev: () => void
  onNext: () => void
  onImport: () => void
  onExport: () => void
  onClearAll: () => void
}

/**
 * Bottom bar：搜索框、快捷键提示、数据操作与上/下一项导航。
 *
 * 导入 / 导出 / 清空都放在这里而不是托盘右键菜单——它们低频、需要确认，
 * 也更应该出现在用户正看着的这个界面上。
 */
export default function ToolBar(props: ToolBarProps) {
  const placeholderVisible = () => props.searchText().length === 0

  return (
    <div class="toolbar">
      <div class="search">
        <span class="search-glyph">&#xE721;</span>
        <input
          class="search-input"
          type="text"
          spellcheck={false}
          autocomplete="off"
          value={props.searchText()}
          onInput={(event) => props.onSearchChange(event.currentTarget.value)}
          onCompositionStart={() => props.onCompositionStart()}
          onCompositionEnd={(event) => props.onCompositionEnd(event.currentTarget.value)}
        />
        <Show when={placeholderVisible()}>
          <span class="search-placeholder">搜索剪贴板记录...</span>
        </Show>
        <Show when={!placeholderVisible()}>
          <button class="clear-btn" title="清空搜索" onClick={() => props.onSearchChange('')}>
            &#xE711;
          </button>
        </Show>
      </div>

      <div class="shortcuts">
        <span class="shortcut-tag">
          <kbd>Enter</kbd> 粘贴
        </span>
        <span class="shortcut-tag">
          <kbd>Ctrl+P</kbd> 置顶
        </span>
        <span class="shortcut-tag">
          <kbd>Del</kbd> 删除
        </span>
        <span class="shortcut-tag">
          <kbd>Esc</kbd> 关闭
        </span>
      </div>

      <div class="data-actions">
        <button
          class="text-action"
          title="导入之前备份的 .json（也可以直接把文件拖进窗口）"
          onClick={() => props.onImport()}
        >
          <span class="action-glyph">&#xE8B5;</span>
          导入
        </button>
        <button
          class="text-action"
          title="把全部历史导出为单个 .json 备份文件"
          onClick={() => props.onExport()}
        >
          <span class="action-glyph">&#xE896;</span>
          导出
        </button>
        <button
          class="text-action danger"
          title="清空全部历史记录（不可撤销）"
          disabled={props.totalCount() === 0}
          onClick={() => props.onClearAll()}
        >
          <span class="action-glyph">&#xE74D;</span>
          清空
        </button>
      </div>

      <div class="nav">
        <button
          class="nav-btn"
          disabled={!props.canPrev()}
          title={`上一项 (Left / Up) ${props.hotkeyLabel()}`}
          onClick={() => props.onPrev()}
        >
          &#xE76B;
        </button>
        <button
          class="nav-btn"
          disabled={!props.canNext()}
          title={`下一项 (Right / Down) ${props.hotkeyLabel()}`}
          onClick={() => props.onNext()}
        >
          &#xE76C;
        </button>
      </div>
    </div>
  )
}
