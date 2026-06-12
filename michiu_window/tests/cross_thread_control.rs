use michiu_window::{EventPump, LogicalSize, WindowBuilder, init_dpi_awareness};
use std::{
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
    time::{Duration, Instant},
};

// cargo test --test cross_thread_control
//
// Win32 API の GetWindowTextW は通常のローカル環境では動作するものの、
// デスクトップセッションが制限されている環境や、ウィンドウが完全に構築されて表示された直後のタイミングでは、
// OS内部の同期遅延によって 0 を返して失敗する

fn run_on_clean_thread<F>(f: F)
where
    F: FnOnce() + Send + 'static,
{
    let handle = std::thread::spawn(f);
    handle.join().expect("Test thread panicked");
}

#[test]
fn test_integration_cross_thread_control_lifecycle() {
    let _ = init_dpi_awareness();

    run_on_clean_thread(|| {
        // UIスレッドで初期ウィンドウを生成
        let builder = WindowBuilder::new()
            .with_title("Initial Title")
            .with_inner_size(LogicalSize::new(400.0, 300.0));

        let validated = builder
            .into_unvalidated()
            .try_into()
            .expect("Integration builder validation failed");

        let window = michiu_window::Window::build(validated).expect("Failed to build window");

        // スレッドセーフな WindowHandle の切り出しとクローン
        let unvalidated_handle = window.handle();
        let handle = unvalidated_handle.assume_valid();
        let handle_clone = handle.clone();

        // 実行状態を追跡するためのアトミックフラグ
        let executed = Arc::new(AtomicBool::new(false));
        let executed_clone = executed.clone();

        // バックグラウンドスレッドを立ち上げてウィンドウを操作
        let bg_thread = std::thread::spawn(move || {
            // UIスレッドとは異なるスレッドからタイトル変更を要求
            // 内部で自動的に PostMessageW(WM_WINDOW_COMMAND) の非同期経路が走る
            unsafe {
                handle_clone.run_on_ui_thread(move |_hwnd| {
                    executed_clone.store(true, Ordering::SeqCst);
                });
            }
        });

        // バックグラウンドスレッドの処理が確実に完了するのを待つ
        bg_thread.join().expect("Background thread panicked");

        // UIスレッド側でメッセージループを回して非同期コマンドを反映
        let mut event_pump = EventPump::new();
        let start_time = Instant::now();

        // ポストされたクロージャがUIスレッド上で安全に実行され、
        // アトミックフラグが true に書き換わるまでメッセージポンプを回す
        while start_time.elapsed() < Duration::from_secs(2) {
            let _ = event_pump.poll_event();

            if executed.load(Ordering::SeqCst) {
                break;
            }
            std::thread::sleep(Duration::from_millis(10));
        }

        // バックグラウンドからの非同期変更がUIスレッドのループによって正しく適用されたことを確認
        assert!(
            executed.load(Ordering::SeqCst),
            "Failed to execute background thread's closure on the UI thread via message pump"
        );

        // クリーンアップ
        window.destroy();
    });
}
