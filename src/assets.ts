import { convertFileSrc, isTauri } from '@tauri-apps/api/core'

/**
 * 把后端给的 blob 文件名拼成 WebView 可直接加载的 URL。
 *
 * 后端只传 `<sha256>.png` 这样的文件名，图片本体通过 Tauri 的 asset 协议
 * 从 `%LOCALAPPDATA%\WindowsPaste\blobs\` 直读——不经过 IPC，
 * 也不会在每次推送时被重新 base64 编码。
 *
 * `blobsRoot` 由宿主在 ready 时下发，作为 prop 一路传到卡片；
 * 刻意不做成模块级可变变量：那样宿主下发路径时不会触发重渲染，
 * 图片会一直停在占位状态。
 */

/** 在浏览器里跑开发预览时没有 resource 协议，返回 null，卡片退回文字预览。 */
export function blobUrl(
  blobsRoot: string | null,
  fileName: string | null | undefined,
): string | null {
  if (!fileName || !blobsRoot) {
    // 排查用：blobsRoot 为空说明宿主没有下发路径（多半是 ready 推送丢了）
    if (fileName && !blobsRoot) {
      console.warn('[assets] blobsRoot 尚未下发，图片无法解析')
    }
    return null
  }
  if (!isTauri()) return null

  // Windows 路径分隔符在拼 URL 前统一
  const full = `${blobsRoot.replace(/[\\/]+$/, '')}\\${fileName}`
  try {
    return convertFileSrc(full)
  } catch (error) {
    console.error('[assets] convertFileSrc 失败:', full, error)
    return null
  }
}
