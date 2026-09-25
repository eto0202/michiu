use crate::{
    Context, ExternalTexture, ExternalTextureMetadata, LayoutPoint, LayoutRect, LayoutSize,
    MichiuSoA,
};
use std::sync::Arc;
use windows::Win32::Foundation::{HWND, LPARAM, POINT, WPARAM};
use windows::Win32::Graphics::DirectComposition::IDCompositionVisual2;
use windows::Win32::Graphics::Imaging::IWICImagingFactory;

/// 単一の静止画像 `TextureView` を保持する `ExternalTexture` 実装
#[derive(Debug, Clone)]
pub struct StaticExternalTexture {
    pub view: wgpu::TextureView,
    pub metadata: ExternalTextureMetadata,
}

impl ExternalTexture for StaticExternalTexture {
    fn resolve_view(&self, _device: &wgpu::Device, _queue: &wgpu::Queue) -> wgpu::TextureView {
        self.view.clone()
    }

    fn metadata(&self) -> ExternalTextureMetadata {
        self.metadata
    }
}

// ====================================================================================
// ====================================================================================

/// `IDCompositionVisual2` を供給するためのトレイト
pub trait ExternalVisual: Send + Sync {
    /// `DirectComposition` ツリーに載せる `IDCompositionVisual2` を返します。
    fn resolve_visual(&self) -> IDCompositionVisual2;

    /// 静止テクスチャが存在する場合は `ExternalTexture` として返します。
    /// `Some` の間は通常のテクスチャとして描画され、DComp 側は不可視化されます。
    /// `None` の場合はアクティブ（動画・操作中）とみなされ、DComp マウント & 穴あけ（Punchout）が行われます。
    fn static_texture(&self) -> Option<Arc<dyn ExternalTexture>> {
        None
    }

    /// 毎フレームのレイアウト・状態更新フック
    fn update(&self, _cx: &VisualUpdateContext) {}

    /// OSからの生入力を透過的に転送する汎用フック。
    /// イベントを消費した場合は true、スルー（ライブラリ側で通常処理）する場合は false を返す。
    fn handle_raw_input(
        &self,
        _msg: u32,
        _wparam: WPARAM,
        _lparam: LPARAM,
        _local_phys_pos: POINT,
    ) -> bool {
        false
    }

    /// メタデータを取得します。
    fn metadata(&self) -> ExternalVisualMetadata;
}

impl<T: ExternalVisual + ?Sized> ExternalVisual for Arc<T> {
    fn resolve_visual(&self) -> IDCompositionVisual2 {
        (**self).resolve_visual()
    }

    fn static_texture(&self) -> Option<Arc<dyn ExternalTexture>> {
        (**self).static_texture()
    }

    fn update(&self, cx: &VisualUpdateContext) {
        (**self).update(cx);
    }

    fn handle_raw_input(
        &self,
        msg: u32,
        wparam: WPARAM,
        lparam: LPARAM,
        local_phys_pos: POINT,
    ) -> bool {
        (**self).handle_raw_input(msg, wparam, lparam, local_phys_pos)
    }

    fn metadata(&self) -> ExternalVisualMetadata {
        (**self).metadata()
    }
}

/// `ExternalVisual` の毎フレーム更新時に渡されるコンテキスト
pub struct VisualUpdateContext<'a> {
    pub hwnd: HWND,
    pub rect: LayoutRect,
    pub scale_factor: f32,
    /// 操作中（フォーカスやマウスホバー等）かどうか
    pub is_interactive: bool,
    /// リサイズやアニメーション中ではなく、描画が完全に安定しているか
    pub is_stable: bool,
    pub device: &'a wgpu::Device,
    pub queue: &'a wgpu::Queue,
    pub wic_factory: &'a IWICImagingFactory,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ExternalVisualMetadata {
    /// 初期の推奨サイズ（レイアウトへの反映用）
    pub size: LayoutSize,
    /// DComp側での角丸クリップ（RectangleClip）をエンジン側に任せるかどうか
    pub auto_clip: bool,
    /// DComp側のアフィン変換（Transform2）をエンジン側に任せるかどうか
    pub auto_transform: bool,
}

impl Default for ExternalVisualMetadata {
    fn default() -> Self {
        Self {
            size: LayoutSize::ZERO,
            auto_clip: true,
            auto_transform: true,
        }
    }
}

/// 物理ピクセル座標を基に最前面の `ExternalVisual` を特定し、生入力を転送する。
/// イベントが消費された場合は true を返す。
pub fn dispatch_raw_input_to_external_visual(
    cx: &mut Context,
    msg: u32,
    wparam: WPARAM,
    lparam: LPARAM,
    window_phys_pos: LayoutPoint,
    scale_factor: f32,
) -> bool {
    let logical_pos = LayoutPoint::new(
        window_phys_pos.x / scale_factor,
        window_phys_pos.y / scale_factor,
    );

    // プレス中要素があればそちらを優先、なければヒット要素
    let target_id = cx
        .events
        .evt_interaction_states
        .pressed
        .or_else(|| cx.hit_test(logical_pos));

    let Some(id) = target_id else {
        return false;
    };

    if !cx
        .topology
        .topo_active_masks
        .at(id)
        .has_external_visual_content()
    {
        return false;
    }

    let Some(visual) = cx.contents.cont_external_visual.find(id).cloned() else {
        return false;
    };

    let rect = cx.outputs.out_rects.find_or_default(id, &mut cx.debug);
    let local_phys_pos = POINT {
        x: (window_phys_pos.x - rect.x * scale_factor).round() as i32,
        y: (window_phys_pos.y - rect.y * scale_factor).round() as i32,
    };

    visual.handle_raw_input(msg, wparam, lparam, local_phys_pos)
}
