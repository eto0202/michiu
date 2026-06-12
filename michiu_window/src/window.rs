use crate::error::{MichiuError, Result};
use crate::{
    CursorIcon, FileDropTarget, ImeRelayServer, LogicalSize, PhysicalPoint, PhysicalSize,
    PreferredAppMode, Tray, WindowBuilder, WindowHandle, translate_and_push,
};
use michiu_guard::{Unvalidated, Validated};
use raw_window_handle::{
    DisplayHandle as RwhDisplayHandle, HandleError, HasDisplayHandle, HasWindowHandle,
    Win32WindowHandle, WindowHandle as RwhWindowHandle,
};
use std::{borrow::Cow, ffi::c_void, marker::PhantomData, num::NonZeroIsize, sync::Arc};
use windows::Win32::Foundation::{FreeLibrary, GlobalFree};
use windows::Win32::System::Memory::GlobalSize;
use windows::Win32::UI::Shell::{DefSubclassProc, RemoveWindowSubclass};
use windows::Win32::UI::WindowsAndMessaging::{
    HTCAPTION, IDC_CROSS, IDC_HAND, IDC_IBEAM, IDC_WAIT, SetCursor, WM_NCLBUTTONDOWN,
};
use windows::{
    Win32::{
        Foundation::{
            ERROR_CLASS_ALREADY_EXISTS, HANDLE, HGLOBAL, HINSTANCE, HWND, LPARAM, LRESULT, RECT,
            WPARAM,
        },
        Graphics::{
            Dwm::{DWMWA_USE_IMMERSIVE_DARK_MODE, DwmSetWindowAttribute},
            Gdi::{
                GetMonitorInfoW, HBRUSH, MONITOR_DEFAULTTONEAREST, MONITORINFO, MapWindowPoints,
                MonitorFromWindow,
            },
        },
        System::{
            DataExchange::{
                CloseClipboard, EmptyClipboard, GetClipboardData, OpenClipboard, SetClipboardData,
            },
            LibraryLoader::{GetModuleHandleW, GetProcAddress, LoadLibraryW},
            Memory::{GMEM_MOVEABLE, GlobalLock, GlobalUnlock},
            Ole::{CF_UNICODETEXT, IDropTarget, RegisterDragDrop},
            Registry::{
                HKEY, HKEY_CURRENT_USER, KEY_READ, REG_DWORD, RegCloseKey, RegOpenKeyExW,
                RegQueryValueExW,
            },
        },
        UI::{
            HiDpi::{
                AdjustWindowRectExForDpi, DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2,
                EnableNonClientDpiScaling, GetDpiForSystem, GetDpiForWindow,
                SetProcessDpiAwarenessContext,
            },
            Input::KeyboardAndMouse::{ReleaseCapture, SetCapture},
            Shell::SetWindowSubclass,
            WindowsAndMessaging::{
                CREATESTRUCTW, CS_HREDRAW, CS_VREDRAW, CW_USEDEFAULT, ClipCursor, CreateWindowExW,
                DefWindowProcW, DestroyWindow, GWL_EXSTYLE, GWL_STYLE, GWLP_USERDATA,
                GetClientRect, GetWindowLongPtrW, GetWindowRect, GetWindowThreadProcessId, HICON,
                HWND_TOP, IDC_ARROW, IsWindow, LoadCursorW, MINMAXINFO, RegisterClassExW, SW_HIDE,
                SW_SHOW, SWP_FRAMECHANGED, SWP_NOACTIVATE, SWP_NOMOVE, SWP_NOSIZE, SWP_NOZORDER,
                SendMessageW, SetWindowLongPtrW, SetWindowPos, SetWindowTextW, ShowWindow,
                WINDOW_EX_STYLE, WINDOW_STYLE, WM_GETMINMAXINFO, WM_NCCREATE, WM_NCDESTROY,
                WM_SETICON, WNDCLASSEXW, WS_BORDER, WS_CAPTION, WS_CHILD, WS_DLGFRAME,
                WS_EX_APPWINDOW, WS_EX_CLIENTEDGE, WS_EX_DLGMODALFRAME, WS_EX_LAYERED,
                WS_EX_NOREDIRECTIONBITMAP, WS_EX_STATICEDGE, WS_EX_TRANSPARENT, WS_MAXIMIZE,
                WS_MAXIMIZEBOX, WS_MINIMIZEBOX, WS_OVERLAPPEDWINDOW, WS_POPUP, WS_THICKFRAME,
                WS_VISIBLE,
            },
        },
    },
    core::{PCSTR, PCWSTR},
};

/// Represents an active, on-screen Win32 window created by the framework.
///
/// Due to strict Win32 thread-affinity rules, `Window` is marked as **`!Send` and `!Sync`**.
/// All UI mutations and message loops using this type must remain on the specific UI thread
/// that instantiated the window.
///
/// To reference or manipulate this window from other background threads, obtain an unvalidated,
/// cloneable handle using [`Window::handle`].
///
/// This structure automatically implements [`raw_window_handle::HasWindowHandle`] and
/// [`raw_window_handle::HasDisplayHandle`] (v0.6)
/// for seamless integration with external rendering libraries.
pub struct Window {
    hwnd: HWND,
    hinstance: HINSTANCE,
    thread_id: u32,
    #[allow(dead_code)]
    tray: Option<Tray>,
    #[allow(dead_code)]
    drop_target: Option<IDropTarget>,
    #[allow(dead_code)]
    ime_relay: Option<Arc<ImeRelayServer>>,
    _marker: PhantomData<*const ()>, // !Send and !Sync
}

/// A thread-safe, unique identifier used to distinguish windows from each other.
///
/// Under the hood, this wraps the raw `HWND` pointer representation as an `isize` integer.
/// It implements `Clone`, `Copy`, `PartialEq`, `Eq`, and `Hash`, making it suitable
/// for use as keys in hash maps or routing events in multi-window environments.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct WindowId(pub(crate) isize);

impl WindowId {
    pub fn id(&self) -> isize {
        self.0
    }
}

const DEFAULT_CLASS_NAME: &str = concat!("MichiuWindowClass_", env!("CARGO_PKG_VERSION"));

impl Window {
    /// Instantiates a new Win32 window based on the validated configuration parameters.
    ///
    /// # Errors
    /// Returns [`MichiuError::WindowCreationFailed`] if class registration fails or `CreateWindowExW`
    /// returns a null handle.
    ///
    /// # Examples
    ///
    /// ```no_run
    /// # use michiu_window::{Window, WindowBuilder};
    /// # fn main() -> Result<(), Box<dyn std::error::Error>> {
    /// let builder = WindowBuilder::new().with_title("Main Frame");
    /// let validated = builder.into_unvalidated().try_into()?;
    ///
    /// let window = Window::build(validated)?;
    /// # Ok(())
    /// # }
    /// ```
    pub fn build(builder: Validated<WindowBuilder<'_>>) -> Result<Self> {
        let hmodule = unsafe { GetModuleHandleW(None).map_err(MichiuError::UnexpectedOsError)? };
        let hinstance = HINSTANCE(hmodule.0);

        let builder = builder.into_inner();

        let class_name_utf16 = get_class_name_utf16(DEFAULT_CLASS_NAME, &builder);
        let class_name = PCWSTR(class_name_utf16.as_ptr());

        register_window_class(class_name, DEFAULT_CLASS_NAME, hinstance, &builder)?;

        let style = build_raw_style(&builder);
        let ex_style = build_raw_ex_style(&builder);

        let dpi = unsafe { GetDpiForSystem() };
        let scale_factor = dpi as f64 / 96.0;

        let (x, y) = match builder.position {
            Some(pos) => {
                let pos = pos.to_physical(scale_factor);
                (pos.x, pos.y)
            }
            _ => (CW_USEDEFAULT, CW_USEDEFAULT),
        };

        // WM_DPICHANGED メッセージを受信した場合、ウィンドウサイズは後で調整される
        let (width, height) = calc_window_rect(style, ex_style, &builder, dpi, scale_factor)?;

        let title_wide: Vec<u16> = builder.title.encode_utf16().chain(Some(0)).collect();
        let lp_window_name = PCWSTR(title_wide.as_ptr());

        let mut ime_relay = None;
        if let Some(port) = builder.ime_expose_port
            && let Ok(relay) = ImeRelayServer::start(port)
        {
            ime_relay = Some(Arc::new(relay));
        }

        let window_state = Box::new(WindowState {
            message_filter: builder.message_filter,
            auto_dpi_scaling: builder.auto_dpi_scaling,
            is_cursor_inside: false,
            min_inner_size: builder.min_inner_size,
            max_inner_size: builder.max_inner_size,
            saved_style: None,
            saved_ex_style: None,
            saved_rect: None,
            current_cursor: CursorIcon::Default,
            ime_relay: ime_relay.clone(),
        });
        let state_raw_ptr: *mut WindowState = Box::into_raw(window_state);

        // スタック上に作成コンテキストを用意
        let mut context = CreationContext {
            state: state_raw_ptr,
            was_taken: false,
        };

        let hwnd_parent = builder.parent_hwnd;
        let hwnd_result = unsafe {
            CreateWindowExW(
                ex_style,
                class_name,
                lp_window_name,
                style,
                x,
                y,
                width,
                height,
                hwnd_parent,
                None,
                None,
                // filter_raw_ptr の代わりに、context のポインタを渡す
                Some(&mut context as *mut CreationContext as *const c_void),
            )
        };

        let hwnd = match hwnd_result {
            Ok(h) => h,
            Err(err) => {
                // コンテキストが WndProc に受け取られていなければここで安全に手動解放
                // すでに受け取られていた場合は、WM_NCDESTROY 側で解放されるため何もしない（二重解放を防ぐ）
                if !context.was_taken && !state_raw_ptr.is_null() {
                    unsafe {
                        let _ = Box::from_raw(state_raw_ptr);
                    }
                }
                return Err(MichiuError::WindowCreationFailed {
                    title: builder.title,
                    class_name: builder
                        .custom_class_name
                        .clone()
                        .unwrap_or(DEFAULT_CLASS_NAME.into()),
                    source: err,
                });
            }
        };

        if let Some(i) = builder.icon {
            let h = i.as_raw().0 as isize;
            unsafe {
                // Alt+Tab や タスクバーの大きい表示用のアイコン (通常 32x32 や 48x48)
                // Win32仕様: ICON_BIG = 1
                let _ = SendMessageW(hwnd, WM_SETICON, Some(WPARAM(1)), Some(LPARAM(h)));

                // ウィンドウのタイトルバー左上や、タスクバーの小さい表示用のアイコン (通常 16x16)
                // Win32仕様: ICON_SMALL = 0
                let _ = SendMessageW(hwnd, WM_SETICON, Some(WPARAM(0)), Some(LPARAM(h)));
            }
        }

        let mut drop_target = None;
        if builder.drag_and_drop {
            let target_impl = FileDropTarget::new(hwnd);
            // windows-rs の implement により自動生成される From キャスト経由で COM インターフェースを取得
            let target: IDropTarget = target_impl.into();

            unsafe {
                let _ = RegisterDragDrop(hwnd, &target);
            }
            drop_target = Some(target);
        }

        let mut process_id: u32 = 0;
        let thread_id = unsafe { GetWindowThreadProcessId(hwnd, Some(&mut process_id)) };

        let tray = builder.tray;

        Ok(Self {
            hwnd,
            hinstance,
            thread_id,
            tray,
            drop_target,
            ime_relay,
            _marker: PhantomData,
        })
    }

    /// Obtains a cloneable, thread-safe, unvalidated handle referencing this window.
    ///
    /// The returned [`WindowHandle`] can be safely sent to background worker threads.
    ///
    /// # Examples
    ///
    /// ```no_run
    /// # use michiu_window::{Window, WindowBuilder};
    /// # fn main() -> Result<(), Box<dyn std::error::Error>> {
    /// # let window = Window::build(WindowBuilder::new().into_unvalidated().try_into()?)?;
    /// let unvalidated_handle = window.handle();
    ///
    /// // Safely pass the handle to a background thread
    /// std::thread::spawn(move || {
    ///     let handle = unvalidated_handle.assume_valid();
    ///     handle.set_title("Processing Complete");
    /// });
    /// # Ok(())
    /// # }
    /// ```
    pub fn handle(&self) -> Unvalidated<WindowHandle> {
        Unvalidated::new(WindowHandle {
            hwnd: self.hwnd,
            hinstance: self.hinstance,
            thread_id: self.thread_id,
        })
    }

    /// Retrieves the current physical DPI value for this window.
    pub fn dpi(&self) -> u32 {
        unsafe { GetDpiForWindow(self.hwnd) }
    }

    /// Retrieves the current scaling ratio between physical pixels and logical pixels (e.g., `1.5` for 150% scaling).
    pub fn scale_factor(&self) -> f64 {
        self.dpi() as f64 / 96.0
    }

    /// Safely registers a high-level subclassing callback closure for this window.
    ///
    /// The handler closure processes Win32 messages and returns a [`SubclassResult`].
    /// If [`SubclassResult::Continue`] is returned, the next subclass, main wnd_proc, and custom
    /// filters in the chain are automatically and safely called.
    ///
    /// Upon receiving `WM_NCDESTROY`, the heap-allocated closure is automatically and safely freed,
    /// preventing any memory leaks.
    ///
    /// # Examples
    ///
    /// ```no_run
    /// # use michiu_window::{Window, WindowBuilder, SubclassResult};
    /// # use windows::Win32::UI::WindowsAndMessaging::WM_USER;
    /// # fn main() -> Result<(), Box<dyn std::error::Error>> {
    /// # let window = Window::build(WindowBuilder::new().into_unvalidated().try_into()?)?;
    /// window.subclass(101, |hwnd, msg, wparam, lparam| {
    ///     if msg == WM_USER + 100 {
    ///         println!("Custom hook executed!");
    ///         return SubclassResult::Intercept(windows::Win32::Foundation::LRESULT(1));
    ///     }
    ///     SubclassResult::Continue
    /// })?;
    /// # Ok(())
    /// # }
    /// ```
    pub fn subclass<F>(&self, id_subclass: usize, handler: F) -> Result<()>
    where
        F: FnMut(HWND, u32, WPARAM, LPARAM) -> SubclassResult + 'static,
    {
        let boxed_handler = Box::into_raw(Box::new(handler));

        unsafe extern "system" fn safe_subclass_proc<F2>(
            hwnd: HWND,
            msg: u32,
            wparam: WPARAM,
            lparam: LPARAM,
            id_subclass: usize,
            ref_data: usize,
        ) -> LRESULT
        where
            F2: FnMut(HWND, u32, WPARAM, LPARAM) -> SubclassResult + 'static,
        {
            let handler_ptr = ref_data as *mut F2;
            if !handler_ptr.is_null() {
                let handler = unsafe { &mut *handler_ptr };

                // ユーザーのハンドラを実行
                match handler(hwnd, msg, wparam, lparam) {
                    SubclassResult::Intercept(result) => {
                        // メッセージが WM_NCDESTROY の場合のみ、メモリ解放と後続処理のためにインターセプトを拒否
                        if msg == WM_NCDESTROY {
                            // ハンドラのメモリ解放
                            unsafe {
                                let _ = Box::from_raw(handler_ptr);
                                let _ = RemoveWindowSubclass(
                                    hwnd,
                                    Some(safe_subclass_proc::<F2>),
                                    id_subclass,
                                );
                                return DefSubclassProc(hwnd, msg, wparam, lparam);
                            }
                        }
                        return result;
                    }
                    SubclassResult::Continue => {
                        if msg == WM_NCDESTROY {
                            unsafe {
                                let _ = Box::from_raw(handler_ptr);
                                let _ = RemoveWindowSubclass(
                                    hwnd,
                                    Some(safe_subclass_proc::<F2>),
                                    id_subclass,
                                );
                            }
                        }
                    }
                }
            }
            // インターセプトしない場合はライブラリ側が責任を持って自動でチェーンを呼ぶ
            unsafe { DefSubclassProc(hwnd, msg, wparam, lparam) }
        }

        unsafe {
            let result = SetWindowSubclass(
                self.hwnd,
                Some(safe_subclass_proc::<F>),
                id_subclass,
                boxed_handler as usize,
            );
            if result.as_bool() {
                Ok(())
            } else {
                let _ = Box::from_raw(boxed_handler);
                Err(MichiuError::SubclassSetupFailed {
                    hwnd: self.hwnd,
                    subclass_id: id_subclass,
                    source: windows::core::Error::from_thread(),
                })
            }
        }
    }

    /// Registers an unsafe raw Win32 subclass procedure for this window.
    ///
    /// # Safety
    /// The caller must ensure that:
    /// 1. The subclass procedure (`subclass_proc`) is valid and its context remains active.
    /// 2. Unhandled messages are correctly forwarded to [`windows::Win32::UI::Shell::DefSubclassProc`].
    ///
    /// Failing to forward messages, or dereferencing invalid raw pointers passed in `ref_data`,
    /// can cause memory leaks, visual hangs, or fatal crashes in the OS message loop.
    ///
    /// # Examples
    ///
    /// Here is how to register the custom subclass procedure callback with a window instance:
    ///
    /// ```no_run
    /// # use michiu_window::{Window, WindowBuilder, RawSubclassProc};
    /// # use windows::Win32::Foundation::{HWND, WPARAM, LPARAM, LRESULT};
    /// # use windows::Win32::UI::Shell::DefSubclassProc;
    /// # use windows::Win32::UI::WindowsAndMessaging::WM_USER;
    /// #
    /// # unsafe extern "system" fn my_custom_subclass_proc(
    /// #     hwnd: HWND, msg: u32, wparam: WPARAM, lparam: LPARAM, id_subclass: usize, ref_data: usize
    /// # ) -> LRESULT {
    /// #     DefSubclassProc(hwnd, msg, wparam, lparam)
    /// # }
    /// #
    /// # fn main() -> Result<(), Box<dyn std::error::Error>> {
    /// # let window = Window::build(WindowBuilder::new().into_unvalidated().try_into()?)?;
    /// // Subclass ID to uniquely identify this registration
    /// let subclass_id = 999;
    /// // Arbitrary user data passed down to the procedure (e.g., pointer to a state structure)
    /// let ref_data = 0;
    ///
    /// unsafe {
    ///     window.raw_subclass(subclass_id, ref_data, my_custom_subclass_proc)?;
    /// }
    /// # Ok(())
    /// # }
    /// ```
    pub unsafe fn raw_subclass(
        &self,
        id_subclass: usize,
        ref_data: usize,
        subclass_proc: RawSubclassProc,
    ) -> Result<()> {
        let result =
            unsafe { SetWindowSubclass(self.hwnd, Some(subclass_proc), id_subclass, ref_data) };

        if result.as_bool() {
            Ok(())
        } else {
            Err(MichiuError::SubclassSetupFailed {
                hwnd: self.hwnd,
                subclass_id: id_subclass,
                source: windows::core::Error::from_thread(),
            })
        }
    }

    /// Returns the raw Win32 `HWND` handle associated with the window.
    pub fn hwnd(&self) -> HWND {
        self.hwnd
    }

    /// Returns the raw Win32 `HINSTANCE` module handle associated with the window.
    pub fn hinstance(&self) -> HINSTANCE {
        self.hinstance
    }

    /// Returns the OS Thread ID of the UI thread that created this window.
    pub fn thread_id(&self) -> u32 {
        self.thread_id
    }

    /// Returns the unique `WindowId` of this window.
    pub fn id(&self) -> WindowId {
        WindowId(self.hwnd().0 as isize)
    }

    /// Explicitly consumes and destroys the window on the UI thread, releasing all OS resources.
    ///
    /// Since this method consumes the ownership (`self`), the window variable cannot be used
    /// after calling this method.
    pub fn destroy(self) {}

    /// Updates the window caption (title) text.
    pub fn set_title(&self, title: impl Into<Cow<'static, str>>) {
        let title_str = title.into();

        unsafe {
            let title_wide: Vec<u16> = title_str.encode_utf16().chain(std::iter::once(0)).collect();
            let _ = SetWindowTextW(self.hwnd, PCWSTR(title_wide.as_ptr()));
        }
    }

    /// Shows or hides the window based on the provided boolean value.
    pub fn set_visible(&self, visible: bool) {
        unsafe {
            let show_cmd = if visible { SW_SHOW } else { SW_HIDE };
            let _ = ShowWindow(self.hwnd, show_cmd);
        }
    }

    /// Resizes the physical client area of the window.
    pub fn set_size(&self, size: PhysicalSize) {
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
    }

    /// Relocates the window to the specified physical screen coordinates.
    pub fn set_position(&self, position: PhysicalPoint) {
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
    }

    /// Applies native Windows 11 dark or light modes to the title bar and context menus.
    ///
    /// # Examples
    ///
    /// ```no_run
    /// # use michiu_window::{Window, WindowBuilder, PreferredAppMode};
    /// # fn main() -> Result<(), Box<dyn std::error::Error>> {
    /// # let window = Window::build(WindowBuilder::new().into_unvalidated().try_into()?)?;
    /// // Forces native dark mode theme across the window and context menus
    /// window.set_theme(PreferredAppMode::ForceDark);
    /// # Ok(())
    /// # }
    /// ```
    pub fn set_theme(&self, mode: PreferredAppMode) {
        unsafe {
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

            enable_dark_mode_titlebar(self.hwnd(), enable_dark_titlebar);
            // プロセス全体のコンテキストメニューにテーマを適用
            set_app_theme(mode);
        }
    }

    /// Copies the specified text content to the Win32 system clipboard.
    ///
    /// # Errors
    /// Returns an error if opening or emptying the clipboard fails, or memory allocation fails.
    pub fn set_clipboard_text(&self, text: impl Into<Cow<'static, str>>) -> Result<()> {
        set_clipboard_text_impl(self.hwnd, &text.into())
    }

    /// Retrieves the current text content from the system clipboard.
    ///
    /// # Errors
    /// Returns an error if the clipboard cannot be opened, or the data is not Unicode text.
    pub fn get_clipboard_text(&self) -> Result<String> {
        get_clipboard_text_impl(self.hwnd)
    }

    /// Captures the mouse cursor so that the window continues to receive mouse events
    /// even if the cursor moves outside the window's boundary.
    pub fn set_cursor_capture(&self, capture: bool) {
        set_cursor_capture_impl(self.hwnd, capture);
    }

    /// Restricts the mouse cursor within the physical client area of the window.
    pub fn set_cursor_clipping(&self, clip: bool) {
        set_cursor_clipping_impl(self.hwnd, clip);
    }

    /// Moves the window to the exact physical center of the monitor screen it is currently on.
    pub fn center_on_screen(&self) {
        center_on_screen_impl(self.hwnd);
    }

    /// Toggles borderless fullscreen mode on or off.
    ///
    /// This temporarily removes window borders and expands to cover the current active monitor.
    pub fn set_fullscreen(&self, fullscreen: bool) {
        set_fullscreen_impl(self.hwnd, fullscreen);
    }

    /// Sends a Win32 non-client click signal (HTCAPTION) to the OS, allowing the user
    /// to drag and move the window by clicking on custom regions (e.g., custom client title bars).
    pub fn set_start_dragging(&self) {
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
    }

    /// Changes the standard mouse cursor shape (arrow, hand, text field, wait) for the window.
    pub fn set_cursor_icon(&self, cursor: CursorIcon) {
        unsafe {
            let state_ptr = GetWindowLongPtrW(self.hwnd, GWLP_USERDATA) as *mut WindowState;
            if !state_ptr.is_null() {
                (*state_ptr).current_cursor = cursor;
                let _ = apply_cursor_icon_impl(self.hwnd, cursor);
            }
        }
    }
}

impl Drop for Window {
    fn drop(&mut self) {
        unsafe {
            let is_alive = IsWindow(Some(self.hwnd)).as_bool();
            if is_alive {
                let _ = DestroyWindow(self.hwnd);
            }
        }
    }
}

impl HasWindowHandle for Window {
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

impl HasDisplayHandle for Window {
    fn display_handle(&self) -> std::result::Result<RwhDisplayHandle<'_>, HandleError> {
        Ok(RwhDisplayHandle::windows())
    }
}

/// Initializes the entire application process for High-DPI support (Per-Monitor DPI v2).
///
/// This must be called at the very beginning of the `main` function before creating any window.
/// If skipped, Windows OS will render the entire application window blurry and stretched.
///
/// # Returns
/// Returns `true` if High-DPI initialization succeeded, or `false` (e.g., already set).
///
/// # Examples
///
/// ```no_run
/// use michiu_window::init_dpi_awareness;
///
/// fn main() {
///     if init_dpi_awareness() {
///         println!("High-DPI Per-Monitor v2 initialized successfully.");
///     }
/// }
/// ```
pub fn init_dpi_awareness() -> bool {
    unsafe {
        // Windows 10 Creators Update以降の推奨されるDPIモード
        SetProcessDpiAwarenessContext(DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2).is_ok()
    }
}

/// Describes the action to take after custom subclass message processing.
///
/// Used alongside [`Window::subclass`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SubclassResult {
    /// Completely intercepts the message, blocking further propagation to downstream procedures (including MessageFilter).
    Intercept(LRESULT),
    /// Continues forwarding the message to the next procedure in the subclass/WndProc chain.
    Continue,
}

/// The raw system function pointer signature required for Win32 subclass procedures.
///
/// Under the hood, this targets Win32 subclass callbacks. Any message not handled by this
/// procedure must be forwarded down the subclass chain by calling [`windows::Win32::UI::Shell::DefSubclassProc`].
///
/// # Examples
///
/// Here is how to create a compliant subclass procedure callback function:
///
/// ```no_run
/// use windows::Win32::Foundation::{HWND, WPARAM, LPARAM, LRESULT};
/// use windows::Win32::UI::WindowsAndMessaging::WM_USER;
/// use windows::Win32::UI::Shell::DefSubclassProc;
///
/// // Note: Subclass procedures must be marked `unsafe extern "system"`
/// unsafe extern "system" fn my_custom_subclass_proc(
///     hwnd: HWND,
///     msg: u32,
///     wparam: WPARAM,
///     lparam: LPARAM,
///     id_subclass: usize,
///     ref_data: usize,
/// ) -> LRESULT {
///     // Handle a specific custom message and block further propagation
///     if msg == WM_USER + 1 {
///         println!("Raw custom message intercepted in subclass proc!");
///         return LRESULT(100);
///     }
///
///     // Crucial: You MUST forward unhandled messages to the next procedure in the chain
///     DefSubclassProc(hwnd, msg, wparam, lparam)
/// }
/// ```
pub type RawSubclassProc = unsafe extern "system" fn(
    hwnd: HWND,
    msg: u32,
    wparam: WPARAM,
    lparam: LPARAM,
    id_subclass: usize,
    ref_data: usize,
) -> LRESULT;

/// A safe wrapper around a custom closure used to intercept and pre-filter raw Windows messages
/// directly at the beginning of the `global_wnd_proc` handler.
///
/// If the closure returns `Some(LRESULT)`, the message bypasses the rest of the message loop,
/// including standard translate handlers and default window procedures.
///
/// # Safety
/// To prevent resource leaks and fatal crashes, `WM_NCDESTROY` is protected internally
/// and cannot be bypassed by this filter.
#[derive(Clone)]
pub struct MessageFilter(pub Arc<dyn Fn(HWND, u32, WPARAM, LPARAM) -> Option<LRESULT>>);

impl std::fmt::Debug for MessageFilter {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "<MessageFilter>")
    }
}

/// Runtime state management structure maintained for each window
pub(crate) struct WindowState {
    pub(crate) message_filter: Option<MessageFilter>,
    pub(crate) auto_dpi_scaling: bool,
    pub(crate) is_cursor_inside: bool,
    pub(crate) min_inner_size: Option<LogicalSize>,
    pub(crate) max_inner_size: Option<LogicalSize>,
    pub(crate) saved_style: Option<WINDOW_STYLE>,
    pub(crate) saved_ex_style: Option<WINDOW_EX_STYLE>,
    pub(crate) saved_rect: Option<RECT>,
    pub(crate) current_cursor: CursorIcon,
    pub(crate) ime_relay: Option<Arc<ImeRelayServer>>,
}

struct CreationContext {
    state: *mut WindowState,
    was_taken: bool,
}

unsafe extern "system" fn global_wnd_proc(
    hwnd: HWND,
    msg: u32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    if msg == WM_NCCREATE {
        let create_struct = lparam.0 as *const CREATESTRUCTW;
        if !create_struct.is_null() {
            let context_ptr = unsafe { (*create_struct).lpCreateParams as *mut CreationContext };
            if !context_ptr.is_null() {
                let context = unsafe { &mut *context_ptr };
                unsafe {
                    SetWindowLongPtrW(hwnd, GWLP_USERDATA, context.state as isize);
                }
                context.was_taken = true; // 所有権がWndProc側に移ったことをマーク
            }
        }

        // 非クライアント領域（タイトルバーや境界線、スクロールバー等）の自動DPIスケーリングを有効化（Windows 10 1607以降）
        unsafe {
            let _ = EnableNonClientDpiScaling(hwnd);
        }
    }

    // 登録された WindowState を取得
    let state_ptr = unsafe { GetWindowLongPtrW(hwnd, GWLP_USERDATA) as *mut WindowState };

    // カスタムメッセージフィルターの実行
    let mut custom_result = None;
    if !state_ptr.is_null() {
        let state = unsafe { &*state_ptr };
        if let Some(ref filter) = state.message_filter {
            custom_result = (filter.0)(hwnd, msg, wparam, lparam);
        }
    }

    // フィルターが値を返した場合、処理をバイパスしてリターンする
    if let Some(result) = custom_result {
        // WM_NCDESTROY だけは絶対にここでバイパスしてはならない
        // バイパスすると WindowState のメモリ解放がスキップされ DefWindowProcW も呼ばれなくなる
        if msg != WM_NCDESTROY {
            return result;
        }
    }

    if !state_ptr.is_null() {
        let state = unsafe { &mut *state_ptr };

        if let Some(result) = translate_and_push(hwnd, msg, wparam, lparam, state)
            && msg != WM_NCDESTROY
        {
            return result;
        }
    }

    if msg == WM_GETMINMAXINFO {
        let mmi = lparam.0 as *mut MINMAXINFO;

        if !mmi.is_null() && !state_ptr.is_null() {
            let state = unsafe { &*state_ptr };
            let mmi = unsafe { &mut *mmi };

            // 現在このウィンドウが存在しているモニターのDPIをリアルタイムに取得
            let dpi = unsafe { GetDpiForWindow(hwnd) };
            let scale_factor = dpi as f64 / 96.0;

            // 最小サイズ制限の動的スケーリング適用
            if let Some(logical_min) = state.min_inner_size {
                let physical_min = logical_min.to_physical(scale_factor);
                mmi.ptMinTrackSize.x = physical_min.width;
                mmi.ptMinTrackSize.y = physical_min.height;
            }

            // 最大サイズ制限の動的スケーリング適用
            if let Some(logical_max) = state.max_inner_size {
                let physical_max = logical_max.to_physical(scale_factor);
                mmi.ptMaxTrackSize.x = physical_max.width;
                mmi.ptMaxTrackSize.y = physical_max.height;
            }
        }
        return LRESULT(0); // OSのデフォルト処理をスキップ
    }

    if msg == WM_NCDESTROY {
        let old_ptr = unsafe { SetWindowLongPtrW(hwnd, GWLP_USERDATA, 0) as *mut WindowState };
        if !old_ptr.is_null() {
            let _boxed_filter = unsafe { Box::from_raw(old_ptr) };
        }
    }

    unsafe { DefWindowProcW(hwnd, msg, wparam, lparam) }
}

fn register_window_class(
    class_name: PCWSTR,
    default_class_name: &'static str,
    hinstance: HINSTANCE,
    builder: &WindowBuilder<'_>,
) -> Result<()> {
    let hcursor = unsafe { LoadCursorW(None, IDC_ARROW).map_err(MichiuError::UnexpectedOsError)? };

    let wnd_class = WNDCLASSEXW {
        cbSize: std::mem::size_of::<WNDCLASSEXW>() as u32,
        style: CS_HREDRAW | CS_VREDRAW,
        lpfnWndProc: Some(global_wnd_proc),
        cbClsExtra: 0,
        cbWndExtra: 0,
        hInstance: hinstance,
        hIcon: HICON::default(),
        hCursor: hcursor,
        hbrBackground: HBRUSH::default(),
        lpszMenuName: PCWSTR::null(),
        lpszClassName: class_name,
        hIconSm: HICON::default(),
    };

    let atom = unsafe { RegisterClassExW(&wnd_class) };
    if atom == 0 {
        let err = windows::core::Error::from_thread();

        // エラーがすでに登録されている場合（ERROR_CLASS_ALREADY_EXISTS）、
        // これは2回目以降のウィンドウであるため、操作が正常に完了したかのように処理を進めても問題ない
        if err.code() != ERROR_CLASS_ALREADY_EXISTS.to_hresult() {
            let class_str_to_report = builder
                .custom_class_name
                .clone()
                .unwrap_or_else(|| default_class_name.into());

            return Err(MichiuError::ClassRegistrationFailed {
                class_name: class_str_to_report,
                source: err,
            });
        }
    }

    Ok(())
}

fn get_class_name_utf16(default_class_name: &str, builder: &WindowBuilder<'_>) -> Vec<u16> {
    if let Some(ref custom_name) = builder.custom_class_name {
        custom_name.encode_utf16().chain(Some(0)).collect()
    } else {
        default_class_name.encode_utf16().chain(Some(0)).collect()
    }
}

fn calc_window_rect(
    style: WINDOW_STYLE,
    ex_style: WINDOW_EX_STYLE,
    builder: &WindowBuilder<'_>,
    dpi: u32,
    scale_factor: f64,
) -> Result<(i32, i32)> {
    match builder.inner_size {
        None => Ok((CW_USEDEFAULT, CW_USEDEFAULT)),
        Some(logical_size) => {
            let (physical_width, physical_height) = if builder.auto_dpi_scaling {
                let physical_size = logical_size.to_physical(scale_factor);
                (physical_size.width, physical_size.height)
            } else {
                (logical_size.width as i32, logical_size.height as i32)
            };
            let mut rect = RECT {
                left: 0,
                top: 0,
                right: physical_width,
                bottom: physical_height,
            };
            unsafe {
                AdjustWindowRectExForDpi(&mut rect, style, false, ex_style, dpi).map_err(
                    |err| MichiuError::GeometryUpdateFailed {
                        x: 0,
                        y: 0,
                        width: logical_size.width as u32,
                        height: logical_size.height as u32,
                        dpi,
                        source: err,
                    },
                )?;
            }
            let outer_width = rect.right - rect.left;
            let outer_height = rect.bottom - rect.top;
            Ok((outer_width, outer_height))
        }
    }
}

fn build_raw_style(builder: &WindowBuilder<'_>) -> WINDOW_STYLE {
    if let Some(raw) = builder.raw_style {
        raw
    } else {
        let mut s = WINDOW_STYLE(0);
        if builder.overlapped_window {
            s |= WS_OVERLAPPEDWINDOW;
        } else if builder.parent_hwnd.is_some() {
            s |= WS_CHILD;
        } else {
            s |= WS_POPUP;
        }

        if builder.visible {
            s |= WS_VISIBLE;
        } else {
            s &= !WS_VISIBLE;
        }

        if builder.resizable {
            s |= WS_THICKFRAME | WS_MAXIMIZEBOX;
        } else {
            s &= !(WS_THICKFRAME | WS_MAXIMIZEBOX);
        }

        if builder.maximized {
            s |= WS_MAXIMIZE;
        } else {
            s &= !WS_MAXIMIZE;
        }

        if !builder.decorations {
            s &= !(WS_CAPTION | WS_BORDER | WS_THICKFRAME | WS_DLGFRAME);
            if builder.parent_hwnd.is_none() {
                s |= WS_POPUP;
            }
        }
        s
    }
}

fn build_raw_ex_style(builder: &WindowBuilder<'_>) -> WINDOW_EX_STYLE {
    if let Some(raw_ex) = builder.raw_ex_style {
        raw_ex
    } else {
        let mut ex = WINDOW_EX_STYLE(0);
        if builder.transparent {
            ex |= WS_EX_LAYERED;
        }

        if !builder.hittest {
            ex |= WS_EX_TRANSPARENT;
        }

        if builder.taskbar_button {
            ex |= WS_EX_APPWINDOW;
        } else {
            ex &= !WS_EX_APPWINDOW;
        }

        if builder.no_redirection_bitmap {
            ex |= WS_EX_NOREDIRECTIONBITMAP;
        }
        ex
    }
}

/// Sets the window title bar to dark mode (Windows 10 version 1809 or later).
///
/// # Safety
///
/// This function dynamically loads and calls Windows' undocumented DwmAPI ordinal API.
pub(crate) unsafe fn enable_dark_mode_titlebar(hwnd: HWND, enable: bool) {
    let value: i32 = if enable { 1 } else { 0 };
    // DWMWA_USE_IMMERSIVE_DARK_MODE (属性値: 20)
    let _ = unsafe {
        DwmSetWindowAttribute(
            hwnd,
            DWMWA_USE_IMMERSIVE_DARK_MODE,
            &value as *const i32 as *const _,
            std::mem::size_of::<i32>() as u32,
        )
    };
}

/// Forces Windows 11/10 native dark mode on all standard context menus (HMENU).
/// (Windows 10 version 1903 or later)
///
/// # Safety
///
/// This function dynamically loads and calls Windows' undocumented UXTheme ordinal API.
pub(crate) unsafe fn set_app_theme(mode: PreferredAppMode) {
    let uxtheme_name: Vec<u16> = "uxtheme.dll"
        .encode_utf16()
        .chain(std::iter::once(0))
        .collect();

    unsafe {
        if let Ok(h_uxtheme) = LoadLibraryW(PCWSTR(uxtheme_name.as_ptr())) {
            // 序数 135: SetPreferredAppMode (ダークモード設定の強制適用)
            // GetProcAddress に序数を渡すため、キャストして PCSTR に包む
            let set_preferred_app_mode_ptr = GetProcAddress(h_uxtheme, PCSTR(135 as *const u8));
            if let Some(func) = set_preferred_app_mode_ptr {
                let set_preferred_app_mode: unsafe extern "system" fn(i32) -> i32 =
                    std::mem::transmute(func);
                set_preferred_app_mode(mode as i32);
            }

            // 序数 136: FlushMenuThemes (メニューテーマのキャッシュフラッシュ＆即時適用)
            let flush_menu_themes_ptr = GetProcAddress(h_uxtheme, PCSTR(136 as *const u8));
            if let Some(func) = flush_menu_themes_ptr {
                let flush_menu_themes: unsafe extern "system" fn() = std::mem::transmute(func);
                flush_menu_themes();
            }

            // 動的ロードしたライブラリハンドルを確実に返却してアンロード
            let _ = FreeLibrary(h_uxtheme);
        }
    }
}

/// Checks whether the current Windows OS is set to dark mode.
pub(crate) fn is_system_dark_mode() -> bool {
    unsafe {
        let subkey: Vec<u16> = "Software\\Microsoft\\Windows\\CurrentVersion\\Themes\\Personalize"
            .encode_utf16()
            .chain(std::iter::once(0))
            .collect();

        let mut hkey = HKEY::default();

        if RegOpenKeyExW(
            HKEY_CURRENT_USER,
            windows::core::PCWSTR(subkey.as_ptr()),
            Some(0),
            KEY_READ,
            &mut hkey,
        )
        .is_ok()
        {
            let value_name: Vec<u16> = "AppsUseLightTheme"
                .encode_utf16()
                .chain(std::iter::once(0))
                .collect();

            let mut value_type = REG_DWORD;
            let mut data = 0u32;
            let mut data_size = std::mem::size_of::<u32>() as u32;

            let res = RegQueryValueExW(
                hkey,
                windows::core::PCWSTR(value_name.as_ptr()),
                None,
                Some(&mut value_type),
                Some(&mut data as *mut u32 as *mut u8),
                Some(&mut data_size),
            );

            let _ = RegCloseKey(hkey);

            if res.is_ok() {
                // AppsUseLightTheme == 0 ならダークモード
                // AppsUseLightTheme == 1 ならライトモード
                return data == 0;
            }
        }
    }
    false // 取得失敗時は安全のためにライトモード（false）をデフォルトとする
}

pub(crate) fn set_clipboard_text_impl(hwnd: HWND, text: &str) -> Result<()> {
    unsafe {
        if OpenClipboard(Some(hwnd)).is_err() {
            return Err(MichiuError::UnexpectedOsError(
                windows::core::Error::from_thread(),
            ));
        }
        let _ = EmptyClipboard();

        let wide: Vec<u16> = text.encode_utf16().chain(std::iter::once(0)).collect();
        let byte_count = wide.len() * 2;

        let h_mem = windows::Win32::System::Memory::GlobalAlloc(GMEM_MOVEABLE, byte_count)
            .map_err(MichiuError::UnexpectedOsError)?;
        let ptr = GlobalLock(HGLOBAL(h_mem.0));
        if ptr.is_null() {
            let _ = CloseClipboard();
            return Err(MichiuError::UnexpectedOsError(
                windows::core::Error::from_thread(),
            ));
        }

        std::ptr::copy_nonoverlapping(wide.as_ptr(), ptr as *mut u16, wide.len());
        let _ = GlobalUnlock(HGLOBAL(h_mem.0));

        if SetClipboardData(CF_UNICODETEXT.0 as u32, Some(HANDLE(h_mem.0 as _))).is_err() {
            let _ = GlobalFree(Some(h_mem)); // メモリの解放漏れを防止
            let _ = CloseClipboard();
            return Err(MichiuError::UnexpectedOsError(
                windows::core::Error::from_thread(),
            ));
        }
        let _ = CloseClipboard();
        Ok(())
    }
}

pub(crate) fn get_clipboard_text_impl(hwnd: HWND) -> Result<String> {
    unsafe {
        if OpenClipboard(Some(hwnd)).is_err() {
            return Err(MichiuError::UnexpectedOsError(
                windows::core::Error::from_thread(),
            ));
        }
        let h_mem = GetClipboardData(CF_UNICODETEXT.0 as u32).map_err(|err| {
            let _ = CloseClipboard();
            MichiuError::UnexpectedOsError(err)
        })?;

        let ptr = GlobalLock(HGLOBAL(h_mem.0));
        if ptr.is_null() {
            let _ = CloseClipboard();
            return Err(MichiuError::UnexpectedOsError(
                windows::core::Error::from_thread(),
            ));
        }

        let max_bytes = GlobalSize(HGLOBAL(h_mem.0));
        let max_u16_count = max_bytes / 2; // u16 (Wide文字) の最大要素数に換算

        let len = {
            let mut p = ptr as *const u16;
            let mut count = 0;
            // ヌル終端、または割り当てられたバッファの上限サイズに達した時点でループを打ち切る
            while count < max_u16_count && *p != 0 {
                count += 1;
                p = p.add(1);
            }
            count
        };

        let slice = std::slice::from_raw_parts(ptr as *const u16, len);
        let text = String::from_utf16_lossy(slice);

        let _ = GlobalUnlock(HGLOBAL(h_mem.0));
        let _ = CloseClipboard();
        Ok(text)
    }
}

pub(crate) fn set_cursor_capture_impl(hwnd: HWND, capture: bool) {
    unsafe {
        if capture {
            SetCapture(hwnd);
        } else {
            let _ = ReleaseCapture();
        }
    }
}

pub(crate) fn set_cursor_clipping_impl(hwnd: HWND, clip: bool) {
    unsafe {
        if clip {
            let mut rect = RECT::default();
            let _ = GetClientRect(hwnd, &mut rect);

            let mut points = [
                windows::Win32::Foundation::POINT {
                    x: rect.left,
                    y: rect.top,
                },
                windows::Win32::Foundation::POINT {
                    x: rect.right,
                    y: rect.bottom,
                },
            ];
            // クライアント座標からスクリーン全体の絶対座標にマップ
            let _ = MapWindowPoints(Some(hwnd), Some(HWND::default()), &mut points);

            let screen_rect = RECT {
                left: points[0].x,
                top: points[0].y,
                right: points[1].x,
                bottom: points[1].y,
            };
            let _ = ClipCursor(Some(&screen_rect));
        } else {
            let _ = ClipCursor(None);
        }
    }
}

pub(crate) fn get_monitor_rect(hwnd: HWND) -> RECT {
    unsafe {
        let monitor = MonitorFromWindow(hwnd, MONITOR_DEFAULTTONEAREST);
        let mut info = MONITORINFO {
            cbSize: std::mem::size_of::<MONITORINFO>() as u32,
            ..Default::default()
        };
        let _ = GetMonitorInfoW(monitor, &mut info);
        info.rcMonitor // フルスクリーンのため rcMonitor を使用
    }
}

pub(crate) fn center_on_screen_impl(hwnd: HWND) {
    let monitor_rect = get_monitor_rect(hwnd);
    let mut window_rect = RECT::default();
    let _ = unsafe { GetWindowRect(hwnd, &mut window_rect) };

    let win_width = window_rect.right - window_rect.left;
    let win_height = window_rect.bottom - window_rect.top;

    let monitor_width = monitor_rect.right - monitor_rect.left;
    let monitor_height = monitor_rect.bottom - monitor_rect.top;

    let x = monitor_rect.left + (monitor_width - win_width) / 2;
    let y = monitor_rect.top + (monitor_height - win_height) / 2;

    let _ = unsafe {
        SetWindowPos(
            hwnd,
            None,
            x,
            y,
            0,
            0,
            SWP_NOSIZE | SWP_NOZORDER | SWP_NOACTIVATE,
        )
    };
}

pub(crate) fn set_fullscreen_impl(hwnd: HWND, fullscreen: bool) {
    let state_ptr = unsafe { GetWindowLongPtrW(hwnd, GWLP_USERDATA) as *mut WindowState };
    if state_ptr.is_null() {
        return;
    }
    let state = unsafe { &mut *state_ptr };

    if fullscreen {
        if state.saved_rect.is_none() {
            unsafe {
                // 元のウインドウスタイルと位置を退避
                let style = WINDOW_STYLE(GetWindowLongPtrW(hwnd, GWL_STYLE) as _);
                let ex_style = WINDOW_EX_STYLE(GetWindowLongPtrW(hwnd, GWL_EXSTYLE) as _);
                let mut rect = RECT::default();
                let _ = GetWindowRect(hwnd, &mut rect);

                state.saved_style = Some(style);
                state.saved_ex_style = Some(ex_style);
                state.saved_rect = Some(rect);

                // 境界線やタイトルバーのスタイルを除去
                let new_style =
                    style & !(WS_CAPTION | WS_THICKFRAME | WS_MAXIMIZEBOX | WS_MINIMIZEBOX);
                let new_ex_style =
                    ex_style & !(WS_EX_DLGMODALFRAME | WS_EX_CLIENTEDGE | WS_EX_STATICEDGE);

                let _ = SetWindowLongPtrW(hwnd, GWL_STYLE, new_style.0 as _);
                let _ = SetWindowLongPtrW(hwnd, GWL_EXSTYLE, new_ex_style.0 as _);

                let monitor_rect = get_monitor_rect(hwnd);
                // モニター全体を覆うようにサイズ変更
                let _ = SetWindowPos(
                    hwnd,
                    Some(HWND_TOP),
                    monitor_rect.left,
                    monitor_rect.top,
                    monitor_rect.right - monitor_rect.left,
                    monitor_rect.bottom - monitor_rect.top,
                    SWP_FRAMECHANGED | SWP_NOACTIVATE,
                );
            }
        }
    } else if let Some(rect) = state.saved_rect {
        let style = state.saved_style.unwrap();
        let ex_style = state.saved_ex_style.unwrap();

        unsafe {
            // 元のスタイルに復元
            let _ = SetWindowLongPtrW(hwnd, GWL_STYLE, style.0 as _);
            let _ = SetWindowLongPtrW(hwnd, GWL_EXSTYLE, ex_style.0 as _);

            let _ = SetWindowPos(
                hwnd,
                None,
                rect.left,
                rect.top,
                rect.right - rect.left,
                rect.bottom - rect.top,
                windows::Win32::UI::WindowsAndMessaging::SWP_FRAMECHANGED | SWP_NOACTIVATE,
            );

            state.saved_style = None;
            state.saved_ex_style = None;
            state.saved_rect = None;
        }
    }
}

#[allow(unused)]
pub(crate) fn apply_cursor_icon_impl(hwnd: HWND, cursor: crate::CursorIcon) -> LRESULT {
    unsafe {
        let idc = match cursor {
            crate::CursorIcon::Default => IDC_ARROW,
            crate::CursorIcon::Hand => IDC_HAND,
            crate::CursorIcon::IBeam => IDC_IBEAM,
            crate::CursorIcon::Wait => IDC_WAIT,
            crate::CursorIcon::Cross => IDC_CROSS,
        };

        // システムのカーソルリソースをロードして適用
        if let Ok(hcursor) = LoadCursorW(None, idc) {
            let _ = SetCursor(Some(hcursor));
        }

        LRESULT(1) // DefWindowProcWに流さない
    }
}

#[cfg(test)]
mod tests;
