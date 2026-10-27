# Windows Paste (Tauri 2 + Vite + SolidJS 2.0)

一个基于 **Tauri 2 + Vite + SolidJS 2.0 (TypeScript)** 的高性能 Windows 剪贴板历史管理器，深度还原 macOS 上著名应用 **[Paste](https://pasteapp.io/)** 的质感与交互体验。

随时按下全局快捷键 `Ctrl + Shift + V`，在屏幕正中央瞬间唤出剪贴板面板，进行高速搜索、预览与一键粘贴。

---

## 核心特性

- **现代超轻量架构**：基于 Tauri 2 + 嵌入式 WebView2，利用原生 Chromium 渲染引擎与硬件加速；前端全面迁移至 **SolidJS 2.0**，抛弃虚拟 DOM，采用细粒度响应式更新，前端打包产物仅 **~113 KB**，内存开销轻盈如初。
- **纯原生丝滑滚动**：基于虚拟列表（Virtual Windowing）与原生 CSS 绝对定位平滑过渡，完美解决海量卡片滚动的性能与掉帧瓶颈。
- **Paste.app 原生卡片美学**：
  - **极淡白底（Near-White）现代质感**：面板采用纯净清爽的极淡雪白底板，卡片纯白立体，排版清晰干净。
  - **彩色类型头部色带**：每张卡片顶部是一条按内容类型着色的微渐变色带（文本橙 / 链接绿 / 图像红 / 文件琥珀 / 颜色紫），左侧显示类型名与相对时间（如「4 小时前」），右侧是类型图标，悬停时平滑替换为置顶与删除按钮；
  - **按类型区分的信息展示**：
    - 纯文本：纯白正文，底部居中显示字符数；
    - 网页链接：显示完整 URL，底部标注主机域名（如 `github.com`）；
    - 屏幕截图与图像：缩略图铺满正文，底部浮一条半透明胶囊显示像素分辨率；
    - 文件列表：清晰展示包含图标的文件清单与总文件数；
    - 颜色代码：智能识别 `#RGB` / `#RRGGBB` / `rgb(...)`，正文呈现大色块色板，底部显示色值。
  - **堆叠卡片层次**：悬停或选中时卡片上沿探出两层同色薄边，像一叠待取的历史记录；平时完全收起，列表整洁无噪点。
  - **皇家蓝高对比聚焦外环**：选中时呈现 macOS 风格的 2px `#007aff` 亮蓝环与微浮升效果。
- **本地离线 OCR 图文识别（免安装、0 体积）**：
  - 基于 Windows 原生 WinRT OCR 引擎，**无需电脑安装微信，无需联网，无需外挂体积庞大的模型文件**；
  - 复制或截图进来的图片在后台自动静默识别文字并建立索引，**支持在搜索框输入文字直接搜索图片**；
  - 图片卡片右键支持「查看识别文字」只读弹窗（可自由选中局部文本），或一键「复制图片中的文字 (OCR)」，历史老图片同样支持右键补做识别。
- **YouTube 风格分类标签栏 (`feed-filter-chip-bar`)**：
  - 支持无感鼠标滚轮平滑横向滚动，溢出时自动浮现渐变边缘遮罩与左右翻页圆形箭头；
  - 支持 11 种专属个性调色盘，标签栏末尾的「+」可直接新建分类并自定义色彩；顶部胶囊与卡片脚部标签实时联动；
  - 右键任意分类胶囊，展开管理气泡随时更换颜色或安全删除分类。
- **生产力交互与快捷键**：
  - `Left` / `Right` / `Up` / `Down` 矩阵式键盘导航；
  - 拖拽卡片可手动排序（置顶与未置顶各自成组），移动时带有滑动与吸附动画；
  - 双击卡片直接复制内容到剪贴板，**保持窗口留在前台不退到后台**；按 `Enter` 键或右键选择「立即粘贴」可直接将内容回写并注入 `Ctrl + V` 到前台第三方应用；
  - `Ctrl + P` 或悬停卡片头部点击图钉按钮置顶条目；
  - `Del` 或悬停卡片头部点击垃圾桶按钮移除记录（删除后选中焦点保留在当前槽位，不跳回首张卡片）；
  - 标题栏内置「使用指南」交互动效弹窗、以及「设置与关于」弹窗。
- **版本更新检测与升级引导**：
  - 在「设置与关于」弹窗内点击「检查更新」，直接读取 GitHub Releases API；
  - 若有新版本，高亮展示新版本号、发布日期与更新日志，并提供「下载安装包 (.exe)」及前往 GitHub 发布页的快捷入口。
- **本地存储与隐私保护**：采用嵌入式 SQLite（`rusqlite bundled`，WAL 模式），数据完全保存在本地 `%LOCALAPPDATA%\WindowsPaste\history.db`，图片缓存存为本地 PNG（sha256 内容寻址），绝不上云。
- **单实例常驻**：命名互斥量保证只跑一个进程；再次启动会唤回已有面板，而不是开出第二个托盘图标和第二个数据库连接。
- **导入 / 导出**：面板底部工具栏支持单文件 JSON（图片内嵌 base64）一键导入导出，把 `.json` 拖进面板即可导入。以内容 hash 为幂等键——同一个包导入十次，结果与导入一次相同。

---

## 技术架构

| 模块 | 技术选型 | 说明 |
| :--- | :--- | :--- |
| **应用框架** | Tauri 2.12+ | 轻量级跨平台桌面框架，结合原生 Rust 与现代 Webview2 |
| **前端视图** | Vite 8 + SolidJS 2.0 (TypeScript) | 无虚拟 DOM，细粒度响应式更新，产物体积仅 113 KB |
| **文字识别 (OCR)** | Windows WinRT `Windows.Media.Ocr` | 系统底层免安装离线 OCR 引擎，0 外部依赖，后台静默索引 |
| **剪贴板监听** | Win32 `AddClipboardFormatListener` | 顶层隐藏窗口事件驱动，零 CPU 空转轮询 |
| **全局快捷键** | Win32 `RegisterHotKey` | 独立工作线程监听 `Ctrl + Shift + V` (`WM_HOTKEY`) |
| **模拟粘贴** | Win32 `SendInput` | 恢复前台焦点窗口并回放 `Ctrl + V` 击键 |
| **本地存储** | SQLite 3 (via `rusqlite` bundled) | 静态编译进本地二进制，支持容量自动修剪与模糊索引 |
| **托盘常驻** | Win32 `Shell_NotifyIconW` | 原生系统托盘常驻与右键菜单 |

---

## 项目结构

```
├── index.html                 # Webview 根挂载点
├── package.json               # 前端依赖配置
├── vite.config.ts             # Vite + Solid 插件配置
├── tsconfig.json              # TypeScript 编译配置
├── src/                       # 前端工程 (基于 SolidJS 2.0)
│   ├── main.tsx               # 前端入口 (挂载 ErrorBoundary 全局兜底)
│   ├── App.tsx                # 主壳层、全局键盘事件、导入确认与 toast
│   ├── bridge.ts              # Tauri invoke 与 listen 桥接层
│   ├── updater.ts             # GitHub Releases 版本检测与 SemVer 解析
│   ├── time.ts                # 卡片头部的相对时间格式化
│   ├── tagColors.ts           # 分类标签自定义颜色调色盘与持久化
│   ├── ocr.ts                 # OCR 文本结构解析与格式约定
│   ├── solid-utils.ts         # Solid 2 生命周期与事件辅助工具
│   ├── assets.ts              # blob 文件名 → asset 协议 URL
│   ├── styles.css             # Paste.app 样式令牌、暗色主题与核心样式
│   ├── types.ts               # 剪贴板条目与前后端协议类型定义
│   └── components/            # UI 组件库
│       ├── TitleBar.tsx       # 无边框标题栏与窗口控制
│       ├── TagFilterBar.tsx   # YouTube 风格分类标签横向滚动胶囊栏
│       ├── CardGrid.tsx       # 虚拟滚动与拖拽排序卡片网格
│       ├── ClipCard.tsx       # 单张卡片组件 (类型色带与正文)
│       ├── ContextMenu.tsx    # 右键上下文菜单与分类/OCR 菜单
│       ├── ConfirmDialog.tsx  # 导入/清空/删除分类确认对话框
│       ├── SettingsDialog.tsx # 设置与关于弹窗 (版本更新/存储目录定位)
│       ├── OnboardingGuide.tsx# 新手功能指引与操作动效演示
│       ├── OcrTextViewer.tsx  # 图片识别文字只读查看窗
│       ├── ErrorBoundary.tsx  # 渲染崩溃兜底与恢复
│       ├── Toast.tsx          # 轻量结果回执提示
│       └── ToolBar.tsx        # 底部搜索框、数据导入导出与快捷键提示
└── src-tauri/                 # Rust 原生后端工程
    ├── Cargo.toml             # Rust 依赖声明
    ├── tauri.conf.json        # Tauri 窗口与安全策略配置
    ├── build.rs               # 构建配置
    ├── icons/                 # 应用图标 (ico, png)
    └── src/                   # Rust 源码
        ├── lib.rs             # Tauri 应用入口调度与生命周期管理
        ├── main.rs            # 可执行程序入口
        ├── commands.rs        # Tauri #[command] 核心命令实现
        ├── state.rs           # 全局状态与 DTO 序列化
        ├── clipboard.rs       # Win32 剪贴板监听与格式嗅探（含 DIB 越界校验）
        ├── db.rs              # SQLite 本地存储层与 blob 生命周期
        ├── exporter.rs        # 导入 / 导出（单文件 JSON + base64 图片）
        ├── hotkey.rs          # Win32 全局热键后台线程
        ├── ocr.rs             # Windows 原生 WinRT 本地免安装 OCR 引擎
        ├── paste.rs           # 原生格式回写与 SendInput 模拟粘贴
        ├── single_instance.rs # 命名互斥量单实例保护
        ├── tray.rs            # 原生系统托盘
        ├── logging.rs         # 极简本地运行日志服务
        ├── filedialog.rs      # Win32 原生模态打开文件对话框
        └── window_util.rs     # 窗口屏幕居中与 DWM 直角支持
```

---

## 构建与运行

### 1. 运行前端开发预览
```powershell
npm run dev
```

### 2. 运行桌面原生端（带测试数据）
```powershell
# 1. 编译前端静态产物
npm run build

# 2. 启动原生桌面应用（首次灌入 50 条测试数据并立即呼出展示）
cargo run --manifest-path src-tauri/Cargo.toml -- --seed 50 --show
```

### 3. 一键打包 NSIS 安装包 (推荐)
```powershell
npm run build
npx tauri build --bundles nsis
```
打包产物位于 `src-tauri\target\release\bundle\nsis\Windows Paste_0.2.0_x64-setup.exe`。

### 4. 构建单一发布可执行程序
```powershell
npm run build
cargo build --release --manifest-path src-tauri/Cargo.toml
```
编译产物位于 `src-tauri\target\release\windows-paste.exe`。直接双击运行或加入开机自启即可使用。

---

## 自动化测试

### 1. 一键端到端功能自检 (E2E Self-Test)
无需复杂测试框架，应用内置了自动化测试套件，一键全自动跑完数据库、搜索过滤、焦点导航、置顶删除、剪贴板格式写入、窗口属性、分类 Tag 设置过滤与任意列数下的跨行导航共 10 项核心功能体检：
```powershell
npm run test:selftest
# 或直接调用
cargo run --manifest-path src-tauri/Cargo.toml -- --selftest
```

> **自检与 `--seed` 都跑在临时目录里**（`%TEMP%\WindowsPaste-selftest-<pid>-<ts>`），不会碰 `%LOCALAPPDATA%\WindowsPaste` 的真实历史。需要指定数据目录时用 `--data-dir <path>`。

### 2. 后端单元测试
```powershell
npm run test
# 或直接调用
cargo test --manifest-path src-tauri/Cargo.toml
```

覆盖：去重与“重复项提到最前”、导入幂等、导入不覆盖本地分类、blob 释放与孤儿清理、`content` 截断、任意列数下的跨行导航、导出导入往返（含图片字节级还原）、OCR 内容前后缀剥离等 27 项单元测试。

---

## 运行日志

应用内置了生产环境轻量本地运行日志（文件大小自动上限 2MB 循环滚动）：
- **日志路径**：`%LOCALAPPDATA%\WindowsPaste\app.log`
- 记录内容：进程启动退出信息、孤立图片文件清理、窗口唤起/焦点抢占状态、慢操作预警、以及前端运行时未捕获错误堆栈（协助定位白屏等潜在前端问题）。
- 可在「设置与关于」弹窗中点击「打开存储目录」直接在文件资源管理器中找到日志文件。
