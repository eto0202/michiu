use std::{cell::RefCell, rc::Rc};

use webview2_com::{
    AddScriptToExecuteOnDocumentCreatedCompletedHandler,
    CreateCoreWebView2CompositionControllerCompletedHandler,
    CreateCoreWebView2EnvironmentCompletedHandler,
    Microsoft::Web::WebView2::Win32::{
        COREWEBVIEW2_COLOR, COREWEBVIEW2_MOUSE_EVENT_KIND, COREWEBVIEW2_MOUSE_EVENT_VIRTUAL_KEYS,
        COREWEBVIEW2_MOVE_FOCUS_REASON_PROGRAMMATIC, CreateCoreWebView2EnvironmentWithOptions,
        ICoreWebView2CompositionController, ICoreWebView2Controller, ICoreWebView2Controller2,
        ICoreWebView2Environment, ICoreWebView2Environment3,
    },
};
use windows::{
    Win32::{
        Foundation::{HMODULE, HWND, LPARAM, POINT, RECT, WPARAM},
        Graphics::{
            Direct3D::*,
            Direct3D11::*,
            DirectComposition::{IDCompositionVisual, *},
            Dxgi::*,
        },
        UI::WindowsAndMessaging::{WM_MOUSEHWHEEL, WM_MOUSEWHEEL},
    },
    core::{Interface, PCWSTR, PWSTR, w},
};

use crate::{LayoutRect, WebView2Contents};

pub(crate) unsafe fn init_webview2_composition(
    hwnd: HWND,
    webview_visual: IDCompositionVisual2,
    controller_slot: Rc<RefCell<Option<ICoreWebView2Controller>>>,
    settings: WebView2Contents,
    rect: LayoutRect,
    scale_factor: f32,
    env_slot: Rc<RefCell<Option<ICoreWebView2Environment3>>>, // 引数を追加
) -> Result<(), Box<dyn std::error::Error>> {
    let webview_visual_clone = webview_visual.clone();
    let controller_slot_clone = controller_slot.clone();
    let settings_clone = settings.clone();

    // プリウォーム済み環境（Environment）の利用
    if let Some(ref env3) = *env_slot.borrow() {
        let env3_clone = env3.clone();

        // 【最重要修正】wait_for_async_operation（メッセージポンプ同期待機）を完全に排除
        // handler の作成のみを登録して、即座に関数をリターンさせます。
        let handler = webview2_com::CreateCoreWebView2CompositionControllerCompletedHandler::create(
            Box::new(
                move |res, controller: Option<ICoreWebView2CompositionController>| {
                    // このクロージャは、後から非同期にブラウザの初期化が終わった瞬間に実行されます。
                    res.map_err(webview2_com::Error::WindowsError);

                    let comp_controller = controller.unwrap();
                    unsafe { comp_controller.SetRootVisualTarget(&webview_visual_clone) }?;

                    let base_controller: ICoreWebView2Controller = comp_controller.cast()?;

                    let phys_w = (rect.width * scale_factor).round() as i32;
                    let phys_h = (rect.height * scale_factor).round() as i32;

                    // 位置（left, top）は DComp 側に一任するため 0 に設定
                    let bounds = RECT {
                        left: 0,
                        top: 0,
                        right: phys_w,
                        bottom: phys_h,
                    };
                    unsafe {
                        base_controller.SetBounds(bounds)?;
                        base_controller.SetIsVisible(true)?;
                    }

                    // DComp 側での WebView2 背景色の初期設定
                    // ページロード前に真っ白（または真っ黒）にチラつくのを完全に防ぎます
                    if let Ok(controller2) = base_controller.cast::<ICoreWebView2Controller2>() {
                        // ARGB 形式で親コンテナと同じ色 (0.08, 0.08, 0.12) を透過度 255 (不透明) でセット
                        let dcomp_bg_color = COREWEBVIEW2_COLOR {
                            A: 255,
                            R: 20,
                            G: 20,
                            B: 30,
                        };
                        unsafe {
                            let _ = controller2.SetDefaultBackgroundColor(dcomp_bg_color);
                        }
                    }

                    let webview = unsafe { base_controller.CoreWebView2()? };
                    let web_settings = unsafe { webview.Settings()? };

                    unsafe {
                        let _ = web_settings.SetIsScriptEnabled(settings_clone.enable_scripts);
                        let _ = web_settings.SetAreDevToolsEnabled(settings_clone.enable_dev_tools);
                        let _ = web_settings
                            .SetAreDefaultContextMenusEnabled(settings_clone.enable_context_menu);
                    }

                    // ユーザースクリプト登録も非同期で安全に登録
                    for script in &settings_clone.user_scripts {
                        let script_u16: Vec<u16> = script.encode_utf16().chain(Some(0)).collect();
                        let pcw_script = PCWSTR(script_u16.as_ptr());
                        let webview_clone = webview.clone();

                        let script_handler = webview2_com::AddScriptToExecuteOnDocumentCreatedCompletedHandler::create(
                            Box::new(|res, _id| {
                                res.map_err(webview2_com::Error::WindowsError);
                                Ok(())
                            })
                        );
                        unsafe {
                            webview_clone
                                .AddScriptToExecuteOnDocumentCreated(pcw_script, &script_handler)?;
                        }
                    }

                    let url_u16: Vec<u16> =
                        settings_clone.url.encode_utf16().chain(Some(0)).collect();
                    unsafe {
                        webview.Navigate(PCWSTR(url_u16.as_ptr()))?;
                    }

                    *controller_slot_clone.borrow_mut() = Some(base_controller);
                    Ok(())
                },
            ),
        );

        // 非同期処理をキックして即時リターン（ノンブロッキング）
        unsafe {
            env3_clone.CreateCoreWebView2CompositionController(hwnd, &handler)?;
        }

        return Ok(());
    }

    // 万が一初期起動時（env_slot がまだロード途中）の場合のフォールバック
    // こちらも同様に非同期コールバックのみで繋ぐように修正します
    let env_slot_clone = env_slot.clone();
    let handler = webview2_com::CreateCoreWebView2EnvironmentCompletedHandler::create(Box::new(
        move |res, environment: Option<ICoreWebView2Environment>| {
            res.map_err(webview2_com::Error::WindowsError);
            let env = environment.unwrap();
            let env3: ICoreWebView2Environment3 = env.cast()?;

            // 環境スロットにキャッシュを保存
            *env_slot_clone.borrow_mut() = Some(env3.clone());

            let webview_visual_clone2 = webview_visual_clone.clone();
            let controller_slot_clone2 = controller_slot_clone.clone();
            let settings_clone2 = settings_clone.clone();

            let controller_handler =
                webview2_com::CreateCoreWebView2CompositionControllerCompletedHandler::create(
                    Box::new(
                        move |res, controller: Option<ICoreWebView2CompositionController>| {
                            res.map_err(webview2_com::Error::WindowsError);
                            let comp_controller = controller.unwrap();
                            unsafe { comp_controller.SetRootVisualTarget(&webview_visual_clone2) }?;

                            let base_controller: ICoreWebView2Controller =
                                comp_controller.cast()?;

                            let phys_w = (rect.width * scale_factor).round() as i32;
                            let phys_h = (rect.height * scale_factor).round() as i32;

                            let bounds = RECT {
                                left: 0,
                                top: 0,
                                right: phys_w,
                                bottom: phys_h,
                            };
                            unsafe {
                                base_controller.SetBounds(bounds)?;
                                base_controller.SetIsVisible(true)?;
                            }

                            if let Ok(controller2) =
                                base_controller.cast::<ICoreWebView2Controller2>()
                            {
                                // ARGB 形式で親コンテナと同じ色 (0.08, 0.08, 0.12) を透過度 255 (不透明) でセット
                                let dcomp_bg_color = COREWEBVIEW2_COLOR {
                                    A: 255,
                                    R: 20,
                                    G: 20,
                                    B: 30,
                                };
                                unsafe {
                                    let _ = controller2.SetDefaultBackgroundColor(dcomp_bg_color);
                                }
                            }

                            let webview = unsafe { base_controller.CoreWebView2()? };
                            let url_u16: Vec<u16> =
                                settings_clone2.url.encode_utf16().chain(Some(0)).collect();
                            unsafe {
                                webview.Navigate(PCWSTR(url_u16.as_ptr()))?;
                            }

                            *controller_slot_clone2.borrow_mut() = Some(base_controller);
                            Ok(())
                        },
                    ),
                );

            unsafe {
                env3.CreateCoreWebView2CompositionController(hwnd, &controller_handler)?;
            }

            Ok(())
        },
    ));

    unsafe {
        CreateCoreWebView2EnvironmentWithOptions(None, None, None, &handler)?;
    }

    Ok(())
}
