use michiu_window::{EventPump, LogicalSize, WindowBuilder, init_dpi_awareness};

// cargo test --test basic_lifecycle

fn run_on_clean_thread<F>(f: F)
where
    F: FnOnce() + Send + 'static,
{
    let handle = std::thread::spawn(f);
    handle.join().expect("Test thread panicked");
}

#[test]
fn test_integration_minimal_lifecycle() {
    // プロセスレベルのDPI認識の初期化
    let _ = init_dpi_awareness();

    // COMやSTA等のスレッド初期化への干渉を防ぐためクリーンなスレッドで実行
    run_on_clean_thread(|| {
        // ビルダーの組み立てと検証
        let builder = WindowBuilder::new()
            .with_title("Michiu Integration Test")
            .with_inner_size(LogicalSize::new(400.0, 300.0));

        let validated = builder
            .into_unvalidated()
            .try_into()
            .expect("Integration builder validation failed");

        // ウィンドウ構築
        let window = michiu_window::Window::build(validated)
            .expect("Failed to build window in integration context");

        // 基本的なプロパティチェックと外部属性更新
        assert!(window.dpi() > 0, "DPI should be a positive value");
        assert!(
            window.scale_factor() > 0.0,
            "Scale factor should be a positive value"
        );

        // タイトルを動的に変更
        window.set_title("Michiu Integration - Title Updated");

        // イベントポンプの最小限の駆動テスト
        let mut event_pump = EventPump::new();
        // 最初のポーリング
        let _event = event_pump.poll_event();

        // ウィンドウの明示的な破棄
        window.destroy();
    });
}
