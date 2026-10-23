//! Windows 任务栏系统托盘服务 (System Tray Icon)
//!
//! 基于原生 Win32 Shell_NotifyIconW 与 TrackPopupMenu 实现系统托盘常驻。
//! 支持左键单击/双击唤起主面板，右键弹出快捷菜单（呼出、清空历史、退出），
//! 完全不依赖外部第三方 crate，零额外打包体积开销。

use std::sync::{Arc, Mutex};
use std::thread;
use windows_sys::Win32::Foundation::{HINSTANCE, HWND, LPARAM, LRESULT, POINT, WPARAM};
use windows_sys::Win32::System::LibraryLoader::GetModuleHandleW;
use windows_sys::Win32::UI::Shell::{
    Shell_NotifyIconW, NOTIFYICONDATAW, NIF_ICON, NIF_MESSAGE, NIF_TIP, NIM_ADD, NIM_DELETE,
};
use windows_sys::Win32::UI::WindowsAndMessaging::{
    AppendMenuW, CreatePopupMenu, CreateWindowExW, DefWindowProcW, DestroyMenu, DestroyWindow,
    DispatchMessageW, GetCursorPos, GetMessageW, LoadIconW, PostQuitMessage, RegisterClassW,
    SetForegroundWindow, TrackPopupMenu, TranslateMessage, HICON, IDI_APPLICATION, MF_GRAYED,
    MF_SEPARATOR, MF_STRING, MSG, TPM_BOTTOMALIGN, TPM_LEFTALIGN, TPM_RETURNCMD, TPM_RIGHTBUTTON,
    WM_APP, WM_DESTROY, WM_LBUTTONDBLCLK, WM_LBUTTONUP, WM_RBUTTONUP, WNDCLASSW,
};

use crate::clipboard::wide_str;

const WM_TRAYICON: u32 = WM_APP + 101;
const ID_TRAY_SHOW: u32 = 1001;
const ID_TRAY_EXIT: u32 = 1003;

type TrayCallback = Arc<dyn Fn() + Send + Sync + 'static>;

static TRAY_SHOW: Mutex<Option<TrayCallback>> = Mutex::new(None);
static TRAY_EXIT: Mutex<Option<TrayCallback>> = Mutex::new(None);

/// 托盘菜单的动作集合。
///
/// 托盘只保留"唤出面板"和"退出"两件事——导入、导出、清空历史都是
/// destructive / 低频操作，放在界面里更合适：那里有确认对话框、能显示进度、
/// 也不会因为右键点错而误清一整年的记录。
pub struct TrayActions<FShow, FExit> {
    pub on_show: FShow,
    pub on_exit: FExit,
}

pub struct TrayIcon;

impl TrayIcon {
    /// 启动托盘后台消息循环线程
    pub fn start<FShow, FExit>(actions: TrayActions<FShow, FExit>)
    where
        FShow: Fn() + Send + Sync + 'static,
        FExit: Fn() + Send + Sync + 'static,
    {
        if let Ok(mut l) = TRAY_SHOW.lock() { *l = Some(Arc::new(actions.on_show)); }
        if let Ok(mut l) = TRAY_EXIT.lock() { *l = Some(Arc::new(actions.on_exit)); }

        thread::spawn(move || unsafe {
            let class_name = wide_str("WindowsPaste_TrayWindow");
            let hinstance = GetModuleHandleW(std::ptr::null());

            // 从 exe 内嵌资源中加载应用自定义图标（tauri 打包时写入 ICON 资源），
            // 不再回退到系统通用空白图标。失败时才降级到 IDI_APPLICATION。
            let app_icon = load_app_icon(hinstance);

            let wnd_class = WNDCLASSW {
                style: 0,
                lpfnWndProc: Some(tray_wnd_proc),
                cbClsExtra: 0,
                cbWndExtra: 0,
                hInstance: hinstance,
                hIcon: app_icon,
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

            let icon = app_icon;
            let mut tip = [0u16; 128];
            let tip_text = wide_str("Windows Paste (Ctrl+Shift+V)");
            let copy_len = tip_text.len().min(127);
            tip[..copy_len].copy_from_slice(&tip_text[..copy_len]);

            let mut nid: NOTIFYICONDATAW = std::mem::zeroed();
            nid.cbSize = std::mem::size_of::<NOTIFYICONDATAW>() as u32;
            nid.hWnd = hwnd;
            nid.uID = 1;
            nid.uFlags = NIF_MESSAGE | NIF_ICON | NIF_TIP;
            nid.uCallbackMessage = WM_TRAYICON;
            nid.hIcon = icon;
            nid.szTip = tip;

            Shell_NotifyIconW(NIM_ADD, &mut nid);

            let mut msg: MSG = std::mem::zeroed();
            while GetMessageW(&mut msg, std::ptr::null_mut(), 0, 0) > 0 {
                TranslateMessage(&msg);
                DispatchMessageW(&msg);
            }

            Shell_NotifyIconW(NIM_DELETE, &mut nid);
            DestroyWindow(hwnd);
        });
    }
}

/// 从磁盘 PNG 解码为 32x32 HICON（托盘专用，按当前 DPI 选取合适的尺寸）。
///
/// 这是在 Windows 托盘图标缓存失效时的可靠方案：每次进程启动都会重新从
/// PNG 文件解码并创建一个新的 HICON 句柄，因此 Shell 缓存里的旧图标无法影响它。
unsafe fn load_tray_icon_from_png() -> Option<HICON> {
    use windows_sys::Win32::Graphics::Gdi::{
        CreateBitmap, CreateDIBSection, DeleteObject, BITMAPINFO, BITMAPINFOHEADER, DIB_RGB_COLORS,
        HBITMAP,
    };
    use windows_sys::Win32::System::Memory::{GetProcessHeap, HeapAlloc, HeapFree, HEAP_ZERO_MEMORY};
    use windows_sys::Win32::UI::WindowsAndMessaging::{CreateIconIndirect, ICONINFO};

    // 1. 定位 PNG：优先 exe 同级目录，其次开发期的 src-tauri 目录
    let mut candidates: Vec<std::path::PathBuf> = Vec::new();
    if let Ok(exe) = std::env::current_exe() {
        if let Some(dir) = exe.parent() {
            candidates.push(dir.join("tray-icon.png"));
            candidates.push(dir.join("icons").join("icon.png"));
        }
    }
    candidates.push(std::path::PathBuf::from("src-tauri/tray-icon.png"));
    candidates.push(std::path::PathBuf::from("src-tauri/icons/icon.png"));

    let bytes = candidates.iter().find_map(|p| std::fs::read(p).ok())?;
    let img = image::load_from_memory(&bytes).ok()?.to_rgba8();
    let (w, h) = img.dimensions();
    if w == 0 || h == 0 {
        return None;
    }

    // 2. 先在堆上准备 BITMAPINFO + 像素数据，交给 CreateDIBSection 填充
    let header_len = std::mem::size_of::<BITMAPINFO>() + 3 * std::mem::size_of::<u32>();
    let pixel_len = (w * h * 4) as usize;

    let heap = GetProcessHeap();
    let info_mem = HeapAlloc(heap, HEAP_ZERO_MEMORY, header_len);
    if info_mem.is_null() {
        return None;
    }
    let info = info_mem as *mut BITMAPINFO;

    // BITMAPINFOHEADER
    (*info).bmiHeader.biSize = std::mem::size_of::<BITMAPINFOHEADER>() as u32;
    (*info).bmiHeader.biWidth = w as i32;
    (*info).bmiHeader.biHeight = -(h as i32); // 负数 = top-down，与 PNG 行序一致
    (*info).bmiHeader.biPlanes = 1;
    (*info).bmiHeader.biBitCount = 32;
    (*info).bmiHeader.biCompression = 0; // BI_RGB
    (*info).bmiHeader.biSizeImage = pixel_len as u32;

    let mut bits: *mut core::ffi::c_void = std::ptr::null_mut();
    let hbm_color: HBITMAP =
        CreateDIBSection(std::ptr::null_mut(), info, DIB_RGB_COLORS, &mut bits, std::ptr::null_mut(), 0);
    HeapFree(heap, 0, info_mem);

    if hbm_color.is_null() || bits.is_null() {
        if !hbm_color.is_null() {
            DeleteObject(hbm_color as *mut core::ffi::c_void);
        }
        return None;
    }

    // 3. 写入 BGRA 像素
    let dst = bits as *mut u8;
    for (i, px) in img.pixels().enumerate() {
        let idx = i * 4;
        *dst.add(idx) = px[2];     // B
        *dst.add(idx + 1) = px[1]; // G
        *dst.add(idx + 2) = px[0]; // R
        *dst.add(idx + 3) = px[3]; // A
    }

    // 4. 创建单色掩码位图（32bpp 带 alpha 时掩码实际不参与渲染）
    let hbm_mask = CreateBitmap(w as i32, h as i32, 1, 1, std::ptr::null());

    let icon_info = ICONINFO {
        fIcon: 1,
        xHotspot: 0,
        yHotspot: 0,
        hbmMask: hbm_mask,
        hbmColor: hbm_color,
    };

    let hicon = CreateIconIndirect(&icon_info);

    // 清理临时 GDI 对象（CreateIconIndirect 会复制位图数据）
    DeleteObject(hbm_color as *mut core::ffi::c_void);
    DeleteObject(hbm_mask as *mut core::ffi::c_void);

    if hicon.is_null() {
        None
    } else {
        Some(hicon)
    }
}

/// 从可执行文件资源中加载应用图标（IDI_APPLICATION 作为最后兜底）。
///
/// Tauri 构建时会把 `bundle.icon` 中的 ico 写入 exe 资源，因此这里按
/// 资源序号挑选最合适的一档，托盘与窗口都能拿到清晰的彩色图标。
unsafe fn load_app_icon(hinstance: HINSTANCE) -> HICON {
    use windows_sys::Win32::UI::WindowsAndMessaging::{
        LoadImageW, IMAGE_ICON, LR_DEFAULTSIZE, LR_SHARED,
    };

    // 0. 优先：直接从 PNG 解码（绕过 Shell 图标缓存，托盘显示最可靠）
    if let Some(hicon) = load_tray_icon_from_png() {
        return hicon;
    }

    // 1. 其次：按资源 ID 从 exe 内嵌资源加载
    for id in [1usize, 2] {
        let h = LoadImageW(
            hinstance,
            id as *const u16,
            IMAGE_ICON,
            0,
            0,
            LR_DEFAULTSIZE | LR_SHARED,
        ) as HICON;
        if !h.is_null() {
            return h;
        }
    }

    // 2. 兜底：系统默认应用图标
    LoadIconW(std::ptr::null_mut(), IDI_APPLICATION)
}

unsafe extern "system" fn tray_wnd_proc(
    hwnd: HWND,
    msg: u32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    match msg {
        WM_TRAYICON => {
            let event = lparam as u32;
            if event == WM_LBUTTONUP || event == WM_LBUTTONDBLCLK {
                if let Ok(lock) = TRAY_SHOW.lock() {
                    if let Some(ref cb) = *lock {
                        cb();
                    }
                }
            } else if event == WM_RBUTTONUP {
                let mut pt: POINT = std::mem::zeroed();
                GetCursorPos(&mut pt);

                let hmenu = CreatePopupMenu();
                let m_show = wide_str("唤起剪贴板 (&S)\tCtrl+Shift+V");
                let m_exit = wide_str("退出 Windows Paste (&X)");

                AppendMenuW(hmenu, MF_STRING, ID_TRAY_SHOW as usize, m_show.as_ptr());
                AppendMenuW(hmenu, MF_SEPARATOR, 0, std::ptr::null());
                AppendMenuW(hmenu, MF_STRING, ID_TRAY_EXIT as usize, m_exit.as_ptr());

                SetForegroundWindow(hwnd);
                let cmd = TrackPopupMenu(
                    hmenu,
                    TPM_RETURNCMD | TPM_RIGHTBUTTON | TPM_BOTTOMALIGN | TPM_LEFTALIGN,
                    pt.x,
                    pt.y,
                    0,
                    hwnd,
                    std::ptr::null(),
                ) as u32;

                DestroyMenu(hmenu);

                match cmd {
                    ID_TRAY_SHOW => {
                        crate::logging::log("[托盘] 点击唤起");
                        if let Ok(lock) = TRAY_SHOW.lock() {
                            if let Some(ref cb) = *lock {
                                cb();
                            } else {
                                crate::logging::log("[托盘] 唤起回调为空");
                            }
                        } else {
                            crate::logging::log("[托盘] 唤起回调锁失败");
                        }
                    }
                    ID_TRAY_EXIT => {
                        if let Ok(lock) = TRAY_EXIT.lock() {
                            if let Some(ref cb) = *lock {
                                cb();
                            }
                        }
                    }
                    _ => {}
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
