/** Clipboard entry kinds, mirroring the Rust ClipboardItemType enum. */
export type ClipboardItemType = 'Text' | 'Image' | 'Files'

/** One history entry as delivered from the native host. */
export interface ClipboardItem {
  id: number
  type: ClipboardItemType
  /** Searchable plain text. For files entries this is the path list. */
  content: string
  /** Display-ready preview line. */
  preview: string
  /**
   * 这是一个图片条目，值得去拉缩略图。
   *
   * 列表本身**不带**图片数据：拉起缩略图要单独发 `loadThumbnail`，
   * 结果通过 `thumbnail` 推送回来并按 id 缓存在 App 里。
   * 这样每次推送的负载与图片数量无关，不会越用越慢。
   */
  hasImage: boolean
  /** Preformatted timestamp for the card footer. */
  createdAtLabel: string
  /** 同一时间的 RFC3339 原文，用于在卡片头部显示相对时间。 */
  createdAt: string
  pinned: boolean
  /** Optional custom category tag (e.g. "代码", "工作", "常用"). */
  tag: string | null
  /** 原文超过上限、入库时被截断过。 */
  truncated: boolean
}

/** 导入预检结果：确认前先告诉用户会发生什么。 */
export interface ImportPreview {
  total: number
  /** 已存在且不会变化的数量 */
  existing: number
  /** 会被新增的数量 */
  fresh: number
  formatVersion: number
  exportedAt: string
}

export interface SelectionState {
  itemId: number | null
  canPrev: boolean
  canNext: boolean
}

/** Requests the web UI sends to the native host. */
export type HostRequest =
  | { type: 'ready' }
  | { type: 'search'; text: string }
  | { type: 'select'; itemId: number }
  | { type: 'move'; delta: number }
  | { type: 'paste'; itemId: number }
  | { type: 'copy'; itemId: number }
  | { type: 'copyText'; text: string }
  | { type: 'openDataDir' }
  | { type: 'delete'; itemId: number }
  | { type: 'togglePin'; itemId: number }
  | { type: 'setTag'; itemId: number; tag: string | null }
  | { type: 'reorder'; itemId: number; beforeId: number | null }
  | { type: 'filterTag'; tag: string | null }
  | { type: 'deleteTag'; tag: string }
  | { type: 'columns'; columns: number }
  | { type: 'export'; path?: string | null }
  | { type: 'previewImport'; path: string }
  | { type: 'import'; path: string }
  | { type: 'pickImportFile' }
  | { type: 'clearAll' }
  | { type: 'loadThumbnail'; itemId: number }
  | { type: 'dragMove' }
  | { type: 'minimize' }
  | { type: 'maximize' }
  | { type: 'toggleMaximize' }
  | { type: 'close' }
  | { type: 'quit' }

/** Messages the native host pushes to the web UI. */
export type HostPush =
  | {
      type: 'items'
      items: ClipboardItem[]
      isEmpty: boolean
      totalCount?: number
      allTags?: string[]
      /** 每个分类下的条目数，用于删除确认框展示影响范围 */
      tagCounts?: Record<string, number>
      activeTag?: string | null
    }
  | { type: 'selection'; selection: SelectionState }
  | { type: 'hotkey'; label: string }
  | { type: 'windowState'; isMaximized: boolean }
  | { type: 'toast'; message: string }
  | {
      type: 'importPreview'
      path: string
      preview?: ImportPreview
      error?: string
    }
  | { type: 'paths'; blobsRoot: string; dataRoot?: string; version?: string }
  /** 按需拉取的缩略图回来了；`imageUrl` 为 null 表示这张图取不到。 */
  | { type: 'thumbnail'; itemId: number; imageUrl: string | null }
  /** 面板被唤出：宿主已清空搜索词与分类筛选，前端同步归零。 */
  | { type: 'reset' }
  | { type: 'hotkeyUnavailable'; label: string; message: string }
