#![allow(clippy::pedantic, clippy::restriction, unused_must_use)]

use std::cell::Cell;

use michiu_ui::{
    ComposedRenderer, ElementState, ImeState, Modifiers, MouseButton, VirtualKey, prelude::*,
    raw_wheel_delta_to_logical_pixels,
};
use windows::{
    Win32::{
        Foundation::*,
        Graphics::Gdi::*,
        System::LibraryLoader::GetModuleHandleW,
        UI::{
            Input::{
                Ime::{
                    GCS_COMPATTR, GCS_COMPSTR, GCS_CURSORPOS, GCS_RESULTSTR,
                    ImmGetCompositionStringW, ImmGetContext, ImmReleaseContext,
                },
                KeyboardAndMouse::{GetKeyState, VK_CONTROL, VK_SHIFT},
            },
            WindowsAndMessaging::*,
        },
    },
    core::{PCWSTR, w},
};

use crate::AppState;

// Win32 ウィンドウプロシージャ
unsafe extern "system" fn wnd_proc(
    hwnd: HWND,
    msg: u32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    // 1. ウィンドウ作成時に AppState の生ポインタを GWLP_USERDATA に退避
    if msg == WM_CREATE {
        let create_struct = lparam.0 as *const CREATESTRUCTW;
        let app_state_ptr = (unsafe { *create_struct }).lpCreateParams as *mut AppState;
        unsafe { SetWindowLongPtrW(hwnd, GWLP_USERDATA, app_state_ptr as isize) };

        return LRESULT(0);
    }

    // GWLP_USERDATA から AppState の参照を取得
    let app_state_ptr = unsafe { GetWindowLongPtrW(hwnd, GWLP_USERDATA) } as *mut AppState;
    if !app_state_ptr.is_null() {
        let app = unsafe { &mut *app_state_ptr };

        match msg {
            WM_ERASEBKGND => {
                // DComp描画時はGDIによる背景自動消去を抑制し描画競合をに回避
                return LRESULT(1);
            }
            WM_NCDESTROY => {
                // ウィンドウ破棄時に生ポインタを安全に Box に戻してメモリ解放
                let app_state_ptr =
                    unsafe { SetWindowLongPtrW(hwnd, GWLP_USERDATA, 0) } as *mut AppState;
                if !app_state_ptr.is_null() {
                    let _ = unsafe { Box::from_raw(app_state_ptr) };
                }
                unsafe { PostQuitMessage(0) };
                return LRESULT(0);
            }
            WM_NULL => {
                let app_ptr = unsafe { GetWindowLongPtrW(hwnd, GWLP_USERDATA) } as *mut AppState;
                if !app_ptr.is_null() {
                    let app = unsafe { &mut *app_ptr };

                    // バックグラウンドから届いた CSS 更新タスクなどを安全に消化
                    app.context.process_main_thread_tasks();

                    // 消化によってレイアウトや描画に変更があった場合のみ、同期および再描画を実行
                    if app.context.is_render_dirty() {
                        app.context
                            .sync_layout_and_render_list(app.root_id, app.renderer.layout_size);
                        app.renderer.update_composition_tree(&mut app.context);
                        app.renderer.draw(&app.context); // これが内部で InvalidateRect 等を適切に走らせます
                    }
                }
                return LRESULT(0);
            }
            WM_DPICHANGED => {
                // wparam から新しい DPI の値を取得 (96 DPI = 1.0倍)
                let dpi = (wparam.0 & 0xffff) as u32;
                let scale = dpi as f32 / 96.0;
                let (width, height) = client_rect(hwnd);

                // 新しいスケール因数で wgpu と Taffy レイアウトをリサイズ同期
                app.renderer.resize((width, height), scale);

                // レイアウト再計算
                app.context
                    .sync_layout_and_render_list(app.root_id, app.renderer.layout_size);
                app.renderer.update_composition_tree(&mut app.context);

                let _ = unsafe { InvalidateRect(Some(hwnd), None, false) };
                let _ = unsafe { UpdateWindow(hwnd) };
                return LRESULT(0);
            }
            WM_SIZE => {
                let width = (lparam.0 & 0xffff) as u32;
                let height = ((lparam.0 >> 16) & 0xffff) as u32;

                // レンダラーのリサイズとレイアウト物理サイズの更新
                app.renderer
                    .resize((width, height), app.renderer.scale_factor);

                // Taffy レイアウトツリーの同期と確定座標再計算
                app.context
                    .sync_layout_and_render_list(app.root_id, app.renderer.layout_size);
                app.renderer.update_composition_tree(&mut app.context);

                // 再描画要求
                let _ = unsafe { InvalidateRect(Some(hwnd), None, false) };
                let _ = unsafe { UpdateWindow(hwnd) };
                return LRESULT(0);
            }
            WM_PAINT => {
                let total_start = std::time::Instant::now();
                let layout_start = std::time::Instant::now();

                let mut ps = PAINTSTRUCT::default();
                let _hdc = unsafe { BeginPaint(hwnd, &mut ps) };

                app.context.tick_transitions();
                app.context.tick_animations();
                app.context.tick_drag_autoscroll();

                app.context
                    .sync_layout_and_render_list(app.root_id, app.renderer.layout_size);

                let layout_elapsed = layout_start.elapsed();

                app.renderer.update_composition_tree(&mut app.context);

                app.renderer.draw(&app.context);
                app.context.clear_render_dirty();

                let _ = unsafe { EndPaint(hwnd, &ps) };

                let total_elapsed = total_start.elapsed();

                thread_local! {
                    static LAST_PRINT: Cell<Option<std::time::Instant>> = const { Cell::new(None) };
                    static MAX_LAYOUT: Cell<f64> = const { Cell::new(0.0) };
                    static MAX_TOTAL: Cell<f64> = const { Cell::new(0.0) };
                    static FRAME_COUNT: Cell<u32> = const { Cell::new(0) };
                }

                let layout_ms = layout_elapsed.as_secs_f64() * 1000.0;
                let total_ms = total_elapsed.as_secs_f64() * 1000.0;

                FRAME_COUNT.with(|c| c.set(c.get() + 1));
                MAX_LAYOUT.with(|c| c.set(c.get().max(layout_ms)));
                MAX_TOTAL.with(|c| c.set(c.get().max(total_ms)));

                let now = std::time::Instant::now();
                let should_print = LAST_PRINT.with(|c| match c.get() {
                    None => {
                        c.set(Some(now));
                        true
                    }
                    Some(last) => {
                        if now.duration_since(last).as_secs_f32() >= 1.0 {
                            c.set(Some(now));
                            true
                        } else {
                            false
                        }
                    }
                });

                if should_print {
                    println!(
                        "[FPS: {:>3}]  Max Layout: {:6.2}ms  |  Max Total CPU: {:6.2}ms",
                        FRAME_COUNT.with(|c| c.replace(0)),
                        MAX_LAYOUT.with(|c| c.replace(0.0)),
                        MAX_TOTAL.with(|c| c.replace(0.0))
                    );
                }

                // アニメーションがまだ継続中の場合、次のフレームの再描画要求を自給自足してループさせます
                if app.context.has_active_animations() {
                    let _ = unsafe { windows::Win32::Graphics::Dwm::DwmFlush() };
                    let _ = unsafe { InvalidateRect(Some(hwnd), None, false) };
                }
                return LRESULT(0);
            }

            WM_MOUSEMOVE => {
                let x = (lparam.0 & 0xffff) as i16 as f32;
                let y = ((lparam.0 >> 16) & 0xffff) as i16 as f32;

                // DPIスケールを考慮して論理座標に直して注入
                let logical_pos =
                    LayoutPoint::new(x / app.renderer.scale_factor, y / app.renderer.scale_factor);

                app.context.inject_pointer_move(logical_pos);

                // インタラクションによる変化（ホバー状態）をリアルタイムに再描画
                let _ = unsafe { InvalidateRect(Some(hwnd), None, false) };
                let _ = unsafe { UpdateWindow(hwnd) };
                return LRESULT(0);
            }

            WM_LBUTTONDOWN | WM_LBUTTONUP => {
                let state = if msg == WM_LBUTTONDOWN {
                    ElementState::Pressed
                } else {
                    ElementState::Released
                };

                // VK_CONTROL(0x11) および VK_SHIFT(0x10) の物理状態を直接クエリ
                let ctrl_pressed = unsafe { GetKeyState(0x11) } < 0;
                let shift_pressed = unsafe { GetKeyState(0x10) } < 0;

                // 物理キー状態から Modifiers を正確に一元同期
                let modifiers = Modifiers {
                    shift: shift_pressed,
                    ctrl: ctrl_pressed,
                    alt: false,
                    logo: false,
                };

                app.context
                    .inject_pointer_button(MouseButton::Left, state, modifiers);

                app.context
                    .sync_layout_and_render_list(app.root_id, app.renderer.layout_size);
                app.renderer.update_composition_tree(&mut app.context);
                // フォーカス取得（点滅カーソル表示開始）のために再描画
                let _ = unsafe { InvalidateRect(Some(hwnd), None, false) };
                return LRESULT(0);
            }
            WM_RBUTTONDOWN | WM_RBUTTONUP => {
                let state = if msg == WM_RBUTTONDOWN {
                    ElementState::Pressed
                } else {
                    ElementState::Released
                };

                app.context
                    .inject_pointer_button(MouseButton::Right, state, Modifiers::default());

                app.context
                    .sync_layout_and_render_list(app.root_id, app.renderer.layout_size);
                app.renderer.update_composition_tree(&mut app.context);

                let _ = unsafe { InvalidateRect(Some(hwnd), None, false) };
                return LRESULT(0);
            }
            WM_LBUTTONDBLCLK => {
                let ctrl_pressed = unsafe { GetKeyState(0x11) } < 0;
                let shift_pressed = unsafe { GetKeyState(0x10) } < 0;

                let modifiers = Modifiers {
                    shift: ctrl_pressed,
                    ctrl: shift_pressed,
                    alt: false,
                    logo: false,
                };

                app.context.inject_pointer_double_click(modifiers);

                let _ = unsafe { InvalidateRect(Some(hwnd), None, false) };
                return LRESULT(0);
            }
            WM_MOUSEWHEEL => {
                // wparam の上位16ビットから符号付き生ホイールデルタを取得
                let raw_delta = (wparam.0 >> 16) as i16 as f32;

                // システムのマウス設定行数に準拠した論理ピクセル移動量へ変換
                let scroll_y = raw_wheel_delta_to_logical_pixels(raw_delta);

                // 2次元スクロールとして注入 (縦スクロールのため X は 0.0 固定)
                app.context.inject_mouse_wheel(0.0, scroll_y);

                // スクロールによって変化した絶対座標と表示制限を瞬時に再計算
                app.context
                    .sync_layout_and_render_list(app.root_id, app.renderer.layout_size);
                app.renderer.update_composition_tree(&mut app.context);

                // 画面を再描画
                let _ = unsafe { InvalidateRect(Some(hwnd), None, false) };
                return LRESULT(0);
            }

            // マウスホイール処理（横
            WM_MOUSEHWHEEL => {
                let raw_delta = (wparam.0 >> 16) as i16 as f32;

                // 横スクロール時は、右チルト（プラス値）された際に
                // 右方向へスクロール（オフセット加算）させるため、符号の方向性を補正
                let scroll_x = -raw_wheel_delta_to_logical_pixels(raw_delta);

                // 2次元スクロールとして注入 (横スクロールのため Y は 0.0 固定)
                app.context.inject_mouse_wheel(scroll_x, 0.0);

                app.context
                    .sync_layout_and_render_list(app.root_id, app.renderer.layout_size);
                app.renderer.update_composition_tree(&mut app.context);

                let _ = unsafe { InvalidateRect(Some(hwnd), None, false) };
                return LRESULT(0);
            }
            WM_CHAR => {
                if let Some(ch) = std::char::from_u32(wparam.0 as u32) {
                    app.context.inject_character(ch);

                    app.context
                        .sync_layout_and_render_list(app.root_id, app.renderer.layout_size);
                    app.renderer.update_composition_tree(&mut app.context);

                    let _ = unsafe { InvalidateRect(Some(hwnd), None, false) };
                }
                return LRESULT(0);
            }
            WM_KEYDOWN => {
                let ctrl_pressed = unsafe { GetKeyState(VK_CONTROL.0 as i32) } < 0;
                let shift_pressed = unsafe { GetKeyState(VK_SHIFT.0 as i32) } < 0;

                let modifiers = Modifiers {
                    ctrl: ctrl_pressed,
                    shift: shift_pressed,
                    alt: false, // WM_KEYDOWN 時、Altは通常 WM_SYSKEYDOWN で捕捉されるため false
                    logo: false,
                };

                if ctrl_pressed {
                    match wparam.0 as i32 {
                        // Ctrl + C
                        0x43 => {
                            // 'C' キー
                            if let Some(selected_text) = app.context.get_selected_text() {
                                set_win32_clipboard(&selected_text);
                            }
                            return LRESULT(0);
                        }
                        // Ctrl + V
                        0x56 => {
                            // 'V' キー
                            if let Some(pasted_text) = get_win32_clipboard() {
                                app.context.inject_paste(&pasted_text);
                            }
                            return LRESULT(0);
                        }
                        // Ctrl + X (切り取り)
                        0x58 => {
                            // 'X'
                            if let Some(cut_text) = app.context.inject_cut() {
                                set_win32_clipboard(&cut_text);
                            }
                            return LRESULT(0);
                        }
                        // Ctrl + Z (Undo)
                        0x5A => {
                            // 'Z'
                            app.context.inject_undo();
                            return LRESULT(0);
                        }
                        // Ctrl + Y (Redo)
                        0x59 => {
                            // 'Y'
                            app.context.inject_redo();
                            return LRESULT(0);
                        }
                        _ => {}
                    }
                }

                let vk = wparam.0 as u16;
                // バックスペース、デリート、矢印キーおよびショートカット文字を正確にマッピング
                let key = match vk {
                    0x08 => VirtualKey::BACK,
                    0x2E => VirtualKey::DELETE,
                    0x25 => VirtualKey::LEFT,
                    0x27 => VirtualKey::RIGHT,
                    0x26 => VirtualKey::UP,
                    0x28 => VirtualKey::DOWN,
                    0x41 => VirtualKey::A,
                    0x43 => VirtualKey::C,
                    0x09 => VirtualKey::TAB,
                    0x0D => VirtualKey::RETURN,
                    0x20 => VirtualKey::SPACE,
                    _ => VirtualKey::UNKNOWN,
                };

                app.context
                    .inject_keyboard_key(key, ElementState::Pressed, modifiers);

                app.context
                    .sync_layout_and_render_list(app.root_id, app.renderer.layout_size);
                app.renderer.update_composition_tree(&mut app.context);

                let _ = unsafe { InvalidateRect(Some(hwnd), None, false) };
                return LRESULT(0);
            }
            WM_IME_SETCONTEXT => {
                // UI 側で未確定文字列を描画するため、OS の標準 IME コンポジションウィンドウの
                // 重複描画フラグ（ISC_SHOWUICOMPOSITIONWINDOW = 0x80000000）を取り除く
                let mut lp = lparam;
                lp.0 &= !(0x80000000);
                return unsafe { DefWindowProcW(hwnd, msg, wparam, lp) };
            }
            WM_IME_STARTCOMPOSITION => {
                // 標準 IME ウィンドウの不要な初期位置描画を防ぐため LRESULT(1) を返す
                return LRESULT(1);
            }
            WM_IME_ENDCOMPOSITION => {
                // IME の非確定終了（確定 or キャンセル時）に伴い状態をクリア
                let ime_state = ImeState {
                    is_open: false,
                    composition_text: String::new(),
                    result_text: String::new(),
                    ..Default::default()
                };
                app.context.inject_ime(ime_state);
                let _ = unsafe { InvalidateRect(Some(hwnd), None, false) };
                return LRESULT(1);
            }
            WM_IME_COMPOSITION => {
                let himc = unsafe { ImmGetContext(hwnd) };
                if !himc.is_invalid() {
                    let mut ime_state = ImeState {
                        is_open: true,
                        ..Default::default()
                    };

                    // IME から確定されたテキストを取得
                    if (lparam.0 & GCS_RESULTSTR.0 as isize) != 0 {
                        let len = unsafe { ImmGetCompositionStringW(himc, GCS_RESULTSTR, None, 0) };
                        if len > 0 {
                            let mut buf = vec![0u16; (len as usize) / 2];
                            unsafe {
                                ImmGetCompositionStringW(
                                    himc,
                                    GCS_RESULTSTR,
                                    Some(buf.as_mut_ptr() as *mut _),
                                    len as u32,
                                );
                            }
                            ime_state.result_text = String::from_utf16_lossy(&buf);
                        }
                    }

                    // IME から未確定のテキストを取得
                    if (lparam.0 & GCS_COMPSTR.0 as isize) != 0 {
                        let len = unsafe { ImmGetCompositionStringW(himc, GCS_COMPSTR, None, 0) };
                        if len > 0 {
                            let mut buf = vec![0u16; (len as usize) / 2];
                            unsafe {
                                ImmGetCompositionStringW(
                                    himc,
                                    GCS_COMPSTR,
                                    Some(buf.as_mut_ptr() as *mut _),
                                    len as u32,
                                );
                            }
                            ime_state.composition_text = String::from_utf16_lossy(&buf);
                            // エラー時のためのフォールバックとして、組成文字列の末尾を一旦セット
                            ime_state.composition_cursor =
                                ime_state.composition_text.encode_utf16().count();
                        }
                    }

                    // 未確定テキスト内における詳細なIMEカーソル位置を取得
                    if (lparam.0 & GCS_CURSORPOS.0 as isize) != 0 {
                        let cursor_pos =
                            unsafe { ImmGetCompositionStringW(himc, GCS_CURSORPOS, None, 0) };
                        if cursor_pos >= 0 {
                            ime_state.composition_cursor = cursor_pos as usize;
                        }
                    }

                    // 未確定テキストの文節属性情報を取得
                    if (lparam.0 & GCS_COMPATTR.0 as isize) != 0 {
                        let len = unsafe { ImmGetCompositionStringW(himc, GCS_COMPATTR, None, 0) };
                        if len > 0 {
                            let mut attrs = vec![0u8; len as usize];
                            unsafe {
                                ImmGetCompositionStringW(
                                    himc,
                                    GCS_COMPATTR,
                                    Some(attrs.as_mut_ptr() as *mut _),
                                    len as u32,
                                );
                            }
                            ime_state.composition_attrs = attrs;
                        }
                    }

                    // IME 情報を入力処理へ注入
                    app.context.inject_ime(ime_state);

                    let _ = unsafe { ImmReleaseContext(hwnd, himc) };
                }

                app.context
                    .sync_layout_and_render_list(app.root_id, app.renderer.layout_size);
                app.renderer.update_composition_tree(&mut app.context);

                let _ = unsafe { InvalidateRect(Some(hwnd), None, false) };
                return LRESULT(1); // OS標準の描画処理を完全に抑制
            }
            WM_SETCURSOR => {
                // lparam の下位16ビットが HTCLIENT の時のみ適用
                let hit_test = (lparam.0 & 0xffff) as u32;

                if hit_test == HTCLIENT {
                    let app_ptr =
                        unsafe { GetWindowLongPtrW(hwnd, GWLP_USERDATA) } as *mut AppState;
                    if !app_ptr.is_null() {
                        let app = unsafe { &mut *app_ptr };

                        // 現在ホバーされている要素があるかチェック
                        // なければプレス中ID等の解決は内部の resolve_cursor に
                        if let Some(hovered_id) = app.context.entity_id_hovered() {
                            // プレスロック状態、通常ホバー状態、親の Global 指定、リサイズ個別設定から最適な CursorIcon を解決
                            let cursor_icon = app.context.resolve_cursor(hovered_id);

                            // 物理 HCURSOR ハンドルを取得（独自カーソルがあればそれ、なければ IDC_ARROW 等を標準ロード）
                            let hcursor = cursor_icon.to_hcursor();

                            // OS の物理カーソルとしてセット
                            unsafe { SetCursor(Some(hcursor)) };

                            // OSによる矢印への自動復元を防止
                            return LRESULT(1);
                        }
                    }
                }
                // ホバー要素がない場合は、システム標準の処理にフォールバック
                return unsafe { DefWindowProcW(hwnd, msg, wparam, lparam) };
            }
            _ => {}
        }
    }

    unsafe { DefWindowProcW(hwnd, msg, wparam, lparam) }
}

pub fn register_class() -> windows_result::Result<(HMODULE, PCWSTR, WNDCLASSW)> {
    unsafe {
        let h_instance = GetModuleHandleW(None)?;
        let class_name = w!("MichiuSampleCollectionClass");

        // ウィンドウクラスの登録
        let wnd_class = WNDCLASSW {
            lpfnWndProc: Some(wnd_proc),
            hInstance: h_instance.into(),
            lpszClassName: class_name,
            hbrBackground: HBRUSH::default(), // 背景ブラシを完全にクリア（GDIによる描画競合を防止）
            hCursor: HCURSOR::default(),
            ..Default::default()
        };
        RegisterClassW(&wnd_class);

        Ok((h_instance, class_name, wnd_class))
    }
}

pub fn create_window(h_instance: HMODULE, class_name: PCWSTR) -> windows_result::Result<HWND> {
    let hwnd = unsafe {
        CreateWindowExW(
            WS_EX_NOREDIRECTIONBITMAP, // GDIリダイレクションを無効化し DComp を露出させる
            class_name,
            w!("Michiu Sample Collection"),
            WS_OVERLAPPEDWINDOW,
            CW_USEDEFAULT,
            CW_USEDEFAULT,
            1000,
            800,
            None,
            None,
            Some(HINSTANCE(h_instance.0)),
            None, // WM_CREATE 時には一旦 Null にしておく
        )?
    };

    Ok(hwnd)
}

pub fn create_renderer(
    hwnd: HWND,
    scale_factor: f32,
) -> Result<ComposedRenderer, Box<dyn std::error::Error>> {
    let initial_layout_size = LayoutSize::new(900.0, 700.0);
    // レンダラーを作成
    let renderer = pollster::block_on(ComposedRenderer::new(
        hwnd,
        initial_layout_size,
        scale_factor,
    ))?;

    Ok(renderer)
}

pub fn client_rect(hwnd: HWND) -> (u32, u32) {
    let mut client_rect = RECT::default();
    let _ = unsafe { GetClientRect(hwnd, &mut client_rect) };
    let width = (client_rect.right - client_rect.left) as u32;
    let height = (client_rect.bottom - client_rect.top) as u32;

    (width, height)
}

pub fn show_window(hwnd: HWND) -> bool {
    unsafe {
        let show = ShowWindow(hwnd, SW_SHOW);
        let update = UpdateWindow(hwnd);

        show.as_bool() || update.as_bool()
    }
}

pub fn message_loop() {
    let mut msg = MSG::default();
    unsafe {
        while GetMessageW(&mut msg, None, 0, 0).as_bool() {
            let _ = TranslateMessage(&msg);
            DispatchMessageW(&msg);
        }
    }
}
