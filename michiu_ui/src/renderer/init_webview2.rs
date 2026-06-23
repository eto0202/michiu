use std::{cell::RefCell, rc::Rc};

use webview2_com::{
    AddScriptToExecuteOnDocumentCreatedCompletedHandler,
    CreateCoreWebView2CompositionControllerCompletedHandler,
    CreateCoreWebView2EnvironmentCompletedHandler,
    Microsoft::Web::WebView2::Win32::{
        COREWEBVIEW2_MOUSE_EVENT_KIND, COREWEBVIEW2_MOUSE_EVENT_VIRTUAL_KEYS,
        COREWEBVIEW2_MOVE_FOCUS_REASON_PROGRAMMATIC, CreateCoreWebView2EnvironmentWithOptions,
        ICoreWebView2CompositionController, ICoreWebView2Controller, ICoreWebView2Environment,
        ICoreWebView2Environment3,
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
        // すでに起動時に環境のロードが完了している場合
        let env3_clone = env3.clone();

        CreateCoreWebView2CompositionControllerCompletedHandler::wait_for_async_operation(
            Box::new(move |handler| unsafe {
                env3_clone
                    .CreateCoreWebView2CompositionController(hwnd, &handler)
                    .map_err(webview2_com::Error::WindowsError)
            }),
            Box::new(
                move |res, controller: Option<ICoreWebView2CompositionController>| {
                    res?;
                    let comp_controller = controller.unwrap();
                    unsafe { comp_controller.SetRootVisualTarget(&webview_visual_clone) }?;

                    let base_controller: ICoreWebView2Controller = comp_controller.cast()?;

                    let phys_x = (rect.x * scale_factor).round() as i32;
                    let phys_y = (rect.y * scale_factor).round() as i32;
                    let phys_w = (rect.width * scale_factor).round() as i32;
                    let phys_h = (rect.height * scale_factor).round() as i32;

                    let bounds = RECT {
                        left: phys_x,
                        top: phys_y,
                        right: phys_x + phys_w,
                        bottom: phys_y + phys_h,
                    };
                    unsafe {
                        base_controller.SetBounds(bounds)?;
                        base_controller.SetIsVisible(true)?;
                    }

                    let webview = unsafe { base_controller.CoreWebView2()? };
                    let web_settings = unsafe { webview.Settings()? };

                    unsafe {
                        let _ = web_settings.SetIsScriptEnabled(settings_clone.enable_scripts);
                        let _ = web_settings.SetAreDevToolsEnabled(settings_clone.enable_dev_tools);
                        let _ = web_settings
                            .SetAreDefaultContextMenusEnabled(settings_clone.enable_context_menu);
                    }

                    for script in &settings_clone.user_scripts {
                        let script_u16: Vec<u16> = script.encode_utf16().chain(Some(0)).collect();
                        let pcw_script = PCWSTR(script_u16.as_ptr());
                        let webview_clone = webview.clone();

                        // 2. WebView2にドキュメント生成（ロード）時に自動実行するスクリプトとして登録
                        AddScriptToExecuteOnDocumentCreatedCompletedHandler::wait_for_async_operation(
                                Box::new(move |handler| unsafe {
                                    // 非同期でスクリプトを追加
                                    webview_clone.AddScriptToExecuteOnDocumentCreated(pcw_script, &handler)
                                        .map_err(webview2_com::Error::WindowsError)
                                }),
                                Box::new(|res, _id| {
                                    res?; // 登録完了エラーチェック
                                    Ok(())
                                })
                            ).unwrap();
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
        )?;

        return Ok(());
    }

    // 万が一起動直後で環境ロードがまだ終わっていない場合
    // 環境の作成から順に非同期で行う
    CreateCoreWebView2EnvironmentCompletedHandler::wait_for_async_operation(
        Box::new(|handler| unsafe {
            CreateCoreWebView2EnvironmentWithOptions(None, None, None, &handler)
                .map_err(webview2_com::Error::WindowsError)
        }),
        Box::new(move |res, environment: Option<ICoreWebView2Environment>| {
            res?;
            let env = environment.unwrap();
            let env3: ICoreWebView2Environment3 = env.cast()?;
            let webview_visual_clone2 = webview_visual_clone.clone();
            let controller_slot_clone2 = controller_slot_clone.clone();
            let settings_clone2 = settings_clone.clone();

            CreateCoreWebView2CompositionControllerCompletedHandler::wait_for_async_operation(
                Box::new(move |handler| unsafe {
                    env3.CreateCoreWebView2CompositionController(hwnd, &handler)
                        .map_err(webview2_com::Error::WindowsError)
                }),
                Box::new(
                    move |res, controller: Option<ICoreWebView2CompositionController>| {
                        res?;
                        let comp_controller = controller.unwrap();
                        unsafe { comp_controller.SetRootVisualTarget(&webview_visual_clone2) }?;

                        let base_controller: ICoreWebView2Controller = comp_controller.cast()?;

                        let phys_x = (rect.x * scale_factor).round() as i32;
                        let phys_y = (rect.y * scale_factor).round() as i32;
                        let phys_w = (rect.width * scale_factor).round() as i32;
                        let phys_h = (rect.height * scale_factor).round() as i32;

                        let bounds = RECT {
                            left: phys_x,
                            top: phys_y,
                            right: phys_x + phys_w,
                            bottom: phys_y + phys_h,
                        };
                        unsafe {
                            base_controller.SetBounds(bounds)?;
                            base_controller.SetIsVisible(true)?;
                        }

                        let webview = unsafe { base_controller.CoreWebView2()? };
                        let web_settings = unsafe { webview.Settings()? };

                        unsafe {
                            let _ = web_settings.SetIsScriptEnabled(settings_clone2.enable_scripts);
                            let _ = web_settings
                                .SetAreDevToolsEnabled(settings_clone2.enable_dev_tools);
                            let _ = web_settings.SetAreDefaultContextMenusEnabled(
                                settings_clone2.enable_context_menu,
                            );
                        }

                        for script in &settings_clone2.user_scripts {
                            let script_u16: Vec<u16> =
                                script.encode_utf16().chain(Some(0)).collect();
                        }

                        let url_u16: Vec<u16> =
                            settings_clone2.url.encode_utf16().chain(Some(0)).collect();
                        unsafe {
                            webview.Navigate(PCWSTR(url_u16.as_ptr()))?;
                        }

                        *controller_slot_clone2.borrow_mut() = Some(base_controller);

                        Ok(())
                    },
                ),
            )
            .unwrap();

            Ok(())
        }),
    )?;

    Ok(())
}
