use std::{borrow::Cow, fmt};

use thiserror::Error;
use windows::Win32::Foundation::HWND;

/// The unified error type returned by all windowing, tray, and input method subsystems.
///
/// Under the hood, this integrates with [`thiserror`] to provide localized system context.
/// It encapsulates raw Win32 system errors ([`windows::core::Error`]) where applicable,
/// and provides helpful developer diagnostics and remedy advice.
///
/// # Error Handling Policy
///
/// `michiu_window` aims to provide "Context-Aware" errors. Instead of just giving you
/// a raw OS error code, we wrap it with information about what was happening.
///
/// ## Common Error Types:
///
/// - `WindowCreationFailed`: You probably passed an invalid parameter or the class wasn't registered.
/// - `ThreadMismatch`: Windows GUI objects are thread-affine. You cannot call window methods
///   from a thread other than the one that created it.
/// - `InvalidHandleState`: The window handle is no longer valid, has been recycled, or was already closed.
///
/// ## Inspecting OS Errors:
///
/// Most variants contain a `source` field which is a `windows::core::Error`.
/// You can extract the `HRESULT` or the localized system message from it.
#[derive(Error, Debug)]
pub enum MichiuError {
    /// Failed to register a window class via `RegisterClassExW`.
    #[error("Failed to register window class '{class_name}'. System error: {source}")]
    ClassRegistrationFailed {
        class_name: Cow<'static, str>,
        #[source]
        source: windows::core::Error,
    },

    /// Failed to create a window via `CreateWindowExW`.
    #[error("Failed to create window '{title}' (class: '{class_name}'). System error: {source}")]
    WindowCreationFailed {
        title: Cow<'static, str>,
        class_name: Cow<'static, str>,
        #[source]
        source: windows::core::Error,
    },

    /// Failed to initialize COM or Windows Runtime (WinRT) threading contexts.
    #[error("Failed to initialize COM/WinRT context ({context_type:?}). System error: {source}")]
    ComInitializationFailed {
        context_type: &'static str,
        #[source]
        source: windows::core::Error,
    },

    /// Failed to create the dummy hidden window used internally to process system tray events.
    #[error("Failed to create the system tray dummy window. System error: {source}")]
    TrayWindowCreationFailed {
        #[source]
        source: windows::core::Error,
    },

    /// Failed to register or alter the system tray icon via `Shell_NotifyIconW`.
    #[error("Failed to register or update system tray icon. System error: {source}")]
    TrayIconOperationFailed {
        #[source]
        source: windows::core::Error,
    },

    /// Failed to create a popup menu for the system tray via `CreatePopupMenu`.
    #[error("Failed to create popup menu for system tray. System error: {source}")]
    PopupMenuCreationFailed {
        #[source]
        source: windows::core::Error,
    },

    /// Failed to update the window title.
    #[error("Failed to set window title to '{attempted_title}'. System error: {source}")]
    SetTitleFailed {
        attempted_title: Cow<'static, str>,
        #[source]
        source: windows::core::Error,
    },

    /// Failed to resize, move, or change window bounds.
    #[error(
        "Failed to alter window geometry (x: {x}, y: {y}, width: {width}, height: {height}, dpi: {dpi}). System error: {source}"
    )]
    GeometryUpdateFailed {
        x: i32,
        y: i32,
        width: u32,
        height: u32,
        dpi: u32,
        #[source]
        source: windows::core::Error,
    },

    /// Validation on the window handle (`HWND`) has failed (e.g., closed on OS side, recycled).
    #[error("Window handle validation failed: {reason}")]
    InvalidHandleState { reason: &'static str },

    /// Attempted to access a window from a thread other than the one that created it.
    /// Windows UI elements are thread-affine and cannot be manipulated from foreign threads.
    #[error(
        "Thread mismatch: Window must be accessed from thread {expected}, but was accessed from {actual}."
    )]
    ThreadMismatch { expected: u32, actual: u32 },

    /// A builder parameter or a property value failed validation.
    #[error("Validation failed for {parameter}: {message}")]
    ValidationError {
        parameter: &'static str,
        message: Cow<'static, str>,
    },

    /// Failed to load an external system resource, icon, or cursor.
    #[error("Failed to load resource '{path}'. System message: {source}")]
    ResourceLoadFailed {
        path: Cow<'static, str>,
        source: windows::core::Error,
    },

    /// Failed to install a window subclass via `SetWindowSubclass`.
    #[error(
        "Failed to set window subclass (ID: {subclass_id}) on HWND {hwnd:?}. System error: {source}"
    )]
    SubclassSetupFailed {
        hwnd: HWND,
        subclass_id: usize,
        #[source]
        source: windows::core::Error,
    },

    /// Failed to acquire the IME context (HIMC) for the window.
    #[error("Failed to acquire IME context (HIMC) for HWND {hwnd:?}.")]
    ImeContextAcquisitionFailed { hwnd: HWND },

    /// Failed to query or decode the IME composition or result string.
    #[error("Failed to query IME string (buffer index: {index}). System error: {source}")]
    ImeStringQueryFailed {
        index: u32,
        #[source]
        source: windows::core::Error,
    },

    /// A generic fallback for other unexpected Windows OS errors.
    #[error("Unexpected OS error (HRESULT: {0:?})")]
    UnexpectedOsError(#[from] windows::core::Error),
}

unsafe impl Send for MichiuError {}
unsafe impl Sync for MichiuError {}

/// A specialized Type Alias representing the result of any `michiu_window` operation.
pub type Result<T> = std::result::Result<T, MichiuError>;

/// A wrapper format designed to print a rich, easy-to-read troubleshooting report.
///
/// When printed using standard `{}` formatting, it prints the base error message,
/// followed by raw OS HRESULT codes (with common explanations) and helpful remedy advice.
pub struct RichReport<'a>(&'a MichiuError);

impl<'a> fmt::Display for RichReport<'a> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let err = self.0;

        // 基本エラーメッセージ
        write!(f, "Error: {}", err)?;

        // OSのエラー情報を委譲 (SysErrorInfo の Display を呼び出す)
        if let Some(sys_info) = err.sys_error_info() {
            write!(f, "\n\n[OS Error Details]:\n{}", sys_info)?;
        }

        // 対処法の追加
        if let Some(remedy_text) = err.remedy() {
            write!(f, "\n\n[How to Fix / Remedy]:\n{}", remedy_text)?;
        }

        Ok(())
    }
}

/// Parsed OS error information bundled for easy inspection.
#[derive(Debug, Clone)]
pub struct SysErrorInfo {
    /// The raw HRESULT code returned by the OS API.
    pub hresult: windows::core::HRESULT,
    /// The localized system error message retrieved from the OS.
    pub message: String,
}

impl fmt::Display for SysErrorInfo {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "Code: {} ({})\nMessage: {}",
            self.hex_code(),
            self.hresult.0,
            self.message
        )?;

        if let Some(explanation) = self.common_explanation() {
            write!(f, "\n\n[Error Explanation]:\n{}", explanation)?;
        }
        Ok(())
    }
}

impl MichiuError {
    /// Wraps this error in a helper type designed to render a beautiful diagnostic report.
    #[inline]
    pub fn report(&self) -> RichReport<'_> {
        RichReport(self)
    }
    /// Returns explicit, practical remedy advice (troubleshooting hints) to resolve this error.
    /// Returns `None` if no specific remedy is defined.
    pub fn remedy(&self) -> Option<&'static str> {
        match self {
            Self::WindowCreationFailed { class_name, .. } => {
                if class_name.starts_with("MichiuWindowClass_") {
                    Some(
                        "This is an internal framework error. Please ensure your Windows OS supports basic Win32 window creation.",
                    )
                } else {
                    Some(
                        "Ensure that the custom class name you provided has been registered properly before calling build().",
                    )
                }
            }

            Self::ClassRegistrationFailed { .. } => Some(
                "Verify that the window class name is unique and not already registered by another part of the application. \
                Also check if the provided WNDCLASSEXW structure parameters are valid.",
            ),

            Self::ThreadMismatch { .. } => Some(
                "Windows GUI elements are thread-affine. You must interact with the window \
                only from the thread that created it (typically the thread that runs the main message loop).",
            ),

            Self::ValidationError { parameter, .. } => match *parameter {
                "no_redirection_bitmap" => Some(
                    "DirectComposition (no_redirection_bitmap) requires a valid ComContext. \
                        Please initialize a ComContext and pass it to your WindowBuilder.",
                ),
                "transparent" => Some(
                    "Transparent windows cannot have standard OS decorations. \
                        Please call `with_decorations(false)` on your WindowBuilder.",
                ),
                _ => Some(
                    "Review the WindowBuilder parameters to ensure they satisfy the window specifications.",
                ),
            },

            Self::ComInitializationFailed { context_type, .. } => {
                if *context_type == "COM Single" || *context_type == "COM Multi" {
                    Some(
                        "Ensure you are not trying to initialize COM with a different threading model (Apartment vs Multi) on a thread that already has COM initialized.",
                    )
                } else {
                    Some(
                        "Ensure the Windows Runtime (WinRT) is supported on this system and that you haven't already initialized it with a different threading model.",
                    )
                }
            }

            Self::TrayWindowCreationFailed { .. } => Some(
                "Verify that your system has not reached the limit for window handles (HWNDs). \
                Also, ensure that your window class registration was successful before creating the tray.",
            ),

            Self::TrayIconOperationFailed { .. } => Some(
                "Ensure that the window handle (HWND) associated with the tray is valid and active. \
                In some environments, shell-related functions might fail temporarily if the explorer.exe process is restarting.",
            ),

            Self::InvalidHandleState { .. } => Some(
                "Make sure you do not call window manipulation methods after the window receives WM_CLOSE/WM_DESTROY \
                or after the Window object goes out of scope.",
            ),

            Self::ImeContextAcquisitionFailed { .. } => Some(
                "This usually occurs because the window handle (HWND) is invalid/already destroyed, \
                            or you are trying to access the IME from a foreign thread. Ensure you call this from the UI thread.",
            ),

            Self::ImeStringQueryFailed { .. } => Some(
                "Verify that the IME context remains valid and has not been closed by the OS \
                            or the user switching input methods during composition. Also ensure the thread has OLE initialized.",
            ),

            _ => None,
        }
    }

    /// Extracts the raw Windows OS error details (HRESULT and system message).
    /// Returns `None` if the error variant is not derived from an underlying OS error (such as `ThreadMismatch`).
    pub fn sys_error_info(&self) -> Option<SysErrorInfo> {
        let win_err = match self {
            Self::WindowCreationFailed { source, .. } => Some(source),
            Self::SetTitleFailed { source, .. } => Some(source),
            Self::GeometryUpdateFailed { source, .. } => Some(source),
            Self::ResourceLoadFailed { source, .. } => Some(source),
            Self::ClassRegistrationFailed { source, .. } => Some(source),
            Self::SubclassSetupFailed { source, .. } => Some(source),
            Self::TrayWindowCreationFailed { source, .. } => Some(source),
            Self::TrayIconOperationFailed { source, .. } => Some(source),
            Self::ComInitializationFailed { source, .. } => Some(source),
            Self::PopupMenuCreationFailed { source } => Some(source),
            Self::UnexpectedOsError(source) => Some(source),
            Self::ImeStringQueryFailed { source, .. } => Some(source),
            _ => None,
        };

        win_err.map(|err| SysErrorInfo {
            hresult: err.code(),
            message: err.message().to_string(),
        })
    }
}

impl SysErrorInfo {
    /// Formats the HRESULT error code as a standard hexadecimal string (e.g., `"0x80070005"`).
    pub fn hex_code(&self) -> String {
        format!("0x{:08X}", self.hresult.0 as u32)
    }

    /// Returns a clear, human-readable English explanation for common Win32 HRESULT codes (e.g., E_ACCESSDENIED).
    /// Returns `None` if the error code is unrecognized.
    pub fn common_explanation(&self) -> Option<&'static str> {
        match self.hresult.0 as u32 {
            0x80070005 => Some(
                "E_ACCESSDENIED: Access is denied. The operation might require administrator privileges, \
                or the target resource might be locked by another process.",
            ),
            0x80070057 => Some(
                "E_INVALIDARG: One or more arguments are invalid. An invalid argument, struct member, \
                or incompatible combination of styles was passed to a Win32 API.",
            ),
            0x80004005 => Some(
                "E_FAIL: Unspecified failure. An unexpected failure occurred internally within the OS, \
                but no detailed reason was reported.",
            ),
            0x8007000E => Some(
                "E_OUTOFMEMORY: Out of memory. System resources are depleted, or an excessively large \
                memory allocation request occurred.",
            ),
            0x800401F0 => Some(
                "CO_E_NOTINITIALIZED: CoInitialize has not been called. The COM library has not been \
                initialized on this thread. Please construct a ComContext to initialize COM before calling COM functions.",
            ),
            0x800401F1 => Some("CO_E_ALREADYINITIALIZED: The COM library is already initialized."),
            0x80070578 => Some(
                "ERROR_INVALID_WINDOW_HANDLE: Invalid window handle (HWND). The target window has already been destroyed, \
                or it was not created successfully.",
            ),
            0x80040154 => Some(
                "REGDB_E_CLASSNOTREG: Class not registered. The COM component you attempted to call is not registered \
                on the system, or there is a target platform mismatch (32-bit vs 64-bit build configuration).",
            ),
            0x80070002 => Some(
                "ERROR_FILE_NOT_FOUND: The system cannot find the file or resource specified. Please check that the path \
                or resource identifier is correct.",
            ),
            _ => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use windows::core::{Error as WinError, HRESULT};

    #[test]
    fn test_error_basic_display_and_debug() {
        // テスト対象のエラーを生成
        let raw_os_error = WinError::new(HRESULT(0x80070005u32 as i32), "Access is denied.");
        let err = MichiuError::WindowCreationFailed {
            title: Cow::Borrowed("MainWindow"),
            class_name: Cow::Borrowed("MichiuWindowClass_0.1.0"),
            source: raw_os_error,
        };

        // 標準の Display 形式 ({}) に変換
        let display_string = err.to_string();
        let debug_string = format!("{:?}", err);

        // `cargo test -- --nocapture` を指定して実行したときのみ出力。
        println!("\n=== [DEBUG] Basic Display ({{}}) ===");
        println!("{}", display_string);

        println!("\n=== [DEBUG] Basic Debug ({{:?}}) ===");
        println!("{}", debug_string);
        println!("====================================\n");

        // standard Display に 期待されるプレフィックスと OSエラー情報が含まれているか
        assert!(display_string.contains("Failed to create window 'MainWindow'"));
        assert!(display_string.contains("System error: Access is denied."));

        // Debug に 構造体の名前やフィールド情報が正しくシリアライズされているか
        assert!(debug_string.contains("WindowCreationFailed"));
        assert!(debug_string.contains("title: \"MainWindow\""));
    }

    #[test]
    fn test_error_rich_report_rendering() {
        // windows-rs の生のエラーを擬似的に生成
        // (0x80070005 = E_ACCESSDENIED)
        let raw_os_error = WinError::new(HRESULT(0x80070005u32 as i32), "Access is denied.");

        let err = MichiuError::WindowCreationFailed {
            title: Cow::Borrowed("MainWindow"),
            class_name: Cow::Borrowed("MichiuWindowClass_0.1.0"),
            source: raw_os_error,
        };

        // レポートを生成
        let report = err.report();
        let report_string = report.to_string();

        // `cargo test -- --nocapture` を指定して実行したときのみ出力
        println!("\n=== [DEBUG] Rendered RichReport ===");
        println!("{}", report_string);
        println!("===================================\n");

        // エラー内容、OSエラー詳細、対処法のすべてが文字列に含まれているか確認
        assert!(report_string.contains("Failed to create window 'MainWindow'"));
        assert!(report_string.contains("Access is denied."));
        assert!(report_string.contains("[OS Error Details]"));
        assert!(report_string.contains("0x80070005"));
        assert!(report_string.contains("[How to Fix / Remedy]"));
        assert!(
            report_string.
                contains("This is an internal framework error. Please ensure your Windows OS supports basic Win32 window creation.")
        );
    }

    #[test]
    fn test_error_remedy_hints() {
        // バリデーションエラーにおける remedy の分岐を検証
        let err_transparent = MichiuError::ValidationError {
            parameter: "transparent",
            message: Cow::Borrowed("Contradicting decorations"),
        };

        let remedy = err_transparent.remedy();
        assert!(remedy.is_some());
        assert!(
            remedy
                .unwrap()
                .contains("Transparent windows cannot have standard OS decorations.")
        );

        // 未知のパラメータに対するデフォルトの remedy
        let err_unknown = MichiuError::ValidationError {
            parameter: "unknown_param",
            message: Cow::Borrowed("generic"),
        };
        assert!(
            err_unknown
                .remedy()
                .unwrap()
                .contains("Review the WindowBuilder parameters")
        );
    }

    #[test]
    fn test_sys_error_info_parsing() {
        // CO_E_NOTINITIALIZED (0x800401F0) を生成
        let win_err = WinError::new(
            HRESULT(0x800401F0u32 as i32),
            "CoInitialize has not been called.",
        );
        let err = MichiuError::UnexpectedOsError(win_err);

        let sys_info = err.sys_error_info();
        assert!(sys_info.is_some());

        let info = sys_info.unwrap();
        assert_eq!(info.hex_code(), "0x800401F0");

        // HRESULTに対する定義済み英語解説が引けるか検証
        let explanation = info.common_explanation();
        assert!(explanation.is_some());
        assert!(
            explanation
                .unwrap()
                .contains("CO_E_NOTINITIALIZED: CoInitialize has not been called.")
        );
    }
}
