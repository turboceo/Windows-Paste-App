<script setup lang="ts">
import { onMounted, ref } from 'vue'
import TitleBar from './components/TitleBar.vue'
import CardGrid from './components/CardGrid.vue'
import ToolBar from './components/ToolBar.vue'
import { onPush, send } from './bridge'
import type { ClipboardItem, HostPush } from './types'

const items = ref<ClipboardItem[]>([])
const isEmpty = ref(true)
const selectedId = ref<number | null>(null)
const canPrev = ref(false)
const canNext = ref(false)
const searchText = ref('')
const hotkeyLabel = ref('')
const isMaximized = ref(false)
const totalCount = ref(0)

function applyItems(push: Extract<HostPush, { type: 'items' }>): void {
  items.value = push.items
  isEmpty.value = push.isEmpty
  if (typeof push.totalCount === 'number') {
    totalCount.value = push.totalCount
  } else if (!searchText.value) {
    totalCount.value = push.items.length
  }
}

function applySelection(push: Extract<HostPush, { type: 'selection' }>): void {
  selectedId.value = push.itemId
  canPrev.value = push.canPrev
  canNext.value = push.canNext
}

onMounted(() => {
  onPush((message) => {
    switch (message.type) {
      case 'items':
        applyItems(message)
        break
      case 'selection':
        applySelection(message)
        break
      case 'hotkey':
        hotkeyLabel.value = message.label
        break
      case 'windowState':
        isMaximized.value = message.isMaximized
        break
    }
  })

  send({ type: 'ready' })
})

function onSearch(value: string): void {
  searchText.value = value
  send({ type: 'search', text: value })
}

function onSelect(item: ClipboardItem): void {
  selectedId.value = item.id
  send({ type: 'select', itemId: item.id })
}

function onPaste(item: ClipboardItem): void {
  send({ type: 'paste', itemId: item.id })
}

function onTogglePin(item: ClipboardItem): void {
  send({ type: 'togglePin', itemId: item.id })
}

function onRemove(item: ClipboardItem): void {
  send({ type: 'delete', itemId: item.id })
}

function onPrev(): void {
  // The host owns selection order, so it resolves the move and pushes back.
  send({ type: 'move', delta: -1 })
}

function onNext(): void {
  send({ type: 'move', delta: 1 })
}
</script>

<template>
  <div class="shell">
    <div class="panel">
      <TitleBar
        :is-maximized="isMaximized"
        :total-count="totalCount"
        :filtered-count="items.length"
      />

      <div class="list-area">
        <CardGrid
          :items="items"
          :selected-id="selectedId"
          @select="onSelect"
          @paste="onPaste"
          @toggle-pin="onTogglePin"
          @remove="onRemove"
        />
        <div v-if="isEmpty" class="empty">
          <span class="empty-glyph">&#xE8C5;</span>
          <p class="empty-title">剪贴板历史为空</p>
          <span class="empty-desc">复制文本、图片或文件后将自动显示在这里</span>
        </div>
      </div>

      <ToolBar
        :search-text="searchText"
        :can-prev="canPrev"
        :can-next="canNext"
        :hotkey-label="hotkeyLabel"
        @update:search-text="onSearch"
        @prev="onPrev"
        @next="onNext"
      />
    </div>
  </div>
</template>

<style scoped>
.shell {
  height: 100%;
  display: flex;
  padding: 0;
}

.panel {
  flex: 1 1 auto;
  display: flex;
  flex-direction: column;
  min-height: 0;
  background: var(--panel-bg);
  border: 1px solid var(--panel-border);
  box-shadow: var(--panel-shadow);
  border-radius: var(--radius-panel);
  overflow: hidden;
  backdrop-filter: blur(28px) saturate(180%);
  -webkit-backdrop-filter: blur(28px) saturate(180%);
}

.list-area {
  position: relative;
  flex: 1 1 auto;
  min-height: 0;
}

.empty {
  position: absolute;
  inset: 0;
  display: flex;
  flex-direction: column;
  align-items: center;
  justify-content: center;
  gap: 8px;
  pointer-events: none;
}

.empty-glyph {
  font-family: 'Segoe MDL2 Assets', 'Segoe Fluent Icons', sans-serif;
  font-size: 38px;
  color: var(--text-dim);
  opacity: 0.6;
}

.empty-title {
  margin: 0;
  font-size: 15px;
  font-weight: 600;
  color: var(--text-muted);
}

.empty-desc {
  font-size: 12.5px;
  color: var(--text-dim);
}
</style>
