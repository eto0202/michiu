#![allow(clippy::pedantic, clippy::restriction)]

use crate::window::{
    client_rect, create_renderer, create_window, message_loop, register_class, show_window,
};
use michiu_ui::{CapacityConfig, ComposedRenderer, Dss, DssSet, EntityId, prelude::*};
use windows::Win32::{
    Foundation::{HWND, LPARAM, WPARAM},
    System::WinRT::{RO_INIT_SINGLETHREADED, RoInitialize},
    UI::{
        HiDpi::{
            DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2, GetDpiForWindow,
            SetProcessDpiAwarenessContext,
        },
        WindowsAndMessaging::{
            GWLP_USERDATA, GetWindowLongPtrW, PostMessageW, SetWindowLongPtrW, WM_NULL,
        },
    },
};

mod app;
mod components;
mod window;

/// ウィンドウメッセージ処理時に Context と Renderer を一元管理するためのアプリケーション状態
struct AppState {
    renderer: ComposedRenderer,
    context: Context,
    root_id: EntityId,
    webview_id: Option<EntityId>,
}

// HWND を Send/Sync 化するラッパー
struct SendHwnd(HWND);
unsafe impl Send for SendHwnd {}
unsafe impl Sync for SendHwnd {}

impl SendHwnd {
    fn wake(&self) {
        let _ = unsafe { PostMessageW(Some(self.0), WM_NULL, WPARAM(0), LPARAM(0)) };
    }
}

#[cfg(feature = "dhat-heap")]
#[global_allocator]
static ALLOC: dhat::Alloc = dhat::Alloc;

// cargo build --example sample_collection --release
fn main() -> Result<(), Box<dyn std::error::Error>> {
    #[cfg(feature = "dhat-heap")]
    let _profiler = dhat::Profiler::new_heap();

    unsafe {
        let _ = SetProcessDpiAwarenessContext(DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2);
        let _ = RoInitialize(RO_INIT_SINGLETHREADED);
    }
    let (h_instance, class_name, _wnd_class) = register_class()?;
    let hwnd = create_window(h_instance, class_name)?;
    let mut context = Context::with_capacity(&CapacityConfig::from_base_nodes(1024));

    let send_hwnd = SendHwnd(hwnd);

    context.set_waker(move || send_hwnd.wake());

    let css_path = concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/examples/sample_collection/global.css"
    );

    let (styles_sig, _guard) = DssSet::builder()
        .add_sheet(Dss::new("global").from_file(css_path).hot_reload(true))
        .build_and_watch(&mut context);

    let root = build_ui(&mut context, move || app::create_root(styles_sig));

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
