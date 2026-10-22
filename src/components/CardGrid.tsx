import { useCallback, useEffect, useLayoutEffect, useRef, useState } from 'react'
import ClipCard from './ClipCard'
import { send } from '../bridge'
import type { ClipboardItem } from '../types'

/** Base parameters for card layout and virtual windowing. */
const TARGET_CARD_WIDTH = 210
const GRID_GAP = 16
/** 卡片高宽比。参考图是竖向长方形，比正方形更能装下一段文字。 */
const CARD_ASPECT = 1.2
/** 卡片顶部堆叠边探出的高度，只有行间距需要为它留位置。 */
const STACK_PEEK = 12
/**
 * 行间距要比列间距大：每张卡上沿有 12px 的"堆叠边"，
 * 用普通 GRID_GAP 会让上一行的底部和下一行的堆叠边粘在一起。
 */
const ROW_GAP = GRID_GAP + STACK_PEEK
const PADDING_TOP = 26
const PADDING_BOTTOM = 20
const PADDING_LEFT = 20
const PADDING_RIGHT = 20
const PADDING_HORIZONTAL = PADDING_LEFT + PADDING_RIGHT
const OVERSCAN_ROWS = 2 // Render buffer rows above and below the visible area

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

/** 浮层渲染只需要这几个值；指针位置走命令式更新，避免每帧重渲染整个网格。 */
interface DragView {
  id: number
  width: number
  height: number
}

interface CardGridProps {
  items: ClipboardItem[]
  /** 已拉取的缩略图，key 是条目 id */
  thumbnails: Record<number, string | null>
  onRequestThumbnail: (itemId: number) => void
  selectedId: number | null
  onSelect: (item: ClipboardItem) => void
  onPaste: (item: ClipboardItem) => void
  onTogglePin: (item: ClipboardItem) => void
  onRemove: (item: ClipboardItem) => void
  onContextMenu?: (item: ClipboardItem, event: React.MouseEvent) => void
}

/**
 * Virtualised, four-ish column grid of clipboard cards.
 *
 * 卡片不再交给 CSS Grid 自动排版，而是按 index 计算绝对坐标：
 * 这样拖拽改变顺序时，每张卡片的 `transform` 会自己过渡到新位置，
 * 不需要手写 FLIP 测量，也不会因为虚拟列表的挂载/卸载而丢动画。
 */
export default function CardGrid({
  items,
  thumbnails,
  onRequestThumbnail,
  selectedId,
  onSelect,
  onPaste,
  onTogglePin,
  onRemove,
  onContextMenu,
}: CardGridProps) {
  const containerRef = useRef<HTMLDivElement | null>(null)
  const [containerWidth, setContainerWidth] = useState(0)
  const [containerHeight, setContainerHeight] = useState(0)
  const [scrollTop, setScrollTop] = useState(0)
  const [columns, setColumns] = useState(1)

  /** 拖拽中的乐观顺序；null 表示直接跟随宿主推送。 */
  const [dragItems, setDragItems] = useState<ClipboardItem[] | null>(null)
  const [dragView, setDragView] = useState<DragView | null>(null)
  const [justDroppedId, setJustDroppedId] = useState<number | null>(null)

  const pendingRef = useRef<PendingDrag | null>(null)
  const dragRef = useRef<DragSession | null>(null)
  const dragItemsRef = useRef<ClipboardItem[] | null>(null)
  const ghostRef = useRef<HTMLDivElement | null>(null)
  const itemsRef = useRef(items)
  itemsRef.current = items

  /** 拖拽结束后的那次 click 不应该顺带改变选中项。 */
  const suppressClickRef = useRef(false)
  const syncTimerRef = useRef<number | null>(null)

  const rafRef = useRef<number | null>(null)

  const measure = useCallback(() => {
    const el = containerRef.current
    if (!el) return
    setContainerWidth(el.clientWidth)
    setContainerHeight(el.clientHeight)
    setScrollTop(el.scrollTop)
  }, [])

  const recomputeColumns = useCallback(() => {
    const el = containerRef.current
    if (!el) return
    const usable = el.clientWidth - PADDING_HORIZONTAL
    if (usable <= 0) return
    const next = Math.max(1, Math.floor((usable + GRID_GAP) / (TARGET_CARD_WIDTH + GRID_GAP)))
    setColumns((prev) => {
      if (prev !== next) {
        send({ type: 'columns', columns: next })
      }
      return next
    })
  }, [])

  useLayoutEffect(() => {
    measure()
    recomputeColumns()

    const el = containerRef.current
    if (!el) return

    const observer = new ResizeObserver(() => {
      measure()
      recomputeColumns()
    })
    observer.observe(el)

    return () => observer.disconnect()
  }, [measure, recomputeColumns])

  const onScroll = useCallback((event: React.UIEvent<HTMLDivElement>) => {
    const target = event.currentTarget
    if (rafRef.current !== null) return
    rafRef.current = requestAnimationFrame(() => {
      setScrollTop(target.scrollTop)
      rafRef.current = null
    })
  }, [])

  useEffect(() => {
    return () => {
      if (rafRef.current !== null) {
        cancelAnimationFrame(rafRef.current)
      }
      if (syncTimerRef.current !== null) {
        window.clearTimeout(syncTimerRef.current)
      }
      document.body.classList.remove('is-dragging-card')
    }
  }, [])

  const activeItems = dragItems ?? items
  const usable = containerWidth - PADDING_HORIZONTAL
  const cardWidth = usable > 0 && columns > 0
    ? Math.max(100, (usable - (columns - 1) * GRID_GAP) / columns)
    : TARGET_CARD_WIDTH
  const cardHeight = Math.round(cardWidth * CARD_ASPECT)
  const rowStep = cardHeight + ROW_GAP

  /** 指针事件回调里要用最新的布局参数，又不该因此重建监听器。 */
  const geometryRef = useRef({ columns, cardWidth, cardHeight, rowStep })
  geometryRef.current = { columns, cardWidth, cardHeight, rowStep }

  const totalRows =
    activeItems.length === 0 || columns <= 0 ? 0 : Math.ceil(activeItems.length / columns)

  const totalContentHeight =
    totalRows === 0
      ? 0
      : PADDING_TOP +
        totalRows * cardHeight +
        Math.max(0, totalRows - 1) * ROW_GAP +
        PADDING_BOTTOM

  let visibleItems: ClipboardItem[] = []
  let startIndex = 0
  if (totalRows > 0 && rowStep > 0) {
    const topRow = Math.max(0, Math.floor((scrollTop - PADDING_TOP) / rowStep))
    const bottomRow = Math.min(
      totalRows - 1,
      Math.floor((scrollTop + containerHeight - PADDING_TOP) / rowStep),
    )
    const bufferedStartRow = Math.max(0, topRow - OVERSCAN_ROWS)
    const bufferedEndRow = Math.min(totalRows - 1, bottomRow + OVERSCAN_ROWS)

    startIndex = bufferedStartRow * columns
    const endIndex = Math.min(activeItems.length, (bufferedEndRow + 1) * columns)
    visibleItems = activeItems.slice(startIndex, endIndex)
  }

  const draggedItem = dragView
    ? activeItems.find((item) => item.id === dragView.id) ?? null
    : null

  const clearOverlay = useCallback(() => {
    dragItemsRef.current = null
    setDragItems(null)
    if (syncTimerRef.current !== null) {
      window.clearTimeout(syncTimerRef.current)
      syncTimerRef.current = null
    }
  }, [])

  /** 浮层跟随鼠标。用 style 直接写，避免每次 pointermove 都重渲染整个网格。 */
  const applyGhostPosition = useCallback(() => {
    const session = dragRef.current
    const ghost = ghostRef.current
    if (!session || !ghost) return
    const left = session.pointerX - session.grabX
    const top = session.pointerY - session.grabY
    ghost.style.transform = `translate3d(${left}px, ${top}px, 0)`
  }, [])

  /**
   * 根据指针位置计算落点，并在顺序真的变化时更新本地数组。
   *
   * 以被拖卡片的中心去对齐最近的槽位中心；置顶段与未置顶段之间不允许互插。
   */
  const syncTarget = useCallback(() => {
    const session = dragRef.current
    const container = containerRef.current
    const list = dragItemsRef.current
    if (!session || !container || !list || list.length === 0) return

    const { columns: cols, cardWidth: cw, cardHeight: ch, rowStep: rs } = geometryRef.current
    if (cols <= 0 || cw <= 0 || rs <= 0) return

    const rect = container.getBoundingClientRect()
    const centerX = session.pointerX - rect.left - session.grabX + cw / 2
    const centerY =
      session.pointerY - rect.top + container.scrollTop - session.grabY + ch / 2

    const col = Math.round((centerX - PADDING_LEFT - cw / 2) / (cw + GRID_GAP))
    const row = Math.round((centerY - PADDING_TOP - ch / 2) / rs)
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

    dragItemsRef.current = next
    session.index = target
    setDragItems(next)
  }, [])

  const handlePointerMove = useCallback(
    (event: PointerEvent) => {
      const pending = pendingRef.current
      const session = dragRef.current

      if (!session) {
        if (!pending || event.pointerId !== pending.pointerId) return
        const distance = Math.hypot(
          event.clientX - pending.startX,
          event.clientY - pending.startY,
        )
        if (distance < DRAG_THRESHOLD) return

        // 拖拽正式成立：快照当前顺序，之后所有位置变化都基于它
        const list = (dragItemsRef.current ?? itemsRef.current).slice()
        const index = list.findIndex((item) => item.id === pending.id)
        if (index === -1) {
          pendingRef.current = null
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
          width: rect.width || geometryRef.current.cardWidth,
          height: rect.height || geometryRef.current.cardHeight,
        }

        // 捕获指针：拖出窗口再松手也能收到 pointerup，不会留下一个卡住的浮层。
        // 只在越过阈值后才捕获，普通点击仍然落在卡片上。
        try {
          containerRef.current?.setPointerCapture(pending.pointerId)
        } catch {
          // 某些合成事件没有可捕获的指针，忽略即可
        }

        dragItemsRef.current = list
        dragRef.current = nextSession
        pendingRef.current = null

        setDragItems(list)
        setDragView({ id: pending.id, width: nextSession.width, height: nextSession.height })
        document.body.classList.add('is-dragging-card')
        event.preventDefault()
        return
      }

      session.pointerX = event.clientX
      session.pointerY = event.clientY
      applyGhostPosition()
      syncTarget()
      event.preventDefault()
    },
    [applyGhostPosition, syncTarget],
  )

  const endDrag = useCallback(
    (commit: boolean) => {
      const session = dragRef.current
      pendingRef.current = null
      if (!session) return

      const list = dragItemsRef.current
      dragRef.current = null
      document.body.classList.remove('is-dragging-card')
      setDragView(null)

      try {
        containerRef.current?.releasePointerCapture(session.pointerId)
      } catch {
        // 指针已经抬起时捕获会自然释放，这里失败无需处理
      }

      if (!commit || !list) {
        clearOverlay()
        return
      }

      const index = list.findIndex((item) => item.id === session.id)
      const dragged = index >= 0 ? list[index] : null
      if (!dragged) {
        clearOverlay()
        return
      }

      // 落点之后的那一条就是后端需要的 before_id；没有则移到本组末尾
      const next = list[index + 1]
      const beforeId = next && next.pinned === dragged.pinned ? next.id : null

      suppressClickRef.current = true
      window.setTimeout(() => {
        suppressClickRef.current = false
      }, 0)

      setJustDroppedId(dragged.id)
      window.setTimeout(() => {
        setJustDroppedId((current) => (current === dragged.id ? null : current))
      }, 260)

      send({ type: 'reorder', itemId: dragged.id, beforeId })

      // 乐观顺序保留到宿主 push 回来。正常情况下下面的 effect 会立刻清掉；
      // 这里的超时只是兜底，避免后端异常时界面永远停在本地顺序。
      if (syncTimerRef.current !== null) window.clearTimeout(syncTimerRef.current)
      syncTimerRef.current = window.setTimeout(() => {
        syncTimerRef.current = null
        dragItemsRef.current = null
        setDragItems(null)
      }, 1500)
    },
    [clearOverlay],
  )

  const handlePointerUp = useCallback(() => endDrag(true), [endDrag])
  const handlePointerCancel = useCallback(() => endDrag(false), [endDrag])
  const handleWindowBlur = useCallback(() => {
    // Alt+Tab / 窗口失焦时不会有 pointerup，主动收掉浮层，避免它卡在屏幕上
    if (dragRef.current) endDrag(false)
  }, [endDrag])

  const handleDragKeyDown = useCallback(
    (event: KeyboardEvent) => {
      if (event.key !== 'Escape') return
      if (!dragRef.current && !pendingRef.current) return
      event.preventDefault()
      // 拖拽中的 Esc 只取消拖拽，不应该顺手把面板关掉
      event.stopImmediatePropagation()
      endDrag(false)
    },
    [endDrag],
  )

  useEffect(() => {
    window.addEventListener('pointermove', handlePointerMove, { passive: false })
    window.addEventListener('pointerup', handlePointerUp)
    window.addEventListener('pointercancel', handlePointerCancel)
    window.addEventListener('blur', handleWindowBlur)
    window.addEventListener('keydown', handleDragKeyDown)

    return () => {
      window.removeEventListener('pointermove', handlePointerMove)
      window.removeEventListener('pointerup', handlePointerUp)
      window.removeEventListener('pointercancel', handlePointerCancel)
      window.removeEventListener('blur', handleWindowBlur)
      window.removeEventListener('keydown', handleDragKeyDown)
    }
  }, [
    handlePointerMove,
    handlePointerUp,
    handlePointerCancel,
    handleWindowBlur,
    handleDragKeyDown,
  ])

  // 拖到边缘时自动滚动；指针不动也要继续计算落点。
  useEffect(() => {
    if (!dragView) return

    let frame = 0
    const tick = () => {
      const container = containerRef.current
      const session = dragRef.current
      if (container && session) {
        const rect = container.getBoundingClientRect()
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
          const before = container.scrollTop
          container.scrollTop += delta
          if (container.scrollTop !== before) {
            syncTarget()
          }
        }
      }
      frame = requestAnimationFrame(tick)
    }

    frame = requestAnimationFrame(tick)
    return () => cancelAnimationFrame(frame)
  }, [dragView, syncTarget])

  // 浮层挂载后先摆到当前指针位置，避免第一帧闪到左上角
  useLayoutEffect(() => {
    if (dragView) applyGhostPosition()
  }, [dragView, applyGhostPosition])

  // 宿主推送的顺序与本地一致时，撤掉乐观覆盖层
  useEffect(() => {
    if (!dragItems || dragRef.current) return
    const sameOrder =
      items.length === dragItems.length &&
      items.every((item, index) => item.id === dragItems[index].id)
    const localIds = new Set(dragItems.map((item) => item.id))
    const sameSet =
      items.length === dragItems.length && items.every((item) => localIds.has(item.id))
    if (sameOrder || !sameSet) {
      clearOverlay()
    }
  }, [items, dragItems, clearOverlay])

  // Keep the selected card in view when selection changes.
  useEffect(() => {
    // 拖拽期间由指针决定滚动位置，自动滚动不能和"保持选中项可见"打架
    if (dragRef.current || pendingRef.current) return
    const el = containerRef.current
    if (!el || selectedId === null) return
    const index = activeItems.findIndex((item) => item.id === selectedId)
    if (index === -1) return

    const row = Math.floor(index / columns)
    const itemTop = PADDING_TOP + row * rowStep
    const itemBottom = itemTop + cardHeight
    const currentScrollTop = el.scrollTop
    const viewH = el.clientHeight

    if (itemTop < currentScrollTop + PADDING_TOP) {
      el.scrollTop = Math.max(0, itemTop - PADDING_TOP)
    } else if (itemBottom > currentScrollTop + viewH - PADDING_BOTTOM) {
      el.scrollTop = itemBottom - viewH + PADDING_BOTTOM
    }
  }, [selectedId, activeItems, columns, rowStep, cardHeight])

  // Reset scroll when the dataset shrinks below the current viewport.
  useEffect(() => {
    const el = containerRef.current
    if (el && el.scrollTop > totalContentHeight) {
      el.scrollTop = 0
    }
  }, [activeItems.length, totalContentHeight])

  const handleCardPointerDown = useCallback(
    (event: React.PointerEvent<HTMLDivElement>, item: ClipboardItem) => {
      if (event.button !== 0) return
      // 图钉 / 删除按钮有自己的点击语义，不要被拖拽吞掉
      if ((event.target as HTMLElement).closest('button')) return
      if (dragRef.current) return

      pendingRef.current = {
        id: item.id,
        startX: event.clientX,
        startY: event.clientY,
        pointerId: event.pointerId,
        sourceEl: event.currentTarget,
      }
    },
    [],
  )

  const handleSelect = useCallback(
    (item: ClipboardItem) => {
      if (suppressClickRef.current) return
      onSelect(item)
    },
    [onSelect],
  )

  return (
    <>
      <div ref={containerRef} className="viewport-container" onScroll={onScroll}>
        <div className="virtual-spacer" style={{ height: `${totalContentHeight}px` }}>
          <div className="visible-grid">
            {visibleItems.map((item, offset) => {
              const index = startIndex + offset
              const row = Math.floor(index / columns)
              const col = index % columns
              const x = PADDING_LEFT + col * (cardWidth + GRID_GAP)
              const y = PADDING_TOP + row * rowStep
              const isSource = dragView?.id === item.id

              return (
                <div
                  key={item.id}
                  className={`card-slot${isSource ? ' is-drag-source' : ''}${
                    justDroppedId === item.id ? ' just-dropped' : ''
                  }`}
                  style={{
                    width: `${cardWidth}px`,
                    height: `${cardHeight}px`,
                    transform: `translate3d(${x}px, ${y}px, 0)`,
                  }}
                  onPointerDown={(event) => handleCardPointerDown(event, item)}
                  onDragStart={(event) => event.preventDefault()}
                >
                  <ClipCard
                    item={item}
                    thumbnail={thumbnails[item.id]}
                    onRequestThumbnail={onRequestThumbnail}
                    selected={item.id === selectedId}
                    onSelect={handleSelect}
                    onPaste={onPaste}
                    onTogglePin={onTogglePin}
                    onRemove={onRemove}
                    onContextMenu={onContextMenu}
                  />
                  {isSource && <div className="drop-slot" aria-hidden="true" />}
                </div>
              )
            })}
          </div>
        </div>
      </div>

      {/* 浮层放在滚动容器外面：viewport 的 contain 会让 fixed 元素被裁剪 */}
      {dragView && draggedItem && (
        <div
          ref={ghostRef}
          className="drag-ghost"
          style={{ width: `${dragView.width}px`, height: `${dragView.height}px` }}
          aria-hidden="true"
        >
          <ClipCard
            item={draggedItem}
            thumbnail={thumbnails[draggedItem.id]}
            onRequestThumbnail={() => {}}
            selected={false}
            onSelect={() => {}}
            onPaste={() => {}}
            onTogglePin={() => {}}
            onRemove={() => {}}
          />
        </div>
      )}
    </>
  )
}
