import { useEffect, useState } from 'react'

interface OnboardingGuideProps {
  open: boolean
  onClose: () => void
}

interface StepInfo {
  title: string
  subtitle: string
  detail: string
  tag: string
}

const STEPS: StepInfo[] = [
  {
    tag: '随时呼出',
    title: '全局快捷键唤起面板',
    subtitle: '按 Ctrl + Shift + V 随叫随到',
    detail: '无论你在任何程序（浏览器、编辑器、微信、办公软件）中，按下热键即可在鼠标当前屏幕居中唤起历史记录，双击或按 Enter 直接粘贴。',
  },
  {
    tag: '快捷清理',
    title: '删除卡片记录',
    subtitle: '鼠标悬停或单键快速剔除',
    detail: '选中心仪的卡片按键盘 Del 键直接删除，或将鼠标移至卡片顶部点击出现的垃圾桶图标。删除后光标会自动留在原位置，不会打乱阅读流。',
  },
  {
    tag: '智能分类',
    title: '添加标签与专属色彩',
    subtitle: '右键卡片一键归类并挑个颜色',
    detail: '在任意卡片上右键 -> 选择「新建分类标签」，支持为标签自由挑选专属颜色！打上标签后，卡片与顶部导航胶囊都会以该色彩高亮。',
  },
  {
    tag: '标签管理',
    title: '更换颜色与删除分类',
    subtitle: '右键顶部分类胶囊随时管理',
    detail: '在顶部 YouTube 风格的标签胶囊上点击右键，即可展开专属调色盘随时换色，或安全删除不需要的分类（删除分类不会删除记录本身）。',
  },
]

export default function OnboardingGuide({ open, onClose }: OnboardingGuideProps) {
  const [currentStep, setCurrentStep] = useState(0)

  useEffect(() => {
    if (open) {
      setCurrentStep(0)
    }
  }, [open])

  useEffect(() => {
    function handleKeyDown(e: KeyboardEvent) {
      if (!open) return
      if (e.key === 'Escape') {
        onClose()
      } else if (e.key === 'ArrowRight' || e.key === 'Enter') {
        if (currentStep < STEPS.length - 1) {
          setCurrentStep((prev) => prev + 1)
        } else {
          onClose()
        }
      } else if (e.key === 'ArrowLeft') {
        if (currentStep > 0) {
          setCurrentStep((prev) => prev - 1)
        }
      }
    }
    window.addEventListener('keydown', handleKeyDown)
    return () => window.removeEventListener('keydown', handleKeyDown)
  }, [open, currentStep, onClose])

  if (!open) return null

  const step = STEPS[currentStep]

  return (
    <div className="guide-backdrop" onClick={onClose}>
      <div className="guide-modal" onClick={(e) => e.stopPropagation()}>
        {/* 顶部关闭按钮 */}
        <button className="guide-close-btn" onClick={onClose} title="关闭引导 (Esc)">
          &#xE711;
        </button>

        {/* 上半部分：生动的模拟动画演示区 */}
        <div className="guide-stage-container">
          {currentStep === 0 && <StageHotkey />}
          {currentStep === 1 && <StageDeleteCard />}
          {currentStep === 2 && <StageAddTag />}
          {currentStep === 3 && <StageManageTag />}
        </div>

        {/* 下半部分：功能说明与引导控制 */}
        <div className="guide-content">
          <div className="guide-badge-row">
            <span className="guide-step-tag">{step.tag}</span>
            <span className="guide-step-counter">
              {currentStep + 1} / {STEPS.length}
            </span>
          </div>

          <h2 className="guide-title">{step.title}</h2>
          <p className="guide-subtitle">{step.subtitle}</p>
          <p className="guide-detail">{step.detail}</p>

          <div className="guide-footer">
            {/* 分步指示器 */}
            <div className="guide-dots">
              {STEPS.map((_, i) => (
                <button
                  key={i}
                  className={`guide-dot${currentStep === i ? ' active' : ''}`}
                  onClick={() => setCurrentStep(i)}
                  title={`跳转到第 ${i + 1} 步`}
                />
              ))}
            </div>

            {/* 操作控制 */}
            <div className="guide-actions">
              {currentStep > 0 && (
                <button
                  className="guide-btn secondary"
                  onClick={() => setCurrentStep((prev) => prev - 1)}
                >
                  上一步
                </button>
              )}
              {currentStep < STEPS.length - 1 ? (
                <button
                  className="guide-btn primary"
                  onClick={() => setCurrentStep((prev) => prev + 1)}
                >
                  下一步
                </button>
              ) : (
                <button className="guide-btn primary finish" onClick={onClose}>
                  开始使用
                </button>
              )}
            </div>
          </div>
        </div>
      </div>
    </div>
  )
}

/** 动画场景 1: Ctrl + Shift + V 快捷键呼出演示 */
function StageHotkey() {
  return (
    <div className="stage-scene stage-hotkey">
      <div className="anim-desktop-bg">
        <div className="anim-window-mockup">
          <div className="anim-win-header">
            <span className="mock-dot" />
            <span className="mock-dot" />
            <span className="mock-dot" />
          </div>
          <div className="anim-win-text-lines">
            <div className="mock-line w70" />
            <div className="mock-line w40" />
          </div>
        </div>
      </div>

      {/* 虚拟键盘按键敲击 */}
      <div className="anim-keyboard-keys">
        <div className="key-cap key-ctrl">Ctrl</div>
        <span className="key-plus">+</span>
        <div className="key-cap key-shift">Shift</div>
        <span className="key-plus">+</span>
        <div className="key-cap key-v">V</div>
      </div>

      {/* 呼出的剪贴板窗口 */}
      <div className="anim-paste-panel">
        <div className="panel-mock-header">
          <span className="panel-brand-icon">&#xE8A5;</span>
          <span className="panel-brand-text">Windows Paste</span>
        </div>
        <div className="panel-mock-cards">
          <div className="mock-mini-card c1" />
          <div className="mock-mini-card c2 active" />
          <div className="mock-mini-card c3" />
        </div>
      </div>
    </div>
  )
}

/** 动画场景 2: 删除卡片演示 */
function StageDeleteCard() {
  return (
    <div className="stage-scene stage-delete">
      <div className="anim-card-row">
        <div className="anim-demo-card normal left">
          <div className="demo-card-head blue" />
          <div className="demo-card-body">
            <div className="mock-line w80" />
            <div className="mock-line w50" />
          </div>
        </div>

        {/* 即将被删除的目标卡片 */}
        <div className="anim-demo-card target-delete">
          <div className="demo-card-head orange">
            <span className="trash-icon-anim">&#xE74D;</span>
          </div>
          <div className="demo-card-body">
            <div className="mock-line w70" />
            <div className="mock-line w90" />
          </div>

          {/* 模拟光标悬停与删除动作 */}
          <div className="anim-cursor-hand" />
        </div>

        <div className="anim-demo-card normal right">
          <div className="demo-card-head green" />
          <div className="demo-card-body">
            <div className="mock-line w60" />
          </div>
        </div>
      </div>

      <div className="del-tip-bubble">
        <span>按 <b>Del</b> 或点击垃圾桶</span>
      </div>
    </div>
  )
}

/** 动画场景 3: 添加标签与颜色演示 */
function StageAddTag() {
  return (
    <div className="stage-scene stage-add-tag">
      <div className="anim-demo-card with-context-menu">
        <div className="demo-card-head green" />
        <div className="demo-card-body">
          <div className="mock-line w80" />
          <div className="mock-line w40" />
        </div>

        {/* 动态出现的标签胶囊 */}
        <div className="anim-tag-pill-popup">
          <span className="anim-pill-dot" />
          <span>工作常用</span>
        </div>
      </div>

      {/* 模拟右键菜单与调色盘弹窗 */}
      <div className="anim-context-menu-pop">
        <div className="ctx-item">
          <span className="ctx-icon">&#xE77F;</span>
          <span>立即粘贴</span>
        </div>
        <div className="ctx-item active-submenu">
          <span className="ctx-icon">&#xE8EC;</span>
          <span>添加到分类</span>
          <span className="ctx-arrow">&#xE76C;</span>
        </div>
        <div className="ctx-sub-palette">
          <div className="sub-input-row">
            <span className="palette-preview-dot" />
            <span className="fake-input-text">工作常用|</span>
          </div>
          <div className="sub-palette-colors">
            <span className="pal-c c-orange" />
            <span className="pal-c c-green active" />
            <span className="pal-c c-blue" />
            <span className="pal-c c-purple" />
            <span className="pal-c c-red" />
          </div>
        </div>
      </div>
    </div>
  )
}

/** 动画场景 4: 标签栏管理与删除分类演示 */
function StageManageTag() {
  return (
    <div className="stage-scene stage-manage-tag">
      {/* 模拟 YouTube 风格胶囊栏 */}
      <div className="anim-chip-bar-container">
        <div className="anim-chip all">全部</div>
        <div className="anim-chip tagged active">
          <span className="chip-dot-green" />
          <span>工作常用</span>
        </div>
        <div className="anim-chip tagged orange">
          <span className="chip-dot-orange" />
          <span>代码片段</span>
        </div>
      </div>

      {/* 鼠标右键点击胶囊弹出的气泡管理窗 */}
      <div className="anim-chip-manage-popover">
        <div className="pop-head">
          <span className="pop-dot" />
          <span>工作常用</span>
        </div>
        <div className="pop-label">更改颜色：</div>
        <div className="pop-colors">
          <span className="p-dot c-green" />
          <span className="p-dot c-blue pick-anim" />
          <span className="p-dot c-orange" />
          <span className="p-dot c-purple" />
          <span className="p-dot c-pink" />
        </div>
        <div className="pop-line" />
        <div className="pop-del-btn">
          <span className="del-icon">&#xE74D;</span>
          <span>删除该分类</span>
        </div>
      </div>

      <div className="tag-mgmt-hint">
        <span>右键任意分类胶囊，随时换色或删除</span>
      </div>
    </div>
  )
}
