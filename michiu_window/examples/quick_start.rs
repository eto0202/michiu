use michiu_window::{Event, EventPump, MichiuEvent, Window, WindowBuilder, init_dpi_awareness};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    // 1. Initialize High-DPI support (Per-Monitor v2)
    init_dpi_awareness();

    // 2. Configure, validate, and build a simple window
    let builder = WindowBuilder::new().with_title("Michiu Minimal Window");
    let validated = builder.into_unvalidated().try_into()?;
    let window = Window::build(validated)?;
    // It should be safe since the window was just created. Probably.
    let handle = window.handle().assume_valid();

    // 3. Initialize the EventPump to drive the message loop on the UI thread
    let mut event_pump = EventPump::new();

    // 4. Event-driven loop using `wait_event()`
    while let Some(event) = event_pump.wait_event()? {
        if let MichiuEvent::Window { event, .. } = event {
            match event {
                Event::CloseRequested => {
                    // Destroy the window directly from the event loop using the handle
                    handle.destroy();
                }
                Event::Destroyed => {
                    // Exit the loop cleanly after the window is fully destroyed
                    break;
                }
                _ => {}
            }
        }
    }
    Ok(())
}
