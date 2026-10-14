<script setup lang="ts">
import { computed, nextTick, onMounted, onUnmounted, ref, watch } from 'vue'
import ClipCard from './ClipCard.vue'
import { send } from '../bridge'
import type { ClipboardItem } from '../types'

/** Base parameters for card layout and virtual windowing */
const TARGET_CARD_WIDTH = 236
const GRID_GAP = 16
const PADDING_TOP = 18
const PADDING_BOTTOM = 18
const PADDING_LEFT = 20
const PADDING_RIGHT = 20
const PADDING_HORIZONTAL = PADDING_LEFT + PADDING_RIGHT
const OVERSCAN_ROWS = 2 // Render buffer rows above and below visible area

const props = defineProps<{
  items: ClipboardItem[]
  selectedId: number | null
}>()

const emit = defineEmits<{
  (event: 'select', item: ClipboardItem): void
  (event: 'paste', item: ClipboardItem): void
  (event: 'togglePin', item: ClipboardItem): void
  (event: 'remove', item: ClipboardItem): void
}>()

const containerRef = ref<HTMLElement | null>(null)
const containerWidth = ref(0)
const containerHeight = ref(0)
const scrollTop = ref(0)
const columns = ref(1)

/** Update dimensions from the DOM container */
function measureContainer(): void {
  const el = containerRef.value
  if (!el) return
  containerWidth.value = el.clientWidth
  containerHeight.value = el.clientHeight
  scrollTop.value = el.scrollTop
}

/** Calculate columns based on current viewport width */
function recomputeColumns(): void {
  const usable = containerWidth.value - PADDING_HORIZONTAL
  if (usable <= 0) return

  const next = Math.max(1, Math.floor((usable + GRID_GAP) / (TARGET_CARD_WIDTH + GRID_GAP)))
  if (next !== columns.value) {
    columns.value = next
    send({ type: 'columns', columns: next })
  }
}

/** Single card width and height (square aspect ratio) */
const cardWidth = computed(() => {
  const usable = containerWidth.value - PADDING_HORIZONTAL
  if (usable <= 0 || columns.value <= 0) return TARGET_CARD_WIDTH
  return Math.max(100, (usable - (columns.value - 1) * GRID_GAP) / columns.value)
})

const cardHeight = computed(() => cardWidth.value)
const rowStep = computed(() => cardHeight.value + GRID_GAP)

/** Total rows in the dataset */
const totalRows = computed(() => {
  if (props.items.length === 0 || columns.value <= 0) return 0
  return Math.ceil(props.items.length / columns.value)
})

/** Total virtual height of the entire list to back the native scrollbar */
const totalContentHeight = computed(() => {
  if (totalRows.value === 0) return 0
  return PADDING_TOP + totalRows.value * cardHeight.value + Math.max(0, totalRows.value - 1) * GRID_GAP + PADDING_BOTTOM
})

/** Visible row range with overscan buffer */
const visibleRange = computed(() => {
  if (totalRows.value === 0 || rowStep.value <= 0) {
    return { startRow: 0, endRow: 0, startIndex: 0, endIndex: 0, offsetY: 0 }
  }

  // Exact row at top of viewport
  const topRow = Math.max(0, Math.floor((scrollTop.value - PADDING_TOP) / rowStep.value))
  // Exact row at bottom of viewport
  const bottomRow = Math.min(
    totalRows.value - 1,
    Math.floor((scrollTop.value + containerHeight.value - PADDING_TOP) / rowStep.value),
  )

  const bufferedStartRow = Math.max(0, topRow - OVERSCAN_ROWS)
  const bufferedEndRow = Math.min(totalRows.value - 1, bottomRow + OVERSCAN_ROWS)

  const startIndex = bufferedStartRow * columns.value
  const endIndex = Math.min(props.items.length, (bufferedEndRow + 1) * columns.value)
  const offsetY = PADDING_TOP + bufferedStartRow * rowStep.value

  return {
    startRow: bufferedStartRow,
    endRow: bufferedEndRow,
    startIndex,
    endIndex,
    offsetY,
  }
})

/** Slice of items currently inside the visible window + overscan buffer */
const visibleItems = computed(() => {
  const { startIndex, endIndex } = visibleRange.value
  return props.items.slice(startIndex, endIndex)
})

/** Grid track and positioning styles for the visible virtual window */
const gridStyle = computed(() => ({
  transform: `translate3d(0, ${visibleRange.value.offsetY}px, 0)`,
  gridTemplateColumns: `repeat(${columns.value}, minmax(0, 1fr))`,
  gap: `${GRID_GAP}px`,
  paddingLeft: `${PADDING_LEFT}px`,
  paddingRight: `${PADDING_RIGHT}px`,
}))

/** Handle scroll updates throttled via requestAnimationFrame */
let rafId: number | null = null

function onScroll(event: Event): void {
  const target = event.target as HTMLElement
  if (rafId !== null) return
  rafId = requestAnimationFrame(() => {
    scrollTop.value = target.scrollTop
    rafId = null
  })
}

/** Precision viewport scroll tracking when selection changes */
function ensureSelectedVisible(id: number | null): void {
  if (id === null || !containerRef.value) return
  const index = props.items.findIndex(item => item.id === id)
  if (index === -1) return

  const row = Math.floor(index / columns.value)
  const itemTop = PADDING_TOP + row * rowStep.value
  const itemBottom = itemTop + cardHeight.value

  const el = containerRef.value
  const currentScrollTop = el.scrollTop
  const viewH = el.clientHeight

  if (itemTop < currentScrollTop + PADDING_TOP) {
    el.scrollTop = Math.max(0, itemTop - PADDING_TOP)
  } else if (itemBottom > currentScrollTop + viewH - PADDING_BOTTOM) {
    el.scrollTop = itemBottom - viewH + PADDING_BOTTOM
  }
}

watch(
  () => props.selectedId,
  (newId) => {
    nextTick(() => ensureSelectedVisible(newId))
  },
)

watch(
  () => props.items.length,
  () => {
    measureContainer()
    recomputeColumns()
    // When items list completely changes (e.g. search filter), reset scroll
    if (containerRef.value && scrollTop.value > totalContentHeight.value) {
      containerRef.value.scrollTop = 0
    }
  },
)

let observer: ResizeObserver | null = null

onMounted(() => {
  measureContainer()
  recomputeColumns()

  observer = new ResizeObserver(() => {
    measureContainer()
    recomputeColumns()
  })

  if (containerRef.value) {
    observer.observe(containerRef.value)
  }
})

onUnmounted(() => {
  observer?.disconnect()
  observer = null
  if (rafId !== null) {
    cancelAnimationFrame(rafId)
    rafId = null
  }
})
</script>

<template>
  <div ref="containerRef" class="viewport-container" @scroll.passive="onScroll">
    <!-- Virtual height spacer matching the complete dataset -->
    <div class="virtual-spacer" :style="{ height: `${totalContentHeight}px` }">
      <!-- Visible window grid translated to active scroll offset -->
      <div class="visible-grid" :style="gridStyle">
        <ClipCard
          v-for="item in visibleItems"
          :key="item.id"
          :item="item"
          :selected="item.id === selectedId"
          @select="emit('select', $event)"
          @paste="emit('paste', $event)"
          @toggle-pin="emit('togglePin', $event)"
          @remove="emit('remove', $event)"
        />
      </div>
    </div>
  </div>
</template>

<style scoped>
.viewport-container {
  height: 100%;
  overflow-y: auto;
  overflow-x: hidden;
  position: relative;
  scrollbar-gutter: stable;
  /* Hardware acceleration for smooth scrolling */
  will-change: scroll-position;
  contain: strict;
}

.virtual-spacer {
  position: relative;
  width: 100%;
  pointer-events: none;
}

.visible-grid {
  position: absolute;
  top: 0;
  left: 0;
  right: 0;
  display: grid;
  align-content: start;
  pointer-events: auto;
}
</style>
