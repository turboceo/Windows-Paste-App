import { For, Show, createEffect, createMemo, createSignal } from 'solid-js'
import type { ClipboardItem } from '../types'
import { relativeTime } from '../time'
import { hasOcrText } from '../ocr'

interface ClipCardProps {
  item: ClipboardItem
  /** 当前处于哪个分类视图下（null 表示「全部」） */
  activeTag?: () => string | null
  /**
   * 缩略图 data URL。`undefined` = 还没拉过，`null` = 拉过但取不到。
   * 两者在视觉上都是占位，但前者会触发一次拉取。
   */
  thumbnail: () => string | null | undefined
  onRequestThumbnail: (itemId: number) => void
  selected: () => boolean
  tagColor?: () => string | undefined
  onSelect: (item: ClipboardItem) => void
  onDoubleClick?: (item: ClipboardItem) => void
  onPaste: (item: ClipboardItem) => void
  onTogglePin: (item: ClipboardItem) => void
  onRemove: (item: ClipboardItem) => void
  onContextMenu?: (item: ClipboardItem, event: MouseEvent) => void
}

/**
 * One history card styled after Paste.app.
 *
 * 顶部是同色系的彩色色带（类型名 + 相对时间），下面是纯白正文，
 * 底部一行居中的统计信息。
 *
 * Solid 版本用 `createMemo` 承接原来 `useMemo` 的职责：分类判定、
 * 类型图标、角标文案都由 item 派生，并保证同一份 item 只算一次。
 */
export default function ClipCard(props: ClipCardProps) {
  const item = () => props.item

  const isUrl = createMemo(() => {
    if (item().type !== 'Text') return false
    return /^https?:\/\/[^\s]+$/i.test(item().content.trim())
  })

  const urlDomain = createMemo(() => {
    if (!isUrl()) return ''
    try {
      return new URL(item().content.trim()).hostname.replace(/^www\./, '')
    } catch {
      return ''
    }
  })

  const isColorCode = createMemo(() => {
    if (item().type !== 'Text') return false
    const trimmed = item().content.trim()
    return (
      /^#([0-9a-f]{3}|[0-9a-f]{6}|[0-9a-f]{8})$/i.test(trimmed) ||
      /^rgba?\((\s*\d+\s*,?){3,4}\)$/i.test(trimmed)
    )
  })

  const fileList = createMemo(() => {
    if (item().type !== 'Files') return []
    return item()
      .content.split(/\r?\n/)
      .map((p) => p.trim())
      .filter(Boolean)
      .map((fullPath) => ({ name: fullPath.split(/[\\/]/).pop() || fullPath, fullPath }))
  })

  const semanticCategory = createMemo(() => {
    if (item().type === 'Image') return 'image'
    if (item().type === 'Files') return 'files'
    if (isUrl()) return 'link'
    if (isColorCode()) return 'color'
    return 'text'
  })

  const typeGlyph = createMemo(() => {
    switch (semanticCategory()) {
      case 'image':
        return '\uEB9F'
      case 'files':
        return '\uE8B7'
      case 'link':
        return '\uE71B'
      case 'color':
        return '\uE790'
      default:
        return '\uE8A5'
    }
  })

  const typeName = createMemo(() => {
    switch (semanticCategory()) {
      case 'image':
        return '图像'
      case 'files':
        return '文件'
      case 'link':
        return '链接'
      case 'color':
        return '颜色'
      default:
        return '文本'
    }
  })

  const metaBadge = createMemo(() => {
    switch (semanticCategory()) {
      case 'link':
        return urlDomain() || '网页'
      case 'files':
        return `${fileList().length} 个文件`
      case 'color':
        return item().content.trim()
      case 'image': {
        const match = item().content.match(/(\d+)\s*x\s*(\d+)/i)
        const sizeStr = match ? `${match[1]} × ${match[2]}` : '图片'
        return hasOcrText(item().content, item().type) ? `${sizeStr} · 含文字` : sizeStr
      }
      default: {
        const len = item().content.length
        if (len === 0) return ''
        // 被截断过的内容加个后缀，避免用户以为复制的东西只有这么点
        return item().truncated ? `${len}+ 字符` : `${len} 字符`
      }
    }
  })

  const timeLabel = createMemo(() => relativeTime(item().createdAt))

  /** 加载失败的缩略图不再重试，直接显示占位 */
  const [imageFailed, setImageFailed] = createSignal(false)

  const imageSrc = () => (item().type === 'Image' ? props.thumbnail() ?? null : null)

  // 缩略图换了（重新推送过）就重置失败状态
  createEffect(
    () => imageSrc(),
    () => {
      setImageFailed(false)
    },
  )

  /** 缩略图按需拉取：卡片被挂载（即滚动到可见区域）时才请求 */
  createEffect(
    () => [item().hasImage, props.thumbnail()] as const,
    ([hasImage, thumb]) => {
      if (hasImage && thumb === undefined) {
        props.onRequestThumbnail(item().id)
      }
    },
  )

  const tagStyle = () => {
    const color = props.tagColor?.()
    if (!color) return undefined
    return {
      'background-color': `${color}18`,
      color,
      'border-color': `${color}38`,
    }
  }

  /** 置顶过的分类卡片，在非对应分类视图下限制删除 */
  const isDeleteDisabled = createMemo(() => {
    return !!(item().pinned && item().tag && props.activeTag?.() !== item().tag)
  })

  return (
    <div class={["card-shell", `cat-${semanticCategory()}`, { selected: props.selected() }]}>
      <div
        class={[
          "paste-card",
          `cat-${semanticCategory()}`,
          { selected: props.selected(), pinned: item().pinned },
        ]}
        onClick={() => props.onSelect(item())}
        onDblClick={() => {
          if (props.onDoubleClick) props.onDoubleClick(item())
          else props.onPaste(item())
        }}
        onContextMenu={(event) => {
          event.preventDefault()
          props.onContextMenu?.(item(), event)
        }}
      >
        <header class="card-band">
          <div class="band-main">
            <span class="band-title">{typeName()}</span>
            <Show when={timeLabel()}>
              <span class="band-time" title={item().createdAtLabel}>
                {timeLabel()}
              </span>
            </Show>
          </div>

          <div class="band-actions">
            <span class="band-glyph" aria-hidden="true">
              {typeGlyph()}
            </span>
            <button
              class={["band-btn", "pin", { on: item().pinned }]}
              title={item().pinned ? '取消置顶 (Ctrl+P)' : '置顶 (Ctrl+P)'}
              aria-pressed={item().pinned ? 'true' : 'false'}
              onClick={(event) => {
                event.stopPropagation()
                props.onTogglePin(item())
              }}
            >
              {item().pinned ? '\uE840' : '\uE718'}
            </button>
            <button
              class={["band-btn", "del", { disabled: isDeleteDisabled() }]}
              title={
                isDeleteDisabled()
                  ? `已在分类「${item().tag}」中置顶，需切换至对应分类后方可删除`
                  : '删除 (Del)'
              }
              aria-disabled={isDeleteDisabled() ? 'true' : 'false'}
              onClick={(event) => {
                event.stopPropagation()
                props.onRemove(item())
              }}
            >
              &#xE74D;
            </button>
          </div>
        </header>

        <div class="card-body">
          <Show when={item().type === 'Image'}>
            <div class="image-stage">
              <Show
                when={imageSrc() && !imageFailed()}
                fallback={<span class="image-fallback">{item().preview || '图像'}</span>}
              >
                <img
                  src={imageSrc() ?? ''}
                  alt=""
                  class="hero-image"
                  loading="lazy"
                  decoding="async"
                  // asset 协议被 scope 拒绝、或 blob 文件已被删掉时，
                  // <img> 只会静默显示为空白。这里兜底成占位，至少能看出"这是一张图"。
                  onError={() => setImageFailed(true)}
                />
              </Show>
            </div>
          </Show>

          <Show when={item().type !== 'Image' && isColorCode()}>
            <div class="color-stage">
              <div class="color-swatch" style={{ 'background-color': item().content.trim() }} />
            </div>
          </Show>

          <Show when={item().type !== 'Image' && !isColorCode() && semanticCategory() === 'files'}>
            <div class="files-stage">
              <For each={fileList().slice(0, 4)}>
                {(file) => (
                  <div class="file-row">
                    <span class="file-icon">&#xE8A5;</span>
                    <span class="file-name" title={file.fullPath}>
                      {file.name}
                    </span>
                  </div>
                )}
              </For>
              <Show when={fileList().length > 4}>
                <span class="files-more">还有 {fileList().length - 4} 个文件...</span>
              </Show>
            </div>
          </Show>

          <Show
            when={
              item().type !== 'Image' && !isColorCode() && semanticCategory() !== 'files' && isUrl()
            }
          >
            <div class="link-stage">
              <span class="link-preview">{item().content.trim()}</span>
            </div>
          </Show>

          <Show
            when={
              item().type !== 'Image' &&
              !isColorCode() &&
              semanticCategory() !== 'files' &&
              !isUrl()
            }
          >
            <p class="text-preview">{item().preview}</p>
          </Show>
        </div>

        <footer class="card-foot">
          <Show when={item().tag}>
            <span class="card-tag-pill" title={`分类标签：${item().tag}`} style={tagStyle()}>
              <span class="tag-icon">&#xE8EC;</span>
              {item().tag}
            </span>
          </Show>
          <Show when={metaBadge()}>
            <span class="foot-meta">{metaBadge()}</span>
          </Show>
        </footer>
      </div>
    </div>
  )
}
