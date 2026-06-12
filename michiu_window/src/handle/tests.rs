use super::*;
use crate::{ComContext, EventPump, Window, WindowBuilder};
use michiu_guard::{Unvalidated, Validate, Validated};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use windows::Win32::UI::WindowsAndMessaging::GetDesktopWindow;

fn run_on_clean_thread<F>(f: F)
where
    F: FnOnce() + Send + 'static,
{
    let handle = std::thread::spawn(f);
    handle.join().expect("Test thread panicked");
}

#[test]
fn test_handle_properties_normal() {
    run_on_clean_thread(|| {
        let builder = WindowBuilder::new().with_title("PropertyTest");
        let window = Window::build(builder.validate_into().unwrap()).unwrap();

        // WindowHandle の生成
        let unvalidated_handle = window.handle();
        let handle = unvalidated_handle.assume_valid(); // 検証済みと仮定して取り出し

        // ゲッターが返す値が Window 本体と完全に一致するか検証
        assert_eq!(handle.hwnd(), window.hwnd());
        assert_eq!(handle.hinstance(), window.hinstance());
        assert_eq!(handle.thread_id(), window.thread_id());
        assert_eq!(handle.id(), window.id());

        window.destroy();
    });
}

#[test]
fn test_handle_thread_affinity_checks() {
    run_on_clean_thread(|| {
        let builder = WindowBuilder::new().with_title("AffinityTest");
        let window = Window::build(builder.validate_into().unwrap()).unwrap();
        let handle = window.handle().assume_valid();

        // UIスレッド（作成元スレッド）上での検証
        assert!(
            handle.is_on_ui_thread(),
            "Handle should report being on the UI thread"
        );
        assert!(handle.assert_ui_thread().is_ok());

        // バックグラウンドスレッド（作成元と異なるスレッド）上での検証
        let handle_clone = handle.clone();
        let thread_handle = std::thread::spawn(move || {
            // 他スレッドなので false を返すはず
            assert!(
                !handle_clone.is_on_ui_thread(),
                "Handle should report not being on the UI thread in foreign thread"
            );

            // Mismatch エラーになるはず
            let assert_result = handle_clone.assert_ui_thread();
            assert!(assert_result.is_err());
            assert!(matches!(
                assert_result.unwrap_err(),
                MichiuError::ThreadMismatch { .. }
            ));
        });
        thread_handle.join().unwrap();

        window.destroy();
    });
}

#[test]
fn test_handle_validation_scenarios() {
    run_on_clean_thread(|| {
        // 正常（存命ウインドウ）
        let builder = WindowBuilder::new().with_title("ValidationTest");
        let window = Window::build(builder.validate_into().unwrap()).unwrap();
        let handle = window.handle();

        let val_res: Result<Validated<WindowHandle>> = handle.clone().try_into();
        assert!(
            val_res.is_ok(),
            "Validation of alive window handle should succeed"
        );

        // 異常（破棄済み）
        let raw_hwnd = window.hwnd();
        window.destroy(); // 物理破棄

        let zombie_handle = WindowHandle {
            hwnd: raw_hwnd,
            hinstance: HINSTANCE::default(),
            thread_id: 0,
        };
        let val_res_zombie = zombie_handle.validate();
        assert!(
            val_res_zombie.is_err(),
            "Validation must fail for destroyed window"
        );
        assert!(matches!(
            val_res_zombie.unwrap_err(),
            MichiuError::InvalidHandleState { .. }
        ));

        // 異常（他プロセスのHWND再利用検知）
        // Windows デスクトップの HWND は確実に別プロセス (PID) が所有している。
        let desktop_hwnd = unsafe { GetDesktopWindow() };
        let foreign_handle = WindowHandle {
            hwnd: desktop_hwnd,
            hinstance: HINSTANCE::default(),
            thread_id: 0,
        };

        let val_res_foreign = foreign_handle.validate();
        assert!(
            val_res_foreign.is_err(),
            "Validation must fail for window owned by another PID"
        );
        match val_res_foreign.unwrap_err() {
            MichiuError::ValidationError { parameter, message } => {
                assert_eq!(parameter, "WindowHandle");
                assert!(message.contains("recycled by another process"));
            }
            other => panic!("Expected ValidationError, got: {:?}", other),
        }
    });
}

#[test]
fn test_handle_run_on_ui_thread_synchronous() {
    run_on_clean_thread(|| {
        let builder = WindowBuilder::new().with_title("SyncRunTest");
        let window = Window::build(builder.into_unvalidated().try_into().unwrap()).unwrap();
        let handle = window.handle().assume_valid();

        let executed = Arc::new(AtomicBool::new(false));
        let executed_clone = executed.clone();
        // スレッド安全な isize として切り出す。
        let expected_hwnd_val = window.hwnd().0 as isize;

        // UIスレッド上で run_on_ui_thread を呼ぶ
        unsafe {
            handle.run_on_ui_thread(move |hwnd| {
                // クロージャに渡された hwnd も isize にキャストして比較する
                assert_eq!(hwnd.0 as isize, expected_hwnd_val);
                executed_clone.store(true, Ordering::SeqCst);
            });
        }

        // 同期パスなので、PostMessage メッセージループを経由せず、
        // この行に到達した時点で即座に実行完了しているはず
        assert!(
            executed.load(Ordering::SeqCst),
            "Closure was not executed synchronously on the UI thread"
        );

        window.destroy();
    });
}

#[test]
fn test_handle_clipboard_read_from_foreign_thread_normal() {
    run_on_clean_thread(|| {
        // OLE STA の初期化
        let com_ctx = ComContext::new_com_single().unwrap();
        let builder = WindowBuilder::new()
            .with_title("HandleClipboardTest")
            .with_com_context(&com_ctx);
        let window = Window::build(builder.validate_into().unwrap()).unwrap();
        let handle = window.handle().assume_valid();

        let test_text = "Cross-thread clipboard transfer data";

        // UIスレッド側からテキストを書き込む
        window.set_clipboard_text(test_text).unwrap();

        // バックグラウンドスレッドを立ち上げる
        let handle_clone = handle.clone();
        let thread_handle = std::thread::spawn(move || {
            // バックグラウンドスレッドから get_clipboard_text() を呼び出し、
            // スレッドアフィニティに衝突せず同期的に安全に読み戻せるか検証
            let read_res = handle_clone.get_clipboard_text();
            assert!(
                read_res.is_ok(),
                "Failed to read clipboard from background thread: {:?}",
                read_res.err()
            );
            assert_eq!(read_res.unwrap(), "Cross-thread clipboard transfer data");
        });
        thread_handle.join().unwrap();

        window.destroy();
    });
}

#[test]
fn test_handle_wake_up_with_mpsc_channel_normal() {
    run_on_clean_thread(|| {
        let builder = WindowBuilder::new().with_title("WakeUpTestWindow");
        let window = Window::build(builder.validate_into().unwrap()).unwrap();
        let handle = window.handle().assume_valid();

        // 標準の MPSC チャネルを用意
        let (tx, rx) = std::sync::mpsc::channel();

        // バックグラウンドスレッドを立ち上げてデータを送信し、wake_up を実行する
        let handle_clone = handle.clone();
        let bg_thread = std::thread::spawn(move || {
            tx.send("Wake Up Signal!").unwrap();
            // メッセージループを叩き起こして回収パスに到達させる
            handle_clone.wake_up();
        });

        // バックグラウンドスレッドの処理完了を同期
        bg_thread.join().expect("Background thread panicked");

        // UIスレッド側のメッセージループ
        let mut event_pump = EventPump::new();
        let start_time = std::time::Instant::now();
        let mut signal_received = false;

        while start_time.elapsed() < std::time::Duration::from_secs(2) {
            // wake_up() によって送られた WM_NULL がメッセージループを回し、
            // 即座にチャネルの try_recv のチェックに処理を到達させる
            let _ = event_pump.poll_event();

            if let Ok(msg) = rx.try_recv() {
                assert_eq!(msg, "Wake Up Signal!");
                signal_received = true;
                break;
            }
            std::thread::sleep(std::time::Duration::from_millis(10));
        }

        // wake_up によってメッセージループが活性化し、チャネルから正しくデータが回収できたことことを確認
        assert!(
            signal_received,
            "Failed to wake up the message loop and retrieve the message from the channel within timeout"
        );

        window.destroy();
    });
}

#[test]
fn test_handle_asynchronous_destroy_and_zombie_validation() {
    run_on_clean_thread(|| {
        let builder = WindowBuilder::new().with_title("AsyncDestroyTestWindow");
        let window = Window::build(builder.validate_into().unwrap()).unwrap();
        let handle = window.handle().assume_valid();

        // 初期状態では当然 IsWindow は true
        assert!(unsafe { IsWindow(Some(handle.hwnd())).as_bool() });

        // バックグラウンドスレッドから非同期に物理破棄を要求させる
        let handle_clone = handle.clone();
        let bg_thread = std::thread::spawn(move || {
            // 他スレッドから安全に destroy をキック
            handle_clone.destroy();
        });
        bg_thread.join().expect("Background thread panicked");

        // メッセージループを回して、UIスレッド側で非同期破棄コマンドを実行させる
        let mut event_pump = EventPump::new();
        let start_time = std::time::Instant::now();
        let mut window_destroyed = false;

        while start_time.elapsed() < std::time::Duration::from_secs(2) {
            let _ = event_pump.poll_event();

            // OSの IsWindow APIを用いて、ウィンドウが物理的に解体されたか監視
            let is_alive = unsafe {
                windows::Win32::UI::WindowsAndMessaging::IsWindow(Some(handle.hwnd())).as_bool()
            };
            if !is_alive {
                window_destroyed = true;
                break;
            }
            std::thread::sleep(std::time::Duration::from_millis(10));
        }

        // バックグラウンドからの非同期 destroy 要求により、無事に物理破棄が完了したことを確認
        assert!(
            window_destroyed,
            "Asynchronous destroy command failed to destroy the window"
        );

        // 破棄されたあとのハンドルは、バリデーションでゾンビを即座に検知してエラーを返すことを確認
        let validation_res: Result<Validated<WindowHandle>> =
            Unvalidated::new(handle.into_inner()).try_into();
        assert!(
            validation_res.is_err(),
            "A handle to a destroyed window must fail validation"
        );
        assert!(
            matches!(
                validation_res.unwrap_err(),
                MichiuError::InvalidHandleState { .. }
            ),
            "Expected InvalidHandleState error"
        );
    });
}
