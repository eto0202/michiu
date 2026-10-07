# michiu_window

This crate handles the windows for the `michiu` crate.

## Examples

[michiu_window examples](https://github.com/eto0202/michiu/tree/main/michiu_window/examples)

## Quick Start

```rust,no_run
use michiu_window::{Event, EventPump, MichiuEvent, Window, WindowBuilder, init_dpi_awareness};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    // 1. Initialize High-DPI support (Per-Monitor v2)
    init_dpi_awareness();

    // 2. Configure, validate, and build a simple window
    let builder = WindowBuilder::new().with_title("Michiu Minimal Window");
    // Validate the builder settings here to guarantee window safety.
    let validated = builder.into_unvalidated().try_into()?;
    let window = Window::build(validated)?;
    // Explicitly assume the handle is valid (since the window was just built).
    let handle = window.handle().assume_valid();

    // 3. Initialize the EventPump to drive the message loop on the UI thread
    let mut event_pump = EventPump::new();

    // 4. Event-driven loop using `wait_event()`
    while let Some(event) = event_pump.wait_event()? {
        // Note that MichiuEvent also includes a User variant, not just Window.
        if let MichiuEvent::Window { event, .. } = event {
            match event {
                Event::CloseRequested => {
                    // You can also handle close confirmation logic here.
                    // Destroy the window directly from the event loop using the handle
                    handle.destroy();
                }
                Event::Destroyed => {
                    // Post-processing after the window is completely destroyed.
                    // Exit the event loop.
                    break;
                }
                _ => {}
            }
        }
    }
    Ok(())
}
```
