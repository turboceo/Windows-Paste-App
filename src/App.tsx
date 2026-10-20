import { useCallback, useEffect, useRef, useState } from 'react'
import TitleBar from './components/TitleBar'
import CardGrid from './components/CardGrid'
import ToolBar from './components/ToolBar'
import ContextMenu, { type ContextMenuState } from './components/ContextMenu'
import ConfirmDialog from './components/ConfirmDialog'
import Toast, { type ToastState } from './components/Toast'
import { getCurrentWebview } from '@tauri-apps/api/webview'
import { isHosted, onPush, send } from './bridge'
import type { ClipboardItem, HostPush, ImportPreview } from './types'

/** 搜索防抖窗口：连续输入合并成一次后端查询。 */
const SEARCH_DEBOUNCE_MS = 120

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
  /** blob 目录绝对路径，图片卡片用它拼 asset URL */
  const [blobsRoot, setBlobsRoot] = useState<string | null>(null)

  // 分类 Tag 状态
  const [allTags, setAllTags] = useState<string[]>([])
  const [activeTag, setActiveTag] = useState<string | null>(null)

  // toast 与导入确认
  const [toast, setToast] = useState<ToastState>({ visible: false, message: '' })
  /** 热键注册失败时的常驻警告；空字符串表示没有 */
  const [hotkeyWarning, setHotkeyWarning] = useState('')
  const [pendingImport, setPendingImport] = useState<{
    path: string
    preview: ImportPreview
  } | null>(null)
  /** 清空历史是不可撤销的破坏性操作，必须先确认 */
  const [pendingClear, setPendingClear] = useState(false)

  // 右键上下文菜单状态
  const [contextMenu, setContextMenu] = useState<ContextMenuState>({
    visible: false,
    x: 0,
    y: 0,
    item: null,
  })

  const searchTextRef = useRef(searchText)
  searchTextRef.current = searchText

  // 输入法组合期间不触发查询：拼音还没上屏就逐字母查，中文字打一个词会白跑好几次全表扫描
  const composingRef = useRef(false)
  const debounceRef = useRef<number | null>(null)
  const toastTimerRef = useRef<number | null>(null)

  const showToast = useCallback((message: string) => {
    if (!message) return
    if (toastTimerRef.current !== null) {
      window.clearTimeout(toastTimerRef.current)
    }
    setToast({ visible: true, message })
    toastTimerRef.current = window.setTimeout(() => {
      setToast((prev) => ({ ...prev, visible: false }))
      toastTimerRef.current = null
    }, 3200)
  }, [])

  // 清理定时器，避免面板隐藏后仍在跑
  useEffect(() => {
    return () => {
      if (debounceRef.current !== null) window.clearTimeout(debounceRef.current)
      if (toastTimerRef.current !== null) window.clearTimeout(toastTimerRef.current)
    }
  }, [])

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
          showToast(message.message)
          break
        case 'importPreview':
          if (message.error) {
            showToast(`无法导入：${message.error}`)
          } else if (message.preview) {
            setPendingImport({ path: message.path, preview: message.preview })
          }
          break
        case 'paths':
          // 用 state 而不是模块变量：宿主下发路径时必须触发一次重渲染，
          // 否则卡片停在图片占位状态。
          setBlobsRoot(message.blobsRoot)
          break
        case 'reset':
          // 宿主在唤出面板时清空搜索与分类，前端必须跟着归零
          setSearchText('')
          setActiveTag(null)
          break
        case 'hotkeyUnavailable':
          // 热键被占用：一直挂在界面上，而不是 3 秒就消失
          setHotkeyWarning(message.message)
          break
      }
    }, () => {
      // 监听器就绪后再请求初始数据，否则整批初始推送会丢
      send({ type: 'ready' })
    })

    return unsubscribe
  }, [showToast])

  /** 输入框变化：立即更新本地值，但请求延后 120ms 合并。 */
  const onSearch = useCallback((value: string) => {
    setSearchText(value)

    if (debounceRef.current !== null) {
      window.clearTimeout(debounceRef.current)
      debounceRef.current = null
    }

    // 输入法组合中不查库，等 compositionend 再发
    if (composingRef.current) return

    debounceRef.current = window.setTimeout(() => {
      send({ type: 'search', text: value })
      debounceRef.current = null
    }, SEARCH_DEBOUNCE_MS)
  }, [])

  /** 输入法上屏后立刻补发一次查询，避免等满防抖窗口。 */
  const onCompositionEnd = useCallback(
    (value: string) => {
      composingRef.current = false
      if (debounceRef.current !== null) {
        window.clearTimeout(debounceRef.current)
        debounceRef.current = null
      }
      send({ type: 'search', text: value })
    },
    [],
  )

  const onExport = useCallback(() => {
    send({ type: 'export', path: null })
  }, [])

  const onRequestImport = useCallback((path: string) => {
    send({ type: 'previewImport', path })
  }, [])

  /**
   * 点"导入"时选文件。
   *
   * 不用 `<input type=file>`：webview 里拿不到真实文件系统路径，后端无法据此读文件。
   * 文件对话框由宿主用 Win32 `GetOpenFileNameW` 弹出，选完直接把路径回传给前端。
   */
  const onPickImportFile = useCallback(() => {
    if (!isHosted()) {
      showToast('导入需要在桌面应用里使用')
      return
    }
    send({ type: 'pickImportFile' })
  }, [showToast])

  const onRequestClearAll = useCallback(() => {
    setPendingClear(true)
  }, [])

  const onConfirmClearAll = useCallback(() => {
    send({ type: 'clearAll' })
    setPendingClear(false)
  }, [])

  /**
   * 把 `.json` 拖进面板即视为导入请求。
   *
   * Tauri 的 drag-drop 事件会给出真实文件系统路径，比 `<input type=file>`
   * 可靠——后者在 webview 里被沙箱挡掉了路径。
   */
  useEffect(() => {
    if (!isHosted()) return

    let unlisten: (() => void) | undefined
    let disposed = false

    getCurrentWebview()
      .onDragDropEvent((event) => {
        if (event.payload.type !== 'drop') return
        const dropped = event.payload.paths?.find((p) => p.toLowerCase().endsWith('.json'))
        if (dropped) {
          onRequestImport(dropped)
        } else if (event.payload.paths?.length) {
          showToast('只支持导入 .json 导出包')
        }
      })
      .then((fn) => {
        if (disposed) fn()
        else unlisten = fn
      })
      .catch(() => {
        // 拖放不可用不影响其它功能
      })

    return () => {
      disposed = true
      unlisten?.()
    }
  }, [onRequestImport, showToast])

  const onConfirmImport = useCallback(() => {
    if (!pendingImport) return
    // 先关对话框，导入结果通过 toast 回报。
    // 失败时后端只会推 toast，不会重新打开这个对话框。
    send({ type: 'import', path: pendingImport.path })
    setPendingImport(null)
  }, [pendingImport])

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

      // 输入法组合中（拼音未上屏）不要抢键：Enter 是在选候选词，
      // 方向键是在选候选词，都不应该触发粘贴 / 移动选中项。
      if (composingRef.current && event.key !== 'Escape') {
        return
      }

      // 导入 / 清空对话框都是模态的：只让 Esc 生效，其余按键（尤其是 Enter）
      // 不应该穿透到下面的卡片上。
      if ((pendingImport || pendingClear) && event.key !== 'Escape') {
        return
      }

      switch (event.key) {
        case 'Escape':
          if (pendingClear) {
            setPendingClear(false)
            return
          }
          if (pendingImport) {
            setPendingImport(null)
            return
          }
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
        // 绝对值 > 1 表示"按行走"，具体跨几张卡由后端的 columns 决定。
        // 之前这里写的是 ±4，窗口列数一变方向键就错位。
        case 'ArrowUp':
          if (!typing) {
            event.preventDefault()
            send({ type: 'move', delta: -2 })
          }
          break
        case 'ArrowDown':
          if (!typing) {
            event.preventDefault()
            send({ type: 'move', delta: 2 })
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
  }, [
    selectedId,
    onPrev,
    onNext,
    contextMenu.visible,
    onCloseContextMenu,
    pendingImport,
    pendingClear,
  ])

  return (
    <div className="shell">
      <div className="panel">
        {/* 1. 顶部无边框标题栏 */}
        <TitleBar
          isMaximized={isMaximized}
          totalCount={totalCount}
          filteredCount={items.length}
        />

        {/* 热键不可用时的常驻提示：这个失败必须让人看见 */}
        {hotkeyWarning && (
          <div className="hotkey-warning" role="alert">
            <span className="hotkey-warning-glyph">&#xE7BA;</span>
            <span className="hotkey-warning-text">{hotkeyWarning}</span>
            <button
              className="hotkey-warning-close"
              aria-label="关闭提示"
              onClick={() => setHotkeyWarning('')}
            >
              &#xE711;
            </button>
          </div>
        )}

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
            blobsRoot={blobsRoot}
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
          onCompositionStart={() => {
            composingRef.current = true
          }}
          onCompositionEnd={onCompositionEnd}
          onPrev={onPrev}
          onNext={onNext}
          totalCount={totalCount}
          onImport={onPickImportFile}
          onExport={onExport}
          onClearAll={onRequestClearAll}
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

        {/* 6. 导入确认：先告诉用户会发生什么，再写库 */}
        <ConfirmDialog
          open={pendingImport !== null}
          title="导入剪贴板历史"
          confirmLabel="导入"
          onCancel={() => setPendingImport(null)}
          onConfirm={onConfirmImport}
        >
          {pendingImport && (
            <>
              <p className="dialog-line">
                这个文件包含 <b>{pendingImport.preview.total}</b> 条记录，导出于{' '}
                {formatTimestamp(pendingImport.preview.exportedAt)}。
              </p>
              <p className="dialog-line">
                其中 <b>{pendingImport.preview.fresh}</b> 条会被新增，
                <b>{pendingImport.preview.existing}</b> 条已存在（只合并置顶与分类，不重复插入）。
              </p>
              <p className="dialog-hint">导入是幂等的：同一个文件导入多次，结果与导入一次相同。</p>
            </>
          )}
        </ConfirmDialog>

        {/* 7. 清空历史：不可撤销，必须二次确认 */}
        <ConfirmDialog
          open={pendingClear}
          title="清空全部历史记录？"
          confirmLabel="清空"
          onCancel={() => setPendingClear(false)}
          onConfirm={onConfirmClearAll}
        >
          <p className="dialog-line">
            将删除全部 <b>{totalCount}</b> 条记录，以及已缓存的图片文件。
          </p>
          <p className="dialog-hint">
            这个操作无法撤销。如果只是想让列表清爽一些，可以先「导出」一份备份。
          </p>
        </ConfirmDialog>

        {/* 8. 轻量 toast，替代静默失败 */}
        <Toast toast={toast} />
      </div>
    </div>
  )
}

/** 把后端给的 RFC3339 时间缩成 `2026-10-20 14:03`，解析失败就原样返回。 */
function formatTimestamp(value: string): string {
  const date = new Date(value)
  if (Number.isNaN(date.getTime())) return value
  const pad = (n: number) => String(n).padStart(2, '0')
  return `${date.getFullYear()}-${pad(date.getMonth() + 1)}-${pad(date.getDate())} ${pad(
    date.getHours(),
  )}:${pad(date.getMinutes())}`
}
