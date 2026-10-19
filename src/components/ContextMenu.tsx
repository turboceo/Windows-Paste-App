import React, { useEffect, useRef, useState } from 'react'
import type { ClipboardItem } from '../types'

export interface ContextMenuState {
  visible: boolean
  x: number
  y: number
  item: ClipboardItem | null
}

interface ContextMenuProps {
  menu: ContextMenuState
  allTags: string[]
  onClose: () => void
  onPaste: (item: ClipboardItem) => void
  onTogglePin: (item: ClipboardItem) => void
  onRemove: (item: ClipboardItem) => void
  onSetTag: (item: ClipboardItem, tag: string | null) => void
}

const PRESET_TAGS = ['常用', '代码', '工作', '灵感', '重要']

export default function ContextMenu({
  menu,
  allTags,
  onClose,
  onPaste,
  onTogglePin,
  onRemove,
  onSetTag,
}: ContextMenuProps) {
  const [showSubMenu, setShowSubMenu] = useState(false)
  const [isCreatingTag, setIsCreatingTag] = useState(false)
  const [newTagInput, setNewTagInput] = useState('')
  const menuRef = useRef<HTMLDivElement | null>(null)
  const inputRef = useRef<HTMLInputElement | null>(null)

  // 合并预设与已有的所有分类，保持去重
  const mergedTags = Array.from(new Set([...PRESET_TAGS, ...allTags])).filter(Boolean)

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
        onMouseEnter={() => setShowSubMenu(true)}
        onMouseLeave={() => !isCreatingTag && setShowSubMenu(false)}
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
              return (
                <div
                  key={tag}
                  className={`menu-item tag-item${isActive ? ' active-tag' : ''}`}
                  onClick={() => {
                    onSetTag(item, tag)
                    onClose()
                  }}
                >
                  <span className="tag-dot" />
                  <span className="item-label">{tag}</span>
                  {isActive && <span className="check-glyph">&#xE73E;</span>}
                </div>
              )
            })}

            <div className="menu-divider" />

            {/* 新建分类输入框 */}
            {!isCreatingTag ? (
              <div
                className="menu-item new-tag"
                onClick={(e) => {
                  e.stopPropagation()
                  setIsCreatingTag(true)
                }}
              >
                <span className="item-glyph">&#xE710;</span>
                <span className="item-label">新建分类标签...</span>
              </div>
            ) : (
              <form className="new-tag-form" onSubmit={handleCreateTagSubmit} onClick={(e) => e.stopPropagation()}>
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
