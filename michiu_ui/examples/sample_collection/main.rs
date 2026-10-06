#![allow(clippy::pedantic, clippy::restriction)]

use crate::{
    logger::logger,
    window::{
        client_rect, create_renderer, create_window, message_loop, register_class, show_window,
    },
};
use michiu_ui::{
    CapacityConfig, CssLoader, ExternalDataSetBuilder, WebView2Contents, WebView2Visual, prelude::*,
};
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
mod logger;
mod window;

/// ウィンドウメッセージ処理時に Context と Renderer を一元管理するためのアプリケーション状態
struct AppState {
    renderer: MichiuRenderer,
    context: Context,
    root_id: EntityId,
}

#[derive(Clone)]
pub struct GitHubVisual(WebView2Visual);
#[derive(Clone)]
pub struct YouTubeVisual(WebView2Visual);

// HWND を Send/Sync 化するラッパー
struct SendHwnd(HWND);
unsafe impl Send for SendHwnd {}
unsafe impl Sync for SendHwnd {}

impl SendHwnd {
    fn wake(&self) {
        let _ = unsafe { PostMessageW(Some(self.0), WM_NULL, WPARAM(0), LPARAM(0)) };
    }
}

pub const ALLOW_LOG: bool = false;
// これ起動めちゃ遅くなるので注意
pub const ALLOW_STRESS_TEST: bool = false;

#[cfg(feature = "dhat-heap")]
#[global_allocator]
static ALLOC: dhat::Alloc = dhat::Alloc;

// cargo build --timings --example sample_collection
// cargo build --example sample_collection --release
// cargo run --example sample_collection --release --features dhat-heap
fn main() -> Result<(), Box<dyn std::error::Error>> {
    #[cfg(feature = "dhat-heap")]
    let _profiler = dhat::Profiler::builder().trim_backtraces(None).build();

    unsafe {
        let _ = SetProcessDpiAwarenessContext(DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2);
        let _ = RoInitialize(RO_INIT_SINGLETHREADED);
    }
    let (h_instance, class_name, _wnd_class) = register_class()?;
    let hwnd = create_window(h_instance, class_name)?;

    let dpi = unsafe { GetDpiForWindow(hwnd) };
    let scale_factor = dpi as f32 / 96.0;
    let renderer = create_renderer(hwnd, scale_factor)?;

    let inspector = MichiuInspector::new();
    let _sub = inspector.subscribe(None);

    let mut context =
        Context::with_capacity_and_inspector(&CapacityConfig::from_base_nodes(1024), &inspector)
            .with_accessibility(hwnd);

    // デバッグログ用のスレッド
    #[cfg(feature = "trace-error")]
    logger(_sub);

    let device = renderer.composition_device()?;
    let task_sender = context.task_sender();

    let github = WebView2Contents::from_url("https://github.com/eto0202/michiu/tree/main")
        .enable_context_menu(true)
        .enable_dev_tools(true)
        .allow_interaction(true)
        .always_active(false);

    let youtube = WebView2Contents::from_url("https://www.youtube.com/")
        .allow_interaction(true)
        .always_active(true);

    let github_visual = WebView2Visual::new(&device, hwnd, github, scale_factor, &task_sender)
        .expect("Failed to create WebView2Visual");

    let youtube_visual = WebView2Visual::new(&device, hwnd, youtube, scale_factor, &task_sender)
        .expect("Failed to create WebView2Visual");

    WebView2Visual::prewarm_webview2();

    let send_hwnd = SendHwnd(hwnd);
    context.set_waker(move || send_hwnd.wake());

    let css_path = concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/examples/sample_collection/global.css"
    );
    let (styles_sig, _guard) = ExternalDataSetBuilder::new()
        .add("global", css_path, CssLoader)
        .watch(&mut context)?;

    let root = build_ui(&mut context, move || {
        let (read_github, _) = create_signal(GitHubVisual(github_visual));
        let (read_youtube, _) = create_signal(YouTubeVisual(youtube_visual));
        app::create_root()
            .provide(styles_sig)
            .provide(read_github)
            .provide(read_youtube)
    });

    let app_state = Box::new(AppState {
        renderer,
        context,
        root_id: root.id(),
    });

    unsafe { SetWindowLongPtrW(hwnd, GWLP_USERDATA, Box::into_raw(app_state) as isize) };

    let _ = show_window(hwnd);

    message_loop();

    Ok(())
}
