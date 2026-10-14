<script setup lang="ts">
import { computed } from 'vue'

const props = defineProps<{
  searchText: string
  canPrev: boolean
  canNext: boolean
  hotkeyLabel: string
}>()

const emit = defineEmits<{
  (event: 'update:searchText', value: string): void
  (event: 'prev'): void
  (event: 'next'): void
}>()

const placeholderVisible = computed(() => props.searchText.length === 0)

function onInput(event: Event): void {
  const target = event.target as HTMLInputElement
  emit('update:searchText', target.value)
}
</script>

<template>
  <div class="toolbar">
    <div class="search">
      <span class="search-glyph">&#xE721;</span>
      <input
        class="search-input"
        type="text"
        spellcheck="false"
        autocomplete="off"
        :value="searchText"
        @input="onInput"
      />
      <span v-if="placeholderVisible" class="search-placeholder">
        搜索剪贴板记录...
      </span>
      <button
        v-if="!placeholderVisible"
        class="clear-btn"
        title="清空搜索"
        @click="emit('update:searchText', '')"
      >
        &#xE711;
      </button>
    </div>

    <div class="shortcuts">
      <span class="shortcut-tag"><kbd>Enter</kbd> 粘贴</span>
      <span class="shortcut-tag"><kbd>Ctrl+P</kbd> 置顶</span>
      <span class="shortcut-tag"><kbd>Del</kbd> 删除</span>
      <span class="shortcut-tag"><kbd>Esc</kbd> 关闭</span>
    </div>

    <div class="nav">
      <button class="nav-btn" :disabled="!canPrev" title="上一项 (Left / Up)" @click="emit('prev')">
        &#xE76B;
      </button>
      <button class="nav-btn" :disabled="!canNext" title="下一项 (Right / Down)" @click="emit('next')">
        &#xE76C;
      </button>
    </div>
  </div>
</template>

<style scoped>
.toolbar {
  display: flex;
  align-items: center;
  gap: 14px;
  padding: 12px 18px;
  background: rgba(255, 255, 255, 0.45);
  border-top: 1px solid rgba(0, 0, 0, 0.06);
  backdrop-filter: blur(10px);
}

.search {
  position: relative;
  flex: 1 1 320px;
  max-width: 460px;
  display: flex;
  align-items: center;
  border-radius: 11px;
  border: 1px solid rgba(0, 0, 0, 0.08);
  background: rgba(255, 255, 255, 0.85);
  box-shadow: 0 1px 3px rgba(0, 0, 0, 0.03), inset 0 1px 2px rgba(0, 0, 0, 0.02);
  height: 38px;
  transition: all 180ms ease;
}

.search:focus-within {
  border-color: #3b82f6;
  background: #ffffff;
  box-shadow: 0 0 0 3px rgba(59, 130, 246, 0.16), 0 2px 6px rgba(0, 0, 0, 0.04);
}

.search-glyph {
  font-family: 'Segoe MDL2 Assets', 'Segoe Fluent Icons', sans-serif;
  position: absolute;
  left: 13px;
  font-size: 14px;
  color: var(--text-dim);
  pointer-events: none;
}

.search-input {
  flex: 1 1 auto;
  height: 100%;
  border: 0;
  outline: none;
  background: transparent;
  padding: 0 32px 0 38px;
  font-size: 13.5px;
  font-family: inherit;
  color: var(--text-main);
  user-select: text;
  cursor: text;
}

.search-placeholder {
  position: absolute;
  left: 38px;
  font-size: 13.5px;
  color: var(--text-dim);
  pointer-events: none;
}

.clear-btn {
  font-family: 'Segoe MDL2 Assets', 'Segoe Fluent Icons', sans-serif;
  position: absolute;
  right: 8px;
  border: 0;
  background: transparent;
  width: 22px;
  height: 22px;
  border-radius: 50%;
  display: flex;
  align-items: center;
  justify-content: center;
  font-size: 11px;
  color: var(--text-dim);
  cursor: pointer;
  transition: all 120ms ease;
}

.clear-btn:hover {
  background: rgba(0, 0, 0, 0.08);
  color: var(--text-main);
}

/* Productivity shortcut hints */
.shortcuts {
  display: flex;
  align-items: center;
  gap: 12px;
  margin-left: auto;
}

.shortcut-tag {
  display: inline-flex;
  align-items: center;
  gap: 4px;
  font-size: 11.5px;
  color: var(--text-muted);
}

.shortcut-tag kbd {
  font-family: -apple-system, BlinkMacSystemFont, "Segoe UI", system-ui, sans-serif;
  font-size: 10.5px;
  font-weight: 600;
  line-height: 1;
  padding: 3px 5px;
  border-radius: 5px;
  background: rgba(0, 0, 0, 0.05);
  border: 1px solid rgba(0, 0, 0, 0.08);
  color: var(--text-main);
  box-shadow: 0 1px 0 rgba(0, 0, 0, 0.04);
}

.nav {
  display: flex;
  gap: 6px;
  flex: 0 0 auto;
}

.nav-btn {
  width: 34px;
  height: 34px;
  border: 1px solid rgba(0, 0, 0, 0.08);
  border-radius: 10px;
  background: rgba(255, 255, 255, 0.75);
  box-shadow: 0 1px 2px rgba(0, 0, 0, 0.03);
  color: var(--text-main);
  font-family: 'Segoe MDL2 Assets', 'Segoe Fluent Icons', sans-serif;
  font-size: 13px;
  cursor: pointer;
  display: inline-flex;
  align-items: center;
  justify-content: center;
  transition: all 150ms ease;
}

.nav-btn:hover:not(:disabled) {
  background: #ffffff;
  border-color: rgba(0, 0, 0, 0.14);
  box-shadow: 0 3px 8px rgba(0, 0, 0, 0.06);
  transform: translateY(-1px);
}

.nav-btn:active:not(:disabled) {
  transform: translateY(0);
  background: rgba(240, 240, 243, 0.9);
}

.nav-btn:disabled {
  opacity: 0.35;
  cursor: default;
  border-color: transparent;
  background: transparent;
  box-shadow: none;
}
</style>
