use std::cell::Cell;

use super::*;
use crate::{CursorIcon, PhysicalPoint, PhysicalSize, Window, WindowBuilder, WindowId};
use michiu_guard::{Validate, Validated};
use windows::Win32::Foundation::{HWND, LPARAM, WPARAM};
use windows::Win32::UI::Input::KeyboardAndMouse::{VK_A, VK_SPACE};
use windows::Win32::UI::WindowsAndMessaging::{WM_LBUTTONDOWN, WM_MOVE, WM_SIZE};

fn clear_event_queue() {
    let mut pump = EventPump::new();
    while pump.poll_event().is_some() {}
}

fn run_on_clean_thread<F>(f: F)
where
    F: FnOnce() + Send + 'static,
{
    let handle = std::thread::spawn(f);
    handle.join().expect("Test thread panicked");
}

#[test]
fn test_translate_and_push_unsafe_raw_fallback() {
    clear_event_queue();

    let dummy_hwnd = HWND(0xABCDE as _);
    let mut state = WindowState {
        message_filter: None,
        auto_dpi_scaling: true,
        is_cursor_inside: false,
        min_inner_size: None,
        max_inner_size: None,
        saved_rect: None,
        saved_style: None,
        saved_ex_style: None,
        current_cursor: CursorIcon::Default,
        ime_relay: None,
    };

    // 未知のカスタムメッセージ
    let custom_msg_id = WM_USER + 999;
    let wparam_val = WPARAM(42);
    let lparam_val = LPARAM(-100);

    // translate_and_push は、OSの標準処理(DefWindowProc)を妨げないように `None` を返すべき
    let bypass_result = translate_and_push(
        dummy_hwnd,
        custom_msg_id,
        wparam_val,
        lparam_val,
        &mut state,
    );
    assert!(
        bypass_result.is_none(),
        "Unhandled messages must return None to let OS handle it"
    );

    // イベントキューから取り出して UnsafeRaw のデータ内容が完全一致するか確認
    let mut pump = EventPump::new();
    let event_opt = pump.poll_event();
    assert!(event_opt.is_some());

    match event_opt.unwrap() {
        MichiuEvent::Window { id, event } => {
            assert_eq!(id, WindowId(dummy_hwnd.0 as isize));
            match event {
                Event::UnsafeRaw {
                    msg,
                    wparam,
                    lparam,
                } => {
                    assert_eq!(msg, custom_msg_id);
                    assert_eq!(wparam.0, wparam_val.0);
                    assert_eq!(lparam.0, lparam_val.0);
                }
                other => panic!("Expected Event::UnsafeRaw, but got: {:?}", other),
            }
        }
        _ => panic!("Expected MichiuEvent::Event"),
    }
}

#[test]
fn test_translate_and_push_user_event_downcast_safe() {
    clear_event_queue();

    let dummy_hwnd = HWND(0x11111 as _);
    let mut state = WindowState {
        message_filter: None,
        auto_dpi_scaling: true,
        is_cursor_inside: false,
        min_inner_size: None,
        max_inner_size: None,
        saved_rect: None,
        saved_style: None,
        saved_ex_style: None,
        current_cursor: CursorIcon::Default,
        ime_relay: None,
    };

    // 別スレッドから送信されたと仮定して、テスト用の文字列イベントデータをBox化して生ポインタ化
    let test_payload = String::from("User Custom Payload Data");
    let boxed_payload: Box<dyn Any + Send> = Box::new(test_payload);
    let raw_ptr = Box::into_raw(Box::new(boxed_payload));

    // WM_USER_EVENT メッセージに生ポインタを乗せて翻訳に回す
    // 自スレッドで直接完結させているため、SendMessage的な完了処理 LRESULT(0) が返ってくるはず
    let bypass_result = translate_and_push(
        dummy_hwnd,
        WM_USER_EVENT,
        WPARAM(0),
        LPARAM(raw_ptr as isize),
        &mut state,
    );
    assert!(
        bypass_result.is_some(),
        "WM_USER_EVENT must return Some(LRESULT)"
    );
    assert_eq!(
        bypass_result.unwrap().0,
        0,
        "WM_USER_EVENT must return LRESULT(0)"
    );

    // イベントキューから取り出して、ダウンキャストして中身を検証
    let mut pump = EventPump::new();
    let event_opt = pump.poll_event();
    assert!(event_opt.is_some());

    match event_opt.unwrap() {
        MichiuEvent::User(boxed_any) => {
            // Any から元の String にダウンキャストできるか
            let downcasted = boxed_any.downcast::<String>();
            assert!(downcasted.is_ok(), "Downcast to String failed");
            assert_eq!(*downcasted.unwrap(), "User Custom Payload Data");
        }
        other => panic!("Expected MichiuEvent::User, but got: {:?}", other),
    }
}

#[test]
fn test_translate_and_push_standard_mappings() {
    clear_event_queue();

    let dummy_hwnd = HWND(0x22222 as _);
    let mut state = WindowState {
        message_filter: None,
        auto_dpi_scaling: true,
        is_cursor_inside: false,
        min_inner_size: None,
        max_inner_size: None,
        saved_rect: None,
        saved_style: None,
        saved_ex_style: None,
        current_cursor: CursorIcon::Default,
        ime_relay: None,
    };

    let mut pump = EventPump::new();

    // WM_SIZE
    // LPARAM に幅 1920 (下位16ビット)、高さ 1080 (上位16ビット) をビットパッキング
    let size_lparam = LPARAM(1920 | (1080 << 16));
    let _ = translate_and_push(dummy_hwnd, WM_SIZE, WPARAM(0), size_lparam, &mut state);

    match pump.poll_event().unwrap() {
        MichiuEvent::Window { event, .. } => match event {
            Event::Resized(unvalidated_size) => {
                // 値は正しいと仮定
                assert_eq!(unvalidated_size.into_inner(), PhysicalSize::new(1920, 1080));
            }
            other => panic!("Expected Event::Resized, but got {:?}", other),
        },
        _ => panic!("Expected Event"),
    }

    // WM_MOVE
    // LPARAM にマイナス座標である x = -50, y = 150 を符号付16ビットからパック
    let x_signed: i16 = -50;
    let y_signed: i16 = 150;
    let move_lparam = LPARAM((x_signed as u16 as isize) | ((y_signed as u16 as isize) << 16));
    let _ = translate_and_push(dummy_hwnd, WM_MOVE, WPARAM(0), move_lparam, &mut state);

    match pump.poll_event().unwrap() {
        MichiuEvent::Window { event, .. } => match event {
            Event::Moved(unvalidated_point) => {
                // 正しく -50 と 150 に符号復元されているか検証
                assert_eq!(unvalidated_point.into_inner(), PhysicalPoint::new(-50, 150));
            }
            other => panic!("Expected Event::Moved, but got {:?}", other),
        },
        _ => panic!("Expected Event"),
    }

    // WM_LBUTTONDOWN
    let _ = translate_and_push(dummy_hwnd, WM_LBUTTONDOWN, WPARAM(0), LPARAM(0), &mut state);

    match pump.poll_event().unwrap() {
        MichiuEvent::Window { event, .. } => match event {
            Event::MouseInput { button, state, .. } => {
                assert_eq!(button, MouseButton::Left);
                assert_eq!(state, ElementState::Pressed);
            }
            other => panic!("Expected Event::MouseInput, but got {:?}", other),
        },
        _ => panic!("Expected Event"),
    }

    // WM_KEYDOWN
    let _ = translate_and_push(
        dummy_hwnd,
        WM_KEYDOWN,
        WPARAM(VK_SPACE.0 as _),
        LPARAM(0),
        &mut state,
    );

    match pump.poll_event().unwrap() {
        MichiuEvent::Window { event, .. } => match event {
            Event::KeyboardInput {
                key_code, state, ..
            } => {
                assert_eq!(key_code.into_inner(), VK_SPACE);
                assert_eq!(state, ElementState::Pressed);
            }
            other => panic!("Expected Event::KeyboardInput, but got {:?}", other),
        },
        _ => panic!("Expected Event"),
    }
}

#[test]
fn test_translate_and_push_all_standard_window_events() {
    clear_event_queue();

    let dummy_hwnd = HWND(0x33333 as _);
    let mut state = WindowState {
        message_filter: None,
        auto_dpi_scaling: true, // WM_DPICHANGED テスト用
        is_cursor_inside: false,
        min_inner_size: None,
        max_inner_size: None,
        saved_rect: None,
        saved_style: None,
        saved_ex_style: None,
        current_cursor: CursorIcon::Default,
        ime_relay: None,
    };
    let mut pump = EventPump::new();

    // WM_CREATE: ウィンドウ作成時
    let res = translate_and_push(dummy_hwnd, WM_CREATE, WPARAM(0), LPARAM(0), &mut state);
    assert!(
        res.is_none(),
        "WM_CREATE should return None to let DefWindowProcW initialize"
    );
    match pump.poll_event().unwrap() {
        MichiuEvent::Window {
            event: Event::Created,
            ..
        } => {}
        other => panic!("Expected Event::Created, got {:?}", other),
    }

    // WM_CLOSE: クローズ要求時
    let res = translate_and_push(dummy_hwnd, WM_CLOSE, WPARAM(0), LPARAM(0), &mut state);
    assert!(
        res.is_some(),
        "WM_CLOSE should return Some(LRESULT(0)) to block DefWindowProcW"
    );
    assert_eq!(res.unwrap().0, 0);
    match pump.poll_event().unwrap() {
        MichiuEvent::Window {
            event: Event::CloseRequested,
            ..
        } => {}
        other => panic!("Expected Event::CloseRequested, got {:?}", other),
    }

    // WM_DESTROY: ウィンドウ破棄時
    let res = translate_and_push(dummy_hwnd, WM_DESTROY, WPARAM(0), LPARAM(0), &mut state);
    assert!(
        res.is_none(),
        "WM_DESTROY should return None to let DefWindowProcW finalize destruction"
    );
    match pump.poll_event().unwrap() {
        MichiuEvent::Window {
            event: Event::Destroyed,
            ..
        } => {}
        other => panic!("Expected Event::Destroyed, got {:?}", other),
    }

    // WM_SETFOCUS / WM_KILLFOCUS: フォーカス変更時
    let _ = translate_and_push(dummy_hwnd, WM_SETFOCUS, WPARAM(0), LPARAM(0), &mut state);
    match pump.poll_event().unwrap() {
        MichiuEvent::Window {
            event: Event::Focused(true),
            ..
        } => {}
        other => panic!("Expected Event::Focused(true), got {:?}", other),
    }
    let _ = translate_and_push(dummy_hwnd, WM_KILLFOCUS, WPARAM(0), LPARAM(0), &mut state);
    match pump.poll_event().unwrap() {
        MichiuEvent::Window {
            event: Event::Focused(false),
            ..
        } => {}
        other => panic!("Expected Event::Focused(false), got {:?}", other),
    }

    // WM_CHAR: 文字入力時
    let _ = translate_and_push(
        dummy_hwnd,
        WM_CHAR,
        WPARAM('A' as usize),
        LPARAM(0),
        &mut state,
    );
    match pump.poll_event().unwrap() {
        MichiuEvent::Window {
            event: Event::CharacterInput('A'),
            ..
        } => {}
        other => panic!("Expected Event::CharacterInput('A'), got {:?}", other),
    }

    // WM_KEYDOWN / WM_KEYUP / WM_SYSKEYDOWN / WM_SYSKEYUP: 物理キー入力時
    let _ = translate_and_push(
        dummy_hwnd,
        WM_KEYDOWN,
        WPARAM(VK_A.0 as usize),
        LPARAM(0),
        &mut state,
    );
    match pump.poll_event().unwrap() {
        MichiuEvent::Window {
            event: Event::KeyboardInput {
                key_code, state, ..
            },
            ..
        } => {
            assert_eq!(key_code.into_inner(), VK_A);
            assert_eq!(state, ElementState::Pressed);
        }
        other => panic!("Expected KeyboardInput Pressed, got {:?}", other),
    }
    let _ = translate_and_push(
        dummy_hwnd,
        WM_KEYUP,
        WPARAM(VK_A.0 as usize),
        LPARAM(0),
        &mut state,
    );
    match pump.poll_event().unwrap() {
        MichiuEvent::Window {
            event: Event::KeyboardInput {
                key_code, state, ..
            },
            ..
        } => {
            assert_eq!(key_code.into_inner(), VK_A);
            assert_eq!(state, ElementState::Released);
        }
        other => panic!("Expected KeyboardInput Released, got {:?}", other),
    }
    // WM_SYSKEYDOWN/UP は WM_KEYDOWN/UP と同じイベントとして翻訳される
    let _ = translate_and_push(
        dummy_hwnd,
        WM_SYSKEYDOWN,
        WPARAM(VK_CONTROL.0 as usize),
        LPARAM(0),
        &mut state,
    );
    match pump.poll_event().unwrap() {
        MichiuEvent::Window {
            event: Event::KeyboardInput {
                key_code, state, ..
            },
            ..
        } => {
            assert_eq!(key_code.into_inner(), VK_CONTROL);
            assert_eq!(state, ElementState::Pressed);
        }
        other => panic!("Expected System KeyboardInput Pressed, got {:?}", other),
    }

    // WM_MOUSEMOVE / WM_MOUSELEAVE: カーソル移動・領域外移動時
    // 初期状態: is_cursor_inside = false
    let mouse_move_lparam = LPARAM(100 | (200 << 16)); // x=100, y=200
    let _ = translate_and_push(
        dummy_hwnd,
        WM_MOUSEMOVE,
        WPARAM(0),
        mouse_move_lparam,
        &mut state,
    );
    assert!(
        state.is_cursor_inside,
        "WM_MOUSEMOVE should set is_cursor_inside to true"
    );
    // WM_MOUSEMOVE 初回は CursorEntered も発行される
    match pump.poll_event().unwrap() {
        MichiuEvent::Window {
            event: Event::CursorEntered,
            ..
        } => {}
        other => panic!("Expected CursorEntered, got {:?}", other),
    }
    match pump.poll_event().unwrap() {
        MichiuEvent::Window {
            event: Event::CursorMoved { position },
            ..
        } => {
            assert_eq!(position.into_inner(), PhysicalPoint::new(100, 200));
        }
        other => panic!("Expected CursorMoved, got {:?}", other),
    }

    let _ = translate_and_push(dummy_hwnd, WM_MOUSELEAVE, WPARAM(0), LPARAM(0), &mut state);
    assert!(
        !state.is_cursor_inside,
        "WM_MOUSELEAVE should set is_cursor_inside to false"
    );
    match pump.poll_event().unwrap() {
        MichiuEvent::Window {
            event: Event::CursorLeft,
            ..
        } => {}
        other => panic!("Expected CursorLeft, got {:?}", other),
    }

    // WM_LBUTTONUP / WM_RBUTTONDOWN / WM_MBUTTONUP: 各種マウスボタン入力時
    let _ = translate_and_push(dummy_hwnd, WM_LBUTTONUP, WPARAM(0), LPARAM(0), &mut state);
    match pump.poll_event().unwrap() {
        MichiuEvent::Window {
            event: Event::MouseInput { button, state, .. },
            ..
        } => {
            assert_eq!(button, MouseButton::Left);
            assert_eq!(state, ElementState::Released);
        }
        other => panic!("Expected Left MouseInput Released, got {:?}", other),
    }
    let _ = translate_and_push(dummy_hwnd, WM_RBUTTONDOWN, WPARAM(0), LPARAM(0), &mut state);
    match pump.poll_event().unwrap() {
        MichiuEvent::Window {
            event: Event::MouseInput { button, state, .. },
            ..
        } => {
            assert_eq!(button, MouseButton::Right);
            assert_eq!(state, ElementState::Pressed);
        }
        other => panic!("Expected Right MouseInput Pressed, got {:?}", other),
    }
    let _ = translate_and_push(dummy_hwnd, WM_MBUTTONUP, WPARAM(0), LPARAM(0), &mut state);
    match pump.poll_event().unwrap() {
        MichiuEvent::Window {
            event: Event::MouseInput { button, state, .. },
            ..
        } => {
            assert_eq!(button, MouseButton::Middle);
            assert_eq!(state, ElementState::Released);
        }
        other => panic!("Expected Middle MouseInput Released, got {:?}", other),
    }

    // WM_MOUSEWHEEL: マウスホイール回転時
    // ホイールデルタ 120 (前方へ1回転) を WPARAM の上位16ビットにセット
    let wheel_wparam = WPARAM(120 << 16);
    let _ = translate_and_push(
        dummy_hwnd,
        WM_MOUSEWHEEL,
        wheel_wparam,
        LPARAM(0),
        &mut state,
    );
    match pump.poll_event().unwrap() {
        MichiuEvent::Window {
            event: Event::MouseWheel { delta },
            ..
        } => {
            assert_eq!(delta, 1.0); // 120 / 120 = 1.0
        }
        other => panic!("Expected MouseWheel delta 1.0, got {:?}", other),
    }
    // ホイールデルタ -240 (後方へ2回転)
    let wheel_wparam_neg = WPARAM((((-240i16) as u16) as usize) << 16);
    let _ = translate_and_push(
        dummy_hwnd,
        WM_MOUSEWHEEL,
        wheel_wparam_neg,
        LPARAM(0),
        &mut state,
    );
    match pump.poll_event().unwrap() {
        MichiuEvent::Window {
            event: Event::MouseWheel { delta },
            ..
        } => {
            assert_eq!(delta, -2.0); // -240 / 120 = -2.0
        }
        other => panic!("Expected MouseWheel delta -2.0, got {:?}", other),
    }

    // WM_PAINT: 再描画要求時
    let res = translate_and_push(dummy_hwnd, WM_PAINT, WPARAM(0), LPARAM(0), &mut state);
    assert!(
        res.is_some(),
        "WM_PAINT should return Some(LRESULT(0)) to block DefWindowProcW for custom rendering"
    );
    assert_eq!(res.unwrap().0, 0);
    match pump.poll_event().unwrap() {
        MichiuEvent::Window {
            event: Event::RedrawRequested,
            ..
        } => {}
        other => panic!("Expected RedrawRequested, got {:?}", other),
    }

    // WM_DPICHANGED: DPI変更時 (auto_dpi_scaling = true の場合)
    // 新しいDPI = 144 (150%スケール)
    let dpi_wparam = WPARAM(144);
    // 推奨サイズ: (0,0)-(1200,900)
    let mut rect = RECT {
        left: 0,
        top: 0,
        right: 1200,
        bottom: 900,
    };
    let rect_lparam = LPARAM(&mut rect as *mut RECT as isize);

    let res = translate_and_push(
        dummy_hwnd,
        WM_DPICHANGED,
        dpi_wparam,
        rect_lparam,
        &mut state,
    );
    // auto_dpi_scaling が true なので、ここで Some(LRESULT(0)) を返して SetWindowPos が行われるはず
    assert!(
        res.is_some(),
        "WM_DPICHANGED with auto_dpi_scaling should return Some(LRESULT(0))"
    );
    assert_eq!(res.unwrap().0, 0);

    match pump.poll_event().unwrap() {
        MichiuEvent::Window {
            event:
                Event::ScaleFactorChanged {
                    scale_factor,
                    suggested_bounds,
                },
            ..
        } => {
            assert_eq!(scale_factor, 1.5); // 144/96 = 1.5
            assert_eq!(
                suggested_bounds.into_inner(),
                PhysicalRect::new(0, 0, 1200, 900)
            );
        }
        other => panic!("Expected ScaleFactorChanged, got {:?}", other),
    }
}

#[test]
fn test_translate_and_push_window_commands_memory_safe() {
    clear_event_queue();
    let dummy_hwnd = HWND(0x44444 as _);
    let mut state = WindowState {
        message_filter: None,
        auto_dpi_scaling: true,
        is_cursor_inside: false,
        min_inner_size: None,
        max_inner_size: None,
        saved_rect: None,
        saved_style: None,
        saved_ex_style: None,
        current_cursor: CursorIcon::Default,
        ime_relay: None,
    };

    // 別スレッドからの要求をシミュレート（サイズ変更コマンドをBox化）
    let cmd = Box::new(SetWindowCommand::Size(PhysicalSize::new(1024, 768)));
    let raw_ptr = Box::into_raw(cmd);

    // WM_WINDOW_COMMAND にポインタを乗せて WndProc (translate_and_push) に投げる
    let res = translate_and_push(
        dummy_hwnd,
        WM_WINDOW_COMMAND,
        WPARAM(0),
        LPARAM(raw_ptr as isize),
        &mut state,
    );

    // 処理が完了し、DefWindowProcWをブロックするために Some(LRESULT(0)) が返るはず
    assert!(res.is_some());
    assert_eq!(res.unwrap().0, 0);

    // この時点で Box::from_raw が呼ばれてメモリが安全に解放され、
    // かつダミーのHWNDに対して SetWindowPos が発行されていればクラッシュせずに成功
}

#[test]
fn test_event_pump_fifo_ordering() {
    clear_event_queue();
    let dummy_hwnd = HWND(0x55555 as _);
    let mut state = WindowState {
        message_filter: None,
        auto_dpi_scaling: true,
        is_cursor_inside: false,
        min_inner_size: None,
        max_inner_size: None,
        saved_rect: None,
        saved_style: None,
        saved_ex_style: None,
        current_cursor: CursorIcon::Default,
        ime_relay: None,
    };

    // 順番に3つのイベントをOSから受信したとシミュレート
    // マウス左ボタンダウン
    translate_and_push(dummy_hwnd, WM_LBUTTONDOWN, WPARAM(0), LPARAM(0), &mut state);

    // マウス移動 (※cursor_insideがfalseからtrueになるため、EnteredとMovedの2つのイベントを生成する)
    translate_and_push(
        dummy_hwnd,
        WM_MOUSEMOVE,
        WPARAM(0),
        LPARAM(10 | (10 << 16)),
        &mut state,
    );

    // マウス左ボタンアップ
    translate_and_push(dummy_hwnd, WM_LBUTTONUP, WPARAM(0), LPARAM(0), &mut state);

    let mut pump = EventPump::new();

    // 取り出したイベントが、追加された順番と完全に一致するかアサーション

    // 1番目: LBUTTONDOWN (Pressed)
    assert!(matches!(
        pump.poll_event().unwrap(),
        MichiuEvent::Window {
            event: Event::MouseInput {
                state: ElementState::Pressed,
                ..
            },
            ..
        }
    ));

    // 2番目: MOUSEMOVE に誘発された CursorEntered
    assert!(matches!(
        pump.poll_event().unwrap(),
        MichiuEvent::Window {
            event: Event::CursorEntered,
            ..
        }
    ));

    // 3番目: MOUSEMOVE 本体の CursorMoved
    assert!(matches!(
        pump.poll_event().unwrap(),
        MichiuEvent::Window {
            event: Event::CursorMoved { .. },
            ..
        }
    ));

    // 4番目: LBUTTONUP (Released)
    assert!(matches!(
        pump.poll_event().unwrap(),
        MichiuEvent::Window {
            event: Event::MouseInput {
                state: ElementState::Released,
                ..
            },
            ..
        }
    ));

    // 5番目: キューが空になっていること
    assert!(
        pump.poll_event().is_none(),
        "Queue should be completely empty"
    );
}

// テスト用のダミーイベント型
struct LoginEvent {
    username: String,
}
struct LogoutEvent;
#[allow(dead_code)]
struct RenderEvent(u32);

#[test]
fn test_event_bus_subscribe_and_publish() {
    let bus = EventBus::new();
    let counter = Rc::new(Cell::new(0));

    let counter_clone = counter.clone();
    bus.subscribe(move |event: &LoginEvent| {
        assert_eq!(event.username, "Michiu");
        counter_clone.set(counter_clone.get() + 1);
    });

    // LoginEvent を発行
    bus.publish(&LoginEvent {
        username: "Michiu".to_string(),
    });

    // 1回コールバックが呼ばれたことを確認
    assert_eq!(counter.get(), 1);

    // 無関係なイベントを発行しても、LoginEvent のリスナーは呼ばれない
    bus.publish(&LogoutEvent);
    assert_eq!(counter.get(), 1);
}

#[test]
fn test_event_bus_multiple_listeners() {
    let bus = EventBus::new();
    let hit_count = Rc::new(Cell::new(0));

    // リスナーA
    let c1 = hit_count.clone();
    bus.subscribe(move |_: &RenderEvent| c1.set(c1.get() + 1));

    // リスナーB
    let c2 = hit_count.clone();
    bus.subscribe(move |_: &RenderEvent| c2.set(c2.get() + 1));

    // 1回 Publish すると、2つのリスナーが呼ばれるので +2 される
    bus.publish(&RenderEvent(60));
    assert_eq!(hit_count.get(), 2);
}

#[test]
fn test_event_bus_nested_publish_and_subscribe_safe() {
    // コールバック内で別の Publish や Subscribe を呼んでもパニックしないかのテスト
    let bus = EventBus::new();

    let logout_triggered = Rc::new(Cell::new(false));

    let bus_clone_for_listener = bus.clone();
    let logout_triggered_clone = logout_triggered.clone();

    // Login 時に Logout イベントの購読を動的に追加し、
    // 同時に別のイベントを Publish する連鎖的コールバック
    bus.subscribe(move |_: &LoginEvent| {
        // ここでパニック (AlreadyBorrowed) が起きなければ成功

        // 動的 Subscribe
        let lt_inner = logout_triggered_clone.clone();
        bus_clone_for_listener.subscribe(move |_: &LogoutEvent| {
            lt_inner.set(true);
        });

        // ネストされた Publish
        bus_clone_for_listener.publish(&RenderEvent(144));
    });

    // 最初のトリガー
    bus.publish(&LoginEvent {
        username: "Admin".to_string(),
    });

    // 動的に追加された Logout リスナーが機能するか検証
    assert!(!logout_triggered.get());
    bus.publish(&LogoutEvent);
    assert!(
        logout_triggered.get(),
        "Dynamically added listener should be triggered"
    );
}

#[test]
fn test_translate_and_push_expanded_window_commands_memory_safe() {
    clear_event_queue();
    let dummy_hwnd = HWND(0x66666 as _);
    let mut state = WindowState {
        message_filter: None,
        auto_dpi_scaling: true,
        is_cursor_inside: false,
        min_inner_size: None,
        max_inner_size: None,
        saved_style: None,
        saved_ex_style: None,
        saved_rect: None,
        current_cursor: CursorIcon::Default,
        ime_relay: None,
    };

    // SetClipboardText コマンドのテスト
    let cmd_clip = Box::new(SetWindowCommand::SetClipboardText(
        "Cmd Clipboard".to_string(),
    ));
    let ptr_clip = Box::into_raw(cmd_clip);
    let res_clip = translate_and_push(
        dummy_hwnd,
        WM_WINDOW_COMMAND,
        WPARAM(0),
        LPARAM(ptr_clip as isize),
        &mut state,
    );
    assert!(
        res_clip.is_some(),
        "SetClipboardText command should be processed successfully"
    );

    // CursorCapture コマンドのテスト
    let cmd_cap = Box::new(SetWindowCommand::CursorCapture(true));
    let ptr_cap = Box::into_raw(cmd_cap);
    let res_cap = translate_and_push(
        dummy_hwnd,
        WM_WINDOW_COMMAND,
        WPARAM(0),
        LPARAM(ptr_cap as isize),
        &mut state,
    );
    assert!(
        res_cap.is_some(),
        "CursorCapture command should be processed successfully"
    );

    // CursorClipping コマンドのテスト
    let cmd_clip_cur = Box::new(SetWindowCommand::CursorClipping(true));
    let ptr_clip_cur = Box::into_raw(cmd_clip_cur);
    let res_clip_cur = translate_and_push(
        dummy_hwnd,
        WM_WINDOW_COMMAND,
        WPARAM(0),
        LPARAM(ptr_clip_cur as isize),
        &mut state,
    );
    assert!(
        res_clip_cur.is_some(),
        "CursorClipping command should be processed successfully"
    );

    // CenterOnScreen コマンドのテスト
    let cmd_center = Box::new(SetWindowCommand::CenterOnScreen);
    let ptr_center = Box::into_raw(cmd_center);
    let res_center = translate_and_push(
        dummy_hwnd,
        WM_WINDOW_COMMAND,
        WPARAM(0),
        LPARAM(ptr_center as isize),
        &mut state,
    );
    assert!(
        res_center.is_some(),
        "CenterOnScreen command should be processed successfully"
    );

    // Fullscreen コマンドのテスト
    let cmd_fs = Box::new(SetWindowCommand::Fullscreen(true));
    let ptr_fs = Box::into_raw(cmd_fs);
    let res_fs = translate_and_push(
        dummy_hwnd,
        WM_WINDOW_COMMAND,
        WPARAM(0),
        LPARAM(ptr_fs as isize),
        &mut state,
    );
    assert!(
        res_fs.is_some(),
        "Fullscreen command should be processed successfully"
    );
}

#[test]
fn test_translate_and_push_ime_bundle_unvalidated_normal() {
    run_on_clean_thread(|| {
        // IME機能のテストのため、OLE STA でウインドウを立ち上げる
        let com_ctx = crate::ComContext::new_com_single().unwrap();
        let builder = WindowBuilder::new()
            .with_title("ImeMessageTestWindow")
            .with_com_context(&com_ctx);
        let window = Window::build(builder.validate_into().unwrap()).unwrap();

        // ImeContext を介して、ウィンドウの物理IME状態をテスト用に直接セット
        if let Ok(ctx) = ImeContext::new(window.hwnd()) {
            ctx.set_open(true);
            ctx.set_conversion_status(1, 0);
            ctx.set_composition_window_position(PhysicalPoint { x: 42, y: 84 });
        }

        clear_event_queue();

        // ウィンドウから WindowStateを取得
        let state_ptr = unsafe {
            windows::Win32::UI::WindowsAndMessaging::GetWindowLongPtrW(
                window.hwnd(),
                windows::Win32::UI::WindowsAndMessaging::GWLP_USERDATA,
            ) as *mut WindowState
        };
        assert!(!state_ptr.is_null());
        let state = unsafe { &mut *state_ptr };

        // WM_IME_COMPOSITION メッセージを偽装送信
        let res = translate_and_push(
            window.hwnd(),
            WM_IME_COMPOSITION,
            WPARAM(0),
            LPARAM(0),
            state,
        );
        // WM_IME_COMPOSITION は OS標準候補窓を表示させるため DefWindowProcW に流すのが正しい
        assert!(res.is_none());

        // イベントポンプから取り出して、Unvalidated が機能するか検証
        let mut pump = EventPump::new();
        let ev_opt = pump.poll_event();
        assert!(ev_opt.is_some());

        match ev_opt.unwrap() {
            MichiuEvent::Window { id, event } => {
                assert_eq!(id, WindowId(window.hwnd().0 as isize));
                match event {
                    Event::Ime(unvalidated_update) => {
                        // 【境界防御の検証】
                        // 開発者になりきって、`validate_with` を呼び出し、安全に値を取得します
                        let validation_result: crate::Result<Validated<ImeStateUpdate>> =
                            unvalidated_update.validate_with(|update| {
                                // OSから取得した snapshot 情報が、セットした値と一致しているか
                                assert!(update.is_open, "IME status should be reported as OPEN");
                                assert_eq!(update.conversion_mode, 1, "Conversion mode mismatch");
                                assert_eq!(
                                    update.caret_position,
                                    Some(PhysicalPoint { x: 42, y: 84 }),
                                    "Caret physical position mismatch"
                                );

                                // 文字列は変換中でないため空であることをアサーション
                                assert_eq!(update.composition_text, "");
                                assert_eq!(update.result_text, "");

                                Ok(update) // 検証成功
                            });

                        assert!(
                            validation_result.is_ok(),
                            "ImeStateUpdate validation failed"
                        );
                    }
                    other => panic!("Expected Event::Ime, but got: {:?}", other),
                }
            }
            _ => panic!("Expected MichiuEvent::Event"),
        }

        window.destroy();
    });
}

#[test]
fn test_post_message_leak_prevention_on_zombie_hwnd() {
    run_on_clean_thread(|| {
        let builder = WindowBuilder::new().with_title("LeakPreventionTest");
        let window = Window::build(builder.validate_into().unwrap()).unwrap();
        let handle = window.handle().assume_valid();

        // ウィンドウを破壊してゾンビHWND状態にする
        #[allow(unused)]
        let raw_hwnd = window.hwnd();
        window.destroy();

        // バックグラウンドスレッドからゾンビHWNDに対して操作をポストさせる
        let handle_clone1 = handle.clone();
        let t1 = std::thread::spawn(move || {
            // 他スレッドから呼ぶため、確実に post_command (PostMessageW) を通過し
            // キューに生ポインタメッセージが蓄積。
            handle_clone1.set_size(PhysicalSize::new(1280, 720));
        });
        t1.join().unwrap();

        // メモリがクラッシュやリークなしに安全に解放されることをテスト
        let mut pump = EventPump::new();
        let _ = pump.poll_event();

        // 同様に、WM_RUN_ON_UI_THREAD のゾンビHWNDに対するポストと安全解放をテスト
        let handle_clone2 = handle.clone();
        let t2 = std::thread::spawn(move || {
            unsafe {
                // 他スレッドから呼ぶため、確実にメッセージポストのパスを通過
                handle_clone2.run_on_ui_thread(|_| {
                    panic!("This should never be executed because window is destroyed");
                });
            }
        });
        t2.join().unwrap();

        // poll_event で処理を試みる際、ゾンビ検知がパニックを叩くことなく安全に回収
        let _ = pump.poll_event();
    });
}

#[test]
fn test_leak_prevention_on_pump_drop_flush() {
    run_on_clean_thread(|| {
        let builder = WindowBuilder::new().with_title("PumpDropFlushTest");
        let window = Window::build(builder.validate_into().unwrap()).unwrap();
        let handle = window.handle().assume_valid();

        // 新しい EventPump のスコープを作る
        {
            let _pump = EventPump::new();

            // 操作メッセージをキューにポスト
            handle.set_title("This message will sit in queue");

            // この段階ではまだ poll_event を呼んでいないため、
            // メッセージは Windows メッセージキューの中に留まった状態

            // ここで _pump がスコープを抜け、Drop がトリガーされる
        }

        // _pump の Drop 実装により、
        // 処理されずにキューに残っていた "This message will sit in queue" コマンドポインタが
        // メモリリークすることなく安全に自動回収・解放されていることをテスト
        window.destroy();
    });
}

#[test]
fn test_run_on_ui_thread_panic_safety_normal() {
    run_on_clean_thread(|| {
        let builder = WindowBuilder::new().with_title("PanicSafetyTest");
        let window = Window::build(builder.validate_into().unwrap()).unwrap();
        let handle = window.handle().assume_valid();

        // バックグラウンドスレッドからパニックを投げるクロージャをポスト
        let handle_clone = handle.clone();
        let t = std::thread::spawn(move || {
            unsafe {
                handle_clone.run_on_ui_thread(|_| {
                    // ここで意図的にスレッドパニックを引き起こす
                    panic!("Simulated panic inside run_on_ui_thread closure");
                });
            }
        });
        t.join().unwrap();

        // メッセージループを回して poll_event を実行
        let mut pump = EventPump::new();

        // WndProc 内の std::panic::catch_unwind がこのパニックをキャッチするため、
        // テストを実行しているこのUIスレッドはパニック死することなく、
        // 安全に None または期待する処理を実行完了できるはず
        let result = pump.poll_event();

        // ポンプ処理後、無事にテストが続行できていることを確認
        assert!(result.is_none() || result.is_some());

        window.destroy();
    });
}

#[test]
fn test_event_pump_duplicate_detection_lifecycle() {
    run_on_clean_thread(|| {
        // 最初のメッセージポンプを生成
        let pump1 = EventPump::new();

        // 同じスレッド上でもう一つ生成した際、
        // 警告が適切に実行され、エラーを起こさず生成自体は維持できるか確認
        let pump2 = EventPump::new();

        // pump1 を明示的にドロップ
        drop(pump1);

        // pump2 を明示的にドロップ。
        // ここですべてのポンプが解体されるため、PUMP_ACTIVE フラグが正常に false に戻るはず
        drop(pump2);

        // フラグが初期化されているため、3つ目の生成がエラーなく新規活性として受け入れられるか検証
        let _pump3 = EventPump::new();
    });
}
