<script setup lang="ts">
import { computed } from 'vue'
import type { ClipboardItem } from '../types'

const props = defineProps<{
  item: ClipboardItem
  selected: boolean
}>()

const emit = defineEmits<{
  (event: 'select', item: ClipboardItem): void
  (event: 'paste', item: ClipboardItem): void
  (event: 'togglePin', item: ClipboardItem): void
  (event: 'remove', item: ClipboardItem): void
}>()

// URL detection
const isUrl = computed(() => {
  if (props.item.type !== 'Text') return false
  const trimmed = props.item.content.trim()
  return /^https?:\/\/[^\s]+$/i.test(trimmed)
})

const urlDomain = computed(() => {
  if (!isUrl.value) return ''
  try {
    const url = new URL(props.item.content.trim())
    return url.hostname.replace(/^www\./, '')
  } catch {
    return ''
  }
})

// Hex / RGB color code detection
const isColorCode = computed(() => {
  if (props.item.type !== 'Text') return false
  const trimmed = props.item.content.trim()
  return /^#([0-9a-f]{3}|[0-9a-f]{6}|[0-9a-f]{8})$/i.test(trimmed) || /^rgba?\((\s*\d+\s*,?){3,4}\)$/i.test(trimmed)
})

// File paths split
const fileList = computed(() => {
  if (props.item.type !== 'Files') return []
  return props.item.content
    .split(/\r?\n/)
    .map(p => p.trim())
    .filter(Boolean)
    .map(fullPath => {
      const name = fullPath.split(/[\\/]/).pop() || fullPath
      return { name, fullPath }
    })
})

// Paste-style semantic category
const semanticCategory = computed(() => {
  if (props.item.type === 'Image') return 'image'
  if (props.item.type === 'Files') return 'files'
  if (isUrl.value) return 'link'
  if (isColorCode.value) return 'color'
  return 'text'
})

// Glyphs matching Paste.app / Windows icon set
const typeGlyph = computed(() => {
  switch (semanticCategory.value) {
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

const typeName = computed(() => {
  switch (semanticCategory.value) {
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

// Paste.app signature header right badge (char count / domain / file count / image dimensions)
const metaBadge = computed(() => {
  switch (semanticCategory.value) {
    case 'link':
      return urlDomain.value || '网页'
    case 'files':
      return `${fileList.value.length} 个文件`
    case 'color':
      return props.item.content.trim()
    case 'image': {
      // Content contains like "1920 x 1080 image"
      const match = props.item.content.match(/(\d+)\s*x\s*(\d+)/i)
      return match ? `${match[1]}×${match[2]}` : '图片'
    }
    default: {
      const len = props.item.content.length
      return len > 0 ? `${len} 字符` : ''
    }
  }
})

function onDoubleClick(): void {
  emit('paste', props.item)
}
</script>

<template>
  <div
    class="paste-card card"
    :class="[`cat-${semanticCategory}`, { selected, pinned: item.pinned }]"
    @click="emit('select', item)"
    @dblclick="onDoubleClick"
  >
    <div class="card-inner">
      <!-- Paste.app style Header -->
      <header class="card-head">
        <div class="pill-badge">
          <span class="pill-glyph">{{ typeGlyph }}</span>
          <span class="pill-title">{{ typeName }}</span>
        </div>

        <div class="head-meta">
          <span v-if="metaBadge" class="meta-label" :title="metaBadge">{{ metaBadge }}</span>

          <button
            class="pin-btn"
            :class="{ on: item.pinned }"
            :title="item.pinned ? '取消置顶 (Ctrl+P)' : '置顶 (Ctrl+P)'"
            :aria-pressed="item.pinned"
            @click.stop="emit('togglePin', item)"
          >
            {{ item.pinned ? '\uE840' : '\uE718' }}
          </button>
        </div>
      </header>

      <!-- Paste.app style Body -->
      <div class="card-body">
        <!-- Image Card -->
        <div v-if="item.imageUrl" class="image-stage">
          <img :src="item.imageUrl" alt="" class="hero-image" />
        </div>

        <!-- Color Code Card -->
        <div v-else-if="isColorCode" class="color-stage">
          <div class="color-swatch" :style="{ backgroundColor: item.content.trim() }"></div>
          <span class="color-hex">{{ item.content.trim() }}</span>
        </div>

        <!-- File List Card -->
        <div v-else-if="semanticCategory === 'files'" class="files-stage">
          <div v-for="(file, idx) in fileList.slice(0, 4)" :key="idx" class="file-row">
            <span class="file-icon">&#xE8A5;</span>
            <span class="file-name" :title="file.fullPath">{{ file.name }}</span>
          </div>
          <span v-if="fileList.length > 4" class="files-more">还有 {{ fileList.length - 4 }} 个文件...</span>
        </div>

        <!-- Link Card -->
        <div v-else-if="isUrl" class="link-stage">
          <span class="link-preview">{{ item.content.trim() }}</span>
        </div>

        <!-- Plain Text Card -->
        <p v-else class="text-preview">{{ item.preview }}</p>
      </div>

      <!-- Paste.app style Footer -->
      <footer class="card-foot">
        <span class="timestamp">{{ item.createdAtLabel }}</span>
        <button
          class="delete-btn"
          title="删除 (Del)"
          @click.stop="emit('remove', item)"
        >
          &#xE74D;
        </button>
      </footer>
    </div>
  </div>
</template>

<style scoped>
.paste-card {
  aspect-ratio: 1 / 1;
  border-radius: var(--radius-card);
  border: 1px solid rgba(0, 0, 0, 0.07);
  box-shadow: var(--card-shadow);
  cursor: pointer;
  position: relative;
  overflow: hidden;
  transition:
    transform 160ms cubic-bezier(0.16, 1, 0.3, 1),
    box-shadow 160ms cubic-bezier(0.16, 1, 0.3, 1),
    border-color 160ms ease,
    background-color 160ms ease;
}

/* Paste.app Signature Category Background Colors */
.paste-card.cat-text {
  background-color: var(--paste-bg-text);
  border-color: var(--paste-border-text);
}
.paste-card.cat-link {
  background-color: var(--paste-bg-link);
  border-color: var(--paste-border-link);
}
.paste-card.cat-image {
  background-color: var(--paste-bg-image);
  border-color: var(--paste-border-image);
}
.paste-card.cat-color {
  background-color: var(--paste-bg-color);
  border-color: var(--paste-border-color);
}
.paste-card.cat-files {
  background-color: var(--paste-bg-files);
  border-color: var(--paste-border-files);
}

/* Hover State: Gentle lift */
.paste-card:hover {
  transform: translateY(-3px);
  box-shadow: var(--card-shadow-hover);
}

.paste-card:active {
  transform: scale(0.985);
}

/* Paste.app Signature Active / Selected Ring */
.paste-card.selected {
  transform: translateY(-3px);
  background-color: #ffffff !important;
  border-color: transparent !important;
  box-shadow: var(--paste-selected-ring) !important;
  z-index: 10;
}

.card-inner {
  height: 100%;
  display: flex;
  flex-direction: column;
  padding: 13px 15px 11px;
}

/* Header */
.card-head {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 8px;
  flex: 0 0 auto;
}

/* Pill badge on top left */
.pill-badge {
  display: inline-flex;
  align-items: center;
  gap: 5px;
  padding: 2.5px 8px 2.5px 6px;
  border-radius: 999px;
  font-size: 11px;
  font-weight: 600;
  line-height: 1;
}

.cat-text .pill-badge {
  background: rgba(37, 99, 235, 0.1);
  color: var(--paste-accent-text);
}
.cat-link .pill-badge {
  background: rgba(5, 150, 105, 0.12);
  color: var(--paste-accent-link);
}
.cat-image .pill-badge {
  background: rgba(234, 88, 12, 0.1);
  color: var(--paste-accent-image);
}
.cat-color .pill-badge {
  background: rgba(147, 51, 234, 0.1);
  color: var(--paste-accent-color);
}
.cat-files .pill-badge {
  background: rgba(217, 119, 6, 0.12);
  color: var(--paste-accent-files);
}

.pill-glyph {
  font-family: 'Segoe MDL2 Assets', 'Segoe Fluent Icons', sans-serif;
  font-size: 13px;
}

.pill-title {
  letter-spacing: -0.1px;
}

/* Header right: metadata badge & pin button */
.head-meta {
  display: flex;
  align-items: center;
  gap: 6px;
  max-width: 60%;
}

.meta-label {
  font-size: 11px;
  font-weight: 500;
  color: var(--text-muted);
  white-space: nowrap;
  overflow: hidden;
  text-overflow: ellipsis;
  letter-spacing: -0.1px;
}

.pin-btn {
  font-family: 'Segoe MDL2 Assets', 'Segoe Fluent Icons', sans-serif;
  border: 0;
  background: transparent;
  width: 22px;
  height: 22px;
  border-radius: 6px;
  display: inline-flex;
  align-items: center;
  justify-content: center;
  font-size: 12px;
  color: var(--text-dim);
  cursor: pointer;
  opacity: 0;
  transition: all 140ms ease;
}

.paste-card:hover .pin-btn,
.pin-btn.on {
  opacity: 1;
}

.pin-btn.on {
  color: var(--pin-amber);
  background: var(--pin-amber-bg);
  border-radius: 999px;
}

.pin-btn:hover {
  background: rgba(0, 0, 0, 0.07);
  color: var(--text-main);
}

/* Body: Content Presentation */
.card-body {
  flex: 1 1 auto;
  min-height: 0;
  margin: 10px 0 8px;
  display: flex;
}

/* Text Presentation */
.text-preview {
  margin: 0;
  font-size: 13.5px;
  line-height: 1.55;
  color: var(--text-main);
  overflow: hidden;
  display: -webkit-box;
  -webkit-line-clamp: 6;
  -webkit-box-orient: vertical;
  word-break: break-word;
  white-space: pre-wrap;
  font-feature-settings: "cv02", "cv03", "cv04", "cv11";
}

/* Link Presentation */
.link-stage {
  width: 100%;
  display: flex;
  align-items: flex-start;
}

.link-preview {
  font-size: 13.5px;
  line-height: 1.5;
  color: #047857;
  font-weight: 500;
  word-break: break-all;
  overflow: hidden;
  display: -webkit-box;
  -webkit-line-clamp: 5;
  -webkit-box-orient: vertical;
}

/* Color Presentation */
.color-stage {
  width: 100%;
  display: flex;
  flex-direction: column;
  align-items: center;
  justify-content: center;
  gap: 8px;
}

.color-swatch {
  width: 100%;
  height: 70px;
  border-radius: 8px;
  border: 1px solid rgba(0, 0, 0, 0.1);
  box-shadow: 0 2px 6px rgba(0, 0, 0, 0.06);
}

.color-hex {
  font-family: Consolas, Monaco, monospace;
  font-size: 13px;
  font-weight: 600;
  color: var(--text-main);
}

/* Files Presentation */
.files-stage {
  width: 100%;
  display: flex;
  flex-direction: column;
  gap: 5px;
  overflow: hidden;
}

.file-row {
  display: flex;
  align-items: center;
  gap: 6px;
  font-size: 12.5px;
  color: var(--text-main);
  overflow: hidden;
}

.file-icon {
  font-family: 'Segoe MDL2 Assets', 'Segoe Fluent Icons', sans-serif;
  font-size: 12px;
  color: #d97706;
  flex: 0 0 auto;
}

.file-name {
  white-space: nowrap;
  overflow: hidden;
  text-overflow: ellipsis;
  font-weight: 500;
}

.files-more {
  font-size: 11px;
  color: var(--text-muted);
  margin-top: 2px;
}

/* Image Presentation */
.image-stage {
  width: 100%;
  height: 100%;
  border-radius: 8px;
  background: rgba(0, 0, 0, 0.02);
  border: 1px solid rgba(0, 0, 0, 0.05);
  display: flex;
  align-items: center;
  justify-content: center;
  overflow: hidden;
}

.hero-image {
  max-width: 100%;
  max-height: 100%;
  object-fit: contain;
}

/* Footer */
.card-foot {
  flex: 0 0 auto;
  display: flex;
  align-items: center;
  justify-content: space-between;
}

.timestamp {
  font-size: 10.5px;
  font-weight: 500;
  color: var(--text-muted);
  letter-spacing: -0.1px;
}

.delete-btn {
  font-family: 'Segoe MDL2 Assets', 'Segoe Fluent Icons', sans-serif;
  border: 0;
  background: transparent;
  width: 22px;
  height: 22px;
  border-radius: 6px;
  display: inline-flex;
  align-items: center;
  justify-content: center;
  font-size: 11px;
  color: var(--text-dim);
  cursor: pointer;
  opacity: 0;
  transition: all 120ms ease;
}

.paste-card:hover .delete-btn {
  opacity: 1;
}

.delete-btn:hover {
  background: rgba(239, 68, 68, 0.12);
  color: #dc2626;
}
</style>
