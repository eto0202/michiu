use crate::{Renderer, RendererUserConfig};
use michiu_ui::{Context, LayoutSize};
use windows::Win32::Graphics::DirectComposition::IDCompositionVisual2;

pub struct CpuRenderer {}

impl Renderer for CpuRenderer {
    fn resize(&mut self, p_width: u32, p_height: u32, scale_factor: f32) {
        todo!()
    }

    fn render(&mut self, cx: &mut Context, scale_factor: f32) {
        todo!()
    }
}

impl CpuRenderer {
    #[inline]
    #[must_use]
    pub fn new(visual: &IDCompositionVisual2, size: LayoutSize, scale_factor: f32) -> Self {
        Self::with_config(visual, size, scale_factor, &RendererUserConfig::default())
    }

    #[inline]
    #[must_use]
    pub fn with_config(
        visual: &IDCompositionVisual2,
        size: LayoutSize,
        scale_factor: f32,
        user_config: &RendererUserConfig,
    ) -> Self {
        todo!()
    }
}
