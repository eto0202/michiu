# michiu

[![Crates.io](https://img.shields.io/crates/v/michiu.svg)](https://crates.io/crates/michiu)
[![Docs.rs](https://docs.rs/michiu/badge.svg)](https://docs.rs/michiu)
[![License](https://img.shields.io/badge/license-MIT%20OR%20Apache--2.0-blue.svg)](LICENSE)
[![GitHub Actions Workflow Status](https://img.shields.io/github/actions/workflow/status/eto0202/michiu/ci.yml?branch=main&label=CI)](https://github.com/eto0202/michiu/actions)

The `michiu` is developing a GUI library for Windows.

Please note that since this is still under development, breaking changes may occur without prior notice.

- [michiu](https://crates.io/crates/michiu)
- [michiu_ui](https://crates.io/crates/michiu_ui)
- [michiu_window](https://crates.io/crates/michiu_window)
- [michiu_guard](https://crates.io/crates/michiu_guard)

### Pronunciation

`michiu` is pronounced roughly like "mee-chee-oo" (written in Japanese as 「みちぅ」).

## Quick Start

Add the following to your `Cargo.toml`:

```toml
[dependencies]
michiu = "0.0.3"
pollster = "1.0.1"
```

Then in src/main.rs:

```rust
use michiu::prelude::*;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Preventing Screen Blur
    let _ = MichiuWindow::init_dpi_awareness();
    // COM Initialization for WinRT/DirectComposition
    let com = MichiuComContext::new_ro_single()?;

    // 1. Create window
    let builder = MichiuWindowBuilder::new()
        .with_title("Sample App")
        .with_com_context(&com)
        .with_no_redirection_bitmap(true) // Required when using DirectComposition
        .with_inner_size(LogicalSize::new(600.0, 500.0))
        .into_unvalidated(); // ↓
                             // Type-Safe Verification by michiu_guard
    let window = MichiuWindow::build(builder.try_into()?)?;

    // 2. Initialize renderer
    let scale_factor = window.scale_factor() as f32;
    let handle = window.handle().assume_valid();
    // MichiuRenderer::new is async. Use any executor of your choice.
    let renderer = pollster::block_on(MichiuRenderer::new(
        handle.hwnd(),
        LayoutSize::new(600.0, 500.0),
        scale_factor,
    ))?;

    // 3. Build UI and application host
    let (mut app, mut pump) = MichiuAppBuilder::new(window).build_with_ui(renderer, move || {
         // Initialize the signal.
        let (count, set_count) = create_signal(0u32);

        flex(
            ts().size_full()
                .justify_center()
                .items_center()
                .bg_color(rgb(22, 24, 29)))
        .child(
            flex(
                ts().size((200.0, 100.0))
                    .r(6.0)
                    .justify_center()
                    .border_solid(2.0)
                    .border_color(rgb(209, 92, 174))
                    .pressed(ts().transform_scale(0.98, 0.98)),
             )
             .on_click(move || set_count.set(count.get() + 1))
             .label(
                 move || format!("Count: {}", count.get()),
                 ts().text_color(rgb(156, 158, 163)).font_size(22.0),
             ),
         )
     })?;

    // 4. Main event loop
    while pump.wait_event(|event, _, raw| {
        let resp = app.standard_handle_window_event(&event, &raw);

        if resp.needs_redraw { app.redraw_requested(); }
        if resp.needs_update_window { app.update_window(); }
        if resp.consumed { return; }

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
```

Run the application:

```
cargo run --release
```

## Screenshots

#### Quick Start Sample

<p align="center">
    <img 
        width="600" 
        alt="Quick Start Sample" 
        src="https://github.com/user-attachments/assets/55aad95b-4c9c-4d68-9fc9-3ce441cb436f" />
</p>

#### Flexbox Sample

<p align="center">
    <img 
        width="600" 
        alt="Flexbox Sample" 
        src="https://github.com/user-attachments/assets/2f99f893-5ebf-4996-b636-59d0455b3c5b" />
</p>

#### WebView2 (External Visual) Sample

<p align="center">
    <img 
        width="600" 
        alt="WebView2 (External Visual) Sample" 
        src="https://github.com/user-attachments/assets/098367bc-6758-470f-9822-1be7a944bcc1" />
</p>

## License

Licensed under either of

- Apache License, Version 2.0 ([LICENSE-APACHE](LICENSE-APACHE) or http://www.apache.org/licenses/LICENSE-2.0)
- MIT license ([LICENSE-MIT](LICENSE-MIT) or http://opensource.org/licenses/MIT)

at your option.

### Contribution

Unless you explicitly state otherwise, any contribution intentionally submitted
for inclusion in the work by you, as defined in the Apache-2.0 license, shall be
dual licensed as above, without any additional terms or conditions.
