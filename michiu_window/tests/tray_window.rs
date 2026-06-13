use michiu_window::{
    CustomTrayMenu, Event, EventPump, Icon, LogicalSize, MichiuEvent, TrayBuilder, TrayMenuItem,
    WindowBuilder, init_dpi_awareness,
};
use std::time::{Duration, Instant};
use windows::Win32::{
    Foundation::{LPARAM, WPARAM},
    UI::WindowsAndMessaging::{IDI_APPLICATION, LoadIconW, PostMessageW, WM_CLOSE},
};

// cargo test --test tray_window

fn run_on_clean_thread<F>(f: F)
where
    F: FnOnce() + Send + 'static,
{
    let handle = std::thread::spawn(f);
    handle.join().expect("Test thread panicked");
}

#[test]
fn test_integration_tray_and_multi_window_routing() {
    let _ = init_dpi_awareness();

    run_on_clean_thread(|| {
        // システム標準アイコンのロード
        let hicon_sys =
            unsafe { LoadIconW(None, IDI_APPLICATION).expect("Failed to load IDI_APPLICATION") };
        let icon = unsafe { Icon::from_raw(hicon_sys) };

        // カスタムメニューとしてポップアップ表示させるサブウィンドウの構築
        // デコレーションなし、タスクバー非表示でビルド
        let menu_builder = WindowBuilder::new()
            .with_title("Custom Tray Menu Window")
            .with_decorations(false)
            .with_taskbar_button(false)
            .with_visible(false) // 初期状態は非表示
            .with_inner_size(LogicalSize::new(200.0, 150.0));

        let validated_menu_builder = menu_builder
            .into_unvalidated()
            .try_into()
            .expect("Menu window validation failed");

        let menu_window = michiu_window::Window::build(validated_menu_builder)
            .expect("Failed to build custom menu window");

        // カスタムメニューとネイティブアイテムを含むトレイを構築
        let custom_menu = CustomTrayMenu::new(menu_window.handle().assume_valid().into_inner());

        let tray_builder = TrayBuilder::new()
            .with_icon(icon.clone())
            .with_tooltip("Michiu Integration Tray")
            .with_custom_menu(custom_menu)
            // ネイティブのメニュー項目も併設
            .with_menu_item(TrayMenuItem::new("Exit Application"));

        let tray = michiu_window::Tray::build(tray_builder.into_unvalidated().try_into().unwrap())
            .expect("Failed to build Tray");

        // クローンしてバックグラウンドへ持ち出し可能にする
        let tray_clone = tray.clone();

        // トレイをバインドしたメインウィンドウの構築
        let main_builder = WindowBuilder::new()
            .with_title("Main Tray Window")
            .with_icon(icon.clone())
            .with_inner_size(LogicalSize::new(500.0, 400.0))
            .with_tray(tray);

        let window =
            michiu_window::Window::build(main_builder.into_unvalidated().try_into().unwrap())
                .expect("Failed to build main window");

        // 各種ウィンドウの一意な WindowId と WindowHandle の取得
        let main_id = window.id();
        let menu_id = menu_window.id();

        let main_handle = window.handle().assume_valid();
        let menu_handle = menu_window.handle().assume_valid();

        // 両ウィンドウが異なる一意なIDを持っていることを確認
        assert_ne!(main_id, menu_id, "WindowIds must be unique for each window");

        // バックグラウンドスレッドから、それぞれのウィンドウに対してメッセージを個別にポストさせる
        let bg_thread = std::thread::spawn(move || {
            // メインウィンドウに対して CloseRequested を要求
            unsafe {
                let _ = PostMessageW(Some(main_handle.hwnd()), WM_CLOSE, WPARAM(0), LPARAM(0));
            }

            // カスタムメニューウィンドウに対して CloseRequested を要求
            unsafe {
                let _ = PostMessageW(Some(menu_handle.hwnd()), WM_CLOSE, WPARAM(0), LPARAM(0));
            }

            tray_clone
                .show_balloon(
                    "Dynamic Background Notification",
                    "A task running on a foreign thread has finished successfully.",
                )
                .expect(
                    "Failed to dynamically trigger balloon notification from background thread",
                );
        });

        bg_thread.join().expect("Background thread panicked");

        // UIスレッド側で1つのメッセージループを回し、複数のウィンドウからのイベントをIDで識別回収
        let mut event_pump = EventPump::new();
        let start_time = Instant::now();

        let mut main_close_received = false;
        let mut menu_close_received = false;

        // 両ウィンドウのクローズイベントを、ID識別を介して正しく検知するまで回す
        while start_time.elapsed() < Duration::from_secs(2) {
            if let Some(event) = event_pump.wait_event().unwrap()
                && let MichiuEvent::Window { id, event } = event
                && let Event::CloseRequested = event
            {
                if id == main_id {
                    main_close_received = true;
                } else if id == menu_id {
                    menu_close_received = true;
                } else {
                    panic!("Received CloseRequested from an unrecognized WindowId");
                }
            }

            if main_close_received && menu_close_received {
                break;
            }
        }

        // マルチウィンドウイベントの個別のルーティングが成功したことを確認
        assert!(
            main_close_received,
            "Failed to route and receive CloseRequested event for the Main window"
        );
        assert!(
            menu_close_received,
            "Failed to route and receive CloseRequested event for the Custom Menu window"
        );

        // 明示的にクリーンアップ
        window.destroy();
        menu_window.destroy();
    });
}
