//! # Michiu
//!
//! **Michiu** is a modular, reactive GUI toolkit for Windows built on `DirectComposition`, `wgpu`, and AccessKit.
//!
//! This crate serves as the **facade crate**, seamlessly unifying windowing ([`michiu_window`]),
//! UI core logic and rendering ([`michiu_ui`]), and validation guards ([`michiu_guard`]) into a cohesive API.
//!
//! ## Core Philosophy
//! - **User-Driven Lifecycle**: You retain full ownership of the message loop and application lifecycle.
//! - **Standard Pipelines with Escape Hatches**: Ready-to-use helpers ([`MichiuApp::standard_handle_window_event`], [`MichiuApp::standard_redraw`]) eliminate boilerplate while leaving internal state accessible for low-level Win32/DirectX integration.
//! - **Thread-Safe Background Interop**: Safe, ergonomic channels to update UI state, signals, and windows from worker threads without manual waker plumbing.
//!
//! ## Application Setup & Initialization
//!
//! Windows GUI applications require a few one-time setup steps at the very beginning of `main()`:
//!
//! 1. **High-DPI Awareness ([`MichiuWindow::init_dpi_awareness`])**:
//!    Enables Per-Monitor DPI v2. Must be called before creating any windows.
//!    Without this, Windows will stretch and blur the UI on high-DPI displays (e.g., 4K monitors or laptops at 125%/150% scaling).
//!
//! 2. **COM / `WinRT` Context ([`MichiuComContext`])**:
//!    `DirectComposition` and modern Windows APIs require the thread to be initialized for WinRT/COM.
//!    Michiu provides an RAII guard ([`MichiuComContext`]) to safely manage this lifetime:
//!    - **Default (Most Apps)**: Use [`MichiuComContext::new_ro_single()`] (`WinRT` STA).
//!    - **With File Drag & Drop**: Use [`MichiuComContext::new_winrt_ole_combo()`] (Initializes `WinRT` followed by `OLE` for shell D&D support).
//!
//!    Keep the returned `_com` guard alive for the duration of `main()`.
//!
//! 3. **Disable Redirection Bitmap ([`with_no_redirection_bitmap(true)`](crate::window::MichiuWindowBuilder::with_no_redirection_bitmap))**:
//!    **Required for the window.** Because [`MichiuRenderer`] renders using `DirectComposition`, the window must be created with `WS_EX_NOREDIRECTIONBITMAP` (`true`).
//!    This prevents the Desktop Window Manager (DWM) from allocating redundant legacy redirection surfaces, allowing `DirectComposition` visuals to attach cleanly.
//!
//! 4. **Type-Safe Builder Validation ([`into_unvalidated`](crate::window::MichiuWindowBuilder::into_unvalidated) & `try_into`):**
//!    [`MichiuWindow::build`] strictly requires a validated builder wrapper ([`Validated<MichiuWindowBuilder>`](michiu_guard::Validated)) to prevent creating windows with invalid or conflicting OS parameters.
//!    Calling `.into_unvalidated().try_into()?` triggers the [`michiu_guard`] validation pipeline, transforming the unverified configuration into a verified, safe state.
//!    For details, see [`michiu_guard`].
//!
//! ---
//!
//! ## Quick Start
//!
//! Here is a minimal, complete application featuring a reactive button:
//!
//! ```no_run
//! use michiu::prelude::*;
//!
//! fn main() -> Result<()> {
//!     // Preventing Screen Blur
//!     let _ = MichiuWindow::init_dpi_awareness();
//!     // COM Initialization for WinRT/DirectComposition
//!     let com = MichiuComContext::new_ro_single()?;
//!
//!     // 1. Create window
//!     let builder = MichiuWindowBuilder::new()
//!         .with_title("Sample App")
//!         .with_com_context(&com)
//!         .with_no_redirection_bitmap(true) // Required when using DirectComposition
//!         .with_inner_size(LogicalSize::new(600.0, 500.0))
//!         .into_unvalidated(); // ↓
//!                              // Type-Safe Verification by michiu_guard
//!     let window = MichiuWindow::build(builder.try_into()?)?;
//!
//!     // 2. Initialize renderer
//!     let scale_factor = window.scale_factor() as f32;
//!     let handle = window.handle().assume_valid();
//!     let rect = handle.client_rect();
//!     // MichiuRenderer::new is async. Use any executor of your choice.
//!     let renderer = pollster::block_on(MichiuRenderer::new(
//!         handle.hwnd(),
//!         rect.cast(),
//!         scale_factor,
//!         None,
//!     ))?;
//!
//!     // 3. Build UI and application host
//!     let (mut app, mut pump) = MichiuAppBuilder::new(window).build_with_ui(renderer, move || {
//!         let (count, set_count) = create_signal(0u32); // Initialize the signal.
//!
//!         flex(
//!             ii().size_full()
//!             .justify_center()
//!             .items_center()
//!             .bg_color(rgb(22, 24, 29)))
//!         .child(
//!             flex(
//!                 ii().size((200.0, 100.0))
//!                 .r(6.0)
//!                 .justify_center()
//!                 .border_solid(2.0)
//!                 .border_color(rgb(209, 92, 174))
//!                 .pressed(ii().transform_scale(0.98, 0.98)),
//!             )
//!             .on_click(move || set_count.set(count.get() + 1))
//!             .label(
//!                 move || format!("Count: {}", count.get()),
//!                 ii().text_color(rgb(156, 158, 163)).font_size(22.0),
//!             ),
//!         )
//!     })?;
//!
//!
//!     // 4. Main event loop
//!     while pump.wait_event(|event, _, raw| {
//!         let resp = app.standard_handle_window_event(&event, &raw);
//!
//!         if resp.needs_redraw { app.redraw_requested(); }
//!         if resp.needs_update_window { app.update_window(); }
//!         if resp.consumed { return; }
//!
//!         if let MichiuAnyEvent::Window { event, .. } = event {
//!             match event {
//!                 MichiuEvent::CloseRequested => {
//!                     app.destroy();
//!                 }
//!                 MichiuEvent::Destroyed => {
//!                     app.quit();
//!                 }
//!                 MichiuEvent::RedrawRequested => {
//!                     app.standard_redraw();
//!                 }
//!                 _ => {}
//!             }
//!         }
//!     })? {}
//!
//!     Ok(())
//! }
//! ```
//!
//! ---
//!
//! ## Multi-Threading & Background Tasks
//!
//! While [`MichiuWindow`], [`MichiuApp`], and [`MichiuEventPump`] are **thread-affine** and must live on the UI thread,
//! Michiu provides three built-in mechanisms to safely interact with the UI from worker threads:
//!
//! ### 1. Thread-Safe Window Control ([`WindowHandle`])
//! Cloned handles are `Send + Sync`. Invoking methods like [`destroy`](crate::window::WindowHandle::destroy)
//! or [`redraw_requested`](crate::window::WindowHandle::redraw_requested) from a background thread automatically routes
//! the command asynchronously to the UI thread.
//!
//! ### 2. Updating Signals from Background Threads
//! You can extract a [`SignalSender<T>`](crate::ui::MichiuTaskSender) from any [`WriteSignal<T>`].
//! It can be moved into any worker thread to update reactive UI state directly:
//!
//! ```no_run
//! # use michiu::ui::prelude::*;
//! let (count, set_count) = create_signal(0u32);
//! let tx = set_count.sender(); // SignalSender<u32> is Send + Sync
//!
//! std::thread::spawn(move || {
//!     // Automatically wakes the main loop and updates the UI signal
//!     tx.send(42);
//! });
//! ```
//!
//! ### 3. Arbitrary Context Tasks
//! For broader mutations, acquire a [`MichiuTaskSender`](crate::ui::MichiuTaskSender) via [`Context::task_sender`].
//! Closures sent through this sender are automatically boxed, scheduled for main-thread execution,
//! and trigger an immediate event-loop wake-up:
//!
//! ```no_run
//! # use michiu::prelude::*;
//! # fn run(app: &MichiuApp) {
//! let task_tx = app.context.task_sender();
//!
//! std::thread::spawn(move || {
//!     // Do heavy calculation or I/O...
//!     let result = "Computation finished";
//!
//!     let _ = task_tx.send(move |cx: &mut Context| {
//!         // Runs safely on the UI thread with mutable access to Context
//!     });
//! });
//! # }
//! ```
//!
//! ## Registers an event-loop wakeup callback ([`Context::set_waker`]).
//!
//! This callback is automatically invoked whenever a background task is submitted via [`MichiuTaskSender`](crate::ui::MichiuTaskSender) or [`SignalSender`](crate::ui::SignalSender).
//!
//! ### Note (Automatic Setup)
//! You **rarely need to call this manually**. When using [`MichiuAppBuilder`],
//! it automatically configures a waker that posts `WM_NULL` to wake the UI thread's event pump.
//!
//! Call this only if you are constructing a custom headless environment or integrating with an external event loop.
//!
//! ```no_run
//! # use michiu::ui::Context;
//! # fn run(mut context: Context) {
//! let my_waker = move || {
//!     // Wake up custom event loop...
//! };
//! context.set_waker(my_waker);
//! # }
//! ```
//!
//! ## Task Submission ([`Context::process_main_thread_tasks`])
//! Drains and executes all pending closures sent from background threads on the UI thread.
//!
//! This executes tasks dispatched via [`MichiuTaskSender`](crate::ui::MichiuTaskSender) and updates signals sent via [`SignalSender`](crate::ui::SignalSender).
//!
//! ### Automatic Processing
//! Normally, this is executed automatically within [`MichiuApp::standard_handle_window_event`] upon receiving `WM_NULL`.
//!
//! ### Manual Event Loop Usage
//! If you are writing a custom event loop without using [`standard_handle_window_event`](MichiuApp::standard_handle_window_event),
//! call this method inside your `WM_NULL` handler, then check [`has_dirty`](crate::ui::Context::has_dirty) to determine if a redraw is needed:
//!
//! ```no_run
//! # use michiu::prelude::*;
//! # use windows::Win32::UI::WindowsAndMessaging::WM_NULL;
//! # fn handle_raw(app: &mut MichiuApp, raw: MichiuRawEvent) {
//! match raw.msg {
//!     WM_NULL => {
//!         // 1. Drain background tasks
//!         app.context.process_main_thread_tasks();
//!
//!         // 2. Request a redraw only if UI state/layout became dirty
//!         if app.context.has_dirty() {
//!             app.redraw_requested();
//!         }
//!     }
//!     _ => {}
//! }
//! # }
//! ```
use crate::{
    guard::Validated,
    ui::{CapacityConfig, CharIndex, ImeState, prelude::*, raw_wheel_delta_to_logical_pixels},
    window::{ImeStateUpdate, PhysicalPoint, PhysicalSize, WheelDelta, WindowHandle, prelude::*},
};
use thiserror::Error;
use windows::Win32::{
    Graphics::Gdi::ScreenToClient,
    UI::{
        Input::KeyboardAndMouse::{
            VIRTUAL_KEY, VK_LBUTTON, VK_PACKET, VK_PROCESSKEY, VK_XBUTTON1, VK_XBUTTON2,
        },
        WindowsAndMessaging::{WM_ENTERSIZEMOVE, WM_EXITSIZEMOVE, WM_NULL},
    },
};

#[cfg(feature = "ui")]
#[doc(inline)]
pub use michiu_ui as ui;

#[cfg(feature = "guard")]
#[doc(inline)]
pub use michiu_guard as guard;

#[cfg(feature = "window")]
#[doc(inline)]
pub use michiu_window as window;

#[doc(hidden)]
pub mod prelude {
    pub use crate::guard::{Unvalidated, Validate, Validated};
    pub use crate::ui::prelude::*;
    pub use crate::window::prelude::*;
    pub use crate::*;
}
#[doc(hidden)]
pub mod accessibility {
    pub use crate::ui::accessibility::*;
}
#[doc(hidden)]
pub mod config {
    pub use crate::ui::config::*;
}

/// Integrated execution status of the Michiu GUI application.
///
/// It centrally manages windows, renderers, UI contexts, and settings,
/// and serves as a hub connecting the user-controlled main loop to various system components.
///
/// # Where to Use It
/// This is primarily used within the event loop (`MichiuEventPump`).
/// By calling [`MichiuApp::standard_handle_window_event`] or [`MichiuApp::standard_redraw`],
/// you can execute standard event handling and the rendering pipeline in just a few lines of code.
///
/// # How to Obtain It
/// Instead of instantiating it directly, create it using [`MichiuAppBuilder`].
///
/// ```no_run
/// # use michiu::prelude::*;
/// # fn run(window: MichiuWindow, renderer: MichiuRenderer) -> Result<()> {
/// let (mut app, mut pump) = MichiuAppBuilder::new(window)
///     .build_with_ui(renderer, move || {
///         // Building the UI Tree
///         div_n()
///     })?;
/// # Ok(())
/// # }
/// ```
///
/// # Thread Affinity
/// Due to constraints of the Win32 window message queue and composition,
/// ** [`MichiuApp`] is limited to the main thread (UI thread).**
///
/// (To manipulate windows from a background thread, use the thread-safe [`WindowHandle`].)
///
/// # Escape Hatch
/// With the exception of `root_id`, each of the main fields is exposed as `pub`.
///
/// If you want to perform low-level operations not covered by the standard helper methods
/// (such as integration with the Win32 API,
/// direct access to `Context` or `MichiuRender`, or custom layout synchronization),
/// you can use these fields directly.
///
/// # Examples
/// ```no_run
/// # use michiu::prelude::*;
/// # fn event_loop(mut app: MichiuApp, mut pump: MichiuEventPump) -> Result<()> {
/// while pump.wait_event(|event, _, raw| {
///     let resp = app.standard_handle_window_event(&event, &raw);
///     if resp.needs_redraw {
///         app.redraw_requested();
///     }
///     if resp.needs_update_window {
///         app.update_window();
///     }
///     if resp.consumed {
///         return;
///     }
///
///     if let MichiuAnyEvent::Window { event, .. } = event {
///         match event {
///             MichiuEvent::CloseRequested => {
///                 app.destroy();
///             },
///             MichiuEvent::Destroyed => {
///                 app.quit();
///             },
///             MichiuEvent::RedrawRequested => {
///                 app.standard_redraw();
///             },
///             _ => {}
///         }
///     }
/// })? {}
/// # Ok(())
/// # }
/// ```
pub struct MichiuApp {
    /// A context that manages the UI's reactive state, signals, layout tree, and more.
    pub context: Context,

    /// A rendering engine that uses `wgpu` and `DirectComposition`.
    ///
    /// This can be used to obtain the `IDCompositionDevice` required to create an `ExternalVisual`,
    /// as well as for manually resizing surfaces.
    pub renderer: MichiuRenderer,

    /// The main window owned by the application.
    ///
    /// Ensures the window's lifetime (prevents accidental destruction via RAII).
    pub window: MichiuWindow,

    /// A verified handle for window manipulation.
    ///
    /// Used for redraw requests, changing cursor icons, switching views, and obtaining a Win32 `HWND`, among other things.
    pub handle: Validated<WindowHandle>,

    /// The `Entity` of the root entity.
    ///
    /// Private for security reasons.
    ///
    /// Set using [`MichiuApp::set_root`], [`MichiuApp::build_ui`],
    /// or [`MichiuAppBuilder::build_with_ui`],
    /// and retrieve using [`MichiuApp::root`] or [`MichiuApp::try_root`].
    root: Option<Entity>,

    /// Operation policy settings specified in the builder
    /// (such as automatic cursor resolution and enabling shortcuts).
    pub config: MichiuBuilderConfig,
}

/// Results of executing the window event handler ([`MichiuApp::standard_handle_window_event`]).
///
/// This indicates how the UI state has changed as a result of processing the event,
/// and specifies what screen update actions the main loop should take next.
///
/// # Where to Use It
/// It is received immediately after an event is processed within the main loop (`MichiuEventPump`).
///
/// # How to Obtain It
/// Depending on the flag, drawing requests and subsequent processing are skipped.
///
/// # Examples
/// ```no_run
/// # use michiu::prelude::*;
/// # fn handle(app: &mut MichiuApp, event: MichiuAnyEvent, raw: MichiuRawEvent) {
/// let resp = app.standard_handle_window_event(&event, &raw);
///
/// // When a normal redraw is required
/// if resp.needs_redraw {
///     app.redraw_requested();
/// }
///
/// // When immediate forced rendering is required, such as during resizing
/// if resp.needs_update_window {
///     app.update_window();
/// }
///
/// // When the UI consumes input (skipping app-specific actions)
/// if resp.consumed {
///     return;
/// }
/// # }
/// ```
///
/// ## Why are there three flags?
/// ### `consumed` (event interception)
/// Indicates whether a UI entity (such as a button or input field) has processed this action.
///
/// For example, when typing in a text input field, if this flag is `true`,
/// the subsequent processing is interrupted with a `return` to prevent global shortcuts—such
/// as playback—from being accidentally triggered by the app-wide spacebar.
///
/// ### `needs_redraw` (Normal Redraw)
/// This is a request to update the screen at the next rendering opportunity,
/// such as when a button is hovered over or a signal is updated.
///
/// Internally in Win32, this area is simply marked as invalid (requiring a redraw),
/// and the actual rendering occurs when the message loop has a moment to spare.
///
/// ### `needs_update_window` (Immediate Synchronous Rendering)
/// This is a special request that occurs during events such as dragging and resizing a window.
///
/// If only the standard `needs_redraw` is used,
/// rendering will lag slightly when the window frame is dragged rapidly with the mouse.
///
/// To prevent this, this flag is set to force the OS to execute an immediate,
/// on-the-spot redraw (`UpdateWindow`) without waiting for the message queue.
#[derive(Debug, Clone, Copy)]
pub struct MichiuEventResponse {
    /// Whether this event was consumed by the UI (or `ExternalVisual`).
    ///
    /// If `true`, it means a UI entity was clicked or a text input field has focus.
    ///
    /// Use this to skip application-specific keyboard shortcuts, game controls, and similar actions.
    pub consumed: bool,

    /// Whether a redraw is required due to a change in the UI state
    /// (such as a hover, animation, or completion of an asynchronous task).
    ///
    /// If `true`, call [`MichiuApp::redraw_requested`].
    pub needs_redraw: bool,

    /// Whether immediate synchronous rendering is required—without waiting for the message
    /// queue—during window resizing, for example.
    ///
    /// If `true`, call [`MichiuApp::update_window`].
    pub needs_update_window: bool,
}

impl MichiuApp {
    /// It builds the UI tree and automatically registers it as the root entity.
    ///
    /// Since `set_root` is called automatically,
    /// you can use `root_id` in subsequent processing to retrieve the `Entity` of the root entity.
    #[inline]
    pub fn build_ui<F>(&mut self, f: F) -> Entity
    where
        F: FnOnce() -> Entity,
    {
        let root = build_ui(&mut self.context, f);
        self.set_root(root);
        root
    }

    /// Register the root entity.
    /// From this point on, you can retrieve the root entity using [`MichiuApp::root`].
    #[inline]
    pub fn set_root(&mut self, root_id: Entity) {
        self.root = Some(root_id);
    }

    /// Retrieves the `Entity` of the root entity.
    ///
    /// To avoid a panic, use [`MichiuApp::try_root`] instead.
    ///
    /// # Panics
    ///
    /// Panics if the root entity has not yet been registered.
    ///
    /// Please register the root entity in advance by calling [`MichiuApp::build_ui`],
    /// [`MichiuAppBuilder::build_with_ui`], or
    /// [`MichiuApp::set_root`].
    ///
    /// If this error occurs even though the root entity has been registered correctly,
    /// please report it to [GitHub Issues](https://github.com/eto0202/michiu/issues).
    #[must_use]
    #[inline]
    #[expect(clippy::expect_used)]
    pub fn root(&self) -> Entity {
        self.root.expect(
            "The root ID was not found.\
             Possible cause:\
                - The root ID was not registered. Call `build_ui`, `build_with_ui`, or `set_root` first.",
        )
    }

    /// Retrieves the `Entity` of the root entity.
    ///
    /// Returns `None` if it has not yet been registered.
    #[must_use]
    #[inline]
    pub fn try_root(&self) -> Option<Entity> {
        self.root
    }

    /// Thread-safely requests a redraw of the window.
    ///
    /// This is typically called when [`MichiuEventResponse::needs_redraw`] is `true`
    /// or when the UI is updated.
    ///
    /// # Performance
    /// - **On the UI thread**: Directly invokes `InvalidateRect` with zero overhead.
    /// - **On a background thread**: Automatically dispatches the request to the UI thread asynchronously.
    #[inline]
    pub fn redraw_requested(&self) {
        self.handle.redraw_requested();
    }

    /// Forces the window to redraw immediately without waiting for the message queue (thread-safely).
    ///
    /// This is typically called when [`MichiuEventResponse::needs_update_window`] is `true`,
    /// or when you want to update the screen immediately—such as during a resize.
    ///
    /// # Performance
    /// - **On the UI thread**: Directly invokes `UpdateWindow` with zero overhead.
    /// - **On a background thread**: Automatically dispatches the request to the UI thread asynchronously.
    #[inline]
    pub fn update_window(&self) {
        self.handle.update_window();
    }

    /// Requests that the window be closed (thread-safely).
    ///
    /// This is typically called when [`MichiuEvent::CloseRequested`] is received.
    ///
    /// # Performance
    /// - **On the UI thread**: Directly invokes `DestroyWindow` with zero overhead.
    /// - **On a background thread**: Automatically dispatches the request to the UI thread asynchronously.
    #[inline]
    pub fn destroy(&self) {
        self.handle.destroy();
    }

    /// Requests that the event loop terminate `WM_QUIT` (thread-safely).
    ///
    /// This is typically called when [`MichiuEvent::Destroyed`] is received.
    ///
    /// # Performance
    /// - **On the UI thread**: Directly invokes `PostQuitMessage` with zero overhead.
    /// - **On a background thread**: Automatically dispatches the request to the UI thread asynchronously.
    #[inline]
    pub fn quit(&self) {
        self.handle.quit();
    }

    /// Thread-safely updates the window visibility.
    ///
    /// # Performance
    /// - **On the UI thread**: Directly invokes `ShowWindow` with zero overhead.
    /// - **On a background thread**: Automatically dispatches the request to the UI thread asynchronously.
    #[inline]
    pub fn set_visible(&self, is_visual: bool) {
        self.handle.set_visible(is_visual);
    }

    /// Performs standard processing of window events and Win32 messages received from the OS.
    ///
    /// Forwards input events to [`Context`] and automatically dispatches them based
    /// on the configuration ([`MichiuBuilderConfig`]).
    ///
    /// # Items Processed Automatically
    /// - **Pointer/Mouse**: Cursor movement, clicks, double-clicks, wheel scrolling, automatic cursor shape resolution
    /// - **Keyboard/Text**: Standard key presses, character input, IME, basic shortcuts (Ctrl+C/V/X/Z/Y)
    /// - **Window State Synchronization**: Renderer updates due to resizing; detection of start/end of drag-and-resize
    /// - **Main Thread Tasks**: Processing asynchronous tasks (`WM_NULL`) received from the background
    /// - `ExternalVisual`: If enabled, dispatching raw input to external composition
    ///
    /// # Events Not Handled (Requires User Handling)
    /// To leave control over the application’s lifecycle and termination confirmation to the user, the following events are not handled within this method.
    ///
    /// - [`MichiuEvent::CloseRequested`]: Termination confirmation or a call to [`MichiuApp::destroy`]
    /// - [`MichiuEvent::Destroyed`]: Termination of the message loop via [`MichiuApp::quit`]
    /// - [`MichiuEvent::RedrawRequested`]: Actual frame rendering via [`MichiuApp::standard_redraw`]
    ///
    /// # Examples
    /// ```no_run
    /// # use michiu::prelude::*;
    /// # fn run(mut app: MichiuApp, mut pump: MichiuEventPump) -> Result<()> {
    /// while pump.wait_event(|event, _, raw| {
    ///     // Process events using the standard handler
    ///     let resp = app.standard_handle_window_event(&event, &raw);
    ///
    ///     // Issue drawing requests in response to state changes
    ///     if resp.needs_redraw {
    ///         app.redraw_requested();
    ///     }
    ///     if resp.needs_update_window {
    ///         app.update_window();
    ///     }
    ///
    ///     // Skip custom operations if the UI has already handled them
    ///     if resp.consumed {
    ///         return;
    ///     }
    ///
    ///     // Write your own code to handle lifecycle events that the standard handler does not handle
    ///     if let MichiuAnyEvent::Window { event, .. } = event {
    ///         match event {
    ///             MichiuEvent::CloseRequested => {
    ///                 app.destroy();
    ///             },
    ///             MichiuEvent::Destroyed => {
    ///                 app.quit();
    ///             },
    ///             MichiuEvent::RedrawRequested => {
    ///                 app.standard_redraw();
    ///             },
    ///             _ => {}
    ///         }
    ///     }
    /// })? {}
    /// # Ok(())
    /// # }
    /// ```
    #[expect(clippy::too_many_lines)]
    pub fn standard_handle_window_event(
        &mut self,
        event: &MichiuAnyEvent,
        raw: &MichiuRawEvent,
    ) -> MichiuEventResponse {
        let mut consumed = false;
        let mut ext_consumed = false;
        let mut needs_redraw = false;
        let mut needs_update_window = false;

        let has_hovered = self
            .context
            .interaction_id(InteractionState::Hovered)
            .is_some();
        let has_focused = self
            .context
            .interaction_id(InteractionState::Focused)
            .is_some();

        match raw.msg {
            WM_NULL => {
                // バックグラウンドから届いた CSS 更新タスクなどを安全に消化
                self.context.process_main_thread_tasks();

                // 消化によってレイアウトや描画に変更があった場合のみ再描画を実行
                if self.context.has_dirty() {
                    needs_redraw = true;
                }
                consumed = true;
            }
            WM_ENTERSIZEMOVE => {
                self.context.set_window_resized(true);
                needs_redraw = true;
                needs_update_window = true;
            }
            // ウィンドウドラッグリサイズの完了をキャッチ
            WM_EXITSIZEMOVE => {
                self.context.set_window_resized(false);
                // リサイズ完了後の再描画を即座にキックして、新サイズでの静止画キャプチャを誘発
                needs_redraw = true;
                needs_update_window = true;
            }
            _ => {}
        }

        if let MichiuAnyEvent::Window { event, .. } = event {
            match event {
                MichiuEvent::Resized(phy_size) => {
                    let result = phy_size
                        .validate_with(MichiuValidator::window_bounds)
                        .map(Validated::into_inner);

                    let size = match result {
                        Ok(size) => size,
                        #[allow(unused)]
                        Err(e) => {
                            debug_eprintln!("Debug: invalid window bounds: {:?}", e);
                            PhysicalSize::new(1, 1)
                        }
                    };

                    let width = size.width.cast_unsigned();
                    let height = size.height.cast_unsigned();
                    // レンダラーのリサイズとレイアウト物理サイズの更新
                    self.renderer
                        .resize(width, height, self.renderer.scale_factor());

                    // 再描画要求
                    needs_redraw = true;
                    needs_update_window = true;
                }
                MichiuEvent::CursorMoved { position } => {
                    let result = position
                        .validate_with(MichiuValidator::cursor_bounds)
                        .map(Validated::into_inner);

                    match result {
                        Ok(pos) => {
                            let phys_pos = pos.cast();
                            let logical_pos = pos.cast_scaled(self.window.scale_factor());

                            self.context
                                .inject_action(EventAction::PointerMove(logical_pos));

                            if self.config.external_visual_support {
                                ext_consumed =
                                    MichiuRenderer::dispatch_raw_input_to_external_visual(
                                        &mut self.context,
                                        raw.msg,
                                        raw.wparam,
                                        raw.lparam,
                                        phys_pos,
                                        self.renderer.scale_factor(),
                                    );
                            }

                            consumed = has_hovered || ext_consumed;
                            needs_redraw = true;
                        }
                        #[allow(unused)]
                        Err(e) => {
                            debug_eprintln!("Debug: cursor out of bounds: {:?}", e);
                        }
                    }
                }
                MichiuEvent::CursorLeft => {
                    // ウィンドウ外に去ったため、論理空間外へポインタを移動させてホバーを確実に解除
                    self.context
                        .inject_action(EventAction::PointerMove(LayoutPoint::new(
                            -9999.0, -9999.0,
                        )));

                    needs_redraw = true;
                }
                MichiuEvent::MouseInput {
                    button,
                    modifiers,
                    state,
                    click_count,
                } => {
                    let button = button.cast();
                    let modifiers = modifiers.cast();
                    let state = state.cast();

                    self.context.inject_action(EventAction::PointerButton {
                        button,
                        state,
                        modifiers,
                    });

                    if click_count == &2 {
                        self.context
                            .inject_action(EventAction::PointerDoubleClick { modifiers });
                    }

                    if self.config.external_visual_support {
                        let (x, y) = raw.lparam.to_point_f32();
                        let phys_pos = LayoutPoint::new(x, y);

                        ext_consumed = MichiuRenderer::dispatch_raw_input_to_external_visual(
                            &mut self.context,
                            raw.msg,
                            raw.wparam,
                            raw.lparam,
                            phys_pos,
                            self.renderer.scale_factor(),
                        );
                    }

                    consumed = has_hovered || ext_consumed;
                    needs_redraw = true;
                }
                MichiuEvent::CharacterInput(c) => {
                    self.context.inject_action(EventAction::Character(*c));
                    consumed = has_focused;
                    needs_redraw = true;
                }
                MichiuEvent::KeyboardInput {
                    key_code,
                    modifiers,
                    state,
                } => {
                    let mut shortcut_handled = false;

                    let result = key_code
                        .validate_with(MichiuValidator::keyboard_input)
                        .map(Validated::into_inner);

                    match result {
                        Ok(key_code) => {
                            let key = key_code.into();
                            let modifiers = modifiers.cast();
                            let state = state.cast();

                            if modifiers.ctrl
                                && self.config.default_shortcuts
                                && state == ElementState::Pressed
                            {
                                match i32::try_from(raw.wparam.0).unwrap_or(0) {
                                    // Ctrl + C
                                    0x43 if let Some(selected_text) =
                                        self.context.get_selected_text() =>
                                    {
                                        let _ = set_win32_clipboard(&selected_text);
                                        shortcut_handled = true;
                                    }
                                    // Ctrl + V
                                    0x56 if has_focused
                                        && let Some(pasted_text) = get_win32_clipboard() =>
                                    {
                                        self.context
                                            .inject_action(EventAction::Paste(pasted_text.into()));
                                        shortcut_handled = true;
                                    }
                                    // Ctrl + X (切り取り)
                                    0x58 if let Some(selected_text) =
                                        self.context.get_selected_text() =>
                                    {
                                        let _ = set_win32_clipboard(&selected_text);
                                        self.context.inject_action(EventAction::Cut);
                                        shortcut_handled = true;
                                    }
                                    // Ctrl + Z (Undo)
                                    0x5A if has_focused => {
                                        self.context.inject_action(EventAction::Undo);
                                        shortcut_handled = true;
                                    }
                                    // Ctrl + Y (Redo)
                                    0x59 if has_focused => {
                                        self.context.inject_action(EventAction::Redo);
                                        shortcut_handled = true;
                                    }
                                    _ => {}
                                }
                            }

                            self.context.inject_action(EventAction::KeyboardKey {
                                key,
                                state,
                                modifiers,
                            });

                            consumed = has_focused || shortcut_handled;
                            needs_redraw = true;
                        }
                        Err(MichiuError::Invalidkey(
                            KeyValidationError::ImeProcessKey | KeyValidationError::PacketKey,
                        )) => {
                            // IME中や文字直接入力は無視
                        }
                        #[allow(unused)]
                        Err(e) => {
                            debug_eprintln!("Debug: invalid key event: {:?}", e);
                        }
                    }
                }
                MichiuEvent::MouseWheel {
                    raw_delta_x,
                    raw_delta_y,
                } => {
                    let dx = raw_delta_x
                        .validate_with(MichiuValidator::wheel_event)
                        .map_or(WheelDelta(0), Validated::into_inner);
                    let dy = raw_delta_y
                        .validate_with(MichiuValidator::wheel_event)
                        .map_or(WheelDelta(0), Validated::into_inner);

                    // 両方 0 なら何もしない
                    #[expect(clippy::as_conversions, clippy::cast_precision_loss)]
                    if !(dx.0 == 0 && dy.0 == 0) {
                        // 横スクロール時は、右チルト（プラス値）された際に
                        // 右方向へスクロール（オフセット加算）させるため、符号の方向性を補正

                        let scroll_x = -raw_wheel_delta_to_logical_pixels(dx.raw() as f32);
                        let scroll_y = raw_wheel_delta_to_logical_pixels(dy.raw() as f32);

                        self.context
                            .inject_action(EventAction::MouseWheel { scroll_x, scroll_y });

                        if self.config.external_visual_support {
                            let mut pt = raw.lparam.to_point();
                            let _ = unsafe { ScreenToClient(self.handle.hwnd(), &raw mut pt) };

                            let phys_pos = LayoutPoint::new(pt.x as f32, pt.y as f32);
                            ext_consumed = MichiuRenderer::dispatch_raw_input_to_external_visual(
                                &mut self.context,
                                raw.msg,
                                raw.wparam,
                                raw.lparam,
                                phys_pos,
                                self.renderer.scale_factor(),
                            );
                        }

                        consumed = has_hovered || ext_consumed;
                        needs_redraw = true;
                    }
                }
                MichiuEvent::Ime(ime) => {
                    let reslut = ime
                        .clone()
                        .validate_with(MichiuValidator::ime_event)
                        .map(Validated::into_inner);

                    match reslut {
                        Ok(ime) => {
                            self.context.inject_action(EventAction::Ime(ime.cast()));
                            consumed = has_focused;
                            needs_redraw = true;
                        }
                        #[allow(unused)]
                        Err(e) => {
                            debug_eprintln!("Debug: invalid ime event: {:?}", e);
                        }
                    }
                }
                _ => {}
            }
        }

        MichiuEventResponse {
            consumed,
            needs_redraw,
            needs_update_window,
        }
    }

    /// Executes the standard frame rendering pipeline.
    ///
    /// Handles the entire pipeline in a single flow, from draining the event queue to layout calculations,
    ///
    /// rendering via wgpu, `DirectComposition` synchronization,
    /// and continuous frame requests during active animations.
    ///
    /// # Rendering Pipeline Executed
    /// 1. **Frame Start (`begin_frame`)**: Drains the event queue and updates interaction states (e.g., hover).
    /// 2. **Cursor Shape Update**: Synchronizes the mouse cursor icon based on the latest hover entity.
    /// 3. **System Frame Update (`tick_system_frame`)**: Advances animations and auto-drag states.
    /// 4. **Layout Synchronization (`sync_layout`)**: Recalculates layout in two passes to resolve scrollbars.
    /// 5. **Composition Update (`update_composition_tree`)**: Synchronizes `DirectComposition` for external visuals and the background.
    /// 6. **Accessibility Update**: Updates accessibility information if enabled.
    /// 7. **Drawing (`draw`)**: Performs text layout, builds/uploads the instance buffer, renders to the surface, and commits the composition. If `MichiuInspector` is active, inspection logs are dispatched immediately after this step.
    /// 8. **Synchronization & Next-Frame Request**: If there are active frames (e.g., animations), synchronizes according to the configured policy (`DwmFlush`, etc.) and immediately schedules the next frame.
    ///
    /// # When to Call
    /// Call this when receiving [`MichiuEvent::RedrawRequested`] within the main loop.
    ///
    /// # Notes
    /// To profile individual rendering phases or inject custom operations between steps,
    /// you can bypass this method and invoke the internal phase methods directly.
    ///
    /// # Panics
    /// In debug builds, this method panics if the root entity (`root_id`) has not been registered.
    ///
    /// In release builds, it returns early without performing any operations.
    pub fn standard_redraw(&mut self) {
        debug_assert!(
            self.try_root().is_some(),
            "The root ID was not found.\
             Possible cause:\
               - The root ID was not registered. Call `build_ui`, `build_with_ui`, or `set_root` first."
        );

        let Some(root_id) = self.try_root() else {
            return;
        };

        self.context.begin_frame();

        self.standard_update_cursor_icon();

        self.context.tick_system_frame(&TickType::All);
        self.context
            .sync_layout(root_id, self.renderer.window_size());
        self.renderer.update_composition_tree(&mut self.context);

        self.renderer.draw(&mut self.context);

        if self.config.accessibility_support {
            self.context.update_accessibility();
        }

        if self.context.has_active_frame() {
            if self.config.sync_mode == AutoSyncMode::DwmFlush {
                let _ = self.handle.dwm_flush();
            }
            self.handle.redraw_requested();
        }
    }

    /// Updates the window cursor icon based on the currently hovered UI entity.
    ///
    /// This standard method resolves the appropriate cursor icon for the hovered entity and automatic applies it to the OS.
    /// Normally, this is executed automatically within [`standard_redraw`](Self::standard_redraw).
    /// Call this manually if you are constructing a custom rendering pipeline.
    ///
    /// # Behavior
    /// 1. **Configuration Guard**: If [`MichiuBuilderConfig::auto_resolve_cursor`] is `false`, this method returns immediately without doing anything.
    /// 2. **Hover Lookup**: Queries the latest hovered entity (`InteractionState::Hovered`) from [`Context`].
    /// 3. **Fallback**: If no entity is hovered, or if the hovered entity specifies no custom cursor, it falls back to the default system cursor ([`CursorIcon::Default`]).
    /// 4. **OS Synchronization**: Converts the resolved icon into a native Win32 `HCURSOR` and updates the window's cursor via the underlying `SetCursor` API.
    ///
    /// # When to Call
    /// If calling manually, always invoke this after draining the event queue (i.e., immediately after [`Context::begin_frame`]).
    /// Calling it before `begin_frame` will read outdated hover information from the previous frame.
    pub fn standard_update_cursor_icon(&mut self) {
        if !self.config.auto_resolve_cursor {
            return;
        }

        let target_cursor =
            if let Some(hovered) = self.context.interaction_id(InteractionState::Hovered) {
                self.context.resolve_cursor(hovered)
            } else {
                CursorIcon::Default(None)
            };

        if let Ok(hcursor) = target_cursor.to_hcursor() {
            self.handle
                .set_cursor_icon(window::CursorIcon::Other(hcursor));
        }
    }
}

/// Vertical synchronization policy for rendering active frames in animations and similar content.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum AutoSyncMode {
    /// Calls `DwmFlush` and waits for the OS's vertical sync (V-Sync) (default).
    ///
    /// Prevents screen tearing and ensures smooth rendering in sync with the display's refresh rate.
    #[default]
    DwmFlush,

    /// Sends a redraw request immediately without waiting for V-Sync.
    ///
    /// Specify this when you want to reduce rendering latency or during benchmark testing.
    Immediate,
}

/// Configuration for the standard pipeline of [`MichiuApp`].
///
/// This is typically specified using the `with_*` methods of [`MichiuAppBuilder`].
#[expect(clippy::struct_excessive_bools)]
#[derive(Debug, Clone, Copy)]
pub struct MichiuBuilderConfig {
    /// Whether to automatically dispatch raw input messages to `ExternalVisual`.
    ///
    /// - **Default**: `false`
    /// - **Purpose**: If you are not using `ExternalVisual`, leaving this set to `false` reduces the overhead of dispatching input every frame.
    pub external_visual_support: bool,

    /// Whether to automatically handle basic editing shortcuts (Ctrl+C/V/X/Z/Y).
    ///
    /// - **Default**: `true`
    /// - **Purpose**: Set this to `false` if the app wants to fully manage its own global key bindings, such as Ctrl+Z.
    pub default_shortcuts: bool,

    /// Rendering synchronization method during animation.
    ///
    /// - **Default**: [`AutoSyncMode::DwmFlush`]
    pub sync_mode: AutoSyncMode,

    /// Whether to use accessibility features.
    ///
    /// - **Default**: `false`
    /// - **Purpose**: Set to `true` if you want to support assistive technologies such as screen readers.
    pub accessibility_support: bool,

    /// Whether to automatically update the window's cursor icon based on the entity being hovered over.
    ///
    /// - **Default**: `true`
    /// - **Purpose**: Set to `false` if you want to draw your own cursor or keep the OS cursor shape fixed.
    pub auto_resolve_cursor: bool,
}

impl MichiuBuilderConfig {
    fn new() -> Self {
        Self {
            external_visual_support: false,
            default_shortcuts: true,
            sync_mode: AutoSyncMode::default(),
            accessibility_support: false,
            auto_resolve_cursor: true,
        }
    }
}

impl Default for MichiuBuilderConfig {
    fn default() -> Self {
        Self::new()
    }
}

/// A builder for constructing [`MichiuApp`] and [`MichiuEventPump`].
///
/// # Recommended Initialization Order
/// Since creating a [`MichiuRenderer`] requires the HWND and scale factor of a [`MichiuWindow`],
/// please initialize them in the following order.
///
/// 1. Create a [`MichiuWindow`]. This requires a [`MichiuWindowBuilder`] and, depending on the options, possibly a [`MichiuComContext`] first.
/// 2. Asynchronously initialize the [`MichiuRenderer`] based on the window information.
/// 3. Pass the window to [`MichiuAppBuilder::new`], then pass the renderer last and call [`build`](Self::build) (or [`build_with_ui`](Self::build_with_ui)).
///
/// # Thread Affinity
/// Due to constraints of the Win32 window message queue and composition, **this builder and the generated [`MichiuApp`] are limited to the main thread (UI thread).**
///
/// (Please use a thread-safe [`WindowHandle`] for window operations from background threads.)
///
/// # Option Settings
/// - [`MichiuInspector`]: Inspector for debugging and diagnostics (default: `None`). Register it using [`with_inspector`](Self::with_inspector).
/// - [`CapacityConfig`]: Initial allocation settings for the `SoA` arena (default: `None`). Register using [`with_capacity_config`](Self::with_capacity_config).
/// - Various behavior flags: Do not manipulate `config` directly; use this builder’s `with_*` method chain instead.
///
/// # Examples
/// ```no_run
/// # use michiu::prelude::*;
/// # fn run(window: MichiuWindow, renderer: MichiuRenderer) -> Result<()> {
/// let (mut app, mut pump) = MichiuAppBuilder::new(window)
///     .with_auto_resolve_cursor(true)
///     .with_default_shortcuts(true)
///     .build_with_ui(renderer, move || {
///         // Root UI tree
///         div_n()
///     })?;
/// # Ok(())
/// # }
/// ```
pub struct MichiuAppBuilder {
    pub config: MichiuBuilderConfig,
    pub window: MichiuWindow,
    pub inspector: Option<MichiuInspector>,
    pub capacity: Option<CapacityConfig>,
}

impl MichiuAppBuilder {
    #[must_use]
    #[inline]
    pub fn new(window: MichiuWindow) -> Self {
        Self {
            config: MichiuBuilderConfig::new(),
            window,
            inspector: None,
            capacity: None,
        }
    }

    /// Consumes the builder settings and creates [`MichiuApp`] and [`MichiuEventPump`].
    ///
    /// # Window Visibility and Flicker Prevention
    /// Due to `AccessKit` initialization requirements, the window must be hidden during accessibility initialization.
    ///
    /// As a safety measure, this method internally forces a call to `set_visible(false)`.
    /// However, if the window was created with `with_visible(true)`,
    /// the window may briefly appear and then disappear, which could cause screen flickering.
    ///
    /// ### Recommended Steps to Prevent Flickering
    /// To completely prevent flickering at startup, we recommend the following steps.
    /// 1. When creating a window, be sure to specify **`.with_visible(false)`** with [`MichiuWindowBuilder`].
    /// 2. Build the app using this method (or [`build_with_ui`](Self::build_with_ui)) and assemble the UI.
    /// 3. Only after everything is ready should you call **`handle.set_visible(true)`** to display the app.
    ///
    /// # Regarding the State of the Root Entity
    /// At the time this method is called, the root entity ([`root_id`](MichiuApp::try_root)) remains `None`.
    ///
    /// Be sure to register the UI using [`MichiuApp::build_ui`] or [`MichiuApp::set_root`] before starting the event loop.
    ///
    /// (If you want to complete registration in a single step, we recommend using [`build_with_ui`](Self::build_with_ui).)
    ///
    /// # Processes that run automatically in the background
    /// - **Automatic Context Selection**: Initializes the optimal context based on the registration status of [`MichiuInspector`] and [`CapacityConfig`].
    /// - **Automatic Waker Registration**: Automatically wires a Waker (`wake_up`) to the context to safely wake up the event loop when a background task completes.
    /// - **Accessibility Integration**: If enabled, binds a Win32 `HWND` to the `AccessKit` adapter.
    ///
    /// # Errors
    /// Returns an error if the window handle ([`WindowHandle`]) has already been disabled or destroyed.
    ///
    /// # Examples
    /// ```no_run
    /// # use michiu::prelude::*;
    /// # fn run(window: MichiuWindow, renderer: MichiuRenderer) -> Result<()> {
    /// let (mut app, mut pump) = MichiuAppBuilder::new(window)
    ///     .with_accessibility_support(true)
    ///     .build(renderer)?;
    ///
    /// // Build UI and register the root entity
    /// app.build_ui(move || {
    ///     // UI tree
    ///     div_n()
    /// });
    ///
    /// // Make the window visible AFTER initialization
    /// app.set_visible(true);
    /// # Ok(())
    /// # }
    /// ```
    #[inline]
    pub fn build(self, renderer: MichiuRenderer) -> window::Result<(MichiuApp, MichiuEventPump)> {
        let handle: Validated<WindowHandle> = self.window.handle().try_into()?;
        let hwnd = handle.hwnd();

        let mut context = match (&self.capacity, &self.inspector) {
            (Some(c), Some(i)) => Context::with_capacity_and_inspector(c, i),
            (Some(c), None) => Context::with_capacity(c),
            (None, Some(i)) => Context::with_inspector(i),
            (None, None) => Context::new(),
        };

        if self.config.accessibility_support {
            self.window.set_visible(false);
            context = context.with_accessibility(hwnd);
        }

        let h_waker = handle.clone();
        context.set_waker(move || h_waker.wake_up());

        let pump = MichiuEventPump::new();
        Ok((
            MichiuApp {
                window: self.window,
                context,
                renderer,
                handle,
                root: None,
                config: self.config,
            },
            pump,
        ))
    }

    /// Uses the builder to create [`MichiuApp`] and [`MichiuEventPump`], and performs the construction and registration of the root UI entity in a single step.
    ///
    /// This is a convenient shortcut equivalent to calling [`MichiuApp::build_ui`] after calling [`build`](Self::build).
    ///
    /// # Window Visibility and Flicker Prevention
    /// As with [`build`](Self::build), the window is forced to remain hidden during initialization if accessibility is enabled.
    ///
    /// To prevent flickering at startup, **specify `.with_visible(false)` when creating the window,
    /// and call `app.set_visible(true)` after this method completes.**
    ///
    /// For information on Waker’s automatic setup and other details, see [`build`](Self::build).
    ///
    /// # Errors
    /// Returns an error if the window handle ([`WindowHandle`]) has already been disabled or destroyed.
    ///
    /// # Examples
    /// ```no_run
    /// # use michiu::prelude::*;
    /// # fn run(window: MichiuWindow, renderer: MichiuRenderer) -> Result<()> {
    /// let (mut app, mut pump) = MichiuAppBuilder::new(window)
    ///     .build_with_ui(renderer, move || {
    ///         // Root UI tree is automatically registered as the application root
    ///         div_n()
    ///     })?;
    ///
    /// # Ok(())
    /// # }
    /// ```
    #[inline]
    pub fn build_with_ui<F>(
        self,
        renderer: MichiuRenderer,
        ui_fn: F,
    ) -> window::Result<(MichiuApp, MichiuEventPump)>
    where
        F: FnOnce() -> Entity,
    {
        let (mut app, pump) = self.build(renderer)?;
        app.build_ui::<F>(ui_fn);
        Ok((app, pump))
    }

    /// Whether to automatically update the window's cursor icon based on the entity being hovered over.
    ///
    /// - **Default**: `true`
    /// - **Purpose**: Set to `false` if you want to draw your own cursor or keep the OS cursor shape fixed.
    #[must_use]
    #[inline]
    pub fn with_auto_resolve_cursor(mut self, enabled: bool) -> Self {
        self.config.auto_resolve_cursor = enabled;
        self
    }

    /// Whether to automatically dispatch raw input messages to `ExternalVisual`.
    ///
    /// - **Default**: `false`
    /// - **Purpose**: If you are not using `ExternalVisual`, leaving this set to `false` reduces the overhead of dispatching input every frame.
    #[must_use]
    #[inline]
    pub fn with_external_visual_support(mut self, enabled: bool) -> Self {
        self.config.external_visual_support = enabled;
        self
    }

    /// Whether to automatically handle basic editing shortcuts (Ctrl+C/V/X/Z/Y).
    ///
    /// - **Default**: `true`
    /// - **Purpose**: Set this to `false` if the app wants to fully manage its own global key bindings, such as Ctrl+Z.
    #[must_use]
    #[inline]
    pub fn with_default_shortcuts(mut self, enabled: bool) -> Self {
        self.config.default_shortcuts = enabled;
        self
    }

    /// Rendering synchronization method during animation.
    ///
    /// - **Default**: [`AutoSyncMode::DwmFlush`]
    #[must_use]
    #[inline]
    pub fn with_sync_mode(mut self, mode: AutoSyncMode) -> Self {
        self.config.sync_mode = mode;
        self
    }

    /// Whether to use accessibility features.
    ///
    /// - **Default**: `false`
    /// - **Purpose**: Set to `true` if you want to support assistive technologies such as screen readers.
    #[must_use]
    #[inline]
    pub fn with_accessibility_support(mut self, enabled: bool) -> Self {
        self.config.accessibility_support = enabled;
        self
    }

    /// Registers an inspector ([`MichiuInspector`]) for diagnostics and performance measurement.
    ///
    /// Allows you to collect layout diagnostic information and error logs output by [`Context`] and [`MichiuRenderer`] in real time.
    ///
    /// # Typical Usage
    /// Typically, you create an inspector, subscribe to the log stream (buffer size is configurable; default is 128), and then receive and process the data in a background thread.
    ///
    /// For details on subscription settings and received data, see [`MichiuInspector`].
    ///
    /// # Examples
    /// ```no_run
    /// # use michiu::prelude::*;
    /// # fn run(window: MichiuWindow, renderer: MichiuRenderer) -> Result<()> {
    /// let inspector = MichiuInspector::new();
    /// let sub = inspector.subscribe(None); // Default buffer size: 128
    ///
    /// // Process diagnostics on a worker thread
    /// std::thread::spawn(move || {
    ///     while let Ok(batch) = sub.recv() {
    ///         // Handle diagnostic batch / error logs
    ///     }
    /// });
    ///
    /// let (app, pump) = MichiuAppBuilder::new(window)
    ///     .with_inspector(inspector)
    ///     .build(renderer)?;
    /// # Ok(())
    /// # }
    /// ```
    #[must_use]
    #[inline]
    pub fn with_inspector(mut self, inspector: MichiuInspector) -> Self {
        self.inspector = Some(inspector);
        self
    }

    /// Sets the initial allocated capacity for the `SoA` arena used within the UI context.
    ///
    /// By default, all capacities are set to `0` (dynamic allocation as needed).
    ///
    /// In applications with a large number of nodes, pre-allocating memory can prevent the overhead of heap reallocation during startup or the initial layout.
    ///
    /// # Recommended Configuration Method
    /// It is convenient to use [`CapacityConfig::from_base_nodes`], which automatically allocates capacity for each internal table based on the specified number of nodes.
    ///
    /// For details, see [`CapacityConfig`].
    ///
    /// # Examples
    /// ```no_run
    /// # use michiu::prelude::*;
    /// # use michiu::ui::CapacityConfig;
    /// # fn run(window: MichiuWindow, renderer: MichiuRenderer) -> Result<()> {
    /// let (app, pump) = MichiuAppBuilder::new(window)
    ///     .with_capacity_config(CapacityConfig::from_base_nodes(1024))
    ///     .build(renderer)?;
    /// # Ok(())
    /// # }
    /// ```
    #[must_use]
    #[inline]
    pub fn with_capacity_config(mut self, config: CapacityConfig) -> Self {
        self.capacity = Some(config);
        self
    }
}

// ===================================================================
// ===================================================================

pub trait Cast<Target> {
    fn cast(self) -> Target;
}

// window -> ui

impl Cast<ui::LayoutSize> for window::LogicalSize {
    #[inline]
    #[expect(clippy::as_conversions, clippy::cast_possible_truncation)]
    fn cast(self) -> LayoutSize {
        ui::LayoutSize::new(self.width as f32, self.height as f32)
    }
}

impl Cast<ui::LayoutSize> for window::PhysicalSize {
    #[inline]
    #[expect(clippy::as_conversions, clippy::cast_precision_loss)]
    fn cast(self) -> LayoutSize {
        ui::LayoutSize::new(self.width as f32, self.height as f32)
    }
}

impl Cast<ui::LayoutPoint> for window::LogicalPoint {
    #[inline]
    #[expect(clippy::as_conversions, clippy::cast_possible_truncation)]
    fn cast(self) -> ui::LayoutPoint {
        ui::LayoutPoint {
            x: self.x as f32,
            y: self.y as f32,
        }
    }
}

impl Cast<ui::LayoutPoint> for window::PhysicalPoint {
    #[inline]
    #[expect(clippy::as_conversions, clippy::cast_precision_loss)]
    fn cast(self) -> ui::LayoutPoint {
        ui::LayoutPoint {
            x: self.x as f32,
            y: self.y as f32,
        }
    }
}

impl Cast<ui::LayoutRect> for window::LogicalRect {
    #[inline]
    #[expect(clippy::as_conversions, clippy::cast_possible_truncation)]
    fn cast(self) -> ui::LayoutRect {
        ui::LayoutRect {
            // 原点は常に小さい方（左・上）を取る
            x: self.left.min(self.right) as f32,
            y: self.top.min(self.bottom) as f32,
            // サイズは常に絶対差を取るのでに負数にならない
            width: (self.right - self.left).abs() as f32,
            height: (self.bottom - self.top).abs() as f32,
        }
    }
}

impl Cast<ui::LayoutRect> for window::PhysicalRect {
    #[inline]
    #[expect(clippy::as_conversions, clippy::cast_precision_loss)]
    fn cast(self) -> ui::LayoutRect {
        ui::LayoutRect {
            x: self.left.min(self.right) as f32,
            y: self.top.min(self.bottom) as f32,
            // abs_diff は 2つの整数の絶対差を u32 で返す
            width: self.right.abs_diff(self.left) as f32,
            height: self.bottom.abs_diff(self.top) as f32,
        }
    }
}

// ui -> window

impl Cast<window::LogicalSize> for ui::LayoutSize {
    #[inline]
    fn cast(self) -> window::LogicalSize {
        LogicalSize {
            width: f64::from(self.width),
            height: f64::from(self.height),
        }
    }
}

impl Cast<window::PhysicalSize> for ui::LayoutSize {
    #[inline]
    fn cast(self) -> window::PhysicalSize {
        window::PhysicalSize {
            width: f32_round_to_i32(self.width).max(0),
            height: f32_round_to_i32(self.height).max(0),
        }
    }
}

impl Cast<window::LogicalPoint> for ui::LayoutPoint {
    #[inline]
    fn cast(self) -> window::LogicalPoint {
        window::LogicalPoint {
            x: f64::from(self.x),
            y: f64::from(self.y),
        }
    }
}

impl Cast<window::PhysicalPoint> for ui::LayoutPoint {
    #[inline]
    fn cast(self) -> window::PhysicalPoint {
        window::PhysicalPoint {
            x: f32_round_to_i32(self.x),
            y: f32_round_to_i32(self.y),
        }
    }
}

impl Cast<window::LogicalRect> for ui::LayoutRect {
    #[inline]
    fn cast(self) -> window::LogicalRect {
        window::LogicalRect {
            left: f64::from(self.x),
            top: f64::from(self.y),
            right: f64::from(self.x + self.width),
            bottom: f64::from(self.y + self.height),
        }
    }
}

impl Cast<window::PhysicalRect> for ui::LayoutRect {
    #[inline]
    fn cast(self) -> window::PhysicalRect {
        let x1 = self.x;
        let x2 = self.x + self.width;
        let y1 = self.y;
        let y2 = self.y + self.height;

        window::PhysicalRect {
            // min を left/top、max を right/bottom に割り当てることで万が一幅や高さがマイナスでも正常な矩形を維持する
            left: f32_round_to_i32(x1.min(x2)),
            top: f32_round_to_i32(y1.min(y2)),
            right: f32_round_to_i32(x1.max(x2)),
            bottom: f32_round_to_i32(y1.max(y2)),
        }
    }
}

#[inline]
#[expect(clippy::as_conversions, clippy::cast_possible_truncation)]
fn f32_round_to_i32(val: f32) -> i32 {
    val.round() as i32
}

// ===================================================================
// ===================================================================

#[inline]
#[expect(clippy::as_conversions, clippy::cast_possible_truncation)]
fn f64_round_to_i32(val: f64) -> i32 {
    val.round() as i32
}

#[inline]
#[expect(clippy::as_conversions, clippy::cast_possible_truncation)]
fn f64_to_f32(val: f64) -> f32 {
    val as f32
}

pub trait CastScaled<Target> {
    fn cast_scaled(self, scale_factor: f64) -> Target;
}

// window -> ui

impl CastScaled<ui::LayoutSize> for window::PhysicalSize {
    #[inline]
    fn cast_scaled(self, scale_factor: f64) -> ui::LayoutSize {
        let scale = scale_factor.max(0.001); // ゼロ除算防止
        ui::LayoutSize::new(
            f64_to_f32(f64::from(self.width) / scale),
            f64_to_f32(f64::from(self.height) / scale),
        )
    }
}

impl CastScaled<ui::LayoutPoint> for window::PhysicalPoint {
    #[inline]
    fn cast_scaled(self, scale_factor: f64) -> ui::LayoutPoint {
        let scale = scale_factor.max(0.001);
        ui::LayoutPoint {
            x: f64_to_f32(f64::from(self.x) / scale),
            y: f64_to_f32(f64::from(self.y) / scale),
        }
    }
}

impl CastScaled<ui::LayoutRect> for window::PhysicalRect {
    #[inline]
    fn cast_scaled(self, scale_factor: f64) -> ui::LayoutRect {
        let scale = scale_factor.max(0.001);
        let x1 = f64::from(self.left) / scale;
        let x2 = f64::from(self.right) / scale;
        let y1 = f64::from(self.top) / scale;
        let y2 = f64::from(self.bottom) / scale;

        ui::LayoutRect {
            // 原点は常に最小値（左・上）を取り反転を正規化
            x: f64_to_f32(x1.min(x2)),
            y: f64_to_f32(y1.min(y2)),
            // サイズは常に絶対差（正数）を保証
            width: f64_to_f32((x2 - x1).abs()),
            height: f64_to_f32((y2 - y1).abs()),
        }
    }
}

// ui -> window

impl CastScaled<window::PhysicalSize> for ui::LayoutSize {
    #[inline]
    fn cast_scaled(self, scale_factor: f64) -> window::PhysicalSize {
        window::PhysicalSize {
            // 四捨五入してスナップかつ負数を防止
            width: f64_round_to_i32(f64::from(self.width) * scale_factor).max(0),
            height: f64_round_to_i32(f64::from(self.height) * scale_factor).max(0),
        }
    }
}

impl CastScaled<window::PhysicalPoint> for ui::LayoutPoint {
    #[inline]
    fn cast_scaled(self, scale_factor: f64) -> window::PhysicalPoint {
        window::PhysicalPoint {
            x: f64_round_to_i32(f64::from(self.x) * scale_factor),
            y: f64_round_to_i32(f64::from(self.y) * scale_factor),
        }
    }
}

impl CastScaled<window::PhysicalRect> for ui::LayoutRect {
    #[inline]
    fn cast_scaled(self, scale_factor: f64) -> window::PhysicalRect {
        let x1 = f64_round_to_i32(f64::from(self.x) * scale_factor);
        let x2 = f64_round_to_i32(f64::from(self.x + self.width) * scale_factor);
        let y1 = f64_round_to_i32(f64::from(self.y) * scale_factor);
        let y2 = f64_round_to_i32(f64::from(self.y + self.height) * scale_factor);

        window::PhysicalRect {
            // min を left/top、max を right/bottom にして Win32 互換を保つ
            left: x1.min(x2),
            top: y1.min(y2),
            right: x1.max(x2),
            bottom: y1.max(y2),
        }
    }
}

// ===================================================================
// ===================================================================

// window -> ui

impl Cast<ui::ElementState> for window::ElementState {
    #[inline]
    fn cast(self) -> ui::ElementState {
        match self {
            window::ElementState::Pressed => ui::ElementState::Pressed,
            window::ElementState::Released => ui::ElementState::Released,
        }
    }
}

impl Cast<ui::Modifiers> for window::Modifiers {
    #[inline]
    fn cast(self) -> ui::Modifiers {
        ui::Modifiers {
            shift: self.contains(window::Modifiers::SHIFT),
            ctrl: self.contains(window::Modifiers::CONTROL),
            alt: self.contains(window::Modifiers::ALT),
            logo: self.contains(window::Modifiers::LOGO),
        }
    }
}

impl Cast<ui::MouseButton> for window::MouseButton {
    #[inline]
    fn cast(self) -> ui::MouseButton {
        match self {
            window::MouseButton::Left => ui::MouseButton::Left,
            window::MouseButton::Right => ui::MouseButton::Right,
            window::MouseButton::Middle => ui::MouseButton::Middle,
            window::MouseButton::Other(param) => {
                if param == VK_XBUTTON1.0 {
                    ui::MouseButton::X1
                } else if param == VK_XBUTTON2.0 {
                    ui::MouseButton::X2
                } else {
                    ui::MouseButton::Left // 一旦 Left にフォールバック
                }
            }
        }
    }
}

// ui -> window

impl Cast<window::ElementState> for ui::ElementState {
    #[inline]
    fn cast(self) -> window::ElementState {
        match self {
            ElementState::Pressed => window::ElementState::Pressed,
            ElementState::Released => window::ElementState::Released,
        }
    }
}

impl Cast<window::Modifiers> for ui::Modifiers {
    #[inline]
    fn cast(self) -> window::Modifiers {
        let mut modifiers = window::Modifiers::empty();

        if self.shift {
            modifiers.insert(window::Modifiers::SHIFT);
        }
        if self.ctrl {
            modifiers.insert(window::Modifiers::CONTROL);
        }
        if self.alt {
            modifiers.insert(window::Modifiers::ALT);
        }
        if self.logo {
            modifiers.insert(window::Modifiers::LOGO);
        }

        modifiers
    }
}

impl Cast<window::MouseButton> for ui::MouseButton {
    #[inline]
    fn cast(self) -> window::MouseButton {
        match self {
            MouseButton::Left => window::MouseButton::Left,
            MouseButton::Right => window::MouseButton::Right,
            MouseButton::Middle => window::MouseButton::Middle,
            MouseButton::X1 => window::MouseButton::Other(VK_XBUTTON1.0),
            MouseButton::X2 => window::MouseButton::Other(VK_XBUTTON2.0),
        }
    }
}

// ===================================================================
// ===================================================================

impl Cast<ui::ImeState> for window::ImeStateUpdate {
    #[inline]
    fn cast(self) -> ui::ImeState {
        ImeState {
            is_open: self.is_open,
            conversion_mode: self.conversion_mode,
            sentence_mode: self.sentence_mode,
            keyboard_layout_id: self.keyboard_layout_id,
            composition_text: self.composition_text.into(),
            result_text: self.result_text.into(),
            caret_position: self.caret_position.map(Cast::cast),
            composition_cursor: CharIndex(self.composition_cursor),
            composition_attrs: self.composition_attrs,
        }
    }
}

pub trait CastIme<Target> {
    fn cast_ime(self, window_id: WindowId) -> Target;
}

impl CastIme<window::ImeStateUpdate> for ui::ImeState {
    fn cast_ime(self, window_id: WindowId) -> window::ImeStateUpdate {
        window::ImeStateUpdate {
            window_id: window_id.id(),
            is_open: self.is_open,
            conversion_mode: self.conversion_mode,
            sentence_mode: self.sentence_mode,
            keyboard_layout_id: self.keyboard_layout_id,
            composition_text: self.composition_text.into(),
            result_text: self.result_text.into(),
            caret_position: self.caret_position.map(Cast::cast),
            composition_cursor: self.composition_cursor.0,
            composition_attrs: self.composition_attrs,
        }
    }
}

pub type Result<T> = std::result::Result<T, MichiuError>;

#[derive(Debug, Error)]
pub enum MichiuError {
    #[error("window error: {0}")]
    Window(#[from] window::MichiuError),

    #[error("ui error: {0}")]
    Ui(#[from] ui::MichiuError),

    #[error("invalid size: {0}")]
    InvalidSize(#[from] SizeValidationError),

    #[error("invalid point: {0}")]
    InvalidPoint(#[from] PointValidationError),

    #[error("invalid key: {0}")]
    Invalidkey(#[from] KeyValidationError),

    #[error("invalid key: {0}")]
    InvalidWheel(#[from] WheelValidationError),

    #[error("invalid ime: {0}")]
    InvalidIme(#[from] ImeValidationError),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Error)]
pub enum SizeValidationError {
    #[error("window size contains negative values (width: {width}, height: {height})")]
    Negative { width: i32, height: i32 },

    #[error("window size is zero (window may be minimized)")]
    Zero { width: i32, height: i32 },

    #[error(
        "window size exceeds maximum allowed dimension `16_384` (width: {width}, height: {height})"
    )]
    TooLarge { width: i32, height: i32 },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Error)]
pub enum PointValidationError {
    #[error("cursor position is out of reasonable virtual bounds ({x}, {y})")]
    OutOfBounds { x: i32, y: i32 },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Error)]
pub enum KeyValidationError {
    #[error("key code is zero or undefined")]
    Zero,

    #[error("key code exceeds 8-bit virtual key range: {0:#X}")]
    OutOfRange(u16),

    #[error("mouse button code received in keyboard event: {0:#X}")]
    MouseButton(u16),

    #[error("IME is processing the key (VK_PROCESSKEY)")]
    ImeProcessKey,

    #[error("packet/unicode injection key (VK_PACKET)")]
    PacketKey,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Error)]
pub enum WheelValidationError {
    #[error("wheel delta was likely unpacked without sign extension (value: {0})")]
    SignExtensionBug(i32),

    #[error("wheel delta exceeds reasonable physical limits (value: {0})")]
    TooFast(i32),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Error)]
pub enum ImeValidationError {
    #[error(
        "composition cursor is invalid or points out of text bounds (cursor: {cursor}, text_len: {text_len})"
    )]
    CursorOutOfBounds { cursor: usize, text_len: usize },

    #[error(
        "composition attributes length does not match text length (attrs: {attrs_len}, text: {text_len})"
    )]
    AttributeLengthMismatch { attrs_len: usize, text_len: usize },

    #[error("text contains invalid control characters or null bytes")]
    CorruptedText,
}

/// A collection of built-in validation helpers for incoming window events.
///
/// Provides standard validation routines for raw, unverified data ([`Unvalidated`][crate::guard::Unvalidated])
/// originating from Win32 APIs, guarding against out-of-bounds panics, integer overflow, and system quirks.
///
/// Designed to be passed directly as function pointers into [`validate_with`][crate::guard::Unvalidated::validate_with].
///
/// # Available Validators
///
/// - [`window_bounds`][Self::window_bounds]: Guards against zero, negative, and oversized dimensions (minimization & GPU limits).
/// - [`cursor_bounds`][Self::cursor_bounds]: Catches corrupted coordinates outside virtual bounds (preserves valid negative drag coords).
/// - [`keyboard_input`][Self::keyboard_input]: Filters out IME composition keys (`VK_PROCESSKEY`) and invalid 8-bit keycodes.
/// - [`wheel_event`][Self::wheel_event]: Detects sign-extension unpacking bugs and runaway scroll deltas.
/// - [`ime_event`][Self::ime_event]: Prevents null bytes, cursor overflows, and attribute length mismatches.
///
/// # Examples
///
/// ```
/// use michiu::prelude::*;
///
/// # fn test(event: MichiuEvent) {
/// match event {
///     MichiuEvent::Resized(phy_size) => {
///         let valid_size = phy_size
///             .validate_with(MichiuValidator::window_bounds)
///             .map_or(PhysicalSize::new(1, 1), Validated::into_inner);
///     }
///     MichiuEvent::CursorMoved { position } => {
///         if let Ok(pos) = position.validate_with(MichiuValidator::cursor_bounds) {
///             // Process only valid coordinates
///         }
///     }
///     _ => {}
/// }
/// # }
/// ```
pub struct MichiuValidator;
impl MichiuValidator {
    // 最大テクスチャサイズの目安
    pub const MAX_WINDOW_DIMENSION: i32 = 16_384;

    // Win32の仮想スクリーンとしてあり得る最大境界
    pub const MIN_CURSOR_COORD: i32 = -32_768;
    pub const MAX_CURSOR_COORD: i32 = 32_767;

    // 1フレームあたりの妥当な最大デルタ
    pub const MAX_REASONABLE_DELTA: i32 = 6_000;

    /// Validates that the physical window size is within a safe, renderable range.
    ///
    /// Checks that the dimensions are non-negative, non-zero, and do not exceed
    /// the maximum allowed texture limit ([`MAX_WINDOW_DIMENSION`](MichiuValidator::MAX_WINDOW_DIMENSION)).
    ///
    /// # Errors
    ///
    /// Returns [`MichiuError::InvalidSize`] under the following conditions:
    ///
    /// - **[`SizeValidationError::Negative`]**: If either `width` or `height` is less than 0.
    /// - **[`SizeValidationError::Zero`]**: If either `width` or `height` is equal to 0 (e.g., when minimized).
    /// - **[`SizeValidationError::TooLarge`]**: If either dimension exceeds [`MAX_WINDOW_DIMENSION`](MichiuValidator::MAX_WINDOW_DIMENSION).
    ///
    /// # Examples
    ///
    /// ```
    /// use michiu::prelude::*;
    ///
    /// assert!(MichiuValidator::window_bounds(PhysicalSize::new(800, 600)).is_ok());
    ///
    /// assert!(MichiuValidator::window_bounds(PhysicalSize::new(0, 600)).is_err());
    /// assert!(MichiuValidator::window_bounds(PhysicalSize::new(MichiuValidator::MAX_WINDOW_DIMENSION + 1, 600)).is_err());
    /// ```
    pub fn window_bounds(size: PhysicalSize) -> Result<PhysicalSize> {
        let (w, h) = (size.width, size.height);
        if w < 0 || h < 0 {
            return Err(MichiuError::InvalidSize(SizeValidationError::Negative {
                width: w,
                height: h,
            }));
        }
        if w == 0 || h == 0 {
            return Err(MichiuError::InvalidSize(SizeValidationError::Zero {
                width: w,
                height: h,
            }));
        }
        if w > MichiuValidator::MAX_WINDOW_DIMENSION || h > MichiuValidator::MAX_WINDOW_DIMENSION {
            return Err(MichiuError::InvalidSize(SizeValidationError::TooLarge {
                width: w,
                height: h,
            }));
        }
        Ok(size)
    }

    /// Validates that the cursor position is within a reasonable virtual screen boundary.
    ///
    /// Checks that the coordinates are within the Win32 virtual screen limits ([`MIN_CURSOR_COORD`](MichiuValidator::MIN_CURSOR_COORD)..=[`MAX_CURSOR_COORD`](MichiuValidator::MAX_CURSOR_COORD)) to prevent corrupted data or integer overflow.
    ///
    /// # Notes
    ///
    /// Negative coordinates are considered valid to properly support multi-monitor setups (screens positioned to the left or top of the primary monitor) and mouse drag operations captured outside the window.
    ///
    /// # Errors
    ///
    /// Returns [`MichiuError::InvalidPoint`] ([`PointValidationError::OutOfBounds`])
    /// if either the X or Y coordinate is outside the allowed virtual bounds.
    ///
    /// # Examples
    ///
    /// ```
    /// use michiu::prelude::*;
    ///
    /// assert!(MichiuValidator::cursor_bounds(PhysicalPoint::new(100, 200)).is_ok());
    /// assert!(MichiuValidator::cursor_bounds(PhysicalPoint::new(-50, -100)).is_ok());
    ///
    /// assert!(MichiuValidator::cursor_bounds(PhysicalPoint::new(MichiuValidator::MAX_CURSOR_COORD + 1, 0)).is_err());
    /// assert!(MichiuValidator::cursor_bounds(PhysicalPoint::new(0, MichiuValidator::MIN_CURSOR_COORD - 1)).is_err());
    /// ```
    pub fn cursor_bounds(point: PhysicalPoint) -> Result<PhysicalPoint> {
        let (x, y) = (point.x, point.y);

        // 仮想スクリーンの範囲を逸脱していないかチェック
        if !(MichiuValidator::MIN_CURSOR_COORD..=MichiuValidator::MAX_CURSOR_COORD).contains(&x)
            || !(MichiuValidator::MIN_CURSOR_COORD..=MichiuValidator::MAX_CURSOR_COORD).contains(&y)
        {
            return Err(MichiuError::InvalidPoint(
                PointValidationError::OutOfBounds { x, y },
            ));
        }

        Ok(point)
    }

    /// Validates that a [`VIRTUAL_KEY`] represents a valid, dispatchable keystroke.
    ///
    /// Filters out undefined/corrupted values (outside the 8-bit range), mouse button codes,
    /// and special keys that should not be handled as physical keystrokes (such as IME compositions).
    ///
    /// # Notes
    ///
    /// - **IME Input**: When typing with an active IME (e.g., Japanese input), [`VK_PROCESSKEY`] is emitted.
    ///   This is rejected to prevent triggering UI shortcut keys accidentally.
    /// - **Text Injection**: [`VK_PACKET`], which is used to inject raw Unicode characters via software keyboards
    ///   or input tools, is also rejected as it should be handled by text/character input events instead.
    ///
    /// # Errors
    ///
    /// Returns [`MichiuError::Invalidkey`] under the following conditions:
    ///
    /// - **[`KeyValidationError::Zero`]**: The key code is 0 (undefined).
    /// - **[`KeyValidationError::OutOfRange`]**: The code exceeds the standard 8-bit virtual key range (`> 0xFF`).
    /// - **[`KeyValidationError::MouseButton`]**: The code is a mouse button virtual key (`VK_LBUTTON`..=`VK_XBUTTON2`).
    /// - **[`KeyValidationError::ImeProcessKey`]**: The key is an active IME composition key ([`VK_PROCESSKEY`]).
    /// - **[`KeyValidationError::PacketKey`]**: The key is a character packet injection ([`VK_PACKET`]).
    ///
    /// # Examples
    ///
    /// ```
    /// use michiu::prelude::*;
    /// use windows::Win32::UI::Input::KeyboardAndMouse::*;
    ///
    /// assert!(MichiuValidator::keyboard_input(VK_RETURN).is_ok());
    /// assert!(MichiuValidator::keyboard_input(VK_A).is_ok());
    ///
    /// assert!(MichiuValidator::keyboard_input(VIRTUAL_KEY(0)).is_err());
    /// assert!(MichiuValidator::keyboard_input(VIRTUAL_KEY(0x0100)).is_err());
    ///
    /// assert!(MichiuValidator::keyboard_input(VK_PROCESSKEY).is_err());
    /// assert!(MichiuValidator::keyboard_input(VK_LBUTTON).is_err());
    /// ```
    pub fn keyboard_input(key_code: VIRTUAL_KEY) -> Result<VIRTUAL_KEY> {
        let code = key_code.0;

        // 範囲チェック (0 または 256 以上)
        if code == 0 {
            return Err(MichiuError::Invalidkey(KeyValidationError::Zero));
        }
        if code > 0xFF {
            return Err(MichiuError::Invalidkey(KeyValidationError::OutOfRange(
                code,
            )));
        }

        // マウスボタンの混入チェック
        if (VK_LBUTTON.0..=VK_XBUTTON2.0).contains(&code) {
            return Err(MichiuError::Invalidkey(KeyValidationError::MouseButton(
                code,
            )));
        }

        // IME・特殊入力のチェック
        if key_code == VK_PROCESSKEY {
            return Err(MichiuError::Invalidkey(KeyValidationError::ImeProcessKey));
        }
        if key_code == VK_PACKET {
            return Err(MichiuError::Invalidkey(KeyValidationError::PacketKey));
        }

        Ok(key_code)
    }

    /// Validates that a raw mouse wheel delta ([`WheelDelta`]) is within a safe, physical range.
    ///
    /// Protects against common Win32 sign-extension bugs (where negative values turn into ~65,000) and runaway scroll values caused by faulty drivers or erratic touchpads.
    ///
    /// # Notes
    ///
    /// - **Zero Deltas**: A value of `0` is considered valid by this function alone, because Win32 delivers horizontal and vertical wheel messages separately (meaning one axis is normally `0`).
    ///   Checking whether *both* axes are zero should be handled by the caller.
    ///
    /// # Errors
    ///
    /// Returns [`MichiuError::InvalidWheel`] under the following conditions:
    ///
    /// - **[`WheelValidationError::SignExtensionBug`]**: The value falls within `32_768..=65_535`.
    ///   This strongly indicates a 16-bit signed `short` was mistakenly unpacked without proper sign extension.
    /// - **[`WheelValidationError::TooFast`]**: The absolute value exceeds [`MAX_REASONABLE_DELTA`](MichiuValidator::MAX_REASONABLE_DELTA) (~50 notches).
    ///
    /// # Examples
    ///
    /// ```
    /// use michiu::prelude::*;
    ///
    /// assert!(MichiuValidator::wheel_event(WheelDelta(120)).is_ok());
    /// assert!(MichiuValidator::wheel_event(WheelDelta(-120)).is_ok());
    ///
    /// assert!(MichiuValidator::wheel_event(WheelDelta(65416)).is_err());
    /// assert!(MichiuValidator::wheel_event(WheelDelta(MichiuValidator::MAX_REASONABLE_DELTA + 1)).is_err());
    /// ```
    pub fn wheel_event(raw_delta: WheelDelta) -> Result<WheelDelta> {
        let val = raw_delta.0;

        // 符号なし拡張バグの検知 (0xFF88 などが 65000 付近になるのを弾く)
        if (32_768..=65_535).contains(&val) {
            return Err(MichiuError::InvalidWheel(
                WheelValidationError::SignExtensionBug(val),
            ));
        }

        // 極端な値 (-6000 ..= 6000 を超えるものは弾く)
        if val.abs() > MichiuValidator::MAX_REASONABLE_DELTA {
            return Err(MichiuError::InvalidWheel(WheelValidationError::TooFast(
                val,
            )));
        }

        Ok(raw_delta)
    }

    /// Validates that an [`ImeStateUpdate`] payload is safe for memory and string access.
    ///
    /// Guards against panic-inducing conditions in the UI layer, such as null-byte contamination from Win32 C-strings, out-of-bounds cursor positions (e.g., unsigned overflow from `-1`),
    /// and length mismatches between composition attributes and text.
    ///
    /// # Notes
    ///
    /// - **End-of-text Cursor**: A `composition_cursor` pointing exactly at the end of the string (`cursor == char_count`) is valid and indicates the cursor is at the tail.
    ///
    /// # Errors
    ///
    /// Returns [`MichiuError::InvalidIme`] under the following conditions:
    ///
    /// - **[`ImeValidationError::CorruptedText`]**: Either `composition_text` or `result_text`
    ///   contains embedded null bytes (`\0`).
    /// - **[`ImeValidationError::CursorOutOfBounds`]**: `composition_cursor` exceeds the character
    ///   count of `composition_text`.
    /// - **[`ImeValidationError::AttributeLengthMismatch`]**: The length of `composition_attrs`
    ///   does not match the character count of `composition_text`.
    ///
    /// # Examples
    ///
    /// ```
    /// use michiu::{prelude::*, window::ImeStateUpdate};
    ///
    /// // Valid IME composition state
    /// let mut ime = ImeStateUpdate::default();
    /// ime.composition_text = "Typing".into();
    /// ime.composition_cursor = 6; // Cursor at the end is valid
    /// assert!(MichiuValidator::ime_event(ime).is_ok());
    ///
    /// // Cursor beyond text boundary returns an error
    /// let mut invalid_ime = ImeStateUpdate::default();
    /// invalid_ime.composition_text = "Text".into();
    /// invalid_ime.composition_cursor = 10;
    /// assert!(MichiuValidator::ime_event(invalid_ime).is_err());
    /// ```
    pub fn ime_event(ime: ImeStateUpdate) -> Result<ImeStateUpdate> {
        // Null文字の混入
        if ime.composition_text.contains('\0') || ime.result_text.contains('\0') {
            return Err(MichiuError::InvalidIme(ImeValidationError::CorruptedText));
        }

        let char_count = ime.composition_text.chars().count();

        // カーソル位置
        // 末尾 (cursor == char_count) は正常
        if ime.composition_cursor > char_count {
            return Err(MichiuError::InvalidIme(
                ImeValidationError::CursorOutOfBounds {
                    cursor: ime.composition_cursor,
                    text_len: char_count,
                },
            ));
        }

        // 属性配列の長さ一致
        if !ime.composition_text.is_empty()
            && !ime.composition_attrs.is_empty()
            && ime.composition_attrs.len() != char_count
        {
            return Err(MichiuError::InvalidIme(
                ImeValidationError::AttributeLengthMismatch {
                    attrs_len: ime.composition_attrs.len(),
                    text_len: char_count,
                },
            ));
        }

        Ok(ime)
    }
}

#[macro_export]
macro_rules! debug_eprintln {
    ($($arg:tt)*) => {
        #[cfg(debug_assertions)]
        eprintln!($($arg)*);
    };
}
