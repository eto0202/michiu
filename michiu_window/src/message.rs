use crate::{
    ElementState, Event, ImeContext, ImeStateUpdate, MichiuError, MichiuEvent, Modifiers,
    MouseButton, PhysicalPoint, PhysicalRect, PhysicalSize, SetWindowCommand, WM_RUN_ON_UI_THREAD,
    WM_WINDOW_COMMAND, WheelDelta, WindowId, WindowState, ZOrder,
};
use michiu_guard::Unvalidated;
use std::{
    any::{Any, TypeId},
    cell::{Cell, RefCell},
    collections::HashMap,
    rc::Rc,
};
use windows::{
    Win32::{
        Foundation::{HWND, LPARAM, LRESULT, RECT, WPARAM},
        UI::{
            Controls::WM_MOUSELEAVE,
            Input::{
                Ime::{GCS_COMPATTR, GCS_CURSORPOS, GCS_RESULTSTR, ImmGetCompositionStringW},
                KeyboardAndMouse::{
                    GetKeyState, ReleaseCapture, TME_LEAVE, TRACKMOUSEEVENT, TrackMouseEvent,
                    VIRTUAL_KEY, VK_CONTROL, VK_LWIN, VK_MENU, VK_RWIN, VK_SHIFT,
                },
            },
            WindowsAndMessaging::{
                DestroyWindow, DispatchMessageW, GetMessageW, HTCAPTION, HWND_BOTTOM,
                HWND_NOTOPMOST, HWND_TOPMOST, IsWindow, MSG, PM_REMOVE, PeekMessageW, PostMessageW,
                PostQuitMessage, SW_HIDE, SW_SHOW, SWP_NOACTIVATE, SWP_NOMOVE, SWP_NOSIZE,
                SWP_NOZORDER, SendMessageW, SetWindowPos, SetWindowTextW, ShowWindow,
                TranslateMessage, WM_CHAR, WM_CLOSE, WM_CREATE, WM_DESTROY, WM_DPICHANGED,
                WM_IME_COMPOSITION, WM_IME_ENDCOMPOSITION, WM_IME_NOTIFY, WM_IME_STARTCOMPOSITION,
                WM_INPUTLANGCHANGE, WM_KEYDOWN, WM_KEYUP, WM_KILLFOCUS, WM_LBUTTONDBLCLK,
                WM_LBUTTONDOWN, WM_LBUTTONUP, WM_MBUTTONDBLCLK, WM_MBUTTONDOWN, WM_MBUTTONUP,
                WM_MOUSEHWHEEL, WM_MOUSEMOVE, WM_MOUSEWHEEL, WM_MOVE, WM_NCLBUTTONDOWN, WM_PAINT,
                WM_QUIT, WM_RBUTTONDBLCLK, WM_RBUTTONDOWN, WM_RBUTTONUP, WM_SETCURSOR, WM_SETFOCUS,
                WM_SIZE, WM_SYSKEYDOWN, WM_SYSKEYUP, WM_USER,
            },
        },
    },
    core::PCWSTR,
};

/// For events originating from COM callbacks, such as `IDropTarget`,
/// the value is 0 because there is no corresponding Win32 message.
#[derive(Debug, Clone, Default)]
pub struct RawEvent {
    pub msg: u32,
    pub wparam: WPARAM,
    pub lparam: LPARAM,
}

pub type HandlerPtr = *mut (dyn FnMut(MichiuEvent, WindowId, RawEvent) + 'static);
thread_local! {
    pub static CURRENT_HANDLER: Cell<Option<HandlerPtr>> = const { Cell::new(None) };
    // 重複したメッセージポンプが同じスレッドで活性化されていないかを管理するフラグ
    static PUMP_ACTIVE: Cell<bool> = const { Cell::new(false) };
}

// 呼び出し側（translate_and_push や wnd_proc）で安全にクロージャを実行する関数
pub(crate) fn dispatch_to_active_handler(event: MichiuEvent, id: WindowId, raw: RawEvent) {
    CURRENT_HANDLER.with(|cell| {
        // take することで再帰呼び出し時の多重借用を防ぐ
        if let Some(ptr) = cell.take() {
            unsafe {
                (*ptr)(event, id, raw);
            }
            // 実行が終わったら元に戻す
            cell.set(Some(ptr));
        }
    });
}

pub struct HandlerGuard;
impl Drop for HandlerGuard {
    fn drop(&mut self) {
        CURRENT_HANDLER.with(|h| h.set(None));
    }
}

/// Message ID used internally for posting custom user-defined events ([`MichiuEvent::User`]) to the UI thread.
pub const WM_USER_EVENT: u32 = WM_USER + 101;

/// A thread-affine message pump responsible for polling OS messages and driving the event loop.
///
/// `EventPump` manages a thread-local FIFO event queue and processes Win32 window messages.
/// It must be instantiated and driven exclusively on the UI thread where the windows are created.
///
/// # Threading & Safety Constraints
/// Only one active `EventPump` should run per thread. Creating multiple `EventPump` instances on the
/// same thread will trigger a runtime warning via the `tracing` library to prevent message
/// competition and skipped events.
///
/// Dropping `EventPump` automatically flushes any unprocessed pointer-carrying messages (such as
/// asynchronous closures and commands) remaining in the thread's Win32 message queue to prevent memory leaks.
pub struct EventPump {
    _marker: std::marker::PhantomData<*const ()>,
}

impl EventPump {
    /// Creates a default configured `EventPump` instance.
    ///
    /// Warns if another `EventPump` is already active on the current thread.
    #[must_use]
    #[inline]
    pub fn new() -> Self {
        PUMP_ACTIVE.with(|active| {
                if active.get() {
                    // 重複生成されていた場合は開発ログに警告を出す
                    tracing::warn!(
                        "[michiu_window] Warning: Multiple EventPump instances created on the same thread. \
                         This will lead to event competition and skipped events!"
                    );
                }
                active.set(true);
            });
        Self {
            _marker: std::marker::PhantomData,
        }
    }

    /// Pause the thread until a message arrives, process one message, and then resume.
    ///
    /// # Errors
    /// Returns [`MichiuError::UnexpectedOsError`] if `GetMessageW` returns `-1`.
    pub fn wait_event<F>(&mut self, mut f: F) -> crate::Result<bool>
    where
        F: FnMut(MichiuEvent, WindowId, RawEvent),
    {
        let f_trait: &mut (dyn FnMut(MichiuEvent, WindowId, RawEvent) + '_) = &mut f;
        let erased_ptr: HandlerPtr = unsafe { std::mem::transmute(f_trait) };

        CURRENT_HANDLER.with(|h| h.set(Some(erased_ptr)));
        let _guard = HandlerGuard;

        unsafe {
            let mut msg = MSG::default();

            let res = GetMessageW(&raw mut msg, None, 0, 0);

            if res.0 == 0 {
                // WM_QUIT (0) を受信した場合は正常終了とみなし、残りのメモリをフラッシュ
                flush_remaining_pointer_messages();
                return Ok(false);
            } else if res.0 == -1 {
                // GetMessageW がエラー (-1) を返した場合は、OSエラーを安全に早期リターン
                return Err(MichiuError::UnexpectedOsError(
                    windows::core::Error::from_thread(),
                ));
            }

            let _ = TranslateMessage(&raw const msg);
            DispatchMessageW(&raw const msg);
        }
        Ok(true)
    }

    /// Process all messages in the queue in a non-blocking manner and return.
    pub fn poll_event<F>(&mut self, mut f: F)
    where
        F: FnMut(MichiuEvent, WindowId, RawEvent),
    {
        let f_trait: &mut (dyn FnMut(MichiuEvent, WindowId, RawEvent) + '_) = &mut f;
        let erased_ptr: HandlerPtr = unsafe { std::mem::transmute(f_trait) };

        CURRENT_HANDLER.with(|h| h.set(Some(erased_ptr)));
        let _guard = HandlerGuard;

        unsafe {
            let mut msg = MSG::default();
            // PeekMessageW でメッセージを非ブロッキング取得
            while PeekMessageW(&raw mut msg, None, 0, 0, PM_REMOVE).as_bool() {
                if msg.message == WM_QUIT {
                    break;
                }

                let is_ptr_message = msg.message == WM_WINDOW_COMMAND
                    || msg.message == WM_RUN_ON_UI_THREAD
                    || msg.message == WM_USER_EVENT;

                // HWND が無効（既に破棄されている）場合、DispatchMessageWを呼んでも
                // wnd_procは呼ばれないため、その場で即座にポインタを回収する
                if is_ptr_message && !msg.hwnd.is_invalid() && !IsWindow(Some(msg.hwnd)).as_bool() {
                    free_raw_message_pointer(msg.message, msg.lParam);
                    continue;
                }

                let _ = TranslateMessage(&raw const msg);
                DispatchMessageW(&raw const msg);
            }
        }
    }

    /// Retrieves and returns exactly one event from the queue.
    /// If there are no valid events, it immediately returns `None` (non-blocking).
    pub fn poll_one_event(&mut self) -> Option<MichiuEvent> {
        let mut captured: Option<MichiuEvent> = None;

        // 捕獲用の一時クロージャ
        let mut capture_fn = |event: MichiuEvent, _id: WindowId, _raw: RawEvent| {
            captured = Some(event);
        };

        // スレッドローカルに一時登録
        let f_trait: &mut (dyn FnMut(MichiuEvent, WindowId, RawEvent) + '_) = &mut capture_fn;
        let erased_ptr: HandlerPtr = unsafe { std::mem::transmute(f_trait) };

        CURRENT_HANDLER.with(|h| h.set(Some(erased_ptr)));
        let _guard = HandlerGuard;

        unsafe {
            let mut msg = MSG::default();

            // 1件ずつ Peek してディスパッチする
            while PeekMessageW(&raw mut msg, None, 0, 0, PM_REMOVE).as_bool() {
                if msg.message == WM_QUIT {
                    break;
                }

                // ポインタ型メッセージの安全性チェック）
                let is_ptr_message = msg.message == WM_WINDOW_COMMAND
                    || msg.message == WM_RUN_ON_UI_THREAD
                    || msg.message == WM_USER_EVENT;

                if is_ptr_message && !msg.hwnd.is_invalid() && !IsWindow(Some(msg.hwnd)).as_bool() {
                    free_raw_message_pointer(msg.message, msg.lParam);
                    continue;
                }

                let _ = TranslateMessage(&raw const msg);
                DispatchMessageW(&raw const msg);

                // translate_and_dispatch 経由で MichiuEvent が1つでも捕獲できたら即座にリターン
                // （残りのメッセージはメッセージキューに残る）
                if captured.is_some() {
                    break;
                }
            }
        }

        captured
    }

    /// Discard and clear all remaining unprocessed events.
    #[inline]
    pub fn clear_event(&mut self) {
        self.poll_event(|_, _, _| {});
    }
}

impl Default for EventPump {
    fn default() -> Self {
        Self::new()
    }
}

// EventPump 破棄時に活性化フラグをリセットし、スレッド内で次のポンプ生成を許容する
impl Drop for EventPump {
    fn drop(&mut self) {
        PUMP_ACTIVE.with(|active| {
            active.set(false);
        });
        unsafe {
            flush_remaining_pointer_messages();
        }
    }
}

/// Parses received Win32 messages, translates them into safe Events, and stores them.
///
/// If `Some(LRESULT)` is returned,
/// the `WndProc` bypasses further processing (such as `DefWindowProcW`) and returns immediately.
/// If `None` is returned, processing is not bypassed;
/// instead, the message is passed directly to the OS's default handling.
#[allow(clippy::too_many_lines)]
pub(crate) fn translate_and_dispatch(
    hwnd: HWND,
    msg: u32,
    wparam: WPARAM,
    lparam: LPARAM,
    state: &mut WindowState,
) -> Option<LRESULT> {
    let id = WindowId(hwnd.0 as isize);
    let raw = RawEvent {
        msg,
        wparam,
        lparam,
    };

    // 渡された Event を自動的に Event::Event に包んでキューに積む
    let push_win_event = |e: Event| {
        dispatch_to_active_handler(MichiuEvent::Window { id, event: e }, id, raw.clone());
    };

    match msg {
        WM_CREATE => {
            push_win_event(Event::Created);
            None
        }
        WM_CLOSE => {
            push_win_event(Event::CloseRequested);
            Some(LRESULT(0))
        }
        WM_DESTROY => {
            push_win_event(Event::Destroyed);
            // DefWindowProcW を通して解体を続けさせたいので None を返す
            None
        }
        WM_SIZE => {
            let width = (lparam.0 & 0xffff) as i32;
            let height = ((lparam.0 >> 16) & 0xffff) as i32;
            push_win_event(Event::Resized(Unvalidated::new(PhysicalSize {
                width,
                height,
            })));
            None
        }
        WM_MOVE => {
            let x = (lparam.0 & 0xffff) as i16 as i32;
            let y = ((lparam.0 >> 16) & 0xffff) as i16 as i32;
            push_win_event(Event::Moved(Unvalidated::new(PhysicalPoint::new(x, y))));
            None
        }
        WM_SETFOCUS => {
            push_win_event(Event::Focused(true));
            None
        }
        WM_KILLFOCUS => {
            push_win_event(Event::Focused(false));
            None
        }
        WM_KEYDOWN | WM_SYSKEYDOWN => {
            let key_code = VIRTUAL_KEY(wparam.0 as u16);
            push_win_event(Event::KeyboardInput {
                key_code: Unvalidated::new(key_code),
                modifiers: get_active_modifiers(),
                state: ElementState::Pressed,
            });
            None
        }
        WM_KEYUP | WM_SYSKEYUP => {
            let key_code = VIRTUAL_KEY(wparam.0 as u16);
            push_win_event(Event::KeyboardInput {
                key_code: Unvalidated::new(key_code),
                modifiers: get_active_modifiers(),
                state: ElementState::Released,
            });
            None
        }
        WM_CHAR => {
            if let Some(c) = char::from_u32(wparam.0 as u32) {
                push_win_event(Event::CharacterInput(c));
            }
            None
        }
        WM_MOUSEMOVE => {
            if !state.is_cursor_inside {
                state.is_cursor_inside = true;
                push_win_event(Event::CursorEntered);

                let mut tme = TRACKMOUSEEVENT {
                    cbSize: std::mem::size_of::<TRACKMOUSEEVENT>() as u32,
                    dwFlags: TME_LEAVE,
                    hwndTrack: hwnd,
                    dwHoverTime: 0,
                };
                unsafe {
                    let _ = TrackMouseEvent(&raw mut tme);
                }
            }

            let x = (lparam.0 & 0xffff) as i16 as i32;
            let y = ((lparam.0 >> 16) & 0xffff) as i16 as i32;

            push_win_event(Event::CursorMoved {
                position: Unvalidated::new(PhysicalPoint::new(x, y)),
            });
            None
        }
        WM_MOUSELEAVE => {
            state.is_cursor_inside = false;
            push_win_event(Event::CursorLeft);
            None
        }
        WM_LBUTTONDOWN | WM_LBUTTONDBLCLK => {
            push_win_event(Event::MouseInput {
                button: MouseButton::Left,
                modifiers: get_active_modifiers(),
                state: ElementState::Pressed,
                click_count: if msg == WM_LBUTTONDBLCLK { 2 } else { 1 },
            });
            None
        }
        WM_LBUTTONUP => {
            push_win_event(Event::MouseInput {
                button: MouseButton::Left,
                modifiers: get_active_modifiers(),
                state: ElementState::Released,
                click_count: 1,
            });
            None
        }
        WM_RBUTTONDOWN | WM_RBUTTONDBLCLK => {
            push_win_event(Event::MouseInput {
                button: MouseButton::Right,
                modifiers: get_active_modifiers(),
                state: ElementState::Pressed,
                click_count: if msg == WM_RBUTTONDBLCLK { 2 } else { 1 },
            });
            None
        }
        WM_RBUTTONUP => {
            push_win_event(Event::MouseInput {
                button: MouseButton::Right,
                modifiers: get_active_modifiers(),
                state: ElementState::Released,
                click_count: 1,
            });
            None
        }
        WM_MBUTTONDOWN | WM_MBUTTONDBLCLK => {
            push_win_event(Event::MouseInput {
                button: MouseButton::Middle,
                modifiers: get_active_modifiers(),
                state: ElementState::Pressed,
                click_count: if msg == WM_MBUTTONDBLCLK { 2 } else { 1 },
            });
            None
        }
        WM_MBUTTONUP => {
            push_win_event(Event::MouseInput {
                button: MouseButton::Middle,
                modifiers: get_active_modifiers(),
                state: ElementState::Released,
                click_count: 1,
            });
            None
        }
        WM_MOUSEWHEEL => {
            let delta = (wparam.0 >> 16) as i32;
            push_win_event(Event::MouseWheel {
                raw_delta_x: Unvalidated::new(WheelDelta(0)),
                raw_delta_y: Unvalidated::new(WheelDelta(delta)), // 上がプラス、下がマイナス
            });
            None
        }
        WM_MOUSEHWHEEL => {
            let delta = (wparam.0 >> 16) as i32;
            push_win_event(Event::MouseWheel {
                raw_delta_x: Unvalidated::new(WheelDelta(delta)), // 右がプラス、左がマイナス
                raw_delta_y: Unvalidated::new(WheelDelta(0)),
            });
            None
        }
        WM_PAINT => {
            push_win_event(Event::RedrawRequested);
            // DefWindowProcW のデフォルト描画をスキップ
            Some(LRESULT(0))
        }

        WM_DPICHANGED => {
            // wparam の下位16ビット（LOWORD）に新しいDPI（96, 120, 144, 192など）
            let dpi = (wparam.0 & 0xffff) as u32;
            let scale_factor = dpi as f64 / 96.0; // 96 DPI = 1.0 (100%)

            // lparam には新しいDPIにスケーリングされた推奨サイズ（RECT構造体のポインタ）
            let rect_ptr = lparam.0 as *const RECT;
            if !rect_ptr.is_null() {
                let rect = unsafe { *rect_ptr };
                let bounds = PhysicalRect::new(rect.left, rect.top, rect.right, rect.bottom);

                push_win_event(Event::ScaleFactorChanged {
                    scale_factor,
                    suggested_bounds: Unvalidated::new(bounds),
                });

                if state.auto_dpi_scaling {
                    let rect_ptr = lparam.0 as *const RECT;
                    if !rect_ptr.is_null() {
                        let rect = unsafe { *rect_ptr };
                        unsafe {
                            let _ = SetWindowPos(
                                hwnd,
                                None,
                                rect.left,
                                rect.top,
                                rect.right - rect.left,
                                rect.bottom - rect.top,
                                SWP_NOZORDER | SWP_NOACTIVATE,
                            );
                        }
                    }
                    return Some(LRESULT(0));
                }
            }
            None
        }
        WM_SETCURSOR => {
            // wparam にはウィンドウのHWND、lparam の下位16ビットにはヒットテスト値が入る。
            // ヒットテスト値が HTERROR などのエラーでない場合、かつクライアント領域にある場合はカーソルを上書き
            let hit_test = (lparam.0 & 0xffff) as u32;

            // クライアント領域 (HTCLIENT = 1) での移動時のみカスタムカーソルを適用
            if hit_test == 1 {
                let result = crate::apply_cursor_icon_impl(hwnd, state.current_cursor);
                return Some(result); // DefWindowProcW の呼び出しをバイパス
            }
            None
        }
        // IME関連メッセージ群の統合フック
        // DefWindowProcW に流すことで、OS標準の予測変換候補ウィンドウは正常表示
        WM_IME_STARTCOMPOSITION
        | WM_IME_ENDCOMPOSITION
        | WM_IME_COMPOSITION
        | WM_INPUTLANGCHANGE => {
            push_ime_state_update(hwnd, id, raw, state);

            if !state.default_composition_window {
                return Some(LRESULT(0));
            }

            None
        }
        WM_IME_NOTIFY => {
            let sub_msg = wparam.0 as u32;
            // 状態変更通知 (IMN_SETOPENSTATUS = 0x000F, IMN_SETCONVERSIONMODE = 0x0006) のみフック
            if sub_msg == 0x000F || sub_msg == 0x0006 {
                push_ime_state_update(hwnd, id, raw, state);
            }
            None
        }
        WM_WINDOW_COMMAND => {
            let raw_ptr = lparam.0 as *mut SetWindowCommand;
            if !raw_ptr.is_null() {
                unsafe {
                    let command = Box::from_raw(raw_ptr);

                    match *command {
                        SetWindowCommand::Title(title) => {
                            let title_wide: Vec<u16> =
                                title.encode_utf16().chain(std::iter::once(0)).collect();
                            let _ = SetWindowTextW(hwnd, PCWSTR(title_wide.as_ptr()));
                        }
                        SetWindowCommand::Visible(visible) => {
                            let show_cmd = if visible { SW_SHOW } else { SW_HIDE };
                            let _ = ShowWindow(hwnd, show_cmd);
                        }
                        SetWindowCommand::ZOrder(z_order) => {
                            let hwnd_insert_after = match z_order {
                                ZOrder::Topmost => HWND_TOPMOST,
                                ZOrder::Default => HWND_NOTOPMOST,
                                ZOrder::Bottom => HWND_BOTTOM,
                            };

                            let _ = SetWindowPos(
                                hwnd,
                                Some(hwnd_insert_after),
                                0,
                                0,
                                0,
                                0,
                                SWP_NOMOVE | SWP_NOSIZE | SWP_NOACTIVATE,
                            );
                        }
                        SetWindowCommand::Size(size) => {
                            let _ = SetWindowPos(
                                hwnd,
                                Some(HWND::default()),
                                0,
                                0,
                                size.width,
                                size.height,
                                SWP_NOMOVE | SWP_NOZORDER | SWP_NOACTIVATE,
                            );
                        }
                        SetWindowCommand::Position(position) => {
                            let _ = SetWindowPos(
                                hwnd,
                                Some(HWND::default()),
                                position.x,
                                position.y,
                                0,
                                0,
                                SWP_NOSIZE | SWP_NOZORDER | SWP_NOACTIVATE,
                            );
                        }
                        SetWindowCommand::SetClipboardText(text) => {
                            let _ = crate::set_clipboard_text_impl(hwnd, &text);
                        }
                        SetWindowCommand::CursorCapture(capture) => {
                            crate::set_cursor_capture_impl(hwnd, capture);
                        }
                        SetWindowCommand::CursorClipping(clip) => {
                            crate::set_cursor_clipping_impl(hwnd, clip);
                        }
                        SetWindowCommand::Fullscreen(fullscreen) => {
                            crate::set_fullscreen_impl(hwnd, fullscreen);
                        }
                        SetWindowCommand::CenterOnScreen => {
                            crate::center_on_screen_impl(hwnd);
                        }
                        SetWindowCommand::StartDragging => {
                            let _ = ReleaseCapture();
                            SendMessageW(
                                hwnd,
                                WM_NCLBUTTONDOWN,
                                Some(WPARAM(HTCAPTION as usize)),
                                Some(LPARAM(0)),
                            );
                        }
                        SetWindowCommand::SetCursor(cursor) => {
                            state.current_cursor = cursor;
                            // その場でカーソルを即座に更新
                            let _ = crate::apply_cursor_icon_impl(hwnd, cursor);
                        }
                        SetWindowCommand::Destroy => {
                            let is_alive = IsWindow(Some(hwnd)).as_bool();
                            if is_alive {
                                let _ = DestroyWindow(hwnd);
                            }
                        }
                        SetWindowCommand::Quit => {
                            PostQuitMessage(0);
                        }
                    }
                }
            }

            Some(LRESULT(0))
        }
        WM_RUN_ON_UI_THREAD => {
            let raw_ptr = lparam.0 as *mut Box<dyn FnOnce(HWND) + Send + 'static>;
            if !raw_ptr.is_null() {
                unsafe {
                    let closure = Box::from_raw(raw_ptr);

                    // ユーザーのクロージャが万が一パニックした場合、
                    // WndProc（FFIの境界）を越えて Unwind が起きると致命的なクラッシュ（未定義動作）を引き起こす。
                    // std::panic::catch_unwind を使って安全に保護します。
                    let _ = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                        closure(hwnd);
                    }));
                }
            }

            Some(LRESULT(0))
        }
        WM_USER_EVENT => {
            let raw_ptr = lparam.0 as *mut Box<dyn Any + Send>;
            if !raw_ptr.is_null() {
                unsafe {
                    let boxed_any = Box::from_raw(raw_ptr);
                    dispatch_to_active_handler(MichiuEvent::User(*boxed_any), id, raw);
                }
            }

            Some(LRESULT(0)) // 処理完了
        }
        _ => {
            dispatch_to_active_handler(MichiuEvent::Other, id, raw);
            None
        }
    }
}

// 装飾キーの現在の押し込み状態を非同期的にポーリング。
#[inline]
fn get_active_modifiers() -> Modifiers {
    let mut modifiers = Modifiers::empty();
    unsafe {
        // 戻り値がマイナスならキーは押されていると判定
        if GetKeyState(VK_SHIFT.0 as i32) < 0 {
            modifiers.insert(Modifiers::SHIFT);
        }
        if GetKeyState(VK_CONTROL.0 as i32) < 0 {
            modifiers.insert(Modifiers::CONTROL);
        }
        if GetKeyState(VK_MENU.0 as i32) < 0 {
            modifiers.insert(Modifiers::ALT);
        }
        if GetKeyState(VK_LWIN.0 as i32) < 0 || GetKeyState(VK_RWIN.0 as i32) < 0 {
            modifiers.insert(Modifiers::LOGO);
        }
    }
    modifiers
}

/// A thread-safe, cloneable sender handle used to dispatch user-defined events from background threads to the UI thread.
///
/// `EventSender` wraps the target window's raw handle and leverages `PostMessageW` (with [`WM_USER_EVENT`])
/// to safely marshal arbitrary types `T: Any + Send + 'static` across thread boundaries.
///
/// Dispatched events are received by the UI thread's [`EventPump`] as [`MichiuEvent::User`].
/// Since it utilizes Win32's OS message queue under the hood, calling `send_event` automatically and
/// safely wakes up the UI thread's message loop if it was asleep.
#[derive(Clone)]
pub struct EventSender {
    hwnd: HWND,
}

unsafe impl Send for EventSender {}
unsafe impl Sync for EventSender {}

impl EventSender {
    /// Creates a default configured `EventSender` instance.
    #[must_use]
    #[inline]
    pub fn new(hwnd: HWND) -> Self {
        Self { hwnd }
    }

    /// Dispatches an arbitrary custom event from a background thread to the UI thread.
    ///
    /// Under the hood, this converts the event into a type-erased raw pointer (`Box<dyn Any + Send>`)
    /// and posts it to the UI thread via `PostMessageW`.
    ///
    /// # Examples
    ///
    /// ```no_run
    /// # use michiu_window::{Window, WindowBuilder, EventSender, MichiuEvent};
    /// # struct MyData { score: u32 }
    /// # fn main() -> Result<(), Box<dyn std::error::Error>> {
    /// # let window = Window::build(WindowBuilder::new().into_unvalidated().try_into()?)?;
    /// let sender = window.handle().assume_valid().sender();
    ///
    /// std::thread::spawn(move || {
    ///     // Process some heavy tasks...
    ///     let result = MyData { score: 100 };
    ///
    ///     // Send the result safely to the UI thread
    ///     sender.send_event(result);
    /// });
    /// # Ok(())
    /// # }
    /// ```
    #[inline]
    pub fn send_event<T: Any + Send + 'static>(&self, event: T) {
        let boxed: Box<dyn Any + Send> = Box::new(event);
        self.send_event_inner(boxed);
    }

    fn send_event_inner(&self, boxed: Box<dyn Any + Send>) {
        // Fatポインタを LPARAM (1ポインタ幅) に収めるためにダブルボクシングして生ポインタ化
        let raw_ptr = Box::into_raw(Box::new(boxed));

        // PostMessageW を呼んでUIスレッドのメッセージキューに投げる
        // LPARAM に生ポインタを乗せて引き渡す
        let _ = unsafe {
            PostMessageW(
                Some(self.hwnd),
                WM_USER_EVENT,
                WPARAM(0),
                LPARAM(raw_ptr as isize),
            )
        };
    }
}

// LPARAM から生ポインタを復元し、安全にヒープメモリを解放する
fn free_raw_message_pointer(message: u32, lparam: LPARAM) {
    let raw_ptr = lparam.0;
    if raw_ptr == 0 {
        return;
    }

    // 解放処理中の予期せぬパニックがFFI境界を破壊するのを防ぐ
    let _ = std::panic::catch_unwind(std::panic::AssertUnwindSafe(move || match message {
        WM_WINDOW_COMMAND => {
            let _ = unsafe { Box::from_raw(raw_ptr as *mut SetWindowCommand) };
        }
        WM_RUN_ON_UI_THREAD => {
            let _ =
                unsafe { Box::from_raw(raw_ptr as *mut Box<dyn FnOnce(HWND) + Send + 'static>) };
        }
        WM_USER_EVENT => {
            let _ = unsafe { Box::from_raw(raw_ptr as *mut Box<dyn Any + Send>) };
        }
        _ => {}
    }));
}

// メッセージキューに残存するすべてのポインタ型メッセージを解放
unsafe fn flush_remaining_pointer_messages() {
    let mut msg = unsafe { std::mem::zeroed() };
    // スレッドメッセージキューからカスタムメッセージの範囲 (WM_USER ～ WM_USER + 200) を
    // PM_REMOVE で全て回収しポインタを解放する
    while unsafe { PeekMessageW(&raw mut msg, None, WM_USER, WM_USER + 200, PM_REMOVE) }.as_bool() {
        if msg.message == WM_WINDOW_COMMAND
            || msg.message == WM_RUN_ON_UI_THREAD
            || msg.message == WM_USER_EVENT
        {
            free_raw_message_pointer(msg.message, msg.lParam);
        }
    }
}

// コールバック関数を保持するための型
type Listener = Rc<dyn Fn(&dyn Any)>;

/// A lightweight, single-threaded Publish/Subscribe event bus designed for high-performance, synchronous GUI event routing.
///
/// Based entirely on `Rc` and `RefCell`, `EventBus` has absolutely zero thread-locking overhead,
/// making it ideal for rendering cycles and frame-by-frame updates (e.g., 60 FPS loops).
///
/// It is designed exclusively for UI thread communications. To prevent `RefCell` double-borrow panics,
/// it safely supports nested publishes (publishing an event within another event's subscriber callback)
/// and dynamic subscriptions by cloning the listener lists before triggering dispatch.
#[derive(Clone, Default)]
pub struct EventBus {
    listeners: Rc<RefCell<HashMap<TypeId, Vec<Listener>>>>,
}

impl EventBus {
    /// Creates a new, empty `EventBus` instance.
    #[must_use]
    #[inline]
    pub fn new() -> Self {
        Self {
            listeners: Rc::new(RefCell::new(HashMap::new())),
        }
    }

    /// Registers a callback closure that triggers when an event of type `T` is published.
    ///
    /// # Examples
    ///
    /// ```
    /// # use michiu_window::EventBus;
    /// # struct ScoreUpdate { player: String, points: u32 }
    /// let bus = EventBus::new();
    ///
    /// bus.subscribe(|event: &ScoreUpdate| {
    ///     println!("Player {} scored {} points!", event.player, event.points);
    /// });
    /// ```
    pub fn subscribe<T, F>(&self, callback: F)
    where
        T: Any,
        F: Fn(&T) + 'static,
    {
        let type_id = TypeId::of::<T>();

        // 渡されたコールバックを型消去された &dyn Any を受け取るラッパーで包む
        let wrapper: Listener = Rc::new(move |any_event: &dyn Any| {
            // 呼び出し時に元の型 T にダウンキャストできればコールバックを実行
            if let Some(event) = any_event.downcast_ref::<T>() {
                callback(event);
            }
        });

        self.listeners
            .borrow_mut()
            .entry(type_id)
            .or_default()
            .push(wrapper);
    }

    /// Broadcasts an event of type `T` and synchronously propagates it to all subscribed callbacks.
    ///
    /// # Examples
    ///
    /// ```
    /// # use michiu_window::EventBus;
    /// # struct ScoreUpdate { player: String, points: u32 }
    /// # let bus = EventBus::new();
    /// // Triggers all callbacks registered for `ScoreUpdate` synchronously
    /// bus.publish(&ScoreUpdate {
    ///     player: "Michiu".to_string(),
    ///     points: 150,
    /// });
    /// ```
    pub fn publish<T: Any>(&self, event: &T) {
        let type_id = TypeId::of::<T>();

        // コールバックの中でさらに publish や subscribe が呼ばれても
        // RefCell の Borrow パニックが起きないようにするため、対象リスナーのリスト (Rcのベクタ)
        // を一時的にクローンして取り出し、borrow のスコープを即座に終了
        let listeners_to_call = {
            let map = self.listeners.borrow();
            map.get(&type_id).cloned().unwrap_or_default()
        };

        // 借用が外れた安全な状態でコールバックを順次実行
        for listener in listeners_to_call {
            listener(event); // 型消去された状態として渡す
        }
    }
}

fn push_ime_state_update(hwnd: HWND, id: WindowId, raw: RawEvent, state: &WindowState) {
    // ImeContext を生成して現在の最新状態を一括クエリする
    if let Ok(ctx) = ImeContext::new(hwnd) {
        let is_open = ctx.is_open();
        let (conversion_mode, sentence_mode) = ctx.get_conversion_status();
        let keyboard_layout_id = crate::get_active_keyboard_layout_id();

        // 変換中および確定文字列を取得 (Result<Option<String>> のため安全にフラット化)
        let composition_text = ctx
            .get_composition_string()
            .unwrap_or_default()
            .unwrap_or_default();

        let has_result_flag =
            raw.msg == WM_IME_COMPOSITION && (raw.lparam.0 as u32 & GCS_RESULTSTR.0) != 0;

        let result_text = if has_result_flag {
            ctx.get_result_string()
                .unwrap_or_default()
                .unwrap_or_default()
        } else {
            String::new() // WM_IME_ENDCOMPOSITION 等では空文字にする！
        };

        let caret_position = ctx.get_composition_window_position();

        // WM_IME_COMPOSITION かつ該当フラグが立っている時だけ取得する
        let is_composition_msg = raw.msg == WM_IME_COMPOSITION;
        let lparam_flags = raw.lparam.0 as u32;

        // カーソル位置の取得
        let composition_cursor = if is_composition_msg && (lparam_flags & GCS_CURSORPOS.0) != 0 {
            let cursor_pos = unsafe { ImmGetCompositionStringW(ctx.himc, GCS_CURSORPOS, None, 0) };
            if cursor_pos >= 0 {
                cursor_pos as usize
            } else {
                0
            }
        } else {
            0
        };

        // 文字属性配列（下線情報）の取得
        let composition_attrs = if is_composition_msg && (lparam_flags & GCS_COMPATTR.0) != 0 {
            let len = unsafe { ImmGetCompositionStringW(ctx.himc, GCS_COMPATTR, None, 0) };
            if len > 0 {
                let mut attrs = vec![0u8; len as usize];
                let written = unsafe {
                    ImmGetCompositionStringW(
                        ctx.himc,
                        GCS_COMPATTR,
                        Some(attrs.as_mut_ptr().cast()),
                        len as u32,
                    )
                };
                if written > 0 {
                    attrs.truncate(written as usize);
                    attrs
                } else {
                    Vec::new()
                }
            } else {
                Vec::new()
            }
        } else {
            Vec::new()
        };

        let update = ImeStateUpdate {
            window_id: id.0,
            is_open,
            conversion_mode,
            sentence_mode,
            keyboard_layout_id,
            composition_text,
            result_text,
            caret_position,
            composition_cursor,
            composition_attrs,
        };

        // オプトインされた中継サーバーが有効な場合は即座に外部へプッシュ配信
        if let Some(ref relay) = state.ime_relay {
            relay.send(update.clone());
        }

        // 本ライブラリのメインイベントキューへバンドルイベントとしてプッシュ
        dispatch_to_active_handler(
            MichiuEvent::Window {
                id,
                event: Event::Ime(Unvalidated::new(update)),
            },
            id,
            raw,
        );
    }
}

#[cfg(test)]
mod tests;
