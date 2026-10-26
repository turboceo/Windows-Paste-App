import { Show, createSignal } from 'solid-js'
import { checkLatestRelease, type UpdateInfo } from '../updater'

interface SettingsDialogProps {
  open: boolean
  version: () => string
  dataRoot: () => string
  blobsRoot: () => string
  onClose: () => void
  onOpenDataDir: () => void
  onOpenUrl: (url: string) => void
}

type CheckStatus = 'idle' | 'checking' | 'latest' | 'available' | 'error'

/**
 * 设置与关于弹窗：版本、更新检测、存储目录、常用快捷键。
 *
 * 路径只做展示，打开动作交给后端（`openDataDir` / `openUrl`），
 * 因为 WebView 里拿不到直接操作系统 Shell 的能力。
 */
export default function SettingsDialog(props: SettingsDialogProps) {
  const [checkStatus, setCheckStatus] = createSignal<CheckStatus>('idle')
  const [updateInfo, setUpdateInfo] = createSignal<UpdateInfo | null>(null)
  const [errorMessage, setErrorMessage] = createSignal<string>('')

  const handleCheckUpdate = async () => {
    if (checkStatus() === 'checking') return
    setCheckStatus('checking')
    setErrorMessage('')

    const current = props.version() || '0.2.0'
    const res = await checkLatestRelease(current)

    if (res.status === 'error') {
      setCheckStatus('error')
      setErrorMessage(res.message)
    } else {
      setUpdateInfo(res.data)
      setCheckStatus(res.data.hasUpdate ? 'available' : 'latest')
    }
  }

  return (
    <Show when={props.open}>
      <div class="dialog-backdrop" onClick={() => props.onClose()}>
        <div class="dialog settings-dialog" onClick={(event) => event.stopPropagation()}>
          <div class="settings-header">
            <div class="settings-brand">
              <span class="settings-logo">&#xE8A5;</span>
              <div class="settings-titles">
                <h3 class="settings-app-name">Windows Paste</h3>
                <span class="settings-version-tag">v{props.version() || '0.2.0'}</span>
              </div>
            </div>
            <button class="settings-close-btn" onClick={() => props.onClose()} title="关闭 (Esc)">
              &#xE711;
            </button>
          </div>

          <div class="settings-body">
            {/* 版本更新检测卡片 */}
            <div class="settings-section update-section">
              <div class="section-header">
                <span class="section-title">版本更新</span>
                <button
                  type="button"
                  class={['check-update-btn', { loading: checkStatus() === 'checking' }]}
                  onClick={handleCheckUpdate}
                  disabled={checkStatus() === 'checking'}
                >
                  <span class="btn-icon">&#xE72C;</span>
                  {checkStatus() === 'checking' ? '正在检查...' : '检查更新'}
                </button>
              </div>

              {/* 状态展示与结果 */}
              <Show when={checkStatus() === 'latest'}>
                <div class="update-status-box latest">
                  <span class="status-icon">&#xE73E;</span>
                  <span class="status-text">当前已是最新版本 (v{props.version()})</span>
                </div>
              </Show>

              <Show when={checkStatus() === 'error'}>
                <div class="update-status-box error">
                  <span class="status-icon">&#xE783;</span>
                  <span class="status-text">{errorMessage()}</span>
                </div>
              </Show>

              <Show when={checkStatus() === 'available' && updateInfo()}>
                {(info) => (
                  <div class="update-available-card">
                    <div class="update-card-header">
                      <span class="update-badge">发现新版本</span>
                      <span class="update-ver-label">v{info().latestVersion}</span>
                      <Show when={info().publishedAt}>
                        <span class="update-date">{info().publishedAt}</span>
                      </Show>
                    </div>

                    <Show when={info().releaseNotes}>
                      <div class="update-notes-box">
                        <pre class="update-notes-text">{info().releaseNotes}</pre>
                      </div>
                    </Show>

                    <div class="update-actions-row">
                      <Show when={info().downloadUrl}>
                        <button
                          type="button"
                          class="dialog-btn primary update-action-btn"
                          onClick={() => props.onOpenUrl(info().downloadUrl!)}
                        >
                          <span class="btn-icon">&#xE896;</span>
                          下载安装包 (.exe)
                        </button>
                      </Show>
                      <button
                        type="button"
                        class="dialog-btn secondary update-action-btn"
                        onClick={() => props.onOpenUrl(info().releaseUrl)}
                      >
                        <span class="btn-icon">&#xE8A7;</span>
                        前往 GitHub Release
                      </button>
                    </div>
                  </div>
                )}
              </Show>
            </div>

            {/* 本地存储目录 */}
            <div class="settings-section">
              <div class="section-header">
                <span class="section-title">本地存储目录</span>
                <button
                  type="button"
                  class="open-dir-btn"
                  onClick={() => props.onOpenDataDir()}
                  title="在 Windows 文件资源管理器中定位"
                >
                  <span class="btn-icon">&#xE838;</span>
                  打开存储目录
                </button>
              </div>
              <div class="path-box" title={props.dataRoot() || '默认数据目录'}>
                <span class="path-label">数据主目录:</span>
                <span class="path-val">
                  {props.dataRoot() || '%LOCALAPPDATA%\\WindowsPaste'}
                </span>
              </div>
              <div class="path-box" title={props.blobsRoot() || '图片缓存目录'}>
                <span class="path-label">图片文件库:</span>
                <span class="path-val">
                  {props.blobsRoot() || '%LOCALAPPDATA%\\WindowsPaste\\blobs'}
                </span>
              </div>
            </div>

            {/* 常用快捷键速查 */}
            <div class="settings-section">
              <div class="section-title">快捷键速查</div>
              <div class="shortcut-list">
                <div class="shortcut-row">
                  <span class="sc-desc">唤起 / 隐藏历史面板</span>
                  <span class="sc-keys">
                    <kbd>Ctrl</kbd> + <kbd>Shift</kbd> + <kbd>V</kbd>
                  </span>
                </div>
                <div class="shortcut-row">
                  <span class="sc-desc">复制内容至剪贴板</span>
                  <span class="sc-keys">双击卡片</span>
                </div>
                <div class="shortcut-row">
                  <span class="sc-desc">直接回贴至前台程序</span>
                  <span class="sc-keys">
                    <kbd>Enter</kbd> 或 右键粘贴
                  </span>
                </div>
                <div class="shortcut-row">
                  <span class="sc-desc">置顶 / 取消置顶</span>
                  <span class="sc-keys">
                    <kbd>Ctrl</kbd> + <kbd>P</kbd>
                  </span>
                </div>
                <div class="shortcut-row">
                  <span class="sc-desc">删除选中的卡片</span>
                  <span class="sc-keys">
                    <kbd>Del</kbd>
                  </span>
                </div>
              </div>
            </div>
          </div>

          <div class="settings-footer">
            <span class="settings-tip">剪贴板历史与图片全部保存在本地，绝不上云</span>
            <button class="dialog-btn primary" onClick={() => props.onClose()}>
              完成
            </button>
          </div>
        </div>
      </div>
    </Show>
  )
}
