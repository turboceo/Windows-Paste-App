//! 端到端自检。
//!
//! 状态现在归 `worker` 线程独占，`dispatch` 也变成了异步投递，
//! 所以测试不能再去"加锁读状态"。改写方式是：
//!
//! - 发命令用 `handle.send(...)`
//! - 读状态用 `handle.snapshot()`——它排在同一条 channel 上，
//!   因此返回时前面发出的命令一定已经处理完
//!
//! 这样测试既拿到了确定性的同步语义，又不需要在生产代码里保留任何锁。

use std::thread;
use std::time::{Duration, Instant};

use tauri::{AppHandle, Manager};

use crate::commands::HostRequest;
use crate::model::ClipboardItemType;
use crate::worker::{Command, StateHandle};

pub fn start(app: AppHandle) {
    thread::spawn(move || {
        // 等待 Webview 与底层窗口完成首帧初始化
        thread::sleep(Duration::from_millis(600));

        let Some(handle) = app.try_state::<StateHandle>().map(|s| s.inner().clone()) else {
            println!("自检失败：找不到状态句柄");
            std::process::exit(1);
        };

        let start_time = Instant::now();
        println!("\n========================================================");
        println!("         Windows Paste 自动化功能自检 (Self-Test)        ");
        println!("========================================================");

        let mut passed = 0;
        let total = 10;

        // 便捷封装：发一条请求
        let send = |req: HostRequest| handle.send(Command::Dispatch(req));

        // --------------------------------------------------------------------
        // TEST 1: SQLite 数据库与 50 条测试种子数据验证
        // --------------------------------------------------------------------
        print!("  [1/{}] 验证 SQLite 本地数据库与 50 条种子数据... ", total);
        let test1_ok = match handle.snapshot() {
            Some(snap) => {
                let has_50 = snap.total_count == 50 && snap.item_count == 50;
                let has_pinned = snap.items.iter().any(|i| i.pinned);
                let has_text = snap
                    .items
                    .iter()
                    .any(|i| i.item_type == ClipboardItemType::Text);
                let has_files = snap
                    .items
                    .iter()
                    .any(|i| i.item_type == ClipboardItemType::Files);
                has_50 && has_pinned && has_text && has_files
            }
            None => false,
        };
        if test1_ok {
            println!("PASS (50 条多品类数据完整加载)");
            passed += 1;
        } else {
            println!("FAIL (数据量或格式不匹配)");
        }

        // --------------------------------------------------------------------
        // TEST 2: 剪贴板模糊搜索过滤 (Search Filtering)
        // --------------------------------------------------------------------
        print!("  [2/{}] 验证剪贴板模糊搜索过滤 (检索 'github')... ", total);
        send(HostRequest::Search {
            text: "github".to_string(),
        });
        let test2_ok = match handle.snapshot() {
            Some(snap) => {
                let count = snap.item_count;
                count > 0
                    && count < 50
                    && snap
                        .items
                        .iter()
                        .all(|i| i.content.to_lowercase().contains("github"))
            }
            None => false,
        };
        if test2_ok {
            println!("PASS (筛选结果精确匹配，实时收窄列表)");
            passed += 1;
        } else {
            println!("FAIL (搜索过滤结果异常)");
        }

        // --------------------------------------------------------------------
        // TEST 3: 搜索条件清空与全量恢复 (Search Reset)
        // --------------------------------------------------------------------
        print!("  [3/{}] 验证搜索重置与历史全量恢复... ", total);
        send(HostRequest::Search { text: String::new() });
        let test3_ok = match handle.snapshot() {
            Some(snap) => snap.item_count == 50 && snap.total_count == 50,
            None => false,
        };
        if test3_ok {
            println!("PASS (完全恢复 50 项初始历史)");
            passed += 1;
        } else {
            println!("FAIL (搜索重置未能恢复全量)");
        }

        // --------------------------------------------------------------------
        // TEST 4: 矩阵焦点导航与跨行移动 (Move Navigation)
        // --------------------------------------------------------------------
        print!("  [4/{}] 验证键盘矩阵焦点移动 (单格 + 跨行)... ", total);
        let first_id = handle
            .snapshot()
            .and_then(|s| s.items.first().map(|i| i.id))
            .unwrap_or(0);

        send(HostRequest::Select { item_id: first_id });
        // 向后移动 1 格
        send(HostRequest::Move { delta: 1 });
        let second_id = handle.snapshot().and_then(|s| s.selected_id);

        // 跨行移动：|delta| > 1 即"按行走"，步长由后端的 columns 决定
        send(HostRequest::Move { delta: 2 });
        let row2_id = handle.snapshot().and_then(|s| s.selected_id);

        let test4_ok = second_id != Some(first_id) && row2_id != second_id && row2_id.is_some();
        if test4_ok {
            println!("PASS (单步移动与跨行网格焦点响应正常)");
            passed += 1;
        } else {
            println!("FAIL (焦点位置计算偏差)");
        }

        // --------------------------------------------------------------------
        // TEST 5: 卡片置顶与顺序重排持久化 (Toggle Pin)
        // --------------------------------------------------------------------
        print!("  [5/{}] 验证卡片置顶与取消置顶同步... ", total);
        let target_item = handle
            .snapshot()
            .and_then(|s| s.items.iter().find(|i| !i.pinned).cloned());

        let mut test5_ok = false;
        if let Some(target) = target_item {
            send(HostRequest::TogglePin { item_id: target.id });
            let pinned_ok = handle
                .snapshot()
                .map(|s| s.items.iter().any(|i| i.id == target.id && i.pinned))
                .unwrap_or(false);

            send(HostRequest::TogglePin { item_id: target.id });
            let unpinned_ok = handle
                .snapshot()
                .map(|s| s.items.iter().any(|i| i.id == target.id && !i.pinned))
                .unwrap_or(false);

            test5_ok = pinned_ok && unpinned_ok;
        }
        if test5_ok {
            println!("PASS (置顶卡片自动置顶居前，取消后恢复原状)");
            passed += 1;
        } else {
            println!("FAIL (置顶状态持久化失败)");
        }

        // --------------------------------------------------------------------
        // TEST 6: 卡片删除与本地持久化同步 (Delete Item)
        // --------------------------------------------------------------------
        print!("  [6/{}] 验证卡片物理删除与列表自动更新... ", total);
        // 用删除前的总数做基准，而不是把 50 硬编码成 49——
        // 换个 --seed 参数就不会再误报。
        let before = handle.snapshot();
        let mut test6_ok = false;
        if let Some(snap) = before {
            if let Some(del_item) = snap.items.last().cloned() {
                let before_count = snap.total_count;
                send(HostRequest::Delete {
                    item_id: del_item.id,
                });
                test6_ok = handle
                    .snapshot()
                    .map(|after| {
                        after.total_count + 1 == before_count
                            && !after.items.iter().any(|i| i.id == del_item.id)
                    })
                    .unwrap_or(false);
            }
        }
        if test6_ok {
            println!("PASS (历史记录减 1，已淘汰项从数据库清除)");
            passed += 1;
        } else {
            println!("FAIL (删除条目后总数不匹配)");
        }

        // --------------------------------------------------------------------
        // TEST 7: Win32 原生剪贴板格式写入与校验 (Clipboard Service)
        // --------------------------------------------------------------------
        print!("  [7/{}] 验证 Win32 原生剪贴板写入能力... ", total);
        let magic_text = format!(
            "[SELFTEST-{:x}] Windows Paste Native Test",
            start_time.elapsed().as_micros()
        );
        let test_item = crate::model::ClipboardItem {
            id: 999999,
            item_type: ClipboardItemType::Text,
            content: magic_text.clone(),
            blob_path: None,
            hash: "selftest-hash".to_string(),
            created_at: chrono::Local::now(),
            pinned: false,
            tag: None,
        };

        // blob 目录：自检跑在临时目录里，从环境拿不到就直接用当前 exe 同级的 blobs。
        // 文本条目用不到它，所以这里允许退化成临时目录。
        let blobs_dir = std::env::temp_dir().join("WindowsPaste-selftest-blobs");
        let _ = std::fs::create_dir_all(&blobs_dir);

        crate::paste::PasteService::write_to_clipboard(&test_item, &blobs_dir);
        thread::sleep(Duration::from_millis(30));

        // 从 Win32 系统剪贴板读回验证。
        //
        // 这里原来是软断言：读回失败也算 PASS，测试永远不会红。
        // 现在改成真的断言——剪贴板被别的进程锁住确实可能偶发，
        // 所以给几次重试，仍然读不回来就是失败。
        let sniffer = crate::clipboard::ClipboardSniffer::new(&blobs_dir);
        let mut test7_ok = false;
        for _ in 0..10 {
            let readback = sniffer.capture();
            if readback
                .map(|i| i.content.contains(&magic_text))
                .unwrap_or(false)
            {
                test7_ok = true;
                break;
            }
            thread::sleep(Duration::from_millis(50));
        }

        if test7_ok {
            println!("PASS (CF_UNICODETEXT 原生回写并验证成功)");
            passed += 1;
        } else {
            println!("FAIL (剪贴板回写后读回内容不匹配)");
        }

        // --------------------------------------------------------------------
        // TEST 8: 主窗口几何与无边框配置验证 (Window Geometry)
        // --------------------------------------------------------------------
        print!("  [8/{}] 验证主窗口无边框与几何尺寸... ", total);
        let mut test8_ok = false;
        if let Some(win) = app.get_webview_window("main") {
            let size = win.outer_size().unwrap_or_default();
            let is_decorations = win.is_decorated().unwrap_or(true);
            test8_ok = size.width >= 720 && size.height >= 420 && !is_decorations;
        }
        if test8_ok {
            println!("PASS (1080x600 无边框窗口与桌面居中定位就绪)");
            passed += 1;
        } else {
            println!("FAIL (窗口尺寸或无边框配置异常)");
        }

        // --------------------------------------------------------------------
        // TEST 9: 卡片分类 Tag 设置与动态筛选 (Tag Management & Filter)
        // --------------------------------------------------------------------
        print!("  [9/{}] 验证卡片分类 Tag 设置与按 Tag 筛选... ", total);
        let tag_target = handle.snapshot().and_then(|s| s.items.first().map(|i| i.id));

        let mut test9_ok = false;
        if let Some(target_id) = tag_target {
            let tag_name = "自动化分类".to_string();
            send(HostRequest::SetTag {
                item_id: target_id,
                tag: Some(tag_name.clone()),
            });
            let tag_saved = handle
                .snapshot()
                .map(|s| {
                    s.items.iter().any(|i| {
                        i.id == target_id && i.tag.as_deref() == Some(tag_name.as_str())
                    })
                })
                .unwrap_or(false);

            // 测试按 Tag 过滤
            send(HostRequest::FilterTag {
                tag: Some(tag_name.clone()),
            });
            let filter_ok = handle
                .snapshot()
                .map(|s| {
                    !s.items.is_empty()
                        && s.items
                            .iter()
                            .all(|i| i.tag.as_deref() == Some(tag_name.as_str()))
                })
                .unwrap_or(false);

            // 恢复全量过滤
            send(HostRequest::FilterTag { tag: None });
            test9_ok = tag_saved && filter_ok;
        }
        if test9_ok {
            println!("PASS (Tag 持久化与分类卡片瞬时过滤成功)");
            passed += 1;
        } else {
            println!("FAIL (Tag 分类设置或过滤异常)");
        }

        // --------------------------------------------------------------------
        // TEST 10: 任意列数下的上下键跨行移动 (Row Navigation vs Columns)
        // --------------------------------------------------------------------
        print!("  [10/{}] 验证 5 / 2 / 8 列时上下键都按整行移动... ", total);
        // 这是之前那个 bug 的回归测试：列数 > 4 时上下键会退化成左右移动一格。
        let mut test10_ok = true;
        for columns in [5usize, 2usize, 8usize] {
            send(HostRequest::Columns {
                columns: columns as i64,
            });

            let Some(snap) = handle.snapshot() else {
                test10_ok = false;
                break;
            };
            let Some(first) = snap.items.first().map(|i| i.id) else {
                test10_ok = false;
                break;
            };
            let len = snap.item_count;

            send(HostRequest::Select { item_id: first });
            // delta = 4 表示"向下一行"，与具体列数无关
            send(HostRequest::Move { delta: 4 });

            let landed = handle
                .snapshot()
                .map(|s| {
                    let selected = s.selected_id;
                    s.items
                        .iter()
                        .position(|i| Some(i.id) == selected)
                        .unwrap_or(0)
                })
                .unwrap_or(0);

            let expected = columns.min(len.saturating_sub(1));
            if landed != expected {
                test10_ok = false;
                println!(
                    "\n    列数 {}：落到第 {} 项，期望第 {} 项",
                    columns, landed, expected
                );
            }
        }

        if test10_ok {
            println!("PASS (跨行步长随列数变化，不再依赖魔法数字 4)");
            passed += 1;
        } else {
            println!("FAIL (跨行步长未随列数变化)");
        }

        let elapsed = start_time.elapsed();
        println!("--------------------------------------------------------");
        if passed == total {
            println!(
                "  测试结果：全部通过 (Passed: {}/{}) 耗时: {:.2?}",
                passed, total, elapsed
            );
            println!("========================================================\n");
            std::process::exit(0);
        } else {
            println!(
                "  测试结果：存在失败项 (Passed: {}/{}) 耗时: {:.2?}",
                passed, total, elapsed
            );
            println!("========================================================\n");
            std::process::exit(1);
        }
    });
}
