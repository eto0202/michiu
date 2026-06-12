use michiu_window::{EventPump, LogicalSize, WindowBuilder, init_dpi_awareness};
use std::sync::mpsc;
use std::time::{Duration, Instant};

// cargo test --test event_driven

// アプリケーションが集中管理するコマンド列挙型
enum AppCommand {
    UpdateTitle(String),
}

fn run_on_clean_thread<F>(f: F)
where
    F: FnOnce() + Send + 'static,
{
    let handle = std::thread::spawn(f);
    handle.join().expect("Test thread panicked");
}

#[test]
fn test_integration_event_driven_channel_lifecycle() {
    let _ = init_dpi_awareness();

    run_on_clean_thread(|| {
        // UIスレッドで初期ウィンドウを生成
        let builder = WindowBuilder::new()
            .with_title("Original Title")
            .with_inner_size(LogicalSize::new(400.0, 300.0));

        let validated = builder
            .into_unvalidated()
            .try_into()
            .expect("Integration builder validation failed");

        let window = michiu_window::Window::build(validated).expect("Failed to build window");

        let handle = window.handle().assume_valid();
        let handle_clone = handle.clone();

        // 標準の MPSC チャネルを準備する
        let (tx, rx) = mpsc::channel();

        // バックグラウンドスレッドを立ち上げて、チャネルにタスクを投げて起床を要求
        let bg_thread = std::thread::spawn(move || {
            // バックグラウンドで非同期データ処理が行われたと仮定し、メインスレッドへの指示を送信
            tx.send(AppCommand::UpdateTitle(
                "Title via EventDriven Channel".to_string(),
            ))
            .expect("Failed to send command to channel");

            // チャネルにタスクが蓄積されたため、UIスレッドを即座に起こす
            handle_clone.wake_up();
        });

        // バックグラウンドスレッドの処理完了を同期
        bg_thread.join().expect("Background thread panicked");

        // UIスレッド側のメッセージループ
        let mut event_pump = EventPump::new();
        let start_time = Instant::now();
        let mut command_executed = false;

        // wake_up() で送られたシグナル（WM_NULL）により、
        // スリープ中のメッセージループが確実に1周し、チャネルの try_recv 処理に到達します。
        while start_time.elapsed() < Duration::from_secs(2) {
            // メッセージポンプを1回転させる
            let _ = event_pump.poll_event();

            // 起床したループ内で、チャネルからタスクを取り出して安全に処理（UIスレッド同期更新）
            while let Ok(cmd) = rx.try_recv() {
                match cmd {
                    AppCommand::UpdateTitle(new_title) => {
                        // UIスレッドのコンテキスト上で直接ウィンドウを安全に変更します
                        window.set_title(new_title);
                        command_executed = true;
                    }
                }
            }

            if command_executed {
                break;
            }
            std::thread::sleep(Duration::from_millis(10));
        }

        // チャネルを介したイベント駆動タスクが、UIスレッド上で安全かつ即座に処理されたことを確認
        assert!(
            command_executed,
            "Failed to wake up message loop and process the background channel command"
        );

        // クリーンアップ
        window.destroy();
    });
}
