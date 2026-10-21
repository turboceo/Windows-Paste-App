use std::sync::Arc;
use std::thread;
use windows_sys::Win32::Foundation::{HWND, LPARAM, LRESULT, WPARAM};
use windows_sys::Win32::UI::Input::KeyboardAndMouse::{
    RegisterHotKey, UnregisterHotKey, MOD_CONTROL, MOD_NOREPEAT, MOD_SHIFT,
    VK_V,
};
use windows_sys::Win32::UI::WindowsAndMessaging::{
    CreateWindowExW, DefWindowProcW, DestroyWindow, DispatchMessageW, GetMessageW,
    PostQuitMessage, RegisterClassW, TranslateMessage, MSG, WNDCLASSW, WM_DESTROY,
    WM_HOTKEY,
};

use crate::clipboard::wide_str;

const HOTKEY_ID: i32 = 0x5041; // "PA"

/// 界面上展示的热键文案，注册失败时一并回传。
pub const HOTKEY_LABEL: &str = "Ctrl + Shift + V";

pub struct HotkeyManager;

impl HotkeyManager {
    /// Spawns a dedicated thread listening for Ctrl+Shift+V.
    ///
    /// `on_failed` 在热键被别的程序占用时调用一次。
    /// Ctrl+Shift+V 恰好是 VS Code、Windows Terminal、Notion 都在用的组合，
    /// 撞车概率不低；以前这里直接 `return` 结束线程，用户按热键毫无反应，
    /// 只会以为软件坏了。
    pub fn start<F, E>(on_pressed: F, on_failed: E)
    where
        F: Fn() + Send + Sync + 'static,
        E: Fn(&'static str) + Send + 'static,
    {
        // 包一层 Arc：每次热键都要在独立线程里跑一次，需要可克隆的句柄
        let on_pressed: Arc<dyn Fn() + Send + Sync> = Arc::new(on_pressed);

        thread::spawn(move || unsafe {
            let class_name = wide_str("WindowsPaste_HotkeyWindow");

            let wnd_class = WNDCLASSW {
                style: 0,
                lpfnWndProc: Some(wnd_proc),
                cbClsExtra: 0,
                cbWndExtra: 0,
                hInstance: std::ptr::null_mut(),
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
                std::ptr::null_mut(),
                std::ptr::null(),
            );

            if hwnd.is_null() {
                return;
            }

            let registered = RegisterHotKey(
                hwnd,
                HOTKEY_ID,
                MOD_CONTROL | MOD_SHIFT | MOD_NOREPEAT,
                VK_V as u32,
            );

            if registered != 0 {
                let mut msg: MSG = std::mem::zeroed();
                while GetMessageW(&mut msg, std::ptr::null_mut(), 0, 0) > 0 {
                    if msg.message == WM_HOTKEY && msg.wParam == HOTKEY_ID as usize {
                        // 交给独立线程处理，消息循环立刻回到 GetMessageW。
                        //
                        // 关键：`on_pressed` 内部会 run_on_main_thread，而主线程
                        // 可能正在等锁（比如剪贴板线程持锁做全表序列化）。
                        // 若在这里同步调用，热键线程就被挂住，消息循环停摆，
                        // 之后所有 WM_HOTKEY 都堆在队列里 —— 热键彻底失效。
                        // 起一条短命线程，最坏情况也只是这一次唤出慢，不会累积。
                        let pressed = Arc::clone(&on_pressed);
                        thread::spawn(move || (pressed)());
                    }
                    TranslateMessage(&msg);
                    DispatchMessageW(&msg);
                }
                UnregisterHotKey(hwnd, HOTKEY_ID);
            } else {
                // 被占用：明确告诉上层，而不是安静地什么都不做
                on_failed(HOTKEY_LABEL);
            }

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
        WM_DESTROY => {
            PostQuitMessage(0);
            0
        }
        _ => DefWindowProcW(hwnd, msg, wparam, lparam),
    }
}
