use std::sync::atomic::{AtomicBool, Ordering};

use super::*;
use crate::{ComContext, WindowBuilder};
use michiu_guard::Validate;
use raw_window_handle::{HasDisplayHandle, HasWindowHandle, RawDisplayHandle, RawWindowHandle};
use windows::{
    Win32::{
        Foundation::{ERROR_ACCESS_DENIED, GetLastError},
        System::LibraryLoader::GetModuleHandleW,
        UI::{Shell::DefSubclassProc, WindowsAndMessaging::WM_USER},
    },
    core::w,
};

fn run_on_clean_thread<F>(f: F)
where
    F: FnOnce() + Send + 'static,
{
    let handle = std::thread::spawn(f);
    handle.join().expect("Test thread panicked");
}

#[test]
fn test_build_raw_style_normal() {
    // 標準構成
    let builder: Validated<WindowBuilder> =
        WindowBuilder::new().into_unvalidated().try_into().unwrap();
    let style = build_raw_style(&builder);
    assert!(
        style.contains(WS_OVERLAPPEDWINDOW),
        "Default window should have WS_OVERLAPPEDWINDOW"
    );

    // デコレーションなし構成
    let builder_no_dec = WindowBuilder::new().with_decorations(false);
    let style_no_dec = build_raw_style(&builder_no_dec);
    assert!(
        !style_no_dec.contains(WS_CAPTION),
        "No-decoration style should not contain WS_CAPTION"
    );
    assert!(
        !style_no_dec.contains(WS_BORDER),
        "No-decoration style should not contain WS_BORDER"
    );

    // リサイズ不可構成
    let builder_no_res = WindowBuilder::new().with_resizable(false);
    let style_no_res = build_raw_style(&builder_no_res);
    assert!(
        !style_no_res.contains(WS_THICKFRAME),
        "Non-resizable style should not contain WS_THICKFRAME"
    );
    assert!(
        !style_no_res.contains(WS_MAXIMIZEBOX),
        "Non-resizable style should not contain WS_MAXIMIZEBOX"
    );
}

#[test]
fn test_build_raw_ex_style_normal() {
    // 標準構成
    let builder = WindowBuilder::new();
    let ex_style = build_raw_ex_style(&builder);
    assert!(
        !ex_style.contains(WS_EX_LAYERED),
        "Default ex_style should not be layered"
    );

    // 透明化構成
    let builder_trans = WindowBuilder::new().with_transparent(true);
    let ex_style_trans = build_raw_ex_style(&builder_trans);
    assert!(
        ex_style_trans.contains(WS_EX_LAYERED),
        "Transparent window must have WS_EX_LAYERED"
    );

    // ヒットテスト無視構成
    let builder_no_hit = WindowBuilder::new().with_hittest(false);
    let ex_style_no_hit = build_raw_ex_style(&builder_no_hit);
    assert!(
        ex_style_no_hit.contains(WS_EX_TRANSPARENT),
        "No-hittest window must have WS_EX_TRANSPARENT"
    );
}

#[test]
fn test_calc_window_rect_normal() {
    // 800x600 のインナークライアントサイズを指定
    let builder = WindowBuilder::new().with_inner_size(LogicalSize::new(800.0, 600.0));
    let style = WS_OVERLAPPEDWINDOW;
    let ex_style = WINDOW_EX_STYLE(0);

    // A. 96 DPI (100%等倍スケール)
    let (w_100, h_100) = calc_window_rect(style, ex_style, &builder, 96, 1.0)
        .expect("Failed to calculate window rect at 96 DPI");

    // 外寸（タイトルバーや枠線を含むサイズ）はインナーサイズ (800x600) より必ず大きくなる
    assert!(w_100 > 800, "Outer width must be larger than inner width");
    assert!(h_100 > 600, "Outer height must be larger than inner height");

    // B. 144 DPI (150%拡大スケール)
    let (w_150, h_150) = calc_window_rect(style, ex_style, &builder, 144, 1.5)
        .expect("Failed to calculate window rect at 144 DPI");

    // 150% 拡大時の外寸は等倍時の外寸よりも大きくなる
    assert!(
        w_150 > w_100,
        "Scaled outer width must be larger than unscaled outer width"
    );
    assert!(
        h_150 > h_100,
        "Scaled outer height must be larger than unscaled outer height"
    );
}

#[test]
fn test_get_class_name_utf16_normal() {
    // デフォルト名
    let builder = WindowBuilder::new();
    let name_wide = get_class_name_utf16("DefaultClass", &builder);
    let name_str = String::from_utf16(&name_wide[..name_wide.len() - 1]).unwrap();
    assert_eq!(name_str, "DefaultClass");

    // カスタムクラス名
    let builder_custom = WindowBuilder::new().with_custom_class_name("MyCustomClass");
    let name_custom_wide = get_class_name_utf16("DefaultClass", &builder_custom);
    let name_custom_str =
        String::from_utf16(&name_custom_wide[..name_custom_wide.len() - 1]).unwrap();
    assert_eq!(name_custom_str, "MyCustomClass");
}

#[test]
fn test_register_window_class_normal() {
    run_on_clean_thread(|| {
        let hmodule = unsafe { GetModuleHandleW(None).unwrap() };
        let hinstance = HINSTANCE(hmodule.0);

        let unique_class_str = format!("MichiuWindowTestClass_{}", unsafe {
            windows::Win32::System::Threading::GetCurrentThreadId()
        });
        let unique_class_wide: Vec<u16> = unique_class_str.encode_utf16().chain(Some(0)).collect();
        let class_name = PCWSTR(unique_class_wide.as_ptr());

        let builder = WindowBuilder::new();

        // 新規登録
        let res = register_window_class(class_name, "DefaultTestClass", hinstance, &builder);
        assert!(res.is_ok(), "Failed to register window class");

        // 重複登録が安全に無視されて Ok を返すか
        let res_dup = register_window_class(class_name, "DefaultTestClass", hinstance, &builder);
        assert!(
            res_dup.is_ok(),
            "Duplicate window class registration should return Ok"
        );
    });
}

#[test]
fn test_is_system_dark_mode_safe() {
    // レジストリ読み込み処理を実行して、アクセスエラーやパニックが発生せず
    // 安全に真偽値が取得できることを検証
    let _result = is_system_dark_mode();
}

#[test]
fn test_init_dpi_awareness_safe_and_exact() {
    // SetProcessDpiAwarenessContext はプロセスにつき一度しか設定できない。
    // すでに別テストで設定済みの場合は false になり、GetLastError() が ERROR_ACCESS_DENIED になる。
    let success = init_dpi_awareness();
    if !success {
        let last_error = unsafe { GetLastError() };
        assert_eq!(
            last_error, ERROR_ACCESS_DENIED,
            "If init_dpi_awareness fails, it must be because it was already set (ERROR_ACCESS_DENIED)."
        );
    }
}

#[test]
fn test_set_app_theme_all_modes_safe() {
    run_on_clean_thread(|| {
        // uxtheme.dll の動的ロードおよび内部関数（135, 136番）の呼び出しが
        // すべての PreferredAppMode パターンでクラッシュやパニックを起こさないか検証
        unsafe {
            set_app_theme(PreferredAppMode::Default);
            set_app_theme(PreferredAppMode::AllowDark);
            set_app_theme(PreferredAppMode::ForceDark);
            set_app_theme(PreferredAppMode::ForceLight);
        }
    });
}

#[test]
fn test_enable_dark_mode_titlebar_normal() {
    run_on_clean_thread(|| {
        unsafe {
            let hmodule = GetModuleHandleW(None).unwrap();
            let hinstance = HINSTANCE(hmodule.0);

            let class_name = w!("MichiuDwmTestDummyClass");
            let _ = register_window_class(
                class_name,
                "MichiuDwmTestDummyClass",
                hinstance,
                &WindowBuilder::new(),
            );

            // DWMを設定するための実HWNDダミーを生成
            let dummy_hwnd = CreateWindowExW(
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
                Some(hinstance),
                None,
            )
            .expect("Failed to create temporary window for DWM testing");

            // 有効化・無効化がOSのDWMAPIに安全に受理されるか検証
            enable_dark_mode_titlebar(dummy_hwnd, true);
            enable_dark_mode_titlebar(dummy_hwnd, false);

            // クリーンアップ
            let _ = DestroyWindow(dummy_hwnd);
        }
    });
}

#[test]
fn test_window_build_and_basic_properties_and_rwh_normal() {
    run_on_clean_thread(|| {
        // 有効な Window を構築
        let builder = WindowBuilder::new()
            .with_title("Michiu Core Window")
            .with_inner_size(LogicalSize::new(800.0, 600.0));

        let validated = builder
            .into_unvalidated()
            .try_into()
            .expect("Failed to validate builder");
        let window_result = Window::build(validated);
        assert!(
            window_result.is_ok(),
            "Window::build failed: {:?}",
            window_result.err()
        );

        let window = window_result.unwrap();

        // 基本プロパティが正しく取得できるかを検証
        assert!(
            !window.hwnd().is_invalid(),
            "HWND must be a valid non-null handle"
        );
        assert_eq!(
            window.id(),
            WindowId(window.hwnd().0 as isize),
            "WindowId must match raw HWND cast"
        );
        assert!(window.dpi() > 0, "DPI must be a positive integer");
        assert!(
            window.scale_factor() > 0.0,
            "Scale factor must be a positive float"
        );
        assert_eq!(
            window.thread_id(),
            unsafe { windows::Win32::System::Threading::GetCurrentThreadId() },
            "Window thread ID must match the current running UI thread"
        );

        // 各種プロパティ変更メソッドが、UIスレッド同期でエラーなく実行できるか検証
        window.set_title("New Custom Title");
        window.set_visible(false);
        window.set_size(PhysicalSize::new(1024, 768));
        window.set_position(PhysicalPoint::new(100, 100));
        window.set_theme(PreferredAppMode::ForceDark);

        // raw-window-handle (v0.6) に完全準拠しているかをアサーション検証
        // （wgpu, winit, egui などの外部GUI/描画エコシステムとの連携互換性を保証）
        let rwh_result = window.window_handle();
        assert!(rwh_result.is_ok(), "Failed to fetch raw window handle");

        let rwh_handle = rwh_result.unwrap();
        let raw_handle = rwh_handle.as_raw();

        match raw_handle {
            RawWindowHandle::Win32(win32_handle) => {
                // windows-rs が保持する HWND の生値と、rwh が返す get() の値が完全一致するか
                assert_eq!(
                    win32_handle.hwnd.get(),
                    window.hwnd().0 as isize,
                    "Win32 raw HWND must match Rust handle exactly"
                );

                // HINSTANCE の一致チェック
                if let Some(hinstance_handle) = win32_handle.hinstance {
                    assert_eq!(
                        hinstance_handle.get(),
                        window.hinstance().0 as isize,
                        "Win32 raw HINSTANCE must match Rust hinstance exactly"
                    );
                }
            }
            _ => panic!("Expected RawWindowHandle::Win32 variant on Windows platform"),
        }

        // raw-display-handle (v0.6) の検証
        let rdh_result = window.display_handle();
        assert!(rdh_result.is_ok(), "Failed to fetch raw display handle");

        let rdh_handle = rdh_result.unwrap();
        let raw_display = rdh_handle.as_raw();

        assert!(
            matches!(raw_display, RawDisplayHandle::Windows(_)),
            "Expected RawDisplayHandle::Windows variant on Windows platform"
        );

        // 明示的にウィンドウを破棄
        window.destroy();
    });
}

#[test]
fn test_window_handle_generation_and_validation_lifecycle() {
    run_on_clean_thread(|| {
        let builder = WindowBuilder::new().with_title("HandleLifecycleTestWindow");
        let window = Window::build(builder.into_unvalidated().try_into().unwrap()).unwrap();

        // 未検証ハンドル (Unvalidated<WindowHandle>) の切り出し
        let unvalidated_handle = window.handle();

        // ウィンドウが存命であるため、検証は正常に通過するはず
        let validated_handle_result = unvalidated_handle.try_into();
        assert!(
            validated_handle_result.is_ok(),
            "WindowHandle validation failed: {:?}",
            validated_handle_result.err()
        );

        let validated_handle: Validated<WindowHandle> = validated_handle_result.unwrap();

        // Validated<WindowHandle> が Deref を通じて安全にプロパティを読み取れるか
        assert_eq!(validated_handle.hwnd(), window.hwnd());
        assert_eq!(validated_handle.hinstance(), window.hinstance());
        assert_eq!(validated_handle.thread_id(), window.thread_id());
        assert_eq!(validated_handle.id(), window.id());

        // WindowHandle 自体の raw-window-handle (v0.6) インテグレーション検証
        let rwh_result = validated_handle.window_handle();
        assert!(rwh_result.is_ok());
        let raw_handle = rwh_result.unwrap().as_raw();
        match raw_handle {
            RawWindowHandle::Win32(win32_handle) => {
                assert_eq!(win32_handle.hwnd.get(), window.hwnd().0 as isize);
            }
            _ => panic!("Expected RawWindowHandle::Win32"),
        }

        // ウィンドウを破壊した状態を作り、ハンドルが死んだウィンドウを正しく検知できるかテストする
        window.destroy();

        // すでに解体済みのウィンドウハンドルを再度バリデーションにかける
        let raw_handle_recovered = Unvalidated::new(validated_handle.into_inner());
        let stale_validation_result: Result<Validated<WindowHandle>> =
            raw_handle_recovered.try_into();

        // OS上の破棄を検知して確実にエラー（InvalidHandleState）になることを検証
        assert!(
            stale_validation_result.is_err(),
            "Validation should fail after window destruction"
        );
        assert!(
            matches!(
                stale_validation_result.unwrap_err(),
                MichiuError::InvalidHandleState { .. }
            ),
            "Expected InvalidHandleState error"
        );
    });
}

unsafe extern "system" fn mock_user_subclass_proc(
    hwnd: HWND,
    msg: u32,
    wparam: WPARAM,
    lparam: LPARAM,
    #[allow(unused)] id_subclass: usize,
    ref_data: usize,
) -> LRESULT {
    // 特定のカスタムメッセージが来た場合のみ、インターセプトして書き換える
    if msg == WM_USER + 200 {
        let tracker_ptr = ref_data as *const AtomicBool;
        if !tracker_ptr.is_null() {
            // スレッドセーフに実行フラグを書き換える
            unsafe {
                (*tracker_ptr).store(true, Ordering::SeqCst);
            }
        }
        return LRESULT(0); // ここで処理完了とし、以降のWndProc(DefSubclassProc)には流さない
    }

    // 自分が処理したメッセージ以外は、必ず DefSubclassProc を呼び出して標準のプロシージャ（Rust側の wnd_proc 等）に
    // メッセージを流さなければならない。
    unsafe { DefSubclassProc(hwnd, msg, wparam, lparam) }
}

#[test]
fn test_raw_subclass_normal_and_abnormal_lifecycle() {
    run_on_clean_thread(|| {
        let builder = WindowBuilder::new().with_title("SubclassNormalTest");
        let window = Window::build(builder.into_unvalidated().try_into().unwrap()).unwrap();

        // サブクラス内でコールバックが実行されたかを追跡するアトミックフラグ
        let intercept_tracker = Arc::new(AtomicBool::new(false));
        let tracker_ptr = Arc::as_ptr(&intercept_tracker) as usize;

        // サブクラスID: 101 として登録をフックする
        let register_result =
            unsafe { window.raw_subclass(101, tracker_ptr, mock_user_subclass_proc) };
        assert!(
            register_result.is_ok(),
            "Subclass registration on active HWND should succeed"
        );

        // ウィンドウに対して、カスタムフックが待ち構えているメッセージを同期送信 (SendMessageW)
        unsafe {
            let _ = SendMessageW(
                window.hwnd(),
                WM_USER + 200,
                Some(WPARAM(0)),
                Some(LPARAM(0)),
            );
        }

        // アトミックフラグが立っており、インターセプトが成功しているかを検証
        assert!(
            intercept_tracker.load(Ordering::SeqCst),
            "User custom subclass failed to intercept the targeted message"
        );

        // ウィンドウを安全に破棄
        let raw_hwnd = window.hwnd();
        window.destroy(); // HWND は無効（ゾンビ状態）になる

        // すでに解体されて存在しない HWND に対して SetWindowSubclass の適用を試みる
        let stale_window = Window {
            hwnd: raw_hwnd,
            hinstance: HINSTANCE::default(),
            thread_id: 0,
            tray: None,
            drop_target: None,
            ime_relay: None,
            _marker: PhantomData,
        };

        let bad_register_result =
            unsafe { stale_window.raw_subclass(101, 0, mock_user_subclass_proc) };

        // OSがエラーを返し、Rust側で SubclassSetupFailed にマッピングできているかを検証
        assert!(
            bad_register_result.is_err(),
            "Subclass registration on a destroyed HWND must fail"
        );
        assert!(
            matches!(
                bad_register_result.unwrap_err(),
                MichiuError::SubclassSetupFailed { .. }
            ),
            "Expected SubclassSetupFailed error type"
        );
    });
}

#[test]
fn test_global_wnd_proc_message_filter_bypass_normal() {
    run_on_clean_thread(|| {
        let filter_executed = Arc::new(AtomicBool::new(false));
        let filter_executed_clone = filter_executed.clone();

        // WM_USER + 300 の場合に LRESULT(42) を返して処理を打ち切るフィルターを設定
        let builder = WindowBuilder::new()
            .with_title("MessageFilterTest")
            .with_message_filter(move |_hwnd, msg, _wparam, _lparam| {
                if msg == WM_USER + 300 {
                    filter_executed_clone.store(true, Ordering::SeqCst);
                    return Some(LRESULT(42));
                }
                None
            });

        let window = Window::build(builder.into_unvalidated().try_into().unwrap()).unwrap();

        // カスタムメッセージを同期送信
        let send_result = unsafe {
            SendMessageW(
                window.hwnd(),
                WM_USER + 300,
                Some(WPARAM(0)),
                Some(LPARAM(0)),
            )
        };

        // フィルターが優先実行され、指定した戻り値がOS側へ返されたか
        assert!(
            filter_executed.load(Ordering::SeqCst),
            "Custom message filter was not executed prior to default proc"
        );
        assert_eq!(
            send_result.0, 42,
            "Expected LRESULT(42) returned from the custom filter bypass, but got {}",
            send_result.0
        );

        window.destroy();
    });
}

#[test]
fn test_global_wnd_proc_min_max_info_dpi_scaling_normal() {
    run_on_clean_thread(|| {
        // 論理サイズ制限（Min: 400x300, Max: 800x600）を指定してビルド
        let builder = WindowBuilder::new()
            .with_title("MinMaxScalingTest")
            .with_min_size(LogicalSize::new(400.0, 300.0))
            .with_max_size(LogicalSize::new(800.0, 600.0));

        let window = Window::build(builder.into_unvalidated().try_into().unwrap()).unwrap();

        // OSの伸縮制限データ構造体 (MINMAXINFO) をスタックに確保
        let mut mmi = MINMAXINFO::default();

        // WM_GETMINMAXINFO メッセージを偽装送信
        unsafe {
            let _ = SendMessageW(
                window.hwnd(),
                WM_GETMINMAXINFO,
                Some(WPARAM(0)),
                Some(LPARAM(&mut mmi as *mut MINMAXINFO as isize)),
            );
        }

        // 現在のウインドウのDPI倍率を取得して、正しい物理ピクセルが書き戻されたかアサーション
        let scale = window.scale_factor();
        let expected_min_x = (400.0 * scale).round() as i32;
        let expected_min_y = (300.0 * scale).round() as i32;
        let expected_max_x = (800.0 * scale).round() as i32;
        let expected_max_y = (600.0 * scale).round() as i32;

        assert_eq!(
            mmi.ptMinTrackSize.x, expected_min_x,
            "ptMinTrackSize.x was not scaled properly based on DPI"
        );
        assert_eq!(
            mmi.ptMinTrackSize.y, expected_min_y,
            "ptMinTrackSize.y was not scaled properly based on DPI"
        );
        assert_eq!(
            mmi.ptMaxTrackSize.x, expected_max_x,
            "ptMaxTrackSize.x was not scaled properly based on DPI"
        );
        assert_eq!(
            mmi.ptMaxTrackSize.y, expected_max_y,
            "ptMaxTrackSize.y was not scaled properly based on DPI"
        );

        window.destroy();
    });
}

#[test]
fn test_window_build_with_drag_and_drop_lifecycle() {
    run_on_clean_thread(|| {
        let com_ctx = ComContext::new_com_single().unwrap();
        let builder = WindowBuilder::new()
            .with_title("DndWindowBuildTest")
            .with_drag_and_drop(true)
            .with_com_context(&com_ctx);

        let validated = builder.validate_into().unwrap();
        let window_res = Window::build(validated);
        assert!(
            window_res.is_ok(),
            "Failed to build window with drag_and_drop: {:?}",
            window_res.err()
        );

        let window = window_res.unwrap();

        // 内部の drop_target COMインターフェースが正しく初期化されているか
        assert!(
            window.drop_target.is_some(),
            "Drop target should be registered on build"
        );

        // 解体処理 (内部で RevokeDragDrop が安全に実行されるかの検証)
        window.destroy();
    });
}

#[test]
fn test_window_clipboard_synchronous_normal() {
    run_on_clean_thread(|| {
        // クリップボード操作には OLE STA が必須なので com_single で初期化
        let com_ctx = ComContext::new_com_single().unwrap();
        let builder = WindowBuilder::new()
            .with_title("ClipboardSyncTest")
            .with_com_context(&com_ctx);
        let window = Window::build(builder.validate_into().unwrap()).unwrap();

        let test_text = "Michiu Framework Clipboard Sync Verification Text";

        // クリップボードに書き込み
        let write_res = window.set_clipboard_text(test_text);
        assert!(
            write_res.is_ok(),
            "Failed to write text to clipboard: {:?}",
            write_res.err()
        );

        // クリップボードから読み出し
        let read_res = window.get_clipboard_text();
        assert!(
            read_res.is_ok(),
            "Failed to read text from clipboard: {:?}",
            read_res.err()
        );

        // 書き込んだ値と読み出した値が完全一致することを確認
        assert_eq!(read_res.unwrap(), test_text, "Clipboard content mismatch");

        window.destroy();
    });
}

#[test]
fn test_window_cursor_and_monitor_apis_synchronous_normal() {
    run_on_clean_thread(|| {
        let builder = WindowBuilder::new().with_title("CursorAndMonitorTest");
        let window = Window::build(builder.validate_into().unwrap()).unwrap();

        // カーソルキャプチャの実行確認 (パニックせずに終了するか)
        window.set_cursor_capture(true);
        window.set_cursor_capture(false);

        // カーソルクリッピングの実行確認
        window.set_cursor_clipping(true);
        window.set_cursor_clipping(false);

        // 画面中央配置の実行確認
        window.center_on_screen();

        // フルスクリーンのトグル実行確認
        window.set_fullscreen(true);
        window.set_fullscreen(false);

        window.destroy();
    });
}

#[test]
fn test_safe_subclass_intercept_and_continue_lifecycle() {
    run_on_clean_thread(|| {
        let builder = WindowBuilder::new().with_title("SafeSubclassTestWindow");
        let window = Window::build(builder.validate_into().unwrap()).unwrap();

        let intercept_called = Arc::new(AtomicBool::new(false));
        let intercept_called_clone = intercept_called.clone();

        let continue_called = Arc::new(AtomicBool::new(false));
        let continue_called_clone = continue_called.clone();

        // 独自のメッセージ (WM_USER + 400/401) をフックする安全なサブクラスを登録
        window
            .subclass(201, move |_hwnd, msg, _wparam, _lparam| {
                if msg == WM_USER + 400 {
                    intercept_called_clone.store(true, Ordering::SeqCst);
                    // ここで処理を完全にインターセプト
                    return SubclassResult::Intercept(LRESULT(99));
                }
                if msg == WM_USER + 401 {
                    continue_called_clone.store(true, Ordering::SeqCst);
                    // 処理をせずに次に流す
                    return SubclassResult::Continue;
                }
                SubclassResult::Continue
            })
            .expect("Failed to register safe subclass");

        // Intercept テスト (WM_USER + 400)
        // 指定した戻り値 (LRESULT(99)) が同期的に正しく返るか検証
        let res_intercept = unsafe {
            SendMessageW(
                window.hwnd(),
                WM_USER + 400,
                Some(WPARAM(0)),
                Some(LPARAM(0)),
            )
        };
        assert!(intercept_called.load(Ordering::SeqCst));
        assert_eq!(
            res_intercept.0, 99,
            "Expected custom subclass intercept LRESULT to be 99"
        );

        // Continue テスト (WM_USER + 401)
        // サブクラスハンドラを通過したうえで、後ろのメインWndProcにも伝播してクラッシュしないか検証
        let _res_continue = unsafe {
            SendMessageW(
                window.hwnd(),
                WM_USER + 401,
                Some(WPARAM(0)),
                Some(LPARAM(0)),
            )
        };
        assert!(continue_called.load(Ordering::SeqCst));

        // 安全に破棄（このタイミングで WM_NCDESTROY が走り、クロージャが安全に自動解放される）
        window.destroy();
    });
}
