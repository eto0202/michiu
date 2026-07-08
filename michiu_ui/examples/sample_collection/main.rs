use crate::{
    root::create_root,
    window::{
        client_rect, create_renderer, create_window, message_loop, register_class, show_window,
    },
};
use michiu_ui::{ComposedRenderer, EntityId, prelude::*};
use windows::Win32::{
    System::WinRT::{RO_INIT_SINGLETHREADED, RoInitialize},
    UI::{
        HiDpi::{
            DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2, GetDpiForWindow,
            SetProcessDpiAwarenessContext,
        },
        WindowsAndMessaging::{GWLP_USERDATA, GetWindowLongPtrW, SetWindowLongPtrW},
    },
};

mod components;
mod root;
mod theme;
mod window;

/// ウィンドウメッセージ処理時に Context と Renderer を一元管理するためのアプリケーション状態
struct AppState {
    renderer: ComposedRenderer,
    context: Context,
    root_id: EntityId,
    webview_id: Option<EntityId>,
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    unsafe {
        let _ = SetProcessDpiAwarenessContext(DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2);
        let _ = RoInitialize(RO_INIT_SINGLETHREADED);
    }
    let (h_instance, class_name, _wnd_class) = register_class()?;
    let hwnd = create_window(h_instance, class_name)?;

    let mut context = Context::new();
    let root = create_root(&mut context);

    let dpi = unsafe { GetDpiForWindow(hwnd) };
    let scale_factor = dpi as f32 / 96.0;
    let renderer = create_renderer(hwnd, scale_factor)?;

    let app_state = Box::new(AppState {
        renderer,
        context,
        root_id: root.id(),
        webview_id: None,
    });

    unsafe { SetWindowLongPtrW(hwnd, GWLP_USERDATA, Box::into_raw(app_state) as isize) };

    let app_ptr = unsafe { GetWindowLongPtrW(hwnd, GWLP_USERDATA) } as *mut AppState;
    let app = unsafe { &mut *app_ptr };

    let (width, height) = client_rect(hwnd);
    app.renderer.resize((width, height), scale_factor);
    app.context
        .sync_layout_and_render_list(app.root_id, app.renderer.layout_size);

    if app.webview_id.is_some() {
        app.renderer.prewarm_webview2();
    }

    let _ = show_window(hwnd);

    message_loop();

    Ok(())
}
