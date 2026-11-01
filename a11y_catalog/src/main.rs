#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use crate::logger::logger;
use michiu::{prelude::*, ui::CapacityConfig};

mod app;
mod catalog;
mod logger;

// cargo build --example a11y_catalog  --release
fn main() -> std::result::Result<(), Box<dyn std::error::Error>> {
    let _ = MichiuWindow::init_dpi_awareness();
    let com = MichiuComContext::new_ro_single()?;

    let builder = MichiuWindowBuilder::new()
        .with_title("A11y Catalog")
        .with_visible(false)
        .with_decorations(false)
        .with_no_redirection_bitmap(true)
        .with_inner_size(LogicalSize::new(1000.0, 800.0))
        .with_com_context(&com)
        .with_default_ime_window(false)
        .with_message_filter(filter)
        .into_unvalidated();

    let window = MichiuWindow::build(builder.try_into()?)?;

    let scale_factor = window.scale_factor() as f32;
    let handle = window.handle().assume_valid();

    let client_rect = handle.client_rect();

    let renderer = pollster::block_on(MichiuRenderer::new(
        handle.hwnd(),
        client_rect.cast(),
        scale_factor,
    ))?;

    let inspector = MichiuInspector::new();
    let _sub = inspector.subscribe(None);

    logger(_sub);

    let (mut app, mut pump) = MichiuAppBuilder::new(window)
        .with_accessibility_support(true)
        .with_capacity_config(CapacityConfig::from_base_nodes(1024))
        .with_inspector(inspector)
        .with_sync_mode(AutoSyncMode::DwmFlush)
        .with_default_shortcuts(true)
        .with_auto_resolve_cursor(true)
        .build(renderer)?;

    app.build_ui(move || {
        let (read_handle, _) = create_signal(handle.into_inner());
        app::create_root().provide(read_handle)
    });

    app.set_visible(true);

    event_loop(&mut app, &mut pump)?;

    Ok(())
}

fn event_loop(app: &mut MichiuApp, pump: &mut MichiuEventPump) -> michiu::Result<()> {
    while pump.wait_event(|event, _, raw| {
        let resp = app.standard_handle_window_event(&event, &raw);

        if resp.needs_redraw {
            app.redraw_requested();
        }
        if resp.needs_update_window {
            app.update_window();
        }

        if resp.consumed {
            return;
        }

        if let MichiuAnyEvent::Window { event, .. } = event {
            match event {
                MichiuEvent::CloseRequested => {
                    app.destroy();
                }
                MichiuEvent::Destroyed => {
                    app.quit();
                }
                MichiuEvent::RedrawRequested => {
                    app.standard_redraw();
                }
                _ => {}
            }
        }
    })? {}

    Ok(())
}

use windows::Win32::{
    Foundation::{HWND, LPARAM, LRESULT, POINT, RECT, WPARAM},
    Graphics::Gdi::ScreenToClient,
    UI::{
        HiDpi::GetDpiForWindow,
        WindowsAndMessaging::{
            GetClientRect, HTBOTTOM, HTBOTTOMLEFT, HTBOTTOMRIGHT, HTCAPTION, HTCLIENT, HTLEFT,
            HTMAXBUTTON, HTRIGHT, HTTOP, HTTOPLEFT, HTTOPRIGHT, WM_ERASEBKGND, WM_NCCALCSIZE,
            WM_NCHITTEST,
        },
    },
};

fn filter(hwnd: HWND, msg: u32, _wparam: WPARAM, lparam: LPARAM) -> Option<LRESULT> {
    if msg == WM_ERASEBKGND {
        // OSによるデフォルト背景の塗りつぶしを阻止する
        return Some(LRESULT(1));
    }

    if msg == WM_NCCALCSIZE {
        return Some(LRESULT(0));
    }

    if msg == WM_NCHITTEST {
        // スケールファクター
        let dpi = unsafe { GetDpiForWindow(hwnd) };
        let scale = (dpi as f32) / 96.0;

        // スクリーン座標のマウス位置を取得
        let screen_x = (lparam.0 & 0xffff) as i16 as i32;
        let screen_y = ((lparam.0 >> 16) & 0xffff) as i16 as i32;

        // クライアント座標に変換
        let mut pt = POINT {
            x: screen_x,
            y: screen_y,
        };
        let _ = unsafe { ScreenToClient(hwnd, &raw mut pt) };

        // レンダラーと同じ GetClientRect を取得
        let mut client_rect = RECT::default();
        let _ = unsafe { GetClientRect(hwnd, &raw mut client_rect) };
        let width = client_rect.right; // client_rect.left は常に 0
        let height = client_rect.bottom; // client_rect.top は常に 0

        const BORDER_WIDTH: i32 = 8;
        let btn_w = (47.0 * scale) as i32;
        let btn_h = (40.0 * scale) as i32;

        // ボタンの領域判定（右端から3個分）
        let is_btn_row = pt.y >= 0 && pt.y < btn_h;
        let is_in_btn_area = is_btn_row && pt.x >= width - (btn_w * 3);

        // リサイズ枠の判定 (ボタン領域にいるときは除外)
        let on_left = pt.x < BORDER_WIDTH;
        let on_right = pt.x >= width - BORDER_WIDTH && !is_in_btn_area;
        let on_top = pt.y < BORDER_WIDTH && !is_in_btn_area;
        let on_bottom = pt.y >= height - BORDER_WIDTH;

        // 角
        if on_top && on_left {
            return Some(LRESULT(HTTOPLEFT as isize));
        }
        if on_top && on_right {
            return Some(LRESULT(HTTOPRIGHT as isize));
        }
        if on_bottom && on_left {
            return Some(LRESULT(HTBOTTOMLEFT as isize));
        }
        if on_bottom && on_right {
            return Some(LRESULT(HTBOTTOMRIGHT as isize));
        }

        // 辺
        if on_left {
            return Some(LRESULT(HTLEFT as isize));
        }
        if on_right {
            return Some(LRESULT(HTRIGHT as isize));
        }
        if on_top {
            return Some(LRESULT(HTTOP as isize));
        }
        if on_bottom {
            return Some(LRESULT(HTBOTTOM as isize));
        }

        // ボタンの判定
        // 最大化ボタン: スナップレイアウト等のため HTMAXBUTTON を返す
        if is_btn_row && pt.x >= width - (btn_w * 2) && pt.x < width - btn_w {
            return Some(LRESULT(HTMAXBUTTON as isize));
        }

        // 閉じるボタン & 最小化ボタン: UI側でクリック処理するため HTCLIENT を返す
        // (右端の 3個分のボタンエリアのうち、最大化ボタン以外)
        if is_in_btn_area {
            return Some(LRESULT(HTCLIENT as isize));
        }

        // タイトルバー領域
        if is_btn_row {
            let win_w = (client_rect.right - client_rect.left) as f32;
            let x_f = pt.x as f32;
            let y_f = pt.y as f32;

            // 検索ボックス領域 (幅 35%、最大 600px、高さ 26px、中央配置)
            let search_w = (win_w * 0.35).min(600.0 * scale);
            let center_x = client_rect.left as f32 + win_w / 2.0;
            let search_left = center_x - search_w / 2.0;
            let search_right = center_x + search_w / 2.0;

            let search_h = 26.0 * scale;
            let search_top = client_rect.top as f32 + (btn_h as f32 - search_h) / 2.0;
            let search_bottom = search_top + search_h;

            let is_on_search = x_f >= search_left
                && x_f < search_right
                && y_f >= search_top
                && y_f < search_bottom;

            // トグルボタン領域 (幅 50px、高さ 26px、right: 135px)
            let toggle_right = client_rect.right as f32 - (160.0 * scale);
            let toggle_left = toggle_right - (50.0 * scale);
            let toggle_top = client_rect.top as f32 + (btn_h as f32 - search_h) / 2.0;
            let toggle_bottom = toggle_top + search_h;

            let is_on_toggle = x_f >= toggle_left
                && x_f < toggle_right
                && y_f >= toggle_top
                && y_f < toggle_bottom;

            // UI コントロール上をクリックした場合は通常のクライアント操作
            if is_on_search || is_on_toggle {
                return Some(LRESULT(HTCLIENT as isize));
            }

            return Some(LRESULT(HTCAPTION as isize));
        }

        return Some(LRESULT(HTCLIENT as isize));
    }

    None
}
