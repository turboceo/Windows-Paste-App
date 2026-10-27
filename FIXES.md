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

## 四、交互与性能（P1）| 项 | 修法 |
| :--- | :--- |
| 图片 base64 膨胀 | DTO 只给 blob **文件名**，前端 `convertFileSrc` 走 asset 协议直读；`blobsRoot` 由宿主下发、作为 prop 传入卡片 |
| 上下键列数 bug | 语义改为 `\|delta\| > 1` 即"按行走"，步长只由后端 `columns` 决定；前端发 `±2` 而非 `±4` |
| 重复项幽灵副本 | `add()` 命中任意历史 hash 时改为"触摸"——刷新 `CreatedAt` 把它提到最前，而不是忽略 |
| 搜索无防抖 / 输入法 | 120ms 防抖 + `compositionstart/end` 期间不查库；上屏后立即补发一次 |
| 热键静默失败 | `RegisterHotKey` 失败时推 `hotkeyUnavailable`，界面顶部常驻黄条（不是 3 秒消失的 toast） |
| 数据操作入口藏太深 | 托盘右键菜单里的导入/导出/清空撤掉，改为工具栏上的三个文字按钮；清空走二次确认对话框 |
| 暗色主题 | 只覆盖 `:root` 令牌 + 少量硬编码色，组件零改动 |
| 死代码 / 冗余依赖 | 删除 `directories`、tauri `tray-icon` feature；`get_most_recent_hash`、`get_most_recent_id` 一并移除 |

## 四之二、运行一段时间后唤不起来（假死）

现象：程序启动一段时间后，按 `Ctrl+Shift+V` 没有反应，像是卡死。

这是一条由四个因素叠出来的故障链，四个都修了：

| # | 问题 | 位置 | 修法 |
| :--- | :--- | :--- | :--- |
| 1 | **剪贴板回调跑在窗口过程里** | `clipboard.rs` | 回调原本在 `WndProc` 里同步执行，而它会持 `SharedState` 锁做全表重查与序列化。这期间热键线程要唤出面板就得抢同一把锁，于是按了没反应。现在 `WndProc` 只 `try_send` 一个信号立刻返回，真正的处理移到独立工作线程 |
| 2 | **锁中毒导致永久假死** | `commands.rs` | `lock()` 原来是 `expect("app state poisoned")`：任何一次持锁 panic 都会让这把 Mutex 永久中毒，之后每次唤出都 panic；而 release 是 `panic = "abort"`，进程不退出但功能全废。改成 `unwrap_or_else(|e| e.into_inner())` |
| 3 | **热键线程被回调拖住** | `hotkey.rs` | `on_pressed()` 原本在热键线程的消息循环里同步跑。一旦它卡住，消息循环停摆，后续 `WM_HOTKEY` 全部堆积，热键彻底失效。改为每次投递到短命线程，最坏只影响这一次 |
| 4 | **推送负载随历史增长** | `state.rs` / `commands.rs` | 每次全表推送都内嵌所有图片的 base64 缩略图，表越大越慢。现在列表只带 `hasImage: true`，缩略图由前端对可见卡片按需拉取（`LoadThumbnail`），按 id 缓存 |

另外三处配套修正：

- `show_panel()` 改为**先 `show()` 再刷数据**。原来先持锁 reload + 序列化，主线程可能被挡住，窗口根本来不及显示。
- `toggle_panel()` 的判定从"可见就隐藏"改成"**可见且在前台**才隐藏"。窗口可见但丢了焦点时，按热键应当重新唤出，而不是静默 hide。
- `show()` 之后的 150ms 内忽略失焦事件（`window_util::mark_shown` / `recently_shown_within`）。否则窗口还没真正拿到焦点，系统先报一次失焦，面板会闪一下就消失。

## 四之三、分类标签删除

入口：顶部筛选栏的分类胶囊上**右键**。确认框显示该分类下的记录数，删除后记录变为"未分类"。

实现上要理解一点：**标签不是一个实体**。库里没有 `Tags` 表，标签只是 `ClipboardItems.Tag`
这个字符串字段，`get_all_tags()` 靠 `SELECT DISTINCT` 汇总出来。所以"删除分类"就是
`UPDATE ... SET Tag = NULL WHERE Tag = ?`，条目一条不少。

配套处理：

| 项 | 说明 |
| :--- | :--- |
| 硬编码的 `PRESET_TAGS` 已移除 | 原来前端把 5 个预设标签无条件并进候选列表，删掉的标签下次打开菜单又出现，像是删除功能坏了。现在只列数据库里真实存在的标签 |
| `activeTag` 自动复位 | 删掉的正好是当前筛选中的标签时，把筛选条件清空，否则列表会按一个不存在的分类过滤而变空 |
| `tagCounts` 随推送下发 | `reload()` 一次查出所有分类的条目数（`GROUP BY Tag`，避免 N+1），确认框直接用它 |
| 不误删记录 | 有专门的测试断言"删分类后 `get_total_count` 不变" |

## 四之四、面板不再失焦自动隐藏

去掉了"窗口一失去焦点就 `hide()`"。

原因：这个行为会跟菜单类交互抢焦点。右键菜单、分类子菜单、导入确认框都可能让焦点
短暂离开面板，结果面板连同正在操作的菜单一起消失。用户点一下别处（比如切到别的窗口
复制内容）也会让面板消失，再按热键时常因焦点状态错乱而唤不出来。

**现在收起面板只有三条路径**：Esc、再按一次热键（或点托盘图标）、选中记录后自动粘贴。

连带清理：

| 项 | 说明 |
| :--- | :--- |
| `BlurGuard` 删除 | 它原本只用来判断"是否已经呼出过、能不能因失焦隐藏"，现在没有意义 |
| `mark_shown` / `recently_shown_within` 删除 | 这两个函数专为"刚 show 完的 150ms 内忽略失焦"而存在，随失焦隐藏一起去掉 |
| `toggle_panel` 判定简化 | 从"可见且在前台才隐藏"改为"可见就隐藏"。因为面板不再自动隐藏，用户按热键时窗口通常是失焦的，若还要求前台，热键就永远收不起来 |

## 四之五、拖动窗口掉帧

现象：拖动标题栏移动窗口时明显卡顿，像掉帧。

原因是**同一块像素被模糊了三遍**：

1. `tauri.conf.json` 的 `transparent: true` —— 整个窗口走分层窗口路径，逐像素合成；
2. `window_util::polish()` 里的 `apply_acrylic` —— DWM 对窗口背后的内容实时采样模糊；
3. `.panel` 上的 `backdrop-filter: blur(28px)` —— WebView 合成器**又**对同一块区域模糊一遍。

拖动时窗口背后的内容每帧都在变，两层模糊都得每帧重算，1080×600 的窗口下
足够把帧率拉到 20-30。

**修法**：去掉 CSS 的 `backdrop-filter`，毛玻璃保留 DWM 亚克力那一层。
视觉效果几乎无差别（亚克力本来就是毛玻璃），但模糊从"WebView 里额外一遍"
变成"DWM 原生一遍"。同时去掉 `.titlebar` / `.toolbar` 上的 `blur(10px)`——
它们模糊的是面板内部已有底色的区域，视觉收益接近零，却要每帧合成。

**没能做的优化**：本来想给 `.panel` 加 `transform: translateZ(0)` 提升为独立合成层，
但那样会让它成为"包含块"，内部的 `position: fixed` 元素（右键菜单、确认框、toast）
从相对视口定位变成相对面板定位，坐标全部错位。除非先把这些浮层改成 portal，否则不能加。

### 第二轮：CSS 不是主因

去掉 CSS 那层 `backdrop-filter` 后重新构建，拖动**依然卡**。说明真正的开销不在 WebView，
而在窗口合成这一侧。于是关掉两项：

| 项 | 为什么 |
| :--- | :--- |
| **DWM 亚克力**（`ACRYLIC_ENABLED = false`） | 它让 DWM 对窗口**背后**的屏幕内容实时采样并模糊，窗口每移动一个像素就要重新采样。这是 Windows 上拖窗口掉帧最常见的原因，中低端显卡和副屏上尤甚 |
| **窗口透明**（`transparent: false`） | 透明窗口走分层窗口（layered window）路径，每帧要做 alpha 混合，性能天生不如不透明窗口。而面板是 `--radius-panel: 0px` 且铺满整个窗口，透明**唯一**的作用就是让亚克力透出来——亚克力关了，透明也就没意义了 |

关掉后 `--panel-bg` 提到 0.99、`body` 补上实底，视觉上从"毛玻璃浮层"变成"纯色面板"，
但拖动的流畅度提升明显。

要恢复毛玻璃观感，把 `ACRYLIC_ENABLED` 改回 `true`、`tauri.conf.json` 的
`transparent` 改回 `true`、`--panel-bg` 调回 0.94 即可，三处要一起改。

## 四之六、托盘"唤起剪贴板"经常没反应

现象：右键托盘图标点"唤起剪贴板"，面板不出现；长时间不操作应用后尤其明显。

两个原因叠在一起：

**一、托盘复用了 `toggle_panel`。** 那是热键的语义（可见就收起）。
上一轮去掉"失焦自动隐藏"之后，面板几乎**一直处于可见状态**，
所以点"唤起"时它走的是 `hide()` 分支——用户看到的就是"点了没反应"。
长时间不操作时更容易触发，因为面板多半还留在屏幕上某个位置。

修法：托盘改用 `show_panel`。菜单项写的是"唤起"，语义就该永远是"叫到前面来"。

**二、Windows 前台锁（foreground lock）。** 即使调了 `show()`，
`SetForegroundWindow` 对后台进程通常**静默失败**——只有当前拥有前台的进程、
或刚收到用户输入的进程才能切前台。所以窗口虽然显示出来了，却压在别的窗口后面。

修法：新增 `window_util::force_foreground`，用经典三步绕过：
`AttachThreadInput` 挂到前台线程 → `SetForegroundWindow` → 立刻解挂。
顺带先 `ShowWindow(SW_RESTORE)`，因为最小化状态下光 `show()` 不会还原。

**顺带补上日志**。上一轮加的 `logging` 模块因为编辑中断，`init` 一直没被调用，
等于所有诊断代码都是死代码。现在在 `run()` 里初始化，并且每次唤起都记录
`visible / focused / minimized` 三个状态——下次再出现唤不起来，看日志就能
判断是"压根没 show"还是"show 了但没抢到前台"。

### 第二轮：日志文件根本不存在

用户反馈 `%LOCALAPPDATA%\WindowsPaste\app.log` 不存在。查明两个原因：

1. **日志路径跟随了数据目录**。`logging::init(&paths)` 用的是 `paths.root`，
   而 `--seed` / `--selftest` 会把数据目录指到 `%TEMP%`，日志就跟着跑过去了。
   现在路径**固定**为 `%LOCALAPPDATA%\WindowsPaste\app.log`，与数据目录解耦。
2. **运行的还是旧 exe**。`cargo test` 只编译测试目标，不更新可执行文件；
   改了 `logging` 却不重新 `cargo build`，自然看不到日志。这一条属于验证流程问题，
   不是代码问题，但值得记下来。

### 第三轮：`show_panel` 的短路逻辑

去掉了"已经可见且在前台就直接 `set_focus` 返回"这段短路。

它原本是为避免面板位置跳动，但代价太大：上一轮去掉失焦自动隐藏后，面板
**长期处于可见状态**，点托盘唤出会走这条短路，除了 `set_focus` 什么都不做。
如果窗口在另一台显示器上、或者被别的窗口盖住，用户完全看不到反应。

现在每次唤起都完整执行：`show` → `unminimize` → 按光标所在显示器重新居中
→ `force_foreground` 抢前台。重复居中的轻微跳动，远好于"点了没反应"。

顺序调整也重要：**先还原最小化再定位**，因为最小化状态下 `set_position` 不生效。

## 四之七、新建分类从右键菜单搬到标签栏

现象与动机：原来「新建分类标签」藏在卡片右键 → 添加到分类的子菜单里。
那里看不到任何现有分类，用户想建一个不重名的标签只能靠猜；而且子菜单
空间局促，输入框 + 整行调色盘挤在一起，输入到一半鼠标滑出 4px 间隙还会
被自动收起。

**入口改为标签栏末尾常驻的「+」按钮**：那里能同时看到全部现有分类，
重名校验因此才有意义；空间也宽裕得多。

| 项 | 说明 |
| :--- | :--- |
| 空标签的可见性 | 数据库没有 Tags 表——标签只是 `ClipboardItems.Tag` 字符串字段，由 `SELECT DISTINCT` 汇总。所以一个没人引用的标签在库里不存在、活不过重启。但点「+」的那一刻它必须**立刻**出现在标签栏和右键菜单里，否则像"点了没反应"。因此前端多一份 `pendingTags`，与后端标签合并展示；一旦有卡片用上它，后端推送就会接管，草稿由 effect 清理 |
| 自动归到选中卡片 | 用户建分类的意图九成是给正在看的这张卡归类。所以有选中卡片时顺手 `setTag` 过去，不必再右键一趟 |
| 删除分类要收草稿 | 删分类时若草稿里也躺着同名标签（建了还没分卡片），必须一起清掉，否则它会在标签栏里原地复活 |
| 子菜单空态提示 | 右键菜单现在只剩「选已有分类」，一个分类都没有时给出「点顶部标签栏的 + 新建」的引导，而不是一片空白 |
| 管理浮层与新建浮层合并 | 两种浮层共用一个 `menu` 信号，点外面关闭 / Esc 关闭只需一套监听，也不会出现两个浮层同时开着的中间态 |
| 新建面板定位 | 用「+」按钮的 `getBoundingClientRect` 做锚点，右对齐到按钮右缘、挂在正下方；不跟光标坐标，面板不会乱跑 |

**类型层面的坑**：浮层内容最初内联在 `<Show when={menu()} keyed>` 的回调里，
`state.kind` 的联合类型收窄失败，`state.tag` 报 `Property 'tag' does not exist on type 'true'`。
抽成独立组件 + 用三元分支替代 `<Show>` 后编译期就能分好联合。

**连带清理**：右键菜单里的输入态（`isCreatingTag` / `newTagInput` /
`selectedColor` / `inputEl` / `submitNewTag`）与「正在输入时不收起子菜单」
的特例一并移除；`.new-tag-form` 样式及深色覆盖删除；`.selected-color-preview` /
`.new-tag-confirm` 改由标签栏复用。

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
