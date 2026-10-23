import { For, Show, createEffect, createSignal } from 'solid-js'
import { TAG_COLOR_PALETTE, getTagColor } from '../tagColors'
import { mount } from '../solid-utils'

interface TagFilterBarProps {
  allTags: () => string[]
  activeTag: () => string | null
  tagColors: () => Record<string, string>
  onFilterTag: (tag: string | null) => void
  onSaveTagColor: (tag: string, color: string) => void
  /** 触发"删除分类"确认流程（确认框由 App 管理） */
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
 * - 右键任意胶囊弹出管理浮层，可随时改色或删除分类。
 */
export default function TagFilterBar(props: TagFilterBarProps) {
  let scrollEl: HTMLDivElement | undefined
  const [canScrollLeft, setCanScrollLeft] = createSignal(false)
  const [canScrollRight, setCanScrollRight] = createSignal(false)
  const [menu, setMenu] = createSignal<TagMenuState | null>(null)

  let popoverEl: HTMLDivElement | undefined

  const checkScroll = () => {
    if (!scrollEl) return
    const { scrollLeft, scrollWidth, clientWidth } = scrollEl
    setCanScrollLeft(scrollLeft > 3)
    setCanScrollRight(scrollLeft < scrollWidth - clientWidth - 3)
  }

  mount(() => {
    checkScroll()
    if (!scrollEl) return
    const observer = new ResizeObserver(() => checkScroll())
    observer.observe(scrollEl)
    return () => observer.disconnect()
  })

  // 标签数量变化后内容宽度会变，滚动按钮的可用性要重算
  createEffect(
    () => props.allTags().length,
    () => checkScroll(),
  )

  const scrollBy = (offset: number) => {
    scrollEl?.scrollBy({ left: offset, behavior: 'smooth' })
  }

  // 浮层的视口边界保护：同 ContextMenu 的思路，挂载时量一次、
  // 直接写 style，不经过响应式更新。
  const positionPopover = (el: HTMLDivElement) => {
    popoverEl = el
    const state = menu()
    if (!state) return
    const gap = 10
    const rect = el.getBoundingClientRect()
    const x = Math.max(gap, Math.min(state.x, window.innerWidth - rect.width - gap))
    const y = Math.max(gap, Math.min(state.y, window.innerHeight - rect.height - gap))
    el.style.left = `${x}px`
    el.style.top = `${y}px`
  }

  // 点击外部或按 Esc 关闭浮层。监听在 window 上，捕获点击目标是浮层外的地方。
  createEffect(
    () => menu(),
    (state) => {
      if (!state) return
      const onDown = (event: MouseEvent) => {
        if (popoverEl && !popoverEl.contains(event.target as Node)) {
          setMenu(null)
        }
      }
      const onKey = (event: KeyboardEvent) => {
        if (event.key === 'Escape') setMenu(null)
      }
      window.addEventListener('mousedown', onDown)
      window.addEventListener('keydown', onKey)
      return () => {
        window.removeEventListener('mousedown', onDown)
        window.removeEventListener('keydown', onKey)
      }
    },
  )

  const openMenuFor = (tag: string, event: MouseEvent) => {
    event.preventDefault()
    event.stopPropagation()
    // 重新打开时先归零坐标让 ref 回调重跑：keyed 渲染（Show + 新对象）
    // 每次都产生全新节点，所以 positionPopover 必然重新执行
    setMenu({ tag, x: event.clientX, y: event.clientY })
  }

  return (
    <div class="feed-filter-bar-wrapper">
      <Show when={canScrollLeft()}>
        <div class="chip-nav-wrapper left">
          <button
            class="chip-nav-btn"
            title="向左滚动"
            aria-label="向左滚动标签"
            onClick={() => scrollBy(-200)}
          >
            &#xE76B;
          </button>
        </div>
      </Show>

      <div
        ref={scrollEl}
        class="feed-filter-chip-bar"
        onScroll={checkScroll}
        onWheel={(event) => {
          if (!scrollEl || event.deltaY === 0) return
          scrollEl.scrollLeft += event.deltaY * 0.9
          checkScroll()
        }}
      >
        <button
          class={["tag-filter-pill", { active: props.activeTag() === null }]}
          onClick={() => props.onFilterTag(null)}
        >
          全部
        </button>

        <For each={props.allTags()}>
          {(tag) => {
            const color = () => getTagColor(tag, props.tagColors())
            const active = () => props.activeTag() === tag
            return (
              <button
                class={["tag-filter-pill", { active: active() }]}
                style={active() ? { 'background-color': color(), 'border-color': color() } : undefined}
                title={`${tag} · 右键可修改颜色或删除`}
                onClick={() => props.onFilterTag(active() ? null : tag)}
                onContextMenu={(event) => openMenuFor(tag, event)}
              >
                <span
                  class="tag-dot"
                  style={{ 'background-color': active() ? '#ffffff' : color() }}
                />
                <span class="tag-name">{tag}</span>
              </button>
            )
          }}
        </For>
      </div>

      <Show when={canScrollRight()}>
        <div class="chip-nav-wrapper right">
          <button
            class="chip-nav-btn"
            title="向右滚动"
            aria-label="向右滚动标签"
            onClick={() => scrollBy(200)}
          >
            &#xE76C;
          </button>
        </div>
      </Show>

      {/* 标签管理浮层：换色、删除 */}
      <Show when={menu()} keyed>
        {(state) => (
          <div
            ref={positionPopover}
            class="tag-chip-popover"
            style={{ left: `${state.x}px`, top: `${state.y}px` }}
            onContextMenu={(event) => event.preventDefault()}
          >
            <div class="popover-header">
              <span
                class="tag-dot"
                style={{ 'background-color': getTagColor(state.tag, props.tagColors()) }}
              />
              <span class="popover-title">{state.tag}</span>
            </div>

            <div class="popover-section-label">更改颜色：</div>
            <div class="color-palette-grid">
              <For each={TAG_COLOR_PALETTE}>
                {(color) => (
                  <button
                    type="button"
                    class={[
                      'palette-color-btn',
                      { active: getTagColor(state.tag, props.tagColors()) === color },
                    ]}
                    style={{ 'background-color': color }}
                    title={color}
                    // 换色后保持浮层打开，让用户看到实时生效的效果
                    onClick={() => props.onSaveTagColor(state.tag, color)}
                  />
                )}
              </For>
            </div>

            <div class="popover-divider" />

            <button
              type="button"
              class="popover-delete-btn"
              onClick={() => {
                const target = state.tag
                setMenu(null)
                props.onDeleteTag(target)
              }}
            >
              <span class="del-glyph">&#xE74D;</span>
              删除该分类
            </button>
          </div>
        )}
      </Show>
    </div>
  )
}
