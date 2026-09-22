use std::{rc::Rc, sync::Arc};

use crate::{LayoutRect, LayoutSize};
use windows::Win32::Foundation::{HMODULE, HWND, LPARAM, POINT, RECT, WPARAM};
use windows::Win32::Graphics::DirectComposition::{IDCompositionVisual2, IDCompositionVisual3};
use windows::Win32::Graphics::Imaging::IWICImagingFactory;

/// Trait for dynamically supplying textures
pub trait ExternalTexture: Send + Sync {
    /// Called immediately before rendering,
    /// this function returns the latest [`wgpu::TextureView`] that should be rendered in this frame.
    fn resolve_view(&self, device: &wgpu::Device, queue: &wgpu::Queue) -> wgpu::TextureView;

    /// Retrieve the metadata that controls the rendering method.
    fn metadata(&self) -> ExternalTextureMetadata;
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ExternalTextureMetadata {
    pub size: LayoutSize,
    pub alpha_mode: ExternalTextureAlphaMode,
    pub y_flip: bool,
    /// Whether the texture is an -Srgb-based automatic color space conversion format
    pub is_srgb: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExternalTextureAlphaMode {
    /// Standard alpha. Automatically converted to PMA (multiplied alpha) within the shader.
    Straight,
    /// Multiplied alpha. Composited directly within the shader.
    Premultiplied,
}

// ====================================================================================
// ====================================================================================

/// 単一の静止画像 `TextureView` を保持する `ExternalTexture` 実装
#[derive(Debug, Clone)]
pub struct StaticExternalTexture {
    pub view: wgpu::TextureView,
    pub metadata: ExternalTextureMetadata,
}

impl StaticExternalTexture {
    #[must_use]
    #[inline]
    pub fn new(view: wgpu::TextureView, size: LayoutSize) -> Self {
        Self {
            view,
            metadata: ExternalTextureMetadata {
                size,
                alpha_mode: ExternalTextureAlphaMode::Straight,
                y_flip: false,
                is_srgb: true, // Bgra8UnormSrgb のため
            },
        }
    }
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
