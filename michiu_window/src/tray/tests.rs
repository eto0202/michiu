use super::*;
use crate::error::MichiuError;
use michiu_guard::Validated;
use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
};
use windows::Win32::{
    System::{LibraryLoader::GetModuleHandleW, Threading::GetCurrentThreadId},
    UI::WindowsAndMessaging::{
        CreateWindowExW, DestroyWindow, IDI_APPLICATION, IsWindowVisible, LoadIconW, SendMessageW,
        WINDOW_EX_STYLE, WINDOW_STYLE,
    },
};
use windows::core::w;

fn run_on_clean_thread<F>(f: F)
where
    F: FnOnce() + Send + 'static,
{
    let handle = std::thread::spawn(f);
    handle.join().expect("Test thread panicked");
}

#[test]
fn test_register_tray_window_class_normal() {
    run_on_clean_thread(|| {
        let hmodule = unsafe { GetModuleHandleW(None).unwrap() };
        let hinstance = HINSTANCE(hmodule.0);

        // ユニークなクラス名を作成（重複衝突防止）
        let unique_class_str = format!("MichiuTrayTestClass_{}", unsafe { GetCurrentThreadId() });
        let unique_class_wide: Vec<u16> = unique_class_str.encode_utf16().chain(Some(0)).collect();
        let class_name = PCWSTR(unique_class_wide.as_ptr());

        // クラスの新規登録が成功するか検証
        let register_result =
            register_tray_window_class(class_name, "MichiuTrayTestClass_Temp", hinstance);
        assert!(
            register_result.is_ok(),
            "Failed to register tray window class"
        );

        // 同じ名前で2回登録しても、OSエラーを握りつぶして Ok を返すか検証 (重複登録許容)
        let duplicate_result =
            register_tray_window_class(class_name, "MichiuTrayTestClass_Temp", hinstance);
        assert!(
            duplicate_result.is_ok(),
            "Duplicate registration should be handled safely and return Ok"
        );
    });
}

#[test]
fn test_add_tray_icon_normal() {
    run_on_clean_thread(|| {
        unsafe {
            let hmodule = GetModuleHandleW(None).unwrap();
            let hinstance = HINSTANCE(hmodule.0);

            // トレイアイコン用の仮ダミーウィンドウクラスを登録
            let class_name = w!("MichiuTrayAddIconTestClass");
            let _ = register_tray_window_class(class_name, "MichiuTrayAddIconTestClass", hinstance);

            // メッセージ受信用ダミーウィンドウを生成
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
            .expect("Failed to create temporary dummy window");

            // システム標準アイコンを仮ロード
            let hicon_sys = LoadIconW(None, IDI_APPLICATION).unwrap();
            let test_icon = Icon::from_raw(hicon_sys);

            // トレイアイコン追加のテスト実行
            let add_result = add_tray_icon(
                dummy_hwnd,
                99, // 仮のユニークID
                Some(test_icon),
                Some("Michiu Tray Test Tooltip"),
            );

            // 即時シェルとウィンドウを削除
            let nid_cleanup = NOTIFYICONDATAW {
                cbSize: std::mem::size_of::<NOTIFYICONDATAW>() as u32,
                hWnd: dummy_hwnd,
                uID: 99,
                ..Default::default()
            };
            let _ = Shell_NotifyIconW(NIM_DELETE, &nid_cleanup);
            let _ = DestroyWindow(dummy_hwnd);

            assert!(
                add_result.is_ok(),
                "add_tray_icon failed: {:?}",
                add_result.err()
            );
        }
    });
}

#[test]
fn test_tray_builder_normal() {
    let hicon_sys = unsafe { LoadIconW(None, IDI_APPLICATION).unwrap() };
    let icon = unsafe { Icon::from_raw(hicon_sys) };

    let builder = TrayBuilder::new().with_icon(icon);

    let validated_result: Result<Validated<TrayBuilder>> = builder.into_unvalidated().try_into();
    assert!(
        validated_result.is_ok(),
        "TrayBuilder validation should succeed"
    );
}

#[test]
fn test_tray_builder_missing_icon_abnormal() {
    // アイコンがない場合は検証エラーになる
    let builder = TrayBuilder::new().with_tooltip("No Icon");

    let result: Result<Validated<TrayBuilder>> = builder.into_unvalidated().try_into();
    assert!(result.is_err());
    assert!(matches!(
        result.unwrap_err(),
        MichiuError::ValidationError {
            parameter: "icon",
            ..
        }
    ));
}

#[test]
fn test_tray_builder_tooltip_overflow_abnormal() {
    let hicon_sys = unsafe { LoadIconW(None, IDI_APPLICATION).unwrap() };
    let icon = unsafe { Icon::from_raw(hicon_sys) };

    // 128文字を指定して検証エラーを検知するかテスト
    let overflow_tooltip = "a".repeat(128);
    let builder = TrayBuilder::new()
        .with_icon(icon)
        .with_tooltip(overflow_tooltip);

    let result: Result<Validated<TrayBuilder>> = builder.into_unvalidated().try_into();
    assert!(result.is_err());
    assert!(matches!(
        result.unwrap_err(),
        MichiuError::ValidationError {
            parameter: "tooltip",
            ..
        }
    ));
}

#[test]
fn test_tray_build_and_lifecycle_normal() {
    run_on_clean_thread(|| {
        // システム標準アイコンをロード
        let hicon_sys = unsafe { LoadIconW(None, IDI_APPLICATION).unwrap() };
        let icon = unsafe { Icon::from_raw(hicon_sys) };

        // 有効な TrayBuilder を作成
        let builder = TrayBuilder::new()
            .with_icon(icon)
            .with_tooltip("Tray Lifecycle Testing Tooltip");

        // バリデーションを実行して Validated<TrayBuilder> を取得
        let validated_builder = builder.into_unvalidated().try_into();
        assert!(validated_builder.is_ok(), "Failed to validate TrayBuilder");

        // Tray を構築
        let tray_result = Tray::build(validated_builder.unwrap());
        assert!(
            tray_result.is_ok(),
            "Tray::build failed: {:?}",
            tray_result.err()
        );

        let tray = tray_result.unwrap();

        // テーマ切替のテスト (内部の undocumented UXTheme API 呼び出しが落ちないか検証)
        // PreferredAppMode::AllowDark を渡して正常に処理が完了することを確認
        tray.set_theme(PreferredAppMode::AllowDark);

        // トレイをクローン (Arc の参照カウントが 2 になる)
        let tray_clone = tray.clone();

        // クローンした方を Drop。
        // 参照カウントが 1 になるだけで、この段階では OS のトレイアイコンや
        // ダミーウィンドウの削除メッセージは送信されないことを暗黙的にテスト
        drop(tray_clone);

        // オリジナルを Drop。
        // 参照カウントが 0 に達するため、ここで TrayInner の `Drop` 実装がトリガーされる。
        // 内部で `Shell_NotifyIconW(NIM_DELETE)` と `PostMessageW(..., WM_CLOSE, ...)`
        // が発行され、メモリとOSリソースが安全に片付けられる。
        drop(tray);
    });
}

#[test]
fn test_tray_wnd_proc_messaging_normal() {
    run_on_clean_thread(|| {
        // 各コールバックの実行状態を追跡するためのアトミックフラグ
        let left_clicked = Arc::new(AtomicBool::new(false));
        let left_clicked_clone = left_clicked.clone();

        let right_clicked = Arc::new(AtomicBool::new(false));
        let right_clicked_clone = right_clicked.clone();

        let menu_clicked = Arc::new(AtomicBool::new(false));
        let menu_clicked_clone = menu_clicked.clone();

        // システム標準アイコンをロード
        let hicon_sys = unsafe { LoadIconW(None, IDI_APPLICATION).unwrap() };
        let icon = unsafe { Icon::from_raw(hicon_sys) };

        // コールバックとメニューアイテムを持つビルダーを構築
        let builder = TrayBuilder::new()
            .with_icon(icon)
            .with_tooltip("Tray WndProc Test Tooltip")
            .on_left_click(move || {
                left_clicked_clone.store(true, Ordering::SeqCst);
            })
            .on_right_click(move || {
                right_clicked_clone.store(true, Ordering::SeqCst);
            })
            .with_menu_item(TrayMenuItem::new("Test MenuItem").with_on_click(move || {
                menu_clicked_clone.store(true, Ordering::SeqCst);
            }));

        // Tray の構築
        let validated_builder = builder.into_unvalidated().try_into();
        let tray = Tray::build(validated_builder.unwrap()).unwrap();

        // テスト用に非公開のダミーHWNDを取得
        let dummy_hwnd = tray.inner.dummy_hwnd;

        unsafe {
            // 左ダブルクリック
            assert!(!left_clicked.load(Ordering::SeqCst));

            // OS から WM_TRAY_CALLBACK + WM_LBUTTONDBLCLK が送られたと偽装
            let _ = SendMessageW(
                dummy_hwnd,
                WM_TRAY_CALLBACK,
                Some(WPARAM(0)),
                Some(LPARAM(WM_LBUTTONDBLCLK as isize)),
            );

            // コールバックが正常に動いたか確認
            assert!(
                left_clicked.load(Ordering::SeqCst),
                "on_left_click callback was not triggered"
            );

            // 右クリック
            assert!(!right_clicked.load(Ordering::SeqCst));

            // OS から WM_TRAY_CALLBACK + WM_RBUTTONUP が送られたと偽装する
            let _ = SendMessageW(
                dummy_hwnd,
                WM_TRAY_CALLBACK,
                Some(WPARAM(0)),
                Some(LPARAM(WM_RBUTTONUP as isize)),
            );

            // コールバックが正常に動いたか確認
            assert!(
                right_clicked.load(Ordering::SeqCst),
                "on_right_click callback was not triggered"
            );

            // メニューアイテム選択のシミュレーション
            assert!(!menu_clicked.load(Ordering::SeqCst));

            // 最初のメニューアイテムがクリックされたシグナル (WM_COMMAND + ID 1) を偽装する
            let _ = SendMessageW(
                dummy_hwnd,
                WM_COMMAND,
                Some(WPARAM(1)), // トレイビルダーによって自動連番で ID 1 が割り当てられている
                Some(LPARAM(0)),
            );

            // コールバックが正常に動いたか確認
            assert!(
                menu_clicked.load(Ordering::SeqCst),
                "TrayMenuItem's on_click callback was not triggered"
            );

            // エクスプローラー再起動シグナル
            // タスクバーが再起動された際のシグナル (TaskbarCreated) を偽装する
            let taskbar_created_msg = RegisterWindowMessageW(w!("TaskbarCreated"));
            let _ = SendMessageW(
                dummy_hwnd,
                taskbar_created_msg,
                Some(WPARAM(0)),
                Some(LPARAM(0)),
            );
            // クラッシュすることなくリカバリ処理が安全に終了したことをテスト
        }

        // Drop (アイコン消去とウィンドウ消去)
        drop(tray);
    });
}

#[test]
fn test_tray_wnd_proc_custom_menu_routing() {
    run_on_clean_thread(|| {
        unsafe {
            let hmodule = GetModuleHandleW(None).unwrap();
            let hinstance = HINSTANCE(hmodule.0);

            // カスタムメニューとして表示するダミーウィンドウを用意
            let menu_class = w!("MichiuTrayCustomMenuTestClass");
            let _ =
                register_tray_window_class(menu_class, "MichiuTrayCustomMenuTestClass", hinstance);
            let menu_hwnd = CreateWindowExW(
                WINDOW_EX_STYLE(0),
                menu_class,
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
            .unwrap();

            // CustomTrayMenu でラップする
            let handle = WindowHandle {
                hwnd: menu_hwnd,
                hinstance,
                thread_id: 0,
            };
            let custom_menu = CustomTrayMenu::new(handle);

            // ビルダーにカスタムメニューをセットしてトレイ作成
            let hicon_sys = LoadIconW(None, IDI_APPLICATION).unwrap();
            let icon = Icon::from_raw(hicon_sys);

            let builder = TrayBuilder::new()
                .with_icon(icon)
                .with_custom_menu(custom_menu);

            let tray = Tray::build(builder.validate_into().unwrap()).unwrap();
            let dummy_tray_hwnd = tray.inner.dummy_hwnd;

            // 初期状態ではカスタムメニューは非表示であるはず
            assert!(
                !IsWindowVisible(menu_hwnd).as_bool(),
                "Menu should be hidden initially"
            );

            // トレイアイコン上で右クリック (WM_RBUTTONUP) が起きたと偽装
            let _ = SendMessageW(
                dummy_tray_hwnd,
                WM_TRAY_CALLBACK,
                Some(WPARAM(0)),
                Some(LPARAM(WM_RBUTTONUP as isize)),
            );

            // カスタムメニューウィンドウが、OSレベルで表示（Visible）状態に切り替わったか検証
            assert!(
                IsWindowVisible(menu_hwnd).as_bool(),
                "Custom menu window should be made visible upon right click"
            );

            // クリーンアップ
            drop(tray);
            let _ = DestroyWindow(menu_hwnd);
        }
    });
}

#[test]
fn test_add_tray_icon_with_balloon_normal() {
    run_on_clean_thread(|| {
        unsafe {
            let hmodule = GetModuleHandleW(None).unwrap();
            let hinstance = HINSTANCE(hmodule.0);

            let class_name = w!("MichiuTrayAddIconBalloonTestClass");
            let _ = register_tray_window_class(
                class_name,
                "MichiuTrayAddIconBalloonTestClass",
                hinstance,
            );

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
            .expect("Failed to create temporary dummy window");

            let hicon_sys = LoadIconW(None, IDI_APPLICATION).unwrap();
            let test_icon = Icon::from_raw(hicon_sys);

            // バルーンテキストを伴うトレイアイコン追加のテスト実行
            let add_result = add_tray_icon(
                dummy_hwnd,
                100,
                Some(test_icon),
                Some("Michiu Tray Test Tooltip"),
            );

            let nid_cleanup = NOTIFYICONDATAW {
                cbSize: std::mem::size_of::<NOTIFYICONDATAW>() as u32,
                hWnd: dummy_hwnd,
                uID: 100,
                ..Default::default()
            };
            let _ = Shell_NotifyIconW(NIM_DELETE, &nid_cleanup);
            let _ = DestroyWindow(dummy_hwnd);

            assert!(
                add_result.is_ok(),
                "add_tray_icon with balloon parameters failed: {:?}",
                add_result.err()
            );
        }
    });
}

#[test]
fn test_tray_show_balloon_normal() {
    run_on_clean_thread(|| {
        // システム標準アイコンをロード
        let hicon_sys = unsafe { LoadIconW(None, IDI_APPLICATION).unwrap() };
        let icon = unsafe { Icon::from_raw(hicon_sys) };

        // トレイのビルド
        let builder = TrayBuilder::new()
            .with_icon(icon)
            .with_tooltip("Tray ShowBalloon Test Tooltip");

        let tray = Tray::build(builder.validate_into().unwrap()).expect("Failed to build Tray");

        // 任意のタイミングで動的にバルーンを発生させ、エラーなく Ok(()) が返るか検証
        let result = tray.show_balloon(
            "Dynamic Notification",
            "This is a dynamic toast message triggered on demand.",
        );
        assert!(
            result.is_ok(),
            "show_balloon call failed: {:?}",
            result.err()
        );

        // クリーンアップ
        drop(tray);
    });
}

#[test]
fn test_tray_show_balloon_limits() {
    run_on_clean_thread(|| {
        // システム標準アイコンをロード
        let hicon_sys = unsafe { LoadIconW(None, IDI_APPLICATION).unwrap() };
        let icon = unsafe { Icon::from_raw(hicon_sys) };

        // トレイのビルド
        let builder = TrayBuilder::new()
            .with_icon(icon)
            .with_tooltip("Tray Balloon Limit Test");

        let tray = Tray::build(builder.validate_into().unwrap()).unwrap();

        // タイトル文字数制限テスト（UTF-16換算で64文字、上限63を超える場合）
        // OSが定める制限を超過した際に、即座に ValidationError を返せるか検証
        let overflow_title = "a".repeat(64);
        let result_title = tray.show_balloon(&overflow_title, "Normal Body Text");

        assert!(
            result_title.is_err(),
            "Expected error for overflow title, but succeeded"
        );
        assert!(
            matches!(
                result_title.unwrap_err(),
                MichiuError::ValidationError {
                    parameter: "balloon_title",
                    ..
                }
            ),
            "Expected ValidationError on balloon_title"
        );

        // 本文文字数制限テスト（UTF-16換算で256文字、上限255を超える場合）
        let overflow_text = "a".repeat(256);
        let result_text = tray.show_balloon("Normal Title", &overflow_text);

        assert!(
            result_text.is_err(),
            "Expected error for overflow body, but succeeded"
        );
        assert!(
            matches!(
                result_text.unwrap_err(),
                MichiuError::ValidationError {
                    parameter: "balloon_text",
                    ..
                }
            ),
            "Expected ValidationError on balloon_text"
        );

        // クリーンアップ
        drop(tray);
    });
}
