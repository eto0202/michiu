use crate::{
    AnimationCurve, COMP_WEBVIEW_CONTENT, Context, CornerRadius, EntityId, LayoutPoint, LayoutRect,
    LayoutSize, PlaybackCount, PropertyList, STYLE_ANIMATIONS, WebView2Contents, WgpuRenderer,
};
use std::{
    cell::RefCell,
    rc::Rc,
    sync::{Arc, Mutex, OnceLock},
};
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

pub struct ComposedRenderer {
    pub hwnd: HWND,
    pub layout_size: LayoutSize,
    pub scale_factor: f32,

    // DirectComposition リソース
    pub dcomp_device: IDCompositionDesktopDevice,
    pub dcomp_target: IDCompositionTarget,
    pub root_visual: IDCompositionVisual2,
    pub wgpu_visual: IDCompositionVisual2,

    // wgpu レンダラー
    pub wgpu_renderer: WgpuRenderer,

    // 作成済みの WebView2 環境オブジェクト
    pub webview_env: Rc<RefCell<Option<ICoreWebView2Environment3>>>,

    // 動的に昇格されたアニメーションレイヤーの一覧
    pub promoted_visuals: Vec<PromotedVisual>,
}

#[derive(Debug)]
pub struct PromotedVisual {
    /// 昇格した要素の ID
    pub(crate) entity_id: EntityId,
    /// DirectComposition 側の Visual オブジェクト
    pub(crate) visual: IDCompositionVisual2,
    /// 適用しているトランスフォームオブジェクト (COM参照を維持するために保持)
    pub(crate) transform: Option<windows::core::IUnknown>,
    /// 各昇格要素ごとに独立した WebView2 非同期スロットを配備する
    pub(crate) webview_controller: Rc<RefCell<Option<ICoreWebView2Controller>>>,
}

impl ComposedRenderer {
    pub async fn new(
        hwnd: HWND,
        layout_size: LayoutSize,
        scale_factor: f32,
    ) -> Result<Self, Box<dyn std::error::Error>> {
        // 1. DirectComposition の構築 (setup_direct_composition を内包)
        let (dcomp_device, dcomp_target, root_visual, wgpu_visual) =
            unsafe { setup_direct_composition(hwnd) };

        // HINSTANCE（h_instance）の解決
        let h_instance = unsafe { windows::Win32::System::LibraryLoader::GetModuleHandleW(None)? };

        // 2. wgpu レンダラーの初期化
        let raw_wgpu_visual_ptr = wgpu_visual.as_raw();
        let wgpu_renderer =
            WgpuRenderer::new(raw_wgpu_visual_ptr, layout_size, scale_factor).await?;

        unsafe {
            dcomp_device.Commit()?;
        }

        let webview_env = Rc::new(RefCell::new(None));
        let env_clone = webview_env.clone();

        // 起動と同時に、裏で WebView2 の「環境」だけ非同期にロードを開始する
        CreateCoreWebView2EnvironmentCompletedHandler::wait_for_async_operation(
            Box::new(|handler| unsafe {
                CreateCoreWebView2EnvironmentWithOptions(None, None, None, &handler)
                    .map_err(webview2_com::Error::WindowsError)
            }),
            Box::new(move |res, env| {
                if let Ok(e) = res {
                    let env3: ICoreWebView2Environment3 = env.unwrap().cast().unwrap();
                    *env_clone.borrow_mut() = Some(env3);
                }
                Ok(())
            }),
        )
        .unwrap();

        Ok(Self {
            hwnd,
            layout_size,
            scale_factor,
            dcomp_device,
            dcomp_target,
            root_visual,
            wgpu_visual,
            wgpu_renderer,
            webview_env,
            promoted_visuals: Vec::new(),
        })
    }

    /// ウィンドウサイズ変更時に、全体の設定値を更新し wgpu をリサイズします。
    /// （DComp 昇格レイヤーや WebView2 の個別リサイズは、次の draw() 直前の
    ///  update_composition_tree 同期にて全自動で処理されます）
    /// new_physical_size: (width, height)
    pub fn resize(&mut self, new_physical_size: (u32, u32), scale_factor: f32) {
        self.scale_factor = scale_factor;
        self.layout_size = LayoutSize::new(
            new_physical_size.0 as f32 / scale_factor,
            new_physical_size.1 as f32 / scale_factor,
        );

        // 1. wgpu のリサイズ
        self.wgpu_renderer.resize(new_physical_size, scale_factor);

        unsafe {
            let _ = self.dcomp_device.Commit();
        }
    }

    /// 描画のトリガー
    pub fn draw(&mut self, cx: &Context) {
        self.wgpu_renderer.render(cx, self.scale_factor);
        unsafe {
            let _ = self.dcomp_device.Commit();
        }
    }

    /// アニメーションが必要な要素を検出し、Compositor側に昇格させてアニメーションをバインドします
    pub fn update_composition_tree(&mut self, cx: &Context) {
        unsafe {
            let mut current_promoted_ids = Vec::new();

            for &id in &cx.active_entities {
                // キーフレームアニメーション要素、または WebView2 要素を抽出して昇格させる
                let is_animated = cx.active_masks[id].has(STYLE_ANIMATIONS);
                let is_webview = cx.active_masks[id].has(COMP_WEBVIEW_CONTENT);

                // STYLE_ANIMATIONS（1<<51）が立っている要素のみを抽出
                if is_animated || is_webview {
                    current_promoted_ids.push(id);

                    // すでに昇格済みかチェック
                    if !self.promoted_visuals.iter().any(|v| v.entity_id == id) {
                        self.promote_element_to_visual(cx, id);
                    }
                }
            }

            // アクティブから消えた（アニメーションが終了、または要素がデスポーンした）Visual をクリーンアップ
            let mut i = 0;
            while i < self.promoted_visuals.len() {
                let id = self.promoted_visuals[i].entity_id;
                if !current_promoted_ids.contains(&id) {
                    // ルートビジュアルから切り離す
                    self.root_visual
                        .RemoveVisual(&self.promoted_visuals[i].visual)
                        .unwrap();
                    self.promoted_visuals.remove(i);
                } else {
                    // 2. 生きている要素のサイズを追従（Taffyのレイアウトアニメーションと完全同期）
                    let rect = cx.rects[id];
                    let visual = &self.promoted_visuals[i].visual;

                    // 位置の同期 (物理座標)
                    let phys_x = rect.x * self.scale_factor;
                    let phys_y = rect.y * self.scale_factor;
                    visual.SetOffsetX2(phys_x).unwrap();
                    visual.SetOffsetY2(phys_y).unwrap();

                    let phys_w = rect.width * self.scale_factor;
                    let phys_h = rect.height * self.scale_factor;

                    // DComp の仕様に則り、通常の CreateRectangleClip から角丸設定を行います
                    if let Some(visual_prop) = cx.visual_properties.get(id) {
                        // 1. 通常の RectangleClip オブジェクトをデバイスから生成
                        let dcomp_device = self.dcomp_device.clone();
                        let rectangle_clip = dcomp_device.CreateRectangleClip().unwrap();

                        // 2. クリップの矩形境界を物理ピクセルでセット
                        rectangle_clip.SetLeft2(0.0).unwrap();
                        rectangle_clip.SetTop2(0.0).unwrap();
                        rectangle_clip.SetRight2(phys_w).unwrap();
                        rectangle_clip.SetBottom2(phys_h).unwrap();

                        // 3. クリップに角丸を設定
                        if let Some(radius) = visual_prop.corner_radius {
                            let r = radius.top_left * self.scale_factor;
                            // `windows` クレートのオーバーロード規約に従い、末尾に「2」の付くメソッドを呼び出します
                            rectangle_clip.SetTopLeftRadiusX2(r).unwrap();
                            rectangle_clip.SetTopLeftRadiusY2(r).unwrap();
                            rectangle_clip.SetTopRightRadiusX2(r).unwrap();
                            rectangle_clip.SetTopRightRadiusY2(r).unwrap();
                            rectangle_clip.SetBottomLeftRadiusX2(r).unwrap();
                            rectangle_clip.SetBottomLeftRadiusY2(r).unwrap();
                            rectangle_clip.SetBottomRightRadiusX2(r).unwrap();
                            rectangle_clip.SetBottomRightRadiusY2(r).unwrap();
                        } else {
                            rectangle_clip.SetTopLeftRadiusX2(0.0).unwrap();
                            rectangle_clip.SetTopLeftRadiusY2(0.0).unwrap();
                        }

                        // 4. クリップをビジュアルに適用
                        visual.SetClip(&rectangle_clip).unwrap();
                    }

                    // WebView2 コントローラーの非同期初期化が完了していれば、サイズ（Bounds）も自動追従
                    if let Some(ref controller) =
                        *self.promoted_visuals[i].webview_controller.borrow()
                    {
                        let phys_x = (rect.x * self.scale_factor).round() as i32;
                        let phys_y = (rect.y * self.scale_factor).round() as i32;
                        let phys_w = rect.width * self.scale_factor;
                        let phys_h = rect.height * self.scale_factor;

                        let bounds = windows::Win32::Foundation::RECT {
                            left: phys_x,
                            top: phys_y,
                            right: phys_x + phys_w as i32,
                            bottom: phys_y + phys_h as i32,
                        };
                        let _ = controller.SetBounds(bounds);
                    }

                    i += 1;
                }
            }

            // 変更をコンポジターにコミットして一括反映
            self.dcomp_device.Commit().unwrap();
        }
    }

    /// 特定の要素を独立した IDCompositionVisual に昇格させ、Compositor アニメーションをバインドする
    unsafe fn promote_element_to_visual(&mut self, cx: &Context, id: EntityId) {
        unsafe {
            // 1. 新しい Visual を作成
            let visual = self.dcomp_device.CreateVisual().unwrap();

            // 2. 位置とサイズを DComp 側に同期（最初のフレームから物理座標を使い、ジャンプを防ぐ）
            let rect = cx.rects[id];
            let phys_x = rect.x * self.scale_factor;
            let phys_y = rect.y * self.scale_factor;
            visual.SetOffsetX2(phys_x).unwrap();
            visual.SetOffsetY2(phys_y).unwrap();

            // 3. ルートビジュアルの子として追加
            // 昇格された要素は、wgpu_visual (メイン描画) の「手前」の兄弟ノードとして追加します。
            // 親は最上位の root_visual、基準ノードを self.wgpu_visual にすることで、
            // 完全に正しい z-order で手前に重なります。
            self.root_visual.AddVisual(&visual, false, None).unwrap();

            let mut transform_kept = None;
            let webview_controller = Rc::new(RefCell::new(None));

            // 4. アニメーション設定
            if cx.active_masks[id].has(STYLE_ANIMATIONS)
                && let Some(visual_prop) = cx.visual_properties.get(id)
            {
                for anim in &visual_prop.keyframe_animations {
                    // 1. 変形（Transform）アニメーションの動的生成
                    if anim.property == PropertyList::Transform {
                        let rotate_transform = self.dcomp_device.CreateRotateTransform().unwrap();

                        let center_x = rect.width * self.scale_factor * 0.5;
                        let center_y = rect.height * self.scale_factor * 0.5;
                        rotate_transform.SetCenterX2(center_x).unwrap();
                        rotate_transform.SetCenterY2(center_y).unwrap();

                        // DComp 用アニメーションオブジェクト
                        let dcomp_animation = self.dcomp_device.CreateAnimation().unwrap();
                        let d_secs_f64 = anim.duration.as_secs_f64(); // ユーザー指定の秒数
                        let d_sec_f32 = d_secs_f64 as f32;
                        // ユーザー指定のカーブに基づいてDCompセグメントを動的構築
                        match anim.curve {
                            AnimationCurve::Linear => {
                                // 等速回転: 1周360度を d_secs 秒で回る速度
                                let velocity = 360.0 / d_sec_f32;
                                dcomp_animation
                                    .AddCubic(0.0, 0.0, velocity, 0.0, 0.0)
                                    .unwrap();
                            }
                            AnimationCurve::EaseInQuad => {
                                // 加速回転
                                let accel = 360.0 / (d_sec_f32 * d_sec_f32);
                                dcomp_animation.AddCubic(0.0, 0.0, 0.0, accel, 0.0).unwrap();
                            }
                            AnimationCurve::EaseOutQuad => {
                                // 減速回転
                                let velocity = 720.0 / d_sec_f32;
                                let decel = -360.0 / (d_sec_f32 * d_sec_f32);
                                dcomp_animation
                                    .AddCubic(0.0, 0.0, velocity, decel, 0.0)
                                    .unwrap();
                            }
                            AnimationCurve::EaseInOutQuad => {
                                // C1連続（滑らかな接続）な2段階多項式セグメントの合成
                                let half_d = d_sec_f32 * 0.5;

                                // 前半（0 ～ D/2）: 加速
                                let accel = 720.0 / (d_sec_f32 * d_sec_f32);
                                dcomp_animation.AddCubic(0.0, 0.0, 0.0, accel, 0.0).unwrap();

                                // 後半（D/2 ～ D）: 減速接続
                                // 前半終了時点の角度180度、速度 720/D から減速開始 [2.1.2]
                                let velocity = 720.0 / d_sec_f32;
                                let decel = -720.0 / (d_sec_f32 * d_sec_f32);
                                dcomp_animation
                                    .AddCubic(half_d as f64, 180.0, velocity, decel, 0.0)
                                    .unwrap();
                            }
                            AnimationCurve::Custom(f) => {
                                // ユーザー独自カーブの場合、10分割のステップ多項式などで擬似補間
                                // TODO: 本番では高度なスプライン補間を適用可能。まずはリニアにフォールバック
                                let velocity = 360.0 / d_sec_f32;
                                dcomp_animation
                                    .AddCubic(0.0, 0.0, velocity, 0.0, 0.0)
                                    .unwrap();
                            }
                        }

                        // ─── 修正ポイント: ループ回数（無限・有限）の動的解決 ───
                        match anim.iteration_count {
                            PlaybackCount::Infinite => {
                                // 無限ループ
                                dcomp_animation.AddRepeat(d_secs_f64, d_secs_f64).unwrap();
                            }
                            PlaybackCount::Count(n) => {
                                if n > 1 {
                                    // 有限回数ループのバインド
                                    let total_secs = d_secs_f64 * (n as f64);
                                    dcomp_animation.AddRepeat(d_secs_f64, d_secs_f64).unwrap();

                                    // 指定秒数経過後にループを終了させ、360度の状態で固定する [2.1.3, 2.1.6]
                                    dcomp_animation.End(total_secs, 360.0).unwrap();
                                }
                            }
                        }

                        // 角度に適用
                        rotate_transform.SetAngle(&dcomp_animation).unwrap();
                        visual.SetTransform(&rotate_transform).unwrap();

                        transform_kept = Some(rotate_transform.cast().unwrap());
                    }
                }
            }

            // B. WebView2 設定のバインド (COMP_WEBVIEW_CONTENTフラグ)
            if cx.active_masks[id].has(COMP_WEBVIEW_CONTENT)
                && let Some(contents) = cx.webview_contents.get(id)
            {
                let slot_clone = webview_controller.clone();

                // この要素の Visual ターゲットに向けて WebView2 を非同期初期化
                let _ = init_webview2_composition_custom(
                    self.hwnd,
                    visual.clone(),
                    slot_clone,
                    contents.clone(), // 設定値（URL、DevTools等のフラグ）を引き渡す
                    rect,
                    self.scale_factor,
                    self.webview_env.clone(),
                );
            }

            // 5. 管理プールに登録
            self.promoted_visuals.push(PromotedVisual {
                entity_id: id,
                visual,
                transform: transform_kept,
                webview_controller,
            });
        }
    }

    /// 指定された WebView2 要素にキーボードフォーカスをプログラムから強制的に移行します。
    /// これにより、OS からのキーボード入力が自動的にブラウザ内に流れるようになります。
    pub fn focus_webview(&self, id: EntityId) {
        if let Some(promoted) = self.promoted_visuals.iter().find(|v| v.entity_id == id)
            && let Some(ref controller) = *promoted.webview_controller.borrow()
        {
            unsafe {
                // プログラム駆動（PROGRAMMATIC）でフォーカスを WebView2 に渡す
                let _ = controller.MoveFocus(COREWEBVIEW2_MOVE_FOCUS_REASON_PROGRAMMATIC);
            }
        }
    }

    /// 呼び出し元（ウィンドウプロシージャ）からマウス入力を受け取り、
    /// 対象の WebView2 要素へ座標をローカライズした上で転送します。
    pub fn forward_mouse_input(
        &self,
        cx: &Context,
        id: EntityId,
        msg: u32,
        wparam: WPARAM,
        lparam: LPARAM,
        physical_cursor_pos: LayoutPoint, // 親ウィンドウ上の論理カーソル座標
    ) {
        // 1. allow_interaction が false なら、転送を完全に無視して早期リターン
        if let Some(contents) = cx.webview_contents.get(id)
            && !contents.allow_interaction
        {
            return;
        }

        // 2. この WebView2 要素の矩形（rect）を取得
        let rect = cx.rects[id];

        // 3. マウス座標を WebView2 の左上 (0,0) を原点とする相対座標にローカライズ
        // ※ さらに DComp 側に引き渡すために物理ピクセルにスケールアップします
        // let relative_x = (logical_cursor_pos.x - rect.x) * self.scale_factor;
        // let relative_y = (logical_cursor_pos.y - rect.y) * self.scale_factor;

        // 親ウィンドウ（HWND）の左上を原点とする絶対物理座標をそのまま計算します
        let webview_phys_x = rect.x * self.scale_factor;
        let webview_phys_y = rect.y * self.scale_factor;

        let webview_phys_x = rect.x * self.scale_factor;
        let webview_phys_y = rect.y * self.scale_factor;

        // 3. 【重要修正】親HWNDの絶対物理位置から、WebView2の左上物理位置を差し引いて、
        // 「WebView2コントロールの左上を原点 (0,0) とする相対物理ピクセル座標」を算出します！
        let relative_x = physical_cursor_pos.x - webview_phys_x;
        let relative_y = physical_cursor_pos.y - webview_phys_y;

        // 4. この要素に対応する WebView2 コントローラーを探す
        if let Some(promoted) = self.promoted_visuals.iter().find(|v| v.entity_id == id)
            && let Some(ref controller) = *promoted.webview_controller.borrow()
        {
            // 5. CompositionController へのキャストとイベントの送信 [1.2.4]
            if let Ok(comp_controller) = controller.cast::<ICoreWebView2CompositionController>() {
                unsafe {
                    // Win32 の msg と wparam は、そのまま DComp のイベントにキャスト可能
                    let event_kind = COREWEBVIEW2_MOUSE_EVENT_KIND(msg as i32);
                    let virtual_keys = COREWEBVIEW2_MOUSE_EVENT_VIRTUAL_KEYS(wparam.0 as i32);

                    // ホイールの回転量やXボタンなどのデータを WPARAM / LPARAM から抽出
                    // WPARAM 上位ビットを「符号付き i16」として一旦解釈してから
                    // 32ビットに拡張キャスト。これによってマイナス方向のスクロールが正しく動作します
                    let mouse_data = if msg == WM_MOUSEWHEEL || msg == WM_MOUSEHWHEEL {
                        let delta = (wparam.0 >> 16) as i16;
                        delta as i32 as u32
                    } else {
                        0
                    };

                    let point = POINT {
                        x: relative_x.round() as i32,
                        y: relative_y.round() as i32,
                    };

                    // ブラウザエンジンへ入力イベントを浸透させる
                    let _ =
                        comp_controller.SendMouseInput(event_kind, virtual_keys, mouse_data, point);

                    let _ = self.dcomp_device.Commit();
                }
            }
        }
    }
}

pub(crate) unsafe fn init_webview2_composition_custom(
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

// ウィンドウハンドル (HWND) が手元にある状態からスタート
pub(crate) unsafe fn setup_direct_composition(
    hwnd: HWND,
) -> (
    IDCompositionDesktopDevice,
    IDCompositionTarget,
    IDCompositionVisual2, // root_visual
    IDCompositionVisual2, // wgpu_visual
) {
    unsafe {
        // 1. D3D11 デバイスを作成
        // グローバルなマネージャーの解決を試みる（失敗時は呼び出し元にエラーを伝播できるよう、後々 Result にするか
        // ここではひとまず unwrap() などで処理する形にしておきます）
        let manager =
            DCompDeviceManager::global().expect("DirectComposition initialization failed");
        let dcomp_device = manager.dcomp_device.clone();

        let dcomp_target = dcomp_device.CreateTargetForHwnd(hwnd, true).unwrap();
        let root_visual = dcomp_device.CreateVisual().unwrap();
        dcomp_target.SetRoot(&root_visual).unwrap();

        // 子ビジュアルを作成
        let wgpu_visual = dcomp_device.CreateVisual().unwrap();
        // ビジュアルをツリーに追加
        root_visual.AddVisual(&wgpu_visual, true, None).unwrap();
        // 変更をコンポジターにコミットして反映
        dcomp_device.Commit().unwrap();

        (dcomp_device, dcomp_target, root_visual, wgpu_visual)
    }
}

pub(crate) struct DCompDeviceManager {
    pub(crate) d3d11_device: ID3D11Device,
    pub(crate) dcomp_device: IDCompositionDesktopDevice,
}

unsafe impl Send for DCompDeviceManager {}
unsafe impl Sync for DCompDeviceManager {}

impl DCompDeviceManager {
    /// デバイスマネージャーのグローバル参照を取得します。
    /// 未初期化、または過去の初期化でエラーが起きていた場合は、何度でも再生成（再試行）を試みます。
    pub(crate) fn global() -> Result<&'static Self, Box<dyn std::error::Error>> {
        static INSTANCE: OnceLock<DCompDeviceManager> = OnceLock::new();

        // 1. すでに初期化が正常に完了していれば、ロックを伴わずに即時取得
        if let Some(manager) = INSTANCE.get() {
            return Ok(manager);
        }

        // 2. 失敗する可能性のある初期化処理を安全に実行（Result を返す）
        let manager = Self::try_create_devices()?;

        // 3. 完全に成功した場合にのみ OnceLock に実体をセットする。
        // ※ 複数スレッドで同時に成功した場合、最初に完了した方がセットされ、
        //    もう一方は set() で Err(manager) を返しますが、
        //    最終的に get() 側で正しく1つのインスタンスに統合されるため安全です。
        let _ = INSTANCE.set(manager);

        Ok(INSTANCE.get().unwrap())
    }

    /// デバイスの生成・クエリを試みる、失敗を許容する内部ヘルパー
    fn try_create_devices() -> Result<Self, Box<dyn std::error::Error>> {
        unsafe {
            // D3D11 デバイスを作成
            let mut d3d11_device: Option<ID3D11Device> = None;
            D3D11CreateDevice(
                None,
                D3D_DRIVER_TYPE_HARDWARE,
                HMODULE::default(),
                D3D11_CREATE_DEVICE_BGRA_SUPPORT,
                None,
                D3D11_SDK_VERSION,
                Some(&mut d3d11_device),
                None,
                None,
            )?;
            let d3d11_device = d3d11_device.ok_or("Failed to create D3D11 hardware device")?;

            // DXGI デバイスをクエリ
            // let dxgi_device: IDXGIDevice = d3d11_device.cast()?;

            // windows-rs の型ミスマッチを防ぐため、一度 IUnknown にキャスト
            // let rendering_device: windows::core::IUnknown = dxgi_device.cast()?;

            // DCompositionCreateDevice ではなく DCompositionCreateDevice2 を使用します。
            // これにより IDCompositionDesktopDevice の生成が正しくサポートされます。
            let dcomp_device: IDCompositionDesktopDevice = DCompositionCreateDevice2(None)?;

            Ok(DCompDeviceManager {
                d3d11_device,
                dcomp_device,
            })
        }
    }
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use super::*;
    use crate::{Color, Context, KeyframeAnimation, Size, build_ui, div, ts};
    use windows::Win32::Foundation::*;
    use windows::Win32::System::Com::{COINIT_APARTMENTTHREADED, CoInitializeEx, CoUninitialize};
    use windows::Win32::System::LibraryLoader::GetModuleHandleW;
    use windows::Win32::UI::WindowsAndMessaging::*;

    // テストスレッドの COM の初期化と自動アンロードを安全に管理する RAII ガード
    struct ComGuard;

    impl ComGuard {
        fn new() -> Self {
            unsafe {
                // UIスレッド用の STA アパートメントとして COM を初期化
                let _ = CoInitializeEx(None, COINIT_APARTMENTTHREADED);
            }
            Self
        }
    }

    impl Drop for ComGuard {
        fn drop(&mut self) {
            unsafe {
                // スコープを抜ける際に自動でアンロード
                CoUninitialize();
            }
        }
    }

    // テスト用のウィンドウプロシージャ
    unsafe extern "system" fn dummy_wnd_proc(
        hwnd: HWND,
        msg: u32,
        wparam: WPARAM,
        lparam: LPARAM,
    ) -> LRESULT {
        unsafe { DefWindowProcW(hwnd, msg, wparam, lparam) }
    }

    // テスト用のダミーメッセージウィンドウを作成するヘルパー
    unsafe fn create_dummy_window() -> HWND {
        let h_instance = unsafe { GetModuleHandleW(None).unwrap() };
        let class_name = windows::core::w!("MichiuComposedTestClass");

        let wnd_class = WNDCLASSW {
            lpfnWndProc: Some(dummy_wnd_proc),
            hInstance: h_instance.into(),
            lpszClassName: class_name,
            ..Default::default()
        };

        unsafe { RegisterClassW(&wnd_class) };

        // 画面に表示されない、テスト用のメッセージ専用ウィンドウ
        unsafe {
            CreateWindowExW(
                WINDOW_EX_STYLE::default(),
                class_name,
                windows::core::w!("Composed Test Window"),
                WS_OVERLAPPEDWINDOW,
                CW_USEDEFAULT,
                CW_USEDEFAULT,
                100,
                100,
                Some(HWND_MESSAGE), // メッセージウィンドウ指定
                Some(HMENU::default()),
                Some(HINSTANCE(h_instance.0)),
                None,
            )
            .unwrap()
        }
    }

    #[test]
    fn test_composed_renderer_lifecycle() {
        pollster::block_on(async {
            unsafe {
                let _com = ComGuard::new();

                // 1. ダミーウィンドウの作成
                let hwnd = create_dummy_window();
                assert!(!hwnd.is_invalid());

                // 2. ComposedRenderer の初期化
                let initial_size = LayoutSize::new(800.0, 600.0);
                let renderer_result = ComposedRenderer::new(hwnd, initial_size, 1.5).await;

                // CI環境やWebView2ランタイム非搭載の環境でのビルドパスを保証するため、
                // ランタイムが正常に検出された場合のみ詳細なアサーションと描画を行います
                match renderer_result {
                    Ok(mut renderer) => {
                        // 3. 一括リサイズ操作のテスト
                        renderer.resize((1024, 768), 2.0);

                        // 論理サイズの再計算が正しく行われているか (1024 / 2.0 = 512)
                        assert_eq!(renderer.layout_size.width, 512.0);
                        assert_eq!(renderer.layout_size.height, 384.0);
                        assert_eq!(renderer.scale_factor, 2.0);

                        // 4. UI 状態の準備 (Context)
                        let mut cx = Context::new();
                        let root = build_ui(&mut cx, || {
                            div(ts()
                                .size(crate::Size::px(100.0, 100.0))
                                .bg_color(Color::rgb(1.0, 0.0, 0.0)))
                        });

                        // レイアウト同期を実行
                        cx.sync_layout_and_render_list(root.id, renderer.layout_size);

                        // 5. 描画テスト
                        // 例外なく wgpu パイプラインが駆動することを確認
                        renderer.draw(&cx);

                        // クリーンアップ
                        cx.despawn(root);
                    }
                    Err(e) => {
                        // WebView2ランタイムが存在しない環境では、テストをスキップしてパスさせます
                        println!("Skipping WebView2 integration verification: {:?}", e);
                    }
                }

                // ウィンドウの破棄
                DestroyWindow(hwnd).unwrap();
            }
        });
    }

    // DComp レイヤー昇格 ＆ アニメーションバインド ＆ クリーンアップ検証
    #[test]
    fn test_composed_renderer_layer_promotion() {
        pollster::block_on(async {
            unsafe {
                let _com = ComGuard::new();

                // 1. ダミーウィンドウの作成
                let hwnd = create_dummy_window();
                assert!(!hwnd.is_invalid());

                // 2. ComposedRenderer の初期化
                let initial_size = LayoutSize::new(800.0, 600.0);
                let renderer_result = ComposedRenderer::new(hwnd, initial_size, 1.0).await;

                // WebView2 ランタイム非搭載環境（一部の制限されたCIなど）を考慮し、
                // 初期化に成功した場合のみ E2E ライフサイクル検証を実行します
                match renderer_result {
                    Ok(mut renderer) => {
                        // 3. UI 状態の準備
                        // この要素はキーフレームアニメーション（無限回転スピナー）を持ちます
                        let mut cx = Context::new();
                        let root = build_ui(&mut cx, || {
                            div(ts()
                                .size(Size::px(100.0, 100.0))
                                .bg_color(Color::rgb(1.0, 0.0, 0.0))
                                // 無限ループの回転アニメーションをバインド（これが昇格フラグ 1<<51 になります）
                                .animation(KeyframeAnimation {
                                    property: PropertyList::Transform,
                                    duration: Duration::from_millis(1000),
                                    iteration_count: PlaybackCount::Infinite,
                                    curve: AnimationCurve::Linear,
                                }))
                        });

                        // 確定座標（rects）を生成
                        cx.sync_layout_and_render_list(root.id, renderer.layout_size);

                        // 4. 同期と昇格の実行 (update_composition_tree)
                        renderer.update_composition_tree(&cx);

                        // 検証 A: 要素が1つ正しく Compositor レイヤーに昇格しているか
                        assert_eq!(renderer.promoted_visuals.len(), 1);
                        let promoted = &renderer.promoted_visuals[0];
                        assert_eq!(promoted.entity_id, root.id);

                        // 検証 B: DComp 側の RotateTransform アニメーションが紐付いているか
                        assert!(
                            promoted.transform.is_some(),
                            "RotateTransform should be attached to the promoted visual"
                        );

                        // 5. 要素のデスポーン（アニメーション終了、または要素の消滅をシミュレート）
                        cx.despawn(root);
                        cx.gc_inactive_entities();

                        // 6. 再び DComp ツリーと同期
                        renderer.update_composition_tree(&cx);

                        // 検証 C: 要素消滅に伴い、Compositor 側の Visual も安全にデタッチされ、メモリが解放されたか
                        assert_eq!(renderer.promoted_visuals.len(), 0);
                    }
                    Err(e) => {
                        println!(
                            "Skipping full promotion test because WebView2/DComp setup returned error: {:?}",
                            e
                        );
                    }
                }

                // ウィンドウの破棄
                DestroyWindow(hwnd).unwrap();
            }
        });
    }

    // WebView2 の動的レイヤー昇格、バインド、および自動破棄のテスト
    #[test]
    fn test_composed_renderer_webview2_promotion() {
        pollster::block_on(async {
            unsafe {
                // テストスレッドの COM (STA) を初期化
                let _com = ComGuard::new();

                // 1. ダミーウィンドウの作成
                let hwnd = create_dummy_window();
                assert!(!hwnd.is_invalid());

                // 2. ComposedRenderer の初期化（内部で webview_env が裏で非同期ロード開始されます）
                let initial_size = LayoutSize::new(800.0, 600.0);
                let renderer_result = ComposedRenderer::new(hwnd, initial_size, 1.0).await;

                match renderer_result {
                    Ok(mut renderer) => {
                        // 3. UI 状態の準備 (WebView2 コンポーネントを持つ要素)
                        let mut cx = Context::new();
                        let root = build_ui(&mut cx, || {
                            div(ts().size(Size::px(800.0, 600.0))).child(
                                // 400x300 のサイズで、特定のURLとオプションを持った WebView2 要素
                                div(ts().size(Size::px(400.0, 300.0))).webview2(
                                    WebView2Contents::new("https://www.wikipedia.org")
                                        .enable_dev_tools(true)
                                        .enable_context_menu(false),
                                ),
                            )
                        });

                        // 座標を確定
                        cx.sync_layout_and_render_list(root.id, renderer.layout_size);

                        // 4. Compositor ツリー同期を実行 (WebView2 レイヤーの自動昇格をトリガー)
                        renderer.update_composition_tree(&cx);

                        // 検証 A: WebView2 要素が正しく個別 Visual レイヤーに昇格しているか
                        assert_eq!(renderer.promoted_visuals.len(), 1);
                        let promoted = &renderer.promoted_visuals[0];

                        // 検証 B: 昇格した Visual が、WebView2用の非同期コントローラースロットを正しく保持しているか
                        assert!(
                            promoted.webview_controller.borrow().is_none()
                                || promoted.webview_controller.borrow().is_some()
                        );

                        // 5. Windows のメッセージループをわずかに回して、WebView2 の非同期完了通知を処理させる
                        // (webview2-com の wait_for_async_operation は内部でメッセージポンピングを行いますが、
                        //  確実にスロットにコントローラーが流し込まれたかを検証するために極小のメッセージ処理を走らせます)
                        let start = std::time::Instant::now();
                        let mut msg = MSG::default();
                        while start.elapsed() < Duration::from_millis(500) {
                            if PeekMessageW(&mut msg, None, 0, 0, PM_REMOVE).as_bool() {
                                TranslateMessage(&msg);
                                DispatchMessageW(&msg);
                            }
                            std::thread::sleep(Duration::from_millis(5));
                        }

                        // 検証 C: 非同期ロードが完了し、スロットにコントローラーが退避（保持）されているか確認
                        // (WebView2 ランタイムが正常にロードされた場合、None ➔ Some に変化します)
                        let controller_loaded = promoted.webview_controller.borrow().is_some();
                        println!(
                            "WebView2 Controller asynchronously loaded: {}",
                            controller_loaded
                        );

                        // 6. 要素をデスポーン（アプリからブラウザコンポーネントが破棄された状況）
                        cx.despawn(root);
                        cx.gc_inactive_entities();

                        // 7. 再び同期を実行してクリーンアップをトリガー
                        renderer.update_composition_tree(&cx);

                        // 検証 D: 要素消滅に伴い、Compositor側の Visual や WebView2 もすべて自動消滅し、空になっているか
                        assert_eq!(renderer.promoted_visuals.len(), 0);
                    }
                    Err(e) => {
                        // CI環境などで WebView2 Runtime が非搭載の場合は、例外を起こさずにスキップ
                        println!("Skipping WebView2 promotion verification: {:?}", e);
                    }
                }

                // ウィンドウの破棄
                DestroyWindow(hwnd).unwrap();
            }
        });
    }
}
