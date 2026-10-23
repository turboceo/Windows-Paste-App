import { For, Show, createEffect, createSignal, onCleanup } from 'solid-js'
import type { ClipboardItem } from '../types'
import { TAG_COLOR_PALETTE, getTagColor } from '../tagColors'
import { extractOcrText, hasOcrText } from '../ocr'
import { useWindowEvent } from '../solid-utils'

export interface ContextMenuState {
  visible: boolean
  x: number
  y: number
  item: ClipboardItem | null
}

interface ContextMenuProps {
  menu: () => ContextMenuState
  allTags: () => string[]
  tagColors: () => Record<string, string>
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
 * 用 debounce 而不是 throttle：子菜单与主菜单之间有 4px 间隙，鼠标斜着
 * 划过去时会先落到空隙里触发 mouseleave。等一会儿再收，给它穿过的余量。
 */
const SUBMENU_CLOSE_DELAY = 160

/**
 * 卡片右键菜单。
 *
 * 定位策略（Solid 版本）：每次菜单打开都是一个全新的节点（由外层的
 * `keyed` 渲染保证），所以 ref 回调只会在挂载时跑一次，量完真实尺寸
 * 直接把 left/top 写进 style。整个过程不触发任何响应式更新，
 * 不存在"测量 → 更新 state → 再测量"的循环。
 */
export default function ContextMenu(props: ContextMenuProps) {
  const [showSubMenu, setShowSubMenu] = createSignal(false)
  const [isCreatingTag, setIsCreatingTag] = createSignal(false)
  const [newTagInput, setNewTagInput] = createSignal('')
  const [selectedColor, setSelectedColor] = createSignal<string>(TAG_COLOR_PALETTE[0])
  const [colorEditingTag, setColorEditingTag] = createSignal<string | null>(null)

  let menuEl: HTMLDivElement | undefined
  let subMenuEl: HTMLDivElement | undefined
  let inputEl: HTMLInputElement | undefined
  /** 收起子菜单的延时任务；鼠标回到主菜单项时要取消它 */
  let closeTimer: number | null = null

  // 只列数据库里真实存在的分类。
  // 声明必须放在所有 effect 之前：Solid 的 compute 在创建时就同步执行，
  // 而 const 在初始化前处于暂时性死区（TDZ），后置声明会直接抛
  // "Cannot access before initialization"。
  const mergedTags = () => Array.from(new Set(props.allTags())).filter(Boolean)

  const cancelScheduledClose = () => {
    if (closeTimer !== null) {
      window.clearTimeout(closeTimer)
      closeTimer = null
    }
  }

  const openSubMenu = () => {
    cancelScheduledClose()
    setShowSubMenu(true)
  }

  const scheduleCloseSubMenu = () => {
    // 正在输入新标签时绝不收起，否则输一半的标签会连同菜单一起消失
    if (isCreatingTag()) return
    cancelScheduledClose()
    closeTimer = window.setTimeout(() => {
      closeTimer = null
      setShowSubMenu(false)
    }, SUBMENU_CLOSE_DELAY)
  }

  onCleanup(cancelScheduledClose)

  // 菜单关掉、或右键了另一张卡片时，重置子菜单与输入态。
  // 位置不在这里重置——它由节点重新挂载时的一次性测量负责。
  createEffect(
    // compute：只读依赖
    () => [props.menu().visible, props.menu().item?.id],
    // apply：重置子菜单与输入态
    () => {
      cancelScheduledClose()
      setShowSubMenu(false)
      setIsCreatingTag(false)
      setNewTagInput('')
      setColorEditingTag(null)
      setSelectedColor(TAG_COLOR_PALETTE[0])
    },
  )

  // 点外面或按 Esc 关闭。用 window 监听，避免菜单内部的点击冒泡到卡片。
  useWindowEvent('mousedown', (event) => {
    if (!props.menu().visible) return
    if (menuEl && !menuEl.contains(event.target as Node)) {
      props.onClose()
    }
  })

  useWindowEvent('keydown', (event) => {
    if (!props.menu().visible) return
    if (event.key === 'Escape') props.onClose()
  })

  // 进入输入态后自动聚焦
  createEffect(
    () => isCreatingTag(),
    (creating) => {
      if (creating) queueMicrotask(() => inputEl?.focus())
    },
  )

  // 子菜单翻边与高度限制：只在展开时量一次，直接改 class/style。
  createEffect(
    () => [showSubMenu(), mergedTags().length, isCreatingTag()],
    () => {
      if (!showSubMenu()) return
      // 让 Solid 先完成子菜单的 DOM 插入
      queueMicrotask(() => {
        const el = subMenuEl
        if (!el) return

        el.classList.remove('flip-left')
        el.style.maxHeight = ''

        const gap = 10
        const first = el.getBoundingClientRect()
        if (first.right > window.innerWidth - gap) {
          el.classList.add('flip-left')
        }

        const rect = el.getBoundingClientRect()
        const available = window.innerHeight - rect.top - gap
        if (available > 120 && rect.height > available) {
          el.style.maxHeight = `${available}px`
        }
      })
    },
  )

  /** 挂载时量一次真实尺寸，把菜单摆到不与视口边缘重叠的位置 */
  const positionMenu = (el: HTMLDivElement) => {
    menuEl = el
    const { x, y } = props.menu()
    const gap = 8
    const rect = el.getBoundingClientRect()
    const left = Math.max(gap, Math.min(x, window.innerWidth - rect.width - gap))
    const top = Math.max(gap, Math.min(y, window.innerHeight - rect.height - gap))
    el.style.left = `${left}px`
    el.style.top = `${top}px`
    el.style.visibility = 'visible'
  }

  const submitNewTag = (event: SubmitEvent) => {
    event.preventDefault()
    const item = props.menu().item
    const trimmed = newTagInput().trim()
    if (!item || !trimmed) return

    props.onSaveTagColor?.(trimmed, selectedColor())
    props.onSetTag(item, trimmed)
    setIsCreatingTag(false)
    setNewTagInput('')
    props.onClose()
  }

  return (
    <Show when={props.menu().visible && props.menu().item}>
      {(item) => (
        <div
          ref={positionMenu}
          class="paste-context-menu"
          style={{ visibility: 'hidden' }}
          onContextMenu={(event) => event.preventDefault()}
        >
          {/* 1. 立即粘贴 */}
          <div
            class="menu-item"
            onClick={() => {
              props.onPaste(item())
              props.onClose()
            }}
          >
            <span class="item-glyph">&#xE77F;</span>
            <span class="item-label">立即粘贴</span>
            <kbd class="item-shortcut">Enter</kbd>
          </div>

          {/* 2. 置顶/取消置顶 */}
          <div
            class="menu-item"
            onClick={() => {
              props.onTogglePin(item())
              props.onClose()
            }}
          >
            <span class="item-glyph">{item().pinned ? '\uE840' : '\uE718'}</span>
            <span class="item-label">{item().pinned ? '取消置顶' : '置顶卡片'}</span>
            <kbd class="item-shortcut">Ctrl+P</kbd>
          </div>

          {/* 3. 图片卡片：OCR 文本提取 */}
          <Show when={item().type === 'Image'}>
            <Show
              when={hasOcrText(item().content, item().type)}
              fallback={
                <div
                  class="menu-item"
                  title="已存在的图片也能随时补做一次文字识别"
                  onClick={() => {
                    props.onRunOcr?.(item())
                    props.onClose()
                  }}
                >
                  <span class="item-glyph">&#xE8C1;</span>
                  <span class="item-label">识别图片中的文字 (OCR)</span>
                </div>
              }
            >
              <div
                class="menu-item"
                onClick={() => {
                  props.onViewOcr?.(item())
                  props.onClose()
                }}
              >
                <span class="item-glyph">&#xE8A5;</span>
                <span class="item-label">查看识别文字</span>
              </div>
              <div
                class="menu-item"
                onClick={() => {
                  const ocrText = extractOcrText(item().content)
                  if (ocrText) props.onCopyText?.(ocrText)
                  props.onClose()
                }}
              >
                <span class="item-glyph">&#xE8C8;</span>
                <span class="item-label">复制图片中的文字 (OCR)</span>
              </div>
            </Show>
          </Show>

          <div class="menu-divider" />

          {/* 4. 添加到分类 / 修改分类（展开二级菜单） */}
          <div
            class="menu-item has-submenu"
            onMouseEnter={openSubMenu}
            onMouseLeave={scheduleCloseSubMenu}
          >
            <span class="item-glyph">&#xE8EC;</span>
            <span class="item-label">
              {item().tag ? `分类: ${item().tag}` : '添加到分类'}
            </span>
            <span class="arrow-glyph">&#xE76C;</span>

            <Show when={showSubMenu()}>
              <div class="paste-sub-menu" ref={subMenuEl}>
                <Show when={item().tag}>
                  <div
                    class="menu-item remove-tag"
                    onClick={() => {
                      props.onSetTag(item(), null)
                      props.onClose()
                    }}
                  >
                    <span class="item-glyph">&#xE711;</span>
                    <span class="item-label">清除当前分类</span>
                  </div>
                  <div class="menu-divider" />
                </Show>

                <div class="submenu-title">选择分类：</div>

                <For each={mergedTags()}>
                  {(tag) => {
                    const dotColor = () => getTagColor(tag, props.tagColors())
                    const isActive = () => item().tag === tag
                    const isEditingThis = () => colorEditingTag() === tag
                    return (
                      <div class="tag-item-container">
                        <div
                          class={["menu-item", "tag-item", { 'active-tag': isActive() }]}
                          onClick={() => {
                            props.onSetTag(item(), tag)
                            props.onClose()
                          }}
                        >
                          <span class="tag-dot" style={{ 'background-color': dotColor() }} />
                          <span class="item-label">{tag}</span>
                          <Show when={isActive()}>
                            <span class="check-glyph">&#xE73E;</span>
                          </Show>
                          <button
                            type="button"
                            class="tag-palette-trigger"
                            title="修改此分类颜色"
                            onClick={(event) => {
                              event.stopPropagation()
                              cancelScheduledClose()
                              setColorEditingTag(isEditingThis() ? null : tag)
                            }}
                          >
                            &#xE790;
                          </button>
                        </div>

                        <Show when={isEditingThis()}>
                          <div class="tag-item-inline-palette" onClick={(e) => e.stopPropagation()}>
                            <For each={TAG_COLOR_PALETTE}>
                              {(color) => (
                                <button
                                  type="button"
                                  class={["palette-color-btn", { active: dotColor() === color }]}
                                  style={{ 'background-color': color }}
                                  title={color}
                                  onClick={() => props.onSaveTagColor?.(tag, color)}
                                />
                              )}
                            </For>
                          </div>
                        </Show>
                      </div>
                    )
                  }}
                </For>

                <div class="menu-divider" />

                {/* 新建分类输入框与调色盘 */}
                <Show
                  when={isCreatingTag()}
                  fallback={
                    <div
                      class="menu-item new-tag"
                      onClick={(event) => {
                        event.stopPropagation()
                        // 进入输入态后子菜单不能再被自动收起
                        cancelScheduledClose()
                        setIsCreatingTag(true)
                      }}
                    >
                      <span class="item-glyph">&#xE710;</span>
                      <span class="item-label">新建分类标签</span>
                    </div>
                  }
                >
                  <form class="new-tag-form" onSubmit={submitNewTag} onClick={(e) => e.stopPropagation()}>
                    <div class="new-tag-input-row">
                      <span
                        class="selected-color-preview"
                        style={{ 'background-color': selectedColor() }}
                        title="当前选择的颜色"
                      />
                      <input
                        ref={inputEl}
                        class="new-tag-input"
                        type="text"
                        placeholder="标签名称..."
                        value={newTagInput()}
                        maxlength={12}
                        onInput={(event) => setNewTagInput(event.currentTarget.value)}
                      />
                      <button type="submit" class="new-tag-confirm" title="确认">
                        &#xE73E;
                      </button>
                    </div>
                    <div class="color-palette-row">
                      <For each={TAG_COLOR_PALETTE}>
                        {(color) => (
                          <button
                            type="button"
                            class={["palette-color-btn", { active: selectedColor() === color }]}
                            style={{ 'background-color': color }}
                            title={color}
                            onClick={() => setSelectedColor(color)}
                          />
                        )}
                      </For>
                    </div>
                  </form>
                </Show>
              </div>
            </Show>
          </div>

          <div class="menu-divider" />

          {/* 5. 删除卡片 */}
          <div
            class="menu-item danger"
            onClick={() => {
              props.onRemove(item())
              props.onClose()
            }}
          >
            <span class="item-glyph">&#xE74D;</span>
            <span class="item-label">删除记录</span>
            <kbd class="item-shortcut">Del</kbd>
          </div>
        </div>
      )}
    </Show>
  )
}
