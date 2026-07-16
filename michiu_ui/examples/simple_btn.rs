use michiu_ui::{
    ComposedRenderer, Element, ElementState, EntityId, Modifiers, MouseButton, prelude::*,
};
use std::time::Duration;
use windows::{
    Win32::{
        Foundation::*,
        Graphics::Gdi::*,
        System::{
            LibraryLoader::GetModuleHandleW,
            WinRT::{RO_INIT_SINGLETHREADED, RoInitialize},
        },
        UI::{
            HiDpi::{
                DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2, GetDpiForWindow,
                SetProcessDpiAwarenessContext,
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
                // DComp描画時はGDIによる背景自動消去を抑制し、描画競合を完全に回避します
                return LRESULT(1);
            }
            WM_DPICHANGED => {
                // wparam から新しい DPI の値を取得 (96 DPI = 1.0倍)
                let dpi = (wparam.0 & 0xffff) as u32;
                let scale = dpi as f32 / 96.0;

                let mut client_rect = RECT::default();
                let _ = unsafe { GetClientRect(hwnd, &mut client_rect) };
                let width = (client_rect.right - client_rect.left) as u32;
                let height = (client_rect.bottom - client_rect.top) as u32;

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
                let mut ps = PAINTSTRUCT::default();
                let _hdc = unsafe { BeginPaint(hwnd, &mut ps) };

                // トランジション（アニメーション）を1フレーム進める
                app.context.tick_transitions();

                app.context
                    .sync_layout_and_render_list(app.root_id, app.renderer.layout_size);
                app.renderer.update_composition_tree(&mut app.context);

                // 描画実行
                app.renderer.draw(&app.context);
                app.context.clear_layout_dirty();
                app.context.clear_render_dirty();

                let _ = unsafe { EndPaint(hwnd, &ps) };

                // アニメーションがまだ継続中の場合、次のフレームの再描画要求を自給自足してループさせます
                if app.context.has_active_animations() {
                    let _ = unsafe { InvalidateRect(Some(hwnd), None, false) };
                }
                return LRESULT(0);
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

                // 修飾キーの状態をビットマスクから解決
                let modifiers = Modifiers {
                    shift: (wparam.0 & 0x0004) != 0, // MK_SHIFT
                    ctrl: (wparam.0 & 0x0008) != 0,  // MK_CONTROL
                    alt: false,
                    logo: false,
                };

                app.context
                    .inject_pointer_button(MouseButton::Left, state, modifiers);

                app.context
                    .sync_layout_and_render_list(app.root_id, app.renderer.layout_size);
                app.renderer.update_composition_tree(&mut app.context);

                // クリックによる再描画を反映
                let _ = unsafe { InvalidateRect(Some(hwnd), None, false) };
                let _ = unsafe { UpdateWindow(hwnd) };
                return LRESULT(0);
            }
            _ => {}
        }
    }

    unsafe { DefWindowProcW(hwnd, msg, wparam, lparam) }
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    // 1. メインスレッド（UIスレッド）の COM を STA（Single Threaded Apartment）モードで初期化
    unsafe {
        let _ = SetProcessDpiAwarenessContext(DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2);
        let _ = RoInitialize(RO_INIT_SINGLETHREADED);
    }

    let h_instance = unsafe { GetModuleHandleW(None)? };
    let class_name = w!("MichiuSimpleTextDemoClass");

    // 2. ウィンドウクラスの登録
    unsafe {
        let wnd_class = WNDCLASSW {
            lpfnWndProc: Some(wnd_proc),
            hInstance: h_instance.into(),
            lpszClassName: class_name,
            hbrBackground: HBRUSH::default(), // 背景ブラシを完全にクリア（GDIによる描画競合を防止）
            hCursor: LoadCursorW(None, IDC_ARROW)?,
            ..Default::default()
        };
        RegisterClassW(&wnd_class);
    }

    // 3. UI コンテキストの構築と静的テキスト要素の定義
    let mut context = Context::new();
    let (count, set_count) = context.create_signal(0u32);

    let font_style = ts()
        .font_size(24.0)
        .text_color(Color::WHITE)
        .font_family("Segoe UI")
        .pointer_events_none();

    // 共通のボタン基本スタイル
    let button_style = ts()
        .size((200.0, 130.0))
        .transform(Transform::new().scale(1.0, 1.0))
        .bg_color(hsl(0.0, 0.0, 0.1))
        .border_solid(10.0)
        .border_color(hex(0x323232))
        .box_shadow(blur(8.0).color(Color::BLACK).offset(0.0).spread(2.0))
        .items_center()
        .justify_center()
        .rounded(12.0)
        .trans_bg_color(Duration::from_millis(150), AnimationCurve::EaseInOutQuad)
        .trans_transform(Duration::from_millis(150), AnimationCurve::EaseInOutQuad)
        .trans_box_shadow(Duration::from_millis(150), AnimationCurve::EaseInOutQuad)
        .hovered(
            ts().bg_color(rgb(100, 100, 100))
                .transform(Transform::new().scale(1.05, 1.05))
                .box_shadow(blur(12.0).color(Color::BLACK).offset(0.0).spread(2.0)),
        )
        .pressed(ts().transform(Transform::new().scale(1.02, 1.02)));

    // build_ui を使って要素ツリーを宣言的に組み立て
    let root = build_ui(&mut context, || {
        // ボタン 1 (Signal リアクティブ更新)
        let btn1 = v_flex(&button_style)
            .on_click(move || {
                set_count.set(count.get() + 1);
            })
            .child(text(move || format!("Signal: {}", count.get())).style(&font_style));

        // ボタン 2 (set_contents による自己破壊的・直接的なコンテンツ差し替え更新)
        let font_style_clone = font_style.clone();
        let btn2 = v_flex(&button_style);
        let mut static_count = 0u32;
        let btn2 = btn2
            .child(text(format!("Static: {}", static_count)).style(&font_style))
            .on_click(move || {
                static_count += 1;
                // btn2 を用いて子要素を直接新しいテキスト要素へ差し替え
                btn2.set_contents(
                    text(format!("Static: {}", static_count)).style(&font_style_clone),
                );
            });

        let mut static2_count = 0u32;
        let btn3 = v_flex(&button_style)
            .child(text(format!("Static 2: {}", static2_count)).style(&font_style))
            .on_click_with(move |cx| {
                static2_count += 1;

                // イベント発生時に、コンテキストから現在クリックされた自分自身の Element を特定
                let self_id = cx.entity_id_pressed().unwrap();
                let self_element = Element::from(self_id);

                // 特定した自分自身に対して子要素を差し替え
                self_element
                    .set_contents(text(format!("Static 2: {}", static2_count)).style(&font_style));
            });

        // 親コンテナ（100%全画面、横並び配置に設定）
        h_flex(
            ts().size_full()
                .bg_color(rgba(25, 25, 25, 0.4))
                .items_center()
                .justify_center()
                .gap_col(30.0)
                .backdrop_acrylic(),
        )
        .child(btn1)
        .child(btn2)
        .child(btn3)
    });

    // 4. アプリケーション状態をヒープ上に準備
    // ※ ウィンドウ生成非同期ハンドラ (ComposedRenderer) を作成するためにプレシーティング
    // (ComposedRenderer::new は内部で setup_direct_composition(hwnd) を行います)
    // ウィンドウを作成する（まだlpParamはNullの状態で枠のみを一旦作成、またはlpParamにapp_stateポインタをバインド）
    let initial_layout_size = LayoutSize::new(800.0, 600.0);

    // ダミーの HWND であらかじめレンダラーを作るのを防ぐため、
    // まず HWND を生成し、WM_CREATE のタイミングではなく作成直後に ComposedRenderer を生成して格納します。
    let hwnd = unsafe {
        CreateWindowExW(
            WS_EX_NOREDIRECTIONBITMAP, // GDIリダイレクションを無効化し DComp を露出させる
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
            None, // WM_CREATE 時には一旦 Null にしておく
        )?
    };

    // レンダラーを作成
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

    // 作成完了した AppState の生ポインタを HWND にアタッチ
    unsafe { SetWindowLongPtrW(hwnd, GWLP_USERDATA, Box::into_raw(app_state) as isize) };
    // 最初の WM_SIZE がアタッチ前に無視されてしまうため、
    // ここで正確なクライアント領域サイズを取得し、手動で初回の正確なリサイズとレイアウト同期を叩き込みます。
    let mut client_rect = RECT::default();
    let _ = unsafe { GetClientRect(hwnd, &mut client_rect) };
    let width = (client_rect.right - client_rect.left) as u32;
    let height = (client_rect.bottom - client_rect.top) as u32;

    let app_ptr = unsafe { GetWindowLongPtrW(hwnd, GWLP_USERDATA) } as *mut AppState;
    let app = unsafe { &mut *app_ptr };

    // 正確なクライアント領域ピクセルで wgpu ターゲットを設定し、初回のレイアウト計算を確定
    app.renderer.resize((width, height), scale_factor);
    app.context
        .sync_layout_and_render_list(app.root_id, app.renderer.layout_size);

    // 5. ウィンドウを表示して描画
    unsafe {
        let _ = ShowWindow(hwnd, SW_SHOW);
        let _ = UpdateWindow(hwnd);
    }

    // 6. Win32 メッセージループの開始
    let mut msg = MSG::default();
    unsafe {
        while GetMessageW(&mut msg, None, 0, 0).as_bool() {
            let _ = TranslateMessage(&msg);
            DispatchMessageW(&msg);
        }
    }

    Ok(())
}
