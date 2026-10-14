//! 模拟粘贴与剪贴板回写服务 (Paste Service)
//!
//! 负责在用户选择某条剪贴板记录后：
//! 1. 将数据以原生格式（文本 CF_UNICODETEXT、图像 CF_DIB、文件 CF_HDROP）写回 Windows 系统剪贴板；
//! 2. 将焦点无缝切回呼出面板前正在活跃的前台应用程序；
//! 3. 通过 SendInput 注入合成的 Ctrl + V 按键消息，实现全自动平滑粘贴。

use std::fs;
use std::path::Path;
use std::thread;
use std::time::Duration;

use windows_sys::Win32::Foundation::{HWND, POINT};
use windows_sys::Win32::System::DataExchange::{
    CloseClipboard, EmptyClipboard, OpenClipboard, SetClipboardData,
};
use windows_sys::Win32::System::Memory::{GlobalAlloc, GlobalLock, GlobalUnlock, GMEM_MOVEABLE};
use windows_sys::Win32::UI::Input::KeyboardAndMouse::{
    SendInput, INPUT, INPUT_0, INPUT_KEYBOARD, KEYBDINPUT, KEYEVENTF_KEYUP,
    VK_CONTROL, VK_V,
};
use windows_sys::Win32::UI::WindowsAndMessaging::{GetForegroundWindow, SetForegroundWindow};

use crate::clipboard::wide_str;
use crate::model::{ClipboardItem, ClipboardItemType};

const CF_DIB: u32 = 8;
const CF_UNICODETEXT: u32 = 13;
const CF_HDROP: u32 = 15;

/// Windows 资源管理器文件拖放与复制句柄结构体
#[repr(C)]
struct DropFiles {
    /// 文件名列表在内存块中的起始偏移字节数（通常即结构体大小 20 字节）
    p_files: u32,
    /// 放置点坐标 (x, y)
    pt: POINT,
    /// 是否在非客户区
    f_nc: i32,
    /// 是否为宽字符 UTF-16 编码 (1 为 True)
    f_wide: i32,
}

pub struct PasteService;

impl PasteService {
    /// 获取当前正在处于激活状态的前台窗口句柄 (HWND)
    pub fn get_foreground_window() -> HWND {
        unsafe { GetForegroundWindow() }
    }

    /// 执行粘贴的核心入口：写剪贴板 -> 切回目标窗口 -> 注入 Ctrl+V
    pub fn paste_item(item: &ClipboardItem, target_window: HWND, blobs_dir: &Path) {
        // 1. 将条目按其真实类型（文本/图像/文件）写回系统剪贴板
        Self::write_to_clipboard(item, blobs_dir);

        // 2. 将焦点切回唤出面板前的宿主应用程序窗口
        if !target_window.is_null() {
            unsafe {
                SetForegroundWindow(target_window);
            }
        }

        // 3. 短暂延时 100ms，确保 Windows 窗口管理器平滑完成前台焦点切换
        thread::sleep(Duration::from_millis(100));

        // 4. 发送合成的 Ctrl + V 键盘敲击
        Self::send_ctrl_v();
    }

    /// 根据条目类型将数据装载进剪贴板
    fn write_to_clipboard(item: &ClipboardItem, blobs_dir: &Path) {
        match item.item_type {
            ClipboardItemType::Text => {
                Self::write_text(&item.content);
            }
            ClipboardItemType::Image => {
                // 如果存在本地磁盘缓存的 PNG 位图，则还原为原生 DIB 写入剪贴板
                if let Some(ref blob_name) = item.blob_path {
                    let image_path = blobs_dir.join(blob_name);
                    if image_path.exists() {
                        if Self::write_dib_image(&image_path) {
                            return;
                        }
                    }
                }
                // 兜底降级写文本预览
                Self::write_text(&item.content);
            }
            ClipboardItemType::Files => {
                // 写入 CF_HDROP 文件句柄，同时写入 CF_UNICODETEXT 文本路径
                Self::write_files_and_text(&item.content);
            }
        }
    }

    /// 将纯文本以 CF_UNICODETEXT 格式写入剪贴板
    fn write_text(text: &str) {
        let wide = wide_str(text);
        let byte_len = wide.len() * 2;

        for _ in 0..8 {
            unsafe {
                if OpenClipboard(std::ptr::null_mut()) != 0 {
                    EmptyClipboard();

                    let hmem = GlobalAlloc(GMEM_MOVEABLE, byte_len);
                    if !hmem.is_null() {
                        let ptr = GlobalLock(hmem) as *mut u16;
                        if !ptr.is_null() {
                            std::ptr::copy_nonoverlapping(wide.as_ptr(), ptr, wide.len());
                            GlobalUnlock(hmem);
                            SetClipboardData(CF_UNICODETEXT, hmem);
                        }
                    }

                    CloseClipboard();
                    break;
                }
            }
            thread::sleep(Duration::from_millis(25));
        }
    }

    /// 将图像文件还原为 Win32 原生 CF_DIB 内存块写入剪贴板
    fn write_dib_image(png_path: &Path) -> bool {
        let bytes = match fs::read(png_path) {
            Ok(b) => b,
            Err(_) => return false,
        };
        let dyn_img = match image::load_from_memory(&bytes) {
            Ok(img) => img,
            Err(_) => return false,
        };
        let rgba = dyn_img.to_rgba8();
        let (width, height) = rgba.dimensions();
        if width == 0 || height == 0 {
            return false;
        }

        let w = width as usize;
        let h = height as usize;
        let bpp = 32usize;
        let row_stride = (w * bpp + 31) / 32 * 4;
        let pixel_bytes_len = row_stride * h;
        let header_len = 40usize; // BITMAPINFOHEADER 结构体尺寸为 40 字节
        let total_size = header_len + pixel_bytes_len;

        for _ in 0..8 {
            unsafe {
                if OpenClipboard(std::ptr::null_mut()) != 0 {
                    EmptyClipboard();

                    let hmem = GlobalAlloc(GMEM_MOVEABLE, total_size);
                    if !hmem.is_null() {
                        let ptr = GlobalLock(hmem) as *mut u8;
                        if !ptr.is_null() {
                            // 1. 填充 BITMAPINFOHEADER 报头
                            *(ptr as *mut u32) = 40;                             // biSize
                            *(ptr.add(4) as *mut i32) = width as i32;            // biWidth
                            *(ptr.add(8) as *mut i32) = height as i32;           // biHeight (正数代表 bottom-up)
                            *(ptr.add(12) as *mut u16) = 1;                      // biPlanes
                            *(ptr.add(14) as *mut u16) = 32;                     // biBitCount
                            *(ptr.add(16) as *mut u32) = 0;                      // biCompression = BI_RGB
                            *(ptr.add(20) as *mut u32) = pixel_bytes_len as u32; // biSizeImage
                            *(ptr.add(24) as *mut i32) = 0;                      // biXPelsPerMeter
                            *(ptr.add(28) as *mut i32) = 0;                      // biYPelsPerMeter
                            *(ptr.add(32) as *mut u32) = 0;                      // biClrUsed
                            *(ptr.add(36) as *mut u32) = 0;                      // biClrImportant

                            // 2. 填充 DIB 像素 (自底向上 bottom-up，BGRA 字节排布)
                            let pixel_dst = ptr.add(header_len);
                            for y in 0..h {
                                let src_y = h - 1 - y; // 垂直翻转至标准 DIB 方向
                                let dst_row_start = y * row_stride;
                                for x in 0..w {
                                    let pixel = rgba.get_pixel(x as u32, src_y as u32);
                                    let r = pixel[0];
                                    let g = pixel[1];
                                    let b = pixel[2];
                                    let a = pixel[3];

                                    let dst_idx = dst_row_start + x * 4;
                                    *pixel_dst.add(dst_idx) = b;
                                    *pixel_dst.add(dst_idx + 1) = g;
                                    *pixel_dst.add(dst_idx + 2) = r;
                                    *pixel_dst.add(dst_idx + 3) = a;
                                }
                            }

                            GlobalUnlock(hmem);
                            SetClipboardData(CF_DIB, hmem);
                        }
                    }

                    CloseClipboard();
                    return true;
                }
            }
            thread::sleep(Duration::from_millis(25));
        }
        false
    }

    /// 将换行分隔的文件路径集合写入 CF_HDROP (支持资源管理器粘贴) 及 CF_UNICODETEXT (支持编辑器粘贴)
    fn write_files_and_text(content: &str) {
        let mut buffer_u16: Vec<u16> = Vec::new();
        for line in content.lines() {
            let trimmed = line.trim();
            if !trimmed.is_empty() {
                buffer_u16.extend(trimmed.encode_utf16());
                buffer_u16.push(0); // 单个路径的 null 终止符
            }
        }
        if buffer_u16.is_empty() {
            Self::write_text(content);
            return;
        }
        buffer_u16.push(0); // 整体列表的双 null 结尾

        let drop_header_size = std::mem::size_of::<DropFiles>();
        let total_bytes = drop_header_size + buffer_u16.len() * 2;

        for _ in 0..8 {
            unsafe {
                if OpenClipboard(std::ptr::null_mut()) != 0 {
                    EmptyClipboard();

                    // 1. 设置文件拖拽结构 CF_HDROP
                    let hmem_drop = GlobalAlloc(GMEM_MOVEABLE, total_bytes);
                    if !hmem_drop.is_null() {
                        let ptr = GlobalLock(hmem_drop) as *mut u8;
                        if !ptr.is_null() {
                            let df = DropFiles {
                                p_files: drop_header_size as u32,
                                pt: POINT { x: 0, y: 0 },
                                f_nc: 0,
                                f_wide: 1, // UTF-16
                            };
                            std::ptr::copy_nonoverlapping(
                                &df as *const _ as *const u8,
                                ptr,
                                drop_header_size,
                            );
                            std::ptr::copy_nonoverlapping(
                                buffer_u16.as_ptr() as *const u8,
                                ptr.add(drop_header_size),
                                buffer_u16.len() * 2,
                            );
                            GlobalUnlock(hmem_drop);
                            SetClipboardData(CF_HDROP, hmem_drop);
                        }
                    }

                    // 2. 同时追加写入 CF_UNICODETEXT 纯文本格式，保证文本编辑器中亦可贴出路径
                    let wide = wide_str(content);
                    let byte_len = wide.len() * 2;
                    let hmem_text = GlobalAlloc(GMEM_MOVEABLE, byte_len);
                    if !hmem_text.is_null() {
                        let ptr = GlobalLock(hmem_text) as *mut u16;
                        if !ptr.is_null() {
                            std::ptr::copy_nonoverlapping(wide.as_ptr(), ptr, wide.len());
                            GlobalUnlock(hmem_text);
                            SetClipboardData(CF_UNICODETEXT, hmem_text);
                        }
                    }

                    CloseClipboard();
                    break;
                }
            }
            thread::sleep(Duration::from_millis(25));
        }
    }

    /// 通过 Win32 SendInput 注入合成的 Ctrl + V 击键序列
    fn send_ctrl_v() {
        unsafe {
            let mut inputs = [
                key_input(VK_CONTROL, 0),
                key_input(VK_V, 0),
                key_input(VK_V, KEYEVENTF_KEYUP),
                key_input(VK_CONTROL, KEYEVENTF_KEYUP),
            ];

            SendInput(
                inputs.len() as u32,
                inputs.as_mut_ptr(),
                std::mem::size_of::<INPUT>() as i32,
            );
        }
    }
}

/// 快速构建标准键盘输入事件结构体
fn key_input(vk: u16, flags: u32) -> INPUT {
    INPUT {
        r#type: INPUT_KEYBOARD,
        Anonymous: INPUT_0 {
            ki: KEYBDINPUT {
                wVk: vk,
                wScan: 0,
                dwFlags: flags,
                time: 0,
                dwExtraInfo: 0,
            },
        },
    }
}
