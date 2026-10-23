import React, { useCallback, useEffect, useRef, useState } from 'react'
import type { ClipboardItem } from '../types'
import { TAG_COLOR_PALETTE, getTagColor } from '../tagColors'
import { extractOcrText, hasOcrText } from '../ocr'

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
  onCopyText?: (text: string) => void
  onRunOcr?: (item: ClipboardItem) => void
  onViewOcr?: (item: ClipboardItem) => void
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
  onCopyText,
  onRunOcr,
  onViewOcr,
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
  //
  // 注意这里**不**碰位置：位置由 `<div key>` 重新挂载 + ref 回调一次性算出，
  // 一旦把位置也塞进这个 effect，就会和测量逻辑形成互相触发的更新循环
  // （测量 → setState → 重置 → 再测量），重则抛 Maximum update depth exceeded、
  // 整棵 React 树被卸载成白屏。
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

  // 定位：在节点挂载的那一帧量一次真实尺寸，直接写 style，不经过 React state。
  //
  // 为什么不用 state + useLayoutEffect：那会让"测量 → setState → 重渲染"
  // 和别的 effect 互相触发，形成更新死循环（生产构建里会直接抛
  // Maximum update depth exceeded，整棵 React 树被卸载 —— 白屏）。
  // 用 ref 回调 + 命令式写 style，一次算完就结束，循环从结构上不存在。
  //
  // 配合 `<div key={menuKey}>`：每次右键都是全新节点，ref 回调必然重跑，
  // 所以不需要"位置变了要重新测量"这类依赖。
  const menuKey = `${menu.item?.id ?? '-'}:${menu.x}:${menu.y}`

  const positionMenu = useCallback(
    (el: HTMLDivElement | null) => {
      menuRef.current = el
      if (!el) return

      const gap = 8
      const rect = el.getBoundingClientRect()

      // 期望位置以鼠标坐标为准；放不下就整体左移/上移，保证完整可见。
      const x = Math.max(gap, Math.min(menu.x, window.innerWidth - rect.width - gap))
      const y = Math.max(gap, Math.min(menu.y, window.innerHeight - rect.height - gap))

      el.style.left = `${x}px`
      el.style.top = `${y}px`
      // 位置算好了才显形：避免菜单先在旧位置/左上角闪一下
      el.style.visibility = 'visible'
    },
    [menu.x, menu.y],
  )

  // 子菜单的翻边与高度限制。
  //
  // 同样的思路：不用 useLayoutEffect + state，而是每次 showSubMenu 变化后
  // 量一次、直接改 class 和 style。翻边是"改一次就不再变"的幂等操作，
  // 不存在反复触发的风险。
  useEffect(() => {
    if (!showSubMenu) return
    const menuEl = menuRef.current
    if (!menuEl) return

    const subMenu = menuEl.querySelector<HTMLElement>('.paste-sub-menu')
    if (!subMenu) return

    // 先按默认方向（右侧）量一次
    subMenu.classList.remove('flip-left')
    subMenu.style.maxHeight = ''

    const gap = 10
    const first = subMenu.getBoundingClientRect()
    if (first.right > window.innerWidth - gap) {
      // 右边放不下：翻到主菜单左侧，翻完再量一次高度
      subMenu.classList.add('flip-left')
    }

    const rect = subMenu.getBoundingClientRect()
    const available = window.innerHeight - rect.top - gap
    // 标签特别多时限制高度、内部滚动，避免子菜单溢出视口底部
    if (available > 120 && rect.height > available) {
      subMenu.style.maxHeight = `${available}px`
    }
  }, [showSubMenu, mergedTags.length, isCreatingTag])

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

  return (
    <div
      key={menuKey}
      ref={positionMenu}
      className="paste-context-menu"
      style={{ visibility: 'hidden' }}
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

      {/* 2.5 图片卡片：OCR 文本提取 */}
      {item.type === 'Image' && (
        <>
          {hasOcrText(item.content, item.type) ? (
            <>
              <div
                className="menu-item"
                onClick={() => {
                  onViewOcr?.(item)
                  onClose()
                }}
              >
                <span className="item-glyph">&#xE8A5;</span>
                <span className="item-label">查看识别文字</span>
              </div>
              <div
                className="menu-item"
                onClick={() => {
                  const ocrText = extractOcrText(item.content)
                  if (ocrText && onCopyText) {
                    onCopyText(ocrText)
                  }
                  onClose()
                }}
              >
                <span className="item-glyph">&#xE8C8;</span>
                <span className="item-label">复制图片中的文字 (OCR)</span>
              </div>
            </>
          ) : (
            <div
              className="menu-item"
              title="已存在的图片也能随时补做一次文字识别"
              onClick={() => {
                onRunOcr?.(item)
                onClose()
              }}
            >
              <span className="item-glyph">&#xE8C1;</span>
              <span className="item-label">识别图片中的文字 (OCR)</span>
            </div>
          )}
        </>
      )}

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
