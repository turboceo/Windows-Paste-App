import { Show, createEffect, createSignal } from 'solid-js'
import TitleBar from './components/TitleBar'
import TagFilterBar from './components/TagFilterBar'
import CardGrid from './components/CardGrid'
import ToolBar from './components/ToolBar'
import ContextMenu, { type ContextMenuState } from './components/ContextMenu'
import ConfirmDialog from './components/ConfirmDialog'
import Toast, { type ToastState } from './components/Toast'
import OnboardingGuide from './components/OnboardingGuide'
import SettingsDialog from './components/SettingsDialog'
import OcrTextViewer from './components/OcrTextViewer'
import { getCurrentWebview } from '@tauri-apps/api/webview'
import { isHosted, onPush, send } from './bridge'
import type { ClipboardItem, HostPush, ImportPreview } from './types'
import { loadTagColors, saveTagColors } from './tagColors'
import { extractOcrText, ocrBaseContent } from './ocr'
import { useWindowEvent, useTimeout, mount } from './solid-utils'

/** 搜索防抖窗口：连续输入合并成一次后端查询。 */
const SEARCH_DEBOUNCE_MS = 120

export default function App() {
  const [items, setItems] = createSignal<ClipboardItem[]>([])
  const [isEmpty, setIsEmpty] = createSignal(true)
  const [selectedId, setSelectedId] = createSignal<number | null>(null)
  const [canPrev, setCanPrev] = createSignal(false)
  const [canNext, setCanNext] = createSignal(false)
  const [searchText, setSearchText] = createSignal('')
  const [hotkeyLabel, setHotkeyLabel] = createSignal('')
  const [isMaximized, setIsMaximized] = createSignal(false)
  const [totalCount, setTotalCount] = createSignal(0)

  /**
   * 已拉取的缩略图，key 是条目 id。
   *
   * 单独放在这里而不是塞回 `items`，是为了让「列表数据」和「图片像素」
   * 两条更新路径分开：拉一张图只更新一处绑定，
   * 也不会因为整表替换把已加载的图丢掉。
   */
  const [thumbnails, setThumbnails] = createSignal<Record<number, string | null>>({})
  /** 已发起请求的 id，避免重复拉取 */
  const requestedThumbs = new Set<number>()

  // 分类 Tag 状态
  const [allTags, setAllTags] = createSignal<string[]>([])
  const [tagCounts, setTagCounts] = createSignal<Record<string, number>>({})
  const [activeTag, setActiveTag] = createSignal<string | null>(null)
  const [tagColors, setTagColors] = createSignal<Record<string, string>>(loadTagColors())
  /**
   * 在标签栏新建、但还没有任何条目引用的空分类。
   *
   * 数据库里没有 Tags 表——标签只是 `ClipboardItems.Tag` 这个字符串字段，
   * 由 `SELECT DISTINCT` 汇总出来（见 db.rs 的注释）。所以一个没人引用的
   * 标签在库里根本不存在，也就活不过重启。
   *
   * 但用户点「+」建完分类的那一刻，它必须**立刻**出现在标签栏和右键菜单里，
   * 否则界面就像"点了没反应"。因此这里在前端多留一份，
   * 与后端真实标签合并后展示；一旦有卡片用上它，后端推送就会把它接管过去，
   * 这份草稿随之失去意义（由下面的 effect 清理）。
   */
  const [pendingTags, setPendingTags] = createSignal<string[]>([])
  /** 标签栏与右键菜单实际展示的分类 = 后端真实标签 ∪ 前端空标签 */
  const visibleTags = () => {
    const merged = new Set(pendingTags())
    for (const tag of allTags()) merged.add(tag)
    return Array.from(merged)
  }

  // toast 与导入确认
  const [toast, setToast] = createSignal<ToastState>({ visible: false, message: '' })
  /** 热键注册失败时的常驻警告；空字符串表示没有 */
  const [hotkeyWarning, setHotkeyWarning] = createSignal('')
  const [pendingImport, setPendingImport] = createSignal<{
    path: string
    preview: ImportPreview
  } | null>(null)
  /** 清空历史是不可撤销的破坏性操作，必须先确认 */
  const [pendingClear, setPendingClear] = createSignal(false)
  /** 待删除的分类标签；非空时弹出确认框 */
  const [pendingTagDelete, setPendingTagDelete] = createSignal<string | null>(null)

  // 新手引导弹窗状态：初次启动自动展示，也可通过标题栏随时打开
  const [showGuide, setShowGuide] = createSignal(
    (() => {
      try {
        return !localStorage.getItem('wp_guide_seen')
      } catch {
        return false
      }
    })(),
  )

  // 设置弹窗状态与元信息
  const [showSettings, setShowSettings] = createSignal(false)
  const [appVersion, setAppVersion] = createSignal('0.2.0')
  const [dataRoot, setDataRoot] = createSignal('')
  const [blobsRoot, setBlobsRoot] = createSignal('')

  /** 正在查看的图片 OCR 文本；非空时弹出只读查看窗 */
  const [ocrPreview, setOcrPreview] = createSignal<{ text: string; imageLabel: string } | null>(
    null,
  )

  // 右键上下文菜单状态
  const [contextMenu, setContextMenu] = createSignal<ContextMenuState>({
    visible: false,
    x: 0,
    y: 0,
    item: null,
  })

  // ---- 非响应式的会话状态 ----
  /** 最近一次输入的搜索词。事件回调里读取信号即可拿到最新值，无需 ref 镜像。 */
  let composing = false
  const searchDebounce = useTimeout()
  const toastTimer = useTimeout()

  const onCloseGuide = () => {
    setShowGuide(false)
    try {
      localStorage.setItem('wp_guide_seen', 'true')
    } catch {}
  }

  const showToast = (message: string) => {
    if (!message) return
    toastTimer.set(() => setToast({ visible: false, message: '' }), 3200)
    setToast({ visible: true, message })
  }

  // 订阅宿主推送。onPush 返回解绑函数，组件销毁时释放。
  createEffect(
    () => undefined,
    () => {
      const unsubscribe = onPush(
        (message: HostPush) => {
          switch (message.type) {
            case 'items':
              setItems(message.items)
              setIsEmpty(message.isEmpty)
              if (typeof message.totalCount === 'number') {
                setTotalCount(message.totalCount)
              } else if (!searchText()) {
                setTotalCount(message.items.length)
              }
              if (Array.isArray(message.allTags)) setAllTags(message.allTags)
              if (message.tagCounts) setTagCounts(message.tagCounts)
              if (message.activeTag !== undefined) setActiveTag(message.activeTag ?? null)
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
              if (message.error) showToast(`无法导入：${message.error}`)
              else if (message.preview) setPendingImport({ path: message.path, preview: message.preview })
              break
            case 'paths':
              if (message.version) setAppVersion(message.version)
              if (message.dataRoot) setDataRoot(message.dataRoot)
              if (message.blobsRoot) setBlobsRoot(message.blobsRoot)
              break
            case 'thumbnail':
              setThumbnails((prev) =>
                prev[message.itemId] === message.imageUrl
                  ? prev
                  : { ...prev, [message.itemId]: message.imageUrl },
              )
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
        },
        () => {
          // 监听器就绪后再请求初始数据，否则整批初始推送会丢
          send({ type: 'ready' })
        },
      )
      return unsubscribe
    },
  )

  /** 输入框变化：立即更新本地值，但请求延后 120ms 合并。 */
  const onSearch = (value: string) => {
    setSearchText(value)
    searchDebounce.clear()

    // 输入法组合中不查库，等 compositionend 再发
    if (composing) return

    searchDebounce.set(() => send({ type: 'search', text: value }), SEARCH_DEBOUNCE_MS)
  }

  /** 输入法上屏后立刻补发一次查询，避免等满防抖窗口。 */
  const onCompositionEnd = (value: string) => {
    composing = false
    searchDebounce.clear()
    send({ type: 'search', text: value })
  }

  /** 请求某条图片的缩略图；同一个 id 只发一次。 */
  const onRequestThumbnail = (itemId: number) => {
    if (requestedThumbs.has(itemId)) return
    requestedThumbs.add(itemId)
    send({ type: 'loadThumbnail', itemId })
  }

  // 条目被删掉后释放对应的缩略图与去重集合，避免这块内存只增不减
  createEffect(
    () => items(),
    (list) => {
      setThumbnails((prev) => {
        const alive = new Set(list.map((item) => item.id))
        let changed = false
        const next: Record<number, string | null> = {}
        for (const [key, value] of Object.entries(prev)) {
          const id = Number(key)
          if (alive.has(id)) {
            next[id] = value
          } else {
            changed = true
            requestedThumbs.delete(id)
          }
        }
        return changed ? next : prev
      })
    },
  )

  const onExport = () => send({ type: 'export', path: null })

  /**
   * 后端推送一旦包含某个空标签，就说明它已经被某张卡片引用、
   * 真正落库了——这时该由 `allTags` 接管，草稿里那份要撤掉。
   *
   * 不做这一步的后果：`visibleTags` 会把同一个标签列两次（一次来自
   * pendingTags，一次来自 allTags），标签栏和右键菜单里出现重复胶囊。
   */
  createEffect(
    () => allTags(),
    (backendTags) => {
      setPendingTags((prev) => {
        const next = prev.filter((tag) => !backendTags.includes(tag))
        return next.length === prev.length ? prev : next
      })
    },
  )

  /**
   * 标签栏上新建分类。
   *
   * 颜色立刻存进 localStorage（这是纯前端配置，本来就不该等后端）；
   * 标签名先作为"空标签"进入 pendingTags 让它立即可见、可筛选。
   *
   * 如果当前有选中卡片，顺手把它分到这个新分类下——用户建分类的意图
   * 九成是给正在看的这张卡归类，等他再去右键一趟反而多余。
   * 没有选中卡片时也无妨：标签照样显示，等他自己挑卡片归类。
   */
  const onCreateTag = (tag: string, color: string) => {
    setTagColors((prev) => {
      const next = { ...prev, [tag]: color }
      saveTagColors(next)
      return next
    })

    setPendingTags((prev) => (prev.includes(tag) ? prev : [...prev, tag]))

    const selected = selectedId()
    if (selected !== null) {
      send({ type: 'setTag', itemId: selected, tag })
    }
  }

  const onRequestImport = (path: string) => send({ type: 'previewImport', path })

  /**
   * 点"导入"时选文件。
   *
   * 不用 `<input type=file>`：webview 里拿不到真实文件系统路径，后端无法据此读文件。
   * 文件对话框由宿主用 Win32 `GetOpenFileNameW` 弹出，选完直接把路径回传给前端。
   */
  const onPickImportFile = () => {
    if (!isHosted()) {
      showToast('导入需要在桌面应用里使用')
      return
    }
    send({ type: 'pickImportFile' })
  }

  const onRequestClearAll = () => setPendingClear(true)

  const onConfirmTagDelete = () => {
    const tag = pendingTagDelete()
    if (!tag) return
    send({ type: 'deleteTag', tag })
    setTagColors((prev) => {
      if (!(tag in prev)) return prev
      const next = { ...prev }
      delete next[tag]
      saveTagColors(next)
      return next
    })
    // 草稿里可能也躺着这个标签（建了但还没分给任何卡片）——
    // 删分类必须把它一起收掉，否则它会在标签栏里原地复活。
    setPendingTags((prev) => prev.filter((t) => t !== tag))
    setPendingTagDelete(null)
  }

  const onConfirmClearAll = () => {
    send({ type: 'clearAll' })
    setPendingClear(false)
  }

  /**
   * 把 `.json` 拖进面板即视为导入请求。
   *
   * Tauri 的 drag-drop 事件会给出真实文件系统路径，比 `<input type=file>`
   * 可靠——后者在 webview 里被沙箱挡掉了路径。
   */
  mount(() => {
    if (!isHosted()) return
    let disposed = false
    let unlisten: (() => void) | undefined

    getCurrentWebview()
      .onDragDropEvent((event) => {
        if (event.payload.type !== 'drop') return
        const dropped = event.payload.paths?.find((p) => p.toLowerCase().endsWith('.json'))
        if (dropped) onRequestImport(dropped)
        else if (event.payload.paths?.length) showToast('只支持导入 .json 导出包')
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
  })

  const onConfirmImport = () => {
    const current = pendingImport()
    if (!current) return
    // 先关对话框，导入结果通过 toast 回报。
    // 失败时后端只会推 toast，不会重新打开这个对话框。
    send({ type: 'import', path: current.path })
    setPendingImport(null)
  }

  const onSaveTagColor = (tag: string, color: string) => {
    setTagColors((prev) => {
      const next = { ...prev, [tag]: color }
      saveTagColors(next)
      return next
    })
  }

  const onSelect = (item: ClipboardItem) => {
    // 先本地反映选中态，避免等宿主回推的一帧空窗
    setSelectedId(item.id)
    send({ type: 'select', itemId: item.id })
  }

  const onPaste = (item: ClipboardItem) => send({ type: 'paste', itemId: item.id })
  const onCopy = (item: ClipboardItem) => send({ type: 'copy', itemId: item.id })

  /** 对已存在的图片记录补做一次 OCR（右键菜单里手动触发） */
  const onRunOcr = (item: ClipboardItem) => send({ type: 'runOcr', itemId: item.id })

  /** 打开识别文字查看窗：先看内容，要不要复制由用户决定 */
  const onViewOcr = (item: ClipboardItem) => {
    const text = extractOcrText(item.content)
    if (!text) return
    setOcrPreview({ text, imageLabel: ocrBaseContent(item.content) })
  }

  const onTogglePin = (item: ClipboardItem) => send({ type: 'togglePin', itemId: item.id })
  const onRemove = (item: ClipboardItem) => send({ type: 'delete', itemId: item.id })
  const onSetTag = (item: ClipboardItem, tag: string | null) =>
    send({ type: 'setTag', itemId: item.id, tag })

  const onFilterTag = (tag: string | null) => {
    setActiveTag(tag)
    send({ type: 'filterTag', tag })
  }

  const onContextMenu = (item: ClipboardItem, event: MouseEvent) => {
    setContextMenu({ visible: true, x: event.clientX, y: event.clientY, item })
  }

  const onCloseContextMenu = () =>
    setContextMenu((prev) => ({ ...prev, visible: false }))

  const onPrev = () => send({ type: 'move', delta: -1 })
  const onNext = () => send({ type: 'move', delta: 1 })

  /**
   * 全局键盘交互（Enter/Del/Ctrl+P/Esc/方向键）。
   *
   * 只注册一次监听：Solid 里读信号总是拿最新值，
   * 不需要像 React 那样把每个依赖塞进 deps 数组反复重建监听器。
   */
  useWindowEvent('keydown', (event) => {
    const target = event.target as HTMLElement | null
    const typing = target?.tagName === 'INPUT' || target?.tagName === 'TEXTAREA'
    const selected = selectedId()

    // 输入法组合中（拼音未上屏）不要抢键：Enter 是在选候选词，
    // 方向键是在选候选词，都不应该触发粘贴 / 移动选中项。
    if (composing && event.key !== 'Escape') return

    // 导入 / 清空对话框、新手引导与设置弹窗都是模态的：只让 Esc 生效，
    // 其余按键（尤其是 Enter）不应该穿透到下面的卡片上。
    const modal =
      pendingImport() !== null || pendingClear() || pendingTagDelete() !== null || showGuide() || showSettings() || ocrPreview() !== null
    if (modal && event.key !== 'Escape') return

    switch (event.key) {
      case 'Escape':
        if (ocrPreview()) {
          setOcrPreview(null)
          return
        }
        if (showSettings()) {
          setShowSettings(false)
          return
        }
        if (showGuide()) {
          onCloseGuide()
          return
        }
        if (pendingTagDelete()) {
          setPendingTagDelete(null)
          return
        }
        if (pendingClear()) {
          setPendingClear(false)
          return
        }
        if (pendingImport()) {
          setPendingImport(null)
          return
        }
        if (contextMenu().visible) {
          onCloseContextMenu()
          return
        }
        event.preventDefault()
        send({ type: 'close' })
        break
      case 'Enter':
        if (!typing && selected !== null) {
          event.preventDefault()
          send({ type: 'paste', itemId: selected })
        }
        break
      case 'Delete':
        if (!typing && selected !== null) {
          event.preventDefault()
          send({ type: 'delete', itemId: selected })
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
        if (event.ctrlKey && !typing && selected !== null) {
          event.preventDefault()
          send({ type: 'togglePin', itemId: selected })
        }
        break
    }
  })

  return (
    <div class="shell">
      <div class="panel">
        {/* 1. 顶部无边框标题栏 */}
        <TitleBar
          isMaximized={isMaximized}
          totalCount={totalCount}
          filteredCount={() => items().length}
          onOpenGuide={() => setShowGuide(true)}
          onOpenSettings={() => setShowSettings(true)}
        />

        {/* 热键不可用时的常驻提示：这个失败必须让人看见 */}
        <Show when={hotkeyWarning()}>
          {(message) => (
            <div class="hotkey-warning" role="alert">
              <span class="hotkey-warning-glyph">&#xE7BA;</span>
              <span class="hotkey-warning-text">{message()}</span>
              <button
                class="hotkey-warning-close"
                aria-label="关闭提示"
                onClick={() => setHotkeyWarning('')}
              >
                &#xE711;
              </button>
            </div>
          )}
        </Show>

        {/* 2. YouTube 风格分类 Tag 快捷切换标签栏（含新建分类入口） */}
        <TagFilterBar
          allTags={visibleTags}
          activeTag={activeTag}
          tagColors={tagColors}
          onFilterTag={onFilterTag}
          onSaveTagColor={onSaveTagColor}
          onCreateTag={onCreateTag}
          onDeleteTag={(tag) => setPendingTagDelete(tag)}
        />

        {/* 3. 虚拟卡片网格列表 */}
        <div class="list-area">
          <CardGrid
            items={items}
            thumbnails={thumbnails}
            onRequestThumbnail={onRequestThumbnail}
            selectedId={selectedId}
            tagColors={tagColors}
            onSelect={onSelect}
            onDoubleClick={onCopy}
            onPaste={onPaste}
            onTogglePin={onTogglePin}
            onRemove={onRemove}
            onContextMenu={onContextMenu}
          />
          <Show when={isEmpty()}>
            <div class="empty">
              <span class="empty-glyph">&#xE8C5;</span>
              <p class="empty-title">
                {activeTag() ? `分类 [${activeTag()}] 暂无记录` : '剪贴板历史为空'}
              </p>
              <span class="empty-desc">
                {activeTag()
                  ? '右键任意卡片即可选择加入此分类'
                  : '复制文本、图片或文件后将自动显示在这里'}
              </span>
            </div>
          </Show>
        </div>

        {/* 4. 底部搜索栏与快捷键说明 */}
        <ToolBar
          searchText={searchText}
          canPrev={canPrev}
          canNext={canNext}
          hotkeyLabel={hotkeyLabel}
          totalCount={totalCount}
          onSearchChange={onSearch}
          onCompositionStart={() => {
            composing = true
          }}
          onCompositionEnd={onCompositionEnd}
          onPrev={onPrev}
          onNext={onNext}
          onImport={onPickImportFile}
          onExport={onExport}
          onClearAll={onRequestClearAll}
        />

        {/* 5. 原生风格右键上下文菜单 */}
        <ContextMenu
          menu={contextMenu}
          allTags={visibleTags}
          tagColors={tagColors}
          onClose={onCloseContextMenu}
          onPaste={onPaste}
          onCopyText={(text) => send({ type: 'copyText', text })}
          onRunOcr={onRunOcr}
          onViewOcr={onViewOcr}
          onTogglePin={onTogglePin}
          onRemove={onRemove}
          onSetTag={onSetTag}
          onSaveTagColor={onSaveTagColor}
        />

        {/* 6. 导入确认：先告诉用户会发生什么，再写库 */}
        <ConfirmDialog
          open={pendingImport() !== null}
          title="导入剪贴板历史"
          confirmLabel="导入"
          onCancel={() => setPendingImport(null)}
          onConfirm={onConfirmImport}
        >
          <Show when={pendingImport()}>
            {(current) => (
              <>
                <p class="dialog-line">
                  这个文件包含 <b>{current().preview.total}</b> 条记录，导出于{' '}
                  {formatTimestamp(current().preview.exportedAt)}。
                </p>
                <p class="dialog-line">
                  其中 <b>{current().preview.fresh}</b> 条会被新增，
                  <b>{current().preview.existing}</b> 条已存在（只合并置顶与分类，不重复插入）。
                </p>
                <p class="dialog-hint">导入是幂等的：同一个文件导入多次，结果与导入一次相同。</p>
              </>
            )}
          </Show>
        </ConfirmDialog>

        {/* 7. 清空历史：不可撤销，必须二次确认 */}
        <ConfirmDialog
          open={pendingClear()}
          title="清空全部历史记录？"
          confirmLabel="清空"
          danger
          onCancel={() => setPendingClear(false)}
          onConfirm={onConfirmClearAll}
        >
          <p class="dialog-line">
            将删除全部 <b>{totalCount()}</b> 条记录，以及已缓存的图片文件。
          </p>
          <p class="dialog-hint">
            这个操作无法撤销。如果只是想让列表清爽一些，可以先「导出」一份备份。
          </p>
        </ConfirmDialog>

        {/* 8. 删除分类：说明影响范围，避免用户以为记录会被一起删掉 */}
        <ConfirmDialog
          open={pendingTagDelete() !== null}
          title="删除这个分类？"
          confirmLabel="删除分类"
          danger
          onCancel={() => setPendingTagDelete(null)}
          onConfirm={onConfirmTagDelete}
        >
          <Show when={pendingTagDelete()}>
            {(tag) => (
              <>
                <p class="dialog-line">
                  分类「<b>{tag()}</b>」当前有 <b>{tagCounts()[tag()] ?? 0}</b> 条记录。
                </p>
                <p class="dialog-hint">删除后这些记录会变成"未分类"，内容本身不受影响。</p>
              </>
            )}
          </Show>
        </ConfirmDialog>

        {/* 9. 轻量 toast，替代静默失败 */}
        <Toast toast={toast} />

        {/* 10. 新手功能引导与核心操作动画演示 */}
        <OnboardingGuide open={showGuide()} onClose={onCloseGuide} />

        {/* 11. 设置与关于弹窗 */}
        <SettingsDialog
          open={showSettings()}
          version={appVersion}
          dataRoot={dataRoot}
          blobsRoot={blobsRoot}
          onClose={() => setShowSettings(false)}
          onOpenDataDir={() => send({ type: 'openDataDir' })}
          onOpenUrl={(url) => send({ type: 'openUrl', url })}
        />

        {/* 12. 图片 OCR 识别结果查看（只读，可一键复制） */}
        <OcrTextViewer
          open={ocrPreview() !== null}
          text={() => ocrPreview()?.text ?? ''}
          imageLabel={() => ocrPreview()?.imageLabel ?? ''}
          onCopy={(text) => send({ type: 'copyText', text })}
          onClose={() => setOcrPreview(null)}
        />
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
