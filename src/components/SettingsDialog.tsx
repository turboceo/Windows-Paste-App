import { useEffect } from 'react'

interface SettingsDialogProps {
  open: boolean
  version: string
  dataRoot: string
  blobsRoot: string
  onClose: () => void
  onOpenDataDir: () => void
}

export default function SettingsDialog({
  open,
  version,
  dataRoot,
  blobsRoot,
  onClose,
  onOpenDataDir,
}: SettingsDialogProps) {
  useEffect(() => {
    function handleKeyDown(e: KeyboardEvent) {
      if (!open) return
      if (e.key === 'Escape') {
        onClose()
      }
    }
    window.addEventListener('keydown', handleKeyDown)
    return () => window.removeEventListener('keydown', handleKeyDown)
  }, [open, onClose])

  if (!open) return null

  return (
    <div className="dialog-backdrop" onClick={onClose}>
      <div className="dialog settings-dialog" onClick={(e) => e.stopPropagation()}>
        <div className="settings-header">
          <div className="settings-brand">
            <span className="settings-logo">&#xE8A5;</span>
            <div className="settings-titles">
              <h3 className="settings-app-name">Windows Paste</h3>
              <span className="settings-version-tag">v{version || '0.2.0'}</span>
            </div>
          </div>
          <button className="settings-close-btn" onClick={onClose} title="关闭 (Esc)">
            &#xE711;
          </button>
        </div>

        <div className="settings-body">
          {/* 数据与缓存存储路径 */}
          <div className="settings-section">
            <div className="section-header">
              <span className="section-title">本地存储目录</span>
              <button
                type="button"
                className="open-dir-btn"
                onClick={onOpenDataDir}
                title="在 Windows 文件资源管理器中定位"
              >
                <span className="btn-icon">&#xE838;</span>
                打开存储目录
              </button>
            </div>
            <div className="path-box" title={dataRoot || '默认数据目录'}>
              <span className="path-label">数据主目录:</span>
              <span className="path-val">{dataRoot || '%LOCALAPPDATA%\\WindowsPaste'}</span>
            </div>
            <div className="path-box" title={blobsRoot || '图片缓存目录'}>
              <span className="path-label">图片文件库:</span>
              <span className="path-val">{blobsRoot || '%LOCALAPPDATA%\\WindowsPaste\\blobs'}</span>
            </div>
          </div>

          {/* 常用快捷键速查 */}
          <div className="settings-section">
            <div className="section-title">快捷键速查</div>
            <div className="shortcut-list">
              <div className="shortcut-row">
                <span className="sc-desc">唤起 / 隐藏历史面板</span>
                <span className="sc-keys">
                  <kbd>Ctrl</kbd> + <kbd>Shift</kbd> + <kbd>V</kbd>
                </span>
              </div>
              <div className="shortcut-row">
                <span className="sc-desc">复制内容至剪贴板</span>
                <span className="sc-keys">双击卡片</span>
              </div>
              <div className="shortcut-row">
                <span className="sc-desc">直接回贴至前台程序</span>
                <span className="sc-keys">
                  <kbd>Enter</kbd> 或 右键粘贴
                </span>
              </div>
              <div className="shortcut-row">
                <span className="sc-desc">置顶 / 取消置顶</span>
                <span className="sc-keys">
                  <kbd>Ctrl</kbd> + <kbd>P</kbd>
                </span>
              </div>
              <div className="shortcut-row">
                <span className="sc-desc">删除选中的卡片</span>
                <span className="sc-keys">
                  <kbd>Del</kbd>
                </span>
              </div>
            </div>
          </div>
        </div>

        <div className="settings-footer">
          <span className="settings-tip">剪贴板历史与图片全部保存在本地，绝不上云</span>
          <button className="dialog-btn primary" onClick={onClose}>
            完成
          </button>
        </div>
      </div>
    </div>
  )
}
