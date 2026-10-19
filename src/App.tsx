import { useCallback, useEffect, useRef, useState } from 'react'
import TitleBar from './components/TitleBar'
import CardGrid from './components/CardGrid'
import ToolBar from './components/ToolBar'
import ContextMenu, { type ContextMenuState } from './components/ContextMenu'
import { onPush, send } from './bridge'
import type { ClipboardItem, HostPush } from './types'

export default function App() {
  const [items, setItems] = useState<ClipboardItem[]>([])
  const [isEmpty, setIsEmpty] = useState(true)
  const [selectedId, setSelectedId] = useState<number | null>(null)
  const [canPrev, setCanPrev] = useState(false)
  const [canNext, setCanNext] = useState(false)
  const [searchText, setSearchText] = useState('')
  const [hotkeyLabel, setHotkeyLabel] = useState('')
  const [isMaximized, setIsMaximized] = useState(false)
  const [totalCount, setTotalCount] = useState(0)

  // 分类 Tag 状态
  const [allTags, setAllTags] = useState<string[]>([])
  const [activeTag, setActiveTag] = useState<string | null>(null)

  // 右键上下文菜单状态
  const [contextMenu, setContextMenu] = useState<ContextMenuState>({
    visible: false,
    x: 0,
    y: 0,
    item: null,
  })

  const searchTextRef = useRef(searchText)
  searchTextRef.current = searchText

  useEffect(() => {
    const unsubscribe = onPush((message: HostPush) => {
      switch (message.type) {
        case 'items':
          setItems(message.items)
          setIsEmpty(message.isEmpty)
          if (typeof message.totalCount === 'number') {
            setTotalCount(message.totalCount)
          } else if (!searchTextRef.current) {
            setTotalCount(message.items.length)
          }
          if (Array.isArray(message.allTags)) {
            setAllTags(message.allTags)
          }
          if (message.activeTag !== undefined) {
            setActiveTag(message.activeTag ?? null)
          }
          break
        case 'selection':
          setSelectedId(message.selection.itemId)
          setCanPrev(message.selection.canPrev)
          setCanNext(message.selection.canNext)
          break
        case 'hotkey':
          setHotkeyLabel(message.label)
          break
        case 'windowState':
          setIsMaximized(message.isMaximized)
          break
        case 'toast':
          // eslint-disable-next-line no-console
          console.info('[host]', message.message)
          break
      }
    })

    send({ type: 'ready' })
    return unsubscribe
  }, [])

  const onSearch = useCallback((value: string) => {
    setSearchText(value)
    send({ type: 'search', text: value })
  }, [])

  const onSelect = useCallback((item: ClipboardItem) => {
    setSelectedId(item.id)
    send({ type: 'select', itemId: item.id })
  }, [])

  const onPaste = useCallback((item: ClipboardItem) => {
    send({ type: 'paste', itemId: item.id })
  }, [])

  const onTogglePin = useCallback((item: ClipboardItem) => {
    send({ type: 'togglePin', itemId: item.id })
  }, [])

  const onRemove = useCallback((item: ClipboardItem) => {
    send({ type: 'delete', itemId: item.id })
  }, [])

  const onSetTag = useCallback((item: ClipboardItem, tag: string | null) => {
    send({ type: 'setTag', itemId: item.id, tag })
  }, [])

  const onFilterTag = useCallback((tag: string | null) => {
    setActiveTag(tag)
    send({ type: 'filterTag', tag })
  }, [])

  const onContextMenu = useCallback((item: ClipboardItem, event: React.MouseEvent) => {
    setContextMenu({
      visible: true,
      x: event.clientX,
      y: event.clientY,
      item,
    })
  }, [])

  const onCloseContextMenu = useCallback(() => {
    setContextMenu((prev) => ({ ...prev, visible: false }))
  }, [])

  const onPrev = useCallback(() => send({ type: 'move', delta: -1 }), [])
  const onNext = useCallback(() => send({ type: 'move', delta: 1 }), [])

  // 全局键盘交互处理 (Enter/Del/Ctrl+P/Esc/方向键)
  useEffect(() => {
    function onKeyDown(event: KeyboardEvent): void {
      const target = event.target as HTMLElement | null
      const typing = target?.tagName === 'INPUT' || target?.tagName === 'TEXTAREA'

      switch (event.key) {
        case 'Escape':
          if (contextMenu.visible) {
            onCloseContextMenu()
            return
          }
          event.preventDefault()
          send({ type: 'close' })
          break
        case 'Enter':
          if (!typing && selectedId !== null) {
            event.preventDefault()
            send({ type: 'paste', itemId: selectedId })
          }
          break
        case 'Delete':
          if (!typing && selectedId !== null) {
            event.preventDefault()
            send({ type: 'delete', itemId: selectedId })
          }
          break
        case 'ArrowLeft':
          if (!typing) {
            event.preventDefault()
            onPrev()
          }
          break
        case 'ArrowRight':
          if (!typing) {
            event.preventDefault()
            onNext()
          }
          break
        case 'ArrowUp':
          if (!typing) {
            event.preventDefault()
            send({ type: 'move', delta: -4 })
          }
          break
        case 'ArrowDown':
          if (!typing) {
            event.preventDefault()
            send({ type: 'move', delta: 4 })
          }
          break
        case 'p':
        case 'P':
          if (event.ctrlKey && !typing && selectedId !== null) {
            event.preventDefault()
            send({ type: 'togglePin', itemId: selectedId })
          }
          break
      }
    }

    window.addEventListener('keydown', onKeyDown)
    return () => window.removeEventListener('keydown', onKeyDown)
  }, [selectedId, onPrev, onNext, contextMenu.visible, onCloseContextMenu])

  return (
    <div className="shell">
      <div className="panel">
        {/* 1. 顶部无边框标题栏 */}
        <TitleBar
          isMaximized={isMaximized}
          totalCount={totalCount}
          filteredCount={items.length}
        />

        {/* 2. 分类 Tag 快捷切换标签栏 */}
        <div className="tag-filter-bar">
          <button
            className={`tag-filter-pill${activeTag === null ? ' active' : ''}`}
            onClick={() => onFilterTag(null)}
          >
            全部
          </button>
          {allTags.map((tag) => (
            <button
              key={tag}
              className={`tag-filter-pill${activeTag === tag ? ' active' : ''}`}
              onClick={() => onFilterTag(activeTag === tag ? null : tag)}
            >
              <span className="tag-glyph">&#xE8EC;</span>
              {tag}
            </button>
          ))}
        </div>

        {/* 3. 虚拟卡片网格列表 */}
        <div className="list-area">
          <CardGrid
            items={items}
            selectedId={selectedId}
            onSelect={onSelect}
            onPaste={onPaste}
            onTogglePin={onTogglePin}
            onRemove={onRemove}
            onContextMenu={onContextMenu}
          />
          {isEmpty && (
            <div className="empty">
              <span className="empty-glyph">&#xE8C5;</span>
              <p className="empty-title">
                {activeTag ? `分类 [${activeTag}] 暂无记录` : '剪贴板历史为空'}
              </p>
              <span className="empty-desc">
                {activeTag ? '右键任意卡片即可选择加入此分类' : '复制文本、图片或文件后将自动显示在这里'}
              </span>
            </div>
          )}
        </div>

        {/* 4. 底部搜索栏与快捷键说明 */}
        <ToolBar
          searchText={searchText}
          canPrev={canPrev}
          canNext={canNext}
          hotkeyLabel={hotkeyLabel}
          onSearchChange={onSearch}
          onPrev={onPrev}
          onNext={onNext}
        />

        {/* 5. 原生风格右键上下文菜单 */}
        <ContextMenu
          menu={contextMenu}
          allTags={allTags}
          onClose={onCloseContextMenu}
          onPaste={onPaste}
          onTogglePin={onTogglePin}
          onRemove={onRemove}
          onSetTag={onSetTag}
        />
      </div>
    </div>
  )
}
