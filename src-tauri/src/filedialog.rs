//! 原生文件选择对话框 (Open File Dialog)
//!
//! 导入需要真实文件系统路径，而 `<input type=file>` 在 webview 里拿不到
//! 绝对路径（浏览器出于安全把 `File.path` 去掉了）。所以这里直接用
//! Win32 的 `GetOpenFileNameW`，与项目其余部分（托盘、热键）保持同一套写法，
//! 也不必为了一个对话框引入 Tauri 的 dialog 插件。
//!
//! 注意：`GetOpenFileNameW` 会阻塞调用线程，因此必须在后台线程跑，
//! 否则面板会卡住。

use std::mem::zeroed;
use std::path::PathBuf;

use windows_sys::Win32::UI::Controls::Dialogs::{
    GetOpenFileNameW, CommDlgExtendedError, OFN_EXPLORER, OFN_FILEMUSTEXIST, OFN_HIDEREADONLY,
    OFN_NOCHANGEDIR, OFN_PATHMUSTEXIST, OPENFILENAMEW,
};

use crate::clipboard::wide_str;

/// 文件名缓冲区长度。Windows 路径上限 260，这里留足余量。
const BUFFER_LEN: usize = 1024;

/// 弹出"打开文件"对话框，返回用户选中的绝对路径。
///
/// 用户取消时返回 `None`；出错时也返回 `None`——对一个可选的交互来说，
/// 区分"取消"和"失败"没有实际意义，界面上都是"什么都没发生"。
pub fn pick_open_file(title: &str, filter_label: &str, extension: &str) -> Option<PathBuf> {
    // 双重 null 终止的过滤器串：显示名\0*.json\0\0
    let filter = format!(
        "{}\0*.{}\0\0",
        filter_label,
        extension.trim_start_matches('.')
    );
    let filter_w: Vec<u16> = filter.encode_utf16().collect();
    let title_w = wide_str(title);

    let mut buffer = vec![0u16; BUFFER_LEN];

    unsafe {
        let mut ofn: OPENFILENAMEW = zeroed();
        ofn.lStructSize = std::mem::size_of::<OPENFILENAMEW>() as u32;
        ofn.lpstrFilter = filter_w.as_ptr();
        ofn.lpstrFile = buffer.as_mut_ptr();
        ofn.nMaxFile = BUFFER_LEN as u32;
        ofn.lpstrTitle = title_w.as_ptr();
        ofn.Flags = OFN_EXPLORER
            | OFN_FILEMUSTEXIST
            | OFN_PATHMUSTEXIST
            | OFN_HIDEREADONLY
            // 不改进程当前目录：否则之后所有相对路径都会指向用户刚浏览的文件夹
            | OFN_NOCHANGEDIR;

        let ok = GetOpenFileNameW(&mut ofn);
        if ok == 0 {
            // 0 既可能是"取消"，也可能是错误；两种情况都直接返回
            let _ = CommDlgExtendedError();
            return None;
        }
    }

    // 缓冲区里是 null 终止的 UTF-16
    let end = buffer.iter().position(|&c| c == 0).unwrap_or(buffer.len());
    if end == 0 {
        return None;
    }
    let path = String::from_utf16_lossy(&buffer[..end]);
    Some(PathBuf::from(path))
}
