use std::{path::Path, sync::OnceLock};

use crate::app::theme::Theme;
use image::{DynamicImage, ImageBuffer};
pub use michiu_ui::prelude::*;
use michiu_ui::{ExternalTexture, ExternalTextureAlphaMode, ExternalTextureMetadata, UserSelect};
use tiff_reader::TiffFile;

pub fn container() -> Element {
    v_flex(ts().gap(16.0).p(16.0).r(4.0).size_full()).children([image_list()])
}

fn image_list() -> Element {
    let png_path = concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/examples/sample_collection/assets/screen_shot_1.png"
    );
    let jpg_path = concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/examples/sample_collection/assets/screen_shot_2.jpg"
    );
    let webp_path = concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/examples/sample_collection/assets/screen_shot_3.webp"
    );
    let bmp_path = concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/examples/sample_collection/assets/screen_shot_4.bmp"
    );
    // YCbCr 形式のため tiff-reader を使用
    let tiff_path = concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/examples/sample_collection/assets/screen_shot_5.tiff"
    );
    let screen_shot_1 = ImageExternalTexture::new(png_path).unwrap();
    let screen_shot_2 = ImageExternalTexture::new(jpg_path).unwrap();
    let screen_shot_3 = ImageExternalTexture::new(webp_path).unwrap();
    let screen_shot_4 = ImageExternalTexture::new(bmp_path).unwrap();
    let screen_shot_5 = ImageExternalTexture::to_ycbcr_tiff(tiff_path).unwrap();

    // ここではシンプルに screen_shot_1 のサイズをベースとする
    let size = screen_shot_1.metadata.size;

    let (selected_image, set_selected_image) = create_signal(None::<ImageExternalTexture>);

    let thumbnail = |screen_shot: ImageExternalTexture| {
        external_texture(screen_shot.clone())
            .style(
                ts().r(4.0)
                    .size((size.width / 7.0, size.height / 7.0))
                    .hovered(
                        ts().outline_solid(2.0)
                            .outline_color(dynamic(|t: &Theme| t.primary)),
                    ),
            )
            .on_click(move || {
                set_selected_image.set(Some(screen_shot.clone()));
            })
    };

    h_flex(ts().size_full()).children([
        v_flex(
            ts().gap(8.0)
                .h_full()
                .w(250.0)
                .items_center()
                .justify_center()
                .border_right(BorderStyle::Solid, 1.0)
                .border_color(dynamic(|t: &Theme| t.border)),
        )
        .children([
            thumbnail(screen_shot_1.clone()),
            thumbnail(screen_shot_2.clone()),
            thumbnail(screen_shot_3.clone()),
            thumbnail(screen_shot_4.clone()),
            thumbnail(screen_shot_5.clone()),
        ]),
        v_flex(
            ts().size_full()
                .p(16.0)
                .gap(16.0)
                .items_center()
                .justify_start(),
        )
        .children([
            div_n().child(move || {
                let base = ts().size((size.width / 2.0, size.height / 2.0));
                if let Some(img) = selected_image.get() {
                    external_texture(img).style(&base)
                } else {
                    div(&base)
                }
            }),
            text(move || {
                if let Some(img) = selected_image.get() {
                    img.file_name
                } else {
                    String::new()
                }
            })
            .style(
                ts().text_center()
                    .user_select(UserSelect::Text)
                    .text_color(dynamic(|t: &Theme| t.text_muted)),
            ),
        ]),
    ])
}

#[derive(Debug, Clone)]
pub struct ImageExternalTexture {
    // デコード済みの画像データ
    rgba_image: image::RgbaImage,
    // 初回描画時に作成し、以降再利用するGPUテクスチャビューのキャッシュ
    cached_view: OnceLock<wgpu::TextureView>,
    metadata: ExternalTextureMetadata,
    file_name: String,
}

impl ImageExternalTexture {
    pub fn new(path: impl AsRef<Path>) -> Result<Self, Box<dyn std::error::Error>> {
        let path_ref = path.as_ref();

        let file_name = Self::extract_file_name(path_ref);

        let img = image::open(path_ref)?;
        let rgba_image = img.to_rgba8();

        let (width, height) = rgba_image.dimensions();
        let size = LayoutSize::new(width as f32, height as f32);

        let metadata = ExternalTextureMetadata {
            size,
            alpha_mode: ExternalTextureAlphaMode::Straight,
            y_flip: false,
            is_srgb: true,
        };

        Ok(Self {
            rgba_image,
            cached_view: OnceLock::new(),
            metadata,
            file_name,
        })
    }

    pub fn to_ycbcr_tiff(path: impl AsRef<Path>) -> Result<Self, Box<dyn std::error::Error>> {
        let path_ref = path.as_ref();

        let file_name = Self::extract_file_name(path_ref);

        let tiff = TiffFile::open(path_ref)?;
        let ifd = tiff.ifd(0)?;

        let width = ifd.width();
        let height = ifd.height();
        let size = LayoutSize::new(width as f32, height as f32);

        // YCbCr を RGB としてデコードしている？
        let rgb_bytes = tiff.read_decoded_image_bytes(0)?;
        let buffer =
            ImageBuffer::<image::Rgb<u8>, Vec<u8>>::from_raw(width, height, rgb_bytes).unwrap();
        let rgba_image = DynamicImage::ImageRgb8(buffer).to_rgba8();

        let metadata = ExternalTextureMetadata {
            size,
            alpha_mode: ExternalTextureAlphaMode::Straight,
            y_flip: false,
            is_srgb: true,
        };

        Ok(Self {
            rgba_image,
            cached_view: OnceLock::new(),
            metadata,
            file_name,
        })
    }

    fn extract_file_name(path: impl AsRef<Path>) -> String {
        path.as_ref()
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_default()
    }
}

impl ExternalTexture for ImageExternalTexture {
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
