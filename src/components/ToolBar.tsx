interface ToolBarProps {
  searchText: string
  canPrev: boolean
  canNext: boolean
  hotkeyLabel: string
  /** 历史总条数，用于在空库时禁用"清空" */
  totalCount: number
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
export default function ToolBar({
  searchText,
  canPrev,
  canNext,
  hotkeyLabel,
  totalCount,
  onSearchChange,
  onCompositionStart,
  onCompositionEnd,
  onPrev,
  onNext,
  onImport,
  onExport,
  onClearAll,
}: ToolBarProps) {
  const placeholderVisible = searchText.length === 0

  return (
    <div className="toolbar">
      <div className="search">
        <span className="search-glyph">&#xE721;</span>
        <input
          className="search-input"
          type="text"
          spellCheck={false}
          autoComplete="off"
          value={searchText}
          onChange={(event) => onSearchChange(event.target.value)}
          onCompositionStart={onCompositionStart}
          onCompositionEnd={(event) => onCompositionEnd(event.currentTarget.value)}
        />
        {placeholderVisible && <span className="search-placeholder">搜索剪贴板记录...</span>}
        {!placeholderVisible && (
          <button className="clear-btn" title="清空搜索" onClick={() => onSearchChange('')}>
            &#xE711;
          </button>
        )}
      </div>

      <div className="shortcuts">
        <span className="shortcut-tag">
          <kbd>Enter</kbd> 粘贴
        </span>
        <span className="shortcut-tag">
          <kbd>Ctrl+P</kbd> 置顶
        </span>
        <span className="shortcut-tag">
          <kbd>Del</kbd> 删除
        </span>
        <span className="shortcut-tag">
          <kbd>Esc</kbd> 关闭
        </span>
      </div>

      <div className="data-actions">
        <button
          className="text-action"
          title="导入之前备份的 .json（也可以直接把文件拖进窗口）"
          onClick={onImport}
        >
          <span className="action-glyph">&#xE8B5;</span>
          导入
        </button>
        <button
          className="text-action"
          title="把全部历史导出为单个 .json 备份文件"
          onClick={onExport}
        >
          <span className="action-glyph">&#xE896;</span>
          导出
        </button>
        <button
          className="text-action danger"
          title="清空全部历史记录（不可撤销）"
          onClick={onClearAll}
          disabled={totalCount === 0}
        >
          <span className="action-glyph">&#xE74D;</span>
          清空
        </button>
      </div>

      <div className="nav">
        <button className="nav-btn" disabled={!canPrev} title={`上一项 (Left / Up) ${hotkeyLabel}`} onClick={onPrev}>
          &#xE76B;
        </button>
        <button className="nav-btn" disabled={!canNext} title={`下一项 (Right / Down) ${hotkeyLabel}`} onClick={onNext}>
          &#xE76C;
        </button>
      </div>
    </div>
  )
}
