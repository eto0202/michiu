use crate::{
    Context, ExternalTexture, ExternalTextureMetadata, LayoutPoint, LayoutRect, LayoutSize,
    MichiuSoA,
};
use std::sync::Arc;
use windows::Win32::Foundation::{HWND, LPARAM, POINT, WPARAM};
use windows::Win32::Graphics::DirectComposition::IDCompositionVisual2;
use windows::Win32::Graphics::Imaging::IWICImagingFactory;

/// An `ExternalTexture` implementation that holds a single static image in a `TextureView`
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

/// A trait for providing `IDCompositionVisual2`
pub trait ExternalVisual: Send + Sync {
    /// Returns an `IDCompositionVisual2` to be placed in the `DirectComposition` tree.
    fn resolve_visual(&self) -> IDCompositionVisual2;

    /// If a static texture exists, it is returned as an `ExternalTexture`.
    /// When set to `Some`, it is rendered as a normal texture, and the `DComp` side is made invisible.
    /// When set to `None`, it is considered active, and `DComp` mounting and punchout are performed.
    fn static_texture(&self) -> Option<Arc<dyn ExternalTexture>> {
        None
    }

    /// Layout and State Update Hooks for Each Frame
    fn update(&self, _cx: &VisualUpdateContext) {}

    /// A generic hook that transparently forwards raw input from the OS.
    /// Returns `true` if the event is consumed, and `false` if it is passed through
    ///  (for normal processing by the library).
    fn handle_raw_input(
        &self,
        _msg: u32,
        _wparam: WPARAM,
        _lparam: LPARAM,
        _local_phys_pos: POINT,
    ) -> bool {
        false
    }

    /// Retrieves metadata.
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

/// The context passed to `ExternalVisual` at each frame update
pub struct VisualUpdateContext<'a> {
    pub hwnd: HWND,
    pub rect: LayoutRect,
    pub scale_factor: f32,
    pub is_interactive: bool,
    pub is_stable: bool,
    pub device: &'a wgpu::Device,
    pub queue: &'a wgpu::Queue,
    pub wic_factory: &'a IWICImagingFactory,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ExternalVisualMetadata {
    /// Recommended Initial Size
    pub size: LayoutSize,
    /// Whether to leave the rounded-corner clipping on the DComp side to the engine
    pub auto_clip: bool,
    /// Whether to leave the affine transformation on the DComp side to the engine
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

/// Identifies the topmost `ExternalVisual` based on physical pixel coordinates and forwards the raw input.
/// Returns `true` if the event has been consumed.
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
