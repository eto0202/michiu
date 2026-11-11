#![expect(clippy::ref_as_ptr, clippy::inline_always)]

use crate::{
    MichiuAnyEvent, MichiuError, MichiuEvent, MichiuRawEvent, WindowId, dispatch_to_active_handler,
};
use michiu_guard::Unvalidated;
use std::path::PathBuf;
use windows::Win32::{
    Foundation::{ERROR_CANCELLED, HWND, LPARAM, POINTL, WPARAM},
    System::{
        Com::{
            CLSCTX_ALL, CoCreateInstance, DVASPECT_CONTENT, FORMATETC, IDataObject, TYMED_HGLOBAL,
        },
        Ole::{
            CF_HDROP, DROPEFFECT, DROPEFFECT_COPY, DROPEFFECT_NONE, IDropTarget, IDropTarget_Impl,
            ReleaseStgMedium,
        },
        SystemServices::MODIFIERKEYS_FLAGS,
    },
    UI::Shell::{
        Common::COMDLG_FILTERSPEC, DragQueryFileW, FOS_FORCEFILESYSTEM, FOS_OVERWRITEPROMPT,
        FileOpenDialog, FileSaveDialog, HDROP, IFileOpenDialog, IFileSaveDialog, SIGDN_FILESYSPATH,
    },
};
use windows::core::{Ref, implement};
use windows_core::{HRESULT, HSTRING, PCWSTR};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum FileDialog {
    /// Set the calling window as the parent.
    #[default]
    ThisWindow,
    /// Open independently without a parent window.
    None,
}

impl FileDialog {
    /// Shows a file open dialog and returns the selected file path. If the user cancels, `None` is returned.
    ///
    /// # Errors
    ///
    /// Returns [`MichiuError`]:
    /// - If `CoInitializeEx` has not been called on the calling thread (`CO_E_NOTINITIALIZED`).
    /// - If the operating system fails to instantiate or display the dialog.
    /// - If the selected file path contains invalid characters and cannot be converted.
    #[inline]
    pub fn open_file_dialog(hwnd: Option<HWND>) -> crate::Result<Option<PathBuf>> {
        unsafe {
            let dialog: IFileOpenDialog = CoCreateInstance(&FileOpenDialog, None, CLSCTX_ALL)?;

            if let Err(e) = dialog.Show(hwnd) {
                // キャンセルボタンなどで閉じられた場合は正常系として None を返す
                if e.code() == HRESULT::from_win32(ERROR_CANCELLED.0) {
                    return Ok(None);
                }
                return Err(e.into());
            }

            let item = dialog.GetResult()?;
            let path = item.GetDisplayName(SIGDN_FILESYSPATH)?;

            let path_str = path
                .to_string()
                .map_err(|e| MichiuError::UnexpectedOsError(e.into()))?;

            Ok(Some(PathBuf::from(path_str)))
        }
    }

    /// Shows a file save dialog and returns the save file path. If the user cancels, `None` is returned.
    ///
    /// # Errors
    ///
    /// Returns [`MichiuError`]:
    /// - If `CoInitializeEx` has not been called on the calling thread (`CO_E_NOTINITIALIZED`).
    /// - If the specified filter string (`FileFilter`) is empty or malformed (`E_INVALIDARG`).
    /// - If `IFileSaveDialog` creation fails due to insufficient memory or system limitations.
    /// - If the retrieved file path could not be converted to a valid UTF-8 string.
    #[inline]
    pub fn save_file_dialog(
        hwnd: Option<HWND>,
        options: &FileDialogOptions,
    ) -> crate::Result<Option<PathBuf>> {
        unsafe {
            let dialog: IFileSaveDialog = CoCreateInstance(&FileSaveDialog, None, CLSCTX_ALL)?;

            let mut flags = dialog.GetOptions()? | FOS_FORCEFILESYSTEM;
            if options.overwrite_prompt {
                flags |= FOS_OVERWRITEPROMPT;
            } else {
                flags &= !FOS_OVERWRITEPROMPT;
            }
            dialog.SetOptions(flags)?;

            let title_hstring = options.title.as_ref().map(HSTRING::from);
            if let Some(ref title) = title_hstring {
                dialog.SetTitle(title)?;
            }

            let file_name_hstring = options.default_file_name.as_ref().map(HSTRING::from);
            if let Some(ref name) = file_name_hstring {
                dialog.SetFileName(name)?;
            }

            let ext_hstring = options.default_extension.as_ref().map(HSTRING::from);
            if let Some(ref ext) = ext_hstring {
                dialog.SetDefaultExtension(ext)?;
            }

            // COMDLG_FILTERSPEC に渡すポインタが解放されないよう、HSTRING の実体を一度 Vec に集めてからポインタを渡す
            let filter_hstrings: Vec<(HSTRING, HSTRING)> = options
                .filters
                .iter()
                .map(|f| (HSTRING::from(&f.name), HSTRING::from(&f.spec)))
                .collect();

            let filter_specs: Vec<COMDLG_FILTERSPEC> = filter_hstrings
                .iter()
                .map(|(name, spec)| COMDLG_FILTERSPEC {
                    pszName: PCWSTR(name.as_ptr()),
                    pszSpec: PCWSTR(spec.as_ptr()),
                })
                .collect();

            if !filter_specs.is_empty() {
                dialog.SetFileTypes(&filter_specs)?;
            }

            if let Err(e) = dialog.Show(hwnd) {
                if e.code() == HRESULT::from_win32(ERROR_CANCELLED.0) {
                    return Ok(None);
                }
                return Err(e.into());
            }

            let item = dialog.GetResult()?;
            let path = item.GetDisplayName(SIGDN_FILESYSPATH)?;

            let path_str = path
                .to_string()
                .map_err(|e| MichiuError::UnexpectedOsError(e.into()))?;

            Ok(Some(PathBuf::from(path_str)))
        }
    }
}

/// Defining File Types (Filters)
#[derive(Debug, Clone)]
pub struct FileFilter {
    /// Display name (e.g., `"Text file (*.txt)"`)
    pub name: String,
    /// File extension patterns (e.g., `"*.txt"` or `"*.png;*.jpg"`)
    pub spec: String,
}

impl FileFilter {
    pub fn new(name: impl Into<String>, spec: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            spec: spec.into(),
        }
    }
}

/// Configuration options for the file save dialog.
///
/// # Examples
///
/// ```no_run
/// # use michiu_window::FileDialogOptions;
/// let options = FileDialogOptions::new()
///     .title("Select Output File")
///     .default_file_name("document.txt")
///     .default_extension("txt")
///     .overwrite_prompt(true)
///     .add_filter("Text Files (*.txt)", "*.txt")
///     .add_filter("Markdown Files (*.md)", "*.md")
///     .add_filter("All Files (*.*)", "*.*");
/// ```
#[derive(Debug, Clone)]
pub struct FileDialogOptions {
    /// Window title (if not specified, the Windows default)
    pub title: Option<String>,
    /// The file name displayed initially (e.g., `"untitled.txt"`)
    pub default_file_name: Option<String>,
    /// The file extension automatically added when the extension is omitted (without a period; e.g., `"txt"`)
    pub default_extension: Option<String>,
    /// Whether to display a warning pop-up when saving over existing files (default: `true`)
    pub overwrite_prompt: bool,
    /// File Type (Drop-down Options)
    pub filters: Vec<FileFilter>,
}

impl Default for FileDialogOptions {
    fn default() -> Self {
        Self {
            title: None,
            default_file_name: None,
            default_extension: None,
            overwrite_prompt: true,
            filters: Vec::new(),
        }
    }
}

impl FileDialogOptions {
    #[inline]
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    #[inline]
    #[must_use]
    pub fn title(mut self, title: impl Into<String>) -> Self {
        self.title = Some(title.into());
        self
    }

    #[inline]
    #[must_use]
    pub fn default_file_name(mut self, name: impl Into<String>) -> Self {
        self.default_file_name = Some(name.into());
        self
    }

    #[inline]
    #[must_use]
    pub fn default_extension(mut self, ext: impl Into<String>) -> Self {
        self.default_extension = Some(ext.into());
        self
    }

    #[inline]
    #[must_use]
    pub fn overwrite_prompt(mut self, enable: bool) -> Self {
        self.overwrite_prompt = enable;
        self
    }

    #[inline]
    #[must_use]
    pub fn add_filter(mut self, name: impl Into<String>, spec: impl Into<String>) -> Self {
        self.filters.push(FileFilter::new(name, spec));
        self
    }
}

/// A helper struct used to implement OLE File Drag and Drop functionality for a window.
///
/// This implements the raw Win32 COM `IDropTarget` interface. It processes incoming file drag-and-drop
/// operations (verifying `CF_HDROP` data format), extracts dropped file paths, and automatically
/// posts a [`crate::events::MichiuEvent::FileDropped`] event containing an [`Unvalidated<Vec<PathBuf>>`] payload
/// to the event queue.
///
/// # Examples
///
/// This struct can also be used manually in integration tests to emulate drag-and-drop actions on a window:
///
/// ```no_run
/// # use michiu_window::{MichiuWindow, MichiuWindowBuilder, FileDropTarget, MichiuComContext};
/// # use windows::Win32::System::Ole::IDropTarget;
/// # fn main() -> Result<(), Box<dyn std::error::Error>> {
/// # let com_ctx = MichiuComContext::new_com_single()?;
/// # let window = MichiuWindow::build(MichiuWindowBuilder::new().with_com_context(&com_ctx).into_unvalidated().try_into()?)?;
/// let drop_target_impl = FileDropTarget::new(window.hwnd());
/// // Convert to raw COM IDropTarget interface for invocation
/// let drop_target: IDropTarget = drop_target_impl.into();
/// # Ok(())
/// # }
/// ```
#[implement(IDropTarget)]
pub struct FileDropTarget {
    hwnd: HWND,
}

impl FileDropTarget {
    /// Creates a new `FileDropTarget` helper instance bound to the specified window handle.
    ///
    /// # Examples
    ///
    /// ```no_run
    /// # use michiu_window::{MichiuWindow, MichiuWindowBuilder, FileDropTarget, MichiuComContext};
    /// # fn main() -> Result<(), Box<dyn std::error::Error>> {
    /// # let com_ctx = MichiuComContext::new_com_single()?;
    /// # let window = MichiuWindow::build(MichiuWindowBuilder::new().with_com_context(&com_ctx).into_unvalidated().try_into()?)?;
    /// // Create a drop target bound to the window's HWND
    /// let drop_target = FileDropTarget::new(window.hwnd());
    /// # Ok(())
    /// # }
    /// ```
    #[must_use]
    #[inline]
    pub fn new(hwnd: HWND) -> Self {
        Self { hwnd }
    }

    /// Retrieves the raw window handle (`HWND`) bound to this drop target.
    ///
    /// # Examples
    ///
    /// ```no_run
    /// # use michiu_window::{FileDropTarget};
    /// # use windows::Win32::Foundation::HWND;
    /// # let drop_target = FileDropTarget::new(HWND(std::ptr::null_mut()));
    /// let target_hwnd = drop_target.hwnd();
    /// assert!(target_hwnd.is_invalid());
    /// ```
    #[must_use]
    #[inline]
    pub fn hwnd(&self) -> HWND {
        self.hwnd
    }
}

#[expect(clippy::not_unsafe_ptr_arg_deref)]
impl IDropTarget_Impl for FileDropTarget_Impl {
    fn DragEnter(
        &self,
        object: Ref<IDataObject>,
        _: MODIFIERKEYS_FLAGS,
        _: &POINTL,
        effect: *mut DROPEFFECT,
    ) -> windows::core::Result<()> {
        unsafe {
            if let Ok(data_obj) = object.ok() {
                let format_etc = FORMATETC {
                    cfFormat: CF_HDROP.0,
                    ptd: std::ptr::null_mut(),
                    dwAspect: DVASPECT_CONTENT.0,
                    lindex: -1,
                    tymed: TYMED_HGLOBAL.0 as u32,
                };

                if data_obj.QueryGetData(&raw const format_etc).is_ok() {
                    *effect = DROPEFFECT_COPY;
                } else {
                    *effect = DROPEFFECT_NONE;
                }
            }
        }
        Ok(())
    }

    fn DragOver(
        &self,
        _: MODIFIERKEYS_FLAGS,
        _: &POINTL,
        effect: *mut DROPEFFECT,
    ) -> windows::core::Result<()> {
        unsafe {
            *effect = DROPEFFECT_COPY;
        }
        Ok(())
    }

    fn DragLeave(&self) -> windows::core::Result<()> {
        Ok(())
    }

    fn Drop(
        &self,
        object: Ref<IDataObject>,
        _: MODIFIERKEYS_FLAGS,
        _: &POINTL,
        effect: *mut DROPEFFECT,
    ) -> windows::core::Result<()> {
        unsafe {
            *effect = DROPEFFECT_NONE;

            if let Ok(data_obj) = object.ok() {
                let format_etc = FORMATETC {
                    cfFormat: CF_HDROP.0,
                    ptd: std::ptr::null_mut(),
                    dwAspect: DVASPECT_CONTENT.0,
                    lindex: -1,
                    tymed: TYMED_HGLOBAL.0 as u32,
                };

                // IDataObject から HDROP データを抽出
                if let Ok(mut medium) = data_obj.GetData(&raw const format_etc) {
                    let hdrop = HDROP(medium.u.hGlobal.0.cast());

                    let file_count = DragQueryFileW(hdrop, 0xFFFF_FFFF, None);
                    let mut files = Vec::with_capacity(file_count as usize);

                    for i in 0..file_count {
                        let len = DragQueryFileW(hdrop, i, None);
                        let mut buf = vec![0u16; (len + 1) as usize];
                        DragQueryFileW(hdrop, i, Some(&mut buf));

                        if let Ok(path_str) = String::from_utf16(&buf[..len as usize]) {
                            files.push(PathBuf::from(path_str));
                        }
                    }

                    ReleaseStgMedium(&raw mut medium);

                    if !files.is_empty() {
                        let id = WindowId(self.hwnd.0 as isize);
                        // イベントキューに通知
                        dispatch_to_active_handler(
                            MichiuAnyEvent::Window {
                                id,
                                event: MichiuEvent::FileDropped(Unvalidated::new(files)),
                            },
                            id,
                            MichiuRawEvent {
                                msg: 0,
                                wparam: WPARAM(0),
                                lparam: LPARAM(0),
                            },
                        );
                        *effect = DROPEFFECT_COPY;
                    }
                }
            }
        }
        Ok(())
    }
}

#[expect(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::print_stdout
)]
#[cfg(test)]
mod tests {
    use super::*;
    use crate::{CURRENT_HANDLER, HandlerGuard, HandlerPtr, Result};
    use michiu_guard::Validated;
    use std::os::windows::ffi::OsStrExt;
    use windows::Win32::{
        Foundation::{E_NOTIMPL, HGLOBAL, S_OK},
        System::{
            Com::{
                COINIT_APARTMENTTHREADED, CoInitializeEx, CoUninitialize, IAdviseSink,
                IDataObject_Impl, IEnumFORMATETC, STGMEDIUM,
            },
            Memory::{GMEM_MOVEABLE, GlobalAlloc, GlobalLock, GlobalUnlock},
        },
        UI::Shell::DROPFILES,
    };
    use windows_core::{BOOL, HRESULT};

    // 各テストケースをCOMの初期化状態が完全にクリーンな新規スレッドで実行。
    // 並行して走る他のテストからのスレッド干渉を防ぐ。
    fn run_on_clean_thread<F>(f: F)
    where
        F: FnOnce() + Send + 'static,
    {
        let handle = std::thread::spawn(f);
        handle.join().expect("Test thread panicked");
    }

    #[test]
    fn test_file_drop_target_com_interface_calls() {
        run_on_clean_thread(|| {
            // FileDropTarget を作成し、IDropTarget COMインターフェースとして取得
            let target_impl = FileDropTarget {
                hwnd: HWND::default(),
            };
            let target: IDropTarget = target_impl.into();

            unsafe {
                // DragLeave の COM 経由での呼び出しテスト
                let res_leave = target.DragLeave();
                assert!(
                    res_leave.is_ok(),
                    "DragLeave through COM interface should return Ok"
                );

                // DragOver の COM 経由での呼び出しテスト (戻り値に DROPEFFECT_COPY が書き戻されるか検証)
                let mut effect = DROPEFFECT_NONE;
                let res_over = target.DragOver(
                    MODIFIERKEYS_FLAGS(0),
                    POINTL { x: 0, y: 0 },
                    &raw mut effect,
                );

                assert!(res_over.is_ok(), "DragOver should succeed");
                assert_eq!(
                    effect, DROPEFFECT_COPY,
                    "DragOver must write back DROPEFFECT_COPY to effect pointer"
                );
            }
        });
    }

    // テスト用の擬似 IDataObject モックの実装

    #[implement(IDataObject)]
    struct MockDataObject {
        paths: Vec<std::path::PathBuf>,
    }

    impl IDataObject_Impl for MockDataObject_Impl {
        fn GetData(&self, pformatetc: *const FORMATETC) -> windows::core::Result<STGMEDIUM> {
            unsafe {
                // CF_HDROP (ファイルドロップ形式) が要求された場合、モックの HGLOBAL データを詰めて返す
                if (*pformatetc).cfFormat == CF_HDROP.0 {
                    let h_global = create_mock_hdrop(&self.paths);

                    let mut medium = STGMEDIUM {
                        tymed: TYMED_HGLOBAL.0 as u32,
                        ..Default::default()
                    };
                    medium.u.hGlobal = h_global;

                    return Ok(medium);
                }
            }
            Err(windows::core::Error::from(E_NOTIMPL))
        }

        fn GetDataHere(&self, _: *const FORMATETC, _: *mut STGMEDIUM) -> windows::core::Result<()> {
            Err(windows::core::Error::from(E_NOTIMPL))
        }

        fn QueryGetData(&self, pformatetc: *const FORMATETC) -> HRESULT {
            unsafe {
                if (*pformatetc).cfFormat == CF_HDROP.0 {
                    return S_OK;
                }
            }
            E_NOTIMPL
        }

        fn GetCanonicalFormatEtc(&self, _: *const FORMATETC, _: *mut FORMATETC) -> HRESULT {
            E_NOTIMPL
        }

        fn SetData(
            &self,
            _: *const FORMATETC,
            _: *const STGMEDIUM,
            _: BOOL,
        ) -> windows::core::Result<()> {
            Err(windows::core::Error::from(E_NOTIMPL))
        }

        fn EnumFormatEtc(&self, _: u32) -> windows::core::Result<IEnumFORMATETC> {
            Err(windows::core::Error::from(E_NOTIMPL))
        }

        fn DAdvise(
            &self,
            _: *const FORMATETC,
            _: u32,
            _: Ref<IAdviseSink>,
        ) -> windows::core::Result<u32> {
            Err(windows::core::Error::from(E_NOTIMPL))
        }

        fn DUnadvise(&self, _: u32) -> windows::core::Result<()> {
            Err(windows::core::Error::from(E_NOTIMPL))
        }

        fn EnumDAdvise(&self) -> windows::core::Result<windows::Win32::System::Com::IEnumSTATDATA> {
            Err(windows::core::Error::from(E_NOTIMPL))
        }
    }

    // Win32 OLE 形式の HDROP データを HGLOBAL メモリにパッキングしてダミー生成するヘルパー
    unsafe fn create_mock_hdrop(paths: &[std::path::PathBuf]) -> HGLOBAL {
        let mut path_bytes = Vec::new();
        for p in paths {
            let wide: Vec<u16> = p.as_os_str().encode_wide().chain(Some(0)).collect();
            path_bytes.extend_from_slice(&wide);
        }
        path_bytes.push(0); // ダブルヌル終端用の追加ヌル

        let dropfiles_size = std::mem::size_of::<DROPFILES>();
        let total_size = dropfiles_size + (path_bytes.len() * 2);

        let h_mem = unsafe { GlobalAlloc(GMEM_MOVEABLE, total_size).unwrap() };
        let ptr = unsafe { GlobalLock(h_mem) };

        #[expect(clippy::cast_possible_truncation)]
        let dropfiles = DROPFILES {
            pFiles: dropfiles_size as u32,
            pt: windows::Win32::Foundation::POINT { x: 0, y: 0 },
            fNC: false.into(),
            fWide: true.into(),
        };

        // `ptr` は `dropfiles_size + path_bytes のサイズ` 以上に確保されていること
        // dropfiles_size (20 bytes) は直後の u16 のアライメントも満たしている
        unsafe {
            std::ptr::write(ptr.cast::<DROPFILES>(), dropfiles);
            #[expect(clippy::cast_ptr_alignment)]
            std::ptr::copy_nonoverlapping(
                path_bytes.as_ptr(),
                ptr.cast::<u8>().add(dropfiles_size).cast::<u16>(),
                path_bytes.len(),
            );
        }

        let _ = unsafe { GlobalUnlock(h_mem) };
        h_mem
    }

    // Drop メソッドが正常に動作し、Event::FileDropped がイベントキューにプッシュされるかのテスト
    #[test]
    fn test_file_drop_target_drop_integration_normal() {
        run_on_clean_thread(|| {
            let dummy_hwnd = HWND(0x99999 as _);

            // ターゲットに紐づける DropTarget インスタンスの生成
            let target_impl = FileDropTarget { hwnd: dummy_hwnd };
            let target: IDropTarget = target_impl.into();

            // 投入するファイル一覧の用意
            let expected_paths = vec![
                std::path::PathBuf::from(r"C:\Michiu\test1.txt"),
                std::path::PathBuf::from(r"C:\Michiu\docs\test2.png"),
            ];

            // モックオブジェクトの構築
            let mock_data: IDataObject = MockDataObject {
                paths: expected_paths.clone(),
            }
            .into();

            let mut effect = DROPEFFECT_NONE;

            let mut captured_event = None;
            let mut capture_fn = |event: MichiuAnyEvent, _id: WindowId, _raw: MichiuRawEvent| {
                captured_event = Some(event);
            };

            {
                let f_trait: &mut (dyn FnMut(MichiuAnyEvent, WindowId, MichiuRawEvent) + '_) =
                    &mut capture_fn;
                let erased_ptr: HandlerPtr = unsafe { std::mem::transmute(f_trait) };
                CURRENT_HANDLER.with(|h| h.set(Some(erased_ptr)));
                let _guard = HandlerGuard;

                let res_drop = unsafe {
                    target.Drop(
                        &mock_data,
                        windows::Win32::System::SystemServices::MODIFIERKEYS_FLAGS(0),
                        windows::Win32::Foundation::POINTL { x: 0, y: 0 },
                        &raw mut effect,
                    )
                };

                assert!(res_drop.is_ok());
                assert_eq!(effect, windows::Win32::System::Ole::DROPEFFECT_COPY);
            }

            assert!(
                captured_event.is_some(),
                "No event was received by EventPump"
            );

            match captured_event.unwrap() {
                MichiuAnyEvent::Window { id, event, .. } => {
                    assert_eq!(id, WindowId(dummy_hwnd.0 as isize));
                    match event {
                        MichiuEvent::FileDropped(unvalidated_files) => {
                            let validated: Result<Validated<Vec<PathBuf>>> = unvalidated_files
                                .validate_with(|files| {
                                    assert_eq!(files, expected_paths);
                                    Ok(files)
                                });
                            assert!(validated.is_ok());
                        }
                        other => panic!("Expected Event::FileDropped, got {other:?}"),
                    }
                }
                MichiuAnyEvent::User(_) => panic!("Expected Event"),
            }
        });
    }

    #[test]
    fn test_options_default() {
        let options = FileDialogOptions::default();
        assert_eq!(options.title, None);
        assert_eq!(options.default_file_name, None);
        assert_eq!(options.default_extension, None);
        assert!(options.overwrite_prompt);
        assert!(options.filters.is_empty());
    }

    #[test]
    fn test_options_builder() {
        let options = FileDialogOptions::new()
            .title("Save File")
            .default_file_name("test.txt")
            .default_extension("txt")
            .overwrite_prompt(false)
            .add_filter("Text Files (*.txt)", "*.txt")
            .add_filter("All Files (*.*)", "*.*");

        assert_eq!(options.title.as_deref(), Some("Save File"));
        assert_eq!(options.default_file_name.as_deref(), Some("test.txt"));
        assert_eq!(options.default_extension.as_deref(), Some("txt"));
        assert!(!options.overwrite_prompt);
        assert_eq!(options.filters.len(), 2);
        assert_eq!(options.filters[0].name, "Text Files (*.txt)");
        assert_eq!(options.filters[0].spec, "*.txt");
    }

    #[test]
    fn test_com_instance_creation_and_config() {
        run_on_clean_thread(|| {
            unsafe {
                let _ = CoInitializeEx(None, COINIT_APARTMENTTHREADED);

                // 新しいスコープを作ることで dialog の Drop を先に終わらせる
                // COMインスタンスの生成が成功するか
                {
                    let dialog: windows::core::Result<IFileSaveDialog> =
                        CoCreateInstance(&FileSaveDialog, None, CLSCTX_ALL);
                    assert!(
                        dialog.is_ok(),
                        "Failed to create an instance of `IFileSaveDialog`"
                    );
                    let dialog = dialog.unwrap();

                    // 設定用パラメータ
                    let options = FileDialogOptions::new()
                        .title("Test Dialog")
                        .default_file_name("dummy.txt")
                        .default_extension("txt")
                        .add_filter("Text", "*.txt");

                    // 画面を出す直前までのAPI呼び出しがエラーなく正しくCOM側に渡るかを検証
                    let title = HSTRING::from(options.title.as_ref().unwrap());
                    assert!(dialog.SetTitle(&title).is_ok());

                    let filename = HSTRING::from(options.default_file_name.as_ref().unwrap());
                    assert!(dialog.SetFileName(&filename).is_ok());

                    let ext = HSTRING::from(options.default_extension.as_ref().unwrap());
                    assert!(dialog.SetDefaultExtension(&ext).is_ok());
                }

                CoUninitialize();
            }
        });
    }

    #[test]
    #[ignore = "Since the UI screen is displayed and waiting for user input, this step is typically skipped during testing."]
    fn manual_test_open_file_dialog() {
        run_on_clean_thread(|| {
            let _ = unsafe { CoInitializeEx(None, COINIT_APARTMENTTHREADED) };

            // 親なし（None）でダイアログを表示
            let result = FileDialog::open_file_dialog(None);

            match result {
                Ok(Some(path)) => println!("\nSelected File Path: {path:?}"),
                Ok(None) => println!("\nCancel"),
                Err(e) => panic!("Error: {e:?}"),
            }

            unsafe { CoUninitialize() };
        });
    }

    #[test]
    #[ignore = "Since the UI screen is displayed and waiting for user input, this step is typically skipped during testing."]
    fn manual_test_save_file_dialog() {
        run_on_clean_thread(|| {
            let _ = unsafe { CoInitializeEx(None, COINIT_APARTMENTTHREADED) };

            let options = FileDialogOptions::new()
                .title("Dialog")
                .default_file_name("sample.txt")
                .default_extension("txt")
                .add_filter("Text Files (*.txt)", "*.txt")
                .add_filter("All Files (*.*)", "*.*");

            let result = FileDialog::save_file_dialog(None, &options);

            match result {
                Ok(Some(path)) => println!("\nSave Path: {path:?}"),
                Ok(None) => println!("\nCancel"),
                Err(e) => panic!("Error: {e:?}"),
            }

            unsafe { CoUninitialize() };
        });
    }
}
