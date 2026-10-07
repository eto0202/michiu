#![allow(clippy::unwrap_used)]
#![allow(clippy::expect_used)]
#![allow(clippy::panic)]

use michiu_window::{
    LogicalSize, MichiuAnyEvent, MichiuEvent, MichiuEventPump, MichiuWindow, MichiuWindowBuilder,
    PhysicalSize,
};
use std::time::{Duration, Instant};
use windows::Win32::{
    Foundation::{LPARAM, WPARAM},
    UI::WindowsAndMessaging::{PostMessageW, WM_CLOSE, WM_SIZE, WM_USER},
};

// cargo test --test event_posting

// テスト用のカスタムペイロード構造体
struct MyCustomPayload {
    label: String,
    id: u32,
}

fn run_on_clean_thread<F>(f: F)
where
    F: FnOnce() + Send + 'static,
{
    let handle = std::thread::spawn(f);
    handle.join().expect("Test thread panicked");
}

#[allow(clippy::too_many_lines)]
#[test]
fn test_integration_event_posting_and_translation_lifecycle() {
    let _ = MichiuWindow::init_dpi_awareness();

    run_on_clean_thread(|| {
        let builder = MichiuWindowBuilder::new()
            .with_title("Event Posting Test")
            .with_inner_size(LogicalSize::new(400.0, 300.0));

        let validated = builder
            .into_unvalidated()
            .try_into()
            .expect("Integration builder validation failed");

        let window = michiu_window::MichiuWindow::build(validated).expect("Failed to build window");

        // スレッドセーフな WindowHandle を取得
        let unvalidated_handle = window.handle();
        let handle = unvalidated_handle.assume_valid();

        // MichiuEventSender を取得
        let sender = window.handle().assume_valid().sender();
        let handle_clone = handle.clone();

        // バックグラウンドスレッドを立ち上げて、各種メッセージをポストする
        let bg_thread = std::thread::spawn(move || {
            // MichiuEventSender によるカスタムイベント
            sender.send_event(MyCustomPayload {
                label: "Payload from background".to_string(),
                id: 42,
            });

            // スレッドアフィニティに違反せず、安全に WindowHandle 経由で HWND を取得
            let bg_hwnd = handle_clone.hwnd();

            // 通常のウィンドウメッセージ (WM_CLOSE) の生ポスト
            // UIスレッド側で Event::CloseRequested に翻訳
            unsafe {
                let _ = PostMessageW(Some(bg_hwnd), WM_CLOSE, WPARAM(0), LPARAM(0));
            }

            // 未知のユーザー定義メッセージ (WM_USER + 888) の生ポスト
            // UIスレッド側で Event::UnsafeRaw に翻訳
            unsafe {
                let _ = PostMessageW(Some(bg_hwnd), WM_USER + 888, WPARAM(777), LPARAM(999));
            }

            // 標準ウィンドウメッセージ (WM_SIZE) の生ポスト
            // LPARAM に幅 800 (下位16ビット)、高さ 600 (上位16ビット) をビットパッキング
            // UIスレッド側で Event::Resized(Unvalidated<PhysicalSize>) に翻訳
            let size_lparam = LPARAM(0b11_0010_0000 | (600 << 16));
            unsafe {
                let _ = PostMessageW(Some(bg_hwnd), WM_SIZE, WPARAM(0), size_lparam);
            }
        });

        // ポスト完了を同期
        bg_thread.join().expect("Background thread panicked");

        // UIスレッド側でメッセージループを回してイベントを回収・アサーション
        let mut event_pump = MichiuEventPump::new();
        let start_time = Instant::now();

        let mut custom_event_received = false;
        let mut close_event_received = false;
        let mut raw_event_received = false;
        let mut size_event_received = false;

        // 3つの異なるイベントがすべて順番に正しく回収されるまで、メッセージループを回す
        while start_time.elapsed() < Duration::from_secs(2) {
            event_pump.poll_event(|event, _, raw| {
                // OSが自発的に送信した無関係なシステムメッセージ（NCCREATE: 129 など）は無視し、
                // 自前でポストした 1912 (WM_USER+888) のメッセージが届いた時のみ検証
                if raw.msg == WM_USER + 888 {
                    assert_eq!(raw.wparam.0, 777);
                    assert_eq!(raw.lparam.0, 999);
                    raw_event_received = true;
                }

                match event {
                    MichiuAnyEvent::User(boxed_any) => {
                        let downcasted = boxed_any.downcast::<MyCustomPayload>();
                        assert!(
                            downcasted.is_ok(),
                            "Failed to downcast User back to MyCustomPayload"
                        );

                        let payload = downcasted.unwrap();
                        assert_eq!(payload.label, "Payload from background");
                        assert_eq!(payload.id, 42);

                        custom_event_received = true;
                    }

                    MichiuAnyEvent::Window { event, .. } => {
                        match event {
                            MichiuEvent::CloseRequested => {
                                close_event_received = true;
                            }
                            MichiuEvent::Resized(unvalidated_size) => {
                                // michiu_guard の validate_with を用いて境界検証を実行
                                let validation_result = unvalidated_size.validate_with(|size| {
                                    // 渡されたサイズが想定（800x600）と一致するか検証
                                    if size.width == 800 && size.height == 600 {
                                        Ok(size)
                                    } else {
                                        Err("Unvalidated window size mismatched the expected values.")
                                    }
                                });
                                // 期待通りのリサイズイベントであった場合のみ検証
                                if let Ok(validated_size) = validation_result {
                                    assert_eq!(
                                        validated_size.into_inner(),
                                        PhysicalSize::new(800, 600)
                                    );
                                    size_event_received = true;
                                }
                            }
                            _ => {}
                        }
                    }
                    _ => {}
                }
            });

            if custom_event_received
                && close_event_received
                && raw_event_received
                && size_event_received
            {
                break;
            }
            std::thread::sleep(Duration::from_millis(10));
        }

        // すべてのポスト＆翻訳経路が正しく完結したことをアサーション
        assert!(
            custom_event_received,
            "Failed to receive or verify custom User"
        );
        assert!(
            close_event_received,
            "Failed to receive or verify translated CloseRequested event"
        );
        assert!(
            raw_event_received,
            "Failed to receive or verify UnsafeRaw window event"
        );
        assert!(
            size_event_received,
            "Failed to receive or verify validated Resized event"
        );

        // クリーンアップ
        window.destroy();
    });
}
