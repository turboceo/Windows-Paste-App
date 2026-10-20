use std::ffi::OsString;
use std::fs;
use std::os::windows::ffi::OsStringExt;
use std::path::Path;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Mutex;
use std::thread;
use std::time::Duration;

use chrono::Local;
use image::{ImageBuffer, Rgba};
use sha2::{Digest, Sha256};
use windows_sys::Win32::Foundation::{HWND, LPARAM, LRESULT, WPARAM};
use windows_sys::Win32::System::DataExchange::{
    AddClipboardFormatListener, CloseClipboard, GetClipboardData, IsClipboardFormatAvailable,
    OpenClipboard, RemoveClipboardFormatListener,
};
use windows_sys::Win32::System::LibraryLoader::GetModuleHandleW;
use windows_sys::Win32::System::Memory::{GlobalLock, GlobalSize, GlobalUnlock};
use windows_sys::Win32::UI::Shell::DragQueryFileW;
use windows_sys::Win32::UI::WindowsAndMessaging::{
    CreateWindowExW, DefWindowProcW, DestroyWindow, DispatchMessageW, GetMessageW,
    PostQuitMessage, RegisterClassW, TranslateMessage, MSG, WNDCLASSW,
    WM_CLIPBOARDUPDATE, WM_DESTROY,
};

use crate::model::{clamp_files, clamp_text, ClipboardItem, ClipboardItemType};

const CF_DIB: u32 = 8;
const CF_UNICODETEXT: u32 = 13;
const CF_HDROP: u32 = 15;
const CF_DIBV5: u32 = 17;

static CLIP_CALLBACK: Mutex<Option<Box<dyn Fn() + Send + 'static>>> = Mutex::new(None);
static CLIP_PAUSED: AtomicBool = AtomicBool::new(false);

pub struct ClipboardMonitor;

impl ClipboardMonitor {
    pub fn new() -> Self {
        CLIP_PAUSED.store(false, Ordering::SeqCst);
        Self
    }

    pub fn set_paused(&self, val: bool) {
        CLIP_PAUSED.store(val, Ordering::SeqCst);
    }

    /// Spawns a dedicated background thread running a Win32 message loop
    /// for AddClipboardFormatListener.
    pub fn start<F>(&self, on_change: F)
    where
        F: Fn() + Send + 'static,
    {
        if let Ok(mut lock) = CLIP_CALLBACK.lock() {
            *lock = Some(Box::new(on_change));
        }

        thread::spawn(move || unsafe {
            let class_name = wide_str("WindowsPaste_ClipMonitor");
            let hinstance = GetModuleHandleW(std::ptr::null());

            let wnd_class = WNDCLASSW {
                style: 0,
                lpfnWndProc: Some(wnd_proc),
                cbClsExtra: 0,
                cbWndExtra: 0,
                hInstance: hinstance,
                hIcon: std::ptr::null_mut(),
                hCursor: std::ptr::null_mut(),
                hbrBackground: std::ptr::null_mut(),
                lpszMenuName: std::ptr::null(),
                lpszClassName: class_name.as_ptr(),
            };

            RegisterClassW(&wnd_class);

            let hwnd = CreateWindowExW(
                0,
                class_name.as_ptr(),
                class_name.as_ptr(),
                0,
                0,
                0,
                0,
                0,
                std::ptr::null_mut(),
                std::ptr::null_mut(),
                hinstance,
                std::ptr::null(),
            );

            if hwnd.is_null() {
                return;
            }

            AddClipboardFormatListener(hwnd);

            let mut msg: MSG = std::mem::zeroed();
            while GetMessageW(&mut msg, std::ptr::null_mut(), 0, 0) > 0 {
                TranslateMessage(&msg);
                DispatchMessageW(&msg);
            }

            RemoveClipboardFormatListener(hwnd);
            DestroyWindow(hwnd);
        });
    }
}

unsafe extern "system" fn wnd_proc(
    hwnd: HWND,
    msg: u32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    match msg {
        WM_CLIPBOARDUPDATE => {
            if !CLIP_PAUSED.load(Ordering::SeqCst) {
                if let Ok(lock) = CLIP_CALLBACK.lock() {
                    if let Some(cb) = lock.as_ref() {
                        cb();
                    }
                }
            }
            0
        }
        WM_DESTROY => {
            PostQuitMessage(0);
            0
        }
        _ => DefWindowProcW(hwnd, msg, wparam, lparam),
    }
}

pub struct ClipboardSniffer {
    blobs_dir: std::path::PathBuf,
}

impl ClipboardSniffer {
    pub fn new(blobs_dir: &Path) -> Self {
        Self {
            blobs_dir: blobs_dir.to_path_buf(),
        }
    }

    /// Tries to capture current clipboard payload in order: Files > Image > Text
    pub fn capture(&self) -> Option<ClipboardItem> {
        if let Some(files) = self.try_capture_files() {
            return Some(files);
        }
        if let Some(img) = self.try_capture_image() {
            return Some(img);
        }
        self.try_capture_text()
    }

    fn with_clipboard<T, F>(&self, action: F) -> Option<T>
    where
        F: FnOnce() -> Option<T>,
    {
        for _ in 0..8 {
            unsafe {
                if OpenClipboard(std::ptr::null_mut()) != 0 {
                    let res = action();
                    CloseClipboard();
                    return res;
                }
            }
            thread::sleep(Duration::from_millis(25));
        }
        None
    }

    fn try_capture_text(&self) -> Option<ClipboardItem> {
        unsafe {
            if IsClipboardFormatAvailable(CF_UNICODETEXT) == 0 {
                return None;
            }
        }

        let text = self.with_clipboard(|| unsafe {
            let hmem = GetClipboardData(CF_UNICODETEXT);
            if hmem.is_null() {
                return None;
            }
            let ptr = GlobalLock(hmem) as *const u16;
            if ptr.is_null() {
                return None;
            }

            let mut len = 0;
            while *ptr.add(len) != 0 {
                len += 1;
            }
            let slice = std::slice::from_raw_parts(ptr, len);
            let s = String::from_utf16_lossy(slice);
            GlobalUnlock(hmem);
            Some(s)
        })?;

        let trimmed = text.trim();
        if trimmed.is_empty() {
            return None;
        }

        // hash 永远基于完整原文：截断只影响落库的 payload，不影响"是不是同一次复制"。
        let hash = sha256_str(&format!("text:{}", trimmed));
        Some(ClipboardItem {
            id: 0,
            item_type: ClipboardItemType::Text,
            content: clamp_text(trimmed),
            blob_path: None,
            hash,
            created_at: Local::now(),
            pinned: false,
            tag: None,
        })
    }

    fn try_capture_files(&self) -> Option<ClipboardItem> {
        unsafe {
            if IsClipboardFormatAvailable(CF_HDROP) == 0 {
                return None;
            }
        }

        let paths = self.with_clipboard(|| unsafe {
            let hdrop = GetClipboardData(CF_HDROP) as *mut std::ffi::c_void;
            if hdrop.is_null() {
                return None;
            }

            let count = DragQueryFileW(hdrop, 0xFFFFFFFF, std::ptr::null_mut(), 0);
            if count == 0 {
                return None;
            }

            let mut list = Vec::with_capacity(count as usize);
            let mut buf = vec![0u16; 1024];

            for i in 0..count {
                let len = DragQueryFileW(hdrop, i, buf.as_mut_ptr(), buf.len() as u32);
                if len > 0 {
                    let os_str = OsString::from_wide(&buf[..len as usize]);
                    if let Ok(s) = os_str.into_string() {
                        list.push(s);
                    }
                }
            }

            if list.is_empty() {
                None
            } else {
                Some(list)
            }
        })?;

        let content = paths.join("\n");
        let hash = sha256_str(&format!("files:{}", content));

        Some(ClipboardItem {
            id: 0,
            item_type: ClipboardItemType::Files,
            content: clamp_files(&content),
            blob_path: None,
            hash,
            created_at: Local::now(),
            pinned: false,
            tag: None,
        })
    }

    fn try_capture_image(&self) -> Option<ClipboardItem> {
        let format = unsafe {
            if IsClipboardFormatAvailable(CF_DIBV5) != 0 {
                CF_DIBV5
            } else if IsClipboardFormatAvailable(CF_DIB) != 0 {
                CF_DIB
            } else {
                return None;
            }
        };

        let raw_bytes = self.with_clipboard(|| unsafe {
            let hmem = GetClipboardData(format);
            if hmem.is_null() {
                return None;
            }
            let ptr = GlobalLock(hmem) as *const u8;
            if ptr.is_null() {
                return None;
            }

            // 剪贴板数据的头部是别的进程写的，不能直接相信。
            // 先用 GlobalSize 拿到真实分配长度，之后每一次读取都必须落在这个范围内。
            let alloc_size = GlobalSize(hmem) as usize;

            // 读取 BITMAPINFOHEADER 前先确认结构体本身在范围内
            if alloc_size < 20 {
                GlobalUnlock(hmem);
                return None;
            }

            let header_size = read_u32(ptr.add(0)) as usize;
            if header_size < 40 || header_size > alloc_size {
                GlobalUnlock(hmem);
                return None;
            }

            let width = read_i32(ptr.add(4));
            let height = read_i32(ptr.add(8));
            let bpp = read_u16(ptr.add(14));

            if width <= 0 || height == 0 || (bpp != 24 && bpp != 32) {
                GlobalUnlock(hmem);
                return None;
            }

            let abs_height = height.unsigned_abs() as usize;
            let abs_width = width as usize;
            let row_stride = (abs_width * (bpp as usize) + 31) / 32 * 4;

            // 关键校验：声明尺寸算出来的总长度必须真的装得下。
            // 否则一个撒谎的头部就会让下面的 from_raw_parts 读到分配区之外。
            let total_pixels_size = match row_stride.checked_mul(abs_height) {
                Some(v) => v,
                None => {
                    GlobalUnlock(hmem);
                    return None;
                }
            };
            let needed = match header_size.checked_add(total_pixels_size) {
                Some(v) => v,
                None => {
                    GlobalUnlock(hmem);
                    return None;
                }
            };
            if needed > alloc_size {
                GlobalUnlock(hmem);
                return None;
            }

            // 再挡一次离谱的尺寸，避免为一条恶意数据分配几个 GB
            const MAX_PIXELS: usize = 64 * 1024 * 1024; // 64 MPix ≈ 8192x8192
            if abs_width * abs_height > MAX_PIXELS {
                GlobalUnlock(hmem);
                return None;
            }

            let pixel_ptr = ptr.add(header_size);
            let pixel_slice = std::slice::from_raw_parts(pixel_ptr, total_pixels_size);

            let mut img_buf: ImageBuffer<Rgba<u8>, Vec<u8>> =
                ImageBuffer::new(abs_width as u32, abs_height as u32);

            let is_bottom_up = height > 0;

            for y in 0..abs_height {
                let src_y = if is_bottom_up {
                    abs_height - 1 - y
                } else {
                    y
                };
                let row_start = src_y * row_stride;

                for x in 0..abs_width {
                    let (b, g, r, a) = if bpp == 32 {
                        let idx = row_start + x * 4;
                        (
                            pixel_slice[idx],
                            pixel_slice[idx + 1],
                            pixel_slice[idx + 2],
                            pixel_slice[idx + 3],
                        )
                    } else {
                        let idx = row_start + x * 3;
                        (pixel_slice[idx], pixel_slice[idx + 1], pixel_slice[idx + 2], 255)
                    };

                    img_buf.put_pixel(x as u32, y as u32, Rgba([r, g, b, if bpp == 32 && a == 0 { 255 } else { a }]));
                }
            }

            GlobalUnlock(hmem);

            let mut png_bytes = Vec::new();
            let mut cursor = std::io::Cursor::new(&mut png_bytes);
            img_buf
                .write_to(&mut cursor, image::ImageFormat::Png)
                .ok()?;

            Some((png_bytes, abs_width, abs_height))
        })?;

        let (bytes, width, height) = raw_bytes;
        let hash = sha256_bytes(&bytes);
        let file_name = format!("{}.png", hash);
        let file_path = self.blobs_dir.join(&file_name);

        if !file_path.exists() {
            let _ = fs::write(&file_path, &bytes);
        }

        Some(ClipboardItem {
            id: 0,
            item_type: ClipboardItemType::Image,
            content: format!("{} x {} image", width, height),
            blob_path: Some(file_name),
            hash,
            created_at: Local::now(),
            pinned: false,
            tag: None,
        })
    }
}

/// 从 DIB 头部按字段偏移读取，避免依赖结构体对齐。
///
/// 调用方必须先用 `GlobalSize` 校验过 `offset + size` 在分配范围内。
unsafe fn read_u32(ptr: *const u8) -> u32 {
    u32::from_le(std::ptr::read_unaligned(ptr as *const u32))
}

unsafe fn read_i32(ptr: *const u8) -> i32 {
    i32::from_le(std::ptr::read_unaligned(ptr as *const i32))
}

unsafe fn read_u16(ptr: *const u8) -> u16 {
    u16::from_le(std::ptr::read_unaligned(ptr as *const u16))
}

pub fn wide_str(s: &str) -> Vec<u16> {
    s.encode_utf16().chain(std::iter::once(0)).collect()
}

pub fn sha256_str(s: &str) -> String {
    sha256_bytes(s.as_bytes())
}

pub fn sha256_bytes(b: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(b);
    format!("{:x}", hasher.finalize())
}
