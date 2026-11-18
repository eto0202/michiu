use crate::{CpuTextureView, ExternalTexture, ExternalTextureMetadata, LayoutRect, LayoutSize};
use std::sync::Arc;
use windows::Win32::{
    Foundation::{HWND, LPARAM, POINT, WPARAM},
    Graphics::{DirectComposition::IDCompositionVisual2, Imaging::IWICImagingFactory},
};

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

    fn resolve_cpu(&self, _f: &mut dyn FnMut(CpuTextureView<'_>)) {
        todo!()
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
    pub device: Option<&'a wgpu::Device>,
    pub queue: Option<&'a wgpu::Queue>,
    pub wic_factory: &'a IWICImagingFactory,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ExternalVisualMetadata {
    /// Recommended Initial Size
    pub size: LayoutSize,
    /// Whether to leave the rounded-corner clipping on the Direct Composition side to the engine
    pub auto_clip: bool,
    /// Whether to leave the affine transformation on the Direct Composition side to the engine
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
