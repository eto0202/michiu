# michiu

[![Crates.io](https://img.shields.io/crates/v/michiu.svg)](https://crates.io/crates/michiu)
[![Docs.rs](https://docs.rs/michiu/badge.svg)](https://docs.rs/michiu)
[![License](https://img.shields.io/badge/license-MIT%20OR%20Apache--2.0-blue.svg)](LICENSE)
[![GitHub Actions CI](https://img.shields.io/github/actions/workflow/status/eto0202/michiu/ci.yml?branch=main&label=CI)](https://github.com/eto0202/michiu/actions)
[![Platform](https://img.shields.io/badge/platform-Windows-0078d7.svg?logo=windows&logoColor=white)](https://github.com/eto0202/michiu)

[![Language Policy](https://img.shields.io/badge/Policy-Multilingual-purple)](LANGUAGE_POLICY.md)
[![Accessibility](https://img.shields.io/badge/A11y-AccessKit-pink)](ACCESSIBILITY.md)
[![Good First Issues](https://img.shields.io/github/issues/eto0202/michiu/good%20first%20issue?color=green&label=good%20first%20issues)](https://github.com/eto0202/michiu/issues?q=is%3Aopen+is%3Aissue+label%3A%22good+first+issue%22)

The `michiu` is developing a GUI library for Windows.

### Crates.io

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
michiu = "0.2.2"
pollster = "1.0.1"
```

Then in src/main.rs:

```rust
use michiu::{accessibility::a11y, prelude::*};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Preventing Screen Blur
    let _ = MichiuWindow::init_dpi_awareness();
    // COM Initialization for WinRT/DirectComposition
    let com = MichiuComContext::new_ro_single()?;

    // 1. Create window
    let builder = MichiuWindowBuilder::new()
        .with_title("Sample App")
        .with_visible(false)
        .with_com_context(&com)
        .with_no_redirection_bitmap(true) // Required when using DirectComposition
        .with_inner_size(LogicalSize::new(600.0, 500.0))
        .into_unvalidated(); // ↓
                             // Type-Safe Verification by michiu_guard
    let window = MichiuWindow::build(builder.try_into()?)?;

    // 2. Initialize renderer
    let scale_factor = window.scale_factor() as f32;
    let handle = window.handle().assume_valid();
    let rect = handle.client_rect();
    // MichiuRenderer::new is async. Use any executor of your choice.
    let renderer = pollster::block_on(MichiuRenderer::new(
        handle.hwnd(),
        rect.cast(),
        scale_factor,
    ))?;

    // 3. Build UI and application host
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
        src="https://github.com/user-attachments/assets/40a4db29-c702-488d-ac6e-c846c63221d5" />
</p>

#### WebView2 (External Visual) Sample

<p align="center">
    <img 
        width="600" 
        alt="WebView2 (External Visual) Sample" 
        src="https://github.com/user-attachments/assets/f988e5fd-667f-4e53-9d48-b6a9688a4a9d" />
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

### Attribution Waiver for `examples/`

When copying, adapting, or referencing the example code located in the `examples/` directory in your own projects, **no attribution, copyright notice, or inclusion of the license terms is required**, for both commercial and non-commercial use. Feel free to use them freely as templates or boilerplate.
