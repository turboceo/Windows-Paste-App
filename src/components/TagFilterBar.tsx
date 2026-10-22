import React, { useCallback, useEffect, useRef, useState } from 'react'
import { TAG_COLOR_PALETTE, getTagColor } from '../tagColors'

interface TagFilterBarProps {
  allTags: string[]
  activeTag: string | null
  tagColors: Record<string, string>
  onFilterTag: (tag: string | null) => void
  onSaveTagColor: (tag: string, color: string) => void
  onDeleteTag: (tag: string) => void
}

interface TagMenuState {
  tag: string
  x: number
  y: number
}

/**
 * YouTube 风格的 feed-filter-chip-bar：
 * - 横向平滑滚动，带左右渐变遮罩与导航箭头；
 * - 鼠标滚轮直接在胶囊栏上横向平移；
 * - 右键任意标签胶囊弹出管理菜单，可随时修改分类颜色或删除分类。
 */
export default function TagFilterBar({
  allTags,
  activeTag,
  tagColors,
  onFilterTag,
  onSaveTagColor,
  onDeleteTag,
}: TagFilterBarProps) {
  const scrollRef = useRef<HTMLDivElement | null>(null)
  const [canScrollLeft, setCanScrollLeft] = useState(false)
  const [canScrollRight, setCanScrollRight] = useState(false)
  const [menu, setMenu] = useState<TagMenuState | null>(null)
  const menuRef = useRef<HTMLDivElement | null>(null)

  const checkScroll = useCallback(() => {
    const el = scrollRef.current
    if (!el) return
    const { scrollLeft, scrollWidth, clientWidth } = el
    setCanScrollLeft(scrollLeft > 3)
    setCanScrollRight(scrollLeft < scrollWidth - clientWidth - 3)
  }, [])

  useEffect(() => {
    checkScroll()
    const el = scrollRef.current
    if (!el) return

    const observer = new ResizeObserver(() => checkScroll())
    observer.observe(el)
    return () => observer.disconnect()
  }, [allTags, checkScroll])

  const scrollBy = (offset: number) => {
    if (!scrollRef.current) return
    scrollRef.current.scrollBy({ left: offset, behavior: 'smooth' })
  }

  const handleWheel = (e: React.WheelEvent<HTMLDivElement>) => {
    if (!scrollRef.current) return
    if (e.deltaY !== 0) {
      scrollRef.current.scrollLeft += e.deltaY * 0.9
      checkScroll()
    }
  }

  // 点击外部或 Esc 关闭标签管理浮层
  useEffect(() => {
    if (!menu) return
    const handleDown = (e: MouseEvent) => {
      if (menuRef.current && !menuRef.current.contains(e.target as Node)) {
        setMenu(null)
      }
    }
    const handleKey = (e: KeyboardEvent) => {
      if (e.key === 'Escape') {
        setMenu(null)
      }
    }
    window.addEventListener('mousedown', handleDown)
    window.addEventListener('keydown', handleKey)
    return () => {
      window.removeEventListener('mousedown', handleDown)
      window.removeEventListener('keydown', handleKey)
    }
  }, [menu])

  const handleChipContextMenu = (e: React.MouseEvent, tag: string) => {
    e.preventDefault()
    e.stopPropagation()
    const menuWidth = 200
    const menuHeight = 150
    const x = Math.min(e.clientX, window.innerWidth - menuWidth - 10)
    const y = Math.min(e.clientY, window.innerHeight - menuHeight - 10)
    setMenu({ tag, x, y })
  }

  return (
    <div className="feed-filter-bar-wrapper">
      {/* 左侧平滑滚动按钮与渐变边缘遮罩 */}
      {canScrollLeft && (
        <div className="chip-nav-wrapper left">
          <button
            className="chip-nav-btn"
            title="向左滚动"
            onClick={() => scrollBy(-200)}
            aria-label="向左滚动标签"
          >
            &#xE76B;
          </button>
        </div>
      )}

      {/* 滚动条隐藏的核心容器 */}
      <div
        ref={scrollRef}
        className="feed-filter-chip-bar"
        onScroll={checkScroll}
        onWheel={handleWheel}
      >
        <button
          className={`tag-filter-pill${activeTag === null ? ' active' : ''}`}
          onClick={() => onFilterTag(null)}
        >
          全部
        </button>

        {allTags.map((tag) => {
          const isActive = activeTag === tag
          const color = getTagColor(tag, tagColors)
          return (
            <button
              key={tag}
              className={`tag-filter-pill${isActive ? ' active' : ''}`}
              style={
                isActive
                  ? { backgroundColor: color, borderColor: color }
                  : undefined
              }
              title={`${tag} · 右键可修改颜色或删除`}
              onClick={() => onFilterTag(isActive ? null : tag)}
              onContextMenu={(e) => handleChipContextMenu(e, tag)}
            >
              <span
                className="tag-dot"
                style={{
                  backgroundColor: isActive ? '#ffffff' : color,
                }}
              />
              <span className="tag-name">{tag}</span>
            </button>
          )
        })}
      </div>

      {/* 右侧平滑滚动按钮与渐变边缘遮罩 */}
      {canScrollRight && (
        <div className="chip-nav-wrapper right">
          <button
            className="chip-nav-btn"
            title="向右滚动"
            onClick={() => scrollBy(200)}
            aria-label="向右滚动标签"
          >
            &#xE76C;
          </button>
        </div>
      )}

      {/* 标签右键菜单：可重选颜色、或删除该分类 */}
      {menu && (
        <div
          ref={menuRef}
          className="tag-chip-popover"
          style={{ left: `${menu.x}px`, top: `${menu.y}px` }}
          onContextMenu={(e) => e.preventDefault()}
        >
          <div className="popover-header">
            <span
              className="tag-dot"
              style={{ backgroundColor: getTagColor(menu.tag, tagColors) }}
            />
            <span className="popover-title">{menu.tag}</span>
          </div>

          <div className="popover-section-label">更改颜色：</div>
          <div className="color-palette-grid">
            {TAG_COLOR_PALETTE.map((color) => {
              const isSelected = getTagColor(menu.tag, tagColors) === color
              return (
                <button
                  key={color}
                  type="button"
                  className={`palette-color-btn${isSelected ? ' active' : ''}`}
                  style={{ backgroundColor: color }}
                  title={color}
                  onClick={() => {
                    onSaveTagColor(menu.tag, color)
                    // 修改后保持浮层，让用户看到选中的效果反馈
                  }}
                />
              )
            })}
          </div>

          <div className="popover-divider" />

          <button
            type="button"
            className="popover-delete-btn"
            onClick={() => {
              const targetTag = menu.tag
              setMenu(null)
              onDeleteTag(targetTag)
            }}
          >
            <span className="del-glyph">&#xE74D;</span>
            删除该分类
          </button>
        </div>
      )}
    </div>
  )
}
