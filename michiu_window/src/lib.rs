//! # michiu_window
//!
//! `michiu_window` is a lightweight, robust, and modern Win32 window management library
//! designed with strict thread-safety and modern Rust paradigms.
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
//! use michiu_window::{WindowBuilder, PhysicalSize, Icon};
//! use windows::Win32::UI::WindowsAndMessaging::InvalidateRect;
//!
//! # fn main() -> Result<(), Box<dyn std::error::Error>> {
//! let window = WindowBuilder::new()
//!     .with_title("Michiu Window")
//!     .into_unvalidated()
//!     .try_into()?;
//!
//! let window_handle = window.handle();
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
//!             InvalidateRect(hwnd, None, true);
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
//! Ideal for production-grade applications, GUI frameworks, or games requiring unidirectional
//! data flow and strict state synchronization.
//!
//! Instead of scattered threads mutating the window directly, background threads emit
//! high-level application events (commands) to a centralized message queue, and the UI thread's
//! event loop processes and applies them sequentially.
//!
//! There are two ways to achieve this:
//!
//! ### Option A: Native `EventSender` (Via `WindowEvent::UserEvent`)
//! You can get a thread-safe [`EventSender`] from `WindowHandle::sender()`. It uses `PostMessageW`
//! internally to send a `Box<dyn Any + Send>` to the UI thread, waking up the event loop safely.
//!
//! ### Option B: Standard Rust `std::sync::mpsc` Channels
//! You can use standard Rust channels alongside the non-blocking `poll_event` loop.
//!
//! ```no_run
//! use std::sync::mpsc;
//! use michiu_window::{WindowBuilder, EventPump, WindowEvent, PhysicalSize};
//!
//! // Define your centralized application commands
//! enum AppCommand {
//!     ResizeWindow(PhysicalSize),
//!     UpdateTitle(String),
//! }
//!
//! # fn main() -> Result<(), Box<dyn std::error::Error>> {
//! let window = WindowBuilder::new().into_unvalidated().try_into()?;
//! let mut event_pump = EventPump::new();
//!
//! // Create a standard MPSC channel
//! let (tx, rx) = mpsc::channel();
//!
//! // Spawn background thread
//! std::thread::spawn(move || {
//!     // Do heavy work...
//!     std::thread::sleep(std::Duration::from_secs(2));
//!
//!     // Send command through the channel
//!     let _ = tx.send(AppCommand::ResizeWindow(PhysicalSize { width: 1280, height: 720 }));
//! });
//!
//! loop {
//!     // 1. Process OS window events first (DPI, close requested, resize, etc.)
//!     while let Some(event) = event_pump.poll_event() {
//!         match event {
//!             WindowEvent::CloseRequested => {
//!                 window.destroy(); // Explicitly drop to destroy the window
//!             }
//!             WindowEvent::Destroyed => {
//!                 break;
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
//!
//!     // Update state and render frames here...
//!     std::thread::sleep(std::Duration::from_millis(16)); // ~60 FPS
//! }
//! # Ok(())
//! # }
//! ```
//!
//! ---
//!
//! ## Comparison Matrix
//!
//! | Feature / Metric | Approach 1: Direct Manipulation | Approach 2: Centralized Architecture |
//! | :--- | :--- | :--- |
//! | **Primary Object** | [`WindowHandle`] + direct methods | [`EventSender`] or `mpsc::Sender` |
//! | **Data Flow** | Scattered / Bidirectional | Unidirectional (Concentrated) |
//! | **Best Used For** | Small scripts, helper tools, quick title updates | Large GUI apps, strict state machines |
//! | **Boilerplate** | Extremely low (one-line method calls) | Moderate (requires defining command Enums) |
//! | **Safety Risk** | Low, but requires `unsafe` for custom raw closures | Absolute Zero (100% safe, unified state) |
//! | **Execution** | Asynchronous (queued via OS message loop) | Synchronous / Sequenced inside the main loop |
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
