use michiu_window::{
    EventPump, LogicalSize, MichiuEvent, SubclassResult, WindowBuilder, Event,
    init_dpi_awareness,
};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};
use windows::Win32::Foundation::{HWND, LPARAM, LRESULT, WPARAM};
use windows::Win32::UI::Shell::DefSubclassProc;
use windows::Win32::UI::WindowsAndMessaging::{SendMessageW, WM_USER};

// cargo test --test escape_hatches

// 実行されたエスケープハッチの順序を追跡するためのベクタ
type OrderTracker = Arc<Mutex<Vec<String>>>;

fn run_on_clean_thread<F>(f: F)
where
    F: FnOnce() + Send + 'static,
{
    let handle = std::thread::spawn(f);
    handle.join().expect("Test thread panicked");
}

// 危険な生のサブクラスプロシージャの実装 (WM_USER + 500 のみ追跡して後続に流す)
unsafe extern "system" fn raw_subclass_test_proc(
    hwnd: HWND,
    msg: u32,
    wparam: WPARAM,
    lparam: LPARAM,
    _id_subclass: usize,
    ref_data: usize,
) -> LRESULT {
    let tracker_ptr = ref_data as *const Mutex<Vec<String>>;
    if !tracker_ptr.is_null() && msg == WM_USER + 500 {
        let mut guard = (unsafe { &*tracker_ptr }).lock().unwrap();
        guard.push("raw_subclass".to_string());
    }
    // 危険なサブクラスなので、手動で次のチェーン（安全なサブクラスやメインWndProc）へ流す
    unsafe { DefSubclassProc(hwnd, msg, wparam, lparam) }
}

#[test]
fn test_integration_escape_hatches_coexistence_and_precedence() {
    let _ = init_dpi_awareness();

    run_on_clean_thread(|| {
        let tracker: OrderTracker = Arc::new(Mutex::new(Vec::new()));

        // with_message_filter を登録してウィンドウを構築
        let tracker_for_filter = tracker.clone();
        let builder = WindowBuilder::new()
            .with_title("Escape Hatches Test Window")
            .with_visible(false)
            .with_inner_size(LogicalSize::new(400.0, 300.0))
            .with_message_filter(move |_hwnd, msg, _wparam, _lparam| {
                if msg == WM_USER + 500 {
                    let mut guard = tracker_for_filter.lock().unwrap();
                    guard.push("message_filter".to_string());
                } else if msg == WM_USER + 502 {
                    // MessageFilter の段階で処理を完全にインターセプト（後続のイベントキューへの蓄積を遮断）
                    return Some(LRESULT(99));
                }
                None // 他のメッセージは後続のイベント翻訳（translate_and_push）に流す
            });

        let validated = builder
            .into_unvalidated()
            .try_into()
            .expect("Integration builder validation failed");

        let window = michiu_window::Window::build(validated).expect("Failed to build window");

        // subclass (安全なサブクラス) を登録
        let tracker_for_subclass = tracker.clone();
        window
            .subclass(101, move |_hwnd, msg, _wparam, _lparam| {
                if msg == WM_USER + 500 {
                    let mut guard = tracker_for_subclass.lock().unwrap();
                    guard.push("subclass".to_string());
                } else if msg == WM_USER + 501 {
                    // 安全なサブクラスの段階で処理を完全にインターセプト
                    return SubclassResult::Intercept(LRESULT(42));
                }
                SubclassResult::Continue // 次のサブクラス、またはメイン WndProc へ流す
            })
            .expect("Failed to register safe subclass");

        // raw_subclass (危険な生のサブクラス) を登録 (安全なサブクラスの手前に挿入される)
        unsafe {
            window
                .raw_subclass(102, Arc::as_ptr(&tracker) as usize, raw_subclass_test_proc)
                .expect("Failed to register raw subclass");
        }

        let mut event_pump = EventPump::new();

        // 実行順序（優先度）と伝搬の検証
        let res_500 = unsafe {
            SendMessageW(
                window.hwnd(),
                WM_USER + 500,
                Some(WPARAM(0)),
                Some(LPARAM(0)),
            )
        };

        // 戻り値がOS標準で処理されていることを確認
        assert_eq!(res_500.0, 0);

        // イベントループを1回転させて、最後まで生き残ったメッセージを UnsafeRaw として回収
        let mut raw_event_received = false;
        let start_time = Instant::now();
        while start_time.elapsed() < Duration::from_secs(1) {
            if let Some(event) = event_pump.wait_event().unwrap()
                && let MichiuEvent::Window {
                    event: Event::UnsafeRaw { msg, .. },
                    ..
                } = event
                && msg == WM_USER + 500
            {
                raw_event_received = true;
                break;
            }
        }

        // 全てのエスケープハッチをすり抜けてイベントキューまで届いていること
        assert!(
            raw_event_received,
            "Message was lost in transit and failed to reach the event queue"
        );

        // 実行されたエスケープハッチの優先順位が [raw_subclass -> subclass -> message_filter] の順であること
        let order_history = tracker.lock().unwrap();
        assert_eq!(
            order_history.len(),
            3,
            "Not all escape hatch layers processed the message"
        );
        assert_eq!(order_history[0], "raw_subclass");
        assert_eq!(order_history[1], "subclass");
        assert_eq!(order_history[2], "message_filter");

        drop(order_history); // 解放

        // サブクラスによるインターセプトの検証
        let res_501 = unsafe {
            SendMessageW(
                window.hwnd(),
                WM_USER + 501,
                Some(WPARAM(0)),
                Some(LPARAM(0)),
            )
        };

        // サブクラスが返したカスタムLRESULT(42)が直接返ってきていることを検証
        assert_eq!(
            res_501.0, 42,
            "Subclass failed to return custom intercepted LRESULT"
        );

        // インターセプトされたため、後続のイベントキューには一切届かないことを検証
        let mut intercept_501_received = false;
        let start_time_501 = Instant::now();
        while start_time_501.elapsed() < Duration::from_millis(100) {
            if let Some(event) = event_pump.poll_event()
                && let MichiuEvent::Window {
                    event: Event::UnsafeRaw { msg, .. },
                    ..
                } = event
                && msg == WM_USER + 501
            {
                intercept_501_received = true;
            }
        }
        assert!(
            !intercept_501_received,
            "Subclass intercepted message leaked to the event queue"
        );

        // MessageFilter によるインターセプトの検証
        let res_502 = unsafe {
            SendMessageW(
                window.hwnd(),
                WM_USER + 502,
                Some(WPARAM(0)),
                Some(LPARAM(0)),
            )
        };

        // MessageFilter が返したカスタムLRESULT(99)が直接返ってきていることを検証
        assert_eq!(
            res_502.0, 99,
            "MessageFilter failed to return custom intercepted LRESULT"
        );

        // イベントキューへのプッシュは阻害されていることを検証
        let mut intercept_502_received = false;
        let start_time_502 = Instant::now();
        while start_time_502.elapsed() < Duration::from_millis(100) {
            if let Some(event) = event_pump.poll_event()
                && let MichiuEvent::Window {
                    event: Event::UnsafeRaw { msg, .. },
                    ..
                } = event
                && msg == WM_USER + 502
            {
                intercept_502_received = true;
            }
        }
        assert!(
            !intercept_502_received,
            "MessageFilter intercepted message leaked to the event queue"
        );

        // MessageFilter があっても run_on_ui_thread が正常共存できるかの検証
        let handle = window.handle().assume_valid();
        let handle_clone = handle.clone();
        let ui_thread_executed = Arc::new(AtomicBool::new(false));
        let ui_thread_executed_clone = ui_thread_executed.clone();

        // バックグラウンドスレッドから run_on_ui_thread を叩く
        let bg_thread = std::thread::spawn(move || unsafe {
            handle_clone.run_on_ui_thread(move |_hwnd| {
                ui_thread_executed_clone.store(true, Ordering::SeqCst);
            });
        });
        bg_thread.join().expect("Background thread panicked");

        // MessageFilter が有効なウィンドウでも、システム予約メッセージ（WM_RUN_ON_UI_THREAD）が
        // 正常にフィルターを素通り（None）し、安全に実行されることをアサーション
        let start_time_ui = Instant::now();
        while start_time_ui.elapsed() < Duration::from_secs(1) {
            let _ = event_pump.poll_event();
            if ui_thread_executed.load(Ordering::SeqCst) {
                break;
            }
            std::thread::sleep(Duration::from_millis(10));
        }

        assert!(
            ui_thread_executed.load(Ordering::SeqCst),
            "MessageFilter blocked the system execution of run_on_ui_thread"
        );

        // クリーンアップ
        window.destroy();
    });
}
