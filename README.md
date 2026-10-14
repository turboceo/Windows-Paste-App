# Windows Paste (Rust + Slint 重构版)

一个基于 **Rust + Slint UI** 的高性能 Windows 剪贴板历史管理器，深度还原 macOS 上著名应用 **[Paste](https://pasteapp.io/)** 的质感与体验。
随时按下全局快捷键 `Ctrl + Shift + V`，在屏幕正中央瞬间唤出剪贴板面板，进行高速搜索、预览与一键粘贴。

---

## 核心特性

- **极致性能与轻量**：纯 Rust 静态编译单一二进制文件，体积仅 **12 MB**，内存常驻仅约 **20 MB ~ 30 MB**，绝不依赖 WebView2、Node.js、Chromium 或 .NET 运行时，彻底杜绝浏览器进程白屏与 GPU 崩溃。
- **瞬时呼出（< 20ms）**：原生毫秒级响应，定点居中呈现，绝无闪烁、位移拉伸与排版变形。
- **Paste.app 原生卡片美学**：
  - **智能格式识别与专属底色**：
    - 纯文本：柔和蓝白灰底板（`#F4F8FE`），右上角精确显示字符数；
    - 网页链接：清爽薄荷绿底板（`#EDFCF6`），右上角提取显示主机域名（如 `github.com`）；
    - 屏幕截图与图像：高清居中自适应相框，右上角显示像素分辨率；
    - 文件列表：温暖淡米黄底板（`#FFFEF2`），清晰展示包含图标的文件清单与总文件数；
    - 颜色代码：智能识别 `#RGB` / `#RRGGBB` / `rgb(...)`，中央呈现大色块色板。
  - **皇家蓝高对比聚焦外环**：选中时呈现 macOS 风格的 3px `#007aff` 亮蓝发光环与微浮升效果。
- **视口卡片优化算法**：根据可用宽度动态自适应计算最佳列数（大卡片 236px），各行平滑均匀铺满。
- **生产力标题栏与控制**：
  - 左侧展示剪贴板图标 + 应用名 + **总数目数小胶囊 Badge**（实时显示总条数或筛选比例，如 `50 项` / `2 / 50 项`）；
  - 中间支持鼠标直接拖拽窗口，支持**双击标题栏在最大化与居中还原之间快速切换**；
  - 右侧提供 Windows 风格的最小化、最大化/还原、关闭控制按钮。
- **交互与快捷键**：
  - `Left` / `Right` / `Up` / `Down` 矩阵式键盘导航；
  - `Enter` 或双击卡片直接回写剪贴板并 `SendInput` 注入 `Ctrl + V` 到前台应用；
  - `Ctrl + P` 或悬停点击图钉按钮置顶条目（置顶卡片常驻琥珀金徽章并排在最前）；
  - `Del` 或悬停点击垃圾桶图标移除记录；
  - `Esc` 一键关闭窗口。
- **本地存储与隐私保护**：采用嵌入式 SQLite（`rusqlite bundled`），数据完全保存在本地 `%LOCALAPPDATA%\WindowsPaste\history.db`，图片缓存存为本地 PNG，绝不上云。

---

## 技术架构

| 模块 | 技术选型 | 说明 |
| :--- | :--- | :--- |
| **编程语言** | Rust 2021 (rustc 1.95+) | 极致内存安全、零成本抽象、无 GC |
| **GUI 引擎** | Slint 1.9+ | 现代声明式轻量级原生 UI 引擎，DirectX/D3D 加速 |
| **剪贴板监听** | Win32 `AddClipboardFormatListener` | 顶层隐藏窗口事件驱动，零 CPU 空转轮询 |
| **全局快捷键** | Win32 `RegisterHotKey` | 独立工作线程监听 `Ctrl + Shift + V` (`WM_HOTKEY`) |
| **模拟粘贴** | Win32 `SendInput` | 恢复前台焦点窗口并回放 `Ctrl + V` |
| **本地存储** | SQLite 3 (via `rusqlite` bundled) | 静态编译进单一 exe，支持容量自动修剪与模糊索引 |
| **图像编解码** | `image` (png) | 剪贴板 DIB 位图转码为紧凑 PNG 缩略图 |

---

## 项目结构

```
ui/
  app.slint            Slint 声明式 UI 定义 (TitleBar, PasteCard, ToolBar, MainWindow)
src/
  main.rs              应用程序入口、Slint 状态机与 Win32 事件驱动调度
  model.rs             剪贴板数据结构 (ClipboardItem, ClipboardItemType)
  db.rs                本地 SQLite 数据层与容量修剪 (Database, AppPaths)
  clipboard.rs         Win32 剪贴板监听器与多格式解析器 (Monitor, Sniffer)
  hotkey.rs            Win32 全局热键后台线程管理 (HotkeyManager)
  paste.rs             模拟剪贴板回写与 SendInput Ctrl+V 注入 (PasteService)
Cargo.toml             Cargo 依赖与 Release 链接时优化 (LTO) 配置
build.rs               编译期生成 Slint UI 原生 Rust 结构代码
```

---

## 构建与运行

### 1. 运行调试版
```powershell
cargo run
```

### 2. 灌入测试种子数据并运行
```powershell
# 灌入 50 条包含文本、文件、代码、置顶项的 Paste 风格样本数据
cargo run -- --seed 50
```

### 3. 构建发布版（最优体积与速度）
```powershell
cargo build --release
```
编译产物位于 `target\release\windows-paste.exe`，为一个约 **12 MB** 的纯绿色单文件程序。
直接双击运行或加入开机自启即可使用。
