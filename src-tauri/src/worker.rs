//! 状态所有者线程 (State Owner Worker)
//!
//! 取代原来的 `Mutex<AppState>`。
//!
//! ## 为什么
//!
//! 旧设计里，剪贴板线程持锁做「写库 → 全表重查 → 序列化 → emit」，
//! 而 `emit` 需要主线程参与；主线程此时正等同一把锁去唤出面板。
//! 两个线程互等，应用就永久卡死——这就是"运行一段时间后唤不起来"的成因。
//!
//! ## 现在
//!
//! `AppState` 只被这条线程持有，**没有任何锁**。其他线程想做任何事，
//! 都往 channel 里投一条 `Command`，由这里串行执行：
//!
//! - 没有锁 → 不可能死锁
//! - 没有锁 → 不会中毒
//! - 串行执行 → 状态前后一致，不会读到写了一半的中间态
//!
//! ## 代价
//!
//! 命令变成异步排队。但这本来就成立：`dispatch` 返回 `Result<(), String>`，
//! 前端 `send()` 发完就不管，所有结果都靠 `host-push` 事件回传。

use std::sync::mpsc::{channel, Sender};
use std::thread;

use tauri::AppHandle;

use crate::commands;
use crate::state::AppState;

/// 投递给 owner 线程的一条消息。
pub enum Command {
    /// 前端发来的请求原样转发（绝大多数操作走这里）。
    Dispatch(crate::commands::HostRequest),
    /// 剪贴板新增了一条内容。
    ///
    /// 携带的是已经捕获好的条目——捕获过程（读剪贴板、解码 DIB）很耗时，
    /// 必须在投递之前、在剪贴板线程里完成，不能拖到 owner 线程上。
    Captured(crate::model::ClipboardItem),
    /// 面板被唤出：记住目标窗口、清空筛选、刷新列表。
    ///
    /// 单独设一个命令而不是复用 `Dispatch(Ready)`，是因为它还要携带
    /// 呼出瞬间的前台窗口句柄，而这个值只有主线程知道。
    Summoned { target_hwnd: isize },
    /// 取一份状态快照，用于自检。
    ///
    /// 这是唯一一个"要返回值"的命令：`dispatch` 是异步的，
    /// 自检必须能确定前一条命令**已经处理完**，否则断言会读到旧数据。
    /// 用一条同步 channel 把结果送回调用方即可，不影响 owner 线程的串行性。
    Snapshot(std::sync::mpsc::Sender<Snapshot>),
}

/// 状态快照。只包含自检需要的那几项，避免把整个 AppState 搬出去。
#[derive(Debug, Clone)]
pub struct Snapshot {
    pub total_count: usize,
    pub item_count: usize,
    pub selected_id: Option<i64>,
    pub items: Vec<crate::model::ClipboardItem>,
}

impl Snapshot {
    fn of(state: &AppState) -> Self {
        Self {
            total_count: state.total_count,
            item_count: state.items.len(),
            selected_id: state.selected_id,
            items: state.items.clone(),
        }
    }
}

/// owner 线程的句柄。可以随意 clone，内部只是一个 mpsc Sender。
#[derive(Clone)]
pub struct StateHandle {
    tx: Sender<Command>,
}

impl StateHandle {
    /// 投递一条命令。返回是否成功送达（owner 线程已退出时会失败）。
    ///
    /// 故意**不阻塞**：channel 是无界的，`send` 不会因为队列满而卡住调用方。
    /// 这一点很关键——调用方经常是热键线程或剪贴板线程，
    /// 它们绝不能被状态处理拖慢。
    pub fn send(&self, command: Command) {
        if self.tx.send(command).is_err() {
            crate::logging::log("[worker] 状态线程已退出，命令被丢弃");
        }
    }

    /// 阻塞等待一份状态快照。
    ///
    /// **只给自检用**。正常路径一律走 `send`，不要在生产代码里调用它——
    /// 那等于把异步模型又变回了同步等待。
    ///
    /// 因为 channel 是 FIFO，调用它会顺带保证：之前 `send` 出去的命令
    /// 都已经处理完毕。这正是自检需要的语义。
    pub fn snapshot(&self) -> Option<Snapshot> {
        let (reply_tx, reply_rx) = std::sync::mpsc::channel();
        self.send(Command::Snapshot(reply_tx));
        reply_rx
            .recv_timeout(std::time::Duration::from_secs(5))
            .ok()
    }
}

/// 启动 owner 线程，返回句柄。
pub fn spawn(app: AppHandle, state: AppState) -> StateHandle {
    let (tx, rx) = channel::<Command>();

    thread::Builder::new()
        .name("state-owner".to_string())
        .spawn(move || {
            // AppState 归这条线程所有，从始至终只有它一个持有者
            let mut state = state;
            crate::logging::log("[worker] 状态线程启动");

            while let Ok(command) = rx.recv() {
                let started = std::time::Instant::now();

                match command {
                    Command::Dispatch(request) => {
                        commands::handle(&app, &mut state, request);
                    }
                    Command::Captured(item) => {
                        commands::handle_captured(&app, &mut state, item);
                    }
                    Command::Summoned { target_hwnd } => {
                        commands::handle_summoned(&app, &mut state, target_hwnd);
                    }
                    Command::Snapshot(reply) => {
                        // 发送失败说明调用方已经不等了（超时或退出），忽略即可
                        let _ = reply.send(Snapshot::of(&state));
                    }
                }

                // 单条命令处理过久时留个痕迹。这是排查"卡顿"最直接的信号：
                // 如果这里频繁出现几百毫秒，说明某条路径在做重活。
                let elapsed = started.elapsed();
                if elapsed.as_millis() > 200 {
                    crate::logging::log_slow("处理命令", elapsed);
                }
            }

            crate::logging::log("[worker] 状态线程退出");
        })
        .expect("spawn state owner thread");

    StateHandle { tx }
}
