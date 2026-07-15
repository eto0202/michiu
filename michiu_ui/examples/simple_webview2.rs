use std::time::Duration;

use michiu_ui::{ComposedRenderer, ElementState, EntityId, Modifiers, MouseButton, prelude::*};

use windows::{
    Win32::{
        Foundation::*,
        Graphics::Gdi::*,
        System::{
            LibraryLoader::GetModuleHandleW,
            WinRT::{RO_INIT_SINGLETHREADED, RoInitialize},
        },
        UI::{
            Controls::WM_MOUSELEAVE,
            HiDpi::{
                DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2, GetDpiForWindow,
                SetProcessDpiAwarenessContext,
            },
            Input::KeyboardAndMouse::{
                ReleaseCapture, SetCapture, TME_LEAVE, TRACKMOUSEEVENT, TrackMouseEvent,
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
    webview_id: EntityId,
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
                // DCompツリー側（WebView2等）のBoundsサイズもリサイズに連動して再構築
                app.renderer.update_composition_tree(&mut app.context);

                let _ = unsafe { InvalidateRect(Some(hwnd), None, false) };
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
                // DCompツリーのサイズ追従
                app.renderer.update_composition_tree(&mut app.context);

                // 再描画要求
                let _ = unsafe { InvalidateRect(Some(hwnd), None, false) };
                return LRESULT(0);
            }
            WM_PAINT => {
                let mut ps = PAINTSTRUCT::default();
                let _hdc = unsafe { BeginPaint(hwnd, &mut ps) };

                // 非同期キャプチャの完了など、バックグラウンドからメインスレッドに
                // 届いたタスク（クロージャ）をこの描画フレーム開始時にすべて安全に実行
                app.context.process_main_thread_tasks();

                // トランジション、およびキーフレームアニメーションを 1 Tick 進める
                app.context.tick_transitions();
                app.context.tick_animations();

                // 描画（draw）を行う直前に、必ず Taffy のレイアウトツリーの同期・再計算を実行
                // これにより、クリックによって変化したテキスト要素の「最新の幅」が、
                // リアルタイムに wgpu 側の描画枠（rect）に追従し、文字の縮みを完全に防ぎます。
                app.context
                    .sync_layout_and_render_list(app.root_id, app.renderer.layout_size);
                // 描画直前にDCompツリーおよびWebView2の配置も最新状態に追従させます
                app.renderer.update_composition_tree(&mut app.context);

                // 描画実行
                app.renderer.draw(&app.context);
                app.context.clear_layout_dirty();
                app.context.clear_render_dirty();

                let _ = unsafe { EndPaint(hwnd, &ps) };

                // アニメーション駆動中の場合は Invalidate を自給自足する
                if app.context.has_active_animations() || app.context.is_render_dirty() {
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

                // TrackMouseEvent を使って、マウスが外に出たときに WM_MOUSELEAVE を発行させる
                let mut tme = TRACKMOUSEEVENT {
                    cbSize: std::mem::size_of::<TRACKMOUSEEVENT>() as u32,
                    dwFlags: TME_LEAVE,
                    hwndTrack: hwnd,
                    dwHoverTime: 0,
                };
                let _ = unsafe { TrackMouseEvent(&mut tme) };

                // DPIスケールを考慮して論理座標に直して注入
                let logical_pos =
                    LayoutPoint::new(x / app.renderer.scale_factor, y / app.renderer.scale_factor);

                app.context.inject_pointer_move(logical_pos);

                // マウス移動メッセージを WebView2 コントローラーへ透過的にフォワード
                // 最前面にヒットした要素が WebView2 自身である場合のみ、イベントをフォワードする
                // 修正: ドラッグ（プレス）中であれば pressed 要素を優先ロック、なければヒット要素を取得
                let hit_element = app.context.hit_test(logical_pos);
                let target_element = app.context.interaction_states.pressed.or(hit_element);
                if target_element == Some(app.webview_id) {
                    app.renderer.forward_mouse_input(
                        &app.context,
                        app.webview_id,
                        msg,
                        wparam,
                        lparam,
                        LayoutPoint::new(x, y),
                    );
                }

                // インタラクションによる変化（ホバー状態）をリアルタイムに再描画
                if app.context.is_render_dirty() {
                    let _ = unsafe { InvalidateRect(Some(hwnd), None, false) };
                }
                return LRESULT(0);
            }
            WM_MOUSELEAVE => {
                // ウィンドウ外に去ったため、論理空間外へポインタを移動させてホバーを確実に解除
                app.context
                    .inject_pointer_move(LayoutPoint::new(-9999.0, -9999.0));

                let _ = unsafe { InvalidateRect(Some(hwnd), None, false) };
                return LRESULT(0);
            }
            WM_LBUTTONDOWN | WM_LBUTTONUP | WM_RBUTTONDOWN | WM_RBUTTONUP => {
                let state = if msg == WM_LBUTTONDOWN {
                    ElementState::Pressed
                } else {
                    ElementState::Released
                };

                // マウスキャプチャの Win32 制御
                if state == ElementState::Pressed {
                    unsafe { SetCapture(hwnd) };
                } else {
                    let _ = unsafe { ReleaseCapture() };
                }

                // 修飾キーの状態をビットマスクから解決
                let modifiers = Modifiers {
                    shift: (wparam.0 & 0x0004) != 0, // MK_SHIFT
                    ctrl: (wparam.0 & 0x0008) != 0,  // MK_CONTROL
                    alt: false,
                    logo: false,
                };

                let x = (lparam.0 & 0xffff) as i16 as f32;
                let y = ((lparam.0 >> 16) & 0xffff) as i16 as f32;

                // マウスクリックを WebView2 コントローラーへフォワード
                // これにより、ブラウザ内のリンククリックや各種操作が完璧に動作します。
                // クリック座標の最前面が WebView2 自身である場合のみフォワード
                let logical_pos =
                    LayoutPoint::new(x / app.renderer.scale_factor, y / app.renderer.scale_factor);

                // プレス状態が context.inject_pointer_button 内でクリアされる前にターゲットを特定
                let hit_element = app.context.hit_test(logical_pos);

                // ボタンを離した際は、ドラッグを開始した要素（Pressed）へメッセージを流す
                let target_element = if state == ElementState::Pressed {
                    hit_element
                } else {
                    app.context.interaction_states.pressed.or(hit_element)
                };

                app.context
                    .inject_pointer_button(MouseButton::Left, state, modifiers);

                if target_element == Some(app.webview_id) {
                    app.renderer.forward_mouse_input(
                        &app.context,
                        app.webview_id,
                        msg,
                        wparam,
                        lparam,
                        LayoutPoint::new(x, y),
                    );
                }

                // クリックした要素が実際に WebView2 である場合のみ、キーボードフォーカスをブラウザにアタッチ
                if msg == WM_LBUTTONDOWN
                    && app.context.interaction_states.focused == Some(app.webview_id)
                {
                    app.renderer.focus_webview(app.webview_id);
                }

                // クリックによる再描画を反映
                let _ = unsafe { InvalidateRect(Some(hwnd), None, false) };
                return LRESULT(0);
            }

            WM_MOUSEWHEEL => {
                // WM_MOUSEWHEEL の座標はスクリーン座標であるため、ScreenToClient で
                // クライアント領域の物理ピクセル座標に正確に直す必要がある
                let mut pt = POINT {
                    x: (lparam.0 & 0xffff) as i16 as i32,
                    y: ((lparam.0 >> 16) & 0xffff) as i16 as i32,
                };
                let _ = unsafe { ScreenToClient(hwnd, &mut pt) };

                let physical_pos = LayoutPoint::new(pt.x as f32, pt.y as f32);

                // WebView2 へ縦スクロールイベントを転送
                // スクロール位置の最前面が WebView2 自身である場合のみフォワード
                let logical_pos = LayoutPoint::new(
                    physical_pos.x / app.renderer.scale_factor,
                    physical_pos.y / app.renderer.scale_factor,
                );
                let hit_element = app.context.hit_test(logical_pos);

                if hit_element == Some(app.webview_id) {
                    app.renderer.forward_mouse_input(
                        &app.context,
                        app.webview_id,
                        msg,
                        wparam,
                        lparam,
                        physical_pos,
                    );
                }

                let _ = unsafe { InvalidateRect(Some(hwnd), None, false) };
                return LRESULT(0);
            }
            WM_ACTIVATE => {
                let activate_state = (wparam.0 & 0xffff) as u32;
                if activate_state == WA_INACTIVE {
                    // 他ウィンドウにフォーカスが移った瞬間、アプリ内部のフォーカスを強制的に解除
                    if let Some(focused_id) = app.context.interaction_states.focused {
                        app.context.set_focused(focused_id, false);
                        app.context.interaction_states.focused = None;
                    }

                    // 非アクティブ移行時のキャプチャプロセスを即時トリガー
                    let _ = unsafe { InvalidateRect(Some(hwnd), None, false) };
                }
                return LRESULT(0);
            }
            WM_ENTERSIZEMOVE => {
                app.context.is_window_resizing = true;
                let _ = unsafe { InvalidateRect(Some(hwnd), None, false) };
                return LRESULT(0);
            }
            // ウィンドウドラッグリサイズの完了をキャッチ
            WM_EXITSIZEMOVE => {
                app.context.is_window_resizing = false;
                // リサイズ完了後の再描画を即座にキックして、新サイズでの静止画キャプチャを誘発
                let _ = unsafe { InvalidateRect(Some(hwnd), None, false) };
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
    let webview_id_cell = std::cell::Cell::new(None);

    let hovered_style = ts().bg_color(rgb(110, 110, 110));

    let btn_style = ts()
        .absolute()
        .m_auto()
        .m_l(20.0)
        .size(50.0)
        .bg_color(rgb(100, 100, 100))
        .r(5.0)
        .box_shadow(blur(5.0).spread(1.0).color(Color::BLACK))
        .transform(Transform::new().scale(1.0, 1.0))
        .trans_bg_color(Duration::from_millis(150), AnimationCurve::EaseInOutQuad)
        .trans_size(Duration::from_millis(150), AnimationCurve::EaseInOutQuad)
        .trans_transform(Duration::from_millis(150), AnimationCurve::EaseInOutQuad)
        .trans_box_shadow(Duration::from_millis(150), AnimationCurve::EaseInOutQuad)
        // 擬似クラス状態のスタイルマッピング
        .hovered(
            hovered_style
                .size(150.0)
                .box_shadow(blur(10.0).spread(2.0).color(Color::BLACK)),
        )
        .pressed(ts().transform(Transform::new().scale(0.95, 0.95)));

    // build_ui を使って要素ツリーを宣言的に組み立て
    let root = build_ui(&mut context, || {
        let root_node = v_flex(
            ts().size_full()
                .bg_color(rgba(34, 36, 42, 0.3))
                .items_center()
                .justify_center()
                .gap_col(15.0)
                .backdrop_acrylic(),
        );

        let main = v_flex(
            ts().items_center()
                .justify_center()
                .size_full()
                .m(20.0)
                .bg_color(rgba(34, 36, 42, 0.6)),
        );

        let title = text("WebView2 Composition Demo").style(
            ts().font_size(24.0)
                .text_color(Color::WHITE)
                .font_family("Segoe UI"),
        );

        let webview_element = {
            let wv = webview2(
                WebView2Contents::new("https://www.google.com/maps")
                    .enable_context_menu(true)
                    .enable_dev_tools(true)
                    .allow_interaction(true),
            )
            .style(
                ts().size((pct(80.0), pct(70.0)))
                    .r(12.0)
                    .resizable_all(true)
                    .transform(Transform::new().scale(1.0, 1.0))
                    .trans_border_color(Duration::from_millis(150), AnimationCurve::EaseInOutQuad)
                    .trans_transform(Duration::from_millis(150), AnimationCurve::EaseInOutQuad)
                    .focused(ts().border_solid(2.0).border_color(rgb(150, 150, 150)))
                    .pressed(ts().transform(Transform::new().scale(1.01, 1.01)))
                    .hovered(ts().box_shadow(blur(15.0).spread(1.0).color(Color::BLACK))),
            );

            webview_id_cell.set(Some(wv.id()));
            wv
        };

        root_node.child(
            main.child(title)
                .child(webview_element.child(div(btn_style.bg_color(Color::WHITE)))),
        )
    });

    let webview_id = webview_id_cell.get().expect("WebView2 ID not assigned");
    let initial_layout_size = LayoutSize::new(800.0, 600.0);
    // 4. アプリケーション状態をヒープ上に準備
    // ※ ウィンドウ生成非同期ハンドラ (ComposedRenderer) を作成するためにプレシーティング
    // (ComposedRenderer::new は内部で setup_direct_composition(hwnd) を行います)
    // ウィンドウを作成する（まだlpParamはNullの状態で枠のみを一旦作成、またはlpParamにapp_stateポインタをバインド）
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
        webview_id,
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
    app.renderer.prewarm_webview2();

    // 5. ウィンドウを表示して描画
    unsafe {
        let _ = ShowWindow(hwnd, SW_SHOW);
        let _ = InvalidateRect(Some(hwnd), None, false);
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
