use michiu_window::{ComContext, LogicalSize, MichiuError, WindowBuilder, init_dpi_awareness};
use windows::Win32::System::DataExchange::{CloseClipboard, OpenClipboard};

// cargo test --test clipboard_conflict

fn run_on_clean_thread<F>(f: F)
where
    F: FnOnce() + Send + 'static,
{
    let handle = std::thread::spawn(f);
    handle.join().expect("Test thread panicked");
}

#[test]
fn test_integration_clipboard_lock_conflict_and_recovery() {
    let _ = init_dpi_awareness();

    run_on_clean_thread(|| {
        let com_ctx = ComContext::new_com_single().expect("Failed to initialize OLE STA context");
        let builder = WindowBuilder::new()
            .with_title("Clipboard Conflict Test")
            .with_com_context(&com_ctx)
            .with_visible(false)
            .with_inner_size(LogicalSize::new(200.0, 150.0));

        let window =
            michiu_window::Window::build(builder.into_unvalidated().try_into().unwrap()).unwrap();

        // バックグラウンドを立ち上げクリップボードを排他ロックする
        let bg_thread = std::thread::spawn(move || {
            unsafe {
                // メインウィンドウとは異なるコンテキストとしてクリップボードをオープンしてロック
                let res = OpenClipboard(None);
                assert!(
                    res.is_ok(),
                    "Failed to force lock clipboard from background thread"
                );

                // UIスレッドが衝突テストを実行するまでロックをしばらく維持
                std::thread::sleep(std::time::Duration::from_millis(500));

                let _ = CloseClipboard();
            }
        });

        // バックグラウンドスレッドが確実に OpenClipboard を呼ぶまでウェイト
        std::thread::sleep(std::time::Duration::from_millis(100));

        // ロック状態で、UIスレッド側からクリップボード書き込みを実行
        // ロックが解除されるまで書き込めないため、エラーを返す必要がある
        // 失敗時の `GlobalFree` 漏れ防止が機能し、
        // 且つクラッシュせず安全に UnexpectedOsError にマッピングされていることを検証
        let set_res = window.set_clipboard_text("Attempt to write under conflict");
        assert!(
            set_res.is_err(),
            "Clipboard write should have failed due to conflict lock"
        );

        match set_res.unwrap_err() {
            MichiuError::UnexpectedOsError(err) => {
                // Windowsのエラーコード（GetLastWin32Error / E_FAIL など）が正常にラップされている
                println!(
                    "Safely intercepted clipboard write conflict: {:?}",
                    err.message()
                );
            }
            other => panic!(
                "Expected UnexpectedOsError under clipboard conflict, but got {:?}",
                other
            ),
        }

        // 同様にロック状態での読み込み失敗のハンドリングを検証
        let get_res = window.get_clipboard_text();
        assert!(
            get_res.is_err(),
            "Clipboard read should have failed due to conflict lock"
        );
        assert!(matches!(
            get_res.unwrap_err(),
            MichiuError::UnexpectedOsError(_)
        ));

        // バックグラウンドスレッドの終了とロック解放を同期
        bg_thread.join().expect("Background thread panicked");

        // ロック解除後、速やかに通常の読み書きが正常に機能することを検証
        let success_write = window.set_clipboard_text("Normal Write After Conflict Resolution");
        assert!(
            success_write.is_ok(),
            "Failed to write after clip lock resolution"
        );

        let success_read = window.get_clipboard_text();
        assert!(
            success_read.is_ok(),
            "Failed to read after clip lock resolution"
        );
        assert_eq!(
            success_read.unwrap(),
            "Normal Write After Conflict Resolution",
            "Read content mismatched the written text after recovery"
        );

        window.destroy();
    });
}
