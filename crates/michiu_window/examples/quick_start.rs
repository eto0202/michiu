use michiu_window::{
    MichiuAnyEvent, MichiuEvent, MichiuEventPump, MichiuWindow, MichiuWindowBuilder,
};

// cargo run --example quick_start

fn main() -> Result<(), Box<dyn std::error::Error>> {
    // 1. Initialize High-DPI support (Per-Monitor v2)
    let _ = MichiuWindow::init_dpi_awareness();

    // 2. Configure, validate, and build a simple window
    let builder = MichiuWindowBuilder::new().with_title("Michiu Minimal Window");
    // Validate the builder settings here to guarantee window safety.
    let validated = builder.into_unvalidated().try_into()?;
    let window = MichiuWindow::build(validated)?;
    // Explicitly assume the handle is valid (since the window was just built).
    let handle = window.handle().assume_valid();

    // 3. Initialize the EventPump to drive the message loop on the UI thread
    let mut event_pump = MichiuEventPump::new();

    // 4. Event-driven loop using `wait_event()`
    while event_pump.wait_event(|event, _, _| {
        // Note that MichiuEvent also includes a User variant, not just Window.
        if let MichiuAnyEvent::Window { event, .. } = event {
            match event {
                MichiuEvent::CloseRequested => {
                    // You can also handle close confirmation logic here.
                    // Destroy the window directly from the event loop using the handle
                    handle.destroy();
                }
                MichiuEvent::Destroyed => {
                    // Post-processing after the window is completely destroyed.
                    // Exit the event loop.
                    handle.quit();
                }
                _ => {}
            }
        }
    })? {}

    Ok(())
}
