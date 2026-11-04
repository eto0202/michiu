use crate::error::{MichiuError, Result};
use std::marker::PhantomData;
use windows::Win32::System::{
    Com::{COINIT, COINIT_MULTITHREADED, CoInitializeEx, CoUninitialize},
    Ole::{OleInitialize, OleUninitialize},
    WinRT::{
        RO_INIT_MULTITHREADED, RO_INIT_SINGLETHREADED, RO_INIT_TYPE, RoInitialize, RoUninitialize,
    },
};

/// A thread-affine RAII guard managing the lifecycle of COM or Windows Runtime (`WinRT`) initialization on the current thread.
///
/// Since COM and `WinRT` threading apartments are strictly thread-affine,
///  `ComContext` is **`!Send` and `!Sync`**.
///
/// Cloning a `MichiuComContext` correctly increments the underlying OS-side initialization reference counter
/// on the same thread, and dropping a `MichiuComContext` automatically decrements it by calling the appropriate
/// uninitialization API (`CoUninitialize`, `OleUninitialize`, or `RoUninitialize`).
///
/// Under the hood, this helps prevent runtime threading model conflicts (e.g., `RPC_E_CHANGED_MODE` (0x80010106))
/// by consolidating thread-apartment initialization inside a safe Rust scope.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ComKind {
    Classic(COINIT),
    Ole,
    WinRt(RO_INIT_TYPE),
    WinRtOleCombo,
}

/// A thread-affine RAII guard that manages COM or Windows Runtime (`WinRT`) initialization.
///
/// Because COM and `WinRT` threading models are fundamentally thread-affine, this structure
/// does not implement `Send` or `Sync`. When cloned on the same thread, it correctly increments
/// the OS initialization reference counter. Dropping this guard automatically calls the
/// corresponding uninitialization API (`CoUninitialize` or `RoUninitialize`).
#[derive(Debug)]
pub struct MichiuComContext {
    pub(crate) kind: ComKind,
    _marker: PhantomData<*const ()>, // !Send and !Sync
}

impl MichiuComContext {
    /// Initializes the OLE library on the current thread under the Single-Threaded Apartment (STA) model.
    ///
    /// This apartment model is highly recommended and required if your window utilizes standard system clipboard
    /// operations, IME, or OLE file drag-and-drop ([`crate::builder::MichiuWindowBuilder::with_drag_and_drop`]).
    ///
    /// # Errors
    /// Returns [`MichiuError::ComInitializationFailed`] if the underlying `OleInitialize` fails.
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use michiu_window::MichiuComContext;
    ///
    /// fn main() -> Result<(), Box<dyn std::error::Error>> {
    ///     // Initialize OLE STA on the main UI thread
    ///     let com_ctx = MichiuComContext::new_com_single()?;
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
            kind: ComKind::Ole,
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
            kind: ComKind::Classic(COINIT_MULTITHREADED),
            _marker: PhantomData,
        })
    }

    /// Initializes the Windows Runtime (`WinRT`) on the current thread under the Single-Threaded Apartment model.
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
            kind: ComKind::WinRt(RO_INIT_SINGLETHREADED),
            _marker: PhantomData,
        })
    }

    /// Initializes the Windows Runtime (`WinRT`) on the current thread under the Multi-Threaded Apartment model.
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
            kind: ComKind::WinRt(RO_INIT_MULTITHREADED),
            _marker: PhantomData,
        })
    }

    /// A dedicated method for initializing `WinRT` (STA) and OLE in this order.
    /// Required when using OLE drag and drop and `WinRT` API (notifications, etc.) on the same thread.
    ///
    /// # Errors
    ///
    /// Returns [`MichiuError::ComInitializationFailed`] if `RoInitialize` and `OleInitialize` fails.
    #[inline]
    pub fn new_winrt_ole_combo() -> Result<Self> {
        unsafe {
            // 先に WinRT を STA で初期化
            RoInitialize(RO_INIT_SINGLETHREADED).map_err(|err| {
                MichiuError::ComInitializationFailed {
                    context_type: "WinRT (Combo)",
                    source: err,
                }
            })?;

            // 次に OLE を初期化
            if let Err(err) = OleInitialize(None) {
                // OLEが失敗した場合はWinRT側を片付けてからエラーを返す
                RoUninitialize();
                return Err(MichiuError::ComInitializationFailed {
                    context_type: "OLE (Combo)",
                    source: err,
                });
            }
        }

        Ok(Self {
            kind: ComKind::WinRtOleCombo,
            _marker: PhantomData,
        })
    }

    #[inline]
    pub(crate) fn is_ole(&self) -> bool {
        matches!(self.kind, ComKind::Ole)
    }
}

// When cloning the context, we must also increment the OS-side initialization reference counter
// to maintain a correct balance when drops occur.
impl Clone for MichiuComContext {
    #[inline]
    fn clone(&self) -> Self {
        match self.kind {
            ComKind::Classic(init) => {
                let _ = unsafe { CoInitializeEx(None, init) };
            }
            ComKind::Ole => {
                let _ = unsafe { OleInitialize(None) };
            }
            ComKind::WinRt(init) => {
                let _ = unsafe { RoInitialize(init) };
            }
            ComKind::WinRtOleCombo => unsafe {
                // 初期化時と同じ順序で参照カウントを増やす
                let _ = RoInitialize(RO_INIT_SINGLETHREADED);
                let _ = OleInitialize(None);
            },
        }

        Self {
            kind: self.kind,
            _marker: PhantomData,
        }
    }
}

impl Drop for MichiuComContext {
    fn drop(&mut self) {
        unsafe {
            // Uninitialize only the API that was actually initialized for this context.
            match self.kind {
                ComKind::Classic(_) => CoUninitialize(),
                ComKind::Ole => OleUninitialize(),
                ComKind::WinRt(_) => RoUninitialize(),
                ComKind::WinRtOleCombo => {
                    // 初期化時と逆の順序で解放する
                    OleUninitialize();
                    RoUninitialize();
                }
            }
        }
    }
}

#[expect(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
#[cfg(test)]
mod tests {
    use super::*;
    use crate::error::MichiuError;

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
            let context_result = MichiuComContext::new_com_single();
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
            let context_result = MichiuComContext::new_com_multi();
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
            let context_result = MichiuComContext::new_ro_single();
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
            let context_result = MichiuComContext::new_ro_multi();
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
            let context = MichiuComContext::new_com_single().unwrap();

            // クローンすることにより、OS側の初期化参照カウントが正しくインクリメントされる
            let context_clone = context.clone();

            // 双方が同一スレッド上に安全に共存できることをテスト
            assert!(
                matches!(context.kind, ComKind::Ole),
                "Original context kind should match"
            );
            assert!(
                matches!(context_clone.kind, ComKind::Ole),
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
            let _context_sta = MichiuComContext::new_com_single().unwrap();

            // すでにSTAとして設定された同じスレッドで、MTA (Multi-Threaded) に切り替えようとする
            let context_mta_result = MichiuComContext::new_com_multi();

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
                    let error_code = source.code().0.cast_unsigned();
                    assert_eq!(
                        error_code, 0x8001_0106,
                        "Expected RPC_E_CHANGED_MODE (0x80010106) but got 0x{error_code:08X}"
                    );
                }
                other => panic!("Expected ComInitializationFailed, but got: {other:?}"),
            }
        });
    }

    #[test]
    fn test_com_context_is_ole_check() {
        // OLE STA のテスト
        run_on_clean_thread(|| {
            let context_single = MichiuComContext::new_com_single().unwrap();
            assert!(
                context_single.is_ole(),
                "new_com_single must be recognized as OLE STA"
            );
        });

        // MTA のテスト
        run_on_clean_thread(|| {
            let context_multi = MichiuComContext::new_com_multi().unwrap();
            assert!(!context_multi.is_ole(), "new_com_multi is not OLE");
        });
    }
}
