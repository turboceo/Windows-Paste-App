# 缺陷修复记录

对应 `design-review.html` 第 03 节的问题清单。按 P0（止血）→ P1（体验）顺序修复。

> **编译状态**：第一轮提交经 `cargo test` 报出 5 个编译错误，已全部修复：
>
> | 错误 | 原因 | 修法 |
> | :--- | :--- | :--- |
> | `E0255` name `db` defined multiple times | `pub mod db;` 与 `use crate::db::{self, …}` 撞名 | 改成只 `use crate::db::AppPaths`，调用处用全路径 |
> | `E0432` 找不到 `CreateMutexW` / `CreateEventW` | windows-sys 按依赖裁剪：这两个函数的参数含 `*const SECURITY_ATTRIBUTES`，未启用 `Win32_Security` 就不生成 | `Cargo.toml` 补 `Win32_Security` feature |
> | `E0277` `*mut c_void` cannot be shared between threads | `OnceLock<T>` 要求 `T: Sync` | 给 `InstanceGuard` 补 `unsafe impl Sync` |
> | `E0308` expected `PathBuf`, found `&AppPaths` | 临时目录分支把 `AppPaths` 当路径传了 | 新增 `AppPaths::temporary_root()` 只返回路径 |
> | `E0502` cannot borrow `guard` as immutable | `&mut guard.db` 与 `&guard.paths` 同时借用 | 调用前先把 `blobs_dir` 克隆出来 |
>
> 前端 `tsc` 仍未经本地验证。

## 一、数据完整性（P0）

| 问题 | 位置 | 修法 |
| :--- | :--- | :--- |
| 自检会清空真实历史 | `lib.rs` `run()` | `--selftest` / `--seed` 改走 `AppPaths::temporary()`（`%TEMP%\WindowsPaste-<tag>-<pid>-<ts>`），跑完即弃；新增 `--data-dir <path>` 供显式指定 |
| 图片文件只删一半 | `db.rs` / `commands.rs` / `lib.rs` | `delete()` 与 `clear()` 都返回待删的 blob 名，三处调用点真的去删文件 |
| 历史遗留的孤立 blob | `db.rs` `sweep_orphan_blobs()` | 启动时扫一遍 `blobs/`，删掉没有任何记录引用的 PNG（自愈旧版本的泄漏） |
| DIB 解析信任外部数据 | `clipboard.rs` | 读取前用 `GlobalSize` 拿到真实分配长度，逐项校验 `header_size`、`row_stride × height`、64 MPix 上限；新增 `read_u32/i32/u16` 做按偏移的安全读取 |
| `content` 不截断 | `model.rs` `clamp_text/clamp_files` | 文本按 8192 字符截断（按字符边界，不切坏 UTF-8）；文件列表按整行截断（不留下半个路径）。`hash` 仍按原文计算，去重语义不变 |
| 截断对用户不可见 | `state.rs` / `ClipCard.tsx` | DTO 增加 `truncated`，卡片角标显示 `8192+ 字符` |

## 二、单实例保护（P0）

新增 `single_instance.rs`，不引入新 crate：

- 命名互斥量 `Local\WindowsPaste.Instance`，`ERROR_ALREADY_EXISTS` 即判定已有实例；
- 第二个进程置位命名事件 `Local\WindowsPaste.Activate` 后退出；
- 第一个进程的后台线程等到事件就 `show_panel()`（已可见时只 `set_focus`，不重复居中）。

SQLite 顺带切到 WAL + `synchronous=NORMAL`。

## 三、导入 / 导出（P1）

新增 `exporter.rs`：

- **导出**：单文件 JSON，图片以 base64 内嵌。单文件意味着"这一个文件就是全部数据"，
  不会出现"拷了 db 没拷 blobs 导致图片全裂"的情况。
- **导入**：以 `hash` 为幂等键。已存在时只合并置顶（取并集）与分类（仅当本地为空），
  **不覆盖**本地已有分类、不覆盖 `CreatedAt`。同一个包导入十次 == 导入一次。
- **预检**：`previewImport` 先返回"共 N 条 / M 条已存在"，前端弹确认框再写库。
- **入口**：全部在界面底部的工具栏上——**导入 / 导出 / 清空** 三个带文字的按钮。
  导入也可以把 `.json` 直接拖进窗口（Tauri drag-drop 能给真实路径）。
  托盘菜单只保留「唤起面板」与「退出」，不再承担数据操作。

## 四、交互与性能（P1）

| 项 | 修法 |
| :--- | :--- |
| 图片 base64 膨胀 | DTO 只给 blob **文件名**，前端 `convertFileSrc` 走 asset 协议直读；`blobsRoot` 由宿主下发、作为 prop 传入卡片 |
| 上下键列数 bug | 语义改为 `\|delta\| > 1` 即"按行走"，步长只由后端 `columns` 决定；前端发 `±2` 而非 `±4` |
| 重复项幽灵副本 | `add()` 命中任意历史 hash 时改为"触摸"——刷新 `CreatedAt` 把它提到最前，而不是忽略 |
| 搜索无防抖 / 输入法 | 120ms 防抖 + `compositionstart/end` 期间不查库；上屏后立即补发一次 |
| 热键静默失败 | `RegisterHotKey` 失败时推 `hotkeyUnavailable`，界面顶部常驻黄条（不是 3 秒消失的 toast） |
| 数据操作入口藏太深 | 托盘右键菜单里的导入/导出/清空撤掉，改为工具栏上的三个文字按钮；清空走二次确认对话框 |
| 暗色主题 | 只覆盖 `:root` 令牌 + 少量硬编码色，组件零改动 |
| 死代码 / 冗余依赖 | 删除 `directories`、tauri `tray-icon` feature；`get_most_recent_hash`、`get_most_recent_id` 一并移除 |

## 五、测试与文档

- `db.rs`：新增去重/提最前、导入幂等、导入不覆盖本地分类、blob 释放、孤儿清理共 5 个测试；
  原有 CRUD 测试的断言随 `AddOutcome` 语义更新。
- `exporter.rs`：往返幂等、元数据还原、图片字节级还原、拒绝非本应用文件。
- `commands.rs`：5 列 / 2 列 / 单格 / 越界夹紧的移动测试。
- `model.rs`：字符边界截断、整行截断。
- `selftest.rs`：第 6 项不再硬编码 49；第 7 项去掉"失败也算 PASS"的软断言（改为 10 次重试后仍失败即 FAIL）；
  新增第 10 项跨列数导航回归测试。
- `README.md`：更新特性、目录结构、测试说明，新增导入导出章节。
- `.cursor/plans/webview2_vue_popup.md`：标注作废（它描述的是 WPF+Vue 路线，与当前代码无关）。

## 六、验证步骤

```powershell
# 1. 后端类型与单元测试（最关键，本轮未经编译验证）
cargo test --manifest-path src-tauri/Cargo.toml

# 2. 前端类型检查
npm run build

# 3. 端到端自检（现在跑在临时目录，不会动你的真实历史）
cargo run --manifest-path src-tauri/Cargo.toml -- --selftest

# 4. 手动验证
#    - 双击两次 exe：应只有一个托盘图标，第二个进程立刻退出并唤回面板
#    - 复制一张截图 → 面板应显示图片（走 asset 协议）
#    - 最大化窗口到 5 列以上 → 上下键应整行移动
#    - 中文输入法打"代码" → 搜索不应中途闪动多次结果
#    - 托盘导出 → 检查生成的 json；把它拖回面板 → 应提示"0 条新增，N 条已存在"
#    - 打开系统深色模式 → 面板应跟随
```

## 七、已知未做

- `trim()` 的 `LIMIT -1 OFFSET capacity` 在置顶项很多时语义偏宽松（未置顶项可能略超上限），
  属既有设计，本轮未动。
- 卡片仍是 `<div> + onClick`，可访问性（role / tabindex / aria）未改。
- `CF_HTML` 富文本、粘贴变体、热键可配置仍在待办（对应报告 R11–R13）。
