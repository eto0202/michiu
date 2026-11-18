use michiu_ui::{Context, Entity, ExternalTexture};
use std::sync::Arc;
use windows::Win32::{
    Foundation::HWND,
    Graphics::DirectComposition::{
        IDCompositionDesktopDevice, IDCompositionTarget, IDCompositionVisual2,
    },
};

use crate::CompositionTargetLayer;

pub(crate) struct DCompWorkspace {
    pub(crate) target: IDCompositionTarget,
    pub(crate) root_visual: IDCompositionVisual2,
    pub(crate) wgpu_visual: IDCompositionVisual2,
    // 動的に昇格されたレイヤーの一覧
    pub(crate) promoted_visuals: Vec<PromotedVisual>,
    // `DComp` 側の Visual 削除を wgpu のピクセル定着から数フレーム遅延させるためのキュー
    pub(crate) pending_releases: Vec<PendingDcompRelease>,
}

#[derive(Debug, Clone)]
pub struct PendingDcompRelease {
    pub entity_id: Entity,
    pub frames_left: u32,
}

#[derive(Debug, Clone)]
pub struct PromotedVisual {
    /// 昇格した要素の ID
    pub entity_id: Entity,
    /// `DirectComposition` 側の Visual オブジェクト
    pub visual: IDCompositionVisual2,
    /// 適用しているトランスフォームオブジェクト (COM参照を維持するために保持)
    pub transform: Option<windows::core::IUnknown>,
    // DCompツリーにマウントされており、コントローラーが可視状態であるか
    pub is_visible: bool,
}

impl DCompWorkspace {
    // ウィンドウハンドル (HWND) が手元にある状態からスタート
    pub(crate) fn new(
        hwnd: HWND,
        dcomp: &IDCompositionDesktopDevice,
        target_layer: CompositionTargetLayer,
    ) -> michiu_ui::Result<Self> {
        unsafe {
            let topmost = match target_layer {
                CompositionTargetLayer::Topmost => true,
                CompositionTargetLayer::NonTopmost => false,
            };

            let target = dcomp.CreateTargetForHwnd(hwnd, topmost)?;
            let root_visual = dcomp.CreateVisual()?;
            target.SetRoot(&root_visual)?;

            // wgpu 用のメインビジュアルを1つだけ作成して登録
            let wgpu_visual = dcomp.CreateVisual()?;
            root_visual.AddVisual(&wgpu_visual, true, None)?;

            // 変更をコンポジターにコミットして反映
            dcomp.Commit()?;

            Ok(Self {
                target,
                root_visual,
                wgpu_visual,
                promoted_visuals: Vec::new(),
                pending_releases: Vec::new(),
            })
        }
    }

    #[inline]
    pub(crate) fn processing_delay_queue(&mut self) {
        let mut idx = 0;
        while idx < self.pending_releases.len() {
            if self.pending_releases[idx].frames_left > 0 {
                self.pending_releases[idx].frames_left -= 1;
                idx += 1;
            } else {
                // ディレイ猶予が切れたため、安全に DComp から破棄を実行
                let release = self.pending_releases.remove(idx);
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
                        let _ = unsafe { self.root_visual.RemoveVisual(&promoted.visual) };
                        promoted.is_visible = false;
                    }
                }
            }
        }
    }

    #[inline]
    pub(crate) fn cleanup_visuals(&mut self, cx: &Context, current_promoted_ids: &[Entity]) {
        let mut i = 0;
        while i < self.promoted_visuals.len() {
            let e = self.promoted_visuals[i].entity_id;

            // 生存していない（despawn済みの）要素はクリーンアップ
            if !cx.contains_entities(e) {
                let promoted = &self.promoted_visuals[i];
                if promoted.is_visible {
                    let _ = unsafe { self.root_visual.RemoveVisual(&promoted.visual) };
                }
                self.promoted_visuals.remove(i);
                continue;
            }

            let is_pending_release = self.pending_releases.iter().any(|r| r.entity_id == e);

            // 生きてはいるが、アクティブ維持対象外になった（静止画像へ移行した）もの
            if !current_promoted_ids.contains(&e) && !is_pending_release {
                let promoted = &mut self.promoted_visuals[i];

                // すでにキャッシュが存在し、表示維持が不要になったため、即座に非表示常駐化
                if promoted.is_visible {
                    let _ = unsafe { self.root_visual.RemoveVisual(&promoted.visual) };
                    promoted.is_visible = false;
                }
            }
            i += 1;
        }
    }

    #[inline]
    pub(crate) fn processing_static_texture(
        &mut self,
        cx: &mut Context,
        e: Entity,
        static_tex: Arc<dyn ExternalTexture + 'static>,
    ) {
        cx.set_external_textures(e, static_tex);

        // wgpuの静止画描画と同時にDComp実体を外して二重描画を防止
        if let Some(pos) = self.promoted_visuals.iter().position(|v| v.entity_id == e) {
            let promoted = &mut self.promoted_visuals[pos];
            if promoted.is_visible {
                let _ = unsafe { self.root_visual.RemoveVisual(&promoted.visual) };
                promoted.is_visible = false;
            }
        }
        self.pending_releases.retain(|r| r.entity_id != e);
    }

    #[inline]
    pub(crate) fn processing_punchout_visual(
        &mut self,
        cx: &mut Context,
        e: Entity,
        ex_visual: IDCompositionVisual2,
    ) {
        cx.unset_external_textures(e);

        // 遅延解放待ちにあれば解除
        self.pending_releases.retain(|r| r.entity_id != e);

        // DComp ツリーへのマウント
        if let Some(pos) = self.promoted_visuals.iter().position(|v| v.entity_id == e) {
            let promoted = &mut self.promoted_visuals[pos];
            if !promoted.is_visible {
                let _ = unsafe {
                    self.root_visual
                        .AddVisual(&promoted.visual, false, &self.wgpu_visual)
                };
                promoted.is_visible = true;
            }
        } else {
            let _ = unsafe {
                self.root_visual
                    .AddVisual(&ex_visual, false, &self.wgpu_visual)
            };
            self.promoted_visuals.push(PromotedVisual {
                entity_id: e,
                visual: ex_visual,
                transform: None,
                is_visible: true,
            });
        }
    }
}
