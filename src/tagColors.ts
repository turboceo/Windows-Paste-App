/**
 * 标签颜色预设池与辅助工具。
 *
 * 存储策略：保存在 localStorage 中，键名为 `wp_tag_colors`。
 * 数据库仅存储纯文本标签名称，保证向后兼容以及导出导入时不会受格式破坏。
 */

export const TAG_COLOR_PALETTE = [
  '#ea8a1f', // 暖橙 (默认)
  '#e5484d', // 珊瑚红
  '#f76b15', // 亮橘
  '#e2a336', // 琥珀黄
  '#46a758', // 翠绿
  '#12a594', // 青碧
  '#0090ff', // 纯蓝
  '#3e63dd', // 靛蓝
  '#8e4ec6', // 葡萄紫
  '#d6409f', // 莓果粉
  '#706e6b', // 中性灰
]

const STORAGE_KEY = 'wp_tag_colors'

/** 读取所有已保存的标签颜色映射。 */
export function loadTagColors(): Record<string, string> {
  try {
    const raw = localStorage.getItem(STORAGE_KEY)
    if (!raw) return {}
    const parsed = JSON.parse(raw)
    return typeof parsed === 'object' && parsed !== null ? parsed : {}
  } catch {
    return {}
  }
}

/** 保存标签颜色映射。 */
export function saveTagColors(colors: Record<string, string>): void {
  try {
    localStorage.setItem(STORAGE_KEY, JSON.stringify(colors))
  } catch (e) {
    console.warn('[tagColors] failed to save to localStorage:', e)
  }
}

/**
 * 根据标签名称获取配置的颜色。
 * 若未专门配置，则按标签名字哈希从调色盘中稳定分配一种颜色。
 */
export function getTagColor(tag: string | null | undefined, customColors?: Record<string, string>): string {
  if (!tag) return TAG_COLOR_PALETTE[0]
  if (customColors && customColors[tag]) {
    return customColors[tag]
  }

  // 稳定散列取模
  let hash = 0
  for (let i = 0; i < tag.length; i++) {
    hash = (hash << 5) - hash + tag.charCodeAt(i)
    hash |= 0
  }
  const index = Math.abs(hash) % TAG_COLOR_PALETTE.length
  return TAG_COLOR_PALETTE[index]
}
