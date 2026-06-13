use michiu_guard::Validate;
use raw_window_handle::{
    DisplayHandle as RwhDisplayHandle, HandleError, HasDisplayHandle, HasWindowHandle,
    Win32WindowHandle, WindowHandle as RwhWindowHandle,
};
use windows::Win32::UI::Input::KeyboardAndMouse::ReleaseCapture;
use windows::Win32::UI::WindowsAndMessaging::{
    GWLP_USERDATA, GetWindowLongPtrW, HTCAPTION, SendMessageW, WM_NCLBUTTONDOWN, WM_NULL,
};
use windows::core::PCWSTR;

use crate::error::{MichiuError, Result};
use crate::{
    CursorIcon, EventSender, PhysicalPoint, PhysicalSize, PreferredAppMode, WindowId, WindowState,
    enable_dark_mode_titlebar, is_system_dark_mode, set_app_theme,
};
use std::{borrow::Cow, num::NonZeroIsize};
use windows::Win32::{
    Foundation::{HINSTANCE, HWND, LPARAM, WPARAM},
    System::Threading::{GetCurrentProcessId, GetCurrentThreadId},
    UI::WindowsAndMessaging::{
        GetWindowThreadProcessId, IsWindow, PostMessageW, SW_HIDE, SW_SHOW, SWP_NOACTIVATE,
        SWP_NOMOVE, SWP_NOSIZE, SWP_NOZORDER, SetWindowPos, SetWindowTextW, ShowWindow, WM_USER,
    },
};

/// A lightweight, cloneable, and thread-safe reference to an active Win32 window.
///
/// Unlike [`crate::window::Window`], `WindowHandle` implements **`Send` and `Sync`**, allowing it to be
/// safely transferred to background worker threads.
///
/// Methods on this type are designed to be thread-aware:
/// - If called from the UI thread, they execute the raw Win32 APIs immediately and synchronously.
/// - If called from a background thread, they automatically wrap the command and asynchronously
///   route it to the UI thread's message loop using `PostMessageW`.
///
/// This structure implements [`raw_window_handle::HasWindowHandle`] and
/// [`raw_window_handle::HasDisplayHandle`] (v0.6).
///
/// It also implements [`Validate`] from `michiu_guard` for checking the liveness of the underlying window.
#[derive(Debug, Clone)]
pub struct WindowHandle {
    pub(crate) hwnd: HWND,
    pub(crate) hinstance: HINSTANCE,
    pub(crate) thread_id: u32,
}

unsafe impl Send for WindowHandle {}
unsafe impl Sync for WindowHandle {}

pub(crate) enum SetWindowCommand {
    Title(Cow<'static, str>),
    Visible(bool),
    Size(PhysicalSize),
    Position(PhysicalPoint),
    SetClipboardText(String),
    CursorCapture(bool),
    CursorClipping(bool),
    Fullscreen(bool),
    CenterOnScreen,
    StartDragging,
    SetCursor(CursorIcon),
    Destroy,
}

/// Message ID used internally for posting async Window commands to the UI thread.
pub const WM_WINDOW_COMMAND: u32 = WM_USER + 102;

/// Message ID used internally for dispatching arbitrary closures to the UI thread.
pub const WM_RUN_ON_UI_THREAD: u32 = WM_USER + 103;

impl WindowHandle {
    /// Checks whether the current thread is the UI thread that originally created this window.
    pub fn is_on_ui_thread(&self) -> bool {
        let current_tid = unsafe { GetCurrentThreadId() };
        self.thread_id == current_tid
    }

    /// Assures that the current thread is the UI thread.
    ///
    /// # Errors
    /// Returns [`MichiuError::ThreadMismatch`] if called from a foreign thread.
    pub fn assert_ui_thread(&self) -> Result<()> {
        if self.is_on_ui_thread() {
            Ok(())
        } else {
            let current_tid = unsafe { GetCurrentThreadId() };
            Err(MichiuError::ThreadMismatch {
                expected: self.thread_id,
                actual: current_tid,
            })
        }
    }

    /// Returns the raw Win32 `HWND` associated with the referenced window.
    pub fn hwnd(&self) -> HWND {
        self.hwnd
    }

    /// Returns the raw Win32 `HINSTANCE` associated with the referenced window.
    pub fn hinstance(&self) -> HINSTANCE {
        self.hinstance
    }

    /// Returns the thread ID of the UI thread that created the referenced window.
    pub fn thread_id(&self) -> u32 {
        self.thread_id
    }

    /// Returns the unique `WindowId` of the referenced window.
    pub fn id(&self) -> WindowId {
        WindowId(self.hwnd().0 as isize)
    }

    /// Creates a thread-safe [`EventSender`] that targets this window's raw handle.
    pub fn sender(&self) -> EventSender {
        EventSender::new(self.hwnd)
    }

    /// Posts a harmless empty message (`WM_NULL`) to the window to wake up the message loop asynchronously.
    ///
    /// This is highly recommended when using standard Rust channels ([`std::sync::mpsc`])
    /// or async channels (e.g., Tokio) in background threads to signal the UI thread that new data
    /// has arrived without triggering busy polling.
    ///
    /// # Examples
    ///
    /// ```no_run
    /// # use michiu_window::{Window, WindowBuilder, WindowHandle};
    /// # use std::sync::mpsc;
    /// # fn main() -> Result<(), Box<dyn std::error::Error>> {
    /// # let window = Window::build(WindowBuilder::new().into_unvalidated().try_into()?)?;
    /// let (tx, rx) = mpsc::channel();
    /// let handle = window.handle().assume_valid();
    ///
    /// std::thread::spawn(move || {
    ///     tx.send("Task Finished").unwrap();
    ///     // Wake up the main message loop so it can process the channel immediately
    ///     handle.wake_up();
    /// });
    /// # Ok(())
    /// # }
    /// ```
    pub fn wake_up(&self) {
        unsafe {
            // WM_NULL (0) を投げることで、GetMessageW などのスリープ待機を解除させ、
            // poll_event のループを1周させる
            let _ = PostMessageW(Some(self.hwnd), WM_NULL, WPARAM(0), LPARAM(0));
        }
    }

    /// Thread-safely updates the window title.
    ///
    /// If called from a background thread, the request is automatically routed to the UI thread.
    pub fn set_title(&self, title: impl Into<Cow<'static, str>>) {
        let title_str = title.into();

        if self.is_on_ui_thread() {
            unsafe {
                let title_wide: Vec<u16> =
                    title_str.encode_utf16().chain(std::iter::once(0)).collect();
                let _ = SetWindowTextW(self.hwnd, PCWSTR(title_wide.as_ptr()));
            }
        } else {
            self.post_command(SetWindowCommand::Title(title_str));
        }
    }

    /// Thread-safely updates the window visibility.
    pub fn set_visible(&self, visible: bool) {
        if self.is_on_ui_thread() {
            unsafe {
                let show_cmd = if visible { SW_SHOW } else { SW_HIDE };
                let _ = ShowWindow(self.hwnd, show_cmd);
            }
        } else {
            self.post_command(SetWindowCommand::Visible(visible));
        }
    }

    /// Thread-safely updates the physical client size of the window.
    pub fn set_size(&self, size: PhysicalSize) {
        if self.is_on_ui_thread() {
            unsafe {
                let _ = SetWindowPos(
                    self.hwnd,
                    Some(HWND::default()),
                    0,
                    0,
                    size.width,
                    size.height,
                    SWP_NOMOVE | SWP_NOZORDER | SWP_NOACTIVATE,
                );
            }
        } else {
            self.post_command(SetWindowCommand::Size(size));
        }
    }

    /// Thread-safely updates the physical coordinate position of the window.
    pub fn set_position(&self, position: PhysicalPoint) {
        if self.is_on_ui_thread() {
            unsafe {
                let _ = SetWindowPos(
                    self.hwnd,
                    Some(HWND::default()),
                    position.x,
                    position.y,
                    0,
                    0,
                    SWP_NOSIZE | SWP_NOZORDER | SWP_NOACTIVATE,
                );
            }
        } else {
            self.post_command(SetWindowCommand::Position(position));
        }
    }

    /// Thread-safely applies Windows 11 dark/light theme contexts asynchronously to the UI thread.
    pub fn set_theme(&self, mode: PreferredAppMode) {
        unsafe {
            // run_on_ui_thread に乗せて、安全にUIスレッドで代行実行する
            self.run_on_ui_thread(move |hwnd| {
                // テーマ設定に応じて、タイトルバーを黒にするかどうかを動的に判定する
                let enable_dark_titlebar = match mode {
                    PreferredAppMode::ForceDark => true,   // 強制ダークなので常に黒
                    PreferredAppMode::ForceLight => false, // 強制ライトなので常に白
                    PreferredAppMode::AllowDark | PreferredAppMode::Default => {
                        // システム準拠なのでOS自体のダークモード設定を読み取って自動判定
                        is_system_dark_mode()
                    }
                    _ => false,
                };

                enable_dark_mode_titlebar(hwnd, enable_dark_titlebar);
                // プロセス全体のコンテキストメニューにテーマを適用
                set_app_theme(mode);
            });
        }
    }

    /// Thread-safely copies text to the system clipboard.
    pub fn set_clipboard_text(&self, text: impl Into<Cow<'static, str>>) {
        let text_str = text.into();
        if self.is_on_ui_thread() {
            let _ = crate::set_clipboard_text_impl(self.hwnd, &text_str);
        } else {
            self.post_command(SetWindowCommand::SetClipboardText(text_str.into()));
        }
    }

    /// Retrieves text from the system clipboard synchronously.
    ///
    /// Since reading the clipboard is thread-safe on the OS level, this method executes
    /// immediately on the calling thread without message loop dispatch.
    ///
    /// # Errors
    /// Returns an error if the clipboard cannot be opened or if the data format is invalid.
    pub fn get_clipboard_text(&self) -> Result<String> {
        // クリップボード読み出し自体はOSレベルでどのスレッドから呼んでも安全なので、
        // メッセージループを介さずその場で同期実行して返す。
        crate::get_clipboard_text_impl(self.hwnd)
    }

    /// Thread-safely captures the mouse cursor.
    pub fn set_cursor_capture(&self, capture: bool) {
        if self.is_on_ui_thread() {
            crate::set_cursor_capture_impl(self.hwnd, capture);
        } else {
            self.post_command(SetWindowCommand::CursorCapture(capture));
        }
    }

    /// Thread-safely clips the mouse cursor to the client area.
    pub fn set_cursor_clipping(&self, clip: bool) {
        if self.is_on_ui_thread() {
            crate::set_cursor_clipping_impl(self.hwnd, clip);
        } else {
            self.post_command(SetWindowCommand::CursorClipping(clip));
        }
    }

    /// Thread-safely moves the window to the physical center of the monitor.
    pub fn center_on_screen(&self) {
        if self.is_on_ui_thread() {
            crate::center_on_screen_impl(self.hwnd);
        } else {
            self.post_command(SetWindowCommand::CenterOnScreen);
        }
    }

    /// Thread-safely toggles borderless fullscreen mode.
    pub fn set_fullscreen(&self, fullscreen: bool) {
        if self.is_on_ui_thread() {
            crate::set_fullscreen_impl(self.hwnd, fullscreen);
        } else {
            self.post_command(SetWindowCommand::Fullscreen(fullscreen));
        }
    }

    /// Thread-safely triggers a custom titlebar dragging operation.
    pub fn set_start_dragging(&self) {
        if self.is_on_ui_thread() {
            unsafe {
                // もしマウスキャプチャ中なら解除する
                let _ = ReleaseCapture();
                // OSに対して今まさにタイトルバーが左クリックされたという偽装シグナルを送る
                SendMessageW(
                    self.hwnd,
                    WM_NCLBUTTONDOWN,
                    Some(WPARAM(HTCAPTION as usize)),
                    Some(LPARAM(0)),
                );
            }
        } else {
            self.post_command(SetWindowCommand::StartDragging);
        }
    }

    /// Thread-safely updates the mouse cursor icon shape.
    pub fn set_cursor_icon(&self, cursor: CursorIcon) {
        if self.is_on_ui_thread() {
            unsafe {
                let state_ptr = GetWindowLongPtrW(self.hwnd, GWLP_USERDATA) as *mut WindowState;
                if !state_ptr.is_null() {
                    (*state_ptr).current_cursor = cursor;
                    let _ = crate::apply_cursor_icon_impl(self.hwnd, cursor);
                }
            }
        } else {
            self.post_command(SetWindowCommand::SetCursor(cursor));
        }
    }

    /// Dispatches an arbitrary closure to be executed asynchronously on the UI thread.
    ///
    /// This acts as the ultimate escape hatch for custom Win32 operations that are not natively
    /// wrapped by the library, allowing you to manipulate the window without thread-affinity crashes.
    ///
    /// # Safety
    /// The caller must ensure that:
    /// 1. The underlying window remains alive during the closure's deferred execution.
    /// 2. The closure's internal operations do not violate Win32 thread-affinity or cause undefined behavior.
    ///
    /// # Examples
    ///
    /// ```no_run
    /// # use michiu_window::{Window, WindowBuilder, WindowHandle};
    /// # use windows::Win32::Graphics::Gdi::InvalidateRect;
    /// # fn main() -> Result<(), Box<dyn std::error::Error>> {
    /// # let window = Window::build(WindowBuilder::new().into_unvalidated().try_into()?)?;
    /// let handle = window.handle().assume_valid();
    ///
    /// std::thread::spawn(move || {
    ///     // Execute custom raw Win32 calls on the UI thread
    ///     unsafe {
    ///         handle.run_on_ui_thread(|hwnd| {
    ///             InvalidateRect(Some(hwnd), None, true);
    ///         });
    ///     }
    /// });
    /// # Ok(())
    /// # }
    /// ```
    pub unsafe fn run_on_ui_thread<F>(&self, f: F)
    where
        F: FnOnce(HWND) + Send + 'static,
    {
        if self.is_on_ui_thread() {
            // 自スレッド（UIスレッド）ならその場で即座に実行する
            f(self.hwnd);
        } else {
            // 他スレッドなら、クロージャをダブルBox化して生ポインタに変換し、PostMessageW で投げる
            // FnOnce をトレイトオブジェクトにするため、一度 Box で包んでから、
            // さらにポインタ化のための外枠の Box で包む
            let closure: Box<dyn FnOnce(HWND) + Send + 'static> = Box::new(f);
            let raw_ptr = Box::into_raw(Box::new(closure));

            unsafe {
                let res = PostMessageW(
                    Some(self.hwnd),
                    WM_RUN_ON_UI_THREAD,
                    WPARAM(0),
                    LPARAM(raw_ptr as isize),
                );

                // 送信失敗（ウィンドウがすでに破棄されている等）した場合は、
                // メモリリークを防ぐために即座に回収して解放する
                if res.is_err() {
                    let _ = Box::from_raw(raw_ptr);
                }
            }
        }
    }

    fn post_command(&self, cmd: SetWindowCommand) {
        let raw_ptr = Box::into_raw(Box::new(cmd));
        unsafe {
            let res = PostMessageW(
                Some(self.hwnd),
                WM_WINDOW_COMMAND,
                WPARAM(0),
                LPARAM(raw_ptr as isize),
            );
            if res.is_err() {
                let _ = Box::from_raw(raw_ptr);
            }
        }
    }

    /// Thread-safely destroys the window and releases all associated OS resources (thread-safe).
    ///
    /// If called from the UI thread, this executes `DestroyWindow` immediately;
    /// if called from a background thread, the request is automatically and safely routed
    /// to the UI thread asynchronously.
    ///
    /// Once the window is destroyed, all cloned `WindowHandle` instances referencing this window
    /// will fail validation (returning [`MichiuError::InvalidHandleState`]), preventing zombie access.
    ///
    /// # Examples
    ///
    /// ```no_run
    /// # use michiu_window::{Window, WindowBuilder, WindowHandle};
    /// # fn main() -> Result<(), Box<dyn std::error::Error>> {
    /// # let window = Window::build(WindowBuilder::new().into_unvalidated().try_into()?)?;
    /// let handle = window.handle().assume_valid();
    ///
    /// // Destroy the window thread-safely from a background thread
    /// std::thread::spawn(move || {
    ///     handle.destroy();
    /// });
    /// # Ok(())
    /// # }
    /// ```
    pub fn destroy(&self) {
        if self.is_on_ui_thread() {
            unsafe {
                use windows::Win32::UI::WindowsAndMessaging::DestroyWindow;
                let _ = DestroyWindow(self.hwnd);
            }
        } else {
            self.post_command(SetWindowCommand::Destroy);
        }
    }
}

impl Validate for WindowHandle {
    type Error = MichiuError;

    /// Validates whether the window is still alive and belongs to the current process.
    ///
    /// Checks:
    /// 1. If the underlying `HWND` is still recognized as a valid window by the OS (`IsWindow`).
    /// 2. If the window's owner Process ID (PID) matches the current process's PID (detects recycled HWNDs).
    ///
    /// # Errors
    /// Returns [`MichiuError::InvalidHandleState`] if the window was already destroyed,
    /// or [`MichiuError::ValidationError`] if the handle was recycled by another process.
    ///
    /// # Examples
    ///
    /// ```no_run
    /// # use michiu_window::{Window, WindowBuilder, WindowHandle};
    /// # use michiu_guard::{Unvalidated, Validated};
    /// # fn main() -> Result<(), Box<dyn std::error::Error>> {
    /// # let window = Window::build(WindowBuilder::new().into_unvalidated().try_into()?)?;
    /// let unvalidated_handle: Unvalidated<WindowHandle> = window.handle();
    ///
    /// // Safe validation check before usage
    /// let validated_handle: Validated<WindowHandle> = unvalidated_handle.try_into()?;
    /// # Ok(())
    /// # }
    /// ```
    fn validate(self) -> Result<Self> {
        let is_alive = unsafe { IsWindow(Some(self.hwnd)).as_bool() };
        if !is_alive {
            return Err(MichiuError::InvalidHandleState {
                reason: "The window has already been destroyed or closed on the OS side.",
            });
        }

        let mut process_id = 0u32;
        let _ = unsafe { GetWindowThreadProcessId(self.hwnd, Some(&mut process_id)) };

        let current_pid = unsafe { GetCurrentProcessId() };
        if process_id != current_pid {
            return Err(MichiuError::ValidationError {
                parameter: "WindowHandle",
                message: "Window handle has been recycled by another process.".into(),
            });
        }

        Ok(self)
    }
}

impl HasWindowHandle for WindowHandle {
    fn window_handle(&self) -> std::result::Result<RwhWindowHandle<'_>, HandleError> {
        let hwnd_val = self.hwnd.0 as isize;
        let non_zero_hwnd = NonZeroIsize::new(hwnd_val).ok_or(HandleError::Unavailable)?;

        let mut win32_handle = Win32WindowHandle::new(non_zero_hwnd);

        let hinstance_val = self.hinstance.0 as isize;
        if let Some(non_zero_hinstance) = NonZeroIsize::new(hinstance_val) {
            win32_handle.hinstance = Some(non_zero_hinstance);
        }

        unsafe { Ok(RwhWindowHandle::borrow_raw(win32_handle.into())) }
    }
}

impl HasDisplayHandle for WindowHandle {
    fn display_handle(&self) -> std::result::Result<RwhDisplayHandle<'_>, HandleError> {
        Ok(RwhDisplayHandle::windows())
    }
}

#[cfg(test)]
mod tests;
