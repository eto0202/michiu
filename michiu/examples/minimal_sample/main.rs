#![allow(clippy::pedantic, clippy::restriction)]

use michiu_guard::Validated;
use michiu_ui::prelude::*;
use michiu_window::{
    ComContext, Event, EventPump, LogicalSize, MichiuEvent, Window, WindowBuilder, WindowHandle,
    init_dpi_awareness,
};

struct AppState {
    renderer: ComposedRenderer,
    context: Context,
    root_id: EntityId,
}

const ALLOW_LOG: bool = true;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let _ = init_dpi_awareness();

    let com = ComContext::new_ro_single()?;

    let builder = WindowBuilder::new()
        .with_title("Sample App")
        .with_visible(false)
        .with_com_context(&com)
        .with_no_redirection_bitmap(true)
        .with_inner_size(LogicalSize::new(600.0, 500.0))
        .into_unvalidated();

    let window = Window::build(builder.try_into()?)?;

    let scale_factor = window.scale_factor() as f32;
    let handle = window.handle().assume_valid();
    let hwnd = handle.hwnd();

    let renderer = pollster::block_on(ComposedRenderer::new(
        hwnd,
        LayoutSize::new(600.0, 500.0),
        scale_factor,
    ))?;

    let mut context = Context::new();

    let root = build_ui(&mut context, move || {
        let (count, set_count) = create_signal(0u32);

        flex(
            ts().size_full()
                .justify_center()
                .items_center()
                .bg_color(rgb(22, 24, 29)),
        )
        .child(
            flex(
                ts().r(6.0)
                    .size((200.0, 100.0))
                    .border_solid(2.0)
                    .border_color(rgb(209, 92, 174))
                    .justify_center()
                    .pressed(ts().transform_scale(0.98, 0.98)),
            )
            .on_click(move || set_count.set(count.get() + 1))
            .label(
                move || format!("Count: {}", count.get()),
                ts().text_color(rgb(156, 158, 163)).font_size(22.0),
            ),
        )
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

fn event_loop(app: &mut AppState, handle: Validated<WindowHandle>) -> michiu_window::Result<()> {
    let mut event_pump = EventPump::new();

    while event_pump.wait_event(|event, _, _| {
        if let MichiuEvent::Window { event, .. } = event {
            match event {
                Event::CloseRequested => {
                    handle.destroy();
                }
                Event::Destroyed => {
                    handle.quit();
                }
                Event::Resized(phy_size) => {
                    use windows::Win32::Graphics::Gdi::{InvalidateRect, UpdateWindow};
                    let size = phy_size.assume_valid().into_inner();
                    let width = size.width as u32;
                    let height = size.height as u32;

                    app.renderer
                        .resize((width, height), app.renderer.scale_factor());

                    let _ = unsafe { InvalidateRect(Some(handle.hwnd()), None, false) };
                    let _ = unsafe { UpdateWindow(handle.hwnd()) };
                }
                Event::RedrawRequested => {
                    redraw_requested(app, handle.clone());
                }
                Event::CursorMoved { position } => {
                    let pos = position.assume_valid().into_inner();
                    let x = pos.x as f32;
                    let y = pos.y as f32;
                    let logical_pos = LayoutPoint::new(
                        x / app.renderer.scale_factor(),
                        y / app.renderer.scale_factor(),
                    );

                    app.context
                        .inject_user_action(UserAction::PointerMove(logical_pos));
                }
                Event::MouseInput {
                    button,
                    modifiers,
                    state,
                    ..
                } => {
                    let button = match button {
                        michiu_window::MouseButton::Left => MouseButton::Left,
                        michiu_window::MouseButton::Right => MouseButton::Right,
                        michiu_window::MouseButton::Middle => MouseButton::Middle,
                        michiu_window::MouseButton::Other(_) => MouseButton::X1,
                    };
                    let modifiers = Modifiers {
                        shift: modifiers == michiu_window::Modifiers::SHIFT,
                        ctrl: modifiers == michiu_window::Modifiers::CONTROL,
                        alt: modifiers == michiu_window::Modifiers::ALT,
                        logo: modifiers == michiu_window::Modifiers::LOGO,
                    };
                    let state = match state {
                        michiu_window::ElementState::Pressed => ElementState::Pressed,
                        michiu_window::ElementState::Released => ElementState::Released,
                    };

                    app.context.inject_user_action(UserAction::PointerButton {
                        button,
                        state,
                        modifiers,
                    });
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
    app.renderer.draw(&mut app.context);
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
        static LAST_PRINT: std::cell::Cell<Option<std::time::Instant>>  = const { std::cell::Cell::new(None) };
        // 1秒間のデータを一時保存するバッファ
        static FRAME_DATA: std::cell::RefCell<Vec<FrameMetrics>>  = const { std::cell::RefCell::new(Vec::new()) };
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
