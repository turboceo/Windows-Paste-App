# Windows Paste 项目设计模式与最佳实践

## 项目概述

Windows Paste 是一个基于 Tauri 2.0 + SolidJS 2.0 构建的 Windows 剪贴板管理工具。它提供原生的 Windows 剪贴板监听、全局热键、本地 SQLite 历史记录存储，以及平滑的虚拟卡片网格界面。

### 核心特性

- **原生剪贴板监听**：使用 Win32 `AddClipboardFormatListener` API 监听剪贴板变化
- **全局热键**：`Ctrl+Shift+V` 唤出/隐藏面板
- **本地存储**：SQLite 数据库 + Blob 文件系统
- **原生 OCR**：使用 Windows 10/11 内置的 WinRT OCR API
- **虚拟列表**：仅渲染可见卡片，支持拖拽排序

---

## 核心架构模式

### 1. Tauri + SolidJS 前后端分离

**架构图**：
```
┌─────────────────────────────────────────────────────────────┐
│                        Tauri WebView                         │
│  ┌───────────────────────────────────────────────────────┐   │
│  │                  SolidJS 2.0 UI Layer                  │   │
│  │  • CardGrid (虚拟列表)                                  │   │
│  │  • ClipCard (卡片组件)                                  │   │
│  │  • ContextMenu (右键菜单)                               │   │
│  └───────────────────────────────────────────────────────┘   │
│                         │  send(request)                     │
│                         ▼                                    │
│              ┌─────────────────────┐                         │
│              │      bridge.ts      │                         │
│              │  (Tauri IPC layer)  │                         │
│              └─────────────────────┘                         │
└─────────────────────────────────────────────────────────────┘
                         │
                         │ invoke('dispatch')
                         ▼
┌─────────────────────────────────────────────────────────────┐
│                       Tauri Backend (Rust)                   │
│  ┌───────────────────────────────────────────────────────┐   │
│  │        State Owner Worker Thread (单线程状态)          │   │
│  │  • 持有 AppState (无锁设计)                             │   │
│  │  • 串行处理所有 Command                                 │   │
│  └───────────────────────────────────────────────────────┘   │
│                         ▲                                     │
│    emit("host-push")  │                                     │
│                         │  Command::Dispatch                │
│    ┌──────────────────┴──────────────┐                      │
│    │ 剪贴板监听线程    热键处理线程   │                      │
│    └─────────────────────────────────┘                      │
└─────────────────────────────────────────────────────────────┘
```

**数据流模式**：
```
用户操作 → send(request) → Rust dispatch → worker线程处理
                                      ↓
                            emit("host-push") → 前端更新
```

### 2. 单线程所有权模式 (Single-Threaded Ownership)

**问题根源**：
旧版使用 `Mutex<AppState>`，导致：
- 剪贴板线程持锁执行 "写库 → 全表重查 → 序列化 → emit"
- emit 需要主线程参与
- 主线程等待同一把锁去唤出面板
- 双向互等导致永久卡死

**解决方案**：
AppState 完全归属 dedicated worker 线程，无任何锁。其他线程通过 command queue 异步请求。

```rust
// worker.rs - 串行处理所有状态变更
pub enum Command {
    Dispatch(HostRequest),        // 前端请求
    Captured(ClipboardItem),      // 剪贴板捕获
    Summoned { target_hwnd },     // 面板唤出
    Snapshot(Sender<Snapshot>),   // 状态快照(自检用)
}

// 状态句柄可以安全克隆，内部只是 mpsc Sender
#[derive(Clone)]
pub struct StateHandle {
    tx: Sender<Command>,
}
```

**优势**：
- 无锁设计，避免死锁
- 串行执行，状态一致性保证
- 热键/剪贴板线程不会被阻塞（`send()` 非阻塞）

---

## 关键设计模式

### 模式 1: 命令模式 (Command Pattern)

统一的请求入口，支持异步投递和同步快照查询：

```rust
pub enum Command {
    Dispatch(HostRequest),        // 异步请求
    Snapshot(Sender<Snapshot>),   // 同步查询
}
```

**使用方式**：
```rust
// 异步投递（非阻塞）
state.send(Command::Dispatch(request));

// 同步查询（用于自检）
let snapshot = state.snapshot();  // 阻塞等待
```

### 模式 2: 命令合并 (Debounce Pattern)

剪贴板连续触发时合并成一次捕获：

```rust
// clipboard.rs
static CLIP_SIGNAL: Mutex<Option<SyncSender<()>>> = Mutex::new(None);

// 工作线程中：
while rx.recv().is_ok() {
    thread::sleep(Duration::from_millis(20));  // 合并窗口
    while rx.try_recv().is_ok() {}             // 排空积压
    // 执行捕获
}
```

### 模式 3: 分离捕获与处理

```rust
// clipboard.rs - 剪贴板线程：只做捕获
let captured = ClipboardSniffer::new(&clip_blobs).capture();

// lib.rs - 投递到 owner 线程
state.send(Command::Captured(item));
```

**优势**：
- 捕获耗时操作不阻塞消息循环
- 减少 state-owner 线程持锁时间

### 模式 4: 按需加载缩略图

列表推送只包含 `has_image` 标记，前端对可见卡片单独请求：

```rust
// state.rs
pub fn from_item(item: &ClipboardItem, _blobs_dir: &Path) -> Self {
    Self {
        has_image: matches!(item.item_type, ClipboardItemType::Image)
            && item.blob_path.is_some(),
        // ... 不包含图片数据
    }
}

// ClipCard.tsx - 滚动到可见时才请求
createEffect(() => [item().hasImage, props.thumbnail()], ([hasImage, thumb]) => {
    if (hasImage && thumb === undefined) {
        props.onRequestThumbnail(item().id);
    }
});
```

**优势**：
- 列表推送体积大幅减小
- 图片仅在需要时解码
- 缩略图缓存避免重复解码

### 模式 5: 幂等导入

按 hash 查重，已存在时只合并元数据：

```rust
// db.rs
pub fn upsert_imported(&mut self, item: &mut ClipboardItem) -> Result<ImportOutcome> {
    if let Some(existing_id) = self.find_id_by_hash(&item.hash)? {
        // 合并规则：置顶取并集，标签只在本地为空时补上
        let next_pinned = current_pinned != 0 || item.pinned;
        let next_tag = if current_tag.is_some() { current_tag } else { item.tag.clone() };
        // ...
    }
    // 插入新条目
}
```

### 模式 6: 虚拟列表 + 乐观更新

```typescript
// CardGrid.tsx
const [dragItems, setDragItems] = createSignal<ClipboardItem[] | null>(null);

// 拖拽时立即更新本地顺序
setDragItems(next);
// 1.5秒后超时，等待后端推送验证
syncTimer = setTimeout(() => {
    dragList = null;
    setDragItems(null);
}, 1500);
```

### 模式 7: 短命线程处理耗时操作

```rust
// hotkey.rs - 热键处理
let pressed = Arc::clone(&on_pressed);
thread::spawn(move || (pressed)());  // 启动短命线程

// commands.rs - OCR 处理
std::thread::spawn(move || {
    let result = crate::ocr::recognize_file(&blob_path);
    // 结果通过 channel 发回 owner 线程
});
```

---

## Windows API 集成模式

### 剪贴板监听

```rust
// clipboard.rs - 独立窗口线程
unsafe extern "system" fn wnd_proc(hwnd: HWND, msg: u32, ...) -> LRESULT {
    WM_CLIPBOARDUPDATE => {
        // 极轻量：只投递信号
        tx.try_send(());
    }
}

// 工作线程 - 真正的捕获逻辑
thread::spawn(move || {
    while rx.recv().is_ok() {
        thread::sleep(Duration::from_millis(20));
        let captured = ClipboardSniffer::new(&blobs_dir).capture();
        // 投递给 owner 线程
    }
});
```

### 全局热键

```rust
// hotkey.rs - 独立线程消息循环
RegisterHotKey(hwnd, HOTKEY_ID, MOD_CONTROL | MOD_SHIFT | MOD_NOREPEAT, VK_V);

thread::spawn(move || {
    while GetMessageW(&mut msg, null_mut(), 0, 0) > 0 {
        if msg.message == WM_HOTKEY && msg.wParam == HOTKEY_ID as usize {
            // 启动短命线程处理，不阻塞消息循环
            thread::spawn(|| on_pressed());
        }
    }
});
```

### 单实例保护

```rust
// single_instance.rs - 命名互斥量 + 命名事件
const MUTEX_NAME: &str = "Local\\WindowsPaste.Instance";
const ACTIVATE_EVENT: &str = "Local\\WindowsPaste.Activate";

// 第一个进程持有互斥量
CreateMutexW(null(), 0, MUTEX_NAME);

// 第二个进程通知第一个唤回
SetEvent(CreateEventW(null(), 0, 0, ACTIVATE_EVENT));
```

---

## 数据库设计模式

### WAL 模式避免读写阻塞

```rust
// db.rs
let _ = conn.pragma_update(None, "journal_mode", "WAL");
let _ = conn.pragma_update(None, "synchronous", "NORMAL");
```

### 手动排序机制

```rust
// db.rs
CREATE TABLE ClipboardItems (
    ...
    SortOrder REAL NOT NULL DEFAULT 0
);

// 拖拽排序：重写整个分组的 SortOrder
pub fn reorder(&mut self, item_id: i64, before_id: Option<i64>) -> Result<()> {
    // 1. 查询本组所有 ID（排除被拖拽项）
    // 2. 插入到新位置
    // 3. 事务内按新顺序重写 SortOrder (0, 1, 2, ...)
    tx.execute("UPDATE ClipboardItems SET SortOrder = ?1 WHERE Id = ?2", ...);
}
```

**优势**：
- 避免浮点中点精度耗尽
- 每次拖拽后重排整个分组
- 置顶/未置顶分组独立排序

### 数据修剪策略

```rust
// db.rs
pub fn trim(&self) -> Result<()> {
    // 保留所有置顶项
    // 未置顶项超过 HISTORY_CAPACITY (500) 时删除最老的
    SELECT Id, BlobPath FROM ClipboardItems
    WHERE Pinned = 0
    ORDER BY CreatedAt DESC, Id DESC
    LIMIT -1 OFFSET ?1
}
```

---

## 前端性能优化

### 虚拟列表实现

```typescript
// CardGrid.tsx
const visible = createMemo(() => {
    const topRow = Math.max(0, Math.floor((top - PADDING_TOP) / rowStep));
    const bottomRow = Math.min(rows - 1, Math.floor((top + viewH - PADDING_TOP) / rowStep));
    const bufferedStartRow = Math.max(0, topRow - OVERSCAN_ROWS);
    const bufferedEndRow = Math.min(rows - 1, bottomRow + OVERSCAN_ROWS);
    
    const startIndex = bufferedStartRow * cols;
    const endIndex = Math.min(list.length, (bufferedEndRow + 1) * cols);
    return { items: list.slice(startIndex, endIndex), startIndex };
});
```

### 缩略图缓存

```rust
// state.rs
fn thumbnail_cache() -> &'static Mutex<HashMap<String, Option<String>>> {
    static CACHE: OnceLock<Mutex<HashMap<String, Option<String>>>> = OnceLock::new();
    CACHE.get_or_init(|| Mutex::new(HashMap::new()))
}

fn thumbnail_data_url(blobs_dir: &Path, file_name: &str) -> Option<String> {
    // 先查缓存
    if let Some(hit) = cache.get(file_name) { return hit.clone(); }
    // 缓存未命中才解码
    let computed = build_thumbnail(...);
    cache.insert(file_name.to_string(), computed.clone());
    computed
}
```

---

## 可复用的最佳实践

### 1. 状态管理：避免 Mutex 死锁

**不要**：
```rust
// ❌ 这会导致死锁
let state = AppState { db, items };
let state_mutex = Mutex::new(state);
```

**要**：
```rust
// ✅ 使用 State Owner Worker 模式
pub struct AppState { /* ... */ }  // 无 Mutex
pub enum Command { /* ... */ }
pub struct StateHandle { tx: Sender<Command> }
```

### 2. Windows 消息处理：窗口过程只做轻量操作

**不要**：
```rust
// ❌ 在 WndProc 中执行耗时操作
WM_CLIPBOARDUPDATE => {
    capture_clipboard();  // 阻塞消息循环！
}
```

**要**：
```rust
// ✅ 只投递信号
WM_CLIPBOARDUPDATE => {
    tx.try_send(());  // 立即返回
}
// 真正的捕获在独立工作线程
```

### 3. 热键/剪贴板线程：启动短命线程

**不要**：
```rust
// ❌ 直接在热键线程同步调用主线程
on_pressed() {  // 运行在热键线程
    app.run_on_main_thread(...).wait();  // 阻塞！
}
```

**要**：
```rust
// ✅ 启动短命线程
thread::spawn(move || {
    app.run_on_main_thread(...);
});
```

### 4. 数据持久化：WAL + 手动排序

**SQLite 配置**：
```rust
PRAGMA journal_mode = WAL;   // 读写不互阻
PRAGMA synchronous = NORMAL; // 性能与安全平衡
```

**排序字段**：
```sql
SortOrder REAL NOT NULL DEFAULT 0  -- 手动排序
```

---

## 项目结构参考

```
src-tauri/
├── src/
│   ├── lib.rs           # 应用入口，Tauri Builder 配置
│   ├── worker.rs        # State Owner Worker
│   ├── state.rs         # AppState 状态定义
│   ├── commands.rs      # 请求处理逻辑
│   ├── clipboard.rs     # 剪贴板监听与捕获
│   ├── hotkey.rs        # 全局热键处理
│   ├── db.rs            # SQLite 数据库层
│   ├── ocr.rs           # WinRT 本地 OCR
│   ├── paste.rs         # 粘贴服务（写剪贴板+注入按键）
│   ├── exporter.rs      # 导入/导出
│   ├── single_instance.rs  # 单实例保护
│   └── tray.rs          # 系统托盘
src/
├── main.tsx             # SolidJS 入口
├── app.tsx              # 主应用组件
├── bridge.ts            # 唯一的 Tauri 通信层
├── types.ts             # TypeScript 类型定义
├── components/
│   ├── CardGrid.tsx     # 虚拟卡片网格
│   ├── ClipCard.tsx     # 单个卡片
│   └── ...
```

---

## 测试模式

自检使用 `handle.snapshot()` 获取同步状态：

```rust
// selftest.rs
let snapshot = handle.snapshot();  // 阻塞等待命令处理完
assert_eq!(snapshot.total_count, 50);
```

**运行自检**：
```bash
cargo run -- --selftest
```

---

## 关键配置项

| 配置项 | 值 | 说明 |
|--------|-----|------|
| `HISTORY_CAPACITY` | 500 | 未置顶记录最大数量 |
| `THUMBNAIL_EDGE` | 480px | 缩略图最大边长 |
| `MAX_CONTENT_CHARS` | 8192 | 文本内容最大字符数 |
| `MAX_FILE_PATHS` | 500 | 文件路径列表最大行数 |
| `SEARCH_DEBOUNCE_MS` | 120 | 搜索防抖窗口 |
| `OVERSCAN_ROWS` | 2 | 虚拟列表 overscan 行数 |

---

## 性能基准

- **列表渲染**：仅挂载可见卡片
- **缩略图缓存**：单张图仅解码一次
- **命令处理**：异步非阻塞，微秒级返回
- **数据库**：WAL 模式下读写不互阻

---

## 常见问题

### Q: 为什么不用 Mutex？

A: 原设计使用 `Mutex<AppState>` 导致剪贴板线程和主线程互等，应用永久卡死。

### Q: 为什么剪贴板监听要独立线程？

A: 消息循环必须持续运行，耗时操作会阻塞后续通知。

### Q: 为什么列表推送不内嵌缩略图？

A: 每次推送都解码所有图片会导致严重卡顿，按需加载避免浪费。

### Q: 为什么拖拽排序要重写整个分组？

A: 避免浮点中点精度耗尽，保证排序精度。
