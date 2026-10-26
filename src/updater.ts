/**
 * GitHub Releases 版本检测与解析工具。
 *
 * 仓库地址：https://github.com/turboceo/Windows-Paste-App
 * API 端点：https://api.github.com/repos/turboceo/Windows-Paste-App/releases/latest
 */

export interface UpdateInfo {
  hasUpdate: boolean
  currentVersion: string
  latestVersion: string
  releaseName: string
  releaseNotes: string
  publishedAt: string
  releaseUrl: string
  /** 直接下载链接 (匹配 setup.exe 资产) */
  downloadUrl?: string
}

export type CheckResult =
  | { status: 'success'; data: UpdateInfo }
  | { status: 'error'; message: string }

/**
 * 比较两个语义化版本号（支持诸如 0.2.0、v0.2.1、0.3.0-rc1 等格式）。
 * 如果 target > current 返回 1；相等返回 0；小于返回 -1。
 */
export function compareSemver(current: string, target: string): number {
  const parse = (v: string): number[] => {
    return v
      .trim()
      .replace(/^[vV]/, '')
      .split(/[-+]/)[0] // 去掉预发布标签
      .split('.')
      .map((part) => parseInt(part, 10) || 0)
  }

  const cParts = parse(current)
  const tParts = parse(target)

  const len = Math.max(cParts.length, tParts.length)
  for (let i = 0; i < len; i++) {
    const cVal = cParts[i] ?? 0
    const tVal = tParts[i] ?? 0
    if (tVal > cVal) return 1
    if (tVal < cVal) return -1
  }

  return 0
}

/**
 * 请求 GitHub Releases 检查新版本。
 */
export async function checkLatestRelease(currentVersion: string): Promise<CheckResult> {
  const repo = 'turboceo/Windows-Paste-App'
  const endpoint = `https://api.github.com/repos/${repo}/releases/latest`

  try {
    const controller = new AbortController()
    const timeoutId = setTimeout(() => controller.abort(), 8000)

    const res = await fetch(endpoint, {
      signal: controller.signal,
      headers: {
        Accept: 'application/vnd.github.v3+json',
      },
    })
    clearTimeout(timeoutId)

    if (res.status === 404) {
      return {
        status: 'error',
        message: '仓库暂未发布正式 Release 版本',
      }
    }

    if (res.status === 403) {
      return {
        status: 'error',
        message: 'GitHub API 访问受限，请稍后重试',
      }
    }

    if (!res.ok) {
      return {
        status: 'error',
        message: `检查失败 (HTTP ${res.status})`,
      }
    }

    const data = await res.json()
    const tagName: string = data.tag_name || ''
    const cleanLatest = tagName.replace(/^[vV]/, '')

    // 查找 setup.exe 资产
    let downloadUrl: string | undefined
    if (Array.isArray(data.assets)) {
      const exeAsset = data.assets.find(
        (a: { name?: string; browser_download_url?: string }) =>
          typeof a.name === 'string' && a.name.toLowerCase().endsWith('.exe'),
      )
      if (exeAsset && exeAsset.browser_download_url) {
        downloadUrl = exeAsset.browser_download_url
      }
    }

    const hasUpdate = compareSemver(currentVersion, cleanLatest) > 0

    return {
      status: 'success',
      data: {
        hasUpdate,
        currentVersion,
        latestVersion: cleanLatest || tagName,
        releaseName: data.name || tagName,
        releaseNotes: data.body || '',
        publishedAt: data.published_at ? data.published_at.slice(0, 10) : '',
        releaseUrl: data.html_url || `https://github.com/${repo}/releases`,
        downloadUrl,
      },
    }
  } catch (err) {
    const isAbort = (err as Error)?.name === 'AbortError'
    return {
      status: 'error',
      message: isAbort ? '连接 GitHub 超时，请检查网络' : '网络请求失败，请稍后重试',
    }
  }
}
