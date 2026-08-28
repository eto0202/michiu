use std::{path::Path, sync::OnceLock};

use crate::app::theme::Theme;
pub use michiu_ui::prelude::*;
use michiu_ui::{ExternalTexture, ExternalTextureAlphaMode, ExternalTextureMetadata};

pub fn container() -> Element {
    v_flex(ts().gap(16.0).p(16.0).r(4.0)).children([png()])
}

fn png() -> Element {
    let png_path = concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/examples/sample_collection/assets/screen-shot.png"
    );
    let texture = PngExternalTexture::new(png_path).unwrap();

    external_texture(texture).style(ts())
}

pub struct PngExternalTexture {
    // デコード済みの画像データ
    rgba_image: image::RgbaImage,
    // 初回描画時に作成し、以降再利用するGPUテクスチャビューのキャッシュ
    cached_view: OnceLock<wgpu::TextureView>,
    metadata: ExternalTextureMetadata,
}

impl PngExternalTexture {
    pub fn new(path: impl AsRef<Path>) -> Result<Self, Box<dyn std::error::Error>> {
        let img = image::open(path)?;
        let rgba_image = img.to_rgba8();
        let (width, height) = rgba_image.dimensions();

        let size = LayoutSize::new(width as f32, height as f32);

        let metadata = ExternalTextureMetadata {
            size,
            alpha_mode: ExternalTextureAlphaMode::Straight,
            y_flip: false,
        };

        Ok(Self {
            rgba_image,
            cached_view: OnceLock::new(),
            metadata,
        })
    }
}

impl ExternalTexture for PngExternalTexture {
    fn resolve_view(&self, device: &wgpu::Device, queue: &wgpu::Queue) -> wgpu::TextureView {
        self.cached_view
            .get_or_init(|| {
                let width = self.rgba_image.width();
                let height = self.rgba_image.height();
                let size = wgpu::Extent3d {
                    width,
                    height,
                    depth_or_array_layers: 1,
                };

                let texture = device.create_texture(&wgpu::TextureDescriptor {
                    label: Some("PngExternalTexture"),
                    size,
                    mip_level_count: 1,
                    sample_count: 1,
                    dimension: wgpu::TextureDimension::D2,
                    format: wgpu::TextureFormat::Rgba8UnormSrgb,
                    usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
                    view_formats: &[],
                });

                queue.write_texture(
                    wgpu::TexelCopyTextureInfo {
                        texture: &texture,
                        mip_level: 0,
                        origin: wgpu::Origin3d::ZERO,
                        aspect: wgpu::TextureAspect::All,
                    },
                    &self.rgba_image,
                    wgpu::TexelCopyBufferLayout {
                        offset: 0,
                        bytes_per_row: Some(4 * width),
                        rows_per_image: Some(height),
                    },
                    size,
                );

                texture.create_view(&wgpu::TextureViewDescriptor::default())
            })
            .clone()
    }

    fn metadata(&self) -> ExternalTextureMetadata {
        self.metadata
    }
}
