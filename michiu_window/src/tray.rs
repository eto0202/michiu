use std::{borrow::Cow, sync::Arc};

use michiu_guard::{Unvalidated, Validate, Validated};
use windows::{
    Win32::{
        Foundation::{ERROR_CLASS_ALREADY_EXISTS, HINSTANCE, HWND, LPARAM, LRESULT, POINT, WPARAM},
        Graphics::Gdi::HBRUSH,
        System::LibraryLoader::GetModuleHandleW,
        UI::{
            Shell::{
                NIF_ICON, NIF_INFO, NIF_MESSAGE, NIF_TIP, NIIF_INFO, NIM_ADD, NIM_DELETE,
                NIM_MODIFY, NOTIFYICONDATAW, Shell_NotifyIconW,
            },
            WindowsAndMessaging::{
                AppendMenuW, CREATESTRUCTW, CS_HREDRAW, CS_VREDRAW, CreatePopupMenu,
                CreateWindowExW, DefWindowProcW, DestroyMenu, GWLP_USERDATA, GetCursorPos,
                GetWindowLongPtrW, HICON, HWND_TOPMOST, IDC_ARROW, LoadCursorW, MF_STRING,
                PostMessageW, RegisterClassExW, RegisterWindowMessageW, SW_SHOW, SWP_NOACTIVATE,
                SWP_NOSIZE, SetForegroundWindow, SetWindowLongPtrW, SetWindowPos, ShowWindow,
                TPM_LEFTALIGN, TrackPopupMenu, WINDOW_EX_STYLE, WINDOW_STYLE, WM_CLOSE, WM_COMMAND,
                WM_LBUTTONDBLCLK, WM_NCCREATE, WM_NCDESTROY, WM_NULL, WM_RBUTTONUP, WM_USER,
                WNDCLASSEXW,
            },
        },
    },
    core::{PCWSTR, w},
};

use crate::{Icon, PreferredAppMode, set_app_theme};
use crate::{
    WindowHandle,
    error::{MichiuError, Result},
};

/// Window message sub-ID used to identify tray icon mouse event callbacks.
pub const WM_TRAY_CALLBACK: u32 = WM_USER + 100;

/// Represents an active, reference-counted system tray (notification area) icon on Windows.
///
/// It manages an internal dummy hidden window to handle OS shell notification events asynchronously.
/// `Tray` is designed to be **`Clone`**, **`Send`**, and **`Sync`** (internally managed via `Arc`).
///
/// When all cloned instances of this `Tray` are dropped, the tray icon is automatically
/// deleted from the system shell and its hidden dummy window is safely destroyed.
#[derive(Debug)]
pub struct Tray {
    inner: Arc<TrayInner>,
}

impl Clone for Tray {
    fn clone(&self) -> Self {
        Self {
            inner: self.inner.clone(),
        }
    }
}

#[derive(Debug)]
struct TrayInner {
    dummy_hwnd: HWND,
    u_id: u32,
}

unsafe impl Send for TrayInner {}
unsafe impl Sync for TrayInner {}

impl Drop for TrayInner {
    fn drop(&mut self) {
        let nid = NOTIFYICONDATAW {
            cbSize: std::mem::size_of::<NOTIFYICONDATAW>() as u32,
            hWnd: self.dummy_hwnd,
            uID: self.u_id,
            ..Default::default()
        };
        unsafe {
            // システムトレイからアイコンを削除
            let _ = Shell_NotifyIconW(NIM_DELETE, &nid);
            // 非表示のダミーウィンドウを安全に破棄
            let _ = PostMessageW(Some(self.dummy_hwnd), WM_CLOSE, WPARAM(0), LPARAM(0));
        }
    }
}

struct TrayInternalData {
    on_left_click: Option<Arc<dyn Fn() + Send + Sync>>,
    on_right_click: Option<Arc<dyn Fn() + Send + Sync>>,
    taskbar_created_msg: u32,
    icon: Option<Icon>,
    tooltip: Option<Cow<'static, str>>,
    menu_items: Vec<TrayMenuItem>,
    custom_menu: Option<CustomTrayMenu>,
}

const CLASS_NAME_STR: &str = concat!("MichiuTrayWindowClass_", env!("CARGO_PKG_VERSION"));

impl Tray {
    /// Builds and registers a new system tray icon with the OS using the validated builder parameters.
    ///
    /// # Errors
    /// Returns [`MichiuError::TrayWindowCreationFailed`] if the internal dummy window allocation fails,
    /// or [`MichiuError::TrayIconOperationFailed`] if registering with the OS shell fails.
    ///
    /// # Examples
    ///
    /// ```no_run
    /// # use michiu_window::{Tray, TrayBuilder, Icon};
    /// # fn main() -> Result<(), Box<dyn std::error::Error>> {
    /// # let icon = unsafe { Icon::from_raw(windows::Win32::UI::WindowsAndMessaging::HICON(std::ptr::null_mut())) };
    /// let builder = TrayBuilder::new()
    ///     .with_icon(icon)
    ///     .with_tooltip("My App Tray");
    ///
    /// let tray = Tray::build(builder.into_unvalidated().try_into()?)?;
    /// # Ok(())
    /// # }
    /// ```
    pub fn build(builder: Validated<TrayBuilder>) -> Result<Self> {
        let builder = builder.into_inner();

        let hmodule = unsafe { GetModuleHandleW(None).map_err(MichiuError::UnexpectedOsError)? };
        let hinstance = HINSTANCE(hmodule.0);

        let class_name_wide: Vec<u16> = CLASS_NAME_STR.encode_utf16().chain(Some(0)).collect();
        let class_name = PCWSTR(class_name_wide.as_ptr());

        register_tray_window_class(class_name, CLASS_NAME_STR, hinstance)?;

        // 「TaskbarCreated」の登録済みウィンドウメッセージIDを取得
        let taskbar_created_msg = unsafe { RegisterWindowMessageW(w!("TaskbarCreated")) };

        let internal_data = Box::into_raw(Box::new(TrayInternalData {
            on_left_click: builder.on_left_click,
            on_right_click: builder.on_right_click,
            taskbar_created_msg,
            icon: builder.icon.clone(),
            tooltip: builder.tooltip.clone(),
            menu_items: builder.menu_items,
            custom_menu: builder.custom_menu,
        }));

        // 非表示（WS_VISIBLE なし）のトップレベルダミーウィンドウを作成する。
        let dummy_hwnd = unsafe {
            match CreateWindowExW(
                WINDOW_EX_STYLE(0),
                class_name,
                PCWSTR::default(),
                WINDOW_STYLE(0),
                0,
                0,
                0,
                0,
                None,
                None,
                None,
                Some(internal_data as *const std::ffi::c_void),
            ) {
                Ok(hwnd) => hwnd,
                Err(err) => {
                    // 失敗時にメモリリークを防ぐため、内部データを手動で取得して破棄する。
                    let _ = Box::from_raw(internal_data);
                    return Err(MichiuError::TrayWindowCreationFailed { source: err });
                }
            }
        };

        // このトレイアイコンに関連付けられた一意のID
        let u_id = 1;

        add_tray_icon(dummy_hwnd, u_id, builder.icon, builder.tooltip.as_deref())?;

        Ok(Tray {
            inner: Arc::new(TrayInner { dummy_hwnd, u_id }),
        })
    }

    /// Applies Windows native light or dark mode styling to the tray context menu on this thread.
    pub fn set_theme(&self, mode: PreferredAppMode) {
        unsafe {
            set_app_theme(mode);
        }
    }

    /// Displays a temporary balloon (toast) notification from the system tray icon dynamically.
    ///
    /// This method is safe to call from any background thread since `Tray` is thread-safe.
    ///
    /// # Parameter Limits
    /// * `title` - Up to 63 UTF-16 code units.
    /// * `text` - Up to 255 UTF-16 code units.
    ///
    /// # Errors
    /// Returns [`MichiuError::ValidationError`] if title or text lengths exceed OS limits,
    /// or [`MichiuError::TrayIconOperationFailed`] if sending the modify signal to the OS fails.
    ///
    /// # Examples
    ///
    /// ```no_run
    /// # use michiu_window::{Tray, TrayBuilder, Icon};
    /// # fn main() -> Result<(), Box<dyn std::error::Error>> {
    /// # let icon = unsafe { Icon::from_raw(windows::Win32::UI::WindowsAndMessaging::HICON(std::ptr::null_mut())) };
    /// # let tray = Tray::build(TrayBuilder::new().with_icon(icon).into_unvalidated().try_into()?)?;
    /// // Display a notification from a background thread
    /// let tray_clone = tray.clone();
    /// std::thread::spawn(move || {
    ///     tray_clone.show_balloon(
    ///         "Task Completed",
    ///         "Your heavy background task has finished successfully!"
    ///     ).unwrap();
    /// });
    /// # Ok(())
    /// # }
    /// ```
    pub fn show_balloon(&self, title: &str, text: &str) -> Result<()> {
        let title_len = title.encode_utf16().count();
        if title_len >= 64 {
            return Err(MichiuError::ValidationError {
                parameter: "balloon_title",
                message: format!(
                    "Balloon title exceeds the OS limit of 63 UTF-16 code units (current: {}).",
                    title_len
                )
                .into(),
            });
        }

        let text_len = text.encode_utf16().count();
        if text_len >= 256 {
            return Err(MichiuError::ValidationError {
                    parameter: "balloon_text",
                    message: format!("Balloon text message exceeds the OS limit of 255 UTF-16 code units (current: {}).", text_len).into(),
                });
        }

        let flags = NIF_INFO;

        let mut sz_info = [0u16; 256];
        let mut sz_info_title = [0u16; 64];
        let mut info_flags = windows::Win32::UI::Shell::NOTIFY_ICON_INFOTIP_FLAGS::default();

        // 標準の情報マークアイコン(i)をトースト上に表示
        info_flags |= NIIF_INFO;

        let text_wide: Vec<u16> = text.encode_utf16().collect();
        sz_info[..text_wide.len()].copy_from_slice(&text_wide);

        let title_wide: Vec<u16> = title.encode_utf16().collect();
        sz_info_title[..title_wide.len()].copy_from_slice(&title_wide);

        // すでに登録されている HWND と uID をターゲットにして通知を送信
        let nid = NOTIFYICONDATAW {
            cbSize: std::mem::size_of::<NOTIFYICONDATAW>() as u32,
            hWnd: self.inner.dummy_hwnd,
            uID: self.inner.u_id,
            uFlags: flags,
            szInfo: sz_info,
            szInfoTitle: sz_info_title,
            dwInfoFlags: info_flags,
            ..Default::default()
        };

        // NIM_MODIFYを送信してトースト通知のみをトリガー
        let success = unsafe { Shell_NotifyIconW(NIM_MODIFY, &nid) };
        if success.as_bool() {
            Ok(())
        } else {
            Err(MichiuError::TrayIconOperationFailed {
                source: windows::core::Error::from_thread(),
            })
        }
    }
}

unsafe extern "system" fn tray_wnd_proc(
    hwnd: HWND,
    msg: u32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    if msg == WM_NCCREATE {
        let create_struct = lparam.0 as *const CREATESTRUCTW;
        if !create_struct.is_null() {
            let data_ptr = unsafe { (*create_struct).lpCreateParams as *mut TrayInternalData };
            unsafe {
                SetWindowLongPtrW(hwnd, GWLP_USERDATA, data_ptr as isize);
            }
        }
    }

    let data_ptr = unsafe { GetWindowLongPtrW(hwnd, GWLP_USERDATA) as *mut TrayInternalData };

    if !data_ptr.is_null() {
        let data = unsafe { &*data_ptr };

        // トレイアイコンでのマウス操作
        if msg == WM_TRAY_CALLBACK {
            let event = lparam.0 as u32;
            match event {
                // 左クリックをダブルクリック（またはシングルクリック）
                WM_LBUTTONDBLCLK => {
                    if let Some(ref cb) = data.on_left_click {
                        cb();
                    }
                }
                // 右クリック（ポップアップメニューの表示など）。
                WM_RBUTTONUP => {
                    // カスタムウィンドウメニューが設定されている場合
                    if let Some(ref custom_menu) = data.custom_menu {
                        let hwnd_menu = custom_menu.window_handle.hwnd();
                        unsafe {
                            let mut pt = POINT::default();
                            let _ = GetCursorPos(&mut pt);

                            // マウス位置にカスタムウィンドウ（メニュー）を移動させて表示
                            let _ = SetWindowPos(
                                hwnd_menu,
                                Some(HWND_TOPMOST), // 常に最前面化
                                pt.x,
                                pt.y,
                                0,
                                0,
                                SWP_NOSIZE | SWP_NOACTIVATE,
                            );

                            let _ = ShowWindow(hwnd_menu, SW_SHOW);
                            let _ = SetForegroundWindow(hwnd_menu);

                            // フォーカスロストを確実に効かせるための空メッセージ
                            let _ = PostMessageW(Some(hwnd), WM_NULL, WPARAM(0), LPARAM(0));
                        }
                    }
                    // 手動の右クリックコールバックがある場合
                    else if let Some(ref cb) = data.on_right_click {
                        cb();
                    }
                    // ネイティブのポップアップメニューを構築して表示する
                    else if !data.menu_items.is_empty() {
                        unsafe {
                            match CreatePopupMenu() {
                                Ok(hmenu) => {
                                    for item in &data.menu_items {
                                        let text_wide: Vec<u16> =
                                            item.text.encode_utf16().chain(Some(0)).collect();
                                        let _ = AppendMenuW(
                                            hmenu,
                                            MF_STRING,
                                            item.id as usize,
                                            PCWSTR(text_wide.as_ptr()),
                                        );
                                    }

                                    let mut pt = POINT::default();
                                    let _ = GetCursorPos(&mut pt);

                                    let _ = SetForegroundWindow(hwnd);

                                    let _ = TrackPopupMenu(
                                        hmenu,
                                        TPM_LEFTALIGN,
                                        pt.x,
                                        pt.y,
                                        Some(0),
                                        hwnd,
                                        None,
                                    );

                                    let _ = PostMessageW(Some(hwnd), WM_NULL, WPARAM(0), LPARAM(0));
                                    let _ = DestroyMenu(hmenu);
                                }

                                Err(err) => {
                                    let os_error = MichiuError::UnexpectedOsError(err);
                                    tracing::error!(
                                        "[michiu_window] Failed to show tray popup menu. System error: {}",
                                        os_error
                                    );
                                }
                            }
                        }
                    }
                }
                _ => {}
            }
            return LRESULT(0);
        }

        if msg == WM_COMMAND {
            // wparamの下位16ビットには、クリックされたメニュー項目のIDが含まれている
            let clicked_id = (wparam.0 & 0xFFFF) as u32;

            // 一致するIDを持つアイテムを検索し、そのコールバックを実行します。
            if let Some(cb) = data
                .menu_items
                .iter()
                .find(|i| i.id == clicked_id)
                .and_then(|i| i.on_click.as_ref())
            {
                cb();
            }

            return LRESULT(0);
        }

        // Windows エクスプローラー（タスクバー）が再起動した場合の自動復旧。
        if msg == data.taskbar_created_msg {
            // タスクバーが再起動したため、トレイにアイコンを再度追加する（NIM_ADD）。
            let _ = add_tray_icon(hwnd, 1, data.icon.clone(), data.tooltip.as_deref());
            return LRESULT(0);
        }
    }

    if msg == WM_NCDESTROY && !data_ptr.is_null() {
        let _ = unsafe { Box::from_raw(data_ptr) };
        unsafe { SetWindowLongPtrW(hwnd, GWLP_USERDATA, 0) };
    }

    unsafe { DefWindowProcW(hwnd, msg, wparam, lparam) }
}

fn add_tray_icon(hwnd: HWND, u_id: u32, icon: Option<Icon>, tooltip: Option<&str>) -> Result<()> {
    let mut flags = NIF_MESSAGE;

    // 有効なアイコンハンドルが指定されている場合は、アイコンフラグを追加
    let h_icon = if let Some(i) = icon {
        flags |= NIF_ICON;
        i.as_raw()
    } else {
        HICON::default()
    };

    // ツールチップの文字列バッファをクリアしてコピーする。
    let mut sz_tip = [0u16; 128];
    if let Some(tip_text) = tooltip {
        flags |= NIF_TIP;

        let tip_wide: Vec<u16> = tip_text.encode_utf16().collect();
        // インデックス127の位置にヌル終端文字を残すため、文字列の長さを最大127文字に制限する。
        let len = std::cmp::min(tip_wide.len(), sz_tip.len() - 1);
        sz_tip[..len].copy_from_slice(&tip_wide[..len]);
    }

    let nid = NOTIFYICONDATAW {
        cbSize: std::mem::size_of::<NOTIFYICONDATAW>() as u32,
        hWnd: hwnd,
        uID: u_id,
        uFlags: flags,
        uCallbackMessage: WM_TRAY_CALLBACK,
        hIcon: h_icon,
        szTip: sz_tip,
        ..Default::default()
    };

    // アイコンをOSのシステムトレイに登録(NIM_ADD)
    let success = unsafe { Shell_NotifyIconW(NIM_ADD, &nid) };
    if success.as_bool() {
        Ok(())
    } else {
        Err(MichiuError::TrayIconOperationFailed {
            source: windows::core::Error::from_thread(),
        })
    }
}

/// A custom popup window wrapper used to display a customized UI (e.g., custom window menus)
/// instead of the standard Win32 native context menu when clicking the tray icon.
#[derive(Clone, Debug)]
pub struct CustomTrayMenu {
    window_handle: WindowHandle,
}

impl CustomTrayMenu {
    /// Wraps the specified window handle as a custom popup menu.
    ///
    /// Under the hood, clicking the tray icon will automatically relocate the target window
    /// to the cursor position, set it as topmost, and show it.
    pub fn new(window_handle: WindowHandle) -> Self {
        Self { window_handle }
    }
}

/// Represents a single item inside the tray's native pop-up context menu.
#[derive(Clone)]
pub struct TrayMenuItem {
    pub(crate) id: u32,
    text: Cow<'static, str>,
    on_click: Option<Arc<dyn Fn() + Send + Sync>>,
    on_hover: Option<Arc<dyn Fn() + Send + Sync>>,
    enabled: bool,
}

impl std::fmt::Debug for TrayMenuItem {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("TrayMenuItem")
            .field("id", &self.id)
            .field("text", &self.text)
            .field("on_click", &self.on_click.as_ref().map(|_| "<closure>"))
            .field("on_hover", &self.on_hover.as_ref().map(|_| "<closure>"))
            .field("enabled", &self.enabled)
            .finish()
    }
}

impl TrayMenuItem {
    /// Creates a new native menu item with the specified label text.
    pub fn new(text: impl Into<Cow<'static, str>>) -> Self {
        Self {
            id: 0,
            text: text.into(),
            on_click: None,
            on_hover: None,
            enabled: true,
        }
    }

    /// Registers a callback triggered when clicking this menu item.
    pub fn with_on_click<F>(mut self, on_click: F) -> Self
    where
        F: Fn() + Send + Sync + 'static,
    {
        self.on_click = Some(Arc::new(on_click));
        self
    }

    /// Registers a callback triggered when hovering over this menu item.
    pub fn with_on_hover<F>(mut self, on_hover: F) -> Self
    where
        F: Fn() + Send + Sync + 'static,
    {
        self.on_hover = Some(Arc::new(on_hover));
        self
    }

    /// Sets whether the menu item is enabled (clickable) or grayed out. (Default: `true`)
    pub fn with_enabled(mut self, enabled: bool) -> Self {
        self.enabled = enabled;
        self
    }
}

/// A builder helper used to configure and instantiate a [`Tray`].
///
/// Implements [`Default`] and [`Validate`] (from `michiu_guard`).
#[derive(Clone)]
pub struct TrayBuilder {
    tooltip: Option<Cow<'static, str>>,
    icon: Option<Icon>,
    on_left_click: Option<Arc<dyn Fn() + Send + Sync>>,
    on_right_click: Option<Arc<dyn Fn() + Send + Sync>>,
    menu_items: Vec<TrayMenuItem>,
    next_menu_id: u32,
    custom_menu: Option<CustomTrayMenu>,
    dark_mode_menus: bool,
}

impl std::fmt::Debug for TrayBuilder {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("TrayBuilder")
            .field("tooltip", &self.tooltip)
            .field("icon", &self.icon)
            .field(
                "on_left_click",
                &self.on_left_click.as_ref().map(|_| "<closure>"),
            )
            .field(
                "on_right_click",
                &self.on_right_click.as_ref().map(|_| "<closure>"),
            )
            .field("menu_items", &self.menu_items)
            .field("next_menu_id", &self.next_menu_id)
            .field("custom_menu", &self.custom_menu)
            .field("dark_mode_menus", &self.dark_mode_menus)
            .finish()
    }
}

impl Default for TrayBuilder {
    fn default() -> Self {
        Self {
            tooltip: None,
            icon: None,
            on_left_click: None,
            on_right_click: None,
            menu_items: Vec::new(),
            next_menu_id: 1,
            custom_menu: None,
            dark_mode_menus: false,
        }
    }
}

impl TrayBuilder {
    /// Creates a default configured `TrayBuilder` instance.
    pub fn new() -> Self {
        Self::default()
    }

    /// Sets the tooltip text displayed when hovering over the tray icon.
    ///
    /// Must be under 128 characters.
    pub fn with_tooltip(mut self, tooltip: impl Into<Cow<'static, str>>) -> Self {
        self.tooltip = Some(tooltip.into());
        self
    }

    /// Assigns a custom [`Icon`] for the tray. (Required)
    pub fn with_icon(mut self, icon: Icon) -> Self {
        self.icon = Some(icon);
        self
    }

    /// Registers a callback triggered when left-clicking (or double-clicking) the tray icon.
    pub fn on_left_click<F>(mut self, callback: F) -> Self
    where
        F: Fn() + Send + Sync + 'static,
    {
        self.on_left_click = Some(Arc::new(callback));
        self
    }

    /// Registers a callback triggered when right-clicking the tray icon.
    ///
    /// Note: This is mutually exclusive with custom menu items. If menu items or custom menus
    /// are configured, right-clicking will show the menu instead.
    pub fn on_right_click<F>(mut self, callback: F) -> Self
    where
        F: Fn() + Send + Sync + 'static,
    {
        self.on_right_click = Some(Arc::new(callback));
        self
    }

    /// Appends a standard native item to the tray context menu.
    pub fn with_menu_item(mut self, mut item: TrayMenuItem) -> Self {
        item.id = self.next_menu_id;
        self.next_menu_id += 1;

        self.menu_items.push(item);
        self
    }

    /// Registers a custom popup window to display when right-clicking the tray icon.
    pub fn with_custom_menu(mut self, custom_menu: CustomTrayMenu) -> Self {
        self.custom_menu = Some(custom_menu);
        self
    }

    /// Forces the tray native context menu to Windows 11's native dark mode.
    pub fn with_dark_mode_menus(mut self, enabled: bool) -> Self {
        self.dark_mode_menus = enabled;
        self
    }

    /// Wraps the current builder state into an [`Unvalidated`] wrapper.
    pub fn into_unvalidated(self) -> Unvalidated<Self> {
        Unvalidated::new(self)
    }
}

impl Validate for TrayBuilder {
    type Error = MichiuError;

    /// Validates tray configuration parameters.
    ///
    /// Checks:
    /// 1. A valid [`Icon`] must be provided.
    /// 2. Tooltip length must be under 128 UTF-16 code units.
    ///
    /// # Errors
    /// Returns [`MichiuError::ValidationError`] on failure.
    fn validate(self) -> Result<Self> {
        // アイコンの存在検証
        if self.icon.is_none() {
            return Err(MichiuError::ValidationError {
                parameter: "icon",
                message: "A valid system tray icon must be provided.".into(),
            });
        }

        // ツールチップの文字数制限（UTF-16換算で127文字以下）
        if let Some(ref tip) = self.tooltip {
            let utf16_len = tip.encode_utf16().count();
            if utf16_len >= 128 {
                return Err(MichiuError::ValidationError {
                    parameter: "tooltip",
                    message: format!(
                        "Tray tooltip exceeds the OS limit of 127 UTF-16 code units (current: {}).",
                        utf16_len
                    )
                    .into(),
                });
            }
        }

        Ok(self)
    }
}

fn register_tray_window_class(
    class_name: PCWSTR,
    class_name_str: &'static str,
    hinstance: HINSTANCE,
) -> Result<()> {
    let hcursor = unsafe { LoadCursorW(None, IDC_ARROW).map_err(MichiuError::UnexpectedOsError)? };

    let tray_wnd_class = WNDCLASSEXW {
        cbSize: std::mem::size_of::<WNDCLASSEXW>() as u32,
        style: CS_HREDRAW | CS_VREDRAW,
        lpfnWndProc: Some(tray_wnd_proc),
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

    let atom = unsafe { RegisterClassExW(&tray_wnd_class) };
    if atom == 0 {
        let err = windows::core::Error::from_thread();

        if err.code() != ERROR_CLASS_ALREADY_EXISTS.to_hresult() {
            return Err(MichiuError::ClassRegistrationFailed {
                class_name: class_name_str.into(),
                source: windows::core::Error::from_thread(),
            });
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests;
