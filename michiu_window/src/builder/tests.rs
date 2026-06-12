use super::*;
use crate::error::MichiuError;
use michiu_guard::Validated;

fn run_on_clean_thread<F>(f: F)
where
    F: FnOnce() + Send + 'static,
{
    let handle = std::thread::spawn(f);
    handle.join().expect("Test thread panicked");
}

#[test]
fn test_builder_default_is_valid() {
    // デフォルト設定でタイトルのみを指定したビルダーは正常に検証をパスするはず
    let builder = WindowBuilder::new().with_title("MainWindow");
    let validated: Result<Validated<WindowBuilder>> = builder.into_unvalidated().try_into();
    assert!(validated.is_ok(), "Default builder should be valid");
}

#[test]
fn test_builder_transparent_valid_combination() {
    // 透明ウィンドウを作成する際、デコレーションをオフにすれば検証をパスする
    let builder = WindowBuilder::new()
        .with_title("TransparentWindow")
        .with_transparent(true)
        .with_decorations(false);

    let validated: Result<Validated<WindowBuilder>> = builder.into_unvalidated().try_into();
    assert!(
        validated.is_ok(),
        "Transparent with no decorations should be valid"
    );
}

#[test]
fn test_builder_child_window_valid_combination() {
    // 子ウィンドウを作成する際、親HWNDを渡し、taskbar_button が false であればパスする
    // with_child_of を使うと自動的にフラグが整合するため、パスするのが正しい
    let parent_dummy = HWND(0x12345 as _);
    let builder = WindowBuilder::new()
        .with_title("ChildWindow")
        .with_child_of(parent_dummy);

    let validated: Result<Validated<WindowBuilder>> = builder.into_unvalidated().try_into();
    assert!(
        validated.is_ok(),
        "Child window with integrated with_child_of should be valid"
    );
}

#[test]
fn test_conflict_no_redirection_bitmap_without_com() {
    // COMコンテキストがない状態で DirectComposition (no_redirection_bitmap) を有効化するとエラー
    let builder = WindowBuilder::new()
        .with_title("DirectCompWindow")
        .with_no_redirection_bitmap(true)
        .with_decorations(false); // COMがNoneなのでエラーになるはず

    let result: Result<Validated<WindowBuilder>> = builder.into_unvalidated().try_into();
    assert!(result.is_err());
    assert!(matches!(
        result.unwrap_err(),
        MichiuError::ValidationError {
            parameter: "no_redirection_bitmap",
            ..
        }
    ));
}

#[test]
fn test_conflict_no_redirection_bitmap_with_decorations() {
    // DirectCompositionを有効にしながら、標準デコレーションを有効（デフォルト: true）にした場合はエラー
    // テストスレッドをCOM用にSTA化してComContextを用意
    run_on_clean_thread(|| {
        let com_ctx = ComContext::new_com_single().unwrap();
        let builder = WindowBuilder::new()
            .with_title("DirectCompWindow")
            .with_com_context(&com_ctx)
            .with_no_redirection_bitmap(true)
            .with_decorations(true); // デコレーションがtrueなので衝突エラー

        let result: Result<Validated<WindowBuilder>> = builder.into_unvalidated().try_into();
        assert!(result.is_err());
        assert!(matches!(
            result.unwrap_err(),
            MichiuError::ValidationError {
                parameter: "decorations",
                ..
            }
        ));
    });
}

#[test]
fn test_conflict_transparent_with_decorations() {
    // 透明ウィンドウを有効にしながら、標準デコレーションを有効にした場合はエラー
    let builder = WindowBuilder::new()
        .with_title("TransparentWindow")
        .with_transparent(true)
        .with_decorations(true); // 衝突エラー

    let result: Result<Validated<WindowBuilder>> = builder.into_unvalidated().try_into();
    assert!(result.is_err());
    assert!(matches!(
        result.unwrap_err(),
        MichiuError::ValidationError {
            parameter: "decorations",
            ..
        }
    ));
}

#[test]
fn test_conflict_child_window_with_taskbar_button() {
    // 子ウィンドウに設定したあと、強制的にタスクバーボタンを有効にするとエラー
    let parent_dummy = HWND(0x12345 as _);
    let builder = WindowBuilder::new()
        .with_title("Child")
        .with_child_of(parent_dummy)
        .with_taskbar_button(true); // 子ウィンドウなのにタスクバーに表示しようとして衝突エラー

    let result: Result<Validated<WindowBuilder>> = builder.into_unvalidated().try_into();
    assert!(result.is_err());
    assert!(matches!(
        result.unwrap_err(),
        MichiuError::ValidationError {
            parameter: "taskbar_button",
            ..
        }
    ));
}

#[test]
fn test_conflict_child_window_with_topmost() {
    // 子ウィンドウに最前面 (WS_EX_TOPMOST) を拡張スタイルで直接指定するとエラー
    let parent_dummy = HWND(0x12345 as _);
    let builder = WindowBuilder::new()
        .with_title("Child")
        .with_child_of(parent_dummy)
        .with_raw_ex_style(WS_EX_TOPMOST); // 衝突エラー

    let result: Result<Validated<WindowBuilder>> = builder.into_unvalidated().try_into();
    assert!(result.is_err());
    assert!(matches!(
        result.unwrap_err(),
        MichiuError::ValidationError {
            parameter: "raw_ex_style",
            ..
        }
    ));
}

#[test]
fn test_conflict_child_and_popup_mutually_exclusive() {
    // 生のスタイル設定で WS_CHILD と WS_POPUP を同時に付与すると排他衝突エラー
    let builder = WindowBuilder::new()
        .with_title("ConflictWindow")
        .with_taskbar_button(false) // 手前のタスクバー競合を回避
        .with_raw_style(WS_CHILD | WS_POPUP); // 排他エラー

    let result: Result<Validated<WindowBuilder>> = builder.into_unvalidated().try_into();
    assert!(result.is_err());
    assert!(matches!(
        result.unwrap_err(),
        MichiuError::ValidationError {
            parameter: "raw_style",
            ..
        }
    ));
}

#[test]
fn test_conflict_child_without_parent_handle() {
    // 生のスタイルで WS_CHILD を有効にしながら、親ウィンドウのハンドルが指定されていないとエラー
    let builder = WindowBuilder::new()
        .with_title("OrphanChild")
        .with_taskbar_button(false)
        .with_raw_style(WS_CHILD); // 親ハンドルがNoneなのでエラー

    let result: Result<Validated<WindowBuilder>> = builder.into_unvalidated().try_into();
    assert!(result.is_err());
    assert!(matches!(
        result.unwrap_err(),
        MichiuError::ValidationError {
            parameter: "parent_hwnd",
            ..
        }
    ));
}

#[test]
fn test_conflict_caption_and_dlgframe() {
    // 生スタイルで WS_CAPTION と WS_DLGFRAME を同時指定するとエラー
    let builder = WindowBuilder::new()
        .with_title("ConflictWindow")
        .with_raw_style(WS_CAPTION | WS_DLGFRAME);

    let result: Result<Validated<WindowBuilder>> = builder.into_unvalidated().try_into();
    assert!(result.is_err());
    assert!(matches!(
        result.unwrap_err(),
        MichiuError::ValidationError {
            parameter: "raw_style",
            ..
        }
    ));
}

#[test]
fn test_conflict_minimize_maximize_without_sysmenu() {
    // 最大化・最小化ボタンを有効にする場合、システムメニュー (WS_SYSMENU) が必要
    // (WS_SYSMENU を含めない raw_style でテスト)
    let builder = WindowBuilder::new()
        .with_title("NoSysmenuButtons")
        .with_raw_style(WS_MAXIMIZEBOX); // WS_SYSMENU がないのでエラー

    let result: Result<Validated<WindowBuilder>> = builder.into_unvalidated().try_into();
    assert!(result.is_err());
    assert!(matches!(
        result.unwrap_err(),
        MichiuError::ValidationError {
            parameter: "raw_style",
            ..
        }
    ));
}

#[test]
fn test_conflict_contexthelp_with_minimize_maximize() {
    // ヘルプボタン (WS_EX_CONTEXTHELP) は最大化・最小化ボタンと共存できない
    let builder = WindowBuilder::new()
        .with_title("ConflictWindow")
        .with_raw_style(WS_MAXIMIZEBOX | WS_SYSMENU)
        .with_raw_ex_style(WS_EX_CONTEXTHELP); // 衝突エラー

    let result: Result<Validated<WindowBuilder>> = builder.into_unvalidated().try_into();
    assert!(result.is_err());
    assert!(matches!(
        result.unwrap_err(),
        MichiuError::ValidationError {
            parameter: "raw_ex_style",
            ..
        }
    ));
}

#[test]
fn test_conflict_toolwindow_and_appwindow() {
    // ツールウィンドウ (WS_EX_TOOLWINDOW) と アプリウィンドウ (WS_EX_APPWINDOW) は矛盾するため同時指定不可
    let builder = WindowBuilder::new()
        .with_title("ConflictWindow")
        .with_raw_ex_style(WS_EX_TOOLWINDOW | WS_EX_APPWINDOW); // 衝突エラー

    let result: Result<Validated<WindowBuilder>> = builder.into_unvalidated().try_into();
    assert!(result.is_err());
    assert!(matches!(
        result.unwrap_err(),
        MichiuError::ValidationError {
            parameter: "raw_ex_style",
            ..
        }
    ));
}

#[test]
fn test_conflict_drag_and_drop_without_com() {
    // D&D を有効にしているのに、ComContext が指定されていないとバリデーションエラー
    let builder = WindowBuilder::new()
        .with_title("DndNoComWindow")
        .with_drag_and_drop(true);

    let result: Result<Validated<WindowBuilder>> = builder.into_unvalidated().try_into();
    assert!(result.is_err());
    assert!(matches!(
        result.unwrap_err(),
        MichiuError::ValidationError {
            parameter: "drag_and_drop",
            ..
        }
    ));
}

#[test]
fn test_conflict_drag_and_drop_with_non_ole_com() {
    // D&D を有効にしているのに、非OLEコンテキスト（MTAなど）を渡すとバリデーションエラー
    run_on_clean_thread(|| {
        let com_ctx = ComContext::new_com_multi().unwrap();
        let builder = WindowBuilder::new()
            .with_title("DndBadComWindow")
            .with_drag_and_drop(true)
            .with_com_context(&com_ctx);

        let result: Result<Validated<WindowBuilder>> = builder.into_unvalidated().try_into();
        assert!(result.is_err());
        assert!(matches!(
            result.unwrap_err(),
            MichiuError::ValidationError {
                parameter: "drag_and_drop",
                ..
            }
        ));
    });
}

#[test]
fn test_drag_and_drop_with_ole_com_success() {
    // D&D と OLEアパートメント（new_com_single）が正しく揃っている場合はパスする
    run_on_clean_thread(|| {
        let com_ctx = ComContext::new_com_single().unwrap(); // OLE STA
        let builder = WindowBuilder::new()
            .with_title("DndGoodComWindow")
            .with_drag_and_drop(true)
            .with_com_context(&com_ctx);

        let result: Result<Validated<WindowBuilder>> = builder.into_unvalidated().try_into();
        assert!(
            result.is_ok(),
            "Dnd with OLE context should pass validation successfully"
        );
    });
}
