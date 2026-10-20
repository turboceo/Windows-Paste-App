//! 单实例保护 (Single Instance)
//!
//! 这个应用是常驻进程：它注册全局热键、占用托盘、独占一份 SQLite。
//! 没有单实例保护就会同时跑两个副本——两个托盘图标、双份剪贴板监听、
//! 两个连接写同一个库。注意 `Mutex<AppState>` 只管**进程内**并发，
//! 对跨进程毫无作用。
//!
//! 实现用命名互斥量 + 命名事件：
//! - 第一个进程持有 `Local\WindowsPaste.Instance` 互斥量；
//! - 第二个进程 `CreateMutexW` 后拿到 `ERROR_ALREADY_EXISTS`，说明已有实例；
//! - 它把 `Local\WindowsPaste.Activate` 事件置位后退出，第一个实例的
//!   等待线程收到信号就把面板唤到前台——语义正好等于"再按一次热键"。
//!
//! 用 `Local\` 前缀而不是 `Global\`：只需在同一登录会话内唯一，
//! 不要求跨用户会话，也不需要额外权限。

use std::thread;

use windows_sys::Win32::Foundation::{CloseHandle, GetLastError, HANDLE};
use windows_sys::Win32::System::Threading::{
    CreateEventW, CreateMutexW, SetEvent, WaitForSingleObject,
};

use crate::clipboard::wide_str;

const MUTEX_NAME: &str = "Local\\WindowsPaste.Instance";
const ACTIVATE_EVENT: &str = "Local\\WindowsPaste.Activate";

// 这几个常量在不同 windows-sys 版本里所在的模块不完全一致，
// 直接写字面量比依赖导入路径更稳。
const ERROR_ALREADY_EXISTS: u32 = 183;
const WAIT_OBJECT_0: u32 = 0;
const INFINITE: u32 = 0xFFFF_FFFF;

/// 本进程持有的互斥量。进程退出时由系统回收，不需要显式释放。
pub struct InstanceGuard {
    mutex: HANDLE,
}

// HANDLE 是裸指针，但互斥量句柄既可以在线程间传递、也可以被多个线程同时持有
// （Win32 内核对象本身就是跨线程的）。OnceLock<T> 要求 T: Sync，故两个都实现。
unsafe impl Send for InstanceGuard {}
unsafe impl Sync for InstanceGuard {}

impl InstanceGuard {
    /// 尝试成为唯一实例。
    ///
    /// - `Ok(Some(guard))`：抢到了，正常启动；
    /// - `Ok(None)`：已有实例在跑，调用方应当唤回它然后退出；
    /// - `Err(())`：创建失败（极罕见）。调用方应当照常启动——
    ///   单例机制本身出问题，不应该把整个应用挡住。
    pub fn acquire() -> Result<Option<Self>, ()> {
        let name = wide_str(MUTEX_NAME);

        unsafe {
            let mutex = CreateMutexW(std::ptr::null(), 0, name.as_ptr());
            if mutex.is_null() {
                return Err(());
            }

            // 创建成功但对象已存在时，GetLastError 返回 ERROR_ALREADY_EXISTS
            if GetLastError() == ERROR_ALREADY_EXISTS {
                CloseHandle(mutex);
                return Ok(None);
            }

            Ok(Some(Self { mutex }))
        }
    }

    /// 通知已有实例把面板唤到前台。返回是否成功送达。
    pub fn signal_existing() -> bool {
        let name = wide_str(ACTIVATE_EVENT);

        unsafe {
            // 事件由第一个实例创建；这里拿到句柄后置位
            let event = CreateEventW(std::ptr::null(), 0, 0, name.as_ptr());
            if event.is_null() {
                return false;
            }
            let ok = SetEvent(event) != 0;
            CloseHandle(event);
            ok
        }
    }

    /// 起一条后台线程等待"唤回"信号，收到就调 `on_activate`。
    ///
    /// 这条线程只做一件事：等待命名事件。它不持有任何应用状态，
    /// 回调里应当自己 `run_on_main_thread` 回到主线程。
    pub fn spawn_activation_listener<F>(on_activate: F)
    where
        F: Fn() + Send + 'static,
    {
        thread::spawn(move || unsafe {
            let name = wide_str(ACTIVATE_EVENT);
            let event = CreateEventW(std::ptr::null(), 0, 0, name.as_ptr());
            if event.is_null() {
                return;
            }

            loop {
                if WaitForSingleObject(event, INFINITE) != WAIT_OBJECT_0 {
                    // 句柄失效等异常情况，结束监听，不影响主流程
                    break;
                }
                on_activate();
            }

            CloseHandle(event);
        });
    }
}

impl Drop for InstanceGuard {
    fn drop(&mut self) {
        unsafe {
            if !self.mutex.is_null() {
                CloseHandle(self.mutex);
            }
        }
    }
}
