use std::{
    borrow::Cow,
    cell::{Cell, RefCell},
    rc::Rc,
    sync::{Arc, Mutex, OnceLock},
};
use webview2_com::{
    CapturePreviewCompletedHandler, CreateCoreWebView2EnvironmentCompletedHandler,
    Microsoft::Web::WebView2::Win32::{
        COREWEBVIEW2_CAPTURE_PREVIEW_IMAGE_FORMAT_PNG, COREWEBVIEW2_COLOR,
        COREWEBVIEW2_MOUSE_EVENT_KIND, COREWEBVIEW2_MOUSE_EVENT_VIRTUAL_KEYS,
        COREWEBVIEW2_MOVE_FOCUS_REASON, COREWEBVIEW2_MOVE_FOCUS_REASON_PREVIOUS,
        COREWEBVIEW2_MOVE_FOCUS_REASON_PROGRAMMATIC, CreateCoreWebView2EnvironmentWithOptions,
        ICoreWebView2, ICoreWebView2CompositionController, ICoreWebView2Controller,
        ICoreWebView2Controller2, ICoreWebView2Environment, ICoreWebView2Environment3,
    },
};
use windows::{
    Win32::{
        Foundation::{HGLOBAL, HMODULE, HWND, LPARAM, POINT, RECT, WPARAM},
        Graphics::{
            Direct3D::{D3D_DRIVER_TYPE_HARDWARE, ID3DInclude_Impl},
            Direct3D11::{
                D3D11_CREATE_DEVICE_BGRA_SUPPORT, D3D11_SDK_VERSION, D3D11CreateDevice,
                ID3D11Device,
            },
            DirectComposition::{
                DCompositionCreateDevice2, IDCompositionDesktopDevice,
                IDCompositionDesktopDevice_Impl, IDCompositionDevice, IDCompositionDevice_Impl,
                IDCompositionDevice2_Impl, IDCompositionRectangleClip_Impl, IDCompositionTarget,
                IDCompositionTarget_Impl, IDCompositionTranslateTransform_Impl,
                IDCompositionTranslateTransform3D_Impl, IDCompositionVisual,
                IDCompositionVisual_Impl, IDCompositionVisual2, IDCompositionVisual3_Impl,
            },
            Dxgi::*,
            Gdi::InvalidateRect,
            Imaging::{
                CLSID_WICImagingFactory, GUID_WICPixelFormat32bppBGRA,
                GUID_WICPixelFormat32bppPBGRA, GUID_WICPixelFormat32bppRGBA, IWICImagingFactory,
                WICBitmapDitherTypeNone, WICBitmapInterpolationModeLinear,
                WICBitmapPaletteTypeCustom, WICBitmapPaletteTypeMedianCut,
                WICDecodeMetadataCacheOnDemand,
            },
        },
        System::{
            Com::{
                CLSCTX_INPROC_SERVER, CoCreateInstance, IStream,
                StructuredStorage::{CreateStreamOnHGlobal, GetHGlobalFromStream},
            },
            Memory::{GlobalLock, GlobalSize, GlobalUnlock},
        },
        UI::WindowsAndMessaging::{
            GWL_EXSTYLE, GetWindowLongW, SetWindowLongW, WM_LBUTTONDOWN, WM_LBUTTONUP,
            WM_MBUTTONDOWN, WM_MBUTTONUP, WM_MOUSEHWHEEL, WM_MOUSEMOVE, WM_MOUSEWHEEL,
            WM_RBUTTONDOWN, WM_RBUTTONUP,
        },
    },
    core::{Interface, PCWSTR, PWSTR, w},
};
use windows_core::{HRESULT, HSTRING};

use crate::{
    ExternalTexture, ExternalVisual, ExternalVisualMetadata, LayoutPoint, LayoutRect, LayoutSize,
    MichiuError, StaticExternalTexture, TaskSender, VisualUpdateContext,
};

#[derive(Debug, Clone, PartialEq)]
pub enum WebView2Source {
    /// 外部のWebサイトやローカルのサーバー
    Url(Cow<'static, str>),
    /// 生のHTMLコード
    Html(Cow<'static, str>),
}

impl Default for WebView2Source {
    fn default() -> Self {
        Self::new()
    }
}

impl WebView2Source {
    #[inline]
    #[must_use]
    pub fn new() -> Self {
        Self::Url("about:blank".into())
    }
}

#[allow(clippy::struct_excessive_bools)]
#[derive(Debug, Clone, PartialEq)]
pub struct WebView2Contents {
    pub source: WebView2Source,
    /// イベントフォワード（マウス/キーボード入力を受け付けるか）
    pub allow_interaction: bool,
    /// 右クリックのシステムデフォルトメニューを表示するか
    pub enable_context_menu: bool,
    /// F12で開発者ツールを起動できるか
    pub enable_dev_tools: bool,
    /// `JavaScriptを有効にするか`
    pub enable_scripts: bool,
    /// 起動時（ドキュメント読み込み前）に自動実行させるJavaScript
    pub user_scripts: Vec<Cow<'static, str>>,
    /// ユーザーが操作していなくても、常にコンポジションスレッドで再生し続けるか
    pub always_active: bool,
}

impl Default for WebView2Contents {
    fn default() -> Self {
        Self {
            source: WebView2Source::new(),
            allow_interaction: true,
            enable_context_menu: false, // デフォルトでは消してアプリ感を出す
            enable_dev_tools: false,    // デフォルトはオフ
            enable_scripts: true,
            user_scripts: Vec::new(),
            always_active: false,
        }
    }
}

impl WebView2Contents {
    #[inline]
    #[must_use]
    pub fn new(source: WebView2Source) -> Self {
        Self {
            source,
            ..Default::default()
        }
    }

    #[inline]
    pub fn from_url(url: impl Into<Cow<'static, str>>) -> Self {
        Self::new(WebView2Source::Url(url.into()))
    }

    #[inline]
    pub fn from_html(html: impl Into<Cow<'static, str>>) -> Self {
        Self::new(WebView2Source::Html(html.into()))
    }

    #[inline]
    #[must_use]
    pub fn url(mut self, url: impl Into<Cow<'static, str>>) -> Self {
        self.source = WebView2Source::Url(url.into());
        self
    }

    #[inline]
    #[must_use]
    pub fn html(mut self, html: impl Into<Cow<'static, str>>) -> Self {
        self.source = WebView2Source::Html(html.into());
        self
    }

    #[inline]
    #[must_use]
    pub fn allow_interaction(mut self, allow: bool) -> Self {
        self.allow_interaction = allow;
        self
    }

    #[inline]
    #[must_use]
    pub fn enable_context_menu(mut self, enable: bool) -> Self {
        self.enable_context_menu = enable;
        self
    }

    #[inline]
    #[must_use]
    pub fn enable_dev_tools(mut self, enable: bool) -> Self {
        self.enable_dev_tools = enable;
        self
    }

    #[inline]
    #[must_use]
    pub fn enable_scripts(mut self, enable: bool) -> Self {
        self.enable_scripts = enable;
        self
    }

    /// example
    /// `add_user_script(include_str!("example.js`"))
    #[inline]
    #[must_use]
    pub fn add_user_script(mut self, script: impl Into<Cow<'static, str>>) -> Self {
        self.user_scripts.push(script.into());
        self
    }

    /// 動画プレイヤーやWebGL、アニメーションがある場合、常時レンダリングを有効にする
    #[inline]
    #[must_use]
    pub fn always_active(mut self, always: bool) -> Self {
        self.always_active = always;
        self
    }
}

thread_local! {
    /// UI スレッドごとに 1 つだけ環境をキャッシュする
    static WEBVIEW2_THREAD_ENV: RefCell<Option<ICoreWebView2Environment3>> = const { RefCell::new(None) };
}

/// キャッシュ済みの環境があればクローンして即座に借用を解放して返す
#[must_use]
pub fn get_cached_env() -> Option<ICoreWebView2Environment3> {
    WEBVIEW2_THREAD_ENV.with(|slot| slot.borrow().clone())
}

/// スレッドローカルに環境をキャッシュする
pub fn set_cached_env(env: ICoreWebView2Environment3) {
    WEBVIEW2_THREAD_ENV.with(|slot| {
        *slot.borrow_mut() = Some(env);
    });
}

pub struct WebView2Visual {
    pub visual: IDCompositionVisual2,
    /// コントローラー（非同期生成完了時に格納）
    pub controller: Rc<RefCell<Option<ICoreWebView2Controller>>>,
    /// 静止画キャッシュ（アクティブ時は None、キャプチャ完了時に Some）
    pub cached_texture: Rc<RefCell<Option<Arc<StaticExternalTexture>>>>,
    /// キャプチャ実行中フラグ
    pub is_capturing: Rc<RefCell<bool>>,
    /// 設定情報
    pub contents: WebView2Contents,
    /// メタデータ
    pub metadata: ExternalVisualMetadata,
    current_rect: Cell<LayoutRect>,
    scale_factor: Cell<f32>,
}

// COM ポインタを抱えるため、UI スレッド/STA で安全に扱える前提で Send + Sync を付与
unsafe impl Send for WebView2Visual {}
unsafe impl Sync for WebView2Visual {}

impl ExternalVisual for WebView2Visual {
    fn resolve_visual(&self) -> IDCompositionVisual2 {
        self.visual.clone()
    }

    fn static_texture(&self) -> Option<Arc<dyn ExternalTexture>> {
        self.cached_texture
            .borrow()
            .clone()
            .map(|t| t as Arc<dyn ExternalTexture>)
    }

    fn update(&self, cx: &VisualUpdateContext) {
        self.current_rect.set(cx.rect);
        self.scale_factor.set(cx.scale_factor);

        let phys_w = (cx.rect.width * cx.scale_factor).round() as i32;
        let phys_h = (cx.rect.height * cx.scale_factor).round() as i32;

        if let Some(ref controller) = *self.controller.borrow() {
            // WebView2 自身の Bounds を同期 (0,0 原点)
            let bounds = RECT {
                left: 0,
                top: 0,
                right: phys_w,
                bottom: phys_h,
            };
            let _ = unsafe { controller.SetBounds(bounds) };

            // 操作中・常時アクティブ・トランジション中は実体を維持
            let should_stay_active =
                cx.is_interactive || self.contents.always_active || !cx.is_stable;
            if should_stay_active {
                *self.cached_texture.borrow_mut() = None;
                return;
            }

            // 安定しており、かつまだキャッシュもキャプチャ中もない場合、自動キャプチャを発火
            if self.cached_texture.borrow().is_none()
                && !*self.is_capturing.borrow()
                && phys_w > 0
                && phys_h > 0
                && let Ok(webview) = unsafe { controller.CoreWebView2() }
            {
                *self.is_capturing.borrow_mut() = true;

                let hwnd = cx.hwnd;
                let device = cx.device.clone();
                let queue = cx.queue.clone();
                let wic_factory = cx.wic_factory.clone();
                let size = LayoutSize::new(cx.rect.width, cx.rect.height);

                let cached_texture_clone = self.cached_texture.clone();
                let is_capturing_clone = self.is_capturing.clone();

                // 非同期キャプチャの実行
                let _ = unsafe {
                    Self::trigger_capture_async(
                        &webview,
                        phys_w as u32,
                        phys_h as u32,
                        device,
                        queue,
                        wic_factory,
                        move |result| {
                            if let Ok(wgpu_tex) = result {
                                let view =
                                    wgpu_tex.create_view(&wgpu::TextureViewDescriptor::default());
                                let static_tex = Arc::new(StaticExternalTexture::new(view, size));
                                // キャッシュをセットして強制再描画をキック
                                *cached_texture_clone.borrow_mut() = Some(static_tex);
                            }
                            *is_capturing_clone.borrow_mut() = false;
                            let _ = InvalidateRect(Some(hwnd), None, false);
                        },
                    )
                };
            }
        }
    }

    fn handle_raw_input(
        &self,
        msg: u32,
        wparam: WPARAM,
        _lparam: LPARAM,
        local_phys_pos: POINT,
    ) -> bool {
        if !self.contents.allow_interaction {
            return false;
        }

        match msg {
            WM_MOUSEMOVE | WM_LBUTTONDOWN | WM_LBUTTONUP | WM_RBUTTONDOWN | WM_RBUTTONUP
            | WM_MBUTTONDOWN | WM_MBUTTONUP | WM_MOUSEWHEEL | WM_MOUSEHWHEEL => {
                // 操作が行われたため静止キャッシュをクリア
                *self.cached_texture.borrow_mut() = None;

                if let Some(ref controller) = *self.controller.borrow()
                    && let Ok(comp) = controller.cast::<ICoreWebView2CompositionController>()
                {
                    let event_kind = COREWEBVIEW2_MOUSE_EVENT_KIND(msg as i32);
                    let virtual_keys = COREWEBVIEW2_MOUSE_EVENT_VIRTUAL_KEYS(wparam.0 as i32);
                    let mouse_data = if msg == WM_MOUSEWHEEL || msg == WM_MOUSEHWHEEL {
                        ((wparam.0 >> 16) as i16) as i32 as u32
                    } else {
                        0
                    };
                    unsafe {
                        let _ = comp.SendMouseInput(
                            event_kind,
                            virtual_keys,
                            mouse_data,
                            local_phys_pos,
                        );
                        let _ = controller.MoveFocus(COREWEBVIEW2_MOVE_FOCUS_REASON_PROGRAMMATIC);
                    }
                }
                true
            }
            _ => {
                false
            }
        }
    }

    fn metadata(&self) -> ExternalVisualMetadata {
        self.metadata
    }
}

impl WebView2Visual {
    pub fn new(
        dcomp_device: &IDCompositionDevice,
        hwnd: HWND,
        contents: WebView2Contents,
        scale_factor: f32,
        task_sender: &TaskSender,
    ) -> crate::Result<Self> {
        let visual: IDCompositionVisual2 = unsafe { dcomp_device.CreateVisual()?.cast()? };
        let controller = Rc::new(RefCell::new(None));

        let cached_env = get_cached_env();

        unsafe {
            Self::init_webview2_composition(
                hwnd,
                &visual,
                &controller,
                &contents,
                scale_factor,
                cached_env,
                task_sender,
            )?;
        }

        let metadata = ExternalVisualMetadata {
            size: LayoutSize::ZERO,
            auto_clip: true,
            auto_transform: true,
        };

        Ok(Self {
            visual,
            controller,
            cached_texture: Rc::new(RefCell::new(None)),
            is_capturing: Rc::new(RefCell::new(false)),
            contents,
            metadata,
            current_rect: Cell::default(),
            scale_factor: Cell::new(scale_factor),
        })
    }

    /// マウス入力の転送
    pub fn forward_mouse_input(
        &self,
        msg: u32,
        wparam: WPARAM,
        lparam: LPARAM,
        physical_window_pos: LayoutPoint,
    ) {
        if !self.contents.allow_interaction {
            return;
        }

        // 操作されたので静止キャッシュをクリアして即座に実体復帰
        *self.cached_texture.borrow_mut() = None;

        let rect = self.current_rect.get();
        let scale = self.scale_factor.get();
        let webview_phys_x = rect.x * scale;
        let webview_phys_y = rect.y * scale;

        let relative_point = POINT {
            x: (physical_window_pos.x - webview_phys_x).round() as i32,
            y: (physical_window_pos.y - webview_phys_y).round() as i32,
        };

        if let Some(ref controller) = *self.controller.borrow()
            && let Ok(comp) = controller.cast::<ICoreWebView2CompositionController>()
        {
            let event_kind = COREWEBVIEW2_MOUSE_EVENT_KIND(msg as i32);
            let virtual_keys = COREWEBVIEW2_MOUSE_EVENT_VIRTUAL_KEYS(wparam.0 as i32);
            let mouse_data = if msg == WM_MOUSEWHEEL || msg == WM_MOUSEHWHEEL {
                ((wparam.0 >> 16) as i16) as i32 as u32
            } else {
                0
            };
            unsafe {
                let _ = comp.SendMouseInput(event_kind, virtual_keys, mouse_data, relative_point);
            }
        }
    }

    /// フォーカス移動
    pub fn focus(&self) {
        *self.cached_texture.borrow_mut() = None;

        if let Some(ref controller) = *self.controller.borrow() {
            unsafe {
                let _ = controller.MoveFocus(COREWEBVIEW2_MOVE_FOCUS_REASON_PROGRAMMATIC);
            }
        }
    }

    pub fn prewarm_webview2() {
        if get_cached_env().is_some() {
            return;
        }
        unsafe {
            let _ = CreateCoreWebView2EnvironmentCompletedHandler::wait_for_async_operation(
                Box::new(|handler| {
                    CreateCoreWebView2EnvironmentWithOptions(None, None, None, &handler)
                        .map_err(webview2_com::Error::WindowsError)
                }),
                Box::new(|res, env| {
                    if let Ok(()) = res
                        && let Some(e_ptr) = env
                        && let Ok(env3) = e_ptr.cast::<ICoreWebView2Environment3>()
                    {
                        set_cached_env(env3);
                    }
                    Ok(())
                }),
            );
        }
    }

    #[allow(clippy::too_many_arguments)]
    pub(crate) unsafe fn init_webview2_composition(
        hwnd: HWND,
        webview_visual: &IDCompositionVisual2,
        controller_slot: &Rc<RefCell<Option<ICoreWebView2Controller>>>,
        settings: &WebView2Contents,
        scale_factor: f32,
        env_slot: Option<ICoreWebView2Environment3>,
        sys_task_sender: &TaskSender,
    ) -> crate::Result<()> {
        let webview_visual_clone = webview_visual.clone();
        let controller_slot_clone = controller_slot.clone();
        let settings_clone = settings.clone();
        let task_sender_clone = sys_task_sender.clone();

        // プリウォーム済み環境（Environment）の利用
        if let Some(env3) = env_slot {
            let handler =
                webview2_com::CreateCoreWebView2CompositionControllerCompletedHandler::create(
                    Box::new(
                        move |res, controller: Option<ICoreWebView2CompositionController>| {
                            // このクロージャは後から非同期にブラウザの初期化が終わった瞬間に実行。
                            res.map_err(webview2_com::Error::WindowsError);

                            let comp_controller =
                                controller.ok_or_else(windows_core::Error::from_thread)?;
                            unsafe { comp_controller.SetRootVisualTarget(&webview_visual_clone) }?;

                            let base_controller: ICoreWebView2Controller =
                                comp_controller.cast()?;

                            // どうやら webview2 はデフォルトで半透明らしく、その状態でキャプチャすると半透明な画像として出てくるみたい
                            unsafe {
                                base_controller
                                    .cast::<ICoreWebView2Controller2>()?
                                    .SetDefaultBackgroundColor(COREWEBVIEW2_COLOR {
                                        A: 255,
                                        R: 255,
                                        G: 255,
                                        B: 255,
                                    })
                            };

                            // 位置（left, top）は DComp 側に一任するため 0 に設定
                            let bounds = RECT {
                                left: 0,
                                top: 0,
                                right: 0,
                                bottom: 0,
                            };
                            unsafe {
                                base_controller.SetBounds(bounds)?;
                                base_controller.SetIsVisible(true)?;
                            }

                            // キーボードフォーカス奪還ハンドラをバインド
                            let task_sender_inner = task_sender_clone.clone();
                            let focus_handler =
                                webview2_com::MoveFocusRequestedEventHandler::create(Box::new(
                                    move |_sender, args| {
                                        if let Some(args) = args {
                                            let mut reason = COREWEBVIEW2_MOVE_FOCUS_REASON(0);
                                            let _ = unsafe { args.Reason(&raw mut reason) };

                                            // Tab なら 順順移動(false), Shift+Tab なら 逆順移動(true)
                                            let is_reverse =
                                                reason == COREWEBVIEW2_MOVE_FOCUS_REASON_PREVIOUS;

                                            // メインスレッドの Context にフォーカス循環要求をディスパッチ
                                            let _ = task_sender_inner.send(move |cx| {
                                                cx.cycle_keyboard_focus(is_reverse);
                                            });

                                            // WebView2側に「ホストアプリがフォーカスを奪還した」ことを通知
                                            let _ = unsafe { args.SetHandled(true) };
                                        }
                                        Ok(())
                                    },
                                ));
                            unsafe {
                                base_controller.add_MoveFocusRequested(&focus_handler, &mut 0)?;
                            }

                            let webview = unsafe { base_controller.CoreWebView2()? };
                            let web_settings = unsafe { webview.Settings()? };

                            unsafe {
                                let _ =
                                    web_settings.SetIsScriptEnabled(settings_clone.enable_scripts);
                                let _ = web_settings
                                    .SetAreDevToolsEnabled(settings_clone.enable_dev_tools);
                                let _ = web_settings.SetAreDefaultContextMenusEnabled(
                                    settings_clone.enable_context_menu,
                                );
                            }

                            // ユーザースクリプト登録も非同期で安全に登録
                            for script in &settings_clone.user_scripts {
                                let script_u16: Vec<u16> =
                                    script.encode_utf16().chain(Some(0)).collect();
                                let pcw_script = PCWSTR(script_u16.as_ptr());
                                let webview_clone = webview.clone();

                                let script_handler = webview2_com::AddScriptToExecuteOnDocumentCreatedCompletedHandler::create(
                                Box::new(|res, _id| {
                                    res.map_err(webview2_com::Error::WindowsError);
                                    Ok(())
                                })
                            );
                                unsafe {
                                    webview_clone.AddScriptToExecuteOnDocumentCreated(
                                        pcw_script,
                                        &script_handler,
                                    )?;
                                }
                            }

                            // NavigationCompleted を購読して、描画ピクセルが準備できた段階でスロットに代入する
                            let controller_slot_inner = controller_slot_clone.clone();
                            let base_controller_inner = base_controller.clone();
                            let nav_handler = webview2_com::NavigationCompletedEventHandler::create(
                                Box::new(move |_sender, _args| {
                                    // ページが完全に初期描画フェーズに移行した
                                    *controller_slot_inner.borrow_mut() =
                                        Some(base_controller_inner.clone());

                                    // 強制的に再描画を走らせて穴あけと描画をトリガー
                                    let _ = unsafe { InvalidateRect(Some(hwnd), None, false) };

                                    Ok(())
                                }),
                            );

                            unsafe {
                                webview.add_NavigationCompleted(&nav_handler, &mut 0)?;
                            }

                            let source_string = match &settings_clone.source {
                                WebView2Source::Url(url) => HSTRING::from(url.as_ref()),
                                WebView2Source::Html(html) => HSTRING::from(html.as_ref()),
                            };

                            unsafe {
                                match &settings_clone.source {
                                    WebView2Source::Url(_) => webview.Navigate(&source_string)?,
                                    WebView2Source::Html(_) => {
                                        webview.NavigateToString(&source_string)?;
                                    }
                                }
                            }

                            Ok(())
                        },
                    ),
                );

            // 非同期処理をキックして即時リターン（ノンブロッキング）
            unsafe {
                env3.CreateCoreWebView2CompositionController(hwnd, &handler)?;
            }

            return Ok(());
        }

        // 初期起動時（env_slot がまだロード途中）の場合のフォールバック
        let handler = webview2_com::CreateCoreWebView2EnvironmentCompletedHandler::create(
            Box::new(move |res, environment: Option<ICoreWebView2Environment>| {
                res.map_err(webview2_com::Error::WindowsError);
                let env = environment.ok_or_else(windows_core::Error::from_thread)?;
                let env3: ICoreWebView2Environment3 = env.cast()?;

                // 環境スロットにキャッシュを保存
                set_cached_env(env3.clone());

                let webview_visual_clone2 = webview_visual_clone.clone();
                let controller_slot_clone2 = controller_slot_clone.clone();
                let settings_clone2 = settings_clone.clone();

                let controller_handler =
                    webview2_com::CreateCoreWebView2CompositionControllerCompletedHandler::create(
                        Box::new(
                            move |res, controller: Option<ICoreWebView2CompositionController>| {
                                res.map_err(webview2_com::Error::WindowsError);
                                let comp_controller =
                                    controller.ok_or_else(windows_core::Error::from_thread)?;
                                unsafe {
                                    comp_controller.SetRootVisualTarget(&webview_visual_clone2)
                                }?;

                                let base_controller: ICoreWebView2Controller =
                                    comp_controller.cast()?;

                                unsafe {
                                    base_controller
                                        .cast::<ICoreWebView2Controller2>()?
                                        .SetDefaultBackgroundColor(COREWEBVIEW2_COLOR {
                                            A: 255,
                                            R: 255,
                                            G: 255,
                                            B: 255,
                                        })
                                };

                                let bounds = RECT {
                                    left: 0,
                                    top: 0,
                                    right: 0,
                                    bottom: 0,
                                };
                                unsafe {
                                    base_controller.SetBounds(bounds)?;
                                    base_controller.SetIsVisible(true)?;
                                }

                                // キーボードフォーカス奪還ハンドラをバインド
                                let task_sender_inner = task_sender_clone.clone();
                                let focus_handler =
                                    webview2_com::MoveFocusRequestedEventHandler::create(Box::new(
                                        move |_sender, args| {
                                            if let Some(args) = args {
                                                let mut reason = COREWEBVIEW2_MOVE_FOCUS_REASON(0);
                                                let _ = unsafe { args.Reason(&raw mut reason) };

                                                // Tab なら 順順移動(false), Shift+Tab なら 逆順移動(true)
                                                let is_reverse = reason
                                                    == COREWEBVIEW2_MOVE_FOCUS_REASON_PREVIOUS;

                                                // メインスレッドの Context にフォーカス循環要求をディスパッチ
                                                let _ = task_sender_inner.send(move |cx| {
                                                    cx.cycle_keyboard_focus(is_reverse);
                                                });

                                                // WebView2側に「ホストアプリがフォーカスを奪還した」ことを通知
                                                let _ = unsafe { args.SetHandled(true) };
                                            }
                                            Ok(())
                                        },
                                    ));
                                unsafe {
                                    base_controller
                                        .add_MoveFocusRequested(&focus_handler, &mut 0)?;
                                }

                                let webview = unsafe { base_controller.CoreWebView2()? };

                                let controller_slot_inner = controller_slot_clone.clone();
                                let base_controller_inner = base_controller.clone();
                                let nav_handler =
                                    webview2_com::NavigationCompletedEventHandler::create(
                                        Box::new(move |_sender, _args| {
                                            *controller_slot_inner.borrow_mut() =
                                                Some(base_controller_inner.clone());

                                            let _ =
                                                unsafe { InvalidateRect(Some(hwnd), None, false) };

                                            Ok(())
                                        }),
                                    );

                                unsafe {
                                    webview.add_NavigationCompleted(&nav_handler, &mut 0)?;
                                }

                                let source_string = match &settings_clone.source {
                                    WebView2Source::Url(url) => HSTRING::from(url.as_ref()),
                                    WebView2Source::Html(html) => HSTRING::from(html.as_ref()),
                                };

                                unsafe {
                                    match &settings_clone.source {
                                        WebView2Source::Url(_) => {
                                            webview.Navigate(&source_string)?;
                                        }
                                        WebView2Source::Html(_) => {
                                            webview.NavigateToString(&source_string)?;
                                        }
                                    }
                                }

                                Ok(())
                            },
                        ),
                    );

                unsafe {
                    env3.CreateCoreWebView2CompositionController(hwnd, &controller_handler)?;
                }

                Ok(())
            }),
        );

        unsafe {
            CreateCoreWebView2EnvironmentWithOptions(None, None, None, &handler)?;
        }

        Ok(())
    }

    /// `WebView2` から非アクティブ時の静止画（1フレーム）をキャプチャし、
    /// wgpu 側の `wgpu::Texture` へとピクセルデータを転送します。
    pub(crate) unsafe fn trigger_capture_async<F>(
        webview: &ICoreWebView2,
        width: u32,
        height: u32,
        wgpu_device: wgpu::Device,
        wgpu_queue: wgpu::Queue,
        wic_factory: IWICImagingFactory,
        on_complete: F,
    ) -> crate::Result<()>
    where
        F: FnOnce(crate::Result<wgpu::Texture>) + 'static,
    {
        unsafe {
            // メモリ上に COM の IStream を作成
            let stream = CreateStreamOnHGlobal(HGLOBAL::default(), true)?;
            let stream_clone = stream.clone();

            // 完了コールバックの所有権をハンドラー内に安全に移すため、Option に包む
            let mut on_complete_opt = Some(on_complete);

            // ICoreWebView2::CapturePreview を呼び出し、PNG 形式でストリームに書き込ませる
            // ICoreWebView2CapturePreviewCompletedHandler を用いて非同期完了を同期的に待機
            webview.CapturePreview(
                COREWEBVIEW2_CAPTURE_PREVIEW_IMAGE_FORMAT_PNG,
                &stream,
                // 完了ハンドラー
                &CapturePreviewCompletedHandler::create(Box::new(move |result| {
                    // このクロージャは、WebView2 側の処理完了時にメインスレッド（STA）上で呼び出される
                    let on_complete = on_complete_opt.take().ok_or_else(|| {
                        // とりあえず E_FAIL を返しておく
                        windows_core::Error::from_hresult(HRESULT(0x8000_4005_u32.cast_signed()))
                    })?;

                    if let Err(e) = result {
                        on_complete(Err(e.into()));
                        return Ok(());
                    }

                    // ストリームから HGLOBAL をクエリして WIC で PMA (BGRA8) にデコード
                    let texture_res = Self::process_captured_stream(
                        &wgpu_device,
                        &wgpu_queue,
                        &wic_factory,
                        &stream_clone,
                        width,
                        height,
                    );

                    // 完了コールバックを呼び出してテクスチャを通知
                    on_complete(texture_res);
                    Ok(())
                })),
            )?;

            Ok(())
        }
    }

    /// 内部ヘルパー: ストリームに書き込まれた PNG バイト列を WIC で高速に BGRA8 ピクセル配列にデコードし、
    /// `wgpu::Texture` を作成してアップロード
    unsafe fn process_captured_stream(
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        wic_factory: &IWICImagingFactory,
        stream: &IStream,
        width: u32,
        height: u32,
    ) -> crate::Result<wgpu::Texture> {
        unsafe {
            let hglobal = GetHGlobalFromStream(stream)?;
            let data_ptr = GlobalLock(hglobal);

            // ポインタが null の場合は早期リターン
            if data_ptr.is_null() {
                return Err(MichiuError::GlobalLockFailed);
            }

            let size = GlobalSize(hglobal);

            // 正常な PNG ファイルとして不十分なサイズの場合は即座にエラーとする
            if size < 128 {
                let _ = GlobalUnlock(hglobal);
                return Err(MichiuError::InvalidImageData);
            }

            let png_bytes = std::slice::from_raw_parts(data_ptr as *const u8, size);

            // wgpu 内にすでに定義されている WIC ファクトリ（IWICImagingFactory）を利用
            let wic_stream = wic_factory.CreateStream()?;
            wic_stream.InitializeFromMemory(png_bytes)?;

            // デコーダーとフレームの構築
            let decoder = wic_factory.CreateDecoderFromStream(
                &wic_stream,
                std::ptr::null(),
                WICDecodeMetadataCacheOnDemand,
            )?;

            let frame = decoder.GetFrame(0)?;

            // ストレート BGRA 形式に変換（PBGRA ではなくストレート BGRA）
            let converter = wic_factory.CreateFormatConverter()?;
            converter.Initialize(
                &frame,
                &GUID_WICPixelFormat32bppBGRA,
                WICBitmapDitherTypeNone,
                None,
                0.0,
                WICBitmapPaletteTypeCustom,
            )?;

            // WebView2 から提出された元画像サイズがどうであれ、
            // 目標の wgpu テクスチャサイズ (width x height) へ正確にリサイズ。
            let scaler = wic_factory.CreateBitmapScaler()?;
            scaler.Initialize(&converter, width, height, WICBitmapInterpolationModeLinear)?;

            let mut pixels = vec![0u8; (width * height * 4) as usize];
            scaler.CopyPixels(std::ptr::null(), width * 4, &mut pixels)?;

            // 静止画キャッシュの完全不透明化と、R/B チャンネル反転の修正
            // WIC の仕様上、scaler は入力されたフォーマットをそのまま維持しようとするが
            // GUID_WICPixelFormat32bppRGBA を直接リサイズする際に
            // 内部でWindows標準のBGRAに戻してしまうケースがあるらしい。
            for chunk in pixels.as_chunks_mut::<4>().0 {
                // どうやら現在 [B, G, R, A] で入っているっぽい
                let b = chunk[0];
                let r = chunk[2];
                chunk[0] = r; // B の位置に R を上書き
                chunk[2] = b; // R の位置に B を上書き

                chunk[3] = 255; // 保険としての不透明化
            }

            // メモリロック解除
            // windows-rs の自動 Result 変換のバグ（S_OK/NO_ERROR なのに 0 返却のため Err になる）を
            // 回避するため、? によるエラー早期返却をやめ、単に返り値を破棄する。
            let _ = GlobalUnlock(hglobal);

            // 新規 wgpu::Texture の生成
            let wgpu_texture = device.create_texture(&wgpu::TextureDescriptor {
                label: Some("WebView2 Static Cache Texture"),
                size: wgpu::Extent3d {
                    width,
                    height,
                    depth_or_array_layers: 1,
                },
                mip_level_count: 1,
                sample_count: 1,
                dimension: wgpu::TextureDimension::D2,
                format: wgpu::TextureFormat::Rgba8UnormSrgb,
                usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
                view_formats: &[],
            });

            // VRAM へのアップロード
            queue.write_texture(
                wgpu::TexelCopyTextureInfo {
                    texture: &wgpu_texture,
                    mip_level: 0,
                    origin: wgpu::Origin3d::ZERO,
                    aspect: wgpu::TextureAspect::All,
                },
                &pixels,
                wgpu::TexelCopyBufferLayout {
                    offset: 0,
                    bytes_per_row: Some(width * 4),
                    rows_per_image: Some(height),
                },
                wgpu::Extent3d {
                    width,
                    height,
                    depth_or_array_layers: 1,
                },
            );

            Ok(wgpu_texture)
        }
    }
}
