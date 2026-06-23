#![cfg_attr(docsrs, feature(doc_cfg))]
//! # michiu_window
//!
//! `michiu_window` is a lightweight, robust, and modern Win32 window management library
//! designed with strict thread-safety and modern Rust paradigms.
//! ## Quick Start Guide
//!
//! Here is a minimal, production-ready example demonstrating how to initialize High-DPI support,
//! configure and build a simple window, drive the event loop efficiently using `wait_event` ,
//! and cleanly destroy the window directly from within the loop using its thread-safe handle.
//! ```no_run
//! use michiu_window::{init_dpi_awareness, WindowBuilder, Window, Event, EventPump, MichiuEvent};
//!
//! fn main() -> Result<(), Box<dyn std::error::Error>> {
//!     // 1. Initialize High-DPI support (Per-Monitor v2)
//!     init_dpi_awareness();
//!
//!     // 2. Configure, validate, and build a simple window
//!     let builder = WindowBuilder::new().with_title("Michiu Minimal Window");
//!     // Validate the builder settings here to guarantee window safety.
//!     let validated = builder.into_unvalidated().try_into()?;
//!     let window = Window::build(validated)?;
//!     // Explicitly assume the handle is valid (since the window was just built).
//!     let handle = window.handle().assume_valid();
//!
//!     // 3. Initialize the EventPump to drive the message loop on the UI thread
//!     let mut event_pump = EventPump::new();
//!
//!     // 4. Event-driven loop using `wait_event()`
//!     while let Some(event) = event_pump.wait_event()? {
//!         // Note that MichiuEvent also includes a User variant, not just Window.
//!         if let MichiuEvent::Window { event, .. } = event {
//!             match event {
//!                 Event::CloseRequested => {
//!                     // You can also handle close confirmation logic here.
//!                     // Destroy the window directly from the event loop using the handle
//!                     handle.destroy();
//!                 }
//!                 Event::Destroyed => {
//!                     // Post-processing after the window is completely destroyed.
//!                     // Exit the event loop.
//!                     break;
//!                 }
//!                 _ => {}
//!             }
//!         }
//!     }
//!     Ok(())
//! }
//! ```
//! ### Architectural Note: The OS Input Firewall
//!
//! You might wonder why we need to call `.into_unvalidated().try_into()?` or `.assume_valid()`.
//!
//! Under the hood, `michiu` utilizes our core safety crate, [`michiu_guard`], to protect the framework from untrusted OS inputs. To prevent bugs and security issues, the framework enforces a strict compile-time boundary: core APIs only accept validated types (`Validated<T>`).
//!
//! To learn more about this design philosophy and how it secures your application, check out the [`michiu_guard`] crate documentation.
//!
//! ---
//!
//! ## Choosing Your Event Loop Model
//!
//! `michiu_window` provides two distinct message-polling models designed for different application architectures.
//!
//! ### Option A: `wait_event` (Blocking / Event-Driven) — Recommended
//! **Best Used For**: Desktop utilities, system tray tools, office applications, and general GUI software.
//!
//! It completely suspends the UI thread while idle. It unblocks immediately
//! when the OS generates window messages or when a background thread calls [`WindowHandle::wake_up()`] or
//! [`EventSender::send_event()`].
//!
//! ---
//!
//! ### Option B: `poll_event` (Non-blocking / Polling)
//! **Best Used For**: Real-time games, CAD systems, and high-performance interactive graphics canvases.
//!
//! This method is non-blocking. It instantly processes all available OS messages and continues running,
//! allowing you to update state and render frames continuously (e.g. 60 FPS rendering cycle).
//!
//! ```no_run
//! use michiu_window::{init_dpi_awareness, WindowBuilder, Window, EventPump, MichiuEvent, Event};
//!
//! fn main() -> Result<(), Box<dyn std::error::Error>> {
//!     init_dpi_awareness();
//!
//!     let builder = WindowBuilder::new().with_title("Michiu Real-Time Window");
//!     let window = Window::build(builder.into_unvalidated().try_into()?)?;
//!     let handle = window.handle().assume_valid();
//!     let mut event_pump = EventPump::new();
//!
//!     'main_loop: loop {
//!         // Non-blocking poll; processes all currently pending OS messages instantly
//!         while let Some(event) = event_pump.poll_event() {
//!             match event {
//!                 MichiuEvent::Window { id, event } => match event {
//!                     Event::CloseRequested => {
//!                         handle.destroy();
//!                     }
//!                     Event::Destroyed => {
//!                         break 'main_loop;
//!                     }
//!                     _ => {}
//!                 }
//!                 _ => {}
//!             }
//!         }
//!
//!         // Update your game states and redraw frames continuously here...
//!
//!         // Throttle the loop to target ~60 FPS
//!         std::thread::sleep(std::time::Duration::from_millis(16));
//!     }
//!
//!     Ok(())
//! }
//! ```
//!
//! ---
//!
//! ## Thread-Affinity and Concurrency Model
//!
//! Windows GUI elements are strictly **thread-affine**; raw Win32 APIs that manipulate
//! windows must be executed exclusively on the UI thread that created them.
//!
//! To resolve this constraint without losing Rust's memory safety, `michiu_window` separates
//! window lifecycle management into two key structures:
//!
//! - [`Window`]: Represents the active, owned window instance. It is strictly **`!Send` and `!Sync`**
//!   and must remain on the UI thread where the message loop runs.
//! - [`WindowHandle`]: A Cloneable handle. It is **`Send` and `Sync`**
//!   and can be safely passed to any background worker threads.
//!
//! This library provides two approaches to interact with the UI thread from
//! background threads.
//!
//! ---
//!
//! ## Approach 1: Direct Manipulation (Convenience & Escaping)
//!
//! Ideal for simple scripts, quick utilities, or direct, localized window adjustments.
//!
//! `WindowHandle` implements several thread-safe wrapper methods (like `set_title`, `set_size`,
//! and `set_visible`). These methods automatically check which thread they are currently on:
//! - If called on the UI thread, they execute the underlying Win32 API immediately.
//! - If called on a background thread, they automatically wrap the request in a command
//!   and asynchronously route it to the UI thread via `PostMessageW`.
//!
//! ### The Ultimate Escape Hatch: `run_on_ui_thread`
//! For advanced Win32 operations not natively wrapped by the library, `WindowHandle` provides
//! the `run_on_ui_thread` method. This allows you to dispatch any arbitrary closure to be
//! executed safely and asynchronously on the UI thread.
//!
//! ```no_run
//! use michiu_window::{init_dpi_awareness, WindowBuilder, PhysicalSize, Icon};
//! use windows::Win32::Graphics::Gdi::InvalidateRect;
//!
//! # fn main() -> Result<(), Box<dyn std::error::Error>> {
//! // Always initialize High-DPI support first
//! init_dpi_awareness();
//!
//! let builder = WindowBuilder::new().with_title("Michiu Window");
//! let validated = builder.into_unvalidated().try_into()?;
//! let window = michiu_window::Window::build(validated)?;
//!
//! let window_handle = window.handle().assume_valid();
//!
//! // Spawn a background worker thread
//! std::thread::spawn(move || {
//!     // 1. Using safe, direct thread-routing helpers
//!     window_handle.set_title("Processing data...");
//!     window_handle.set_size(PhysicalSize { width: 1024, height: 768 });
//!
//!     // 2. Using the unsafe escape hatch for custom Win32 calls on the UI thread
//!     unsafe {
//!         window_handle.run_on_ui_thread(|hwnd| {
//!             // This closure executes safely on the UI thread.
//!             // You can call any raw Win32 APIs here without thread-affinity crashes.
//!             InvalidateRect(Some(hwnd), None, true);
//!         });
//!     }
//! });
//! # Ok(())
//! # }
//! ```
//!
//! ---
//!
//! ## Approach 2: Centralized Architecture (UIs & Complex States)
//!
//! Ideal for production-grade applications or GUI frameworks requiring unidirectional
//! data flow and strict state synchronization.
//!
//! Instead of scattered threads mutating the window directly, background threads emit
//! high-level application events (commands) to a centralized message queue, and the UI thread's
//! event loop processes and applies them sequentially.
//!
//! There are two ways to achieve this:
//!
//! ### Option A: Native `EventSender` (Via `MichiuEvent::User`)
//! You can get a thread-safe [`EventSender`] from `WindowHandle::sender()`. It uses `PostMessageW`
//! internally to send a `Box<dyn Any + Send>` to the UI thread, waking up the event loop safely.
//!
//! ### Option B: Standard Rust `std::sync::mpsc` Channels (With wake_up)
//! You can use standard Rust channels alongside the blocking `wait_event` loop.
//! By calling `handle.wake_up()` after sending data to the channel, you can safely wake up
//! the UI thread's sleep state (GetMessage) to process the queue immediately, avoiding any busy polling.
//!
//! ```no_run
//! use std::sync::mpsc;
//! use michiu_window::{init_dpi_awareness, WindowBuilder, EventPump, MichiuEvent, Event, PhysicalSize};
//!
//! // Define your centralized application commands
//! enum AppCommand {
//!     ResizeWindow(PhysicalSize),
//!     UpdateTitle(String),
//! }
//!
//! # fn main() -> Result<(), Box<dyn std::error::Error>> {
//! // Always initialize High-DPI support first
//! init_dpi_awareness();
//!
//! let builder = WindowBuilder::new().with_title("Centralized App");
//! let validated = builder.into_unvalidated().try_into()?;
//! let window = michiu_window::Window::build(validated)?;
//! let mut event_pump = EventPump::new();
//!
//! // Create a standard MPSC channel
//! let (tx, rx) = mpsc::channel();
//! let handle = window.handle().assume_valid();
//! let handle_clone = handle.clone();
//!
//! // Spawn background thread
//! std::thread::spawn(move || {
//!     // Do heavy work...
//!     std::thread::sleep(std::time::Duration::from_secs(2));
//!
//!     // Send command through the channel
//!     tx.send(AppCommand::ResizeWindow(PhysicalSize { width: 1280, height: 720 })).unwrap();
//!
//!     // Wake up the UI thread's message loop immediately
//!     handle_clone.wake_up();
//! });
//!
//! 'main_loop: loop {
//!     // 1. Process OS window events first (DPI, close requested, resize, etc.)
//!     // Blocks on wait_event (0.0% CPU) when idle!
//!     if let Some(event) = event_pump.wait_event()? {
//!         match event {
//!             MichiuEvent::Window { id, event } => match event {
//!                 Event::CloseRequested => {
//!                     handle.destroy();
//!                 }
//!                 Event::Destroyed => {
//!                     // Exit the loop since the window is already gone
//!                     break 'main_loop;
//!                 }
//!                 _ => {}
//!             }
//!             _ => {}
//!         }
//!     }
//!
//!     // 2. Process custom application commands sequentially (Safe UI mutations)
//!     while let Ok(command) = rx.try_recv() {
//!         match command {
//!             AppCommand::ResizeWindow(size) => {
//!                 // Safely execute on the UI thread
//!                 window.set_size(size);
//!             }
//!             AppCommand::UpdateTitle(title) => {
//!                 window.set_title(title);
//!             }
//!         }
//!     }
//! }
//! # Ok(())
//! # }
//! ```
//!
//! ---
//!
//! ## Comparison Matrix
//!
//! | Feature / Metric | Option A: `wait_event` (Event-Driven) | Option B: `poll_event` (Real-Time) |
//! | :--- | :--- | :--- |
//! | **Primary Method** | [`EventPump::wait_event`] | [`EventPump::poll_event`] |
//! | **Idle CPU Usage** | **0.0% (OS-level Sleep)** | High (requires manual thread sleep) |
//! | **Primary Use Case** | Desktop apps, system trays, utilities | Games, CAD, high-performance rendering |
//! | **Latency** | Immediate (unblocks on interrupt) | Under 16ms (tied to throttle sleep) |
//! | **Wake-up Support** | Fully integrated via [`WindowHandle::wake_up`] | N/A (loop runs constantly anyway) |
//!

mod builder;
mod com;
mod error;
mod events;
mod handle;
mod icon;
mod ime;
mod message;
mod tray;
mod types;
mod window;

pub use builder::*;
pub use com::*;
pub use error::*;
pub use events::*;
pub use handle::*;
pub use icon::*;
pub use ime::*;
pub use message::*;
pub use tray::*;
pub use types::*;
pub use window::*;
