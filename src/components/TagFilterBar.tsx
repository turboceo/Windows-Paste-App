import { For, Show, createEffect, createSignal } from 'solid-js'
import { TAG_COLOR_PALETTE, getTagColor } from '../tagColors'
import { mount } from '../solid-utils'

interface TagFilterBarProps {
  allTags: () => string[]
  activeTag: () => string | null
  tagColors: () => Record<string, string>
  onFilterTag: (tag: string | null) => void
  onSaveTagColor: (tag: string, color: string) => void
  /** 在标签栏上新建分类。`color` 是用户当场挑的颜色。 */
  onCreateTag: (tag: string, color: string) => void
  /** 触发"删除分类"确认流程（确认框由 App 管理） */
  onDeleteTag: (tag: string) => void
}

/**
 * 标签栏上的两种浮层。
 *
 * 合并成一个信号而不是两个：这样"点外面关闭"和"Esc 关闭"只需要一套监听，
 * 也不会出现两个浮层同时开着的中间态。
 */
type TagBarMenu =
  | { kind: 'manage'; tag: string; x: number; y: number }
  | { kind: 'create'; x: number; y: number; anchorTop: number; anchorRight: number }

/**
 * YouTube 风格的 feed-filter-chip-bar：
 * - 横向平滑滚动，带左右渐变遮罩与导航箭头；
 * - 鼠标滚轮直接在胶囊栏上横向平移；
 * - 右键任意胶囊弹出管理浮层，可随时改色或删除分类；
 * - 末尾常驻「+」按钮，新建分类（含选色）在这里完成。
 */
export default function TagFilterBar(props: TagFilterBarProps) {
  let scrollEl: HTMLDivElement | undefined
  const [canScrollLeft, setCanScrollLeft] = createSignal(false)
  const [canScrollRight, setCanScrollRight] = createSignal(false)
  const [menu, setMenu] = createSignal<TagBarMenu | null>(null)

  // 新建浮层内部的输入态
  const [draftName, setDraftName] = createSignal('')
  const [draftColor, setDraftColor] = createSignal<string>(TAG_COLOR_PALETTE[0])
  const [draftError, setDraftError] = createSignal('')

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
  //
  // `kind: 'manage'` 用光标坐标；`kind: 'create'` 用「+」按钮的锚点，
  // 这样新建面板永远挂在按钮正下方，不会跟着鼠标乱跑。
  const positionPopover = (el: HTMLDivElement) => {
    popoverEl = el
    const state = menu()
    if (!state) return
    const gap = 10
    const rect = el.getBoundingClientRect()

    let x: number
    let y: number
    if (state.kind === 'create') {
      // 右对齐到「+」按钮的右边缘，超宽时夹回视口内
      x = Math.max(gap, Math.min(state.anchorRight - rect.width, window.innerWidth - rect.width - gap))
      y = Math.max(gap, state.anchorTop)
    } else {
      x = Math.max(gap, Math.min(state.x, window.innerWidth - rect.width - gap))
      y = Math.max(gap, Math.min(state.y, window.innerHeight - rect.height - gap))
    }

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

  // 浮层关掉后清掉草稿，下次是一次全新的开始。
  // 聚焦交给 TagBarPopover 自己：它的挂载时机正好是输入框出现的时机，
  // 把 ref 穿过组件边界反而要多传一个回调。
  createEffect(
    () => menu(),
    (state) => {
      if (!state) {
        setDraftName('')
        setDraftColor(TAG_COLOR_PALETTE[0])
        setDraftError('')
      }
    },
  )

  const openMenuFor = (tag: string, event: MouseEvent) => {
    event.preventDefault()
    event.stopPropagation()
    // 重新打开时先归零坐标让 ref 回调重跑：keyed 渲染（Show + 新对象）
    // 每次都产生全新节点，所以 positionPopover 必然重新执行
    setMenu({ kind: 'manage', tag, x: event.clientX, y: event.clientY })
  }

  /** 打开新建浮层。锚点取按钮自身的位置，保证面板挂在它正下方。 */
  const openCreateMenu = (event: MouseEvent) => {
    event.preventDefault()
    event.stopPropagation()
    const rect = (event.currentTarget as HTMLElement).getBoundingClientRect()
    setMenu({
      kind: 'create',
      x: event.clientX,
      y: event.clientY,
      anchorTop: rect.bottom + 8,
      anchorRight: rect.right,
    })
  }

  /**
   * 提交新建分类。
   *
   * 三处校验，都是「不做就会留下一半状态」的类型：
   * 空名、与现有分类重名、长度超限。
   */
  const submitDraft = () => {
    const name = draftName().trim()
    if (!name) {
      setDraftError('请输入分类名称')
      return
    }
    if (name.length > 12) {
      setDraftError('分类名称最多 12 个字符')
      return
    }
    if (props.allTags().some((tag) => tag === name)) {
      setDraftError('已存在同名分类')
      return
    }

    props.onCreateTag(name, draftColor())
    setMenu(null)
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

        {/* 新建分类入口。常驻在这里而不是塞进右键菜单：
            标签栏能同时看到全部现有分类，重名校验也才有意义。 */}
        <button
          class="tag-create-btn"
          title="新建分类标签"
          aria-label="新建分类标签"
          onClick={openCreateMenu}
        >
          &#xE710;
        </button>
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

      {/* 标签浮层：右键胶囊 = 管理（换色/删除）；点「+」 = 新建 */}
      <Show when={menu()} keyed>
        {(state) => (
          <div
            ref={positionPopover}
            class="tag-chip-popover"
            onContextMenu={(event) => event.preventDefault()}
          >
            <TagBarPopover
              menu={state}
              tagColors={props.tagColors}
              draftName={draftName}
              draftColor={draftColor}
              draftError={draftError}
              onDraftName={(value) => {
                setDraftName(value)
                setDraftError('')
              }}
              onDraftColor={setDraftColor}
              onSubmitDraft={submitDraft}
              onSaveTagColor={props.onSaveTagColor}
              onDeleteTag={(tag) => {
                setMenu(null)
                props.onDeleteTag(tag)
              }}
            />
          </div>
        )}
      </Show>
    </div>
  )
}

/**
 * 浮层内容。抽成独立组件而不是内联在 `<Show>` 的回调里，
 * 是为了让 TypeScript 能正常收窄 `menu.kind`——`keyed` 的回调参数在
 * 联合类型下会被推成 `true`，`menu.tag` 直接报不存在。
 *
 * 这里用提前 return 的三元结构而不是 `<Show>`：两个分支的类型差别在
 * `kind` 字段上，编译期就能把联合分好，读起来也更直白。
 */
function TagBarPopover(props: {
  menu: TagBarMenu
  tagColors: () => Record<string, string>
  draftName: () => string
  draftColor: () => string
  draftError: () => string
  onDraftName: (value: string) => void
  onDraftColor: (color: string) => void
  onSubmitDraft: () => void
  onSaveTagColor: (tag: string, color: string) => void
  onDeleteTag: (tag: string) => void
}) {
  const menu = props.menu

  if (menu.kind === 'manage') {
    const tag = menu.tag
    return (
      <>
        <div class="popover-header">
          <span
            class="tag-dot"
            style={{ 'background-color': getTagColor(tag, props.tagColors()) }}
          />
          <span class="popover-title">{tag}</span>
        </div>

        <div class="popover-section-label">更改颜色：</div>
        <div class="color-palette-grid">
          <For each={TAG_COLOR_PALETTE}>
            {(color) => (
              <button
                type="button"
                class={[
                  'palette-color-btn',
                  { active: getTagColor(tag, props.tagColors()) === color },
                ]}
                style={{ 'background-color': color }}
                title={color}
                // 换色后保持浮层打开，让用户看到实时生效的效果
                onClick={() => props.onSaveTagColor(tag, color)}
              />
            )}
          </For>
        </div>

        <div class="popover-divider" />

        <button
          type="button"
          class="popover-delete-btn"
          onClick={() => props.onDeleteTag(tag)}
        >
          <span class="del-glyph">&#xE74D;</span>
          删除该分类
        </button>
      </>
    )
  }

  return (
    <>
      {/* 新建分类：名字 + 调色盘 + 确认 */}
      <div class="popover-header">
        <span class="popover-title">新建分类</span>
      </div>

      <form
        class="popover-create-form"
        onSubmit={(event) => {
          event.preventDefault()
          props.onSubmitDraft()
        }}
      >
        <div class="new-tag-input-row">
          <span
            class="selected-color-preview"
            style={{ 'background-color': props.draftColor() }}
            title="当前选择的颜色"
          />
          <input
            ref={(el) => queueMicrotask(() => el.focus())}
            class="new-tag-input"
            type="text"
            placeholder="分类名称..."
            value={props.draftName()}
            maxlength={12}
            onInput={(event) => props.onDraftName(event.currentTarget.value)}
          />
          <button type="submit" class="new-tag-confirm" title="确认新建">
            &#xE73E;
          </button>
        </div>

        <div class="color-palette-row">
          <For each={TAG_COLOR_PALETTE}>
            {(color) => (
              <button
                type="button"
                class={["palette-color-btn", { active: props.draftColor() === color }]}
                style={{ 'background-color': color }}
                title={color}
                onClick={() => props.onDraftColor(color)}
              />
            )}
          </For>
        </div>

        <Show when={props.draftError()}>
          <div class="popover-error">{props.draftError()}</div>
        </Show>
      </form>
    </>
  )
}
