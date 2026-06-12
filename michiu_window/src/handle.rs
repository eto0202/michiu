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

/// A lightweight, cloneable reference to a Win32 window that can be safely passed across thread boundaries.
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
}

// 競合しないプライベートなメッセージID
pub const WM_WINDOW_COMMAND: u32 = WM_USER + 102;
pub const WM_RUN_ON_UI_THREAD: u32 = WM_USER + 103;

// TODO: PeekMessage の使用メソッド
impl WindowHandle {
    /// Checks whether the current thread is the UI thread that originally created this window.
    pub fn is_on_ui_thread(&self) -> bool {
        let current_tid = unsafe { GetCurrentThreadId() };
        self.thread_id == current_tid
    }

    /// Ensures that the current thread is the UI thread (for error checking and early return).
    ///
    /// If the thread is not the UI thread, returns a ThreadMismatch error.
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

    /// Retrieves the raw HWND of the referenced window.
    pub fn hwnd(&self) -> HWND {
        self.hwnd
    }

    /// Retrieves the HINSTANCE of the referenced window.
    pub fn hinstance(&self) -> HINSTANCE {
        self.hinstance
    }

    /// Retrieves the UI thread ID associated with this window handle.
    pub fn thread_id(&self) -> u32 {
        self.thread_id
    }

    /// Retrieves the window's unique WindowId
    pub fn id(&self) -> WindowId {
        WindowId(self.hwnd().0 as isize)
    }

    /// Creates a thread-safe EventSender that targets its own HWND.
    pub fn sender(&self) -> EventSender {
        EventSender::new(self.hwnd)
    }

    /// Posts a harmless empty message (WM_NULL) to the window to wake up the message loop asynchronously.
    ///
    /// This is useful when using standard Rust channels (`std::sync::mpsc`) or Tokio channels
    /// to signal the UI thread that new data has arrived in the channel.
    pub fn wake_up(&self) {
        unsafe {
            // WM_NULL (0) を投げることで、GetMessageW などのスリープ待機を解除させ、
            // poll_event のループを1周させる
            let _ = PostMessageW(Some(self.hwnd), WM_NULL, WPARAM(0), LPARAM(0));
        }
    }

    /// Changes the window title (thread-safe).
    ///
    /// If called from the UI thread, the change takes effect immediately;
    /// if called from a background thread, it is automatically and safely passed to the UI thread asynchronously.
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

    /// Sets whether the window should be visible or hidden.
    ///
    /// This method is safe to call from any thread. If called from a background thread,
    /// the request is automatically and asynchronously routed to the UI thread.
    ///
    /// # Examples
    ///
    /// ```no_run
    /// // Hide the window
    /// window_handle.set_visible(false);
    ///
    /// // Show the window
    /// window_handle.set_visible(true);
    /// ```
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

    /// Resizes the client area of the window using the provided physical size.
    ///
    /// This method is safe to call from any thread. If called from a background thread,
    /// the request is automatically and asynchronously routed to the UI thread.
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use michiu_window::PhysicalSize;
    ///
    /// // Resize the window to 1024x768 physical pixels
    /// window_handle.set_size(PhysicalSize { width: 1024, height: 768 });
    /// ```
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

    /// Moves the window to the specified physical screen position.
    ///
    /// This method is safe to call from any thread. If called from a background thread,
    /// the request is automatically and asynchronously routed to the UI thread.
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use michiu_window::PhysicalPoint;
    ///
    /// // Move the window to the coordinate (100, 100) on the screen
    /// window_handle.set_position(PhysicalPoint { x: 100, y: 100 });
    /// ```
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

    /// Dynamically sets the menu theme for the entire application
    /// and the appearance of this window's title bar (thread-safe).
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

    /// Copies the specified text to the system clipboard (thread-safe).
    pub fn set_clipboard_text(&self, text: impl Into<Cow<'static, str>>) {
        let text_str = text.into();
        if self.is_on_ui_thread() {
            let _ = crate::set_clipboard_text_impl(self.hwnd, &text_str);
        } else {
            self.post_command(SetWindowCommand::SetClipboardText(text_str.into()));
        }
    }

    /// Retrieves the current text content from the system clipboard (thread-safe, synchronous on calling thread).
    pub fn get_clipboard_text(&self) -> Result<String> {
        // クリップボード読み出し自体はOSレベルでどのスレッドから呼んでも安全なので、
        // メッセージループを介さずその場で同期実行して返す。
        crate::get_clipboard_text_impl(self.hwnd)
    }

    /// Captures the mouse cursor so that the window continues to receive mouse events (thread-safe).
    pub fn set_cursor_capture(&self, capture: bool) {
        if self.is_on_ui_thread() {
            crate::set_cursor_capture_impl(self.hwnd, capture);
        } else {
            self.post_command(SetWindowCommand::CursorCapture(capture));
        }
    }

    /// Restricts the mouse cursor within the client area of this window (thread-safe).
    pub fn set_cursor_clipping(&self, clip: bool) {
        if self.is_on_ui_thread() {
            crate::set_cursor_clipping_impl(self.hwnd, clip);
        } else {
            self.post_command(SetWindowCommand::CursorClipping(clip));
        }
    }

    /// Moves the window to the exact center of the monitor screen it is currently on (thread-safe).
    pub fn center_on_screen(&self) {
        if self.is_on_ui_thread() {
            crate::center_on_screen_impl(self.hwnd);
        } else {
            self.post_command(SetWindowCommand::CenterOnScreen);
        }
    }

    /// Toggles the borderless fullscreen mode on or off (thread-safe).
    pub fn set_fullscreen(&self, fullscreen: bool) {
        if self.is_on_ui_thread() {
            crate::set_fullscreen_impl(self.hwnd, fullscreen);
        } else {
            self.post_command(SetWindowCommand::Fullscreen(fullscreen));
        }
    }

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

    /// Changes the mouse cursor icon shape for the window (thread-safe).
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

    /// Executes an arbitrary closure on the UI thread.
    ///
    /// # Safety
    ///
    /// This function is unsafe because it executes code asynchronously on the UI thread.
    /// The caller must ensure that:
    ///
    /// 1. The window represented by this handle is still alive when the closure is executed.
    /// 2. The operations performed inside the closure do not violate Win32 thread-safety
    ///    rules or cause undefined behavior (such as dereferencing invalid window pointers).
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
}

impl Validate for WindowHandle {
    type Error = MichiuError;

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
