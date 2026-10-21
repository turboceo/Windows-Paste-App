import { useEffect, useState } from 'react'
import { getVersion } from '@tauri-apps/api/app'
import { isHosted, send } from '../bridge'

interface TitleBarProps {
  isMaximized: boolean
  totalCount: number
  filteredCount: number
}

/** Frameless-window title bar with the app brand, count badge and controls. */
export default function TitleBar({ isMaximized, totalCount, filteredCount }: TitleBarProps) {
  // 版本号从打包配置（tauri.conf.json 的 version）读，不在代码里另存一份，
  // 免得发版时改了一处忘了另一处。
  const [version, setVersion] = useState('')

  useEffect(() => {
    if (!isHosted()) return
    let cancelled = false
    getVersion()
      .then((v) => {
        if (!cancelled) setVersion(v)
      })
      .catch(() => {
        // 读不到就不显示，不影响其它功能
      })
    return () => {
      cancelled = true
    }
  }, [])

  const isFiltering = filteredCount !== totalCount

  const badgeText =
    totalCount === 0 ? '0 项' : isFiltering ? `${filteredCount} / ${totalCount}` : `${totalCount} 项`

  const badgeTitle = isFiltering
    ? `已筛选 ${filteredCount} 项（共 ${totalCount} 项）`
    : `当前共存储 ${totalCount} 条剪贴板历史`

  function onMouseDown(event: React.MouseEvent): void {
    if (event.button === 0 && !(event.target as HTMLElement).closest('.window-control-btn')) {
      send({ type: 'dragMove' })
    }
  }

  function onDoubleClick(event: React.MouseEvent): void {
    if (!(event.target as HTMLElement).closest('.window-control-btn')) {
      send({ type: 'toggleMaximize' })
    }
  }

  return (
    <header className="titlebar" onMouseDown={onMouseDown} onDoubleClick={onDoubleClick}>
      <div className="brand">
        <span className="brand-icon">&#xE8A5;</span>
        <span className="brand-title">剪贴板历史</span>
        <span className={`count-badge${isFiltering ? ' filtering' : ''}`} title={badgeTitle}>
          {badgeText}
        </span>
        {version && (
          <span className="version-badge" title={`Windows Paste v${version}`}>
            v{version}
          </span>
        )}
      </div>

      <div className="drag-region" />

      <div className="window-controls">
        <button
          className="window-control-btn"
          title="最小化"
          onClick={(event) => {
            event.stopPropagation()
            send({ type: 'minimize' })
          }}
        >
          &#xE921;
        </button>

        <button
          className="window-control-btn"
          title={isMaximized ? '还原' : '最大化'}
          onClick={(event) => {
            event.stopPropagation()
            send({ type: 'toggleMaximize' })
          }}
        >
          {isMaximized ? '\uE923' : '\uE922'}
        </button>

        <button
          className="window-control-btn btn-close"
          title="关闭 (Esc)"
          onClick={(event) => {
            event.stopPropagation()
            send({ type: 'close' })
          }}
        >
          &#xE8BB;
        </button>
      </div>
    </header>
  )
}
