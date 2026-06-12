use crate::{ComContext, LogicalPoint, LogicalSize, MessageFilter, Tray};
use crate::{
    Icon,
    error::{MichiuError, Result},
};
use michiu_guard::{Unvalidated, Validate};
use std::{borrow::Cow, sync::Arc};
use windows::Win32::{
    Foundation::{HWND, LPARAM, LRESULT, WPARAM},
    UI::WindowsAndMessaging::{
        WINDOW_EX_STYLE, WINDOW_STYLE, WS_CAPTION, WS_CHILD, WS_DLGFRAME, WS_EX_APPWINDOW,
        WS_EX_CONTEXTHELP, WS_EX_TOOLWINDOW, WS_EX_TOPMOST, WS_MAXIMIZEBOX, WS_MINIMIZEBOX,
        WS_OVERLAPPEDWINDOW, WS_POPUP, WS_SYSMENU,
    },
};

/// A builder helper used to configure the properties and styles of a Win32 window.
#[derive(Debug, Clone)]
pub struct WindowBuilder<'a> {
    pub(crate) title: Cow<'static, str>,
    pub(crate) icon: Option<Icon>,
    pub(crate) inner_size: Option<LogicalSize>,
    pub(crate) min_inner_size: Option<LogicalSize>,
    pub(crate) max_inner_size: Option<LogicalSize>,
    pub(crate) position: Option<LogicalPoint>,
    pub(crate) visible: bool,
    pub(crate) resizable: bool,
    pub(crate) decorations: bool,
    pub(crate) transparent: bool,
    pub(crate) maximized: bool,
    pub(crate) hittest: bool,
    pub(crate) overlapped_window: bool,
    pub(crate) taskbar_button: bool,
    pub(crate) no_redirection_bitmap: bool,
    pub(crate) com_context: Option<&'a ComContext>,
    pub(crate) tray: Option<Tray>,
    pub(crate) parent_hwnd: Option<HWND>,
    pub(crate) custom_class_name: Option<Cow<'static, str>>,
    pub(crate) raw_style: Option<WINDOW_STYLE>,
    pub(crate) raw_ex_style: Option<WINDOW_EX_STYLE>,
    pub(crate) message_filter: Option<MessageFilter>,
    pub(crate) auto_dpi_scaling: bool,
    pub(crate) dark_mode: bool,
    pub(crate) drag_and_drop: bool,
    pub(crate) ime_expose_port: Option<u16>,
}

impl<'a> Default for WindowBuilder<'a> {
    fn default() -> Self {
        Self {
            title: Cow::Borrowed(""),
            icon: None,
            inner_size: None,
            min_inner_size: None,
            max_inner_size: None,
            position: None,
            visible: true,
            resizable: true,
            decorations: true,
            transparent: false,
            maximized: false,
            hittest: true,
            overlapped_window: true,
            taskbar_button: true,
            no_redirection_bitmap: false,
            com_context: None,
            tray: None,
            parent_hwnd: None,
            custom_class_name: None,
            raw_style: None,
            raw_ex_style: None,
            message_filter: None,
            auto_dpi_scaling: true,
            dark_mode: false,
            drag_and_drop: false,
            ime_expose_port: None,
        }
    }
}

impl<'a> WindowBuilder<'a> {
    /// Creates a default configured `WindowBuilder` instance.
    pub fn new() -> Self {
        Self::default()
    }
}

impl<'a> WindowBuilder<'a> {
    /// Sets the window title.
    pub fn with_title(mut self, title: impl Into<Cow<'static, str>>) -> Self {
        self.title = title.into();
        self
    }

    pub fn with_icon(mut self, icon: Icon) -> Self {
        self.icon = Some(icon);
        self
    }

    /// Sets the client area size of the window.
    pub fn with_inner_size(mut self, size: LogicalSize) -> Self {
        self.inner_size = Some(size);
        self
    }

    /// Sets the minimum window size (in logical pixels).
    pub fn with_min_size(mut self, size: LogicalSize) -> Self {
        self.min_inner_size = Some(size);
        self
    }

    /// Limits the maximum size of the window (in logical pixels).
    pub fn with_max_size(mut self, size: LogicalSize) -> Self {
        self.max_inner_size = Some(size);
        self
    }

    /// Sets the initial screen position of the window.
    pub fn with_position(mut self, position: LogicalPoint) -> Self {
        self.position = Some(position);
        self
    }

    /// Sets whether the window is visible upon creation.
    pub fn with_visible(mut self, is_visible: bool) -> Self {
        self.visible = is_visible;
        self
    }

    /// Sets whether the window can be resized by the user.
    pub fn with_resizable(mut self, is_resizable: bool) -> Self {
        self.resizable = is_resizable;
        self
    }

    /// Sets whether the window should have standard OS window decorations (such as titlebar, borders).
    pub fn with_decorations(mut self, is_decorations: bool) -> Self {
        self.decorations = is_decorations;
        self
    }

    /// Sets whether the window background is transparent.
    pub fn with_transparent(mut self, is_transparent: bool) -> Self {
        self.transparent = is_transparent;
        self
    }

    /// Sets whether the window is maximized upon creation.
    pub fn with_maximized(mut self, is_maximized: bool) -> Self {
        self.maximized = is_maximized;
        self
    }

    /// Sets whether the window permits hit testing (mouse click intercepts).
    pub fn with_hittest(mut self, is_hittest: bool) -> Self {
        self.hittest = is_hittest;
        self
    }

    /// Sets whether the window is constructed as a standard overlapped window.
    pub fn with_overlapped_window(mut self, is_enabled: bool) -> Self {
        self.overlapped_window = is_enabled;
        self
    }

    /// Sets whether the window displays a button in the OS taskbar.
    pub fn with_taskbar_button(mut self, is_taskbar_button: bool) -> Self {
        self.taskbar_button = is_taskbar_button;
        self
    }

    /// Configures the window to request no redirection bitmap (required for DirectComposition rendering).
    pub fn with_no_redirection_bitmap(mut self, is_enabled: bool) -> Self {
        self.no_redirection_bitmap = is_enabled;
        self
    }

    /// Binds a valid [`ComContext`] reference to the window creation lifetime.
    pub fn with_com_context(mut self, com_context: &'a ComContext) -> Self {
        self.com_context = Some(com_context);
        self
    }

    /// Associates an optional system tray ([`Tray`]) control with the window.
    pub fn with_tray(mut self, tray: Tray) -> Self {
        self.tray = Some(tray);
        self
    }

    /// Configures the window as a child of another active window handle.
    pub fn with_child_of(mut self, parent_hwnd: HWND) -> Self {
        self.parent_hwnd = Some(parent_hwnd);

        // 内部フラグの整合性を保つ
        self.overlapped_window = false;
        self.taskbar_button = false;

        if let Some(mut raw_style) = self.raw_style {
            raw_style = (raw_style | WS_CHILD) & !WS_OVERLAPPEDWINDOW;
            self.raw_style = Some(raw_style);
        }
        if let Some(mut raw_ex_style) = self.raw_ex_style {
            raw_ex_style &= !WS_EX_APPWINDOW;
            self.raw_ex_style = Some(raw_ex_style);
        }
        self
    }

    /// Sets a custom window class name instead of the default framework-defined one.
    ///
    /// # Warning
    /// If utilizing a custom window class name via `custom_class_name`, please note that if its class styles include
    /// `CS_OWNDC` or `CS_CLASSDC`, you cannot use `WS_EX_COMPOSITED` (double buffering) or `WS_EX_LAYERED` (transparency)
    /// as doing so can trigger rendering artifacts.
    pub fn with_custom_class_name(mut self, class_name: impl Into<Cow<'static, str>>) -> Self {
        self.custom_class_name = Some(class_name.into());
        self
    }

    /// Force overwrites the window's basic Win32 styles (`WINDOW_STYLE`).
    pub fn with_raw_style(mut self, style: WINDOW_STYLE) -> Self {
        self.raw_style = Some(style);
        self
    }

    /// Force overwrites the window's extended Win32 styles (`WINDOW_EX_STYLE`).
    pub fn with_raw_ex_style(mut self, ex_style: WINDOW_EX_STYLE) -> Self {
        self.raw_ex_style = Some(ex_style);
        self
    }

    /// Registers a custom closure filter that executes inside the raw `WndProc` loop.
    pub fn with_message_filter<F>(mut self, filter: F) -> Self
    where
        F: Fn(HWND, u32, WPARAM, LPARAM) -> Option<LRESULT> + 'static,
    {
        self.message_filter = Some(MessageFilter(Arc::new(filter)));
        self
    }

    pub fn with_auto_dpi_scaling(mut self, is_enabled: bool) -> Self {
        self.auto_dpi_scaling = is_enabled;
        self
    }

    /// At startup, sets the entire window (including the title bar and context menus such as right-click menus)
    /// to the native dark mode of Windows 11.
    pub fn with_dark_mode(mut self, enabled: bool) -> Self {
        self.dark_mode = enabled;
        self
    }

    /// Enables OLE file Drag & Drop support for the window (IDropTarget).
    pub fn with_drag_and_drop(mut self, enabled: bool) -> Self {
        self.drag_and_drop = enabled;
        self
    }

    /// Activates the secure localhost OLE/TSF IME JSON Relay API on the specified TCP port.
    pub fn with_ime_expose_port(mut self, port: u16) -> Self {
        self.ime_expose_port = Some(port);
        self
    }

    /// Wraps the current builder state into an [`Unvalidated`] handle ready for validation.
    pub fn into_unvalidated(self) -> Unvalidated<Self> {
        Unvalidated::new(self)
    }
}

impl<'a> Validate for WindowBuilder<'a> {
    type Error = MichiuError;

    fn validate(self) -> Result<Self> {
        if self.no_redirection_bitmap && self.com_context.is_none() {
            return Err(MichiuError::ValidationError {
                parameter: "no_redirection_bitmap",
                message: "DirectComposition (no_redirection_bitmap) requires a valid ComContext."
                    .into(),
            });
        }
        if self.drag_and_drop {
            if let Some(ctx) = self.com_context {
                // コンテキストが OLE STA でない場合は、Win32の E_OUTOFMEMORY 罠を避けるために早期エラーにする
                if !ctx.is_ole() {
                    return Err(MichiuError::ValidationError {
                                parameter: "drag_and_drop",
                                message: "OLE Drag and Drop requires an OLE-initialized ComContext (use ComContext::new_com_single()). \
                                          MTA or WinRT contexts are not compatible on this thread.".into(),
                            });
                }
            } else {
                // コンテキスト自体が渡されていない場合
                return Err(MichiuError::ValidationError {
                            parameter: "drag_and_drop",
                            message: "Drag and Drop (drag_and_drop) requires a valid OLE-initialized ComContext (use ComContext::new_com_single()).".into(),
                        });
            }
        }
        if self.no_redirection_bitmap && self.decorations {
            return Err(MichiuError::ValidationError {
                parameter: "decorations",
                message: "Windows with no_redirection_bitmap enabled cannot have standard OS decorations. Set decorations to false.".into(),
            });
        }

        // Check for inconsistencies between transparent windows and standard decorations.
        if self.transparent && self.decorations {
            return Err(MichiuError::ValidationError {
                parameter: "decorations",
                message: "Transparent windows cannot have standard OS decorations (titlebar/borders) due to rendering artifacts.".into(),
            });
        }

        // Check for conflicts between child windows and taskbar buttons.
        let is_child =
            self.parent_hwnd.is_some() || self.raw_style.is_some_and(|s| s.contains(WS_CHILD));
        if is_child && self.taskbar_button {
            return Err(MichiuError::ValidationError {
                parameter: "taskbar_button",
                message: "Child windows cannot have a taskbar button (WS_EX_APPWINDOW).".into(),
            });
        }

        let has_top = self
            .raw_ex_style
            .is_some_and(|ex| ex.contains(WS_EX_TOPMOST));
        if is_child && has_top {
            return Err(MichiuError::ValidationError {
                parameter: "raw_ex_style",
                message: "A child window (WS_CHILD) cannot have the WS_EX_TOPMOST style.".into(),
            });
        }

        // Validate raw styles for mutual exclusion and dependency constraints.
        if let Some(raw) = self.raw_style {
            // Validate mutual exclusion of WS_CHILD and WS_POPUP.
            if raw.contains(WS_CHILD) && raw.contains(WS_POPUP) {
                return Err(MichiuError::ValidationError {
                    parameter: "raw_style",
                    message: "WS_CHILD and WS_POPUP are mutually exclusive styles.".into(),
                });
            }

            // Validate if a valid parent window is provided for WS_CHILD.
            if raw.contains(WS_CHILD) && self.parent_hwnd.is_none() {
                return Err(MichiuError::ValidationError {
                    parameter: "parent_hwnd",
                    message: "A child window (WS_CHILD) must have a valid parent window handle."
                        .into(),
                });
            }

            // Validate mutual exclusion of WS_CAPTION and WS_DLGFRAME.
            if raw.contains(WS_CAPTION) && raw.contains(WS_DLGFRAME) {
                return Err(MichiuError::ValidationError {
                    parameter: "raw_style",
                    message: "WS_CAPTION and WS_DLGFRAME are mutually exclusive styles.".into(),
                });
            }

            let has_box = raw.contains(WS_MINIMIZEBOX) || raw.contains(WS_MAXIMIZEBOX);
            if has_box && !raw.contains(WS_SYSMENU) {
                return Err(MichiuError::ValidationError {
                    parameter: "raw_style",
                    message: "WS_MINIMIZEBOX or WS_MAXIMIZEBOX requires WS_SYSMENU to be enabled to be visible.".into(),
                });
            }

            // Validate conflicts between WS_EX_CONTEXTHELP and minimize/maximize buttons.
            if let Some(raw_ex) = self.raw_ex_style
                && raw_ex.contains(WS_EX_CONTEXTHELP)
                && (raw.contains(WS_MAXIMIZEBOX) || raw.contains(WS_MINIMIZEBOX))
            {
                return Err(MichiuError::ValidationError {
                        parameter: "raw_ex_style",
                        message: "WS_EX_CONTEXTHELP cannot be combined with WS_MAXIMIZEBOX or WS_MINIMIZEBOX.".into(),
                    });
            }
        }

        if let Some(raw_ex) = self.raw_ex_style
            && raw_ex.contains(WS_EX_TOOLWINDOW)
            && raw_ex.contains(WS_EX_APPWINDOW)
        {
            return Err(MichiuError::ValidationError {
                parameter: "raw_ex_style",
                message:
                    "WS_EX_TOOLWINDOW and WS_EX_APPWINDOW are contradictory and cannot be combined."
                        .into(),
            });
        }

        Ok(self)
    }
}

#[cfg(test)]
mod tests;
