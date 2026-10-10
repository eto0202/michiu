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
//! fn main() -> Result<(), Box<dyn std::error::Error>> {
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
//!     // MichiuRenderer::new is async. Use any executor of your choice
//!         (e.g., pollster, tokio, futures).
//!     let renderer = pollster::block_on(MichiuRenderer::new(
//!         handle.hwnd(),
//!         LayoutSize::new(600.0, 500.0),
//!         scale_factor,
//!     ))?;
//!
//!     // 3. Build UI and application host
//!     let (mut app, mut pump) = MichiuAppBuilder::new(window).build_with_ui(renderer, move || {
//!         let (count, set_count) = create_signal(0u32); // Initialize the signal.
//!
//!         flex(
//!             ts().size_full()
//!             .justify_center()
//!             .items_center()
//!             .bg_color(rgb(22, 24, 29)))
//!         .child(
//!             flex(
//!                 ts().size((200.0, 100.0))
//!                 .r(6.0)
//!                 .justify_center()
//!                 .border_solid(2.0)
//!                 .border_color(rgb(209, 92, 174))
//!                 .pressed(ts().transform_scale(0.98, 0.98)),
//!             )
//!             .on_click(move || set_count.set(count.get() + 1))
//!             .label(
//!                 move || format!("Count: {}", count.get()),
//!                 ts().text_color(rgb(156, 158, 163)).font_size(22.0),
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
//! This callback is automatically invoked whenever a background task is submitted via [`MichiuTaskSender`](crate::ui::MichiuTaskSender) or [`SignalSender`](crate::SignalSender).
//!
//! ### Note (Automatic Setup)
//! You **rarely need to call this manually**. When using [`MichiuAppBuilder`](crate::MichiuAppBuilder),
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
//! call this method inside your `WM_NULL` handler, then check [`has_dirty`](Self::has_dirty) to determine if a redraw is needed:
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
use crate::guard::Validated;
use crate::ui::{
    CapacityConfig, CharIndex, ImeState, VirtualKey, prelude::*, raw_wheel_delta_to_logical_pixels,
};
use crate::window::{WindowHandle, prelude::*};
use windows::Win32::UI::WindowsAndMessaging::{WM_ENTERSIZEMOVE, WM_EXITSIZEMOVE, WM_NULL};

#[cfg(feature = "ui")]
#[doc(inline)]
pub use michiu_ui as ui;

#[cfg(feature = "guard")]
#[doc(inline)]
pub use michiu_guard as guard;

#[cfg(feature = "window")]
#[doc(inline)]
pub use michiu_window as window;

pub mod prelude {
    pub use crate::guard::{Unvalidated, Validate, Validated};
    pub use crate::ui::prelude::*;
    pub use crate::window::prelude::*;
    pub use crate::*;
}
pub mod accessibility {
    pub use crate::ui::accessibility::*;
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
/// # fn run(window: MichiuWindow, renderer: MichiuRenderer) -> Result<(), Box<dyn std::error::Error>> {
/// let (mut app, mut pump) = MichiuAppBuilder::new(window)
///     .build_with_ui(renderer, move || {
///         // Building the UI Tree
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
/// # fn event_loop(mut app: MichiuApp, mut pump: MichiuEventPump) -> Result<(), Box<dyn std::error::Error>> {
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

    /// The `EntityId` of the root element.
    ///
    /// Private for security reasons.
    ///
    /// Set using [`MichiuApp::set_root`], [`MichiuApp::build_ui`],
    /// or [`MichiuAppBuilder::build_with_ui`],
    /// and retrieve using [`MichiuApp::root_id`] or [`MichiuApp::try_root_id`].
    root_id: Option<EntityId>,

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
/// # fn handle(app: &mut MichiuApp, event: MichiuAnyEvent, raw: RawEvent) {
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
/// Indicates whether a UI element (such as a button or input field) has processed this action.
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
    /// If `true`, it means a UI element was clicked or a text input field has focus.
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
    /// It builds the UI tree and automatically registers it as the root element.
    ///
    /// Since `set_root` is called automatically,
    /// you can use `root_id` in subsequent processing to retrieve the `EntityId` of the root element.
    #[inline]
    pub fn build_ui<F>(&mut self, f: F) -> EntityId
    where
        F: FnOnce() -> Element,
    {
        let root = build_ui(&mut self.context, f);
        let id = root.id();
        self.set_root(id);
        id
    }

    /// Register the root element.
    /// From this point on, you can retrieve the root element using [`MichiuApp::root_id`].
    #[inline]
    pub fn set_root(&mut self, root_id: EntityId) {
        self.root_id = Some(root_id);
    }

    /// Retrieves the `EntityId` of the root element.
    ///
    /// To avoid a panic, use [`MichiuApp::try_root_id`] instead.
    ///
    /// # Panics
    ///
    /// Panics if the root element has not yet been registered.
    ///
    /// Please register the root element in advance by calling [`MichiuApp::build_ui`],
    /// [`MichiuAppBuilder::build_with_ui`], or
    /// [`MichiuApp::set_root`].
    ///
    /// If this error occurs even though the root element has been registered correctly,
    /// please report it to [GitHub Issues](https://github.com/eto0202/michiu/issues).
    #[must_use]
    #[inline]
    #[allow(clippy::expect_used)]
    pub fn root_id(&self) -> EntityId {
        self.root_id.expect(
            "The root ID was not found.\
             Possible cause:\
                - The root ID was not registered. Call `build_ui`, `build_with_ui`, or `set_root` first.",
        )
    }

    /// Retrieves the `EntityId` of the root element.
    ///
    /// Returns `None` if it has not yet been registered.
    #[must_use]
    #[inline]
    pub fn try_root_id(&self) -> Option<EntityId> {
        self.root_id
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
    /// # fn run(mut app: MichiuApp, mut pump: MichiuEventPump) -> Result<(), Box<dyn std::error::Error>> {
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
    #[allow(clippy::too_many_lines)]
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
                    let size = phy_size.assume_valid().into_inner();
                    let width = size.width as u32;
                    let height = size.height as u32;
                    // レンダラーのリサイズとレイアウト物理サイズの更新
                    self.renderer
                        .resize(width, height, self.renderer.scale_factor());

                    // 再描画要求
                    needs_redraw = true;
                    needs_update_window = true;
                }
                MichiuEvent::CursorMoved { position } => {
                    let pos = position.assume_valid().into_inner();
                    let x = pos.x as f32;
                    let y = pos.y as f32;
                    let phys_pos = LayoutPoint::new(x, y);
                    let logical_pos = LayoutPoint::new(
                        x / self.renderer.scale_factor(),
                        y / self.renderer.scale_factor(),
                    );

                    self.context
                        .inject_user_action(UserAction::PointerMove(logical_pos));

                    if self.config.external_visual_support {
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
                MichiuEvent::CursorLeft => {
                    // ウィンドウ外に去ったため、論理空間外へポインタを移動させてホバーを確実に解除
                    self.context
                        .inject_user_action(UserAction::PointerMove(LayoutPoint::new(
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
                    let button = match button {
                        michiu_window::MouseButton::Left => MouseButton::Left,
                        michiu_window::MouseButton::Right => MouseButton::Right,
                        michiu_window::MouseButton::Middle => MouseButton::Middle,
                        michiu_window::MouseButton::Other(_) => MouseButton::X1,
                    };
                    let modifiers = Modifiers {
                        shift: modifiers.contains(michiu_window::Modifiers::SHIFT),
                        ctrl: modifiers.contains(michiu_window::Modifiers::CONTROL),
                        alt: modifiers.contains(michiu_window::Modifiers::ALT),
                        logo: modifiers.contains(michiu_window::Modifiers::LOGO),
                    };
                    let state = match state {
                        michiu_window::ElementState::Pressed => ElementState::Pressed,
                        michiu_window::ElementState::Released => ElementState::Released,
                    };

                    self.context.inject_user_action(UserAction::PointerButton {
                        button,
                        state,
                        modifiers,
                    });

                    if click_count == &2 {
                        self.context
                            .inject_user_action(UserAction::PointerDoubleClick { modifiers });
                    }

                    if self.config.external_visual_support {
                        let x = (raw.lparam.0 & 0xffff) as i16 as f32;
                        let y = ((raw.lparam.0 >> 16) & 0xffff) as i16 as f32;
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
                    self.context.inject_user_action(UserAction::Character(*c));
                    consumed = has_focused;
                    needs_redraw = true;
                }
                MichiuEvent::KeyboardInput {
                    key_code,
                    modifiers,
                    state,
                } => {
                    let mut shortcut_handled = false;

                    let key_code = key_code.assume_valid().into_inner();
                    let key = VirtualKey::from_windows(key_code);
                    let modifiers = Modifiers {
                        shift: modifiers.contains(michiu_window::Modifiers::SHIFT),
                        ctrl: modifiers.contains(michiu_window::Modifiers::CONTROL),
                        alt: modifiers.contains(michiu_window::Modifiers::ALT),
                        logo: modifiers.contains(michiu_window::Modifiers::LOGO),
                    };
                    let state = match state {
                        michiu_window::ElementState::Pressed => ElementState::Pressed,
                        michiu_window::ElementState::Released => ElementState::Released,
                    };

                    if modifiers.ctrl
                        && self.config.default_shortcuts
                        && state == ElementState::Pressed
                    {
                        match raw.wparam.0 as i32 {
                            // Ctrl + C
                            0x43 if let Some(selected_text) = self.context.get_selected_text() => {
                                let _ = set_win32_clipboard(&selected_text);
                                shortcut_handled = true;
                            }
                            // Ctrl + V
                            0x56 if has_focused
                                && let Some(pasted_text) = get_win32_clipboard() =>
                            {
                                self.context
                                    .inject_user_action(UserAction::Paste(pasted_text.into()));
                                shortcut_handled = true;
                            }
                            // Ctrl + X (切り取り)
                            0x58 if let Some(selected_text) = self.context.get_selected_text() => {
                                let _ = set_win32_clipboard(&selected_text);
                                self.context.inject_user_action(UserAction::Cut);
                                shortcut_handled = true;
                            }
                            // Ctrl + Z (Undo)
                            0x5A if has_focused => {
                                self.context.inject_user_action(UserAction::Undo);
                                shortcut_handled = true;
                            }
                            // Ctrl + Y (Redo)
                            0x59 if has_focused => {
                                self.context.inject_user_action(UserAction::Redo);
                                shortcut_handled = true;
                            }
                            _ => {}
                        }
                    }

                    self.context.inject_user_action(UserAction::KeyboardKey {
                        key,
                        state,
                        modifiers,
                    });

                    consumed = has_focused || shortcut_handled;
                    needs_redraw = true;
                }
                MichiuEvent::MouseWheel {
                    raw_delta_x,
                    raw_delta_y,
                } => {
                    let mut pt = windows::Win32::Foundation::POINT {
                        x: (raw.lparam.0 & 0xffff) as i16 as i32,
                        y: ((raw.lparam.0 >> 16) & 0xffff) as i16 as i32,
                    };
                    let _ = unsafe {
                        windows::Win32::Graphics::Gdi::ScreenToClient(
                            self.handle.hwnd(),
                            &raw mut pt,
                        )
                    };

                    // 横スクロール時は、右チルト（プラス値）された際に
                    // 右方向へスクロール（オフセット加算）させるため、符号の方向性を補正
                    let scroll_x = {
                        let x = raw_delta_x.assume_valid().into_inner();
                        -raw_wheel_delta_to_logical_pixels(x.raw() as f32)
                    };

                    let scroll_y = {
                        let y = raw_delta_y.assume_valid().into_inner();
                        raw_wheel_delta_to_logical_pixels(y.raw() as f32)
                    };

                    self.context
                        .inject_user_action(UserAction::MouseWheel { scroll_x, scroll_y });

                    if self.config.external_visual_support {
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
                MichiuEvent::Ime(ime) => {
                    let ime = ime.clone().assume_valid().into_inner();
                    let ime_state = ImeState {
                        is_open: ime.is_open,
                        conversion_mode: ime.conversion_mode,
                        sentence_mode: ime.sentence_mode,
                        keyboard_layout_id: ime.keyboard_layout_id,
                        composition_text: ime.composition_text.into(),
                        result_text: ime.result_text.into(),
                        caret_position: ime.caret_position.map(|p| LayoutPoint {
                            x: p.x as f32,
                            y: p.y as f32,
                        }),
                        composition_cursor: CharIndex(ime.composition_cursor),
                        composition_attrs: ime.composition_attrs,
                    };

                    self.context.inject_user_action(UserAction::Ime(ime_state));
                    consumed = has_focused;
                    needs_redraw = true;
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
    /// 2. **Cursor Shape Update**: Synchronizes the mouse cursor icon based on the latest hover element.
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
    /// In debug builds, this method panics if the root element (`root_id`) has not been registered.
    ///
    /// In release builds, it returns early without performing any operations.
    pub fn standard_redraw(&mut self) {
        debug_assert!(
            self.try_root_id().is_some(),
            "The root ID was not found.\
             Possible cause:\
               - The root ID was not registered. Call `build_ui`, `build_with_ui`, or `set_root` first."
        );

        let Some(root_id) = self.try_root_id() else {
            return;
        };

        self.context.begin_frame();

        self.standard_update_cursor_icon();

        self.context.tick_system_frame(&TickType::All);
        self.context
            .sync_layout(root_id, self.renderer.layout_size());
        self.renderer.update_composition_tree(&mut self.context);

        if self.config.accessibility_support {
            self.context.update_accessibility();
        }

        if let Some(_ctx) = self.window.begin_paint() {
            self.renderer.draw(&mut self.context);
        }

        if self.context.has_active_frame() {
            if self.config.sync_mode == AutoSyncMode::DwmFlush {
                self.handle.dwm_flush();
            }
            self.handle.redraw_requested();
        }
    }

    /// Updates the window cursor icon based on the currently hovered UI element.
    ///
    /// This standard method resolves the appropriate cursor icon for the hovered element and automatic applies it to the OS.
    /// Normally, this is executed automatically within [`standard_redraw`](Self::standard_redraw).
    /// Call this manually if you are constructing a custom rendering pipeline.
    ///
    /// # Behavior
    /// 1. **Configuration Guard**: If [`MichiuBuilderConfig::auto_resolve_cursor`] is `false`, this method returns immediately without doing anything.
    /// 2. **Hover Lookup**: Queries the latest hovered element (`InteractionState::Hovered`) from [`Context`].
    /// 3. **Fallback**: If no element is hovered, or if the hovered element specifies no custom cursor, it falls back to the default system cursor ([`CursorIcon::Default`]).
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
                .set_cursor_icon(michiu_window::CursorIcon::Other(hcursor));
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
#[allow(clippy::struct_excessive_bools)]
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

    /// Whether to automatically update the window's cursor icon based on the element being hovered over.
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
/// # fn run(window: MichiuWindow, renderer: MichiuRenderer) -> Result<(), Box<dyn std::error::Error>> {
/// let (mut app, mut pump) = MichiuAppBuilder::new(window)
///     .with_auto_resolve_cursor(true)
///     .with_default_shortcuts(true)
///     .build_with_ui(renderer, move || {
///         // Root UI tree
///     })?;
/// # Ok(())
/// # }
/// ```
#[allow(clippy::struct_excessive_bools)]
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
    /// # Regarding the State of the Root Element
    /// At the time this method is called, the root element ([`root_id`](MichiuApp::try_root_id)) remains `None`.
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
    /// # fn run(window: MichiuWindow, renderer: MichiuRenderer) -> Result<(), Box<dyn std::error::Error>> {
    /// let (mut app, mut pump) = MichiuAppBuilder::new(window)
    ///     .with_accessibility_support(true)
    ///     .build(renderer)?;
    ///
    /// // Build UI and register the root element
    /// app.build_ui(move || {
    ///     // UI tree
    /// });
    ///
    /// // Make the window visible AFTER initialization
    /// app.set_visible(true);
    /// # Ok(())
    /// # }
    /// ```
    #[inline]
    pub fn build(
        self,
        renderer: MichiuRenderer,
    ) -> michiu_window::Result<(MichiuApp, MichiuEventPump)> {
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
                root_id: None,
                config: self.config,
            },
            pump,
        ))
    }

    /// Uses the builder to create [`MichiuApp`] and [`MichiuEventPump`], and performs the construction and registration of the root UI element in a single step.
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
    /// # fn run(window: MichiuWindow, renderer: MichiuRenderer) -> Result<(), Box<dyn std::error::Error>> {
    /// let (mut app, mut pump) = MichiuAppBuilder::new(window)
    ///     .build_with_ui(renderer, move || {
    ///         // Root UI tree is automatically registered as the application root
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
    ) -> michiu_window::Result<(MichiuApp, MichiuEventPump)>
    where
        F: FnOnce() -> Element,
    {
        let (mut app, pump) = self.build(renderer)?;
        app.build_ui::<F>(ui_fn);
        Ok((app, pump))
    }

    /// Whether to automatically update the window's cursor icon based on the element being hovered over.
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
    /// # fn run(window: MichiuWindow, renderer: MichiuRenderer) -> Result<(), Box<dyn std::error::Error>> {
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
    /// # fn run(window: MichiuWindow, renderer: MichiuRenderer) -> Result<(), Box<dyn std::error::Error>> {
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
