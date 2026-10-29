import { For, Show, createEffect, createMemo, createSignal, onCleanup } from 'solid-js'
import ClipCard from './ClipCard'
import { mount } from '../solid-utils'
import { send } from '../bridge'
import type { ClipboardItem } from '../types'

/** Base parameters for card layout and virtual windowing. */
const TARGET_CARD_WIDTH = 210
const GRID_GAP = 16
/** 卡片高宽比。参考图是竖向长方形，比正方形更能装下一段文字。 */
const CARD_ASPECT = 1.2
/**
 * 顶部留白要容得下悬停时探出的层叠边（约 12px），
 * 否则第一行的"卡片堆"会被裁掉。
 */
const PADDING_TOP = 22
const PADDING_BOTTOM = 20
const PADDING_LEFT = 20
const PADDING_RIGHT = 20
const PADDING_HORIZONTAL = PADDING_LEFT + PADDING_RIGHT
const OVERSCAN_ROWS = 2

/** 按下后移动超过这个距离才认定为拖拽，否则仍然是一次普通点击。 */
const DRAG_THRESHOLD = 6
/** 拖到距列表上下边缘多少像素内开始自动滚动。 */
const AUTO_SCROLL_EDGE = 52
const AUTO_SCROLL_MAX_STEP = 18

interface PendingDrag {
  id: number
  startX: number
  startY: number
  pointerId: number
  sourceEl: HTMLElement
}

interface DragSession {
  id: number
  /** 当前在 dragItems 中的下标，会随落点变化。 */
  index: number
  /** 用于在 Esc 取消时主动释放指针捕获。 */
  pointerId: number
  /** 按下点相对卡片左上角的偏移，保证浮层不会在光标下跳一下。 */
  grabX: number
  grabY: number
  pointerX: number
  pointerY: number
  width: number
  height: number
}

interface CardGridProps {
  items: () => ClipboardItem[]
  /** 当前选中的分类标签（null 表示「全部」） */
  activeTag?: () => string | null
  /** 已拉取的缩略图，key 是条目 id */
  thumbnails: () => Record<number, string | null>
  onRequestThumbnail: (itemId: number) => void
  selectedId: () => number | null
  tagColors: () => Record<string, string>
  onSelect: (item: ClipboardItem) => void
  onDoubleClick?: (item: ClipboardItem) => void
  onPaste: (item: ClipboardItem) => void
  onTogglePin: (item: ClipboardItem) => void
  onRemove: (item: ClipboardItem) => void
  onContextMenu?: (item: ClipboardItem, event: MouseEvent) => void
}

/**
 * 虚拟滚动 + 拖拽排序的卡片网格。
 *
 * 卡片按 index 计算绝对坐标（而不是交给 CSS Grid 排版）：
 * 这样拖拽改变顺序时，每张卡片的 `transform` 会自己过渡到新位置，
 * 不需要手写 FLIP 测量，也不会因为虚拟列表的挂载/卸载而丢动画。
 *
 * Solid 版本的关键差异：组件函数只执行一次，因此所有"跨帧保持"的
 * 可变状态都是普通局部变量（相当于 React 的 useRef），
 * 只有真正需要触发渲染的才用 signal。
 */
export default function CardGrid(props: CardGridProps) {
  let containerEl: HTMLDivElement | undefined
  let ghostEl: HTMLDivElement | undefined

  const [containerWidth, setContainerWidth] = createSignal(0)
  const [containerHeight, setContainerHeight] = createSignal(0)
  const [scrollTop, setScrollTop] = createSignal(0)
  const [columns, setColumns] = createSignal(1)

  /** 拖拽中的乐观顺序；null 表示直接跟随宿主推送。 */
  const [dragItems, setDragItems] = createSignal<ClipboardItem[] | null>(null)
  const [dragView, setDragView] = createSignal<{ id: number; width: number; height: number } | null>(
    null,
  )
  const [justDroppedId, setJustDroppedId] = createSignal<number | null>(null)

  // ---- 非响应式的会话状态 ----
  let pending: PendingDrag | null = null
  let session: DragSession | null = null
  let dragList: ClipboardItem[] | null = null
  let suppressClick = false
  let syncTimer: number | null = null
  let rafHandle: number | null = null
  let scrollRaf: number | null = null

  const itemsNow = () => props.items()

  const measure = () => {
    if (!containerEl) return
    setContainerWidth(containerEl.clientWidth)
    setContainerHeight(containerEl.clientHeight)
    setScrollTop(containerEl.scrollTop)
  }

  const recomputeColumns = () => {
    if (!containerEl) return
    const usable = containerEl.clientWidth - PADDING_HORIZONTAL
    if (usable <= 0) return
    const next = Math.max(1, Math.floor((usable + GRID_GAP) / (TARGET_CARD_WIDTH + GRID_GAP)))
    setColumns((prev) => {
      if (prev !== next) send({ type: 'columns', columns: next })
      return next
    })
  }

  mount(() => {
    measure()
    recomputeColumns()
    if (!containerEl) return
    const observer = new ResizeObserver(() => {
      measure()
      recomputeColumns()
    })
    observer.observe(containerEl)
    return () => observer.disconnect()
  })

  onCleanup(() => {
    if (rafHandle !== null) cancelAnimationFrame(rafHandle)
    if (scrollRaf !== null) cancelAnimationFrame(scrollRaf)
    if (syncTimer !== null) window.clearTimeout(syncTimer)
    document.body.classList.remove('is-dragging-card')
  })

  // ---- 几何计算 ----
  const activeItems = () => dragItems() ?? itemsNow()

  const geometry = createMemo(() => {
    const usable = containerWidth() - PADDING_HORIZONTAL
    const cols = columns()
    const cardWidth =
      usable > 0 && cols > 0
        ? Math.max(100, (usable - (cols - 1) * GRID_GAP) / cols)
        : TARGET_CARD_WIDTH
    const cardHeight = Math.round(cardWidth * CARD_ASPECT)
    const rowStep = cardHeight + GRID_GAP
    return { cols, cardWidth, cardHeight, rowStep }
  })

  const totalRows = () => {
    const list = activeItems()
    const cols = geometry().cols
    return list.length === 0 || cols <= 0 ? 0 : Math.ceil(list.length / cols)
  }

  const totalContentHeight = () => {
    const rows = totalRows()
    const { cardHeight } = geometry()
    if (rows === 0) return 0
    return (
      PADDING_TOP + rows * cardHeight + Math.max(0, rows - 1) * GRID_GAP + PADDING_BOTTOM
    )
  }

  const visible = createMemo(() => {
    const rows = totalRows()
    const list = activeItems()
    const { cols, rowStep } = geometry()
    if (rows === 0 || rowStep <= 0) return { items: [] as ClipboardItem[], startIndex: 0 }

    const top = scrollTop()
    const viewH = containerHeight()
    const topRow = Math.max(0, Math.floor((top - PADDING_TOP) / rowStep))
    const bottomRow = Math.min(rows - 1, Math.floor((top + viewH - PADDING_TOP) / rowStep))
    const bufferedStartRow = Math.max(0, topRow - OVERSCAN_ROWS)
    const bufferedEndRow = Math.min(rows - 1, bottomRow + OVERSCAN_ROWS)

    const startIndex = bufferedStartRow * cols
    const endIndex = Math.min(list.length, (bufferedEndRow + 1) * cols)
    return { items: list.slice(startIndex, endIndex), startIndex }
  })

  const draggedItem = () => {
    const view = dragView()
    if (!view) return null
    return activeItems().find((item) => item.id === view.id) ?? null
  }

  const clearOverlay = () => {
    dragList = null
    setDragItems(null)
    if (syncTimer !== null) {
      window.clearTimeout(syncTimer)
      syncTimer = null
    }
  }

  /** 浮层跟随鼠标。直接写 style，避免每次 pointermove 都触发渲染。 */
  const applyGhostPosition = () => {
    if (!session || !ghostEl) return
    ghostEl.style.transform = `translate3d(${session.pointerX - session.grabX}px, ${
      session.pointerY - session.grabY
    }px, 0)`
  }

  /**
   * 根据指针位置计算落点，并在顺序真的变化时更新本地数组。
   *
   * 以被拖卡片的中心去对齐最近的槽位中心；置顶段与未置顶段之间不允许互插。
   */
  const syncTarget = () => {
    const list = dragList
    if (!session || !containerEl || !list || list.length === 0) return

    const { cols, cardWidth, cardHeight, rowStep } = geometry()
    if (cols <= 0 || cardWidth <= 0 || rowStep <= 0) return

    const rect = containerEl.getBoundingClientRect()
    const centerX = session.pointerX - rect.left - session.grabX + cardWidth / 2
    const centerY =
      session.pointerY - rect.top + containerEl.scrollTop - session.grabY + cardHeight / 2

    const col = Math.round((centerX - PADDING_LEFT - cardWidth / 2) / (cardWidth + GRID_GAP))
    const row = Math.round((centerY - PADDING_TOP - cardHeight / 2) / rowStep)
    const clampedCol = Math.min(cols - 1, Math.max(0, col))
    const clampedRow = Math.max(0, row)

    let target = Math.min(list.length - 1, Math.max(0, clampedRow * cols + clampedCol))

    const dragged = list[session.index]
    if (!dragged) return

    // Pinned 永远是连续的前缀，所以分组边界很好算
    let groupStart = 0
    let groupEnd = list.length - 1
    if (dragged.pinned) {
      groupEnd = list.reduce((last, item, index) => (item.pinned ? index : last), 0)
    } else {
      const firstUnpinned = list.findIndex((item) => !item.pinned)
      groupStart = firstUnpinned === -1 ? list.length - 1 : firstUnpinned
    }
    target = Math.min(groupEnd, Math.max(groupStart, target))

    if (target === session.index) return

    const next = list.slice()
    const [moved] = next.splice(session.index, 1)
    next.splice(target, 0, moved)

    dragList = next
    session.index = target
    setDragItems(next)
  }

  const endDrag = (commit: boolean) => {
    const current = session
    pending = null
    if (!current) return

    const list = dragList
    session = null
    document.body.classList.remove('is-dragging-card')
    setDragView(null)

    try {
      containerEl?.releasePointerCapture(current.pointerId)
    } catch {
      // 指针已经抬起时捕获会自然释放，这里失败无需处理
    }

    if (!commit || !list) {
      clearOverlay()
      return
    }

    const index = list.findIndex((item) => item.id === current.id)
    const dragged = index >= 0 ? list[index] : null
    if (!dragged) {
      clearOverlay()
      return
    }

    // 落点之后的那一条就是后端需要的 before_id；没有则移到本组末尾
    const next = list[index + 1]
    const beforeId = next && next.pinned === dragged.pinned ? next.id : null

    suppressClick = true
    window.setTimeout(() => {
      suppressClick = false
    }, 0)

    setJustDroppedId(dragged.id)
    window.setTimeout(() => {
      setJustDroppedId((value) => (value === dragged.id ? null : value))
    }, 260)

    send({ type: 'reorder', itemId: dragged.id, beforeId })

    // 乐观顺序保留到宿主 push 回来；超时只是兜底，避免后端异常时界面卡住
    if (syncTimer !== null) window.clearTimeout(syncTimer)
    syncTimer = window.setTimeout(() => {
      syncTimer = null
      dragList = null
      setDragItems(null)
    }, 1500)
  }

  const handlePointerMove = (event: PointerEvent) => {
    if (!session) {
      if (!pending || event.pointerId !== pending.pointerId) return
      const distance = Math.hypot(event.clientX - pending.startX, event.clientY - pending.startY)
      if (distance < DRAG_THRESHOLD) return

      // 拖拽正式成立：快照当前顺序，之后所有位置变化都基于它
      const list = (dragList ?? itemsNow()).slice()
      const index = list.findIndex((item) => item.id === pending!.id)
      if (index === -1) {
        pending = null
        return
      }

      const rect = pending.sourceEl.getBoundingClientRect()
      const nextSession: DragSession = {
        id: pending.id,
        index,
        pointerId: pending.pointerId,
        grabX: pending.startX - rect.left,
        grabY: pending.startY - rect.top,
        pointerX: event.clientX,
        pointerY: event.clientY,
        width: rect.width || geometry().cardWidth,
        height: rect.height || geometry().cardHeight,
      }

      // 捕获指针：拖出窗口再松手也能收到 pointerup，不会留下卡住的浮层
      try {
        containerEl?.setPointerCapture(pending.pointerId)
      } catch {
        // 某些合成事件没有可捕获的指针，忽略即可
      }

      dragList = list
      session = nextSession
      pending = null

      setDragItems(list)
      setDragView({ id: nextSession.id, width: nextSession.width, height: nextSession.height })
      document.body.classList.add('is-dragging-card')
      event.preventDefault()
      return
    }

    session.pointerX = event.clientX
    session.pointerY = event.clientY
    applyGhostPosition()
    syncTarget()
    event.preventDefault()
  }

  const handlePointerUp = () => endDrag(true)
  const handlePointerCancel = () => endDrag(false)

  mount(() => {
    const onBlur = () => {
      // Alt+Tab / 窗口失焦时不会有 pointerup，主动收掉浮层
      if (session) endDrag(false)
    }
    const onDragKey = (event: KeyboardEvent) => {
      if (event.key !== 'Escape') return
      if (!session && !pending) return
      event.preventDefault()
      // 拖拽中的 Esc 只取消拖拽，不应该顺手把面板关掉
      event.stopImmediatePropagation()
      endDrag(false)
    }

    window.addEventListener('pointermove', handlePointerMove, { passive: false })
    window.addEventListener('pointerup', handlePointerUp)
    window.addEventListener('pointercancel', handlePointerCancel)
    window.addEventListener('blur', onBlur)
    window.addEventListener('keydown', onDragKey)

    return () => {
      window.removeEventListener('pointermove', handlePointerMove)
      window.removeEventListener('pointerup', handlePointerUp)
      window.removeEventListener('pointercancel', handlePointerCancel)
      window.removeEventListener('blur', onBlur)
      window.removeEventListener('keydown', onDragKey)
    }
  })

  // 拖到边缘时自动滚动；指针不动也要继续计算落点。
  createEffect(
    () => dragView(),
    (view) => {
      if (!view) return
      let frame = 0
      const tick = () => {
        if (containerEl && session) {
          const rect = containerEl.getBoundingClientRect()
          let delta = 0
          if (session.pointerY < rect.top + AUTO_SCROLL_EDGE) {
            delta = -Math.min(
              AUTO_SCROLL_MAX_STEP,
              (rect.top + AUTO_SCROLL_EDGE - session.pointerY) * 0.45,
            )
          } else if (session.pointerY > rect.bottom - AUTO_SCROLL_EDGE) {
            delta = Math.min(
              AUTO_SCROLL_MAX_STEP,
              (session.pointerY - (rect.bottom - AUTO_SCROLL_EDGE)) * 0.45,
            )
          }

          if (delta !== 0) {
            const before = containerEl.scrollTop
            containerEl.scrollTop += delta
            if (containerEl.scrollTop !== before) syncTarget()
          }
        }
        frame = requestAnimationFrame(tick)
      }
      frame = requestAnimationFrame(tick)
      return () => cancelAnimationFrame(frame)
    },
  )

  // 浮层挂载后先摆到当前指针位置，避免第一帧闪到左上角
  createEffect(
    () => dragView(),
    (view) => {
      if (view) applyGhostPosition()
    },
  )

  // 宿主推送的顺序与本地一致时，撤掉乐观覆盖层
  createEffect(
    () => [dragItems(), itemsNow()] as const,
    ([local, incoming]) => {
      if (!local || session) return
      const sameOrder =
        incoming.length === local.length && incoming.every((item, i) => item.id === local[i].id)
      const localIds = new Set(local.map((item) => item.id))
      const sameSet =
        incoming.length === local.length && incoming.every((item) => localIds.has(item.id))
      if (sameOrder || !sameSet) clearOverlay()
    },
  )

  // 保持选中项可见。
  //
  // 只在"选中项变了"或"它挪到了别的槽位"时滚动。列表内容更新
  // （例如 OCR 把识别文字写回某条记录）会带着新的 items 触发本 effect，
  // 但那种时候用户可能已经自己滚开了视图——再把它拽回选中卡片，
  // 就是"识别完成瞬间页面跳回另一张卡"的来源。用 key 把这两种情况分开。
  let keepViewKey: string | null = null
  createEffect(
    () => [props.selectedId(), activeItems(), geometry()] as const,
    ([selected, list, geo]) => {
      // 拖拽期间由指针决定滚动位置，不能和自动滚动打架
      if (session || pending) return
      if (!containerEl || selected === null) {
        keepViewKey = null
        return
      }
      const index = list.findIndex((item) => item.id === selected)
      if (index === -1) return

      const key = `${selected}:${index}`
      if (key === keepViewKey) return
      keepViewKey = key

      const row = Math.floor(index / geo.cols)
      const itemTop = PADDING_TOP + row * geo.rowStep
      const itemBottom = itemTop + geo.cardHeight
      const currentScrollTop = containerEl.scrollTop
      const viewH = containerEl.clientHeight

      if (itemTop < currentScrollTop + PADDING_TOP) {
        containerEl.scrollTop = Math.max(0, itemTop - PADDING_TOP)
      } else if (itemBottom > currentScrollTop + viewH - PADDING_BOTTOM) {
        containerEl.scrollTop = itemBottom - viewH + PADDING_BOTTOM
      }
    },
  )

  // 数据量缩到视口以下时把滚动位置归零
  createEffect(
    () => totalContentHeight(),
    (height) => {
      if (containerEl && containerEl.scrollTop > height) {
        containerEl.scrollTop = 0
      }
    },
  )

  const onScroll = () => {
    if (scrollRaf !== null) return
    scrollRaf = requestAnimationFrame(() => {
      scrollRaf = null
      if (containerEl) setScrollTop(containerEl.scrollTop)
    })
  }

  const handleCardPointerDown = (event: PointerEvent, item: ClipboardItem) => {
    if (event.button !== 0) return
    // 图钉 / 删除按钮有自己的点击语义，不要被拖拽吞掉
    if ((event.target as HTMLElement).closest('button')) return
    if (session) return

    pending = {
      id: item.id,
      startX: event.clientX,
      startY: event.clientY,
      pointerId: event.pointerId,
      sourceEl: event.currentTarget as HTMLElement,
    }
  }

  const selectCard = (item: ClipboardItem) => {
    if (suppressClick) return
    props.onSelect(item)
  }

  return (
    <>
      <div ref={containerEl} class="viewport-container" onScroll={onScroll}>
        <div class="virtual-spacer" style={{ height: `${totalContentHeight()}px` }}>
          <div class="visible-grid">
            <For each={visible().items}>
              {(item, offset) => {
                const index = () => visible().startIndex + offset()
                const position = createMemo(() => {
                  const { cols, cardWidth, rowStep } = geometry()
                  const row = Math.floor(index() / cols)
                  const col = index() % cols
                  return {
                    x: PADDING_LEFT + col * (cardWidth + GRID_GAP),
                    y: PADDING_TOP + row * rowStep,
                  }
                })
                const isSource = () => dragView()?.id === item.id
                return (
                  <div
                    class={[
                      'card-slot',
                      {
                        'is-drag-source': isSource(),
                        'just-dropped': justDroppedId() === item.id,
                      },
                    ]}
                    style={{
                      width: `${geometry().cardWidth}px`,
                      height: `${geometry().cardHeight}px`,
                      transform: `translate3d(${position().x}px, ${position().y}px, 0)`,
                    }}
                    onPointerDown={(event) => handleCardPointerDown(event, item)}
                    onDragStart={(event) => event.preventDefault()}
                  >
                    <ClipCard
                      item={item}
                      activeTag={props.activeTag}
                      thumbnail={() => props.thumbnails()[item.id]}
                      onRequestThumbnail={props.onRequestThumbnail}
                      selected={() => item.id === props.selectedId()}
                      tagColor={() => (item.tag ? props.tagColors()[item.tag] : undefined)}
                      onSelect={selectCard}
                      onDoubleClick={props.onDoubleClick}
                      onPaste={props.onPaste}
                      onTogglePin={props.onTogglePin}
                      onRemove={props.onRemove}
                      onContextMenu={props.onContextMenu}
                    />
                    <Show when={isSource()}>
                      <div class="drop-slot" aria-hidden="true" />
                    </Show>
                  </div>
                )
              }}
            </For>
          </div>
        </div>
      </div>

      {/* 浮层放在滚动容器外面：viewport 的 contain 会让 fixed 元素被裁剪 */}
      <Show when={dragView() && draggedItem()}>
        {(_) => {
          const dragged = () => draggedItem()!
          return (
            <div
              ref={ghostEl}
              class="drag-ghost"
              style={{ width: `${dragView()!.width}px`, height: `${dragView()!.height}px` }}
              aria-hidden="true"
            >
              <ClipCard
                item={dragged()}
                thumbnail={() => props.thumbnails()[dragged().id]}
                onRequestThumbnail={() => {}}
                selected={() => false}
                tagColor={() => (dragged().tag ? props.tagColors()[dragged().tag!] : undefined)}
                onSelect={() => {}}
                onPaste={() => {}}
                onTogglePin={() => {}}
                onRemove={() => {}}
              />
            </div>
          )
        }}
      </Show>
    </>
  )
}
