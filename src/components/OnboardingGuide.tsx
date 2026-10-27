import { For, Match, Show, Switch, createEffect, createSignal } from 'solid-js'

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
    detail:
      '无论你在任何程序（浏览器、编辑器、微信、办公软件）中，按下热键即可在鼠标当前屏幕居中唤起历史记录，双击或按 Enter 直接粘贴。',
  },
  {
    tag: '快捷清理',
    title: '删除卡片记录',
    subtitle: '鼠标悬停或单键快速剔除',
    detail:
      '选中心仪的卡片按键盘 Del 键直接删除，或将鼠标移至卡片顶部点击出现的垃圾桶图标。删除后光标会自动留在原位置，不会打乱阅读流。',
  },
  {
    tag: '智能分类',
    title: '添加标签与专属色彩',
    subtitle: '点顶部标签栏的「+」一键归类',
    detail:
      '点顶部标签栏末尾的「+」号新建分类，并当场挑选专属颜色！新分类会自动贴到当前选中的卡片上；也可以在任意卡片上右键 -> 「添加到分类」把它归到已有分类里。打上标签后，卡片与顶部导航胶囊都会以该色彩高亮。',
  },
  {
    tag: '标签管理',
    title: '更换颜色与删除分类',
    subtitle: '右键顶部分类胶囊随时管理',
    detail:
      '在顶部 YouTube 风格的标签胶囊上点击右键，即可展开专属调色盘随时换色，或安全删除不需要的分类（删除分类不会删除记录本身）。',
  },
]

/**
 * 新手引导：四步动画 + 文字说明。
 *
 * 演示区是纯 CSS 动画，不依赖真实数据，所以即使用户剪贴板是空的
 * 也能完整讲清楚交互。左右方向键切换、Enter 下一步、Esc 退出。
 */
export default function OnboardingGuide(props: OnboardingGuideProps) {
  const [step, setStep] = createSignal(0)

  // 每次打开都回到第一步
  createEffect(
    () => props.open,
    (open) => {
      if (open) setStep(0)
    },
  )

  createEffect(
    () => props.open,
    (open) => {
      if (!open) return
      const handler = (event: KeyboardEvent) => {
        if (event.key === 'Escape') {
          props.onClose()
        } else if (event.key === 'ArrowRight' || event.key === 'Enter') {
          if (step() < STEPS.length - 1) setStep((s) => s + 1)
          else props.onClose()
        } else if (event.key === 'ArrowLeft') {
          if (step() > 0) setStep((s) => s - 1)
        }
      }
      window.addEventListener('keydown', handler)
      return () => window.removeEventListener('keydown', handler)
    },
  )

  const current = () => STEPS[step()]

  return (
    <Show when={props.open}>
      <div class="guide-backdrop" onClick={() => props.onClose()}>
        <div class="guide-modal" onClick={(event) => event.stopPropagation()}>
          <button class="guide-close-btn" onClick={() => props.onClose()} title="关闭引导 (Esc)">
            &#xE711;
          </button>

          <div class="guide-stage-container">
            <Switch>
              <Match when={step() === 0}>
                <StageHotkey />
              </Match>
              <Match when={step() === 1}>
                <StageDeleteCard />
              </Match>
              <Match when={step() === 2}>
                <StageAddTag />
              </Match>
              <Match when={step() === 3}>
                <StageManageTag />
              </Match>
            </Switch>
          </div>

          <div class="guide-content">
            <div class="guide-badge-row">
              <span class="guide-step-tag">{current().tag}</span>
              <span class="guide-step-counter">
                {step() + 1} / {STEPS.length}
              </span>
            </div>

            <h2 class="guide-title">{current().title}</h2>
            <p class="guide-subtitle">{current().subtitle}</p>
            <p class="guide-detail">{current().detail}</p>

            <div class="guide-footer">
              <div class="guide-dots">
                <For each={STEPS}>
                  {(_, index) => (
                    <button
                      class={["guide-dot", { active: step() === index() }]}
                      onClick={() => setStep(index())}
                      title={`跳转到第 ${index() + 1} 步`}
                    />
                  )}
                </For>
              </div>

              <div class="guide-actions">
                <Show when={step() > 0}>
                  <button class="guide-btn secondary" onClick={() => setStep((s) => s - 1)}>
                    上一步
                  </button>
                </Show>
                <Show
                  when={step() < STEPS.length - 1}
                  fallback={
                    <button class="guide-btn primary finish" onClick={() => props.onClose()}>
                      开始使用
                    </button>
                  }
                >
                  <button class="guide-btn primary" onClick={() => setStep((s) => s + 1)}>
                    下一步
                  </button>
                </Show>
              </div>
            </div>
          </div>
        </div>
      </div>
    </Show>
  )
}

/** 动画场景 1: Ctrl + Shift + V 快捷键呼出演示 */
function StageHotkey() {
  return (
    <div class="stage-scene stage-hotkey">
      <div class="anim-desktop-bg">
        <div class="anim-window-mockup">
          <div class="anim-win-header">
            <span class="mock-dot" />
            <span class="mock-dot" />
            <span class="mock-dot" />
          </div>
          <div class="anim-win-text-lines">
            <div class="mock-line w70" />
            <div class="mock-line w40" />
          </div>
        </div>
      </div>

      <div class="anim-keyboard-keys">
        <div class="key-cap key-ctrl">Ctrl</div>
        <span class="key-plus">+</span>
        <div class="key-cap key-shift">Shift</div>
        <span class="key-plus">+</span>
        <div class="key-cap key-v">V</div>
      </div>

      <div class="anim-paste-panel">
        <div class="panel-mock-header">
          <span class="panel-brand-icon">&#xE8A5;</span>
          <span class="panel-brand-text">Windows Paste</span>
        </div>
        <div class="panel-mock-cards">
          <div class="mock-mini-card c1" />
          <div class="mock-mini-card c2 active" />
          <div class="mock-mini-card c3" />
        </div>
      </div>
    </div>
  )
}

/** 动画场景 2: 删除卡片演示 */
function StageDeleteCard() {
  return (
    <div class="stage-scene stage-delete">
      <div class="anim-card-row">
        <div class="anim-demo-card normal left">
          <div class="demo-card-head blue" />
          <div class="demo-card-body">
            <div class="mock-line w80" />
            <div class="mock-line w50" />
          </div>
        </div>

        <div class="anim-demo-card target-delete">
          <div class="demo-card-head orange">
            <span class="trash-icon-anim">&#xE74D;</span>
          </div>
          <div class="demo-card-body">
            <div class="mock-line w70" />
            <div class="mock-line w90" />
          </div>
          <div class="anim-cursor-hand" />
        </div>

        <div class="anim-demo-card normal right">
          <div class="demo-card-head green" />
          <div class="demo-card-body">
            <div class="mock-line w60" />
          </div>
        </div>
      </div>

      <div class="del-tip-bubble">
        <span>
          按 <b>Del</b> 或点击垃圾桶
        </span>
      </div>
    </div>
  )
}

/** 动画场景 3: 添加标签与颜色演示 */
function StageAddTag() {
  return (
    <div class="stage-scene stage-add-tag">
      <div class="anim-demo-card with-context-menu">
        <div class="demo-card-head green" />
        <div class="demo-card-body">
          <div class="mock-line w80" />
          <div class="mock-line w40" />
        </div>

        <div class="anim-tag-pill-popup">
          <span class="anim-pill-dot" />
          <span>工作常用</span>
        </div>
      </div>

      <div class="anim-context-menu-pop">
        <div class="ctx-item">
          <span class="ctx-icon">&#xE77F;</span>
          <span>立即粘贴</span>
        </div>
        <div class="ctx-item active-submenu">
          <span class="ctx-icon">&#xE8EC;</span>
          <span>添加到分类</span>
          <span class="ctx-arrow">&#xE76C;</span>
        </div>
        <div class="ctx-sub-palette">
          <div class="sub-input-row">
            <span class="palette-preview-dot" />
            <span class="fake-input-text">工作常用|</span>
          </div>
          <div class="sub-palette-colors">
            <span class="pal-c c-orange" />
            <span class="pal-c c-green active" />
            <span class="pal-c c-blue" />
            <span class="pal-c c-purple" />
            <span class="pal-c c-red" />
          </div>
        </div>
      </div>
    </div>
  )
}

/** 动画场景 4: 标签栏管理与删除分类演示 */
function StageManageTag() {
  return (
    <div class="stage-scene stage-manage-tag">
      <div class="anim-chip-bar-container">
        <div class="anim-chip all">全部</div>
        <div class="anim-chip tagged active">
          <span class="chip-dot-green" />
          <span>工作常用</span>
        </div>
        <div class="anim-chip tagged orange">
          <span class="chip-dot-orange" />
          <span>代码片段</span>
        </div>
      </div>

      <div class="anim-chip-manage-popover">
        <div class="pop-head">
          <span class="pop-dot" />
          <span>工作常用</span>
        </div>
        <div class="pop-label">更改颜色：</div>
        <div class="pop-colors">
          <span class="p-dot c-green" />
          <span class="p-dot c-blue pick-anim" />
          <span class="p-dot c-orange" />
          <span class="p-dot c-purple" />
          <span class="p-dot c-pink" />
        </div>
        <div class="pop-line" />
        <div class="pop-del-btn">
          <span class="del-icon">&#xE74D;</span>
          <span>删除该分类</span>
        </div>
      </div>

      <div class="tag-mgmt-hint">
        <span>右键任意分类胶囊，随时换色或删除</span>
      </div>
    </div>
  )
}
