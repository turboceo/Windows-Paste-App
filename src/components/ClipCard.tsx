import { useEffect, useMemo, useState } from 'react'
import type { ClipboardItem } from '../types'
import { relativeTime } from '../time'

interface ClipCardProps {
  item: ClipboardItem
  /**
   * 缩略图 data URL。`undefined` = 还没拉过，`null` = 拉过但取不到。
   * 两者在视觉上都是占位，但前者会触发一次拉取。
   */
  thumbnail: string | null | undefined
  onRequestThumbnail: (itemId: number) => void
  selected: boolean
  onSelect: (item: ClipboardItem) => void
  onPaste: (item: ClipboardItem) => void
  onTogglePin: (item: ClipboardItem) => void
  onRemove: (item: ClipboardItem) => void
  onContextMenu?: (item: ClipboardItem, event: React.MouseEvent) => void
}

/**
 * One history card styled after Paste.app:
 * 顶部是同色系的彩色色带（类型名 + 相对时间），下面是纯白正文，
 * 底部一行居中的统计信息。
 */
export default function ClipCard({
  item,
  thumbnail,
  onRequestThumbnail,
  selected,
  onSelect,
  onPaste,
  onTogglePin,
  onRemove,
  onContextMenu,
}: ClipCardProps) {
  const isUrl = useMemo(() => {
    if (item.type !== 'Text') return false
    return /^https?:\/\/[^\s]+$/i.test(item.content.trim())
  }, [item.type, item.content])

  const urlDomain = useMemo(() => {
    if (!isUrl) return ''
    try {
      return new URL(item.content.trim()).hostname.replace(/^www\./, '')
    } catch {
      return ''
    }
  }, [isUrl, item.content])

  const isColorCode = useMemo(() => {
    if (item.type !== 'Text') return false
    const trimmed = item.content.trim()
    return (
      /^#([0-9a-f]{3}|[0-9a-f]{6}|[0-9a-f]{8})$/i.test(trimmed) ||
      /^rgba?\((\s*\d+\s*,?){3,4}\)$/i.test(trimmed)
    )
  }, [item.type, item.content])

  const fileList = useMemo(() => {
    if (item.type !== 'Files') return []
    return item.content
      .split(/\r?\n/)
      .map((p) => p.trim())
      .filter(Boolean)
      .map((fullPath) => ({ name: fullPath.split(/[\\/]/).pop() || fullPath, fullPath }))
  }, [item.type, item.content])

  // 缩略图按需拉取：卡片被挂载（即滚动到可见区域）时才请求
  useEffect(() => {
    if (item.hasImage && thumbnail === undefined) {
      onRequestThumbnail(item.id)
    }
  }, [item.hasImage, item.id, thumbnail, onRequestThumbnail])

  const imageSrc = item.type === 'Image' ? thumbnail ?? null : null

  // 加载失败后不要一直重试：记住这张卡已经失败过，直接显示占位
  const [imageFailed, setImageFailed] = useState(false)

  // 缩略图换了（重新推送过）就重置失败状态
  useEffect(() => {
    setImageFailed(false)
  }, [imageSrc])

  const semanticCategory = useMemo(() => {
    if (item.type === 'Image') return 'image'
    if (item.type === 'Files') return 'files'
    if (isUrl) return 'link'
    if (isColorCode) return 'color'
    return 'text'
  }, [item.type, isUrl, isColorCode])

  const typeGlyph = useMemo(() => {
    switch (semanticCategory) {
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
  }, [semanticCategory])

  const typeName = useMemo(() => {
    switch (semanticCategory) {
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
  }, [semanticCategory])

  const metaBadge = useMemo(() => {
    switch (semanticCategory) {
      case 'link':
        return urlDomain || '网页'
      case 'files':
        return `${fileList.length} 个文件`
      case 'color':
        return item.content.trim()
      case 'image': {
        const match = item.content.match(/(\d+)\s*x\s*(\d+)/i)
        return match ? `${match[1]} × ${match[2]}` : '图片'
      }
      default: {
        const len = item.content.length
        if (len === 0) return ''
        // 被截断过的内容加个后缀，避免用户以为复制的东西只有这么点
        return item.truncated ? `${len}+ 字符` : `${len} 字符`
      }
    }
  }, [semanticCategory, urlDomain, fileList.length, item.content])

  const timeLabel = useMemo(() => relativeTime(item.createdAt), [item.createdAt])

  return (
    <div className={`card-shell cat-${semanticCategory}`}>
      <div
        className={`paste-card cat-${semanticCategory}${selected ? ' selected' : ''}${
          item.pinned ? ' pinned' : ''
        }`}
        onClick={() => onSelect(item)}
        onDoubleClick={() => onPaste(item)}
        onContextMenu={(e) => {
          e.preventDefault()
          onContextMenu?.(item, e)
        }}
      >
        <header className="card-band">
          <div className="band-main">
            <span className="band-title">{typeName}</span>
            {timeLabel && (
              <span className="band-time" title={item.createdAtLabel}>
                {timeLabel}
              </span>
            )}
          </div>

          <div className="band-actions">
            <span className="band-glyph" aria-hidden="true">
              {typeGlyph}
            </span>
            <button
              className={`band-btn pin${item.pinned ? ' on' : ''}`}
              title={item.pinned ? '取消置顶 (Ctrl+P)' : '置顶 (Ctrl+P)'}
              aria-pressed={item.pinned}
              onClick={(event) => {
                event.stopPropagation()
                onTogglePin(item)
              }}
            >
              {item.pinned ? '\uE840' : '\uE718'}
            </button>
            <button
              className="band-btn del"
              title="删除 (Del)"
              onClick={(event) => {
                event.stopPropagation()
                onRemove(item)
              }}
            >
              &#xE74D;
            </button>
          </div>
        </header>

        <div className="card-body">
          {item.type === 'Image' ? (
            <div className="image-stage">
              {imageSrc && !imageFailed ? (
                <img
                  src={imageSrc}
                  alt=""
                  className="hero-image"
                  loading="lazy"
                  decoding="async"
                  // asset 协议被 scope 拒绝、或 blob 文件已被删掉时，
                  // <img> 只会静默显示为空白。这里兜底成占位，至少能看出"这是一张图"。
                  onError={() => setImageFailed(true)}
                />
              ) : (
                <span className="image-fallback">{item.preview || '图像'}</span>
              )}
            </div>
          ) : isColorCode ? (
            <div className="color-stage">
              <div className="color-swatch" style={{ backgroundColor: item.content.trim() }} />
            </div>
          ) : semanticCategory === 'files' ? (
            <div className="files-stage">
              {fileList.slice(0, 4).map((file, idx) => (
                <div key={idx} className="file-row">
                  <span className="file-icon">&#xE8A5;</span>
                  <span className="file-name" title={file.fullPath}>
                    {file.name}
                  </span>
                </div>
              ))}
              {fileList.length > 4 && (
                <span className="files-more">还有 {fileList.length - 4} 个文件...</span>
              )}
            </div>
          ) : isUrl ? (
            <div className="link-stage">
              <span className="link-preview">{item.content.trim()}</span>
            </div>
          ) : (
            <p className="text-preview">{item.preview}</p>
          )}
        </div>

        <footer className="card-foot">
          {item.tag && (
            <span className="card-tag-pill" title={`分类标签：${item.tag}`}>
              <span className="tag-icon">&#xE8EC;</span>
              {item.tag}
            </span>
          )}
          {metaBadge && <span className="foot-meta">{metaBadge}</span>}
        </footer>
      </div>
    </div>
  )
}
