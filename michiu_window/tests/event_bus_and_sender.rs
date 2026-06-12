use michiu_window::{
    EventBus, EventPump, LogicalSize, MichiuEvent, WindowBuilder, init_dpi_awareness,
};
use std::time::{Duration, Instant};

// cargo test --test event_bus_and_sender

// 同一UIスレッド内でブロードキャストするカスタムイベント
#[derive(Clone, Debug)]
struct TriggerTaskEvent {
    task_id: u32,
}

// バックグラウンドからメインウィンドウAに直接ポストバックする結果イベント
struct TaskResult {
    from: &'static str,
    result_val: u32,
}

fn run_on_clean_thread<F>(f: F)
where
    F: FnOnce() + Send + 'static,
{
    let handle = std::thread::spawn(f);
    handle.join().expect("Test thread panicked");
}

#[test]
fn test_integration_event_bus_and_sender_cooperation() {
    let _ = init_dpi_awareness();

    run_on_clean_thread(|| {
        // シングルスレッド専用イベントバスを生成
        let event_bus = EventBus::new();

        // 3つのウィンドウを同じUIスレッド上に生成
        let window_a = WindowBuilder::new()
            .with_title("Window A")
            .with_visible(false)
            .with_inner_size(LogicalSize::new(200.0, 150.0))
            .into_unvalidated()
            .try_into()
            .and_then(michiu_window::Window::build)
            .unwrap();

        let window_b = WindowBuilder::new()
            .with_title("Window B")
            .with_visible(false)
            .with_inner_size(LogicalSize::new(200.0, 150.0))
            .into_unvalidated()
            .try_into()
            .and_then(michiu_window::Window::build)
            .unwrap();

        let window_c = WindowBuilder::new()
            .with_title("Window C")
            .with_visible(false)
            .with_inner_size(LogicalSize::new(200.0, 150.0))
            .into_unvalidated()
            .try_into()
            .and_then(michiu_window::Window::build)
            .unwrap();

        // メインウィンドウAに直接結果をポストするための EventSender を準備
        let sender_to_a = window_a.handle().assume_valid().sender();

        // ウィンドウBの振る舞い（EventBusの購読 ＆ 非同期スレッド実行）
        let sender_for_b = sender_to_a.clone();
        event_bus.subscribe(move |event: &TriggerTaskEvent| {
            let sender = sender_for_b.clone();
            let task_id = event.task_id;

            // 擬似的に重い処理を行うバックグラウンドスレッドを立ち上げる
            std::thread::spawn(move || {
                std::thread::sleep(Duration::from_millis(50)); // 重い処理

                // 完了後、直接ウィンドウAへ結果をポストバック
                sender.send_event(TaskResult {
                    from: "Window B",
                    result_val: task_id * 10, // 独自の計算
                });
            });
        });

        // ウィンドウCの振る舞い（同様に購読 ＆ 非同期スレッド実行）
        let sender_for_c = sender_to_a.clone();
        event_bus.subscribe(move |event: &TriggerTaskEvent| {
            let sender = sender_for_c.clone();
            let task_id = event.task_id;

            std::thread::spawn(move || {
                std::thread::sleep(Duration::from_millis(100));

                // 完了後、直接ウィンドウAへ結果をポストバック
                sender.send_event(TaskResult {
                    from: "Window C",
                    result_val: task_id * 20,
                });
            });
        });

        // メインウィンドウAが、イベントバスを用いて一斉にタスク開始をブロードキャスト（UIスレッド上）
        event_bus.publish(&TriggerTaskEvent { task_id: 7 });

        // メインウィンドウAのイベントループを回し、バックグラウンドから返ってきた結果を一極回収
        let mut event_pump = EventPump::new();
        let start_time = Instant::now();
        let mut b_completed = false;
        let mut c_completed = false;

        while start_time.elapsed() < Duration::from_secs(3) {
            // 各バックグラウンドから返ってきた WM_USER_EVENT を回収する
            if let Some(event) = event_pump.poll_event()
                && let MichiuEvent::UserEvent(boxed_any) = event
            {
                // 送信されてきた TaskResult 型にダウンキャスト
                if let Ok(res) = boxed_any.downcast::<TaskResult>() {
                    match res.from {
                        "Window B" => {
                            assert_eq!(res.result_val, 70, "Window B calculated values mismatched");
                            b_completed = true;
                        }
                        "Window C" => {
                            assert_eq!(
                                res.result_val, 140,
                                "Window C calculated values mismatched"
                            );
                            c_completed = true;
                        }
                        _ => {}
                    }
                }
            }

            if b_completed && c_completed {
                break;
            }
            std::thread::sleep(Duration::from_millis(10));
        }

        // 双方向のブロードキャスト＆ポストバック連携が完璧に完結したことを確認
        assert!(
            b_completed,
            "Failed to capture or verify the result from Window B's background task"
        );
        assert!(
            c_completed,
            "Failed to capture or verify the result from Window C's background task"
        );

        // クリーンアップ
        window_a.destroy();
        window_b.destroy();
        window_c.destroy();
    });
}
