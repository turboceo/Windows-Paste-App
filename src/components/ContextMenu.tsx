import React, { useCallback, useEffect, useRef, useState } from 'react'
import type { ClipboardItem } from '../types'
import { TAG_COLOR_PALETTE, getTagColor } from '../tagColors'

export interface ContextMenuState {
  visible: boolean
  x: number
  y: number
  item: ClipboardItem | null
}

interface ContextMenuProps {
  menu: ContextMenuState
  allTags: string[]
  tagColors?: Record<string, string>
  onClose: () => void
  onPaste: (item: ClipboardItem) => void
  onTogglePin: (item: ClipboardItem) => void
  onRemove: (item: ClipboardItem) => void
  onSetTag: (item: ClipboardItem, tag: string | null) => void
  onSaveTagColor?: (tag: string, color: string) => void
}


/**
 * 鼠标离开"添加到分类"一行后，延迟多久收起子菜单。
 *
 * 用 debounce 而不是 throttle：
 * throttle 是"首次立即执行、之后限频"，鼠标一离开就会立刻收起一次，
 * 子菜单照样瞬间消失。这里要的是"离开后等一会儿，期间回来就取消"。
 *
 * 160ms 足够覆盖从主菜单斜向划入子菜单的时间（中间要穿过 4px 的间隙），
 * 又短到不像卡顿。
 */
const SUBMENU_CLOSE_DELAY = 160

export default function ContextMenu({
  menu,
  allTags,
  tagColors,
  onClose,
  onPaste,
  onTogglePin,
  onRemove,
  onSetTag,
  onSaveTagColor,
}: ContextMenuProps) {
  const [showSubMenu, setShowSubMenu] = useState(false)
  const [isCreatingTag, setIsCreatingTag] = useState(false)
  const [newTagInput, setNewTagInput] = useState('')
  const [selectedColor, setSelectedColor] = useState<string>(TAG_COLOR_PALETTE[0])
  const [colorEditingTag, setColorEditingTag] = useState<string | null>(null)
  const menuRef = useRef<HTMLDivElement | null>(null)
  const inputRef = useRef<HTMLInputElement | null>(null)
  /** 收起子菜单的延时任务；鼠标回到主菜单项时要取消它 */
  const closeTimerRef = useRef<number | null>(null)

  const cancelScheduledClose = useCallback(() => {
    if (closeTimerRef.current !== null) {
      window.clearTimeout(closeTimerRef.current)
      closeTimerRef.current = null
    }
  }, [])

  /** 立即展开子菜单（并取消任何待执行的收起）。 */
  const openSubMenu = () => {
    cancelScheduledClose()
    setShowSubMenu(true)
  }

  /**
   * 延迟收起子菜单。
   *
   * 直接 `setShowSubMenu(false)` 是不行的：子菜单与主菜单项之间有 4px 的
   * margin，鼠标斜着划过去时会先落到空隙里，`mouseleave` 当场触发，
   * 子菜单在用户点到之前就被卸载了。等一会儿再收，给鼠标留出穿过的余量。
   */
  const scheduleCloseSubMenu = useCallback(() => {
    // 正在输入新标签时绝不收起，否则输一半的标签会连同菜单一起消失
    if (isCreatingTag) return
    cancelScheduledClose()
    closeTimerRef.current = window.setTimeout(() => {
      closeTimerRef.current = null
      setShowSubMenu(false)
    }, SUBMENU_CLOSE_DELAY)
  }, [isCreatingTag, cancelScheduledClose])

  // 组件卸载时清掉挂起的定时器
  useEffect(() => cancelScheduledClose, [cancelScheduledClose])

  // 菜单关掉、或右键了另一张卡片时，重置子菜单与输入态。
  //
  // 这个组件在菜单隐藏时只是 `return null`、并不会卸载，所以 state 会留着：
  // 不重置的话，"展开子菜单 → 关闭 → 右键另一张卡"会看到子菜单自动弹出来。
  useEffect(() => {
    cancelScheduledClose()
    setShowSubMenu(false)
    setIsCreatingTag(false)
    setNewTagInput('')
    setColorEditingTag(null)
    setSelectedColor(TAG_COLOR_PALETTE[0])
  }, [menu.visible, menu.item?.id, cancelScheduledClose])

  // 只列数据库里真实存在的分类。
  //
  // 之前这里还并了一组硬编码的 PRESET_TAGS，后果是"删掉的标签下次又出现"——
  // 前端无条件把它塞回候选列表，用户会以为删除功能坏了。
  const mergedTags = Array.from(new Set(allTags)).filter(Boolean)

  useEffect(() => {
    function handleClickOutside(e: MouseEvent) {
      if (menuRef.current && !menuRef.current.contains(e.target as Node)) {
        onClose()
      }
    }
    function handleKeyDown(e: KeyboardEvent) {
      if (e.key === 'Escape') {
        onClose()
      }
    }
    if (menu.visible) {
      window.addEventListener('mousedown', handleClickOutside)
      window.addEventListener('keydown', handleKeyDown)
    }
    return () => {
      window.removeEventListener('mousedown', handleClickOutside)
      window.removeEventListener('keydown', handleKeyDown)
    }
  }, [menu.visible, onClose])

  useEffect(() => {
    if (isCreatingTag) {
      inputRef.current?.focus()
    }
  }, [isCreatingTag])

  if (!menu.visible || !menu.item) {
    return null
  }

  const { item } = menu

  function handleCreateTagSubmit(e: React.FormEvent) {
    e.preventDefault()
    const trimmed = newTagInput.trim()
    if (trimmed && item) {
      if (selectedColor && onSaveTagColor) {
        onSaveTagColor(trimmed, selectedColor)
      }
      onSetTag(item, trimmed)
      setIsCreatingTag(false)
      setNewTagInput('')
      onClose()
    }
  }

  // 视口边界保护，防止右键菜单超出屏幕边缘
  const menuWidth = 190
  const menuHeight = 220
  const adjustedX = Math.min(menu.x, window.innerWidth - menuWidth - 10)
  const adjustedY = Math.min(menu.y, window.innerHeight - menuHeight - 10)

  return (
    <div
      ref={menuRef}
      className="paste-context-menu"
      style={{ left: `${adjustedX}px`, top: `${adjustedY}px` }}
      onContextMenu={(e) => e.preventDefault()}
    >
      {/* 1. 立即粘贴 */}
      <div
        className="menu-item"
        onClick={() => {
          onPaste(item)
          onClose()
        }}
      >
        <span className="item-glyph">&#xE77F;</span>
        <span className="item-label">立即粘贴</span>
        <kbd className="item-shortcut">Enter</kbd>
      </div>

      {/* 2. 置顶/取消置顶 */}
      <div
        className="menu-item"
        onClick={() => {
          onTogglePin(item)
          onClose()
        }}
      >
        <span className="item-glyph">{item.pinned ? '\uE840' : '\uE718'}</span>
        <span className="item-label">{item.pinned ? '取消置顶' : '置顶卡片'}</span>
        <kbd className="item-shortcut">Ctrl+P</kbd>
      </div>

      <div className="menu-divider" />

      {/* 3. 添加到分类 / 修改分类 (展开二级菜单) */}
      <div
        className="menu-item has-submenu"
        onMouseEnter={openSubMenu}
        onMouseLeave={scheduleCloseSubMenu}
      >
        <span className="item-glyph">&#xE8EC;</span>
        <span className="item-label">
          {item.tag ? `分类: ${item.tag}` : '添加到分类'}
        </span>
        <span className="arrow-glyph">&#xE76C;</span>

        {/* 二级菜单：选择或新建分类 */}
        {showSubMenu && (
          <div className="paste-sub-menu">
            {item.tag && (
              <>
                <div
                  className="menu-item remove-tag"
                  onClick={() => {
                    onSetTag(item, null)
                    onClose()
                  }}
                >
                  <span className="item-glyph">&#xE711;</span>
                  <span className="item-label">清除当前分类</span>
                </div>
                <div className="menu-divider" />
              </>
            )}

            <div className="submenu-title">选择分类：</div>

            {mergedTags.map((tag) => {
              const isActive = item.tag === tag
              const dotColor = getTagColor(tag, tagColors)
              const isEditingThis = colorEditingTag === tag
              return (
                <div key={tag} className="tag-item-container">
                  <div
                    className={`menu-item tag-item${isActive ? ' active-tag' : ''}`}
                    onClick={() => {
                      onSetTag(item, tag)
                      onClose()
                    }}
                  >
                    <span className="tag-dot" style={{ backgroundColor: dotColor }} />
                    <span className="item-label">{tag}</span>
                    {isActive && <span className="check-glyph">&#xE73E;</span>}
                    <button
                      type="button"
                      className="tag-palette-trigger"
                      title="修改此分类颜色"
                      onClick={(e) => {
                        e.stopPropagation()
                        cancelScheduledClose()
                        setColorEditingTag(isEditingThis ? null : tag)
                      }}
                    >
                      &#xE790;
                    </button>
                  </div>

                  {/* 展开的调色盘小栏 */}
                  {isEditingThis && (
                    <div
                      className="tag-item-inline-palette"
                      onClick={(e) => e.stopPropagation()}
                    >
                      {TAG_COLOR_PALETTE.map((c) => (
                        <button
                          key={c}
                          type="button"
                          className={`palette-color-btn${dotColor === c ? ' active' : ''}`}
                          style={{ backgroundColor: c }}
                          title={c}
                          onClick={() => {
                            if (onSaveTagColor) {
                              onSaveTagColor(tag, c)
                            }
                          }}
                        />
                      ))}
                    </div>
                  )}
                </div>
              )
            })}

            <div className="menu-divider" />

            {/* 新建分类输入框与调色盘 */}
            {!isCreatingTag ? (
              <div
                className="menu-item new-tag"
                onClick={(e) => {
                  e.stopPropagation()
                  // 进入输入态后子菜单不能再被自动收起
                  cancelScheduledClose()
                  setIsCreatingTag(true)
                }}
              >
                <span className="item-glyph">&#xE710;</span>
                <span className="item-label">新建分类标签</span>
              </div>
            ) : (
              <form className="new-tag-form" onSubmit={handleCreateTagSubmit} onClick={(e) => e.stopPropagation()}>
                <div className="new-tag-input-row">
                  <span
                    className="selected-color-preview"
                    style={{ backgroundColor: selectedColor }}
                    title="当前选择的颜色"
                  />
                  <input
                    ref={inputRef}
                    className="new-tag-input"
                    type="text"
                    placeholder="标签名称..."
                    value={newTagInput}
                    onChange={(e) => setNewTagInput(e.target.value)}
                    maxLength={12}
                  />
                  <button type="submit" className="new-tag-confirm" title="确认">
                    &#xE73E;
                  </button>
                </div>
                <div className="color-palette-row">
                  {TAG_COLOR_PALETTE.map((color) => (
                    <button
                      key={color}
                      type="button"
                      className={`palette-color-btn${selectedColor === color ? ' active' : ''}`}
                      style={{ backgroundColor: color }}
                      onClick={() => setSelectedColor(color)}
                      title={color}
                    />
                  ))}
                </div>
              </form>
            )}
          </div>
        )}
      </div>

      <div className="menu-divider" />

      {/* 4. 删除卡片 */}
      <div
        className="menu-item danger"
        onClick={() => {
          onRemove(item)
          onClose()
        }}
      >
        <span className="item-glyph">&#xE74D;</span>
        <span className="item-label">删除记录</span>
        <kbd className="item-shortcut">Del</kbd>
      </div>
    </div>
  )
}
