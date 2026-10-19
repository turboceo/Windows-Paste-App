//! Windows 任务栏系统托盘服务 (System Tray Icon)
//!
//! 基于原生 Win32 Shell_NotifyIconW 与 TrackPopupMenu 实现系统托盘常驻。
//! 支持左键单击/双击唤起主面板，右键弹出快捷菜单（呼出、清空历史、退出），
//! 完全不依赖外部第三方 crate，零额外打包体积开销。

use std::sync::{Arc, Mutex};
use std::thread;
use windows_sys::Win32::Foundation::{HWND, LPARAM, LRESULT, POINT, WPARAM};
use windows_sys::Win32::System::LibraryLoader::GetModuleHandleW;
use windows_sys::Win32::UI::Shell::{
    Shell_NotifyIconW, NOTIFYICONDATAW, NIF_ICON, NIF_MESSAGE, NIF_TIP, NIM_ADD, NIM_DELETE,
};
use windows_sys::Win32::UI::WindowsAndMessaging::{
    AppendMenuW, CreatePopupMenu, CreateWindowExW, DefWindowProcW, DestroyMenu, DestroyWindow,
    DispatchMessageW, GetCursorPos, GetMessageW, LoadIconW, PostQuitMessage, RegisterClassW,
    SetForegroundWindow, TrackPopupMenu, TranslateMessage, IDI_APPLICATION, MF_SEPARATOR,
    MF_STRING, MSG, TPM_BOTTOMALIGN, TPM_LEFTALIGN, TPM_RETURNCMD, TPM_RIGHTBUTTON, WM_APP,
    WM_DESTROY, WM_LBUTTONDBLCLK, WM_LBUTTONUP, WM_RBUTTONUP, WNDCLASSW,
};

use crate::clipboard::wide_str;

const WM_TRAYICON: u32 = WM_APP + 101;
const ID_TRAY_SHOW: u32 = 1001;
const ID_TRAY_CLEAR: u32 = 1002;
const ID_TRAY_EXIT: u32 = 1003;

type TrayCallback = Arc<dyn Fn() + Send + Sync + 'static>;

static TRAY_SHOW: Mutex<Option<TrayCallback>> = Mutex::new(None);
static TRAY_CLEAR: Mutex<Option<TrayCallback>> = Mutex::new(None);
static TRAY_EXIT: Mutex<Option<TrayCallback>> = Mutex::new(None);

pub struct TrayIcon;

impl TrayIcon {
    /// 启动托盘后台消息循环线程
    pub fn start<FShow, FClear, FExit>(
        on_show: FShow,
        on_clear: FClear,
        on_exit: FExit,
    ) where
        FShow: Fn() + Send + Sync + 'static,
        FClear: Fn() + Send + Sync + 'static,
        FExit: Fn() + Send + Sync + 'static,
    {
        if let Ok(mut l) = TRAY_SHOW.lock() { *l = Some(Arc::new(on_show)); }
        if let Ok(mut l) = TRAY_CLEAR.lock() { *l = Some(Arc::new(on_clear)); }
        if let Ok(mut l) = TRAY_EXIT.lock() { *l = Some(Arc::new(on_exit)); }

        thread::spawn(move || unsafe {
            let class_name = wide_str("WindowsPaste_TrayWindow");
            let hinstance = GetModuleHandleW(std::ptr::null());

            let wnd_class = WNDCLASSW {
                style: 0,
                lpfnWndProc: Some(tray_wnd_proc),
                cbClsExtra: 0,
                cbWndExtra: 0,
                hInstance: hinstance,
                hIcon: LoadIconW(std::ptr::null_mut(), IDI_APPLICATION),
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

            let icon = LoadIconW(std::ptr::null_mut(), IDI_APPLICATION);
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
                let m_clear = wide_str("清空历史记录 (&C)");
                let m_exit = wide_str("退出 Windows Paste (&X)");

                AppendMenuW(hmenu, MF_STRING, ID_TRAY_SHOW as usize, m_show.as_ptr());
                AppendMenuW(hmenu, MF_STRING, ID_TRAY_CLEAR as usize, m_clear.as_ptr());
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
                        if let Ok(lock) = TRAY_SHOW.lock() {
                            if let Some(ref cb) = *lock {
                                cb();
                            }
                        }
                    }
                    ID_TRAY_CLEAR => {
                        if let Ok(lock) = TRAY_CLEAR.lock() {
                            if let Some(ref cb) = *lock {
                                cb();
                            }
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
