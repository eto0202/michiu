use std::{cell::RefCell, rc::Rc};

use webview2_com::{
    AddScriptToExecuteOnDocumentCreatedCompletedHandler,
    CreateCoreWebView2CompositionControllerCompletedHandler,
    CreateCoreWebView2EnvironmentCompletedHandler,
    Microsoft::Web::WebView2::Win32::{
        COREWEBVIEW2_COLOR, COREWEBVIEW2_MOUSE_EVENT_KIND, COREWEBVIEW2_MOUSE_EVENT_VIRTUAL_KEYS,
        COREWEBVIEW2_MOVE_FOCUS_REASON, COREWEBVIEW2_MOVE_FOCUS_REASON_PREVIOUS,
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
            DirectComposition::{IDCompositionVisual, IDCompositionVisual2},
            Dxgi::*,
            Gdi::InvalidateRect,
        },
        UI::WindowsAndMessaging::{WM_MOUSEHWHEEL, WM_MOUSEWHEEL},
    },
    core::{Interface, PCWSTR, PWSTR, w},
};
use windows_core::HSTRING;

use crate::{LayoutRect, MichiuError, TaskSender, WebView2Contents, WebView2Source};

#[allow(clippy::too_many_arguments)]
pub(crate) unsafe fn init_webview2_composition(
    hwnd: HWND,
    webview_visual: &IDCompositionVisual2,
    controller_slot: &Rc<RefCell<Option<ICoreWebView2Controller>>>,
    settings: &WebView2Contents,
    rect: LayoutRect,
    scale_factor: f32,
    env_slot: &Rc<RefCell<Option<ICoreWebView2Environment3>>>,
    sys_task_sender: &TaskSender,
) -> crate::Result<()> {
    let webview_visual_clone = webview_visual.clone();
    let controller_slot_clone = controller_slot.clone();
    let settings_clone = settings.clone();
    let task_sender_clone = sys_task_sender.clone();

    // プリウォーム済み環境（Environment）の利用
    if let Some(ref env3) = *env_slot.borrow() {
        let env3_clone = env3.clone();

        let handler = webview2_com::CreateCoreWebView2CompositionControllerCompletedHandler::create(
            Box::new(
                move |res, controller: Option<ICoreWebView2CompositionController>| {
                    // このクロージャは後から非同期にブラウザの初期化が終わった瞬間に実行。
                    res.map_err(webview2_com::Error::WindowsError);

                    let comp_controller =
                        controller.ok_or_else(windows_core::Error::from_thread)?;
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

                    // キーボードフォーカス奪還ハンドラをバインド
                    let task_sender_inner = task_sender_clone.clone();
                    let focus_handler = webview2_com::MoveFocusRequestedEventHandler::create(
                        Box::new(move |_sender, args| {
                            if let Some(args) = args {
                                let mut reason = COREWEBVIEW2_MOVE_FOCUS_REASON(0);
                                let _ = unsafe { args.Reason(&raw mut reason) };

                                // Tab なら 順順移動(false), Shift+Tab なら 逆順移動(true)
                                let is_reverse = reason == COREWEBVIEW2_MOVE_FOCUS_REASON_PREVIOUS;

                                // メインスレッドの Context にフォーカス循環要求をディスパッチ
                                let _ = task_sender_inner.send(move |cx| {
                                    cx.cycle_keyboard_focus(is_reverse);
                                });

                                // WebView2側に「ホストアプリがフォーカスを奪還した」ことを通知
                                let _ = unsafe { args.SetHandled(true) };
                            }
                            Ok(())
                        }),
                    );
                    unsafe {
                        base_controller.add_MoveFocusRequested(&focus_handler, &mut 0)?;
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
                            WebView2Source::Html(_) => webview.NavigateToString(&source_string)?,
                        }
                    }

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

    // 初期起動時（env_slot がまだロード途中）の場合のフォールバック
    let env_slot_clone = env_slot.clone();
    let handler = webview2_com::CreateCoreWebView2EnvironmentCompletedHandler::create(Box::new(
        move |res, environment: Option<ICoreWebView2Environment>| {
            res.map_err(webview2_com::Error::WindowsError);
            let env = environment.ok_or_else(windows_core::Error::from_thread)?;
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
                            let comp_controller =
                                controller.ok_or_else(windows_core::Error::from_thread)?;
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

                            let controller_slot_inner = controller_slot_clone.clone();
                            let base_controller_inner = base_controller.clone();
                            let nav_handler = webview2_com::NavigationCompletedEventHandler::create(
                                Box::new(move |_sender, _args| {
                                    *controller_slot_inner.borrow_mut() =
                                        Some(base_controller_inner.clone());

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
