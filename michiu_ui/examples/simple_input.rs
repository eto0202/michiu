use std::time::Duration;

use michiu_ui::{ComposedRenderer, EntityId, ImeState, VirtualKey, prelude::*};
use windows::{
    Win32::{
        Foundation::*,
        Graphics::Gdi::*,
        System::{
            Com::{COINIT_APARTMENTTHREADED, CoInitializeEx},
            LibraryLoader::GetModuleHandleW,
        },
        UI::{
            HiDpi::{
                DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2, GetDpiForWindow,
                SetProcessDpiAwarenessContext,
            },
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
    core::w,
};

/// ウィンドウメッセージ処理時に Context と Renderer を一元管理するためのアプリケーション状態
struct AppState {
    renderer: ComposedRenderer,
    context: Context,
    root_id: EntityId,
}

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
                return LRESULT(1);
            }
            // マウス移動とクリックによるフォーカス判定
            WM_MOUSEMOVE => {
                let x = (lparam.0 & 0xffff) as i16 as f32 / app.renderer.scale_factor;
                let y = ((lparam.0 >> 16) & 0xffff) as i16 as f32 / app.renderer.scale_factor;
                app.context.inject_pointer_move(LayoutPoint::new(x, y));
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
            // 文字・キーメッセージを入力系へインジェクション ───
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

                    _ => VirtualKey::UNKNOWN,
                };

                app.context
                    .inject_keyboard_key(key, ElementState::Pressed, modifiers); // ★ modifiers を引き渡す

                app.context
                    .sync_layout_and_render_list(app.root_id, app.renderer.layout_size);
                app.renderer.update_composition_tree(&mut app.context);

                let _ = unsafe { InvalidateRect(Some(hwnd), None, false) };
                return LRESULT(0);
            }
            WM_DPICHANGED => {
                let dpi = (wparam.0 & 0xffff) as u32;
                let scale = dpi as f32 / 96.0;

                let mut client_rect = RECT::default();
                let _ = unsafe { GetClientRect(hwnd, &mut client_rect) };
                let width = (client_rect.right - client_rect.left) as u32;
                let height = (client_rect.bottom - client_rect.top) as u32;

                app.renderer.resize((width, height), scale);
                app.context.window.scale_factor = scale;

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

                app.renderer
                    .resize((width, height), app.renderer.scale_factor);
                app.context.window.scale_factor = app.renderer.scale_factor;

                app.context
                    .sync_layout_and_render_list(app.root_id, app.renderer.layout_size);
                app.renderer.update_composition_tree(&mut app.context);

                let _ = unsafe { InvalidateRect(Some(hwnd), None, false) };
                let _ = unsafe { UpdateWindow(hwnd) };
                return LRESULT(0);
            }
            WM_PAINT => {
                let mut ps = PAINTSTRUCT::default();
                let _hdc = unsafe { BeginPaint(hwnd, &mut ps) };

                // トランジション（アニメーション）およびキャレット点滅の更新を要求
                app.context.tick_transitions();

                if app.context.has_active_animations() {
                    app.context
                        .sync_layout_and_render_list(app.root_id, app.renderer.layout_size);
                    app.renderer.update_composition_tree(&mut app.context);
                }

                // 描画実行
                app.renderer.draw(&app.context);
                app.context.clear_render_dirty();

                let _ = unsafe { EndPaint(hwnd, &ps) };

                if app.context.has_active_animations() {
                    let _ = unsafe { InvalidateRect(Some(hwnd), None, false) };
                }

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
            WM_NCDESTROY => {
                let app_state_ptr =
                    unsafe { SetWindowLongPtrW(hwnd, GWLP_USERDATA, 0) } as *mut AppState;
                if !app_state_ptr.is_null() {
                    let _ = unsafe { Box::from_raw(app_state_ptr) };
                }
                unsafe { PostQuitMessage(0) };
                return LRESULT(0);
            }
            _ => {}
        }
    }

    unsafe { DefWindowProcW(hwnd, msg, wparam, lparam) }
}

#[cfg(feature = "dhat")]
#[global_allocator]
static ALLOC: dhat::Alloc = dhat::Alloc;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    #[cfg(feature = "dhat")]
    let _profiler = dhat::Profiler::new_heap();

    unsafe {
        let _ = SetProcessDpiAwarenessContext(DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2);
        let _ = CoInitializeEx(None, COINIT_APARTMENTTHREADED);
    }

    let h_instance = unsafe { GetModuleHandleW(None)? };
    let class_name = w!("MichiuSimpleTextDemoClass");

    unsafe {
        let wnd_class = WNDCLASSW {
            style: CS_DBLCLKS,
            lpfnWndProc: Some(wnd_proc),
            hInstance: h_instance.into(),
            lpszClassName: class_name,
            hbrBackground: HBRUSH::default(),
            hCursor: LoadCursorW(None, IDC_ARROW)?,
            ..Default::default()
        };
        RegisterClassW(&wnd_class);
    }

    let mut context = Context::new();

    let root = build_ui(&mut context, || {
        // 入力文字列シグナル
        let (input_text, set_input_text) = create_signal(String::new());

        // 1. ラッパー（親フレックスコンテナ）
        let input_wrapper = h_flex(
            ts().width(320.0)
                .bg_color(rgb(38, 38, 38))
                .border_solid(1.5)
                .border_color(rgb(65, 65, 65))
                .rounded(6.0)
                .p_x(12.0)
                .p_y(8.0)
                .items_center()
                .gap_col(8.0)
                .focus_within(ts().border_color(Color::BLUE))
                .trans_border_color(Duration::from_millis(300), AnimationCurve::EaseInOutQuad),
        );

        // 2. プレフィックス要素（通貨記号テキスト、余白調整も完全にスタイルの範囲で可能）
        let prefix_element = text("¥").style(
            ts().text_color(rgb(130, 130, 130))
                .font_size(15.0)
                .font_weight(400),
        );

        // 3. インプット本体要素
        let text_input = input(
            InputContents::new((input_text, set_input_text))
                .placeholder("金額を入力")
                .placeholder_color(Color::GRAY)
                .placeholder_select(false)
                .caret_color(rgba(30, 144, 255, 0.5))
                .max_length(20)
                .password(false)
                .is_ime(true)
                .multiline(false)
                .numeric_only(false),
        )
        .style(
            ts().grow() // 残り幅をすべて引き伸ばす
                .h_auto()
                .bg_color(Color::TRANSPARENT)
                .text_color(Color::CYAN)
                .font_size(15.0)
                .font_family("Segoe UI")
                .select_text()
                .select_text_color(Color::CYAN)
                .select_bg_color(rgba(30, 144, 255, 0.2))
                .cursor_text(),
        );

        let base_style = ts()
            .text_color(rgb(130, 130, 130))
            .font_size(13.0)
            .hovered(ts().text_color(Color::WHITE))
            .cursor_pointer();

        // 4. サフィックス要素（文字が入力されている場合のみ非表示コンテナから浮かび上がらせるリアクティブクリアボタン）
        let suffix_element = text("✕")
            .style(move || {
                let has_text = !input_text.get().is_empty();
                let base_style = base_style.clone();

                if has_text {
                    base_style.opacity(1.0)
                } else {
                    base_style.opacity(0.0).pointer_events_none()
                }
            })
            .on_click(move || {
                set_input_text.set(String::new());
            });

        // 5. ツリーの組み立て
        v_flex(
            ts().size_full()
                .bg_color(rgb(25, 25, 25))
                .justify_center()
                .items_center(),
        )
        .child(
            input_wrapper
                .child(prefix_element)
                .child(text_input)
                .child(suffix_element),
        )
    });

    let initial_layout_size = LayoutSize::new(800.0, 600.0);

    let hwnd = unsafe {
        CreateWindowExW(
            WS_EX_NOREDIRECTIONBITMAP,
            class_name,
            w!("Michiu - Simple Text Demo"),
            WS_OVERLAPPEDWINDOW,
            CW_USEDEFAULT,
            CW_USEDEFAULT,
            800,
            600,
            None,
            None,
            Some(HINSTANCE(h_instance.0)),
            None,
        )?
    };

    let dpi = unsafe { GetDpiForWindow(hwnd) };
    let scale_factor = dpi as f32 / 96.0;
    let renderer = pollster::block_on(ComposedRenderer::new(
        hwnd,
        initial_layout_size,
        scale_factor,
    ))?;

    let app_state = Box::new(AppState {
        renderer,
        context,
        root_id: root.id(),
    });

    unsafe { SetWindowLongPtrW(hwnd, GWLP_USERDATA, Box::into_raw(app_state) as isize) };
    let mut client_rect = RECT::default();
    let _ = unsafe { GetClientRect(hwnd, &mut client_rect) };
    let width = (client_rect.right - client_rect.left) as u32;
    let height = (client_rect.bottom - client_rect.top) as u32;

    let app_ptr = unsafe { GetWindowLongPtrW(hwnd, GWLP_USERDATA) } as *mut AppState;
    let app = unsafe { &mut *app_ptr };

    app.renderer.resize((width, height), scale_factor);
    app.context
        .sync_layout_and_render_list(app.root_id, app.renderer.layout_size);

    unsafe {
        let _ = ShowWindow(hwnd, SW_SHOW);
        let _ = UpdateWindow(hwnd);
    }

    let mut msg = MSG::default();
    unsafe {
        while GetMessageW(&mut msg, None, 0, 0).as_bool() {
            let _ = TranslateMessage(&msg);
            DispatchMessageW(&msg);
        }
    }

    Ok(())
}
