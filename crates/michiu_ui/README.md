# michiu_ui

This crate handles UI rendering for the `michiu` crate.

## Crates.io

- [michiu](https://crates.io/crates/michiu)
- [michiu_window](https://crates.io/crates/michiu_window)
- [michiu_guard](https://crates.io/crates/michiu_guard)

## Examples

[michiu_ui examples](https://github.com/eto0202/michiu/tree/main/crates/michiu_ui/examples)

## Basic Initialization

```rust
use michiu_ui::prelude::*;

// Application state for centralized management of Context and Renderer
struct AppState {
    renderer: MichiuRenderer,
    context: Context,
    root_id: Entity,
}

// 1. Create a window and retrieve the HWND and scale factor
let hwnd = create_window();
let scale_factor = get_scale_factor();

// 2. Initialize the renderer (MichiuRenderer::new is async)
let renderer = pollster::block_on(MichiuRenderer::new(
    hwnd,
    LayoutSize::new(600.0, 500.0),
    scale_factor,
))?;

// 3. Initialize `Context`
// To enable accessibility, chain .with_accessibility(hwnd)
let mut context = Context::new();

// 4. Build the UI
let root = build_ui(&mut context, move || {
    let (count, set_count) = create_signal(0u32);

    flex(
        ii().size_full()
            .justify_center()
            .bg_color(Color::GRAY),
    )
    .on_click(move || set_count.set(count.get() + 1))
    .label(
        move || format!("Count: {}", count.get()),
        ii().text_color(Color::WHITE),
    )
});

let app_state = AppState {
    renderer,
    context,
    root_id: root.id(),
};

// 5. Event and render loop (see below)
```

## Event Processing and Rendering Lifecycle

This crate separates **input queuing** from **batch evaluation and rendering** via its pipeline.
In your event loop or window procedure, process frames as follows:

```rust
use michiu_ui::prelude::*;
use windows::Win32::Graphics::Gdi::{BeginPaint, EndPaint, InvalidateRect, PAINTSTRUCT};
use windows::Win32::Graphics::Dwm::DwmFlush;

fn win32_message(app: &mut AppState, hwnd: HWND, event: YourWindowEvent) {
    match event {
        // --- 1. Event Received (Mouse, Keyboard, etc.) ---
        // Do not run layout or redraw immediately; queue actions instead
        YourWindowEvent::MouseInput { button, state, modifiers } => {
            app.context.inject_action(EventAction::PointerButton { button, state, modifiers });
        }
        YourWindowEvent::CursorMoved { x, y } => {
            app.context.inject_action(EventAction::PointerMove(LayoutPoint::new(x, y)));
        }

        // --- 2. Redraw Requested (WM_PAINT, Resize, VSync, etc.) ---
        YourWindowEvent::RedrawRequested => {
            // 1. Flush event queue and update interaction states (hover, focus, click)
            app.context.begin_frame();

            // 2. Advance timers for animations and automated dragging
            app.context.tick_system_frame(&TickType::All);

            // 3. Recalculate layout
            // Executes a two-pass calculation to resolve scrollbar-induced container resizing
            app.context.sync_layout(app.root_id, app.renderer.layout_size());

            // 4. Synchronize DirectComposition tree for backgrounds and external visuals
            app.renderer.update_composition_tree(&mut app.context);

            // 5. Update accessibility (AccessKit/UIA) tree if active
            app.context.update_accessibility();

            // 6. Text shaping, GPU instance buffer uploads, wgpu drawing, and DirectComposition commit
            // Note: BeginPaint/EndPaint are required when handling WM_PAINT in Win32
            let mut ps = PAINTSTRUCT::default();
            let _hdc = unsafe { BeginPaint(hwnd, &mut ps) };

            app.renderer.draw(&mut app.context);

            let _ = unsafe { EndPaint(hwnd, &ps) };

            // 7. If animations are active, schedule the next frame (VSync-aligned)
            if app.context.has_active_frame() {
                let _ = unsafe { DwmFlush() };
                let _ = unsafe { InvalidateRect(Some(hwnd), None, false) };
            }
        }
    }
}
```

## State Management

#### 1. Signal Approach (Declarative & Reactive)

Value updates automatically propagate to subscribing property closures.

```rust
use michiu_ui::prelude::*;

let (count, set_count) = create_signal(0u32);

div_n().on_click(move || set_count.set(count.get() + 1))
       .label(move || format!("Signal: {}", count.get()), ii())
```

#### 2. Direct Approach (Local Variable Capture)

Move the `Entity` directly into an event closure (such as `on_click`) to manipulate it imperatively.

```rust
use michiu_ui::prelude::*;

let e = h_flex(ii().bg_color(Color::BLUE));
let mut count = 0u32;

e.label(move || format!("Direct: {count}"), ii())
  .on_click(move || {
       count += 1;
       e.set_contents(div_n().label(move || format!("Direct: {count}"), ii()))
         .modify(ii().bg_color(Color::RED));
  })
```

#### 3. Event Context Approach (Current Entity Manipulation)

Access the event target (`cx.current()`) directly from the handler argument `cx: &mut Context` (`_with` variants).

```rust
use michiu_ui::prelude::*;

let mut count = 0u32;

h_flex(ii().bg_color(Color::BLUE))
    .label(move || format!("Event: {count}"), ii())
    .on_click_with(move |cx| {
         count += 1;
         cx.current().set_contents(div_n().label(move || format!("Event: {count}"), ii()))
           .modify(ii().bg_color(Color::RED));
    })
```

#### 4. Tag Query Approach (Type-Safe Tree Lookups)

The Tag system attaches zero-sized marker types to entities, allowing you to query and mutate them from anywhere in the tree without knowing the hierarchy.

```rust
use michiu_ui::prelude::*;

let mut count = 0u32;

let left = flex(ii()).tag::<a11y::AButton>();

let right = flex(ii()).label("Click", ii()).on_click_with(move |cx| {
    let e = cx.query_first::<a11y::AButton>();
    count += 1;
    e.set_contents(div_n().label(move || format!("Tag: {count}"), ii()));
});

h_flex(ii()).children([left, right])
```

#### Additional information

Marker types like `a11y::AButton` serve double duty: they convey UI Automation roles (via AccessKit) and act as type-safe query keys. Custom tags can be created using the `A11yInferenceTag` trait.

---

## Reactivity and Context Provision

Calling `.provide()` on a signal upstream in the tree provides state down to any descendant component without manual prop drilling.

### Basic Usage

```rust
use michiu_ui::prelude::*;

// 1. Create and provide signals at the root
pub fn create_root() -> Entity {
    let (theme, _) = create_signal(Theme::dark());
    let (search_text, _) = create_signal(SearchText(String::new()));

    v_flex(move || ii().size_full().bg_color(theme.get().background))
        .provide(theme)
        .provide(search_text)
        .children([
            header(),
            sidebar(),
            main_area(),
        ])
}

// 2. Subscribe from any nested child
fn some_label() -> Entity {
    label("Hello", ii().text_color(dynamic(|t: &Theme| t.primary)))
}
```

### Quick Reference

| Scenario                               | Recommended Approach                   |
| :------------------------------------- | :------------------------------------- |
| Fully static styles                    | `.modify(ii())`                         |
| Styles with conditional logic          | `.modify(\|\| ii())`                    |
| 1–2 dynamic properties                 | `dynamic(\|t: &T\|)`                   |
| Many dynamic properties                | `.modify_d(\|t: &T\|)`                  |
| Dynamic properties + conditional logic | `.modify_d`                             |
| Static layout + few dynamic properties | `modify` + `dynamic`                    |
| Extensive static + dynamic properties  | `.modify(static)` + `.modify_d(dynamic)` |
