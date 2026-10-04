#![allow(clippy::pedantic, clippy::restriction)]

use crate::logger::logger;
use michiu_guard::Validated;
use michiu_ui::{
    CapacityConfig, CharIndex, CssLoader, ExternalDataSetBuilder, ImeState, VirtualKey,
    WebView2Contents, WebView2Visual, dispatch_raw_input_to_external_visual, prelude::*,
    raw_wheel_delta_to_logical_pixels,
};
use michiu_window::{
    ComContext, Event, EventPump, LogicalSize, MichiuEvent, Window, WindowBuilder, WindowHandle,
    init_dpi_awareness,
};
use std::cell::{Cell, RefCell};
use windows::Win32::Foundation::HWND;

mod app;
mod components;
mod logger;

/// ウィンドウメッセージ処理時に Context と Renderer を一元管理するためのアプリケーション状態
struct AppState {
    renderer: ComposedRenderer,
    context: Context,
    root_id: EntityId,
}

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

    let _ = init_dpi_awareness();

    let com = ComContext::new_ro_single()?;

    let builder = WindowBuilder::new()
        .with_title("Michiu Sample Collection")
        .with_visible(false)
        .with_no_redirection_bitmap(true)
        .with_inner_size(LogicalSize::new(1000.0, 800.0))
        .with_com_context(&com)
        .with_default_composition_window(false)
        .into_unvalidated();

    let window = Window::build(builder.try_into()?)?;

    let scale_factor = window.scale_factor() as f32;
    let handle = window.handle().assume_valid();
    let hwnd = handle.hwnd();

    let renderer = create_renderer(hwnd, scale_factor)?;

    let inspector = MichiuInspector::new();
    let _sub = inspector.subscribe(None);

    let mut context =
        Context::with_capacity_and_inspector(&CapacityConfig::from_base_nodes(1024), &inspector)
            .with_accessibility(hwnd);

    let h_clone = handle.clone();
    context.set_waker(move || h_clone.wake_up());

    // デバッグログ用のスレッド
    #[cfg(feature = "trace-error")]
    logger(_sub);

    let device = renderer.composition_device()?;
    let task_sender = context.task_sender();

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
        .watch(&mut context)?;

    let root = build_ui(&mut context, move || {
        let (read_github, _) = create_signal(GitHubVisual(github_visual));
        let (read_youtube, _) = create_signal(YouTubeVisual(youtube_visual));
        app::create_root()
            .provide(styles_sig)
            .provide(read_github)
            .provide(read_youtube)
    });

    let mut app = AppState {
        renderer,
        context,
        root_id: root.id(),
    };

    handle.set_visible(true);

    event_loop(&mut app, handle)?;

    Ok(())
}

pub fn create_renderer(
    hwnd: HWND,
    scale_factor: f32,
) -> Result<ComposedRenderer, Box<dyn std::error::Error>> {
    let initial_layout_size = LayoutSize::new(1000.0, 800.0);
    // レンダラーを作成
    let renderer = pollster::block_on(ComposedRenderer::new(
        hwnd,
        initial_layout_size,
        scale_factor,
    ))?;

    Ok(renderer)
}

fn event_loop(app: &mut AppState, handle: Validated<WindowHandle>) -> michiu_window::Result<()> {
    use windows::Win32::UI::WindowsAndMessaging::{WM_ENTERSIZEMOVE, WM_EXITSIZEMOVE, WM_NULL};

    let hwnd = handle.hwnd();

    let mut event_pump = EventPump::new();

    while event_pump.wait_event(|event, _, raw| {
        match raw.msg {
            WM_NULL => {
                // バックグラウンドから届いた CSS 更新タスクなどを安全に消化
                app.context.process_main_thread_tasks();

                // 消化によってレイアウトや描画に変更があった場合のみ再描画を実行
                if app.context.has_dirty() {
                    handle.redraw_requested();
                }
            }
            WM_ENTERSIZEMOVE => {
                app.context.set_window_resized(true);
                handle.redraw_requested();
                handle.update_window();
            }
            // ウィンドウドラッグリサイズの完了をキャッチ
            WM_EXITSIZEMOVE => {
                app.context.set_window_resized(false);
                // リサイズ完了後の再描画を即座にキックして、新サイズでの静止画キャプチャを誘発
                handle.redraw_requested();
                handle.update_window();
            }

            _ => {}
        }

        if let MichiuEvent::Window { event, .. } = event {
            match event {
                Event::CloseRequested => {
                    handle.destroy();
                }
                Event::Destroyed => {
                    handle.quit();
                }
                Event::Resized(phy_size) => {
                    let size = phy_size.assume_valid().into_inner();
                    let width = size.width as u32;
                    let height = size.height as u32;
                    // レンダラーのリサイズとレイアウト物理サイズの更新
                    app.renderer
                        .resize((width, height), app.renderer.scale_factor());

                    // 再描画要求
                    handle.redraw_requested();
                    handle.update_window();
                }
                Event::RedrawRequested => {
                    redraw_requested(app, handle.clone());
                }
                Event::CursorMoved { position } => {
                    let pos = position.assume_valid().into_inner();
                    let x = pos.x as f32;
                    let y = pos.y as f32;
                    let phys_pos = LayoutPoint::new(x, y);
                    let logical_pos = LayoutPoint::new(
                        x / app.renderer.scale_factor(),
                        y / app.renderer.scale_factor(),
                    );

                    // ホバー中要素から最適なカーソルを解決
                    let target_cursor = if let Some(hovered) =
                        app.context.interaction_id(InteractionState::Hovered)
                    {
                        app.context.resolve_cursor(hovered)
                    } else {
                        CursorIcon::Default(None)
                    };

                    let hcursor = target_cursor.to_hcursor().unwrap();

                    handle.set_cursor_icon(michiu_window::CursorIcon::Other(hcursor));

                    app.context
                        .inject_user_action(UserAction::PointerMove(logical_pos));

                    let _consumed = dispatch_raw_input_to_external_visual(
                        &mut app.context,
                        raw.msg,
                        raw.wparam,
                        raw.lparam,
                        phys_pos,
                        app.renderer.scale_factor(),
                    );

                    // インタラクションによる変化（ホバー状態）をリアルタイムに再描画
                    handle.redraw_requested();
                }
                Event::CursorLeft => {
                    // ウィンドウ外に去ったため、論理空間外へポインタを移動させてホバーを確実に解除
                    app.context
                        .inject_user_action(UserAction::PointerMove(LayoutPoint::new(
                            -9999.0, -9999.0,
                        )));

                    handle.redraw_requested();
                }
                Event::MouseInput {
                    button,
                    modifiers,
                    state,
                    click_count,
                } => {
                    let button = match button {
                        michiu_window::MouseButton::Left => MouseButton::Left,
                        michiu_window::MouseButton::Right => MouseButton::Right,
                        michiu_window::MouseButton::Middle => MouseButton::Middle,
                        michiu_window::MouseButton::Other(_) => MouseButton::X1,
                    };
                    let modifiers = Modifiers {
                        shift: modifiers.contains(michiu_window::Modifiers::SHIFT),
                        ctrl: modifiers.contains(michiu_window::Modifiers::CONTROL),
                        alt: modifiers.contains(michiu_window::Modifiers::ALT),
                        logo: modifiers.contains(michiu_window::Modifiers::LOGO),
                    };
                    let state = match state {
                        michiu_window::ElementState::Pressed => ElementState::Pressed,
                        michiu_window::ElementState::Released => ElementState::Released,
                    };

                    // ホバー中要素から最適なカーソルを解決
                    let target_cursor = if let Some(hovered) =
                        app.context.interaction_id(InteractionState::Hovered)
                    {
                        app.context.resolve_cursor(hovered)
                    } else {
                        CursorIcon::Default(None)
                    };

                    let hcursor = target_cursor.to_hcursor().unwrap();

                    unsafe { windows::Win32::UI::WindowsAndMessaging::SetCursor(Some(hcursor)) };

                    app.context.inject_user_action(UserAction::PointerButton {
                        button,
                        state,
                        modifiers,
                    });

                    if click_count == 2 {
                        app.context
                            .inject_user_action(UserAction::PointerDoubleClick { modifiers });
                    }

                    let x = (raw.lparam.0 & 0xffff) as i16 as f32;
                    let y = ((raw.lparam.0 >> 16) & 0xffff) as i16 as f32;
                    let phys_pos = LayoutPoint::new(x, y);
                    let _consumed = dispatch_raw_input_to_external_visual(
                        &mut app.context,
                        raw.msg,
                        raw.wparam,
                        raw.lparam,
                        phys_pos,
                        app.renderer.scale_factor(),
                    );

                    // フォーカス取得（点滅カーソル表示開始）のために再描画
                    handle.redraw_requested();
                }
                Event::CharacterInput(c) => {
                    app.context.inject_user_action(UserAction::Character(c));
                    handle.redraw_requested();
                }
                Event::KeyboardInput {
                    key_code,
                    modifiers,
                    state,
                } => {
                    let key_code = key_code.assume_valid().into_inner();
                    let key = VirtualKey::from_windows(key_code);
                    let modifiers = Modifiers {
                        shift: modifiers.contains(michiu_window::Modifiers::SHIFT),
                        ctrl: modifiers.contains(michiu_window::Modifiers::CONTROL),
                        alt: modifiers.contains(michiu_window::Modifiers::ALT),
                        logo: modifiers.contains(michiu_window::Modifiers::LOGO),
                    };
                    let state = match state {
                        michiu_window::ElementState::Pressed => ElementState::Pressed,
                        michiu_window::ElementState::Released => ElementState::Released,
                    };

                    if modifiers.ctrl {
                        match raw.wparam.0 as i32 {
                            // Ctrl + C
                            0x43 => {
                                // 'C' キー
                                if let Some(selected_text) = app.context.get_selected_text() {
                                    let _ = set_win32_clipboard(&selected_text);
                                }
                            }
                            // Ctrl + V
                            0x56 => {
                                // 'V' キー
                                if let Some(pasted_text) = get_win32_clipboard() {
                                    app.context
                                        .inject_user_action(UserAction::Paste(pasted_text.into()));
                                }
                            }
                            // Ctrl + X (切り取り)
                            0x58 => {
                                // 'X'
                                if let Some(selected_text) = app.context.get_selected_text() {
                                    let _ = set_win32_clipboard(&selected_text);
                                }
                                app.context.inject_user_action(UserAction::Cut);
                            }
                            // Ctrl + Z (Undo)
                            0x5A => {
                                // 'Z'
                                app.context.inject_user_action(UserAction::Undo);
                            }
                            // Ctrl + Y (Redo)
                            0x59 => {
                                // 'Y'
                                app.context.inject_user_action(UserAction::Redo);
                            }
                            _ => {}
                        }
                    }

                    app.context.inject_user_action(UserAction::KeyboardKey {
                        key,
                        state,
                        modifiers,
                    });

                    handle.redraw_requested();
                }
                Event::MouseWheel {
                    raw_delta_x,
                    raw_delta_y,
                } => {
                    let mut pt = windows::Win32::Foundation::POINT {
                        x: (raw.lparam.0 & 0xffff) as i16 as i32,
                        y: ((raw.lparam.0 >> 16) & 0xffff) as i16 as i32,
                    };
                    let _ = unsafe { windows::Win32::Graphics::Gdi::ScreenToClient(hwnd, &mut pt) };

                    // 横スクロール時は、右チルト（プラス値）された際に
                    // 右方向へスクロール（オフセット加算）させるため、符号の方向性を補正
                    let scroll_x = {
                        let x = raw_delta_x.assume_valid().into_inner();
                        -raw_wheel_delta_to_logical_pixels(x.raw() as f32)
                    };

                    let scroll_y = {
                        let y = raw_delta_y.assume_valid().into_inner();
                        raw_wheel_delta_to_logical_pixels(y.raw() as f32)
                    };

                    app.context
                        .inject_user_action(UserAction::MouseWheel { scroll_x, scroll_y });

                    let phys_pos = LayoutPoint::new(pt.x as f32, pt.y as f32);
                    let _consumed = dispatch_raw_input_to_external_visual(
                        &mut app.context,
                        raw.msg,
                        raw.wparam,
                        raw.lparam,
                        phys_pos,
                        app.renderer.scale_factor(),
                    );

                    // 画面を再描画
                    handle.redraw_requested();
                }
                Event::Ime(ime) => {
                    let ime = ime.assume_valid().into_inner();
                    let ime_state = ImeState {
                        is_open: ime.is_open,
                        conversion_mode: ime.conversion_mode,
                        sentence_mode: ime.sentence_mode,
                        keyboard_layout_id: ime.keyboard_layout_id,
                        composition_text: ime.composition_text.into(),
                        result_text: ime.result_text.into(),
                        caret_position: ime.caret_position.map(|p| LayoutPoint {
                            x: p.x as f32,
                            y: p.y as f32,
                        }),
                        composition_cursor: CharIndex(ime.composition_cursor),
                        composition_attrs: ime.composition_attrs,
                    };

                    app.context.inject_user_action(UserAction::Ime(ime_state));
                    handle.redraw_requested();
                }
                _ => {}
            }
        }
    })? {}

    Ok(())
}

fn redraw_requested(app: &mut AppState, handle: Validated<WindowHandle>) {
    let frame_start = std::time::Instant::now();

    let update_start = std::time::Instant::now();
    app.context.begin_frame();
    app.context.tick_system_frame(&TickType::All);
    let update_elapsed = update_start.elapsed();

    let layout_start = std::time::Instant::now();
    app.context
        .sync_layout(app.root_id, app.renderer.layout_size());
    let layout_elapsed = layout_start.elapsed();

    let comp_start = std::time::Instant::now();
    app.renderer.update_composition_tree(&mut app.context);
    let comp_elapsed = comp_start.elapsed();

    let draw_start = std::time::Instant::now();

    app.context.update_accessibility();

    if let Some(_ctx) = handle.begin_paint() {
        app.renderer.draw(&mut app.context);
    }

    let draw_elapsed = draw_start.elapsed();

    let cpu_active_elapsed = update_elapsed + layout_elapsed + comp_elapsed + draw_elapsed;

    let sync_start = std::time::Instant::now();
    if app.context.has_active_frame() {
        let _ = unsafe { windows::Win32::Graphics::Dwm::DwmFlush() };
        let _ = unsafe {
            windows::Win32::Graphics::Gdi::InvalidateRect(Some(handle.hwnd()), None, false)
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
