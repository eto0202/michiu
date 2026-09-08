use crate::{
    AnimationCurve, Backdrop, ComponentMask, Context, CornerRadius, EntityId, LayoutPoint,
    LayoutRect, LayoutSize, MichiuSoA, PlaybackCount, PropertyList, WebView2Contents, WgpuRenderer,
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
            Direct3D::{D3D_DRIVER_TYPE_HARDWARE, ID3DInclude_Impl},
            Direct3D11::{
                D3D11_CREATE_DEVICE_BGRA_SUPPORT, D3D11_SDK_VERSION, D3D11CreateDevice,
                ID3D11Device,
            },
            DirectComposition::{
                DCompositionCreateDevice2, IDCompositionDesktopDevice,
                IDCompositionDesktopDevice_Impl, IDCompositionDevice_Impl,
                IDCompositionDevice2_Impl, IDCompositionRectangleClip_Impl, IDCompositionTarget,
                IDCompositionTarget_Impl, IDCompositionTranslateTransform_Impl,
                IDCompositionTranslateTransform3D_Impl, IDCompositionVisual,
                IDCompositionVisual_Impl, IDCompositionVisual2, IDCompositionVisual3_Impl,
            },
            Dxgi::*,
            Gdi::InvalidateRect,
            Imaging::{CLSID_WICImagingFactory, IWICImagingFactory},
        },
        System::Com::{CLSCTX_INPROC_SERVER, CoCreateInstance},
        UI::WindowsAndMessaging::{
            GWL_EXSTYLE, GetWindowLongW, SetWindowLongW, WM_MOUSEHWHEEL, WM_MOUSEWHEEL,
        },
    },
    core::{Interface, PCWSTR, PWSTR, w},
};
use windows_numerics::Matrix3x2;

pub struct ComposedRenderer {
    pub hwnd: HWND,
    pub layout_size: LayoutSize,
    pub scale_factor: f32,

    pub wic_factory: IWICImagingFactory,

    /// `DirectComposition` リソース
    pub dcomp_device: IDCompositionDesktopDevice,
    pub dcomp_target: IDCompositionTarget,
    pub root_visual: IDCompositionVisual2,
    /// wgpu 用のビジュアル
    pub wgpu_visual: IDCompositionVisual2,

    /// wgpu レンダラー
    pub wgpu_renderer: WgpuRenderer,

    /// 作成済みの `WebView2` 環境オブジェクト
    pub webview_env: Rc<RefCell<Option<ICoreWebView2Environment3>>>,

    /// 動的に昇格された `WebView2` レイヤーの一覧
    pub promoted_visuals: Vec<PromotedVisual>,

    /// 非同期でキャプチャデコードが完了し、正式に削除（DComp解放）可能になった ID の待ちバッファ
    #[allow(clippy::type_complexity)]
    pub(crate) pending_removals: Rc<RefCell<Vec<(EntityId, Option<wgpu::Texture>)>>>,
    /// `DComp` 側の Visual 削除を wgpu のピクセル定着から数フレーム遅延させるためのキュー
    pub(crate) pending_dcomp_releases: Vec<PendingDcompRelease>,

    /// 現在ウィンドウに適用中のバックドロップ状態
    pub(crate) current_backdrop: Backdrop,

    /// リサイズが完全に安定するまでキャプチャを保留するカウンター
    pub(crate) resize_cooldown_frames: u32,
}

pub(crate) struct PendingDcompRelease {
    pub(crate) entity_id: EntityId,
    pub(crate) frames_left: u32,
}

#[derive(Debug)]
pub struct PromotedVisual {
    /// 昇格した要素の ID
    pub(crate) entity_id: EntityId,
    /// `DirectComposition` 側の Visual オブジェクト
    pub(crate) visual: IDCompositionVisual2,
    /// 適用しているトランスフォームオブジェクト (COM参照を維持するために保持)
    pub(crate) transform: Option<windows::core::IUnknown>,
    /// 各昇格要素ごとに独立した `WebView2` 非同期スロットを配備する
    pub(crate) webview_controller: Rc<RefCell<Option<ICoreWebView2Controller>>>,
    /// 現在バックグラウンドで非同期キャプチャ（スナップショット）を実行中かどうかのフラグ
    pub is_capturing: bool,
    // DCompツリーにマウントされており、コントローラーが可視状態であるか
    pub(crate) is_visible: bool,
}

impl ComposedRenderer {
    pub async fn new(
        hwnd: HWND,
        layout_size: LayoutSize,
        scale_factor: f32,
    ) -> Result<Self, Box<dyn std::error::Error>> {
        // DirectComposition の構築 (setup_direct_composition を内包)
        let (dcomp_device, dcomp_target, root_visual, wgpu_visual) =
            ComposedRenderer::setup_direct_composition(hwnd)?;

        // HINSTANCE（h_instance）の解決
        let h_instance = unsafe { windows::Win32::System::LibraryLoader::GetModuleHandleW(None)? };

        // 2. wgpu レンダラーの初期化
        let raw_visual_ptr = wgpu_visual.as_raw();

        let wgpu_renderer = WgpuRenderer::new(raw_visual_ptr, layout_size, scale_factor).await?;

        unsafe {
            dcomp_device.Commit()?;
        }

        let webview_env = Rc::new(RefCell::new(None));

        let wic_factory: IWICImagingFactory = unsafe {
            CoCreateInstance(&CLSID_WICImagingFactory, None, CLSCTX_INPROC_SERVER).unwrap()
        };

        Ok(Self {
            hwnd,
            layout_size,
            scale_factor,
            wic_factory,
            dcomp_device,
            dcomp_target,
            root_visual,
            wgpu_visual,
            wgpu_renderer,
            webview_env,
            promoted_visuals: Vec::new(),
            pending_removals: Rc::new(RefCell::new(Vec::new())),
            pending_dcomp_releases: Vec::new(),
            current_backdrop: Backdrop::None,
            resize_cooldown_frames: 0,
        })
    }

    /// `WebView2` の環境（Environment）をバックグラウンドで事前ロードし、
    /// `後続のWebView2マウント時における初期化ラグを削減します`。
    ///
    /// 本メソッドは、OS に `WebView2` ランタイムがインストールされていない場合、
    /// クラッシュを発生させずに処理を自動スキップ（サイレントフォールバック）します。
    pub fn prewarm_webview2(&self) {
        // 多重プリウォームロードを防止
        if self.webview_env.borrow().is_some() {
            return;
        }

        let env_clone = self.webview_env.clone();

        unsafe {
            // パニック（unwrap）を起こさずに処理を実行
            let _ = CreateCoreWebView2EnvironmentCompletedHandler::wait_for_async_operation(
                Box::new(|handler| unsafe {
                    CreateCoreWebView2EnvironmentWithOptions(None, None, None, &handler)
                        .map_err(webview2_com::Error::WindowsError)
                }),
                Box::new(move |res, env| {
                    if let Ok(()) = res
                        && let Some(e_ptr) = env
                        && let Ok(env3) = e_ptr.cast::<ICoreWebView2Environment3>()
                    {
                        // 環境オブジェクトを書き換えて、後続のマウント処理でキャッシュが再利用されるようにする
                        *env_clone.borrow_mut() = Some(env3);
                    }

                    Ok(())
                }),
            );
        }
    }

    /// ウィンドウサイズ変更時に、全体の設定値を更新し wgpu をリサイズします。
    /// （`DComp` 昇格レイヤーや `WebView2` の個別リサイズは、次の `draw()` 直前の
    ///  `update_composition_tree` 同期にて全自動で処理されます）
    /// `new_physical_size`: (width, height)
    pub fn resize(&mut self, new_physical_size: (u32, u32), scale_factor: f32) {
        self.scale_factor = scale_factor;
        self.layout_size = LayoutSize::new(
            new_physical_size.0 as f32 / scale_factor,
            new_physical_size.1 as f32 / scale_factor,
        );
        self.wgpu_renderer.resize(new_physical_size, scale_factor);
        //リサイズ時に静止画テクスチャキャッシュをクリア
        self.wgpu_renderer.webview_static_caches.clear();

        // リサイズが発生したため、キャプチャクールダウンを 15 フレームに設定
        // 拡大リサイズ中およびリサイズ直後の不安定なバッファへのキャプチャを遮断
        self.resize_cooldown_frames = 15;

        let _ = unsafe { self.dcomp_device.Commit() };
    }

    /// 描画のトリガー
    pub fn draw(&mut self, cx: &mut Context) {
        self.wgpu_renderer.render(cx, self.scale_factor);
        let _ = unsafe { self.dcomp_device.Commit() };
    }

    pub fn update_composition_tree(&mut self, cx: &mut Context) {
        unsafe {
            if self.resize_cooldown_frames > 0 {
                self.resize_cooldown_frames -= 1;
            }

            // ルート要素（root_node）のスタイルから DWM アクリル効果を自動検出して同期
            if let Some(&root_id) = cx.topology.topo_active_entities.first()
                && let Some(visual_prop) = cx.renders.rnd_visual.get(root_id)
            {
                let target_backdrop = visual_prop.backdrop;

                // スタイル変更があった場合のみ、拡張した apply_system_backdrop を即時更新
                if self.current_backdrop != target_backdrop {
                    apply_system_backdrop(self.hwnd, target_backdrop);
                    self.current_backdrop = target_backdrop;
                }
            }

            // 非同期キャプチャが完了した要素の一括 DComp 解放処理
            let mut completed = self.pending_removals.borrow_mut().split_off(0);
            for (id, texture_opt) in completed {
                // 成功・失敗を問わず、非同期キャプチャが終了したためフラグを確実にクリアする
                if let Some(pos) = self.promoted_visuals.iter().position(|v| v.entity_id == id) {
                    self.promoted_visuals[pos].is_capturing = false;
                }

                if let Some(wgpu_texture) = texture_opt {
                    // wgpu レンダラーへ静止テクスチャビューとして登録
                    let view =
                        wgpu_texture.create_view(&wgpu::wgt::TextureViewDescriptor::default());
                    self.wgpu_renderer.webview_static_caches.insert(id, view);

                    cx.renders.rnd_active_webviews.remove(&id);

                    self.pending_dcomp_releases.push(PendingDcompRelease {
                        entity_id: id,
                        frames_left: 3, // 3フレームの遅延
                    });
                }
            }

            // 遅延キューの消化処理（1〜3フレーム目）
            let mut idx = 0;
            while idx < self.pending_dcomp_releases.len() {
                if self.pending_dcomp_releases[idx].frames_left > 0 {
                    self.pending_dcomp_releases[idx].frames_left -= 1;
                    idx += 1;
                } else {
                    // ディレイ猶予が切れたため、安全に DComp から破棄を実行
                    let release = self.pending_dcomp_releases.remove(idx);
                    let target_id = release.entity_id;

                    if let Some(pos) = self
                        .promoted_visuals
                        .iter()
                        .position(|v| v.entity_id == target_id)
                    {
                        let promoted = &mut self.promoted_visuals[pos];
                        // タイムアウトが切れたため（wgpuへのテクスチャ定着完了）、
                        // 実体は破棄せず、DCompツリーから取り外して不可視にするのみ
                        if promoted.is_visible {
                            let _ = self.root_visual.RemoveVisual(&promoted.visual);
                            if let Some(ref controller) = *promoted.webview_controller.borrow() {
                                let _ = controller.SetIsVisible(false); // 不可視化
                            }
                            promoted.is_visible = false;
                        }
                    }
                }
            }

            let mut current_promoted_ids = Vec::new();

            for &id in &cx.topology.topo_webview_entities {
                // WebView2 要素を抽出して昇格させる
                let is_webview = cx.topology.topo_active_masks.at(id).has_webveiw2_content();

                let is_always_active = cx
                    .contents
                    .cont_webview_contents
                    .get(id)
                    .is_some_and(|c| c.always_active);

                // 要素自身だけでなく、上に重なっている子要素の操作中もアクティブと判定
                let is_interactive = has_interactive_descendant(cx, id);

                // 現在非同期キャプチャの実行中かチェック
                let is_capturing = self
                    .promoted_visuals
                    .iter()
                    .any(|v| v.entity_id == id && v.is_capturing);
                // 対象要素が現在サイズ・トランスフォーム等のアニメーション/トランジション中であるか判定
                let is_transitioning =
                    cx.renders
                        .rnd_active_transitions
                        .get(id)
                        .is_some_and(|list| {
                            list.iter().any(|t| {
                                t.property_list == PropertyList::Width
                                    || t.property_list == PropertyList::Height
                                    || t.property_list == PropertyList::Size
                                    || t.property_list == PropertyList::Transform
                            })
                        })
                        || cx
                            .renders
                            .rnd_active_animations
                            .get(id)
                            .is_some_and(|list| {
                                list.iter().any(|a| {
                                    a.property == PropertyList::Width
                                        || a.property == PropertyList::Height
                                        || a.property == PropertyList::Size
                                        || a.property == PropertyList::Transform
                                })
                            });

                // 要素の物理サイズが前フレームから微細変動（リサイズドラッグなど）しているか判定
                let rect = cx.outputs.out_rects.get(id).copied().unwrap_or_default();
                let prev_rect = cx
                    .outputs
                    .out_prev_rects
                    .get(id)
                    .copied()
                    .unwrap_or_default();
                let is_size_changing = (rect.width - prev_rect.width).abs() > 0.01
                    || (rect.height - prev_rect.height).abs() > 0.01;

                // まだ静止画のテクスチャキャッシュが作成されていないか
                let has_no_cache = !self.wgpu_renderer.webview_static_caches.contains_key(&id);

                let is_pending_release = self
                    .pending_dcomp_releases
                    .iter()
                    .any(|r| r.entity_id == id);

                // 現在ウィンドウがリアルタイムにリサイズ中であるか
                let is_resizing = cx.window.win_is_resizing;

                // インタラクティブ操作中、またはまだキャッシュがなくキャプチャもキックされていない間、
                // あるいはキャプチャ実行中（wgpuにテクスチャが届くのを待っている間）は、DComp上に実体を生かします。
                // トランジションアニメーション中（is_transitioning）も実体（DComp）の昇格表示を維持
                let should_promote = is_webview
                    && !is_pending_release
                    && (is_interactive
                        || is_always_active
                        || has_no_cache
                        || is_capturing
                        || is_transitioning
                        || is_size_changing
                        || is_resizing);
                if should_promote {
                    current_promoted_ids.push(id);

                    if let Some(pos) = self.promoted_visuals.iter().position(|v| v.entity_id == id)
                    {
                        let promoted = &mut self.promoted_visuals[pos];
                        if !promoted.is_visible {
                            // DCompツリーに再マウント
                            self.root_visual
                                .AddVisual(&promoted.visual, false, &self.wgpu_visual)
                                .unwrap();
                            // ブラウザコントロールを再アクティブ化
                            if let Some(ref controller) = *promoted.webview_controller.borrow() {
                                let _ = controller.SetIsVisible(true);
                            }
                            promoted.is_visible = true;
                        }
                    } else {
                        // プールにまだ存在しない、正真正銘の初回生成時のみ、一から非同期マウントをキック
                        self.promote_element_to_visual(cx, id);
                    }

                    if let Some(pos) = self.promoted_visuals.iter().position(|v| v.entity_id == id)
                        && self.promoted_visuals[pos]
                            .webview_controller
                            .borrow()
                            .is_some()
                    {
                        // コントローラーがバインドされた＝初期化完了したため、wgpu 側に穴あけを指示
                        cx.renders.rnd_active_webviews.insert(id);

                        // 実体 WebView2 の表示が可能になった「このフレーム」で初めてキャッシュを解放。
                        if self.wgpu_renderer.webview_static_caches.contains_key(&id) {
                            self.wgpu_renderer.webview_static_caches.remove(&id);
                        }
                    }
                }
            }

            // アクティブから消えた（要素がデスポーンした）Visual をクリーンアップ
            let mut i = 0;
            while i < self.promoted_visuals.len() {
                let id = self.promoted_visuals[i].entity_id;

                // 生存していない（despawn済みの）要素はクリーンアップ
                if !cx.topology.topo_entities.contains_key(id) {
                    let promoted = &self.promoted_visuals[i];
                    if promoted.is_visible {
                        let _ = self.root_visual.RemoveVisual(&promoted.visual);
                    }
                    // WebView2 コントローラーの明示的な破棄クローズ
                    if let Some(ref controller) = *promoted.webview_controller.borrow() {
                        let _ = controller.Close();
                    }
                    self.promoted_visuals.remove(i);
                    continue;
                }

                let is_pending_release = self
                    .pending_dcomp_releases
                    .iter()
                    .any(|r| r.entity_id == id);

                // 生きてはいるが、アクティブ維持対象外になった（静止画像へ移行した）もの
                if !current_promoted_ids.contains(&id) && !is_pending_release {
                    let promoted = &mut self.promoted_visuals[i];

                    // すでにキャッシュが存在し、表示維持が不要になったため、即座に非表示常駐化
                    if promoted.is_visible {
                        let _ = self.root_visual.RemoveVisual(&promoted.visual);
                        if let Some(ref controller) = *promoted.webview_controller.borrow() {
                            let _ = controller.SetIsVisible(false); // 不可視化して常駐
                        }
                        promoted.is_visible = false;
                    }
                    // promoted_visuals 配列からは削除しない
                    i += 1;
                    continue;
                }

                let is_always_active = cx
                    .contents
                    .cont_webview_contents
                    .get(id)
                    .is_some_and(|c| c.always_active);
                let is_interactive = has_interactive_descendant(cx, id);
                let has_no_cache = !self.wgpu_renderer.webview_static_caches.contains_key(&id);

                let is_transitioning =
                    cx.renders
                        .rnd_active_transitions
                        .get(id)
                        .is_some_and(|list| {
                            list.iter().any(|t| {
                                t.property_list == PropertyList::Width
                                    || t.property_list == PropertyList::Height
                                    || t.property_list == PropertyList::Size
                                    || t.property_list == PropertyList::Transform
                            })
                        })
                        || cx
                            .renders
                            .rnd_active_animations
                            .get(id)
                            .is_some_and(|list| {
                                list.iter().any(|a| {
                                    a.property == PropertyList::Width
                                        || a.property == PropertyList::Height
                                        || a.property == PropertyList::Size
                                        || a.property == PropertyList::Transform
                                })
                            });

                let rect = cx.outputs.out_rects.get(id).copied().unwrap_or_default();
                let prev_rect = cx
                    .outputs
                    .out_prev_rects
                    .get(id)
                    .copied()
                    .unwrap_or(LayoutRect::ZERO);
                let is_size_changing = (rect.width - prev_rect.width).abs() > 0.01
                    || (rect.height - prev_rect.height).abs() > 0.01;

                // 要素自体のサイズ・変形アニメーションが終了（is_transitioning = false）するまでキャプチャを保留
                let is_stable = !cx.window.win_is_resizing
                    && self.resize_cooldown_frames == 0
                    && !is_transitioning
                    && !is_size_changing;

                if !is_interactive
                    && !is_always_active
                    && has_no_cache
                    && !is_pending_release
                    && is_stable
                {
                    let promoted = &mut self.promoted_visuals[i];
                    // 一度だけ非同期キャプチャを確実にキックする
                    // コントローラーが Some であることの確認を capturing 状態の遷移よりも前に置くことでデッドロックを防止
                    if !promoted.is_capturing
                        && let Some(ref controller) = *promoted.webview_controller.borrow()
                    {
                        let rect = cx
                            .outputs
                            .out_rects
                            .get(id)
                            .copied()
                            .unwrap_or(LayoutRect::ZERO);
                        let width = (rect.width * self.scale_factor).round() as u32;
                        let height = (rect.height * self.scale_factor).round() as u32;

                        // サイズが0の場合はエラーを避けるために早期リターン
                        if width == 0 || height == 0 {
                            i += 1;
                            continue;
                        }

                        promoted.is_capturing = true;

                        let webview = controller.CoreWebView2().unwrap();
                        // 安全な .get() とアンラップで座標を取得
                        let rect = cx
                            .outputs
                            .out_rects
                            .get(id)
                            .copied()
                            .unwrap_or(LayoutRect::ZERO);

                        let width = (rect.width * self.scale_factor).round() as u32;
                        let height = (rect.height * self.scale_factor).round() as u32;

                        let pending_removals_clone = self.pending_removals.clone();
                        let wgpu_device = self.wgpu_renderer.device.clone();
                        let wgpu_queue = self.wgpu_renderer.queue.clone();
                        let wic_factory = self.wic_factory.clone();
                        let parent_hwnd = self.hwnd; // HWNDの退避

                        // 非同期キャプチャをキック
                        let capture_res = crate::trigger_capture_async(
                            &webview,
                            width,
                            height,
                            wgpu_device,
                            wgpu_queue,
                            wic_factory,
                            move |result| {
                                match result {
                                    Ok(wgpu_texture) => {
                                        pending_removals_clone
                                            .borrow_mut()
                                            .push((id, Some(wgpu_texture)));

                                        // 非同期キャプチャ完了直後に強制再描画をかけて
                                        // DComp 解放と wgpu への描画バインド切り替えを即座に適用する
                                        let _ = InvalidateRect(Some(parent_hwnd), None, false);
                                    }
                                    Err(e) => {
                                        // エラーをコンソールに出力して握りつぶしを防止
                                        // TODO: tracing クレートに変更
                                        eprintln!("Error during WebView2 Capture: {e:?}");
                                        // None を投げてメインスレッドにフラグ回収を促す
                                        pending_removals_clone.borrow_mut().push((id, None));
                                        let _ = InvalidateRect(Some(parent_hwnd), None, false);
                                    }
                                }
                            },
                        );

                        if let Err(e) = capture_res {
                            // TODO: tracing クレートに変更
                            eprintln!("Error triggering CapturePreview API: {e:?}");
                            promoted.is_capturing = false; // API呼び出し自体に失敗した場合は即リセット
                        }
                    }
                }

                // 2. 生きている要素のサイズを追従（Taffyのレイアウトアニメーションと完全同期）
                let rect = cx.outputs.out_rects.get(id).copied().unwrap_or_default();
                // 親の overflow 等で制限された表示領域
                let clip_rect = cx
                    .outputs
                    .out_clip_rects
                    .get(id)
                    .copied()
                    .unwrap_or_default();
                let visual = &self.promoted_visuals[i].visual;

                // 移動中・リサイズ中におけるDCompスワップチェーンの子の影の点滅を防止するため、
                // 要素の絶対座標（rect）およびクリップ境界（clip_rect）が前回から1ピクセルも変化していない場合は、
                // DComp側へのOffset/Clip/Boundsの再設定を完全にスキップして早期スルー。
                let prev_rect = cx
                    .outputs
                    .out_prev_rects
                    .get(id)
                    .copied()
                    .unwrap_or_default();
                let prev_clip = cx
                    .outputs
                    .out_prev_clip_rects
                    .get(id)
                    .copied()
                    .unwrap_or_default();

                // 要素の物理サイズが変化した場合、古いキャッシュテクスチャを即座に破棄（無効化）
                //  初期サイズ決定時（prev_rect が ZERO の起動時フレーム）を除外
                if prev_rect != LayoutRect::ZERO
                    && ((rect.width - prev_rect.width).abs() > 0.5
                        || (rect.height - prev_rect.height).abs() > 0.5)
                    && self.wgpu_renderer.webview_static_caches.contains_key(&id)
                {
                    self.wgpu_renderer.webview_static_caches.remove(&id);
                }

                // トランスフォーム（Transform）が現在トランジション中か判定
                let has_active_transform_anim = cx
                    .renders
                    .rnd_active_transitions
                    .get(id)
                    .is_some_and(|list| {
                        list.iter()
                            .any(|t| t.property_list == PropertyList::Transform)
                    });

                let is_resizing = cx.window.win_is_resizing;
                // トランジション駆動中であれば早期スルーを確実にバイパスして毎フレームの再設定を保証
                if !is_resizing
                    && rect == prev_rect
                    && clip_rect == prev_clip
                    && !has_active_transform_anim
                {
                    i += 1;
                    continue;
                }

                // 位置の同期 (物理座標)
                let phys_x = rect.x * self.scale_factor;
                let phys_y = rect.y * self.scale_factor;
                visual.SetOffsetX2(phys_x).unwrap();
                visual.SetOffsetY2(phys_y).unwrap();

                // DComp 側への 2D アフィン変換行列 (Matrix3x2) の同期を追加
                if let Some(visual_prop) = cx.renders.rnd_visual.get(id) {
                    if let Some(m) = visual_prop.transform {
                        let m11 = m[0][0];
                        let m12 = m[0][1];
                        let m21 = m[1][0];
                        let m22 = m[1][1];

                        // 平行移動量を物理ピクセルにスケーリング
                        let tx = m[3][0] * self.scale_factor;
                        let ty = m[3][1] * self.scale_factor;

                        // トランスフォームの中心 (Transform Origin) を物理ピクセルに解決
                        let origin = visual_prop
                            .transform_origin
                            .map_or([0.5, 0.5], |p| [p.x, p.y]);
                        let origin_x = origin[0] * rect.width * self.scale_factor;
                        let origin_y = origin[1] * rect.height * self.scale_factor;

                        // wgpu 側シェーダーと数学的に完全一致する Origin 考慮の平行移動量 (M31, M32) を計算
                        let m31 = (1.0 - m11) * origin_x - m21 * origin_y + tx;
                        let m32 = -m12 * origin_x + (1.0 - m22) * origin_y + ty;

                        let dcomp_matrix = Matrix3x2 {
                            M11: m11,
                            M12: m12,
                            M21: m21,
                            M22: m22,
                            M31: m31,
                            M32: m32,
                        };

                        let _ = visual.SetTransform2(&raw const dcomp_matrix);
                    } else {
                        // トランスフォームが未定義または解除された場合は単位行列をセットして初期化
                        let dcomp_matrix = Matrix3x2 {
                            M11: 1.0,
                            M12: 0.0,
                            M21: 0.0,
                            M22: 1.0,
                            M31: 0.0,
                            M32: 0.0,
                        };
                        let _ = visual.SetTransform2(&raw const dcomp_matrix);
                    }
                }

                // DComp の仕様に則り、通常の CreateRectangleClip から角丸設定を行います
                if let Some(visual_prop) = cx.renders.rnd_visual.get(id) {
                    // 1. 通常の RectangleClip オブジェクトをデバイスから生成
                    let dcomp_device = self.dcomp_device.clone();
                    let rectangle_clip = dcomp_device.CreateRectangleClip().unwrap();

                    // 2. 絶対クリップ境界（clip_rect）からビジュアルローカルの物理ピクセル範囲を算出してセット
                    let clip_left = (clip_rect.x - rect.x).max(0.0) * self.scale_factor;
                    let clip_top = (clip_rect.y - rect.y).max(0.0) * self.scale_factor;
                    let clip_right = ((clip_rect.x + clip_rect.width) - rect.x).min(rect.width)
                        * self.scale_factor;
                    let clip_bottom = ((clip_rect.y + clip_rect.height) - rect.y).min(rect.height)
                        * self.scale_factor;

                    rectangle_clip.SetLeft2(clip_left).unwrap();
                    rectangle_clip.SetTop2(clip_top).unwrap();
                    rectangle_clip.SetRight2(clip_right).unwrap();
                    rectangle_clip.SetBottom2(clip_bottom).unwrap();

                    // 3. クリップに角丸を設定
                    if let Some(radius) = visual_prop.corner_radius {
                        let r = radius.top_left * self.scale_factor;

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
                if let Some(ref controller) = *self.promoted_visuals[i].webview_controller.borrow()
                {
                    // DComp でOffsetX/Yを設定しているため、WebView2自身の境界空間は常に 0, 0 起点とする。
                    // これにより親ウィンドウの他のピクセル（タイトルテキストなど）の混入を物理的に完全に遮断します。
                    // let phys_x = (rect.x * self.scale_factor).round() as i32;
                    // let phys_y = (rect.y * self.scale_factor).round() as i32;
                    let phys_w = rect.width * self.scale_factor;
                    let phys_h = rect.height * self.scale_factor;

                    let bounds = windows::Win32::Foundation::RECT {
                        left: 0,
                        top: 0,
                        right: phys_w.round() as i32,
                        bottom: phys_h.round() as i32,
                    };
                    let _ = controller.SetBounds(bounds);
                }

                i += 1;
            }

            // 変更をコンポジターにコミットして一括反映
            // コミットは draw の末尾で一括して 1 回だけ行い、DComp と wgpu を完全同期させます。
            // self.dcomp_device.Commit().unwrap();
        }
    }

    /// 特定の要素を独立した `IDCompositionVisual` に昇格させ、Compositor アニメーションをバインドする
    unsafe fn promote_element_to_visual(&mut self, cx: &Context, id: EntityId) {
        unsafe {
            // 1. 新しい Visual を作成
            let visual = self.dcomp_device.CreateVisual().unwrap();

            // 2. 位置とサイズを DComp 側に同期（最初のフレームから物理座標を使い、ジャンプを防ぐ）
            let rect = cx.outputs.out_rects.get(id).copied().unwrap_or_default();
            let phys_x = rect.x * self.scale_factor;
            let phys_y = rect.y * self.scale_factor;
            visual.SetOffsetX2(phys_x).unwrap();
            visual.SetOffsetY2(phys_y).unwrap();

            // 3. 前面 wgpu (wgpu_visual) の直下・背面（insertabove = false）に WebView2 を挿入
            // これにより、重なり順が常に [WebView2] ➔ [wgpu_visual] となり、
            // wgpu 側のパンチアウト透明窓を通して背面が透過。
            self.root_visual
                .AddVisual(&visual, false, &self.wgpu_visual)
                .unwrap();

            let webview_controller = Rc::new(RefCell::new(None));

            // B. WebView2 設定のバインド (COMP_WEBVIEW_CONTENTフラグ)
            if cx.topology.topo_active_masks.at(id).has_webveiw2_content()
                && let Some(contents) = cx.contents.cont_webview_contents.get(id)
            {
                let slot_clone = webview_controller.clone();

                // この要素の Visual ターゲットに向けて WebView2 を非同期初期化
                let _ = crate::init_webview2_composition(
                    self.hwnd,
                    visual.clone(),
                    slot_clone,
                    contents.clone(), // 設定値（URL、DevTools等のフラグ）を引き渡す
                    rect,
                    self.scale_factor,
                    self.webview_env.clone(),
                    cx.task_sender(),
                );
            }

            // 5. 管理プールに登録
            self.promoted_visuals.push(PromotedVisual {
                entity_id: id,
                visual,
                transform: None,
                webview_controller,
                is_capturing: false,
                is_visible: true, // 新規作成時はマウント済み
            });
        }
    }

    /// 指定された `WebView2` 要素にキーボードフォーカスをプログラムから強制的に移行します。
    /// これにより、OS からのキーボード入力が自動的にブラウザ内に流れるようになります。
    pub fn focus_webview(&self, id: EntityId) {
        if let Some(promoted) = self.promoted_visuals.iter().find(|v| v.entity_id == id)
            && let Some(ref controller) = *promoted.webview_controller.borrow()
        {
            // プログラム駆動（PROGRAMMATIC）でフォーカスを WebView2 に渡す
            let _ = unsafe { controller.MoveFocus(COREWEBVIEW2_MOVE_FOCUS_REASON_PROGRAMMATIC) };
        }
    }

    /// 呼び出し元（ウィンドウプロシージャ）からマウス入力を受け取り、
    /// 対象の `WebView2` 要素へ座標をローカライズした上で転送します。
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
        if let Some(contents) = cx.contents.cont_webview_contents.get(id)
            && !contents.allow_interaction
        {
            return;
        }

        // 2. この WebView2 要素の矩形（rect）を取得
        let rect = cx.outputs.out_rects[id];

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
                // Win32 の msg と wparam は、そのまま DComp のイベントにキャスト可能
                let event_kind = COREWEBVIEW2_MOUSE_EVENT_KIND(msg as i32);
                let virtual_keys = COREWEBVIEW2_MOUSE_EVENT_VIRTUAL_KEYS(wparam.0 as i32);

                // ホイールの回転量やXボタンなどのデータを WPARAM / LPARAM から抽出
                // WPARAM 上位ビットを「符号付き i16」として一旦解釈してから
                // 32ビットに拡張キャスト。これによってマイナス方向のスクロールが正しく動作します
                let mouse_data = if msg == WM_MOUSEWHEEL || msg == WM_MOUSEHWHEEL {
                    let delta = (wparam.0 >> 16) as i16;
                    i32::from(delta) as u32
                } else {
                    0
                };

                let point = POINT {
                    x: relative_x.round() as i32,
                    y: relative_y.round() as i32,
                };

                // ブラウザエンジンへ入力イベントを浸透させる
                let _ = unsafe {
                    comp_controller.SendMouseInput(event_kind, virtual_keys, mouse_data, point)
                };

                let _ = unsafe { self.dcomp_device.Commit() };
            }
        }
    }

    // ウィンドウハンドル (HWND) が手元にある状態からスタート
    pub(crate) fn setup_direct_composition(
        hwnd: HWND,
    ) -> Result<
        (
            IDCompositionDesktopDevice,
            IDCompositionTarget,
            IDCompositionVisual2, // root_visual
            IDCompositionVisual2, // wgpu_visual
        ),
        Box<dyn std::error::Error>,
    > {
        unsafe {
            // 1. D3D11 デバイスを作成
            // グローバルなマネージャーの解決を試みる（失敗時は呼び出し元にエラーを伝播できるよう、後々 Result にするか
            // ここではひとまず unwrap() などで処理する形にしておきます）
            let manager = DCompDeviceManager::global()?;
            let dcomp_device = manager.dcomp_device.clone();

            let dcomp_target = dcomp_device.CreateTargetForHwnd(hwnd, true)?;
            let root_visual = dcomp_device.CreateVisual()?;
            dcomp_target.SetRoot(&root_visual)?;

            // wgpu 用のメインビジュアルを1つだけ作成して登録
            let wgpu_visual = dcomp_device.CreateVisual()?;
            root_visual.AddVisual(&wgpu_visual, true, None)?;

            // 変更をコンポジターにコミットして反映
            dcomp_device.Commit()?;

            Ok((dcomp_device, dcomp_target, root_visual, wgpu_visual))
        }
    }
}

// 親子関係を再帰的に走査してアクティビティを伝播するヘルパー関数の追加 ───
fn has_interactive_descendant(cx: &Context, id: EntityId) -> bool {
    // 自分自身がフォーカス、またはアクティブ状態のインタラクション属性を持っているか
    if cx
        .topology
        .topo_active_masks
        .at(id)
        .has_active_interaction_property()
    {
        return true;
    }
    // 子要素を再帰的にチェック
    for &child_id in cx.topology.topo_children.at(id) {
        if has_interactive_descendant(cx, child_id) {
            return true;
        }
    }

    false
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
        // D3D11 デバイスを作成
        let mut d3d11_device: Option<ID3D11Device> = None;
        unsafe {
            D3D11CreateDevice(
                None,
                D3D_DRIVER_TYPE_HARDWARE,
                HMODULE::default(),
                D3D11_CREATE_DEVICE_BGRA_SUPPORT,
                None,
                D3D11_SDK_VERSION,
                Some(&raw mut d3d11_device),
                None,
                None,
            )?;
        }
        let d3d11_device = d3d11_device.ok_or("Failed to create D3D11 hardware device")?;

        // DXGI デバイスをクエリ
        // let dxgi_device: IDXGIDevice = d3d11_device.cast()?;

        // windows-rs の型ミスマッチを防ぐため、一度 IUnknown にキャスト
        // let rendering_device: windows::core::IUnknown = dxgi_device.cast()?;

        // DCompositionCreateDevice ではなく DCompositionCreateDevice2 を使用します。
        // これにより IDCompositionDesktopDevice の生成が正しくサポートされます。
        let dcomp_device: IDCompositionDesktopDevice = unsafe { DCompositionCreateDevice2(None) }?;

        Ok(DCompDeviceManager {
            d3d11_device,
            dcomp_device,
        })
    }
}

#[link(name = "dwmapi")]
unsafe extern "system" {
    fn DwmSetWindowAttribute(
        hwnd: HWND,
        dwattribute: u32,
        pvattribute: *const std::ffi::c_void,
        cbattribute: u32,
    ) -> windows::core::HRESULT;
}

// フレーム拡張用構造体
#[repr(C)]
#[derive(Copy, Clone, Debug)]
pub struct Margins {
    pub left: i32,
    pub right: i32,
    pub top: i32,
    pub bottom: i32,
}

#[link(name = "dwmapi")]
unsafe extern "system" {
    fn DwmExtendFrameIntoClientArea(
        hwnd: HWND,
        pMarInset: *const Margins,
    ) -> windows::core::HRESULT;
}

// apply_system_backdrop を拡張してダークモードとフレーム拡張を統合
pub(crate) fn apply_system_backdrop(hwnd: HWND, backdrop: Backdrop) {
    unsafe {
        // DWMWA_USE_HOSTBACKDROPBRUSH (17) を TRUE に設定
        // DComp（NOREDIRECTIONBITMAP）ウィンドウでアクリル・Micaを透かすため
        let enable_host_backdrop: i32 = 1; // 1 = TRUE
        let _ = DwmSetWindowAttribute(
            hwnd,
            17, // DWMWA_USE_HOSTBACKDROPBRUSH
            (&raw const enable_host_backdrop).cast(),
            std::mem::size_of::<i32>() as u32,
        );
        // 1. DWMWA_USE_IMMERSIVE_DARK_MODE (20) を true に設定（ダークアクリル下地を強制）
        let dark_mode: i32 = 1; // 1 = true
        let _ = DwmSetWindowAttribute(
            hwnd,
            20, // DWMWA_USE_IMMERSIVE_DARK_MODE
            (&raw const dark_mode).cast(),
            std::mem::size_of::<i32>() as u32,
        );

        // DWMWA_USE_HOSTBACKDROPBRUSH (36) を true に設定
        // WS_EX_NOREDIRECTIONBITMAP ウィンドウの背後に
        // DWM が自動でアクリル用のぼかし背景ブラシ（BackdropBrush）を合成
        let use_host_backdrop: i32 = 1; // 1 = TRUE
        let _ = DwmSetWindowAttribute(
            hwnd,
            36, // DWMWA_USE_HOSTBACKDROPBRUSH
            (&raw const use_host_backdrop).cast(),
            std::mem::size_of::<i32>() as u32,
        );

        // 2. DWMWA_SYSTEMBACKDROP_TYPE (38) の設定（アクリル/Micaの適用）
        let backdrop_val = backdrop as i32;
        let _ = DwmSetWindowAttribute(
            hwnd,
            38, // DWMWA_SYSTEMBACKDROP_TYPE
            (&raw const backdrop_val).cast(),
            std::mem::size_of::<i32>() as u32,
        );

        // 3. クライアント領域全体にアクリル・Micaを拡張 (DwmExtendFrameIntoClientArea)
        if backdrop == Backdrop::None {
            // 通常時はフレーム拡張をクリア (0)
            let margins = Margins {
                left: 0,
                right: 0,
                top: 0,
                bottom: 0,
            };
            let _ = DwmExtendFrameIntoClientArea(hwnd, &raw const margins);
        } else {
            // margins に -1 を指定することで、ウィンドウ全体にアクリルを浸透させます
            let margins = Margins {
                left: -1,
                right: -1,
                top: -1,
                bottom: -1,
            };
            let _ = DwmExtendFrameIntoClientArea(hwnd, &raw const margins);
        }
    }
}
