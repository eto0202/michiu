use crate::LayoutSize;

/// Trait for dynamically supplying textures
pub trait ExternalTexture: Send + Sync {
    /// Called immediately before rendering,
    /// this function returns the latest [`wgpu::TextureView`] that should be rendered in this frame.
    fn resolve_view(&self, device: &wgpu::Device, queue: &wgpu::Queue) -> wgpu::TextureView;

    fn resolve_cpu(&self, _f: &mut dyn FnMut(CpuTextureView<'_>)) {}

    /// 静的画像など不変なら Some(固定値)、動的更新時はインクリメント、未管理なら None
    fn cpu_version(&self) -> Option<u64> {
        None
    }

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
    pub compositing_mode: ExternalTextureCompositingMode,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExternalTextureAlphaMode {
    /// Standard alpha. Automatically converted to PMA (multiplied alpha) within the shader.
    Straight,
    /// Multiplied alpha. Composited directly within the shader.
    Premultiplied,
}

/// Defines the color-space semantics expected when compositing
/// this external texture into the destination.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum ExternalTextureCompositingMode {
    /// Linear-light compositing, as used by the normal wgpu
    /// sRGB render-target path.
    LinearLight,

    /// Non-linear/sRGB-space compositing semantics.
    ///
    /// The value is the opacity exponent used to approximate the
    /// difference between the source compositing behavior and the
    /// wgpu render target.
    ///
    /// `1.0` means no correction.
    NonLinear(f32),
}

// CPU レンダリング用のピクセル表現
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum CpuTextureFormat {
    // RGBA8
    Rgba8,
    // 完全不透明な RGBA8
    OpaqueRgba8,
    // BGRA8
    Bgra8,
}

pub struct CpuTextureView<'a> {
    pub data: &'a [u8],
    pub format: CpuTextureFormat,
}
