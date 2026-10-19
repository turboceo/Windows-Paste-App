import { useMemo } from 'react'
import type { ClipboardItem } from '../types'

interface ClipCardProps {
  item: ClipboardItem
  selected: boolean
  onSelect: (item: ClipboardItem) => void
  onPaste: (item: ClipboardItem) => void
  onTogglePin: (item: ClipboardItem) => void
  onRemove: (item: ClipboardItem) => void
  onContextMenu?: (item: ClipboardItem, event: React.MouseEvent) => void
}

/** One history card, styled after Paste.app. */
export default function ClipCard({
  item,
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
        return match ? `${match[1]}×${match[2]}` : '图片'
      }
      default: {
        const len = item.content.length
        return len > 0 ? `${len} 字符` : ''
      }
    }
  }, [semanticCategory, urlDomain, fileList.length, item.content])

  return (
    <div
    className={`paste-card card cat-${semanticCategory}${selected ? ' selected' : ''}${
      item.pinned ? ' pinned' : ''
    }`}
    onClick={() => onSelect(item)}
    onDoubleClick={() => onPaste(item)}
    onContextMenu={(e) => {
      e.preventDefault()
      onContextMenu?.(item, e)
    }}
  >
      <div className="card-inner">
        <header className="card-head">
          <div className="pill-badge">
            <span className="pill-glyph">{typeGlyph}</span>
            <span className="pill-title">{typeName}</span>
          </div>

          <div className="head-meta">
            {metaBadge && (
              <span className="meta-label" title={metaBadge}>
                {metaBadge}
              </span>
            )}

            <button
              className={`pin-btn${item.pinned ? ' on' : ''}`}
              title={item.pinned ? '取消置顶 (Ctrl+P)' : '置顶 (Ctrl+P)'}
              aria-pressed={item.pinned}
              onClick={(event) => {
                event.stopPropagation()
                onTogglePin(item)
              }}
            >
              {item.pinned ? '\uE840' : '\uE718'}
            </button>
          </div>
        </header>

        <div className="card-body">
          {item.imageUrl ? (
            <div className="image-stage">
              <img src={item.imageUrl} alt="" className="hero-image" />
            </div>
          ) : isColorCode ? (
            <div className="color-stage">
              <div className="color-swatch" style={{ backgroundColor: item.content.trim() }} />
              <span className="color-hex">{item.content.trim()}</span>
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
          <div className="foot-left">
            <span className="timestamp">{item.createdAtLabel}</span>
            {item.tag && (
              <span className="card-tag-pill" title={`分类标签：${item.tag}`}>
                <span className="tag-icon">&#xE8EC;</span>
                {item.tag}
              </span>
            )}
          </div>
          <button
            className="delete-btn"
            title="删除 (Del)"
            onClick={(event) => {
              event.stopPropagation()
              onRemove(item)
            }}
          >
            &#xE74D;
          </button>
        </footer>
      </div>
    </div>
  )
}
