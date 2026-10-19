# Windows Paste (Tauri 2 + Vite + React)

一个基于 **Tauri 2 + Vite + React 18 (TypeScript)** 的高性能 Windows 剪贴板历史管理器，深度还原 macOS 上著名应用 **[Paste](https://pasteapp.io/)** 的质感与交互体验。

随时按下全局快捷键 `Ctrl + Shift + V`，在屏幕正中央瞬间唤出剪贴板面板，进行高速搜索、预览与一键粘贴。

---

## 核心特性

- **现代轻量架构**：基于 Tauri 2 + 嵌入式 WebView2，利用原生 Chromium 渲染引擎与硬件加速，告别臃肿的 Electron，内存开销轻盈。
- **纯原生丝滑滚动**：基于 React 虚拟列表（Virtual Windowing）与原生 CSS 平滑滚动，完美解决海量卡片滚动的性能与卡顿瓶颈。
- **Paste.app 原生卡片美学**：
  - **智能格式识别与专属底色**：
    - 纯文本：柔和蓝白灰底板（`#F4F8FE`），右上角精确显示字符数；
    - 网页链接：清爽薄荷绿底板（`#EDFCF6`），右上角提取显示主机域名（如 `github.com`）；
    - 屏幕截图与图像：高清居中自适应相框，右上角显示像素分辨率；
    - 文件列表：温暖淡米黄底板（`#FFFEF2`），清晰展示包含图标的文件清单与总文件数；
    - 颜色代码：智能识别 `#RGB` / `#RRGGBB` / `rgb(...)`，中央呈现大色块色板。
  - **皇家蓝高对比聚焦外环**：选中时呈现 macOS 风格的 3px `#007aff` 亮蓝发光环与微浮升效果。
- **无边框与自适应视口**：
  - 动态自适应计算最佳列数，各行平滑均匀铺满；
  - 居中于当前鼠标光标所在显示器呼出，失焦自动收起（支持 Esc 一键关闭）；
  - Windows 原生 Direct3D / DWM 纯正直角与亚克力背景模糊。
- **生产力交互与快捷键**：
  - `Left` / `Right` / `Up` / `Down` 矩阵式键盘导航；
  - `Enter` 或双击卡片直接回写剪贴板并 `SendInput` 注入 `Ctrl + V` 到前台应用；
  - `Ctrl + P` 或悬停点击图钉按钮置顶条目；
  - `Del` 或悬停点击垃圾桶图标移除记录。
- **本地存储与隐私保护**：采用嵌入式 SQLite（`rusqlite bundled`），数据完全保存在本地 `%LOCALAPPDATA%\WindowsPaste\history.db`，图片缓存存为本地 PNG，绝不上云。

---

## 技术架构

| 模块 | 技术选型 | 说明 |
| :--- | :--- | :--- |
| **应用框架** | Tauri 2.12+ | 轻量级跨平台桌面框架，结合原生 Rust 与现代 Webview |
| **前端视图** | Vite + React 18 + TypeScript | 虚拟滚动卡片网格，响应式状态机，低耦合 bridge 通信 |
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
├── vite.config.ts             # Vite 构建打包配置
├── tsconfig.json              # TypeScript 编译配置
├── src/                       # React 前端工程
│   ├── main.tsx               # 前端入口
│   ├── App.tsx                # 主壳层与全局键盘事件处理
│   ├── bridge.ts              # Tauri invoke 与 listen 桥接层
│   ├── styles.css             # Paste.app 样式令牌与核心样式
│   ├── types.ts               # 剪贴板条目与前后端协议类型定义
│   └── components/            # UI 组件
│       ├── TitleBar.tsx       # 无边框标题栏与窗口控制
│       ├── CardGrid.tsx       # 虚拟滚动卡片网格
│       ├── ClipCard.tsx       # 单张卡片组件
│       └── ToolBar.tsx        # 底部搜索框与快捷键提示
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
        ├── clipboard.rs       # Win32 剪贴板监听与格式嗅探
        ├── db.rs              # SQLite 本地存储层
        ├── hotkey.rs          # Win32 全局热键后台线程
        ├── paste.rs           # 原生格式回写与 SendInput 模拟粘贴
        ├── tray.rs            # 原生系统托盘
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

### 3. 构建单一发布可执行程序
```powershell
npm run build
cargo build --release --manifest-path src-tauri/Cargo.toml
```
编译产物位于 `src-tauri\target\release\windows-paste.exe`。
直接双击运行或加入开机自启即可使用。

---

## 自动化测试

### 1. 一键端到端功能自检 (E2E Self-Test)
无需复杂测试框架，应用内置了自动化测试套件，一键全自动跑完数据库、搜索过滤、焦点导航、置顶删除、剪贴板格式写入、窗口属性与分类 Tag 设置过滤 9 项核心功能体检：
```powershell
npm run test:selftest
# 或直接调用
cargo run --manifest-path src-tauri/Cargo.toml -- --selftest
```

### 2. 后端单元测试
```powershell
npm run test
# 或直接调用
cargo test --manifest-path src-tauri/Cargo.toml
```
