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
pub struct YouTubeVisual(WebView2Visual);

pub const ALLOW_LOG: bool = true;
// これ起動めちゃ遅くなるので注意
pub const ALLOW_STRESS_TEST: bool = false;

#[cfg(feature = "dhat-heap")]
#[global_allocator]
static ALLOC: dhat::Alloc = dhat::Alloc;

// cargo build --timings --example sample_collection
// cargo build --example sample_collection --release
// cargo run --example sample_collection --release --features dhat-heap
fn main() -> Result<(), Box<dyn std::error::Error>> {
    #[cfg(feature = "dhat-heap")]
    let _profiler = dhat::Profiler::builder().trim_backtraces(None).build();

    let _ = MichiuWindow::init_dpi_awareness();

    let com = MichiuComContext::new_ro_single()?;

    let builder = MichiuWindowBuilder::new()
        .with_title("Michiu Sample Collection")
        .with_visible(false)
        .with_no_redirection_bitmap(true)
        .with_inner_size(LogicalSize::new(1000.0, 800.0))
        .with_com_context(&com)
        .with_default_composition_window(false)
        .into_unvalidated();

    let window = MichiuWindow::build(builder.try_into()?)?;

    let scale_factor = window.scale_factor() as f32;
    let handle = window.handle().assume_valid();
    let hwnd = handle.hwnd();

    // レンダラーを作成
    let renderer = pollster::block_on(MichiuRenderer::new(
        hwnd,
        LayoutSize::new(1000.0, 800.0),
        scale_factor,
    ))?;

    let inspector = MichiuInspector::new();
    let _sub = inspector.subscribe(None);

    let (mut app, mut pump) = MichiuAppBuilder::new(window)
        .with_accessibility_support(true)
        .with_capacity_config(CapacityConfig::from_base_nodes(1024))
        .with_inspector(inspector)
        .with_sync_mode(AutoSyncMode::DwmFlush)
        .with_external_visual_support(true)
        .with_default_shortcuts(true)
        .with_auto_resolve_cursor(true)
        .build(renderer)?;

    // デバッグログ用のスレッド
    #[cfg(feature = "trace-error")]
    logger(_sub);

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

    let youtube = WebView2Contents::from_url("https://www.youtube.com/")
        .allow_interaction(true)
        .always_active(true);
    let youtube_visual = WebView2Visual::new(&device, hwnd, youtube, scale_factor, &task_sender)
        .expect("Failed to create WebView2Visual");

    let css_path = concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/examples/sample_collection/global.css"
    );
    let (styles_sig, _guard) = ExternalDataSetBuilder::new()
        .add("global", css_path, CssLoader)
        .watch(&mut app.context)?;

    app.build_ui(move || {
        let (read_github, _) = create_signal(GitHubVisual(github_visual));
        let (read_youtube, _) = create_signal(YouTubeVisual(youtube_visual));
        app::create_root()
            .provide(styles_sig)
            .provide(read_github)
            .provide(read_youtube)
    });

    handle.set_visible(true);

    event_loop(&mut app, &mut pump)?;

    Ok(())
}

fn event_loop(app: &mut MichiuApp, pump: &mut MichiuEventPump) -> michiu_window::Result<()> {
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

    let layout_start = std::time::Instant::now();
    app.context
        .sync_layout(app.root_id(), app.renderer.layout_size());
    let layout_elapsed = layout_start.elapsed();

    let comp_start = std::time::Instant::now();
    app.renderer.update_composition_tree(&mut app.context);
    let comp_elapsed = comp_start.elapsed();

    let draw_start = std::time::Instant::now();

    app.context.update_accessibility();

    if let Some(_ctx) = app.handle.begin_paint() {
        app.renderer.draw(&mut app.context);
    }

    let draw_elapsed = draw_start.elapsed();

    let cpu_active_elapsed = update_elapsed + layout_elapsed + comp_elapsed + draw_elapsed;

    let sync_start = std::time::Instant::now();
    if app.context.has_active_frame() {
        let _ = unsafe { windows::Win32::Graphics::Dwm::DwmFlush() };
        let _ = unsafe {
            windows::Win32::Graphics::Gdi::InvalidateRect(Some(app.handle.hwnd()), None, false)
        };
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
            if count > 0 {
                // 各メトリクスの平均値
                let avg_update =
                    frames.iter().map(|f| f.update).sum::<f64>() / count as f64;
                let avg_layout =
                    frames.iter().map(|f| f.layout).sum::<f64>() / count as f64;
                let avg_comp =
                    frames.iter().map(|f| f.comp).sum::<f64>() / count as f64;
                let avg_draw =
                    frames.iter().map(|f| f.draw).sum::<f64>() / count as f64;
                let avg_sync =
                    frames.iter().map(|f| f.sync).sum::<f64>() / count as f64;
                let avg_cpu =
                    frames.iter().map(|f| f.cpu_active).sum::<f64>() / count as f64;
                let avg_total =
                    frames.iter().map(|f| f.total).sum::<f64>() / count as f64;

                // P99を計算
                frames.sort_by(|a, b| a.cpu_active.partial_cmp(&b.cpu_active).unwrap());
                let p99_idx = (count * 99 / 100).min(count - 1);
                let p99_cpu = frames[p99_idx].cpu_active;
                let max_cpu = frames.last().unwrap().cpu_active;

                if ALLOW_LOG {
                    println!(
                        "[Loop Count: {:>3}] (Total Frame: {:5.2}ms)\n\
                            ├─ Phase Avg:   Upd: {:5.2}ms | Lay: {:5.2}ms | Cmp: {:5.2}ms | Drw: {:5.2}ms | Sync: {:5.2}ms\n\
                            └─ CPU Active:  Avg: {:5.2}ms | P99: {:5.2}ms | Max: {:5.2}ms",
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
