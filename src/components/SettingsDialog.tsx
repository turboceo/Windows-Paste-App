import { Show } from 'solid-js'

interface SettingsDialogProps {
  open: boolean
  version: () => string
  dataRoot: () => string
  blobsRoot: () => string
  onClose: () => void
  onOpenDataDir: () => void
}

/**
 * 设置与关于弹窗：版本、存储目录、常用快捷键。
 *
 * 路径只做展示，真正的打开动作交给后端（`openDataDir`），
 * 因为 WebView 里拿不到可靠的本地路径操作能力。
 */
export default function SettingsDialog(props: SettingsDialogProps) {
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
