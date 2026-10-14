<script setup lang="ts">
import { computed } from 'vue'
import { send } from '../bridge'

const props = defineProps<{
  isMaximized: boolean
  totalCount: number
  filteredCount: number
}>()

const isFiltering = computed(() => props.filteredCount !== props.totalCount)
const badgeText = computed(() => {
  if (props.totalCount === 0) return '0 项'
  if (isFiltering.value) return `${props.filteredCount} / ${props.totalCount}`
  return `${props.totalCount} 项`
})

const badgeTitle = computed(() => {
  if (isFiltering.value) return `已筛选 ${props.filteredCount} 项（共 ${props.totalCount} 项）`
  return `当前共存储 ${props.totalCount} 条剪贴板历史`
})

function onMouseDown(event: MouseEvent): void {
  // Only trigger drag on left click and when not clicking on control buttons
  if (event.button === 0 && !(event.target as HTMLElement).closest('.window-control-btn')) {
    send({ type: 'dragMove' })
  }
}

function onDblClick(event: MouseEvent): void {
  if (!(event.target as HTMLElement).closest('.window-control-btn')) {
    send({ type: 'toggleMaximize' })
  }
}

function minimize(): void {
  send({ type: 'minimize' })
}

function toggleMaximize(): void {
  send({ type: 'toggleMaximize' })
}

function close(): void {
  send({ type: 'close' })
}
</script>

<template>
  <header class="titlebar" @mousedown="onMouseDown" @dblclick="onDblClick">
    <!-- Left: App brand & count badge -->
    <div class="brand">
      <span class="brand-icon">&#xE8A5;</span>
      <span class="brand-title">剪贴板历史</span>
      <span class="count-badge" :class="{ filtering: isFiltering }" :title="badgeTitle">
        {{ badgeText }}
      </span>
    </div>

    <!-- Center: draggable spacer -->
    <div class="drag-region"></div>

    <!-- Right: Window control buttons (Minimize, Maximize/Restore, Close) -->
    <div class="window-controls">
      <button
        class="window-control-btn"
        title="最小化"
        @click.stop="minimize"
      >
        &#xE921;
      </button>

      <button
        class="window-control-btn"
        :title="isMaximized ? '还原' : '最大化'"
        @click.stop="toggleMaximize"
      >
        {{ isMaximized ? '\uE923' : '\uE922' }}
      </button>

      <button
        class="window-control-btn btn-close"
        title="关闭 (Esc)"
        @click.stop="close"
      >
        &#xE8BB;
      </button>
    </div>
  </header>
</template>

<style scoped>
.titlebar {
  display: flex;
  align-items: center;
  height: 38px;
  background: rgba(255, 255, 255, 0.7);
  border-bottom: 1px solid rgba(0, 0, 0, 0.06);
  user-select: none;
  flex: 0 0 38px;
  backdrop-filter: blur(10px);
}

.brand {
  display: flex;
  align-items: center;
  gap: 8px;
  padding: 0 16px;
  color: var(--text-muted);
  font-size: 12.5px;
  font-weight: 600;
  letter-spacing: 0.2px;
  pointer-events: none;
}

.brand-icon {
  font-family: 'Segoe MDL2 Assets', 'Segoe Fluent Icons', sans-serif;
  font-size: 14px;
  color: #2563eb;
}

.count-badge {
  display: inline-flex;
  align-items: center;
  padding: 1px 7px;
  border-radius: 999px;
  font-size: 11px;
  font-weight: 600;
  background: rgba(0, 0, 0, 0.06);
  color: #475569;
  letter-spacing: -0.2px;
  transition: all 160ms ease;
}

.count-badge.filtering {
  background: rgba(37, 99, 235, 0.12);
  color: #1d4ed8;
}

.drag-region {
  flex: 1 1 auto;
  height: 100%;
}

/* Native-style window controls */
.window-controls {
  display: flex;
  height: 100%;
  flex: 0 0 auto;
}

.window-control-btn {
  width: 44px;
  height: 100%;
  border: 0;
  outline: none;
  background: transparent;
  color: var(--text-muted);
  font-family: 'Segoe MDL2 Assets', 'Segoe Fluent Icons', sans-serif;
  font-size: 10px;
  display: inline-flex;
  align-items: center;
  justify-content: center;
  cursor: pointer;
  transition: background-color 100ms ease, color 100ms ease;
}

.window-control-btn:hover {
  background: rgba(0, 0, 0, 0.08);
  color: var(--text-main);
}

.window-control-btn:active {
  background: rgba(0, 0, 0, 0.14);
}

/* Close button turns red on hover, like native Windows */
.window-control-btn.btn-close:hover {
  background: #e81123;
  color: #ffffff;
}

.window-control-btn.btn-close:active {
  background: #c40e1d;
  color: #ffffff;
}
</style>
