// #![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]
#![allow(clippy::pedantic, clippy::restriction)]

use crate::logger::logger;
use michiu::{
    AutoSyncMode, MichiuApp, MichiuAppBuilder,
    ui::{
        CapacityConfig, CssLoader, ExternalDataSetBuilder, WebView2Contents, WebView2Visual,
        prelude::*,
    },
    window::prelude::*,
};
use std::cell::{Cell, RefCell};

mod app;
mod components;
mod logger;

#[derive(Clone)]
pub struct GitHubVisual(WebView2Visual);
#[derive(Clone)]
pub struct CratesVisual(WebView2Visual);

pub const ALLOW_LOG: bool = false;
// これ起動めちゃ遅くなるので注意
pub const ALLOW_STRESS_TEST: bool = false;

#[cfg(feature = "dhat-heap")]
#[global_allocator]
static ALLOC: dhat::Alloc = dhat::Alloc;

// cargo run --features trace-lifecycle
// cargo run --features dhat-heap
// cargo build  --release
// cargo run  --release --features dhat-heap
fn main() -> Result<(), Box<dyn std::error::Error>> {
    #[cfg(feature = "dhat-heap")]
    let _profiler = dhat::Profiler::builder().trim_backtraces(None).build();

    let _ = MichiuWindow::init_dpi_awareness();
    let com = MichiuComContext::new_ro_single()?;

    let builder = MichiuWindowBuilder::new()
        .with_title("Sample Collection")
        .with_visible(false)
        .with_decorations(false)
        .with_no_redirection_bitmap(true)
        .with_inner_size(LogicalSize::new(1000.0, 800.0))
        .with_com_context(&com)
        .with_default_ime_window(false)
        .with_message_filter(filter)
        .into_unvalidated();

    let window = MichiuWindow::build(builder.try_into()?)?;

    let scale_factor = window.scale_factor() as f32;
    let handle = window.handle().assume_valid();
    let hwnd = handle.hwnd();

    let client_rect = handle.client_rect();
    let width = client_rect.width as f32;
    let height = client_rect.height as f32;

    // レンダラーを作成
    let renderer = pollster::block_on(MichiuRenderer::new(
        hwnd,
        LayoutSize::new(width, height),
        scale_factor,
    ))?;

    let inspector = MichiuInspector::new();
    let _sub = inspector.subscribe(None);

    // デバッグログ用のスレッド
    logger(_sub);

    let (mut app, mut pump) = MichiuAppBuilder::new(window)
        .with_accessibility_support(true)
        .with_capacity_config(CapacityConfig::from_base_nodes(1024))
        .with_inspector(inspector)
        .with_sync_mode(AutoSyncMode::DwmFlush)
        .with_external_visual_support(true)
        .with_default_shortcuts(true)
        .with_auto_resolve_cursor(true)
        .build(renderer)?;

    let device = app.renderer.composition_device()?;
    let task_sender = app.context.task_sender();

    WebView2Visual::prewarm_webview2();

    let github = WebView2Contents::from_url("https://github.com/eto0202/michiu/tree/main")
        .enable_context_menu(true)
        .enable_dev_tools(true)
        .allow_interaction(true)
        .always_active(false);
    let github_visual = WebView2Visual::new(&device, hwnd, github, scale_factor, &task_sender)
        .expect("Failed to create WebView2Visual");

    let crates = WebView2Contents::from_url("https://crates.io/crates/michiu")
        .allow_interaction(true)
        .always_active(true);
    let crates_visual = WebView2Visual::new(&device, hwnd, crates, scale_factor, &task_sender)
        .expect("Failed to create WebView2Visual");

    let css_path = concat!(env!("CARGO_MANIFEST_DIR"), "/sample_collection/global.css");
    let (styles_sig, _guard) = ExternalDataSetBuilder::new()
        .add("global", css_path, CssLoader)
        .watch(&mut app.context)?;

    app.build_ui(move || {
        let (read_github, _) = create_signal(GitHubVisual(github_visual));
        let (read_youtube, _) = create_signal(CratesVisual(crates_visual));
        let (read_handle, _) = create_signal(handle.into_inner());
        app::create_root()
            .provide(styles_sig)
            .provide(read_github)
            .provide(read_youtube)
            .provide(read_handle)
    });

    app.set_visible(true);

    event_loop(&mut app, &mut pump)?;

    Ok(())
}

fn event_loop(app: &mut MichiuApp, pump: &mut MichiuEventPump) -> michiu::window::Result<()> {
    while pump.wait_event(|event, _, raw| {
        let resp = app.standard_handle_window_event(&event, &raw);

        if resp.needs_redraw {
            app.redraw_requested();
        }
        if resp.needs_update_window {
            app.update_window();
        }

        if resp.consumed {
            return;
        }

        if let MichiuAnyEvent::Window { event, .. } = event {
            match event {
                MichiuEvent::CloseRequested => {
                    app.destroy();
                }
                MichiuEvent::Destroyed => {
                    app.quit();
                }
                MichiuEvent::RedrawRequested => {
                    redraw_requested(app);
                }
                _ => {}
            }
        }
    })? {}

    Ok(())
}

fn redraw_requested(app: &mut MichiuApp) {
    let frame_start = std::time::Instant::now();

    let update_start = std::time::Instant::now();
    app.context.begin_frame();
    app.standard_update_cursor_icon();
    app.context.tick_system_frame(&TickType::All);
    let update_elapsed = update_start.elapsed();

    let active_entities = app.context.active_entities();
    let dirty_entities = app
        .context
        .raw_context_mut()
        .layouts
        .dirty_entities_mut()
        .len();

    let layout_start = std::time::Instant::now();
    app.context
        .sync_layout(app.root_id(), app.renderer.layout_size());
    let layout_elapsed = layout_start.elapsed();

    let comp_start = std::time::Instant::now();
    app.renderer.update_composition_tree(&mut app.context);
    let comp_elapsed = comp_start.elapsed();

    let draw_start = std::time::Instant::now();
    if let Some(_ctx) = app.window.begin_paint() {
        app.renderer.draw(&mut app.context);
    }
    let draw_elapsed = draw_start.elapsed();

    app.context.update_accessibility();

    let cpu_active_elapsed = update_elapsed + layout_elapsed + comp_elapsed + draw_elapsed;

    let sync_start = std::time::Instant::now();
    if app.context.has_active_frame() {
        let _ = app.handle.dwm_flush();
        app.handle.redraw_requested();
    }
    let sync_elapsed = sync_start.elapsed();
    let total_elapsed = frame_start.elapsed();

    thread_local! {
        static LAST_PRINT: Cell<Option<std::time::Instant>> = const { Cell::new(None) };
        // 1秒間のデータを一時保存するバッファ
        static FRAME_DATA: RefCell<Vec<FrameMetrics>> = const { RefCell::new(Vec::new()) };
    }

    struct FrameMetrics {
        update: f64,
        layout: f64,
        comp: f64,
        draw: f64,
        sync: f64,
        cpu_active: f64,
        total: f64,
    }

    FRAME_DATA.with(|data| {
        data.borrow_mut().push(FrameMetrics {
            update: update_elapsed.as_secs_f64() * 1000.0,
            layout: layout_elapsed.as_secs_f64() * 1000.0,
            comp: comp_elapsed.as_secs_f64() * 1000.0,
            draw: draw_elapsed.as_secs_f64() * 1000.0,
            sync: sync_elapsed.as_secs_f64() * 1000.0,
            cpu_active: cpu_active_elapsed.as_secs_f64() * 1000.0,
            total: total_elapsed.as_secs_f64() * 1000.0,
        });
    });

    let now = std::time::Instant::now();
    let should_print = LAST_PRINT.with(|c| match c.get() {
        None => {
            c.set(Some(now));
            true
        }
        Some(last) => {
            if now.duration_since(last).as_secs_f32() >= 1.0 {
                c.set(Some(now));
                true
            } else {
                false
            }
        }
    });

    if should_print {
        FRAME_DATA.with(|data| {
            let mut frames = data.borrow_mut();
            let count = frames.len();
            let count_f64 = count as f64;
            if count > 0 {
                // 各メトリクスの平均値
                let avg_update =
                    frames.iter().map(|f| f.update).sum::<f64>() / count_f64;
                let avg_layout =
                    frames.iter().map(|f| f.layout).sum::<f64>() / count_f64;
                let avg_comp =
                    frames.iter().map(|f| f.comp).sum::<f64>() / count_f64;
                let avg_draw =
                    frames.iter().map(|f| f.draw).sum::<f64>() / count_f64;
                let avg_sync =
                    frames.iter().map(|f| f.sync).sum::<f64>() / count_f64;
                let avg_cpu =
                    frames.iter().map(|f| f.cpu_active).sum::<f64>() / count_f64;
                let avg_total =
                    frames.iter().map(|f| f.total).sum::<f64>() / count_f64;

                // P99を計算
                frames.sort_by(|a, b| a.cpu_active.partial_cmp(&b.cpu_active).unwrap());
                let p99_idx = (count * 99 / 100).min(count - 1);
                let p99_cpu = frames[p99_idx].cpu_active;
                let max_cpu = frames.last().unwrap().cpu_active;

                if ALLOW_LOG {
                    println!(
                        "[Loop Count: {:>3}] (Total Frame: {:6.3}ms)\n\
                            ├─ Phase Avg:   Upd: {:6.3}ms | Lay: {:6.3}ms | Cmp: {:6.3}ms | Drw: {:6.3}ms | Sync: {:6.3}ms\n\
                            ├─ CPU Active:  Avg: {:6.3}ms | P99: {:6.3}ms | Max: {:6.3}ms\n\
                            └─ Active Entities: {active_entities} | Dirty Entities: {dirty_entities}",
                        count,
                        avg_total,
                        avg_update,
                        avg_layout,
                        avg_comp,
                        avg_draw,
                        avg_sync,
                        avg_cpu,
                        p99_cpu,
                        max_cpu
                    );
                }
                frames.clear();
            }
        });
    }
}

use windows::Win32::{
    Foundation::{HWND, LPARAM, LRESULT, POINT, RECT, WPARAM},
    Graphics::Gdi::ScreenToClient,
    UI::{
        HiDpi::GetDpiForWindow,
        WindowsAndMessaging::{
            GetClientRect, HTBOTTOM, HTBOTTOMLEFT, HTBOTTOMRIGHT, HTCAPTION, HTCLIENT, HTLEFT,
            HTMAXBUTTON, HTRIGHT, HTTOP, HTTOPLEFT, HTTOPRIGHT, WM_ERASEBKGND, WM_NCCALCSIZE,
            WM_NCHITTEST,
        },
    },
};

fn filter(hwnd: HWND, msg: u32, _wparam: WPARAM, lparam: LPARAM) -> Option<LRESULT> {
    if msg == WM_ERASEBKGND {
        // OSによるデフォルト背景の塗りつぶしを阻止する
        return Some(LRESULT(1));
    }

    if msg == WM_NCCALCSIZE {
        return Some(LRESULT(0));
    }

    if msg == WM_NCHITTEST {
        // スケールファクター
        let dpi = unsafe { GetDpiForWindow(hwnd) };
        let scale = (dpi as f32) / 96.0;

        // スクリーン座標のマウス位置を取得
        let screen_x = (lparam.0 & 0xffff) as i16 as i32;
        let screen_y = ((lparam.0 >> 16) & 0xffff) as i16 as i32;

        // クライアント座標に変換
        let mut pt = POINT {
            x: screen_x,
            y: screen_y,
        };
        let _ = unsafe { ScreenToClient(hwnd, &raw mut pt) };

        // レンダラーと同じ GetClientRect を取得
        let mut client_rect = RECT::default();
        let _ = unsafe { GetClientRect(hwnd, &raw mut client_rect) };
        let width = client_rect.right; // client_rect.left は常に 0
        let height = client_rect.bottom; // client_rect.top は常に 0

        const BORDER_WIDTH: i32 = 8;
        let btn_w = (47.0 * scale) as i32;
        let btn_h = (40.0 * scale) as i32;

        // ボタンの領域判定（右端から3個分）
        let is_btn_row = pt.y >= 0 && pt.y < btn_h;
        let is_in_btn_area = is_btn_row && pt.x >= width - (btn_w * 3);

        // リサイズ枠の判定 (ボタン領域にいるときは除外)
        let on_left = pt.x < BORDER_WIDTH;
        let on_right = pt.x >= width - BORDER_WIDTH && !is_in_btn_area;
        let on_top = pt.y < BORDER_WIDTH && !is_in_btn_area;
        let on_bottom = pt.y >= height - BORDER_WIDTH;

        // 角
        if on_top && on_left {
            return Some(LRESULT(HTTOPLEFT as isize));
        }
        if on_top && on_right {
            return Some(LRESULT(HTTOPRIGHT as isize));
        }
        if on_bottom && on_left {
            return Some(LRESULT(HTBOTTOMLEFT as isize));
        }
        if on_bottom && on_right {
            return Some(LRESULT(HTBOTTOMRIGHT as isize));
        }

        // 辺
        if on_left {
            return Some(LRESULT(HTLEFT as isize));
        }
        if on_right {
            return Some(LRESULT(HTRIGHT as isize));
        }
        if on_top {
            return Some(LRESULT(HTTOP as isize));
        }
        if on_bottom {
            return Some(LRESULT(HTBOTTOM as isize));
        }

        // ボタンの判定
        // 最大化ボタン: スナップレイアウト等のため HTMAXBUTTON を返す
        if is_btn_row && pt.x >= width - (btn_w * 2) && pt.x < width - btn_w {
            return Some(LRESULT(HTMAXBUTTON as isize));
        }

        // 閉じるボタン & 最小化ボタン: UI側でクリック処理するため HTCLIENT を返す
        // (右端の 3個分のボタンエリアのうち、最大化ボタン以外)
        if is_in_btn_area {
            return Some(LRESULT(HTCLIENT as isize));
        }

        // タイトルバー領域
        if is_btn_row {
            let win_w = (client_rect.right - client_rect.left) as f32;
            let x_f = pt.x as f32;
            let y_f = pt.y as f32;

            // 検索ボックス領域 (幅 35%、最大 600px、高さ 26px、中央配置)
            let search_w = (win_w * 0.35).min(600.0 * scale);
            let center_x = client_rect.left as f32 + win_w / 2.0;
            let search_left = center_x - search_w / 2.0;
            let search_right = center_x + search_w / 2.0;

            let search_h = 26.0 * scale;
            let search_top = client_rect.top as f32 + (btn_h as f32 - search_h) / 2.0;
            let search_bottom = search_top + search_h;

            let is_on_search = x_f >= search_left
                && x_f < search_right
                && y_f >= search_top
                && y_f < search_bottom;

            // トグルボタン領域 (幅 50px、高さ 26px、right: 135px)
            let toggle_right = client_rect.right as f32 - (160.0 * scale);
            let toggle_left = toggle_right - (50.0 * scale);
            let toggle_top = client_rect.top as f32 + (btn_h as f32 - search_h) / 2.0;
            let toggle_bottom = toggle_top + search_h;

            let is_on_toggle = x_f >= toggle_left
                && x_f < toggle_right
                && y_f >= toggle_top
                && y_f < toggle_bottom;

            // UI コントロール上をクリックした場合は通常のクライアント操作
            if is_on_search || is_on_toggle {
                return Some(LRESULT(HTCLIENT as isize));
            }

            return Some(LRESULT(HTCAPTION as isize));
        }

        return Some(LRESULT(HTCLIENT as isize));
    }

    None
}
