import { useCallback, useEffect, useLayoutEffect, useRef, useState } from 'react'
import ClipCard from './ClipCard'
import { send } from '../bridge'
import type { ClipboardItem } from '../types'

/** Base parameters for card layout and virtual windowing. */
const TARGET_CARD_WIDTH = 236
const GRID_GAP = 16
const PADDING_TOP = 18
const PADDING_BOTTOM = 18
const PADDING_LEFT = 20
const PADDING_RIGHT = 20
const PADDING_HORIZONTAL = PADDING_LEFT + PADDING_RIGHT
const OVERSCAN_ROWS = 2 // Render buffer rows above and below the visible area

interface CardGridProps {
  items: ClipboardItem[]
  /** blob 目录绝对路径；图片卡片用它拼 asset URL */
  blobsRoot: string | null
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
 * Only the visible rows (plus an overscan buffer) are mounted, so a 500 entry
 * history scrolls smoothly with a native scrollbar.
 */
export default function CardGrid({
  items,
  blobsRoot,
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
    }
  }, [])

  const usable = containerWidth - PADDING_HORIZONTAL
  const cardWidth = usable > 0 && columns > 0
    ? Math.max(100, (usable - (columns - 1) * GRID_GAP) / columns)
    : TARGET_CARD_WIDTH
  const cardHeight = cardWidth
  const rowStep = cardHeight + GRID_GAP

  const totalRows = items.length === 0 || columns <= 0 ? 0 : Math.ceil(items.length / columns)

  const totalContentHeight =
    totalRows === 0
      ? 0
      : PADDING_TOP +
        totalRows * cardHeight +
        Math.max(0, totalRows - 1) * GRID_GAP +
        PADDING_BOTTOM

  let offsetY = 0
  let visibleItems: ClipboardItem[] = []
  if (totalRows > 0 && rowStep > 0) {
    const topRow = Math.max(0, Math.floor((scrollTop - PADDING_TOP) / rowStep))
    const bottomRow = Math.min(
      totalRows - 1,
      Math.floor((scrollTop + containerHeight - PADDING_TOP) / rowStep),
    )
    const bufferedStartRow = Math.max(0, topRow - OVERSCAN_ROWS)
    const bufferedEndRow = Math.min(totalRows - 1, bottomRow + OVERSCAN_ROWS)

    const startIndex = bufferedStartRow * columns
    const endIndex = Math.min(items.length, (bufferedEndRow + 1) * columns)
    offsetY = PADDING_TOP + bufferedStartRow * rowStep
    visibleItems = items.slice(startIndex, endIndex)
  }

  // Keep the selected card in view when selection changes.
  useEffect(() => {
    const el = containerRef.current
    if (!el || selectedId === null) return
    const index = items.findIndex((item) => item.id === selectedId)
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
  }, [selectedId, items, columns, rowStep, cardHeight])

  // Reset scroll when the dataset shrinks below the current viewport.
  useEffect(() => {
    const el = containerRef.current
    if (el && el.scrollTop > totalContentHeight) {
      el.scrollTop = 0
    }
  }, [items.length, totalContentHeight])

  return (
    <div ref={containerRef} className="viewport-container" onScroll={onScroll}>
      <div className="virtual-spacer" style={{ height: `${totalContentHeight}px` }}>
        <div
          className="visible-grid"
          style={{
            transform: `translate3d(0, ${offsetY}px, 0)`,
            gridTemplateColumns: `repeat(${columns}, minmax(0, 1fr))`,
            gap: `${GRID_GAP}px`,
            paddingLeft: `${PADDING_LEFT}px`,
            paddingRight: `${PADDING_RIGHT}px`,
          }}
        >
          {visibleItems.map((item) => (
            <ClipCard
              key={item.id}
              item={item}
              blobsRoot={blobsRoot}
              selected={item.id === selectedId}
              onSelect={onSelect}
              onPaste={onPaste}
              onTogglePin={onTogglePin}
              onRemove={onRemove}
              onContextMenu={onContextMenu}
            />
          ))}
        </div>
      </div>
    </div>
  )
}
