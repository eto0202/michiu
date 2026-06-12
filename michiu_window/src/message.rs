use crate::{
    ElementState, ImeContext, ImeStateUpdate, MichiuEvent, Modifiers, MouseButton, PhysicalPoint,
    PhysicalRect, PhysicalSize, SetWindowCommand, WM_RUN_ON_UI_THREAD, WM_WINDOW_COMMAND,
    WindowEvent, WindowId, WindowState,
};
use michiu_guard::Unvalidated;
use std::{
    any::{Any, TypeId},
    cell::{Cell, RefCell},
    collections::{HashMap, VecDeque},
    rc::Rc,
};
use windows::{
    Win32::{
        Foundation::{HWND, LPARAM, LRESULT, RECT, WPARAM},
        Graphics::Gdi::{BeginPaint, EndPaint, PAINTSTRUCT},
        UI::{
            Controls::WM_MOUSELEAVE,
            Input::KeyboardAndMouse::{
                GetKeyState, ReleaseCapture, TME_LEAVE, TRACKMOUSEEVENT, TrackMouseEvent,
                VIRTUAL_KEY, VK_CONTROL, VK_LWIN, VK_MENU, VK_RWIN, VK_SHIFT,
            },
            WindowsAndMessaging::{
                DispatchMessageW, HTCAPTION, PM_REMOVE, PeekMessageW, PostMessageW, SW_HIDE,
                SW_SHOW, SWP_NOACTIVATE, SWP_NOMOVE, SWP_NOSIZE, SWP_NOZORDER, SendMessageW,
                SetWindowPos, SetWindowTextW, ShowWindow, TranslateMessage, WM_CHAR, WM_CLOSE,
                WM_CREATE, WM_DESTROY, WM_DPICHANGED, WM_IME_COMPOSITION, WM_IME_ENDCOMPOSITION,
                WM_IME_NOTIFY, WM_IME_STARTCOMPOSITION, WM_INPUTLANGCHANGE, WM_KEYDOWN, WM_KEYUP,
                WM_KILLFOCUS, WM_LBUTTONDOWN, WM_LBUTTONUP, WM_MBUTTONDOWN, WM_MBUTTONUP,
                WM_MOUSEMOVE, WM_MOUSEWHEEL, WM_MOVE, WM_NCLBUTTONDOWN, WM_PAINT, WM_QUIT,
                WM_RBUTTONDOWN, WM_RBUTTONUP, WM_SETCURSOR, WM_SETFOCUS, WM_SIZE, WM_SYSKEYDOWN,
                WM_SYSKEYUP, WM_USER,
            },
        },
    },
    core::PCWSTR,
};

thread_local! {
    static EVENT_QUEUE: RefCell<VecDeque<MichiuEvent>> = const { RefCell::new(VecDeque::new()) };
    // 重複したメッセージポンプが同じスレッドで活性化されていないかを管理するフラグ
    static PUMP_ACTIVE: Cell<bool> = const { Cell::new(false) };
}

pub const WM_USER_EVENT: u32 = WM_USER + 101;

// WndProc の中でイベントを蓄積する関数
pub(crate) fn push_event(event: MichiuEvent) {
    EVENT_QUEUE.with(|q| q.borrow_mut().push_back(event));
}

pub struct EventPump {
    _marker: std::marker::PhantomData<*const ()>,
}

impl Default for EventPump {
    fn default() -> Self {
        Self::new()
    }
}

impl EventPump {
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

    /// Retrieves a single event (non-blocking).
    pub fn poll_event(&mut self) -> Option<MichiuEvent> {
        // すでにキューにたまっているイベントがあればそれを返す
        if let Some(event) = EVENT_QUEUE.with(|q| q.borrow_mut().pop_front()) {
            return Some(event);
        }

        // キューが空ならOSのメッセージキューからメッセージを処理する
        unsafe {
            let mut msg = std::mem::zeroed();
            // PeekMessageW でメッセージを非ブロッキング取得
            while PeekMessageW(&mut msg, None, 0, 0, PM_REMOVE).as_bool() {
                // WM_QUIT (PostQuitMessage) が送られてきた場合の終了ハンドリング
                if msg.message == WM_QUIT {
                    break;
                }

                let is_ptr_message = msg.message == WM_WINDOW_COMMAND
                    || msg.message == WM_RUN_ON_UI_THREAD
                    || msg.message == WM_USER_EVENT;

                if is_ptr_message {
                    use windows::Win32::UI::WindowsAndMessaging::IsWindow;
                    // HWND が無効（既に破棄されている）場合、DispatchMessageWを呼んでも
                    // wnd_procは呼ばれないため、その場で即座にポインタを回収する
                    if !msg.hwnd.is_invalid() && !IsWindow(Some(msg.hwnd)).as_bool() {
                        free_raw_message_pointer(msg.message, msg.lParam);
                        continue;
                    }
                }

                let _ = TranslateMessage(&msg);
                DispatchMessageW(&msg);

                // メッセージを1つ処理した結果、イベントキューに何か入ったら
                // 1フレーム遅延を防ぐためにループを抜けて即座に呼び出し元に返す
                if !EVENT_QUEUE.with(|q| q.borrow().is_empty()) {
                    break;
                }
            }
        }

        // 溜まったイベントの先頭を返す（無ければ None）
        EVENT_QUEUE.with(|q| q.borrow_mut().pop_front())
    }
}

/// Parses received Win32 messages, translates them into safe WindowEvents, and stores them.
///
/// If `Some(LRESULT)` is returned,
/// the WndProc bypasses further processing (such as DefWindowProcW) and returns immediately.
/// If `None` is returned, processing is not bypassed;
/// instead, the message is passed directly to the OS's default handling.
pub(crate) fn translate_and_push(
    hwnd: HWND,
    msg: u32,
    wparam: WPARAM,
    lparam: LPARAM,
    state: &mut WindowState,
) -> Option<LRESULT> {
    // 渡された WindowEvent を自動的に Event::WindowEvent に包んでキューに積む
    let push_win_event = |e: WindowEvent| {
        push_event(MichiuEvent::WindowEvent {
            window_id: WindowId(hwnd.0 as isize),
            event: e,
        });
    };

    match msg {
        WM_CREATE => {
            push_win_event(WindowEvent::Created);
            None // DefWindowProcW に流して正常に初期化を完了させる
        }
        WM_CLOSE => {
            push_win_event(WindowEvent::CloseRequested);
            // 早期リターンして DefWindowProcW に渡るのをせき止める
            Some(LRESULT(0))
        }
        WM_DESTROY => {
            push_win_event(WindowEvent::Destroyed);
            // DefWindowProcW を通して解体を続けさせたいので None を返す
            None
        }
        WM_SIZE => {
            let width = (lparam.0 & 0xffff) as i32;
            let height = ((lparam.0 >> 16) & 0xffff) as i32;
            push_win_event(WindowEvent::Resized(Unvalidated::new(PhysicalSize::new(
                width, height,
            ))));
            None
        }
        WM_MOVE => {
            let x = (lparam.0 & 0xffff) as i16 as i32;
            let y = ((lparam.0 >> 16) & 0xffff) as i16 as i32;
            push_win_event(WindowEvent::Moved(Unvalidated::new(PhysicalPoint::new(
                x, y,
            ))));
            None
        }
        WM_SETFOCUS => {
            push_win_event(WindowEvent::Focused(true));
            None
        }
        WM_KILLFOCUS => {
            push_win_event(WindowEvent::Focused(false));
            None
        }
        WM_KEYDOWN | WM_SYSKEYDOWN => {
            let key_code = VIRTUAL_KEY(wparam.0 as u16);
            push_win_event(WindowEvent::KeyboardInput {
                key_code: Unvalidated::new(key_code),
                modifiers: get_active_modifiers(),
                state: ElementState::Pressed,
            });
            None
        }
        WM_KEYUP | WM_SYSKEYUP => {
            let key_code = VIRTUAL_KEY(wparam.0 as u16);
            push_win_event(WindowEvent::KeyboardInput {
                key_code: Unvalidated::new(key_code),
                modifiers: get_active_modifiers(),
                state: ElementState::Released,
            });
            None
        }
        WM_CHAR => {
            if let Some(c) = char::from_u32(wparam.0 as u32) {
                push_win_event(WindowEvent::CharacterInput(c));
            }
            None
        }
        WM_MOUSEMOVE => {
            if !state.is_cursor_inside {
                state.is_cursor_inside = true;
                push_win_event(WindowEvent::CursorEntered);

                let mut tme = TRACKMOUSEEVENT {
                    cbSize: std::mem::size_of::<TRACKMOUSEEVENT>() as u32,
                    dwFlags: TME_LEAVE,
                    hwndTrack: hwnd,
                    dwHoverTime: 0,
                };
                unsafe {
                    let _ = TrackMouseEvent(&mut tme);
                }
            }

            let x = (lparam.0 & 0xffff) as i16 as i32;
            let y = ((lparam.0 >> 16) & 0xffff) as i16 as i32;

            push_win_event(WindowEvent::CursorMoved {
                position: Unvalidated::new(PhysicalPoint::new(x, y)),
            });
            None
        }
        WM_MOUSELEAVE => {
            state.is_cursor_inside = false;
            push_win_event(WindowEvent::CursorLeft);
            None
        }
        WM_LBUTTONDOWN => {
            push_win_event(WindowEvent::MouseInput {
                button: MouseButton::Left,
                modifiers: get_active_modifiers(),
                state: ElementState::Pressed,
            });
            None
        }
        WM_LBUTTONUP => {
            push_win_event(WindowEvent::MouseInput {
                button: MouseButton::Left,
                modifiers: get_active_modifiers(),
                state: ElementState::Released,
            });
            None
        }
        WM_RBUTTONDOWN => {
            push_win_event(WindowEvent::MouseInput {
                button: MouseButton::Right,
                modifiers: get_active_modifiers(),
                state: ElementState::Pressed,
            });
            None
        }
        WM_RBUTTONUP => {
            push_win_event(WindowEvent::MouseInput {
                button: MouseButton::Right,
                modifiers: get_active_modifiers(),
                state: ElementState::Released,
            });
            None
        }
        WM_MBUTTONDOWN => {
            push_win_event(WindowEvent::MouseInput {
                button: MouseButton::Middle,
                modifiers: get_active_modifiers(),
                state: ElementState::Pressed,
            });
            None
        }
        WM_MBUTTONUP => {
            push_win_event(WindowEvent::MouseInput {
                button: MouseButton::Middle,
                modifiers: get_active_modifiers(),
                state: ElementState::Released,
            });
            None
        }
        WM_MOUSEWHEEL => {
            // wparam の上位16ビットに回転量
            // （WHEEL_DELTA = 120 の倍数が入るため、120で割って値を規格化する）
            let delta = (wparam.0 >> 16) as i16 as f32 / 120.0;
            push_win_event(WindowEvent::MouseWheel { delta });
            None
        }
        WM_PAINT => {
            let mut ps = PAINTSTRUCT::default();
            unsafe {
                let _hdc = BeginPaint(hwnd, &mut ps);
                push_win_event(WindowEvent::RedrawRequested);
                let _ = EndPaint(hwnd, &ps);
            }
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

                push_win_event(WindowEvent::ScaleFactorChanged {
                    scale_factor,
                    suggested_bounds: Unvalidated::new(bounds),
                });
            }
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
                Some(LRESULT(0))
            } else {
                None
            }
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
            push_ime_state_update(hwnd, state);
            None
        }
        WM_IME_NOTIFY => {
            let sub_msg = wparam.0 as u32;
            // 状態変更通知 (IMN_SETOPENSTATUS = 0x000F, IMN_SETCONVERSIONMODE = 0x0006) のみフック
            if sub_msg == 0x000F || sub_msg == 0x0006 {
                push_ime_state_update(hwnd, state);
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
                    push_event(MichiuEvent::UserEvent(*boxed_any));
                }
            }

            Some(LRESULT(0)) // 処理完了
        }
        _ => {
            push_win_event(WindowEvent::UnsafeRaw {
                msg,
                wparam,
                lparam,
            });
            None // DefWindowProcW を妨げないように None
        }
    }
}

// 装飾キーの現在の押し込み状態を非同期的にポーリング。
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

#[derive(Clone)]
pub struct EventSender {
    hwnd: HWND,
}

unsafe impl Send for EventSender {}
unsafe impl Sync for EventSender {}

impl EventSender {
    pub fn new(hwnd: HWND) -> Self {
        Self { hwnd }
    }

    /// Sends an arbitrary custom event from the background thread to the UI thread.
    pub fn send_event<T: Any + Send + 'static>(&self, event: T) {
        let boxed: Box<dyn Any + Send> = Box::new(event);
        // Box を生ポインタに変換し、所有権を一時的に破棄する
        let raw_ptr = Box::into_raw(Box::new(boxed));

        unsafe {
            // PostMessageW を呼んでUIスレッドのメッセージキューに投げる
            // LPARAM に生ポインタを乗せて引き渡す
            let _ = PostMessageW(
                Some(self.hwnd),
                WM_USER_EVENT,
                WPARAM(0),
                LPARAM(raw_ptr as isize),
            );
        }
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
    while unsafe { PeekMessageW(&mut msg, None, WM_USER, WM_USER + 200, PM_REMOVE) }.as_bool() {
        if msg.message == WM_WINDOW_COMMAND
            || msg.message == WM_RUN_ON_UI_THREAD
            || msg.message == WM_USER_EVENT
        {
            free_raw_message_pointer(msg.message, msg.lParam);
        }
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

// コールバック関数を保持するための型
type Listener = Rc<dyn Fn(&dyn Any)>;

/// A lightweight publish/subscribe system designed exclusively for the UI thread.
///
/// Used to broadcast events between components.
/// Since it is based on `Rc` and `RefCell`,
/// it has absolutely no locking overhead—unlike `Mutex` in multithreaded environments—
/// and runs extremely fast even in a 60 FPS GUI loop.
#[derive(Clone, Default)]
pub struct EventBus {
    listeners: Rc<RefCell<HashMap<TypeId, Vec<Listener>>>>,
}

impl EventBus {
    /// Create a new EventBus.
    pub fn new() -> Self {
        Self {
            listeners: Rc::new(RefCell::new(HashMap::new())),
        }
    }

    /// Registers (subscribes to) a callback that is called when an event of a specific type `T` is emitted.
    ///
    /// # Example
    /// ```rust
    /// bus.subscribe(|event: &MyCustomEvent| {
    ///     println!("Received: {:?}", event);
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

    /// Broadcasts an event of a specific type `T` and propagates it to all subscribed callbacks.
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

fn push_ime_state_update(hwnd: HWND, state: &WindowState) {
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
        let result_text = ctx
            .get_result_string()
            .unwrap_or_default()
            .unwrap_or_default();

        let caret_position = ctx.get_composition_window_position();

        let update = ImeStateUpdate {
            window_id: hwnd.0 as isize,
            is_open,
            conversion_mode,
            sentence_mode,
            keyboard_layout_id,
            composition_text,
            result_text,
            caret_position,
        };

        // オプトインされた中継サーバーが有効な場合は即座に外部へプッシュ配信
        if let Some(ref relay) = state.ime_relay {
            relay.send(update.clone());
        }

        // 本ライブラリのメインイベントキューへバンドルイベントとしてプッシュ
        push_event(MichiuEvent::WindowEvent {
            window_id: WindowId(hwnd.0 as isize),
            event: WindowEvent::Ime(Unvalidated::new(update)),
        });
    }
}

#[cfg(test)]
mod tests;
