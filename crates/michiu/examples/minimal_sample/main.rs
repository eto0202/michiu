#![expect(clippy::pedantic)]

use michiu::{accessibility::a11y, prelude::*};

// cargo build --example minimal_sample --release
fn main() -> std::result::Result<(), Box<dyn std::error::Error>> {
    let _ = MichiuWindow::init_dpi_awareness();
    let com = MichiuComContext::new_ro_single()?;

    let builder = MichiuWindowBuilder::new()
        .with_title("Sample App")
        .with_visible(false)
        .with_com_context(&com)
        .with_no_redirection_bitmap(true)
        .with_inner_size(LogicalSize::new(600.0, 500.0))
        .into_unvalidated();

    let window = MichiuWindow::build(builder.try_into()?)?;

    let scale_factor = window.scale_factor() as f32;
    let handle = window.handle().assume_valid();
    let rect = handle.client_rect();

    let renderer = pollster::block_on(MichiuRenderer::new(
        handle.hwnd(),
        rect.cast(),
        scale_factor,
    ))?;

    let (mut app, mut pump) = MichiuAppBuilder::new(window)
        .with_accessibility_support(true)
        .with_auto_resolve_cursor(true)
        .build_with_ui(renderer, move || {
            let (count, set_count) = create_signal(0u32);

            flex(
                ii().size_full()
                    .justify_center()
                    .items_center()
                    .bg_color(oklch(0.209, 0.0, 0.0)),
            )
            .tag::<a11y::AMain>()
            .child(
                flex(
                    ii().r(4.0)
                        .size((200.0, 40.0))
                        .justify_center()
                        .bg_color(oklch(0.3132, 0.0, 0.0))
                        .cursor_pointer()
                        .pressed(ii().transform_scale(0.98, 0.98))
                        .hovered(ii().bg_color(oklch(0.4017, 0.0, 0.0)))
                        .focusable_self_both()
                        .focused_visible(
                            ii().outline_solid(2.0)
                                .outline_offset(2.0)
                                .outline_color(oklch(0.5433, 0.1913, 266.76)),
                        ),
                )
                .tag::<a11y::AButton>()
                .on_click(move || set_count.set(count.get() + 1))
                .label(
                    move || format!("Count: {}", count.get()),
                    ii().font_size(18.0).text_color(oklch(0.9491, 0.0, 0.0)),
                ),
            )
        })?;
    app.set_visible(true);

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
