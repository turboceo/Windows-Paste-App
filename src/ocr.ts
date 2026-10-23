/**
 * 图片条目的 OCR 文本约定。
 *
 * 后端把识别结果直接拼进 `Content`：`<尺寸描述>\n\n<OCR 文本>`。
 * 这样搜索框无需改动就能命中图片里的字，但也意味着前端必须知道
 * 这条约定才能把"尺寸"和"识别出的文字"分开显示。
 *
 * 这两个函数就是这条约定的唯一入口，不要在别处再手写 split("\n\n")。
 */

/** 是否已经带有 OCR 识别出的文字。 */
export function hasOcrText(content: string, type: string): boolean {
  return type === 'Image' && content.includes('\n\n')
}

/** 取出 `Content` 里真正的 OCR 文本；没有则返回空串。 */
export function extractOcrText(content: string): string {
  const index = content.indexOf('\n\n')
  if (index === -1) return ''
  return content.slice(index + 2).trim()
}

/** 取出 OCR 之前的尺寸描述（例如 `1920 x 1080 image`）。 */
export function ocrBaseContent(content: string): string {
  const index = content.indexOf('\n\n')
  return (index === -1 ? content : content.slice(0, index)).trim()
}
