interface ToolBarProps {
  searchText: string
  canPrev: boolean
  canNext: boolean
  hotkeyLabel: string
  onSearchChange: (value: string) => void
  onPrev: () => void
  onNext: () => void
}

/** Bottom bar: search box, shortcut hints and prev/next navigation. */
export default function ToolBar({
  searchText,
  canPrev,
  canNext,
  hotkeyLabel,
  onSearchChange,
  onPrev,
  onNext,
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
