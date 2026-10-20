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
   * 图片条目的**缩略图 data URL**（`data:image/png;base64,...`）。
   * 由后端解码原图、缩放到 480px 后内联，可以直接交给 `<img src>`。
   */
  imageUrl: string | null
  /** Preformatted timestamp for the card footer. */
  createdAtLabel: string
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
  | { type: 'delete'; itemId: number }
  | { type: 'togglePin'; itemId: number }
  | { type: 'setTag'; itemId: number; tag: string | null }
  | { type: 'filterTag'; tag: string | null }
  | { type: 'columns'; columns: number }
  | { type: 'export'; path?: string | null }
  | { type: 'previewImport'; path: string }
  | { type: 'import'; path: string }
  | { type: 'pickImportFile' }
  | { type: 'clearAll' }
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
  | { type: 'paths'; blobsRoot: string }
  /** 面板被唤出：宿主已清空搜索词与分类筛选，前端同步归零。 */
  | { type: 'reset' }
  | { type: 'hotkeyUnavailable'; label: string; message: string }
