use crate::{
    AnimationCurve, Backdrop, ComponentMask, Context, CornerRadius, DebugStore, EntityId,
    ExternalVisual, ExternalVisualMetadata, LayoutPoint, LayoutRect, LayoutSize, MichiuError,
    MichiuSoA, MichiuTrace, PlaybackCount, PropertyList, ResultTraceExt, StaticExternalTexture,
    VisualUpdateContext, WebView2Contents, WgpuRenderer, WindowsResultTraceExt, flush_trace,
    trace_error, trace_lifecycle,
};
use std::{
    cell::RefCell,
    rc::Rc,
    sync::{Arc, Mutex, OnceLock},
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

    /// 動的に昇格された `WebView2` レイヤーの一覧
    pub promoted_visuals: Vec<PromotedVisual>,

    /// `DComp` 側の Visual 削除を wgpu のピクセル定着から数フレーム遅延させるためのキュー
    pub(crate) pending_dcomp_releases: Vec<PendingDcompRelease>,

    /// 現在ウィンドウに適用中のバックドロップ状態
    pub(crate) current_backdrop: Backdrop,

    /// リサイズが完全に安定するまでキャプチャを保留するカウンター
    pub(crate) resize_cooldown_frames: u32,
}

#[derive(Debug, Clone)]
pub struct PendingDcompRelease {
    pub entity_id: EntityId,
    pub frames_left: u32,
}

#[derive(Debug, Clone)]
pub struct PromotedVisual {
    /// 昇格した要素の ID
    pub entity_id: EntityId,
    /// `DirectComposition` 側の Visual オブジェクト
    pub visual: IDCompositionVisual2,
    /// 適用しているトランスフォームオブジェクト (COM参照を維持するために保持)
    pub transform: Option<windows::core::IUnknown>,
    // DCompツリーにマウントされており、コントローラーが可視状態であるか
    pub is_visible: bool,
}

impl ComposedRenderer {
    /// Initialize the renderer.
    #[inline]
    pub async fn new(
        hwnd: HWND,
        layout_size: LayoutSize,
        scale_factor: f32,
    ) -> crate::Result<Self> {
        // DirectComposition の構築 (setup_direct_composition を内包)
        let (dcomp_device, dcomp_target, root_visual, wgpu_visual) =
            ComposedRenderer::setup_direct_composition(hwnd)?;

        // HINSTANCE（h_instance）の解決
        let h_instance = unsafe { windows::Win32::System::LibraryLoader::GetModuleHandleW(None)? };

        // wgpu レンダラーの初期化
        let raw_visual_ptr = wgpu_visual.as_raw();

        let wgpu_renderer = WgpuRenderer::new(raw_visual_ptr, layout_size, scale_factor).await?;

        unsafe {
            dcomp_device.Commit()?;
        }

        let wic_factory: IWICImagingFactory =
            unsafe { CoCreateInstance(&CLSID_WICImagingFactory, None, CLSCTX_INPROC_SERVER)? };

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
            promoted_visuals: Vec::new(),
            pending_dcomp_releases: Vec::new(),
            current_backdrop: Backdrop::None,
            resize_cooldown_frames: 0,
        })
    }

    /// Updates the overall settings and resizes wgpu when the window size changes.
    ///
    /// `new_physical_size`: (width, height)
    #[inline]
    pub fn resize(&mut self, new_physical_size: (u32, u32), scale_factor: f32) {
        self.scale_factor = scale_factor;
        self.layout_size = LayoutSize::new(
            new_physical_size.0 as f32 / scale_factor,
            new_physical_size.1 as f32 / scale_factor,
        );
        self.wgpu_renderer.resize(new_physical_size, scale_factor);

        // リサイズが発生したため、キャプチャクールダウンを 15 フレームに設定
        // 拡大リサイズ中およびリサイズ直後の不安定なバッファへのキャプチャを遮断
        self.resize_cooldown_frames = 15;

        let _ = unsafe { self.dcomp_device.Commit() };
    }

    /// Drawing Triggers
    #[track_caller]
    #[inline]
    pub fn draw(&mut self, cx: &mut Context) {
        self.wgpu_renderer.render(cx, self.scale_factor);

        unsafe {
            self.dcomp_device
                .Commit()
                .unwrap_or_trace(None, &mut cx.debug);
        };

        #[cfg(feature = "trace-lifecycle")]
        trace_lifecycle!(None, &mut cx.debug, || MichiuTrace::Commit { add: None });

        #[cfg(feature = "trace-lifecycle")]
        {
            let total_entities = cx.topology.topo_entities.len();
            let active_entities = cx.topology.topo_active_entities.len();
            let dirty_layouts = cx.layouts.lay_dirty_entities.len();
            let dirty_renders = cx.renders.rnd_dirty_entities.len();

            #[cfg(feature = "trace-lifecycle")]
            trace_lifecycle!(None, &mut cx.debug, || MichiuTrace::ClearDirtyEntities {
                total_entities,
                active_entities,
                dirty_layouts,
                dirty_renders,
                add: Some(
                    "Immediately after this recording, the dirty flag is cleared and the log is sent."
                ),
            });
        }

        // ダーティフラグをクリア
        cx.clear_dirty();
        // ログを送信
        flush_trace!(cx, self);
    }

    #[track_caller]
    pub fn update_composition_tree(&mut self, cx: &mut Context) {
        unsafe {
            if self.resize_cooldown_frames > 0 {
                self.resize_cooldown_frames -= 1;
            }

            // ルート要素（root_node）のスタイルから DWM アクリル効果を自動検出して同期
            if let Some(&root_id) = cx.topology.topo_active_entities.first()
                && let Some(visual_prop) = cx.renders.rnd_visual.find(root_id)
            {
                let target_backdrop = visual_prop.backdrop;

                // スタイル変更があった場合のみ、拡張した apply_system_backdrop を即時更新
                if self.current_backdrop != target_backdrop {
                    apply_system_backdrop(self.hwnd, target_backdrop);
                    self.current_backdrop = target_backdrop;
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
                            promoted.is_visible = false;
                        }
                    }
                }
            }

            // 外部 Visual の一括更新 ＆ アクティブ判定
            let mut current_promoted_ids = Vec::new();

            // ループの前にスナップショットを取得
            let visual_entries: Vec<(EntityId, Arc<dyn ExternalVisual>)> = cx
                .contents
                .cont_external_visual
                .iter()
                .map(|(id, v)| (id, Arc::clone(v)))
                .collect();

            for (id, visual_entry) in visual_entries {
                let rect = cx.outputs.out_rects.find_or_default(id, &mut cx.debug);
                let dcomp_visual = visual_entry.resolve_visual();
                let metadata = visual_entry.metadata();

                // 安定状態の判定
                let is_interactive = has_interactive_descendant(cx, id);
                let is_transitioning = is_element_transitioning(cx, id);
                let prev_rect = cx.outputs.out_prev_rects.find_or_default(id, &mut cx.debug);
                let is_size_changing = (rect.width - prev_rect.width).abs() > 0.01
                    || (rect.height - prev_rect.height).abs() > 0.01;
                let is_stable = !cx.window.win_is_resized
                    && self.resize_cooldown_frames == 0
                    && !is_transitioning
                    && !is_size_changing;

                // 各 Visual の状態更新フックを実行（WebView2 はここで Bounds 設定や自動キャプチャを行う）
                visual_entry.update(&VisualUpdateContext {
                    hwnd: self.hwnd,
                    rect,
                    scale_factor: self.scale_factor,
                    is_interactive,
                    is_stable,
                    device: &self.wgpu_renderer.device,
                    queue: &self.wgpu_renderer.queue,
                    wic_factory: &self.wic_factory,
                });

                // DComp Visual のレイアウト（位置・クリップ・トランスフォーム）を同期
                self.sync_visual_layout_and_clip(id, &dcomp_visual, metadata, cx);

                // 静止画テクスチャがあるかどうかで分岐
                if let Some(static_tex) = visual_entry.static_texture() {
                    // 静止画
                    cx.contents.cont_external_textures.insert(id, static_tex);
                    cx.topology
                        .topo_active_masks
                        .at_mut(id)
                        .set(ComponentMask::COMP_EXTERNAL_TEXTURE_CONTENT);
                    cx.renders.rnd_active_external_visual.remove(&id);

                    // 直ちに DComp から消すとチラつくため、ディレイキューに登録
                    if !self
                        .pending_dcomp_releases
                        .iter()
                        .any(|r| r.entity_id == id)
                    {
                        self.pending_dcomp_releases.push(PendingDcompRelease {
                            entity_id: id,
                            frames_left: 3,
                        });
                    }
                } else {
                    // 実体表示
                    current_promoted_ids.push(id);
                    cx.topology
                        .topo_active_masks
                        .at_mut(id)
                        .unset(ComponentMask::COMP_EXTERNAL_TEXTURE_CONTENT);

                    // 遅延解放待ちにあれば解除
                    self.pending_dcomp_releases.retain(|r| r.entity_id != id);

                    // DComp ツリーへのマウント
                    if let Some(pos) = self.promoted_visuals.iter().position(|v| v.entity_id == id)
                    {
                        let promoted = &mut self.promoted_visuals[pos];
                        if !promoted.is_visible {
                            let _ = self.root_visual.AddVisual(
                                &promoted.visual,
                                false,
                                &self.wgpu_visual,
                            );
                            promoted.is_visible = true;
                        }
                    } else {
                        let _ = self
                            .root_visual
                            .AddVisual(&dcomp_visual, false, &self.wgpu_visual);
                        self.promoted_visuals.push(PromotedVisual {
                            entity_id: id,
                            visual: dcomp_visual.clone(),
                            transform: None,
                            is_visible: true,
                        });
                    }

                    // wgpu に穴あけ（Punchout）を指示
                    cx.renders.rnd_active_external_visual.insert(id);
                    cx.contents.cont_external_textures.remove(id);
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
                        promoted.is_visible = false;
                    }
                }
                i += 1;
            }
        }
    }

    /// あらゆる `ExternalVisual` に共通の同期ヘルパー
    #[track_caller]
    fn sync_visual_layout_and_clip(
        &self,
        id: EntityId,
        visual: &IDCompositionVisual2,
        metadata: ExternalVisualMetadata,
        cx: &mut Context,
    ) {
        unsafe {
            let rect = cx.outputs.out_rects.find_or_default(id, &mut cx.debug);
            let clip_rect = cx.outputs.out_clip_rects.find_or_default(id, &mut cx.debug);
            let prev_rect = cx.outputs.out_prev_rects.find_or_default(id, &mut cx.debug);
            let prev_clip = cx
                .outputs
                .out_prev_clip_rects
                .find_or_default(id, &mut cx.debug);

            let has_active_transform_anim =
                cx.renders
                    .rnd_active_transitions
                    .find(id)
                    .is_some_and(|list| {
                        list.iter()
                            .any(|t| t.property_list == PropertyList::Transform)
                    });
            let is_resizing = cx.window.win_is_resized;

            // 変化がなければ早期リターン（点滅・CPU負荷防止）
            // sync_layout で prev_rects が既にスワップされているケースを考慮し、
            // 初回またはリサイズ時は確実に同期を通す
            if !is_resizing
                && rect == prev_rect
                && clip_rect == prev_clip
                && !has_active_transform_anim
            {
                return;
            }

            // 位置同期 (物理ピクセル)
            let phys_x = rect.x * self.scale_factor;
            let phys_y = rect.y * self.scale_factor;
            let _ = visual.SetOffsetX2(phys_x);
            let _ = visual.SetOffsetY2(phys_y);

            // 2D アフィン変換 (Transform2) の同期
            if metadata.auto_transform
                && let Some(visual_prop) = cx.renders.rnd_visual.find(id)
            {
                if let Some(m) = visual_prop.transform {
                    let m11 = m[0][0];
                    let m12 = m[0][1];
                    let m21 = m[1][0];
                    let m22 = m[1][1];
                    let tx = m[3][0] * self.scale_factor;
                    let ty = m[3][1] * self.scale_factor;

                    let origin = visual_prop
                        .transform_origin
                        .map_or([0.5, 0.5], |p| [p.x, p.y]);
                    let origin_x = origin[0] * rect.width * self.scale_factor;
                    let origin_y = origin[1] * rect.height * self.scale_factor;

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
                    let identity = Matrix3x2 {
                        M11: 1.0,
                        M12: 0.0,
                        M21: 0.0,
                        M22: 1.0,
                        M31: 0.0,
                        M32: 0.0,
                    };
                    let _ = visual.SetTransform2(&raw const identity);
                }
            }

            // 角丸・Scissor クリップ (RectangleClip) の同期
            if metadata.auto_clip
                && let Some(visual_prop) = cx.renders.rnd_visual.find(id)
            {
                let dcomp_device = self.dcomp_device.clone();
                if let Ok(rectangle_clip) = dcomp_device.CreateRectangleClip() {
                    let clip_left = (clip_rect.x - rect.x).max(0.0) * self.scale_factor;
                    let clip_top = (clip_rect.y - rect.y).max(0.0) * self.scale_factor;
                    let clip_right = ((clip_rect.x + clip_rect.width) - rect.x).min(rect.width)
                        * self.scale_factor;
                    let clip_bottom = ((clip_rect.y + clip_rect.height) - rect.y).min(rect.height)
                        * self.scale_factor;

                    let _ = rectangle_clip.SetLeft2(clip_left);
                    let _ = rectangle_clip.SetTop2(clip_top);
                    let _ = rectangle_clip.SetRight2(clip_right);
                    let _ = rectangle_clip.SetBottom2(clip_bottom);

                    if let Some(radius) = visual_prop.corner_radius {
                        let r = radius.top_left * self.scale_factor;
                        let _ = rectangle_clip.SetTopLeftRadiusX2(r);
                        let _ = rectangle_clip.SetTopLeftRadiusY2(r);
                        let _ = rectangle_clip.SetTopRightRadiusX2(r);
                        let _ = rectangle_clip.SetTopRightRadiusY2(r);
                        let _ = rectangle_clip.SetBottomLeftRadiusX2(r);
                        let _ = rectangle_clip.SetBottomLeftRadiusY2(r);
                        let _ = rectangle_clip.SetBottomRightRadiusX2(r);
                        let _ = rectangle_clip.SetBottomRightRadiusY2(r);
                    } else {
                        let _ = rectangle_clip.SetTopLeftRadiusX2(0.0);
                        let _ = rectangle_clip.SetTopLeftRadiusY2(0.0);
                    }

                    let _ = visual.SetClip(&rectangle_clip);
                }
            }
        }
    }

    // ウィンドウハンドル (HWND) が手元にある状態からスタート
    pub(crate) fn setup_direct_composition(
        hwnd: HWND,
    ) -> crate::Result<(
        IDCompositionDesktopDevice,
        IDCompositionTarget,
        IDCompositionVisual2, // root_visual
        IDCompositionVisual2, // wgpu_visual
    )> {
        unsafe {
            // 1. D3D11 デバイスを作成
            // グローバルなマネージャーの解決を試みる（失敗時は呼び出し元にエラーを伝播できるよう、後々 Result にするか
            // ここではひとまず unwrap() などで処理する形
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

// 親子関係を再帰的に走査してアクティビティを伝播するヘルパー
#[track_caller]
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

fn is_element_transitioning(cx: &Context, id: EntityId) -> bool {
    cx.renders
        .rnd_active_transitions
        .find(id)
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
            .find(id)
            .is_some_and(|list| {
                list.iter().any(|a| {
                    a.property == PropertyList::Width
                        || a.property == PropertyList::Height
                        || a.property == PropertyList::Size
                        || a.property == PropertyList::Transform
                })
            })
}

pub(crate) struct DCompDeviceManager {
    pub(crate) d3d11_device: ID3D11Device,
    pub(crate) dcomp_device: IDCompositionDesktopDevice,
}

unsafe impl Send for DCompDeviceManager {}
unsafe impl Sync for DCompDeviceManager {}

impl DCompDeviceManager {
    /// デバイスマネージャーのグローバル参照を取得。
    /// 未初期化、または過去の初期化でエラーが起きていた場合は再試行。
    pub(crate) fn global() -> crate::Result<&'static Self> {
        static INSTANCE: OnceLock<DCompDeviceManager> = OnceLock::new();

        if let Some(manager) = INSTANCE.get() {
            return Ok(manager);
        }

        let manager = Self::try_create_devices()?;

        // 完全に成功した場合にのみ OnceLock に実体をセットする。
        // 複数スレッドで同時に成功した場合、最初に完了した方がセットされ、
        // もう一方は set() で Err(manager) を返すが、
        // 最終的に get() 側で正しく1つのインスタンスに統合されるため安全。
        let _ = INSTANCE.set(manager);

        INSTANCE
            .get()
            .ok_or(MichiuError::UninitializedDeviceManager)
    }

    /// デバイスの生成・クエリを試みる、失敗を許容する内部ヘルパー
    fn try_create_devices() -> crate::Result<Self> {
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
        let d3d11_device = d3d11_device.ok_or(MichiuError::D3d11DeviceCreationFailed)?;

        // DXGI デバイスをクエリ
        // let dxgi_device: IDXGIDevice = d3d11_device.cast()?;

        // windows-rs の型ミスマッチを防ぐため、一度 IUnknown にキャスト
        // let rendering_device: windows::core::IUnknown = dxgi_device.cast()?;

        // DCompositionCreateDevice ではなく DCompositionCreateDevice2 を使用。
        // これにより IDCompositionDesktopDevice の生成が正しくサポートされる。
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
        // DWMWA_USE_IMMERSIVE_DARK_MODE (20) を true に設定（ダークアクリル下地を強制）
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

        // DWMWA_SYSTEMBACKDROP_TYPE (38) の設定（アクリル/Micaの適用）
        let backdrop_val = backdrop as i32;
        let _ = DwmSetWindowAttribute(
            hwnd,
            38, // DWMWA_SYSTEMBACKDROP_TYPE
            (&raw const backdrop_val).cast(),
            std::mem::size_of::<i32>() as u32,
        );

        // クライアント領域全体にアクリル・Micaを拡張 (DwmExtendFrameIntoClientArea)
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
            // margins に -1 を指定することで、ウィンドウ全体にアクリルを浸透
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
