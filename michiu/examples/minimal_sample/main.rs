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

// cargo build --example minimal_sample --release
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
                    let size = phy_size.assume_valid().into_inner();
                    let width = size.width as u32;
                    let height = size.height as u32;

                    app.renderer
                        .resize((width, height), app.renderer.scale_factor());
                    handle.redraw_requested();
                }
                Event::RedrawRequested => {
                    app.context.begin_frame();
                    app.context.tick_system_frame(&TickType::All);

                    app.context
                        .sync_layout(app.root_id, app.renderer.layout_size());

                    app.renderer.update_composition_tree(&mut app.context);

                    if let Some(_ctx) = handle.begin_paint() {
                        app.renderer.draw(&mut app.context);
                    }
                    if app.context.has_active_frame() {
                        handle.dwm_flush();
                        handle.redraw_requested();
                    }
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
                    handle.redraw_requested();
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
                    handle.redraw_requested();
                }
                _ => {}
            }
        }
    })? {}

    Ok(())
}
