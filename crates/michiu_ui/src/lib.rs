#![expect(
    clippy::similar_names,
    clippy::struct_field_names,
    clippy::too_many_arguments,
    clippy::too_many_lines,
    clippy::struct_excessive_bools,
    clippy::doc_markdown
)]

//! # `michiu_ui`
//!
//! `michiu_ui` is the core engine of the Windows GUI library [`michiu`](https://crates.io/crates/michiu).
//! It handles UI tree construction, reactive state management via signals, layout calculations, and rendering/compositing using wgpu and DirectComposition.
//!
//! > **Note:**
//! > For general GUI application development, we recommend using the top-level crate [`michiu`](https://crates.io/crates/michiu), which integrates window creation and message loops.
//!
//! ---
//!
//! ## Summary
//!
//! The pipeline in this crate consists primarily of four core entities:
//!
//! 1. **Preparing the Renderer**:
//!    [`MichiuRenderer`] is a backend integrating DirectComposition and wgpu (DirectX 12). It is initialized asynchronously by passing a `HWND` created by any windowing backend.
//! 2. **Initializing [`Context`]**:
//!    The central struct managing UI state, style definitions, input queues, and layout caches. Accessibility (AccessKit/UIA) bindings are also configured here.
//! 3. **Building the UI**:
//!    Declaratively construct the UI tree using [`build_ui`].
//! 4. **Event Queuing and Rendering Cycle**:
//!    Queue OS events using [`inject_action`](Context::inject_action), then execute the pipeline during frame rendering to draw to the renderer.
//!
//! ## Basic Initialization
//!
//! ```rust,ignore
//! use michiu_ui::prelude::*;
//!
//! // Application state for centralized management of Context and Renderer
//! struct AppState {
//!     renderer: MichiuRenderer,
//!     context: Context,
//!     root_id: Entity,
//! }
//!
//! // 1. Create a window and retrieve the HWND and scale factor
//! let hwnd = create_window();
//! let scale_factor = get_scale_factor();
//!
//! // 2. Initialize the renderer (MichiuRenderer::new is async)
//! let renderer = pollster::block_on(MichiuRenderer::new(
//!     hwnd,
//!     LayoutSize::new(600.0, 500.0),
//!     scale_factor,
//!     None,
//! ))?;
//!
//! // 3. Initialize `Context`
//! // To enable accessibility, chain .with_accessibility(hwnd)
//! let mut context = Context::new();
//!
//! // 4. Build the UI
//! let root = build_ui(&mut context, move || {
//!     let (count, set_count) = create_signal(0u32);
//!
//!     flex(
//!         ii().size_full()
//!             .justify_center()
//!             .bg_color(rgb(22, 24, 29)),
//!     )
//!     .on_click(move || set_count.set(count.get() + 1))
//!     .label(
//!         move || format!("Count: {}", count.get()),
//!         ii().text_color(rgb(156, 158, 163)),
//!     )
//! });
//!
//! let app_state = AppState {
//!     renderer,
//!     context,
//!     root_id: root.id(),
//! };
//!
//! // 5. Event and render loop (see below)
//! ```
//!
//! ---
//!
//! ## Event Processing and Rendering Lifecycle
//!
//! This crate separates **input queuing** from **batch evaluation and rendering** via its pipeline.
//! In your event loop or window procedure, process frames as follows:
//!
//! ```rust,ignore
//! use michiu_ui::prelude::*;
//! use windows::Win32::Graphics::Gdi::{BeginPaint, EndPaint, InvalidateRect, PAINTSTRUCT};
//! use windows::Win32::Graphics::Dwm::DwmFlush;
//!
//! fn win32_message(app: &mut AppState, hwnd: HWND, event: YourWindowEvent) {
//!     match event {
//!         // --- 1. Event Received (Mouse, Keyboard, etc.) ---
//!         // Do not run layout or redraw immediately; queue actions instead
//!         YourWindowEvent::MouseInput { button, state, modifiers } => {
//!             app.context.inject_action(EventAction::PointerButton { button, state, modifiers });
//!         }
//!         YourWindowEvent::CursorMoved { x, y } => {
//!             app.context.inject_action(EventAction::PointerMove(LayoutPoint::new(x, y)));
//!         }
//!
//!         // --- 2. Redraw Requested (WM_PAINT, Resize, VSync, etc.) ---
//!         YourWindowEvent::RedrawRequested => {
//!             // 1. Flush event queue and update interaction states (hover, focus, click)
//!             app.context.begin_frame();
//!
//!             // 2. Advance timers for animations and automated dragging
//!             app.context.tick_system_frame(&TickType::All);
//!
//!             // 3. Recalculate layout
//!             // Executes a two-pass calculation to resolve scrollbar-induced container resizing
//!             app.context.sync_layout(app.root_id, app.renderer.layout_size());
//!
//!             // 4. Synchronize DirectComposition tree for backgrounds and external visuals
//!             app.renderer.update_composition_tree(&mut app.context);
//!
//!             // 5. Update accessibility (AccessKit/UIA) tree if active
//!             app.context.update_accessibility();
//!
//!             // 6. Text shaping, GPU instance buffer uploads, wgpu drawing, and DirectComposition commit
//!             // Note: BeginPaint/EndPaint are required when handling WM_PAINT in Win32
//!             let mut ps = PAINTSTRUCT::default();
//!             let _hdc = unsafe { BeginPaint(hwnd, &mut ps) };
//!
//!             app.renderer.draw(&mut app.context);
//!
//!             let _ = unsafe { EndPaint(hwnd, &ps) };
//!
//!             // 7. If animations are active, schedule the next frame (VSync-aligned)
//!             if app.context.has_active_frame() {
//!                 let _ = unsafe { DwmFlush() };
//!                 let _ = unsafe { InvalidateRect(Some(hwnd), None, false) };
//!             }
//!         }
//!     }
//! }
//! ```
//!
//! ### Pipeline Phase Responsibilities
//!
//! | Phase | Method | Primary Responsibility |
//! | :--- | :--- | :--- |
//! | **1. Input Evaluation** | `begin_frame` | Dispatches queued `EventAction`s, firing signals and handlers |
//! | **2. System Update** | `tick_system_frame` | Updates time-based state such as animations and momentum |
//! | **3. Layout Finalization** | `sync_layout` | Resolves absolute coordinates via two-pass Flexbox evaluation |
//! | **4. Hardware Composition** | `update_composition_tree` | Syncs transformations and placement of external Visuals |
//! | **5. Accessibility** | `update_accessibility` | Emits structural diffs to screen readers and assistive tech |
//! | **6. Rendering** | `draw` | Shapes text, uploads GPU buffers, and encodes draw calls |
//! | **7. Next Frame Request** | `has_active_frame` | Requests a repaint if animations or transitions are ongoing |
//!
//! For internal phase transitions, see the [`pipeline`] module documentation.
//!
//! ---
//!
//! ## Architecture and Design Philosophy
//!
//! ### 1. Why DirectComposition? (Hardware Composition and External Visuals)
//! Most GUI frameworks either render entirely onto a single surface or stack heavyweight OS child windows (HWNDs).
//! `michiu_ui` is built directly on top of the DirectComposition visual tree.
//!
//! Native `IDCompositionVisual2` instances produced by external sources (video players, image loaders, or native controls) can be embedded directly beneath the wgpu UI layer.
//! This delivers true hardware-level compositing without texture copies or CPU overhead.
//!
//! ### 2. State Management via Data-Oriented Design (DOD)
//! The internal architecture of `Context` is built on a flat Data-Oriented Design.
//!
//! An `Entity` is not a heavy, heap-allocated UI object; it is merely a lightweight index key ([`Entity`]) pointing into an internal Structure of Arrays (SoA).
//! Because of this, you are free from Rust's typical self-referential borrowing constraints and can freely mix four distinct paradigms:
//!
//! | Approach | Key Benefit | Recommended Use Case |
//! | :--- | :--- | :--- |
//! | **1. Signal** | Complete state/view separation | Complex business logic, global state |
//! | **2. Direct** | Zero boilerplate, low overhead | Simple toggles, ephemeral local state |
//! | **3. Event Context** | Self-contained components | Handling reactions to user events |
//! | **4. Tag Query** | Type-safe loose coupling | Direct manipulation of sibling or external entities |
//!
//! #### 1. Signal Approach (Declarative & Reactive)
//! Value updates automatically propagate to subscribing property closures.
//! ```rust,ignore
//! use michiu_ui::prelude::*;
//!
//! let (count, set_count) = create_signal(0u32);
//!
//! div_n().on_click(move || set_count.set(count.get() + 1))
//!        .label(move || format!("Signal: {}", count.get()), ii())
//! ```
//!
//! #### 2. Direct Approach (Local Variable Capture)
//! Move the `Entity` directly into an event closure (such as `on_click`) to manipulate it imperatively.
//! ```rust,ignore
//! use michiu_ui::prelude::*;
//!
//! let e = h_flex(ii().bg_color(Color::BLUE));
//! let mut count = 0u32;
//!
//! e.label(move || format!("Direct: {count}"), ii())
//!   .on_click(move || {
//!        count += 1;
//!        e.set_contents(div_n().label(move || format!("Direct: {count}"), ii()))
//!          .modify(ii().bg_color(Color::RED));
//!   })
//! ```
//!
//! #### 3. Event Context Approach (Current Entity Manipulation)
//! Access the event target (`cx.current()`) directly from the handler argument `cx: &mut Context` (`_with` variants).
//! ```rust,ignore
//! use michiu_ui::prelude::*;
//!
//! let mut count = 0u32;
//!
//! h_flex(ii().bg_color(Color::BLUE))
//!     .label(move || format!("Event: {count}"), ii())
//!     .on_click_with(move |cx| {
//!          count += 1;
//!          cx.current().set_contents(div_n().label(move || format!("Event: {count}"), ii()))
//!            .modify(ii().bg_color(Color::RED));
//!     })
//! ```
//!
//! #### 4. Tag Query Approach (Type-Safe Tree Lookups)
//! The Tag system attaches zero-sized marker types to entities, allowing you to query and mutate them from anywhere in the tree without knowing the hierarchy.
//!
//! In conventional declarative UIs, modifying an ancestor or sibling from a deeply nested child requires lifting state and **prop drilling** callbacks through every intermediate layer.
//!
//! In `michiu_ui`, thanks to flat ID management, an isolated child component can directly and imperatively mutate other entities across the tree.
//! ```rust,ignore
//! use michiu_ui::prelude::*;
//!
//! let mut count = 0u32;
//!
//! let left = flex(ii()).tag::<a11y::AButton>();
//!
//! let right = flex(ii()).label("Click", ii()).on_click_with(move |cx| {
//!     let e = cx.query_first::<a11y::AButton>();
//!     count += 1;
//!     e.set_contents(div_n().label(move || format!("Tag: {count}"), ii()));
//! });
//!
//! h_flex(ii()).children([left, right])
//! ```
//!
//! #### Advantages
//! 1. **Complete Decoupling**:
//!    The triggering entity (`right`) needs zero knowledge of the tree layout; it only targets entities tagged with `a11y::AButton`.
//! 2. **Unified Accessibility (a11y)**:
//!    Marker types like [`a11y::AButton`] serve double duty: they convey UI Automation roles (via AccessKit) and act as type-safe query keys. Custom tags can be created using the [`A11yInferenceTag`](crate::a11y::A11yInferenceTag) trait.
//!
//! ---
//!
//! # Reactivity and Context Provision
//!
//! Calling `.provide()` on a signal upstream in the tree provides state down to any descendant component without manual prop drilling.
//!
//! ## Basic Usage
//!
//! ```rust,ignore
//! use michiu_ui::prelude::*;
//!
//! // 1. Create and provide signals at the root
//! pub fn create_root() -> Entity {
//!     let (theme, _) = create_signal(Theme::dark());
//!     let (search_text, _) = create_signal(SearchText(String::new()));
//!
//!     v_flex(move || ii().size_full().bg_color(theme.get().background))
//!         .provide(theme)
//!         .provide(search_text)
//!         .children([
//!             header(),
//!             sidebar(),
//!             main_area(),
//!         ])
//! }
//!
//! // 2. Subscribe from any nested child
//! fn some_label() -> Entity {
//!     label("Hello", ii().text_color(dynamic(|t: &Theme| t.primary)))
//! }
//! ```
//!
//! ---
//!
//! ## Choosing Between Tags and Signals (Provide)
//!
//! * **Use Tags for**: Event-driven, localized, and imperative interactions (e.g., closing a context menu or targeting a specific sibling on click).
//! * **Use Signals + Provide for**: Globally or sub-tree shared state that should automatically cascade downwards (e.g., themes, search queries, locale).
//!
//! ---
//!
//! # Dynamic Styling and Performance Guide
//!
//! `michiu_ui` offers two patterns for dynamic styling: **block-level evaluation (`modify_d` / `modify`)** and **per-property evaluation (`dynamic`)**.
//!
//! ---
//!
//! ## 1. Equivalence of `modify(move || ...)` and `modify_d`
//!
//! The following two patterns are identical in internal behavior and performance:
//!
//! ```rust,ignore
//! // Pattern A: Passing a closure directly to `modify`
//! .modify(move || {
//!     let t = use_provided::<Theme>().get();
//!     ii().bg_color(t.background)
//! })
//!
//! // Pattern B: Using the `modify_d` shorthand
//! .modify_d(|t: &Theme| {
//!     ii().bg_color(t.background)
//! })
//! ```
//!
//! * **Same Effect Category**: Both register under `EffectCategory::Style` as `Prop::Dynamic`.
//! * **Identical Dependency Tracking**: Calling `.get()` on `Theme` during closure evaluation establishes identical subscription links.
//!
//! ---
//!
//! ## 2. Block-Level (`modify_d`) vs. Per-Property (`dynamic`)
//!
//! | Metric | Block-Level (`modify_d`) | Per-Property (`dynamic`) |
//! | :--- | :--- | :--- |
//! | **Evaluation Unit** | Entire `Modifier` object | Individual properties (color, size, etc.) |
//! | **Tree Traversals (`use_provided`)** | **Once** (resolved at start of closure) | **Once per property** (resolved individually) |
//! | **Re-evaluation Behavior** | Regenerates `Modifier` and bulk-merges (`override_with`) the SoA | Executes individual dynamic setters directly into SoA fields |
//! | **Memory & CPU Characteristics** | Optimized for bulk memory assignment and cache efficiency | Zero struct allocations; minimal overhead |
//!
//! > **Note:**
//! > In `michiu_ui`, SoA merging consists entirely of continuous assignment of simple memory values without heap allocations.
//! > When modifying many properties, traversing the ancestor tree once and performing a bulk memory copy is faster than traversing the tree 10 times to invoke 10 individual closures.
//!
//! ---
//!
//! ## 3. Practical Recommendations
//!
//! ### Rule A: Few dynamic values (1–2 properties)
//! * **Recommended**: **`dynamic()` per property**
//! * **Reason**: Bypasses `Modifier` generation and SoA re-merging entirely.
//!
//! ```rust,ignore
//! div(ii().bg_color(dynamic(|t: &Theme| t.background)))
//! ```
//!
//! ### Rule B: Many dynamic values (borders, backgrounds, text colors, etc.)
//! * **Recommended**: **`modify_d()` block-level**
//! * **Reason**:
//!   - **Ancestral lookup reduction**: Resolves `use_provided` only once, saving CPU cycles on deep trees.
//!   - **Cache locality**: Direct bulk assignment beats sequential call-stack evaluation of multiple closures.
//!   - **Readability**: Keeps static layout and dynamic styling cleanly separated.
//!
//! ```rust,ignore
//! div(ii().w_full().p_y(4.0))
//!     .modify_d(|t: &Theme| {
//!         ii().border_color(t.border)
//!             .bg_color(t.background)
//!             .pressed(ii().border_color(t.primary).bg_color(t.background_hover))
//!     })
//! ```
//!
//! ---
//!
//! ## 4. Quick Reference Matrix
//!
//! | Scenario | Recommended Approach |
//! | :--- | :--- |
//! | Fully static styles | `modify(ii())` |
//! | Styles with conditional logic | `modify(\|\| ii())` |
//! | 1–2 dynamic properties | `dynamic(\|t: &T\|)` |
//! | Many dynamic properties | `modify_d(\|t: &T\|)` |
//! | Dynamic properties + conditional logic | `modify_d` |
//! | Static layout + few dynamic properties | `modify` + `dynamic` |
//! | Extensive static + dynamic properties | `modify(static)` + `modify_d(dynamic)` |
//!
//! ---
//!
//! ## Detailed Guide (Not Yet Implemented)
//!
//! For detailed specifications and internal architecture of each subsystem, refer to the sections below.
//!
//! * [**Renderer Implementation**](crate::renderer) - DirectComposition surface management and rendering command generation via wgpu
//! * [**`Context` Structure**](crate::context) - Global state maintenance, storage management, and lifecycle control
//! * [**`pipeline` module**](crate::pipeline) - Frame progression orchestrator
//! * [**UI Construction and Styling**](crate::entity) - Widget tree construction via [`build_ui`] and the list of properties for `ii()` (style builder)
//! * [**Reactivity**](crate::signal) - Reactive programming model and dependency tracking via [`create_signal`]
//! * [**Tag Feature**](crate::tag) - Assigning identification tags to entities, and tree traversal and querying
//! * [**Accessibility**](crate::context::acce_store) - Integration with AccessKit and tree synchronization
//! * [**Embedding External entities**](crate::external) - Integration of images, videos, external files, and `IDCompositionVisual2`
//! * [**Debugging Tools**](crate::context::debug_store) - Collection of logs (including errors) and visualization of the lifecycle

mod bitmap;
pub mod context;
pub mod entity;
pub mod external;
mod modifier;
mod option;
pub mod renderer;
pub mod signal;
pub mod tag;
mod types;
mod utils;

#[doc(inline)]
pub use bitmap::*;
#[doc(inline)]
pub use context::*;
#[doc(inline)]
pub use entity::*;
#[doc(inline)]
pub use external::*;
#[doc(inline)]
pub use modifier::*;
#[doc(inline)]
#[allow(unused)] // なぜか必要
pub use option::*;
#[doc(inline)]
pub use renderer::*;
#[doc(inline)]
pub use signal::*;
#[doc(inline)]
pub use soa::MichiuSoA;
#[doc(inline)]
pub use tag::*;
#[doc(inline)]
pub use types::*;
#[doc(inline)]
pub use utils::*;

#[doc(hidden)]
pub mod accessibility {
    pub const NO_A11Y_NODE: crate::Prop<Option<accesskit::Node>> = crate::Prop::None;
    pub use crate::a11y::{self, A11yInferenceTag};
    pub use accesskit::{Action, Node, NodeId, Role};
}

#[doc(hidden)]
pub mod config {
    pub use crate::CapacityConfig;
    pub use crate::{CompositionTargetLayer, RendererUserConfig};
    pub use wgpu::{MemoryHints, PresentMode};
}

#[doc(hidden)]
pub mod prelude {
    pub use crate::{
        animation::PropertyList,
        context::{Context, MichiuInspector},
        dnd::{DndDragMode, DndDropTarget},
        entity::{Entity, build_ui},
        input::InputContents,
        m,
        modifier::Modifier,
        pipeline::{EventAction, TickType},
        renderer::MichiuRenderer,
        scrollbar::{ScrollbarDisplay, ScrollbarMode, ScrollbarStyle},
        signal::{ReadSignal, SignalId, WriteSignal},
        types::{
            AnimationCurve, Backdrop, BorderAlignment, BorderStyle, Color, CursorIcon, Display,
            ElementState, InteractionState, KeyframeAnimation, LayoutPoint, LayoutRect, LayoutSize,
            Modifiers, MouseButton, PlaybackCount, PointerEvents, Transform, Transition,
        },
        utils::{
            auto, blur, create_signal, div, div_d, div_n, dynamic, external_texture, fill, flex,
            flex_d, get_win32_clipboard, grid_box, h_flex, h_flex_d, hex, hidden_box, hsl, hsla,
            ii, input, input_area, input_area_d, input_d, offset, oklab, oklaba, oklch, oklcha,
            pct, px, rgb, rgba, set_win32_clipboard, shadow, spread, text, text_d, use_provided,
            use_provided_setter, v_flex, v_flex_d,
        },
    };
}
