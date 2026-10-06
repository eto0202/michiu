#![allow(clippy::pedantic, clippy::restriction)]

use michiu::{MichiuApp, MichiuAppBuilder, ui::prelude::*, window::prelude::*};

// cargo build --example minimal_sample --release
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let _ = MichiuWindow::init_dpi_awareness();

    let com = MichiuComContext::new_ro_single()?;

    let builder = MichiuWindowBuilder::new()
        .with_title("Sample App")
        .with_com_context(&com)
        .with_no_redirection_bitmap(true)
        .with_inner_size(LogicalSize::new(600.0, 500.0))
        .into_unvalidated();

    let window = MichiuWindow::build(builder.try_into()?)?;

    let scale_factor = window.scale_factor() as f32;
    let handle = window.handle().assume_valid();

    let renderer = pollster::block_on(MichiuRenderer::new(
        handle.hwnd(),
        LayoutSize::new(600.0, 500.0),
        scale_factor,
    ))?;

    let (mut app, mut pump) = MichiuAppBuilder::new(window).build_with_ui(renderer, move || {
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
    })?;

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
                    app.standard_redraw();
                }
                _ => {}
            }
        }
    })? {}

    Ok(())
}
