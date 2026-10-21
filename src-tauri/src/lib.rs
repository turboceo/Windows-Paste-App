//! Windows Paste (Tauri 2 + React)
//!
//! Native backend: clipboard monitoring, global hotkey, tray, simulated paste,
//! local SQLite history. The visible UI is a React app rendered in the webview.

pub mod clipboard;
pub mod commands;
pub mod db;
pub mod exporter;
pub mod filedialog;
pub mod hotkey;
pub mod logging;
pub mod model;
pub mod paste;
pub mod selftest;
pub mod single_instance;
pub mod state;
pub mod tray;
pub mod window_util;
pub mod worker;

use tauri::{Manager, WindowEvent};

use crate::clipboard::{ClipboardMonitor, ClipboardSniffer};
use crate::commands::emit_push;
use crate::db::AppPaths;
use crate::single_instance::InstanceGuard;
use crate::state::AppState;

pub use crate::state::HISTORY_CAPACITY;

/// 单实例互斥量的持有者：活到进程退出。
static INSTANCE_GUARD: std::sync::OnceLock<InstanceGuard> = std::sync::OnceLock::new();

/// Builds and runs the Tauri application.
pub fn run() {
    // 命令行参数处理
    let args: Vec<String> = std::env::args().collect();
    let is_selftest = args.iter().any(|a| a == "--selftest");

    // 单实例保护。避免两个副本同时占托盘、双份监听、双写同一个 SQLite。
    // 自检不参与：它跑在临时目录、用独立数据库，被主实例挡住反而没法验证。
    if !is_selftest {
        match InstanceGuard::acquire() {
            Ok(Some(guard)) => {
                // 必须持有到进程结束，否则第二个实例就能抢进来。
                // 放进 OnceLock 而不是 drop，语义上就是"活到进程退出"。
                let _ = INSTANCE_GUARD.set(guard);
            }
            Ok(None) => {
                // 已经有实例在跑：让它把面板唤到前台，然后安静退出。
                // 用户点第二次图标，本来想要的就是那个面板，而不是第二个进程。
                InstanceGuard::signal_existing();
                return;
            }
            Err(()) => {
                // 单例机制本身出错不应该把应用挡住，继续启动。
            }
        }
    }

    // `--data-dir <path>` 让测试与演示可以指向别处，不必动用户的真实历史。
    let data_dir_override = args
        .iter()
        .position(|a| a == "--data-dir")
        .and_then(|index| args.get(index + 1))
        .map(std::path::PathBuf::from);

    // 自检与种子数据一律跑在临时目录里。
    //
    // 这两条路径以前会直接对 %LOCALAPPDATA%\WindowsPaste 调 db.clear()，
    // 跑一次 `npm run test:selftest` 就把用户全部历史换成 50 条假数据。
    // 这里只算出"用哪个根目录"，不再提前构造 AppPaths。
    let temp_root: Option<std::path::PathBuf> = if is_selftest {
        Some(AppPaths::temporary_root("selftest"))
    } else if args.iter().any(|a| a == "--seed") {
        Some(AppPaths::temporary_root("seed"))
    } else {
        None
    };

    let paths = match (data_dir_override, temp_root) {
        // 显式指定的目录优先级最高（方便复现问题、跑集成测试）
        (Some(dir), _) => AppPaths::at(dir),
        (None, Some(root)) => {
            println!("[windows-paste] 使用临时数据目录：{}", root.display());
            AppPaths::at(root)
        }
        (None, None) => AppPaths::get(),
    };

    if is_selftest {
        seed_sample_data(&paths, 50);
    } else if let Some(index) = args.iter().position(|a| a == "--seed") {
        let count = args
            .get(index + 1)
            .and_then(|v| v.parse::<usize>().ok())
            .unwrap_or(50);
        seed_sample_data(&paths, count.clamp(1, 1000));
    }

    // AppState 不再放进 Mutex，而是交给专属的 owner 线程独占。
    // 详见 worker.rs 顶部的说明：这是消除"运行一段时间后假死"的根本改动。
    //
    // blob 目录在构造状态前先取一份：剪贴板监听线程要用它做捕获，
    // 而那时候状态已经归 owner 线程所有，不能再去问它要。
    let initial_blobs_dir = paths.blobs.clone();
    // 还要留一份完整路径：启动时扫孤儿文件要用独立连接，不走 owner 线程
    let initial_paths = paths.clone();

    // 日志要尽早初始化：release 构建是 GUI 程序，没有控制台，
    // 所有 println! / eprintln! 都会被直接丢弃。没有日志就没有排查依据。
    logging::init(&initial_paths);

    let initial_state = AppState::new(paths);

    // Debug helper: `--show`、`--seed` 或 `--selftest` 立即打开面板。
    // `--seed` 现在写的是临时库，不自动显示就没有意义了。
    let show_on_start = args.iter().any(|a| a == "--show")
        || args.iter().any(|a| a == "--seed")
        || is_selftest;

    tauri::Builder::default()
        .invoke_handler(tauri::generate_handler![commands::dispatch])
        .setup(move |app| {
            let handle: tauri::AppHandle = app.handle().clone();

            // 启动 owner 线程，之后所有状态操作都通过它串行执行
            let state_handle = worker::spawn(handle.clone(), initial_state);
            app.manage(state_handle);

            // Native window polish: square corners, centered on the cursor monitor.
            if let Some(window) = app.get_webview_window("main") {
                window_util::polish(&window);
                // 显式设置窗口/任务栏图标（避免依赖资源缓存）
                window_util::apply_window_icon(&window);
                if show_on_start {
                    window_util::center_on_cursor(&window);
                    let _ = window.show();
                    let _ = window.set_focus();
                }
            }

            // Intercept the OS close so the panel hides to the tray instead of
            // tearing down the process (the app stays resident).
            if let Some(window) = app.get_webview_window("main") {
                let hide_handle = handle.clone();
                window.on_window_event(move |event| {
                    if let WindowEvent::CloseRequested { api, .. } = event {
                        api.prevent_close();
                        if let Some(w) = hide_handle.get_webview_window("main") {
                            let _ = w.hide();
                        }
                    }
                });
            }

            // 启动时自愈：清掉 blobs/ 里没有任何记录引用的孤立文件。
            // 历史版本在 delete / clear 之后没有删文件，这些是真实的磁盘泄漏。
            //
            // 这里开一个**独立的只读连接**来做扫描，不去打扰 owner 线程。
            // 启动阶段没必要为了清理几个文件而让状态线程排队。
            if let Ok(sweep_db) = crate::db::Database::open_with_blobs(
                &initial_paths.db,
                HISTORY_CAPACITY,
                initial_paths.blobs.clone(),
            ) {
                let removed = crate::db::sweep_orphan_blobs(&sweep_db, &initial_paths.blobs);
                if removed > 0 {
                    crate::logging::log(&format!("清理孤立图片文件 {} 个", removed));
                }
            }

            // Clipboard listener (WM_CLIPBOARDUPDATE on a dedicated Win32 thread).
            //
            // 这个闭包由 clipboard.rs 的**工作线程**调用（不是窗口过程）。
            //
            // 注意这里没有锁可用，也不需要：捕获过程（读剪贴板、解码 DIB、
            // 写 PNG）全部在本地完成，只依赖 blob 目录这一个路径。拿到结果后
            // 把条目投递给 owner 线程入库即可。
            //
            // blob 目录是启动时固定的，不再需要向状态要——省掉一次跨线程往返。
            let clip_handle = handle.clone();
            let clip_blobs = initial_blobs_dir.clone();
            ClipboardMonitor::new().start(move || {
                let captured = ClipboardSniffer::new(&clip_blobs).capture();
                let Some(item) = captured else {
                    return;
                };

                // 投递后立刻返回：hot 路径上不再有任何阻塞调用
                if let Some(state) = clip_handle.try_state::<worker::StateHandle>() {
                    state.send(worker::Command::Captured(item));
                }
            });

            // Global hotkey: Ctrl+Shift+V summons / dismisses the panel.
            let hotkey_handle = handle.clone();
            let hotkey_fail_handle = handle.clone();
            hotkey::HotkeyManager::start(
                move || {
                    let app = hotkey_handle.clone();
                    // Hop back onto the main thread before touching the window.
                    let _ = app.clone().run_on_main_thread(move || toggle_panel(&app));
                },
                move |label| {
                    let app = hotkey_fail_handle.clone();
                    let _ = app.clone().run_on_main_thread(move || {
                        // 热键被占用是最容易被误判成"软件坏了"的失败，
                        // 必须让用户看见，并给出可操作的下一步。
                        emit_push(
                            &app,
                            serde_json::json!({
                                "type": "hotkeyUnavailable",
                                "label": label,
                                "message": format!(
                                    "快捷键 {} 已被其它程序占用，可从托盘图标打开面板",
                                    label
                                ),
                            }),
                        );
                    });
                },
            );

            // 第二个实例启动时通过命名事件唤回面板。
            let activate_handle = handle.clone();
            InstanceGuard::spawn_activation_listener(move || {
                let app = activate_handle.clone();
                let _ = app.clone().run_on_main_thread(move || {
                    // 已经被唤到前台时不做"隐藏"处理，直接显示
                    show_panel(&app);
                });
            });

            // System tray: 只负责唤出面板与退出。
            // 导入 / 导出 / 清空历史是低频且需要确认的操作，入口放在界面里。
            //
            // 注意托盘用的是 `show_panel` 而**不是** `toggle_panel`。
            // 菜单项写的是"唤起剪贴板"，语义就该永远是"叫到前面来"。
            // 用 toggle 的话，面板恰好还可见时会被隐藏——用户看到的就是
            // "点了没反应"，而且这种情况在长时间不操作时特别容易出现。
            let tray_show = handle.clone();
            let tray_exit = handle.clone();
            tray::TrayIcon::start(tray::TrayActions {
                on_show: move || {
                    let app = tray_show.clone();
                    let _ = app.clone().run_on_main_thread(move || show_panel(&app));
                },
                on_exit: move || {
                    let app = tray_exit.clone();
                    let _ = app.run_on_main_thread(|| std::process::exit(0));
                },
            });

            // 自动化端到端测试流程触发
            if is_selftest {
                selftest::start(handle.clone());
            }

            Ok(())
        })
        // 面板不再监听窗口失焦事件。
        //
        // 之前这里是"一失去焦点就 hide()"，本意是像 Paste 那样点到别处就收起。
        // 实际用起来很糟：右键菜单、分类子菜单、导入确认框都可能让焦点短暂
        // 离开，面板连同正在操作的菜单一起消失；在别的窗口里复制东西再按热键，
        // 也常因焦点状态错乱而唤不出来。
        //
        // 现在只有三种方式收起面板：Esc、再按一次热键（或点托盘）、
        // 以及选中记录后自动粘贴。
        .run(tauri::generate_context!())
        .expect("error while running Windows Paste");
}

/// Summons or dismisses the panel. Used by the global hotkey and the tray.
///
/// 面板可见时，热键就是收起它——不看焦点。
///
/// 之前这里要求"可见**且**在前台"才隐藏，那是为了对冲"失焦自动隐藏"遗留的
/// 状态错乱。现在失焦不再自动隐藏，用户按热键时窗口通常正处于失焦状态
/// （他已经点到别处去了），若还要求前台，热键就永远收不起来，
/// 只能按 Esc。所以判定只看可见性。
pub fn toggle_panel(app: &tauri::AppHandle) {
    let Some(window) = app.get_webview_window("main") else {
        return;
    };

    if window.is_visible().unwrap_or(false) {
        let _ = window.hide();
        return;
    }

    show_panel(app);
}

/// 只负责"把面板显示出来"，不切换可见性。
///
/// 用于第二个实例唤回：用户明确想看到面板，此时不应因为"已经显示"而隐藏它。
pub fn show_panel(app: &tauri::AppHandle) {
    let Some(window) = app.get_webview_window("main") else {
        return;
    };

    // 已经可见**且**在前台时才只把焦点要回来，不重复刷新、不重新居中——
    // 否则每次双击图标面板都会"跳"一下。
    // 可见但丢了焦点的情况要走完整流程：重新居中、重新 show、抢前台。
    if window.is_visible().unwrap_or(false) && window.is_focused().unwrap_or(false) {
        let _ = window.set_focus();
        return;
    }

    // Remember the foreground window so the paste lands in the right place.
    let foreground = paste::PasteService::get_foreground_window();

    // 主线程上只做窗口操作，**不等任何状态**。
    //
    // 以前这里要拿锁做 reload + 全表序列化，而那段代码跑在主线程上，
    // 一旦别的线程正持着锁，窗口就会迟迟显示不出来——用户看到的就是
    // "按了热键没反应"。现在数据刷新改成投递一条命令，主线程立刻返回。
    window_util::center_on_cursor(&window);
    let _ = window.show();
    // 把窗口提到 Z 序最前并强制抢焦点。
    //
    // 单靠 `set_focus()` 不够：Windows 有前台锁（foreground lock），
    // 一个后台进程不能随便把窗口切到前台，`SetForegroundWindow` 会返回
    // false 而什么都不发生。这正是"托盘点了没反应、但窗口其实还在"的成因。
    window_util::force_foreground(&window);
    let _ = window.set_focus();

    // 记下呼出前的目标窗口 + 清空筛选 + 刷新列表，一并交给 owner 线程
    if let Some(state) = app.try_state::<worker::StateHandle>() {
        state.send(worker::Command::Summoned {
            target_hwnd: foreground as isize,
        });
    }
}

/// 按文件名批量删除 blob，尽力而为（文件已被删掉也无所谓）。
pub fn remove_blobs(blobs_dir: &std::path::Path, names: &[String]) {
    for name in names {
        let _ = std::fs::remove_file(blobs_dir.join(name));
    }
}

/// 生成一张 480x320 的示例 PNG（蓝紫渐变 + 对角条纹）。
///
/// 用代码画而不是打包一张图片：示例数据只在 `--seed` / `--selftest` 时用，
/// 没必要为了它往 exe 里塞资源。
fn make_sample_png() -> Vec<u8> {
    use image::{ImageBuffer, Rgba};

    let (w, h) = (480u32, 320u32);
    let mut img: ImageBuffer<Rgba<u8>, Vec<u8>> = ImageBuffer::new(w, h);

    for y in 0..h {
        for x in 0..w {
            // 蓝 → 紫 的对角渐变
            let t = ((x + y) as f32) / ((w + h) as f32);
            let stripe = if ((x / 24) + (y / 24)) % 2 == 0 { 12 } else { 0 };
            let r = (60.0 + 120.0 * t) as u8 + stripe;
            let g = (90.0 + 60.0 * t) as u8;
            let b = (200.0 - 40.0 * t) as u8;
            img.put_pixel(x, y, Rgba([r, g, b, 255]));
        }
    }

    let mut out = Vec::new();
    img.write_to(&mut std::io::Cursor::new(&mut out), image::ImageFormat::Png)
        .expect("encode sample png");
    out
}

/// Fills the history with sample rows so the UI can be inspected without
/// copying real data. Used by `--seed [n]` (default 50).
pub fn seed_sample_data(paths: &AppPaths, count: usize) {
    use crate::model::{ClipboardItem, ClipboardItemType};
    use chrono::Local;

    let mut db = match crate::db::Database::open_with_blobs(
        &paths.db,
        state::HISTORY_CAPACITY,
        paths.blobs.clone(),
    ) {
        Ok(db) => db,
        Err(_) => return,
    };
    let _ = db.clear();

    let samples = [
        "https://github.com/tauri-apps/tauri",
        "npm run tauri dev",
        "SELECT Id, Type, Content FROM ClipboardItems ORDER BY CreatedAt DESC",
        "The quick brown fox jumps over the lazy dog",
        "好记性不如烂笔头",
        "git rebase --interactive HEAD~3",
        "#3b82f6",
        "C:\\Users\\Pyzl\\Documents\\report.pdf",
    ];

    // 种子里放真实的 PNG：否则用 --seed 演示时永远看不到图片卡片，
    // 缩略图相关问题也就永远无法在自检里被发现。
    let sample_png = make_sample_png();

    let base = Local::now() - chrono::Duration::minutes((count * 3) as i64);

    for i in 0..count {
        let sample = samples[i % samples.len()];
        // 每 8 条掺一张图片，让网格里有真实的缩略图
        if i % 8 == 3 {
            // 每条用独立文件名：trim() 删记录时会连带删文件，
            // 共用同一个 blob 的话，删掉其中一条就会让其余图片全变成裂图。
            let file_name = format!("seed-{}.png", i);
            let _ = std::fs::write(paths.blobs.join(&file_name), &sample_png);

            let mut item = ClipboardItem {
                id: 0,
                item_type: ClipboardItemType::Image,
                content: "480 x 320 image".to_string(),
                blob_path: Some(file_name),
                hash: format!("seed-img-{}", i),
                created_at: base + chrono::Duration::minutes((i * 3) as i64),
                pinned: false,
                tag: None,
            };
            let _ = db.add(&mut item);
            continue;
        }

        let item_type = if sample.contains('\\') {
            ClipboardItemType::Files
        } else {
            ClipboardItemType::Text
        };

        let tag = if i % 6 == 0 {
            Some("常用".to_string())
        } else if i % 6 == 1 {
            Some("代码".to_string())
        } else if i % 6 == 2 {
            Some("工作".to_string())
        } else {
            None
        };

        let mut item = ClipboardItem {
            id: 0,
            item_type,
            content: format!("[{:02}] {}", i + 1, sample),
            blob_path: None,
            hash: format!("seed-{}", i),
            created_at: base + chrono::Duration::minutes((i * 3) as i64),
            pinned: i == 2 || i == 15,
            tag,
        };
        let _ = db.add(&mut item);
    }
}
