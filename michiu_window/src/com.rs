use crate::error::{MichiuError, Result};
use crate::{Event, MichiuEvent, WindowId, push_event};
use michiu_guard::Unvalidated;
use std::marker::PhantomData;
use std::path::PathBuf;
use windows::Win32::{
    Foundation::{HWND, POINTL},
    System::{
        Com::{
            COINIT, COINIT_MULTITHREADED, CoInitializeEx, CoUninitialize, DVASPECT_CONTENT,
            FORMATETC, IDataObject, TYMED_HGLOBAL,
        },
        Ole::{
            CF_HDROP, DROPEFFECT, DROPEFFECT_COPY, DROPEFFECT_NONE, IDropTarget, IDropTarget_Impl,
            OleInitialize, OleUninitialize, ReleaseStgMedium,
        },
        SystemServices::MODIFIERKEYS_FLAGS,
        WinRT::{
            RO_INIT_MULTITHREADED, RO_INIT_SINGLETHREADED, RO_INIT_TYPE, RoInitialize,
            RoUninitialize,
        },
    },
    UI::Shell::{DragQueryFileW, HDROP},
};
use windows::core::{Ref, implement};

/// A thread-affine RAII guard managing the lifecycle of COM or Windows Runtime (WinRT) initialization on the current thread.
///
/// Since COM and WinRT threading apartments are strictly thread-affine, `ComContext` is **`!Send` and `!Sync`**.
///
/// Cloning a `ComContext` correctly increments the underlying OS-side initialization reference counter
/// on the same thread, and dropping a `ComContext` automatically decrements it by calling the appropriate
/// uninitialization API (`CoUninitialize`, `OleUninitialize`, or `RoUninitialize`).
///
/// Under the hood, this helps prevent runtime threading model conflicts (e.g., `RPC_E_CHANGED_MODE` (0x80010106))
/// by consolidating thread-apartment initialization inside a safe Rust scope.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ComContextKind {
    Classic(COINIT),
    Ole,
    WinRt(RO_INIT_TYPE),
}

/// A thread-affine RAII guard that manages COM or Windows Runtime (WinRT) initialization.
///
/// Because COM and WinRT threading models are fundamentally thread-affine, this structure
/// does not implement `Send` or `Sync`. When cloned on the same thread, it correctly increments
/// the OS initialization reference counter. Dropping this guard automatically calls the
/// corresponding uninitialization API (`CoUninitialize` or `RoUninitialize`).
#[derive(Debug)]
pub struct ComContext {
    pub(crate) kind: ComContextKind,
    _marker: PhantomData<*const ()>, // !Send and !Sync
}

impl ComContext {
    /// Initializes the OLE library on the current thread under the Single-Threaded Apartment (STA) model.
    ///
    /// This apartment model is highly recommended and required if your window utilizes standard system clipboard
    /// operations, IME, or OLE file drag-and-drop ([`crate::builder::WindowBuilder::with_drag_and_drop`]).
    ///
    /// # Errors
    /// Returns [`MichiuError::ComInitializationFailed`] if the underlying `OleInitialize` fails.
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use michiu_window::ComContext;
    ///
    /// fn main() -> Result<(), Box<dyn std::error::Error>> {
    ///     // Initialize OLE STA on the main UI thread
    ///     let com_ctx = ComContext::new_com_single()?;
    ///     Ok(())
    /// }
    /// ```
    #[inline]
    pub fn new_com_single() -> Result<Self> {
        unsafe {
            OleInitialize(None).map_err(|err| MichiuError::ComInitializationFailed {
                context_type: "OLE (COM Single)",
                source: err,
            })?;
        }
        Ok(Self {
            kind: ComContextKind::Ole,
            _marker: PhantomData,
        })
    }

    /// Initializes the COM library on the current thread under the Multi-Threaded Apartment (MTA) model.
    ///
    /// Useful for background worker threads that handle high-performance, non-GUI COM interfaces.
    ///
    /// # Errors
    /// Returns [`MichiuError::ComInitializationFailed`] if `CoInitializeEx` fails.
    #[inline]
    pub fn new_com_multi() -> Result<Self> {
        unsafe {
            CoInitializeEx(None, COINIT_MULTITHREADED)
                .ok()
                .map_err(|err| MichiuError::ComInitializationFailed {
                    context_type: "COM Multi",
                    source: err,
                })?;
        }
        Ok(Self {
            kind: ComContextKind::Classic(COINIT_MULTITHREADED),
            _marker: PhantomData,
        })
    }

    /// Initializes the Windows Runtime (WinRT) on the current thread under the Single-Threaded Apartment model.
    ///
    /// # Errors
    /// Returns [`MichiuError::ComInitializationFailed`] if `RoInitialize` fails.
    #[inline]
    pub fn new_ro_single() -> Result<Self> {
        unsafe {
            RoInitialize(RO_INIT_SINGLETHREADED).map_err(|err| {
                MichiuError::ComInitializationFailed {
                    context_type: "WinRT Single",
                    source: err,
                }
            })?;
        }
        Ok(Self {
            kind: ComContextKind::WinRt(RO_INIT_SINGLETHREADED),
            _marker: PhantomData,
        })
    }

    /// Initializes the Windows Runtime (WinRT) on the current thread under the Multi-Threaded Apartment model.
    ///
    /// # Errors
    /// Returns [`MichiuError::ComInitializationFailed`] if `RoInitialize` fails.
    #[inline]
    pub fn new_ro_multi() -> Result<Self> {
        unsafe {
            RoInitialize(RO_INIT_MULTITHREADED).map_err(|err| {
                MichiuError::ComInitializationFailed {
                    context_type: "WinRT Multi",
                    source: err,
                }
            })?;
        }
        Ok(Self {
            kind: ComContextKind::WinRt(RO_INIT_MULTITHREADED),
            _marker: PhantomData,
        })
    }

    #[inline]
    pub(crate) fn is_ole(&self) -> bool {
        matches!(self.kind, ComContextKind::Ole)
    }
}

// When cloning the context, we must also increment the OS-side initialization reference counter
// to maintain a correct balance when drops occur.
impl Clone for ComContext {
    #[inline]
    fn clone(&self) -> Self {
        match self.kind {
            ComContextKind::Classic(init) => {
                let _ = unsafe { CoInitializeEx(None, init) };
            }
            ComContextKind::Ole => {
                let _ = unsafe { OleInitialize(None) };
            }
            ComContextKind::WinRt(init) => {
                let _ = unsafe { RoInitialize(init) };
            }
        }

        Self {
            kind: self.kind,
            _marker: PhantomData,
        }
    }
}

impl Drop for ComContext {
    fn drop(&mut self) {
        unsafe {
            // Uninitialize only the API that was actually initialized for this context.
            match self.kind {
                ComContextKind::Classic(_) => CoUninitialize(),
                ComContextKind::Ole => OleUninitialize(),
                ComContextKind::WinRt(_) => RoUninitialize(),
            }
        }
    }
}

/// A helper struct used to implement OLE File Drag and Drop functionality for a window.
///
/// This implements the raw Win32 COM `IDropTarget` interface. It processes incoming file drag-and-drop
/// operations (verifying `CF_HDROP` data format), extracts dropped file paths, and automatically
/// posts a [`crate::events::Event::FileDropped`] event containing an [`Unvalidated<Vec<PathBuf>>`] payload
/// to the event queue.
///
/// # Examples
///
/// This struct can also be used manually in integration tests to emulate drag-and-drop actions on a window:
///
/// ```no_run
/// # use michiu_window::{Window, WindowBuilder, FileDropTarget, ComContext};
/// # use windows::Win32::System::Ole::IDropTarget;
/// # fn main() -> Result<(), Box<dyn std::error::Error>> {
/// # let com_ctx = ComContext::new_com_single()?;
/// # let window = Window::build(WindowBuilder::new().with_com_context(&com_ctx).into_unvalidated().try_into()?)?;
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
    /// # use michiu_window::{Window, WindowBuilder, FileDropTarget, ComContext};
    /// # fn main() -> Result<(), Box<dyn std::error::Error>> {
    /// # let com_ctx = ComContext::new_com_single()?;
    /// # let window = Window::build(WindowBuilder::new().with_com_context(&com_ctx).into_unvalidated().try_into()?)?;
    /// // Create a drop target bound to the window's HWND
    /// let drop_target = FileDropTarget::new(window.hwnd());
    /// # Ok(())
    /// # }
    /// ```
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
    #[inline]
    pub fn hwnd(&self) -> HWND {
        self.hwnd
    }
}

#[allow(clippy::not_unsafe_ptr_arg_deref)]
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

                if data_obj.QueryGetData(&format_etc).is_ok() {
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
                if let Ok(mut medium) = data_obj.GetData(&format_etc) {
                    let hdrop = HDROP(medium.u.hGlobal.0 as _);

                    let file_count = DragQueryFileW(hdrop, 0xFFFFFFFF, None);
                    let mut files = Vec::with_capacity(file_count as usize);

                    for i in 0..file_count {
                        let len = DragQueryFileW(hdrop, i, None);
                        let mut buf = vec![0u16; (len + 1) as usize];
                        DragQueryFileW(hdrop, i, Some(&mut buf));

                        if let Ok(path_str) = String::from_utf16(&buf[..len as usize]) {
                            files.push(PathBuf::from(path_str));
                        }
                    }

                    ReleaseStgMedium(&mut medium);

                    if !files.is_empty() {
                        // イベントキューに通知
                        push_event(MichiuEvent::Window {
                            id: WindowId(self.hwnd.0 as isize),
                            event: Event::FileDropped(Unvalidated::new(files)),
                        });
                        *effect = DROPEFFECT_COPY;
                    }
                }
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use std::os::windows::ffi::OsStrExt;

    use michiu_guard::Validated;
    use windows::Win32::{
        Foundation::{E_NOTIMPL, HGLOBAL, S_OK},
        System::{
            Com::{IAdviseSink, IDataObject_Impl, IEnumFORMATETC, STGMEDIUM},
            Memory::{GMEM_MOVEABLE, GlobalAlloc, GlobalLock, GlobalUnlock},
        },
        UI::Shell::DROPFILES,
    };
    use windows_core::{BOOL, HRESULT};

    use super::*;
    use crate::{EventPump, error::MichiuError};

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
    fn test_com_single_initialization_normal() {
        run_on_clean_thread(|| {
            let context_result = ComContext::new_com_single();
            assert!(
                context_result.is_ok(),
                "Failed to initialize COM STA: {:?}",
                context_result.err()
            );
            // スコープを抜ける際の CoUninitialize 呼び出し（Drop）も暗黙的にテスト
        });
    }

    #[test]
    fn test_com_multi_initialization_normal() {
        run_on_clean_thread(|| {
            let context_result = ComContext::new_com_multi();
            assert!(
                context_result.is_ok(),
                "Failed to initialize COM MTA: {:?}",
                context_result.err()
            );
        });
    }

    #[test]
    fn test_ro_single_initialization_normal() {
        run_on_clean_thread(|| {
            let context_result = ComContext::new_ro_single();
            assert!(
                context_result.is_ok(),
                "Failed to initialize WinRT Single: {:?}",
                context_result.err()
            );
        });
    }

    #[test]
    fn test_ro_multi_initialization_normal() {
        run_on_clean_thread(|| {
            let context_result = ComContext::new_ro_multi();
            assert!(
                context_result.is_ok(),
                "Failed to initialize WinRT Multi: {:?}",
                context_result.err()
            );
        });
    }

    #[test]
    fn test_com_context_cloning_and_reference_balance() {
        run_on_clean_thread(|| {
            let context = ComContext::new_com_single().unwrap();

            // クローンすることにより、OS側の初期化参照カウントが正しくインクリメントされる
            let context_clone = context.clone();

            // 双方が同一スレッド上に安全に共存できることをテスト
            assert!(
                matches!(context.kind, ComContextKind::Ole),
                "Original context kind should match"
            );
            assert!(
                matches!(context_clone.kind, ComContextKind::Ole),
                "Cloned context kind should match"
            );

            // 片方をドロップしても、OS側のCOM初期化はもう片方によって維持される
            drop(context_clone);

            // 最後にオリジナルがドロップされて初めて、スレッドのCOMが完全にアンロードされる
            drop(context);
        });
    }

    #[test]
    fn test_com_threading_mode_conflict_abnormal() {
        run_on_clean_thread(|| {
            // 同一スレッドで STA (Single-Threaded Apartment) を強制的に有効化
            let _context_sta = ComContext::new_com_single().unwrap();

            // すでにSTAとして設定された同じスレッドで、MTA (Multi-Threaded) に切り替えようとする
            let context_mta_result = ComContext::new_com_multi();

            // COMの仕様上、一度設定したスレッドモードは変更できないため、エラーになるはず
            assert!(
                context_mta_result.is_err(),
                "Changing thread mode should fail"
            );

            // エラーコードが Win32 の「RPC_E_CHANGED_MODE (0x80010106)」に合致するかを検証
            let err = context_mta_result.unwrap_err();
            match err {
                MichiuError::ComInitializationFailed {
                    context_type,
                    source,
                } => {
                    assert_eq!(context_type, "COM Multi");

                    // Windows SDK エラーコード: 0x80010106 (RPC_E_CHANGED_MODE)
                    let error_code = source.code().0 as u32;
                    assert_eq!(
                        error_code, 0x80010106,
                        "Expected RPC_E_CHANGED_MODE (0x80010106) but got 0x{:08X}",
                        error_code
                    );
                }
                other => panic!("Expected ComInitializationFailed, but got: {:?}", other),
            }
        });
    }

    #[test]
    fn test_com_context_is_ole_check() {
        // OLE STA のテスト
        run_on_clean_thread(|| {
            let context_single = ComContext::new_com_single().unwrap();
            assert!(
                context_single.is_ole(),
                "new_com_single must be recognized as OLE STA"
            );
        });

        // MTA のテスト
        run_on_clean_thread(|| {
            let context_multi = ComContext::new_com_multi().unwrap();
            assert!(!context_multi.is_ole(), "new_com_multi is not OLE");
        });
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
                let res_over =
                    target.DragOver(MODIFIERKEYS_FLAGS(0), POINTL { x: 0, y: 0 }, &mut effect);

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

        let dropfiles = DROPFILES {
            pFiles: dropfiles_size as u32,
            pt: windows::Win32::Foundation::POINT { x: 0, y: 0 },
            fNC: false.into(),
            fWide: true.into(),
        };

        unsafe {
            std::ptr::write(ptr as *mut DROPFILES, dropfiles);
            std::ptr::copy_nonoverlapping(
                path_bytes.as_ptr(),
                (ptr as *mut u8).add(dropfiles_size) as *mut u16,
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

            // OLE の Drop イベントを擬似呼び出し
            let res_drop = unsafe {
                target.Drop(
                    &mock_data,
                    windows::Win32::System::SystemServices::MODIFIERKEYS_FLAGS(0),
                    windows::Win32::Foundation::POINTL { x: 0, y: 0 },
                    &mut effect,
                )
            };

            assert!(res_drop.is_ok());
            assert_eq!(effect, windows::Win32::System::Ole::DROPEFFECT_COPY);

            // イベントキューからプッシュされたデータを取り出し検証
            let mut pump = EventPump::new();
            let ev = pump.poll_event();
            assert!(ev.is_some());

            match ev.unwrap() {
                MichiuEvent::Window { id, event } => {
                    assert_eq!(id, WindowId(dummy_hwnd.0 as isize));
                    match event {
                        Event::FileDropped(unvalidated_files) => {
                            // 境界防御の validate_with で安全に検査
                            let validated: Result<Validated<Vec<PathBuf>>> = unvalidated_files
                                .validate_with(|files| {
                                    assert_eq!(files, expected_paths);
                                    Ok(files)
                                });
                            assert!(validated.is_ok());
                        }
                        other => panic!("Expected Event::FileDropped, got {:?}", other),
                    }
                }
                _ => panic!("Expected Event"),
            }
        });
    }
}
