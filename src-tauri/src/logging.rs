//! 极简文件日志。
//!
//! 为什么不用 `eprintln!`：release 构建是 GUI 程序，没有附加控制台，
//! stderr 会被直接丢弃。卡死这类问题必须留下能在事后查看的痕迹。
//!
//! 写到 `%LOCALAPPDATA%\WindowsPaste\app.log`，超过 2MB 就滚动一次。
//! 只在关键节点写，不做逐帧记录——日志本身不能成为负担。

use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::PathBuf;
use std::sync::{Mutex, OnceLock};

/// 单个日志文件的上限。超过就截断重写，不做多份归档——
/// 排查卡死通常只需要最近几分钟的内容。
const MAX_LOG_BYTES: u64 = 2 * 1024 * 1024;

static LOG_PATH: OnceLock<PathBuf> = OnceLock::new();
/// 串行化写入，避免多线程交错出半行
static WRITE_LOCK: Mutex<()> = Mutex::new(());

/// 初始化日志路径。在应用启动最早处调用一次。
pub fn init(paths: &crate::db::AppPaths) {
    let path = paths.root.join("app.log");
    let _ = LOG_PATH.set(path.clone());

    // 启动时若文件过大就先清空，避免无限增长
    if let Ok(meta) = fs::metadata(&path) {
        if meta.len() > MAX_LOG_BYTES {
            let _ = fs::remove_file(&path);
        }
    }

    log(&format!(
        "===== 启动 v{} pid={} =====",
        env!("CARGO_PKG_VERSION"),
        std::process::id()
    ));
}

/// 写一行日志。未初始化时静默丢弃（例如单元测试环境）。
pub fn log(message: &str) {
    let Some(path) = LOG_PATH.get() else {
        return;
    };
    let _guard = WRITE_LOCK.lock();
    if let Ok(mut file) = OpenOptions::new().create(true).append(true).open(path) {
        // 时间精确到毫秒：判断"卡住多久"要看这个
        let ts = chrono::Local::now().format("%H:%M:%S%.3f");
        let _ = writeln!(file, "[{}] {}", ts, message);
    }
}

/// 记录一段耗时操作。用于 `lock()` 这类容易被拖住的地方。
pub fn log_slow(label: &str, elapsed: std::time::Duration) {
    log(&format!("[慢] {} 耗时 {:.1}ms", label, elapsed.as_secs_f64() * 1000.0));
}

/// 返回日志文件路径，供界面上"打开日志"之类的入口使用。
pub fn path() -> Option<&'static PathBuf> {
    LOG_PATH.get()
}
