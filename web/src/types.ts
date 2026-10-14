/** Clipboard entry kinds, mirroring the .NET ClipboardItemType enum. */
export type ClipboardItemType = 'Text' | 'Image' | 'Files'

/** One history entry as delivered from the native host. */
export interface ClipboardItem {
  id: number
  type: ClipboardItemType
  /** Searchable plain text. For files entries this is the path list. */
  content: string
  /** Display-ready preview line. */
  preview: string
  /** Blob URL for image entries, otherwise null. */
  imageUrl: string | null
  /** Preformatted timestamp for the card footer. */
  createdAtLabel: string
  pinned: boolean
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
  | { type: 'columns'; columns: number }
  | { type: 'dragMove' }
  | { type: 'minimize' }
  | { type: 'maximize' }
  | { type: 'toggleMaximize' }
  | { type: 'close' }

/** Messages the native host pushes to the web UI. */
export type HostPush =
  | { type: 'items'; items: ClipboardItem[]; isEmpty: boolean; totalCount?: number }
  | { type: 'selection'; itemId: number | null; canPrev: boolean; canNext: boolean }
  | { type: 'hotkey'; label: string }
  | { type: 'windowState'; isMaximized: boolean }
