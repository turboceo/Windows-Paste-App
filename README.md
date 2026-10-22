# Windows Paste (Tauri 2 + Vite + React)

一个基于 **Tauri 2 + Vite + React 18 (TypeScript)** 的高性能 Windows 剪贴板历史管理器，深度还原 macOS 上著名应用 **[Paste](https://pasteapp.io/)** 的质感与交互体验。

随时按下全局快捷键 `Ctrl + Shift + V`，在屏幕正中央瞬间唤出剪贴板面板，进行高速搜索、预览与一键粘贴。

---

## 核心特性

- **现代轻量架构**：基于 Tauri 2 + 嵌入式 WebView2，利用原生 Chromium 渲染引擎与硬件加速，告别臃肿的 Electron，内存开销轻盈。
- **纯原生丝滑滚动**：基于 React 虚拟列表（Virtual Windowing）与原生 CSS 平滑滚动，完美解决海量卡片滚动的性能与卡顿瓶颈。
- **Paste.app 原生卡片美学**：
  - **彩色类型头部**：每张卡片顶部是一条按内容类型着色的色带（文本橙 / 链接绿 / 图像红 / 文件琥珀 / 颜色紫），左侧是类型名与相对时间（如「4 小时前」），右侧是类型图标，悬停时替换为置顶与删除按钮；
  - **按类型区分的信息展示**：
    - 纯文本：纯白正文，底部居中显示字符数；
    - 网页链接：显示完整 URL，底部标注主机域名（如 `github.com`）；
    - 屏幕截图与图像：缩略图铺满正文，底部浮一条半透明胶囊显示像素分辨率；
    - 文件列表：清晰展示包含图标的文件清单与总文件数；
    - 颜色代码：智能识别 `#RGB` / `#RRGGBB` / `rgb(...)`，正文呈现大色块色板，底部显示色值。
  - **堆叠卡片层次**：卡片上沿探出两层同色薄边，像一叠待取的历史记录；
  - **皇家蓝高对比聚焦外环**：选中时呈现 macOS 风格的 2px `#007aff` 亮蓝环与微浮升效果。
- **无边框与自适应视口**：
  - 动态自适应计算最佳列数，各行平滑均匀铺满；
  - 居中于当前鼠标光标所在显示器呼出；**失焦不会自动收起**，只有 Esc、再按一次热键、或选中记录粘贴后才会关闭；
  - Windows 原生 Direct3D / DWM 纯正直角与亚克力背景模糊。
- **生产力交互与快捷键**：
  - `Left` / `Right` / `Up` / `Down` 矩阵式键盘导航；
  - 拖拽卡片可手动排序（置顶与未置顶各自成组），移动时有滑动与吸附动画；
  - `Enter` 或双击卡片直接回写剪贴板并 `SendInput` 注入 `Ctrl + V` 到前台应用；
  - `Ctrl + P` 或悬停卡片头部点击图钉按钮置顶条目；
  - `Del` 或悬停卡片头部点击垃圾桶按钮移除记录（删除后选中停在原来的位置）。
- **本地存储与隐私保护**：采用嵌入式 SQLite（`rusqlite bundled`，WAL 模式），数据完全保存在本地 `%LOCALAPPDATA%\WindowsPaste\history.db`，图片缓存存为本地 PNG（sha256 内容寻址），绝不上云。
- **单实例常驻**：命名互斥量保证只跑一个进程；再次启动会唤回已有面板，而不是开出第二个托盘图标和第二个数据库连接。
- **导入 / 导出**：托盘菜单一键导出为单文件 JSON（图片内嵌 base64），把 `.json` 拖进面板即可导入。以内容 hash 为幂等键——同一个包导入十次，结果与导入一次相同。

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
│   ├── App.tsx                # 主壳层、全局键盘事件、导入确认与 toast
│   ├── bridge.ts              # Tauri invoke 与 listen 桥接层
│   ├── time.ts                # 卡片头部的相对时间格式化
│   ├── assets.ts              # blob 文件名 → asset 协议 URL
│   ├── styles.css             # Paste.app 样式令牌、暗色主题与核心样式
│   ├── types.ts               # 剪贴板条目与前后端协议类型定义
│   └── components/            # UI 组件
│       ├── TitleBar.tsx       # 无边框标题栏与窗口控制
│       ├── CardGrid.tsx       # 虚拟滚动卡片网格
│       ├── ClipCard.tsx       # 单张卡片组件
│       ├── ContextMenu.tsx    # 右键菜单与分类标签
│       ├── ConfirmDialog.tsx  # 导入前的确认对话框
│       ├── Toast.tsx          # 轻量结果提示
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
        ├── paste.rs           # 原生格式回写与 SendInput 模拟粘贴
        ├── single_instance.rs # 命名互斥量单实例保护
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
无需复杂测试框架，应用内置了自动化测试套件，一键全自动跑完数据库、搜索过滤、焦点导航、置顶删除、剪贴板格式写入、窗口属性、分类 Tag 设置过滤与任意列数下的跨行导航共 10 项核心功能体检：
```powershell
npm run test:selftest
# 或直接调用
cargo run --manifest-path src-tauri/Cargo.toml -- --selftest
```

> **自检与 `--seed` 都跑在临时目录里**（`%TEMP%\WindowsPaste-selftest-<pid>-<ts>`），
> 不会碰 `%LOCALAPPDATA%\WindowsPaste` 的真实历史。需要指定数据目录时用
> `--data-dir <path>`。

### 2. 后端单元测试
```powershell
npm run test
# 或直接调用
cargo test --manifest-path src-tauri/Cargo.toml
```

覆盖：去重与"重复项提到最前"、导入幂等、导入不覆盖本地分类、blob 释放与孤儿清理、
`content` 截断、任意列数下的跨行导航、导出导入往返（含图片字节级还原）。

---

## 分类标签

在卡片上右键可把记录加入分类，或新建一个分类。

**删除分类**：在顶部筛选栏的分类胶囊上**右键**，确认框会告知这个分类下有多少条记录。
删除只会把记录移回"未分类"，记录本身不会被删。

分类没有独立的表——它只是记录上的一个字符串字段，由所有记录汇总而来。
所以删掉最后一个引用该标签的记录后，这个分类会自动从列表里消失。

---

## 导入 / 导出

三个入口都在面板底部的工具栏上：**导入**、**导出**、**清空**。

导出会在 `文档` 下生成 `windows-paste-YYYYMMDD-HHMMSS.json`。这是一个**单文件**备份：
图片以 base64 内嵌，所以这一个文件就是全部数据，不需要额外拷贝 `blobs/` 目录。

导入点按钮选文件，或者直接把 `.json` 拖进面板。两种方式都会先弹一个预检对话框，
告诉你"共 N 条、M 条已存在"，确认后才写库。

清空需要二次确认，并会同时删除已缓存的图片文件。

> 托盘图标右键菜单只保留「唤起剪贴板」与「退出」——数据操作放在界面上，
> 因为它们是低频、需要确认、且不应该在右键误点的操作。

导入以内容 `hash` 为幂等键：同一个包导入十次，结果与导入一次完全相同。
已存在的条目只合并置顶与分类标签，**不覆盖**你本地已有的分类。
