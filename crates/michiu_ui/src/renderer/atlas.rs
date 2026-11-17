#![expect(clippy::unreachable, unused)]

use crate::RenderQueue;

#[derive(Debug, Clone, Copy)]
pub enum AtlasFormat {
    R8,    // 1 byte (mask_atlas, user_atlas)
    Rgba8, // 4 bytes (color_atlas)
}

impl AtlasFormat {
    pub(crate) const fn to_wgpu(self) -> wgpu::TextureFormat {
        match self {
            AtlasFormat::R8 => wgpu::TextureFormat::R8Unorm,
            AtlasFormat::Rgba8 => wgpu::TextureFormat::Rgba8UnormSrgb,
        }
    }
    pub(crate) const fn bytes_per_pixel(self) -> usize {
        match self {
            Self::R8 => 1,
            Self::Rgba8 => 4,
        }
    }
}

#[derive(Debug, Clone)]
pub enum AtlasStorage {
    Gpu {
        texture: wgpu::Texture,
        view: wgpu::TextureView,
        sampler: wgpu::Sampler,
    },
    Cpu {
        // mask / user は size * size * 1 bytes
        // color は size * size * 4 bytes
        data: Vec<u8>,
    },
}
impl AtlasStorage {
    #[inline]
    pub(crate) fn view(&self) -> &wgpu::TextureView {
        match self {
            AtlasStorage::Gpu { view, .. } => view,
            AtlasStorage::Cpu { .. } => unreachable!(),
        }
    }

    #[inline]
    pub(crate) fn sampler(&self) -> &wgpu::Sampler {
        match self {
            AtlasStorage::Gpu { sampler, .. } => sampler,
            AtlasStorage::Cpu { .. } => unreachable!(),
        }
    }

    #[inline]
    pub(crate) fn new_cpu(size: u32, atlas_format: AtlasFormat) -> Self {
        let total_bytes = size as usize * size as usize * atlas_format.bytes_per_pixel();
        Self::Cpu {
            data: vec![0; total_bytes],
        }
    }

    #[inline]
    pub(crate) fn new_gpu(size: u32, device: &wgpu::Device, format: wgpu::TextureFormat) -> Self {
        let texture = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("Texture Atlas"),
            size: wgpu::Extent3d {
                width: size,
                height: size,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format,
            usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
            view_formats: &[],
        });

        let view = texture.create_view(&wgpu::TextureViewDescriptor::default());

        let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            address_mode_u: wgpu::AddressMode::ClampToEdge,
            address_mode_v: wgpu::AddressMode::ClampToEdge,
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            ..Default::default()
        });

        Self::Gpu {
            texture,
            view,
            sampler,
        }
    }
}

#[derive(Debug, Clone)]
pub struct TextureAtlas {
    pub storage: AtlasStorage,
    pub atlas_format: AtlasFormat,
    pub size: u32,
    pub current_x: u32,
    pub current_y: u32,
    pub row_max_height: u32,
    pub padding: u32,
}

impl TextureAtlas {
    #[inline]
    pub(crate) fn new_cpu(size: u32, atlas_format: AtlasFormat) -> Self {
        let storage = AtlasStorage::new_cpu(size, atlas_format);
        Self {
            storage,
            atlas_format,
            size,
            current_x: 0,
            current_y: 0,
            row_max_height: 0,
            padding: 2,
        }
    }
    #[inline]
    pub(crate) fn new_gpu(size: u32, atlas_format: AtlasFormat, device: &wgpu::Device) -> Self {
        let storage = AtlasStorage::new_gpu(size, device, atlas_format.to_wgpu());
        Self {
            storage,
            atlas_format,
            size,
            current_x: 0,
            current_y: 0,
            row_max_height: 0,
            padding: 2, // 滲み防止用の余白
        }
    }

    #[inline]
    #[expect(clippy::cast_possible_truncation)]
    pub(crate) fn write_region(
        &mut self,
        queue: &RenderQueue<'_>,
        x: u32,
        y: u32,
        width: u32,
        height: u32,
        src: &[u8],
    ) {
        match (&mut self.storage, queue) {
            (AtlasStorage::Gpu { texture, .. }, RenderQueue::Gpu(q)) => {
                q.write_texture(
                    wgpu::TexelCopyTextureInfo {
                        texture,
                        mip_level: 0,
                        origin: wgpu::Origin3d { x, y, z: 0 },
                        aspect: wgpu::TextureAspect::All,
                    },
                    src,
                    wgpu::TexelCopyBufferLayout {
                        offset: 0,
                        bytes_per_row: Some(width * self.atlas_format.bytes_per_pixel() as u32),
                        rows_per_image: Some(height),
                    },
                    wgpu::Extent3d {
                        width,
                        height,
                        depth_or_array_layers: 1,
                    },
                );
            }
            (AtlasStorage::Cpu { data }, RenderQueue::Cpu) => {
                let bytes_per_pixel = self.atlas_format.bytes_per_pixel();

                let row_bytes = width as usize * bytes_per_pixel;
                let atlas_stride = self.size as usize * bytes_per_pixel;

                for row in 0..height as usize {
                    let dst_start =
                        (y as usize + row) * atlas_stride + x as usize * bytes_per_pixel;
                    let src_start = row * row_bytes;
                    data[dst_start..dst_start + row_bytes]
                        .copy_from_slice(&src[src_start..src_start + row_bytes]);
                }
            }
            _ => unreachable!(),
        }
    }

    /// 新しい要素のための領域をアトラス上に確保し、座標を返す
    pub(crate) fn allocate(&mut self, width: u32, height: u32) -> Option<(u32, u32)> {
        let padded_w = width + self.padding;
        let padded_h = height + self.padding;

        // 横幅が足りない場合は次の段へ
        if self.current_x + padded_w > self.size {
            self.current_x = 0;
            self.current_y += self.row_max_height;
            self.row_max_height = 0;
        }

        // 縦幅が足りない場合は失敗（アトラス溢れ）
        if self.current_y + padded_h > self.size {
            return None;
        }

        let x = self.current_x;
        let y = self.current_y;

        // 状態更新
        self.current_x += padded_w;
        self.row_max_height = self.row_max_height.max(padded_h);

        Some((x, y))
    }

    #[inline]
    pub(crate) fn clear(&mut self) {
        self.current_x = 0;
        self.current_y = 0;
        self.row_max_height = 0;
        if let AtlasStorage::Cpu { data } = &mut self.storage {
            data.fill(0);
        }
    }

    /// ピクセル座標を NDC (0.0 ~ 1.0) の UV 座標に変換する
    #[expect(clippy::cast_precision_loss)]
    #[inline]
    pub(crate) fn texel_to_uv(
        &self,
        x: u32,
        y: u32,
        width: u32,
        height: u32,
    ) -> ([f32; 2], [f32; 2]) {
        let s = self.size as f32;
        (
            [x as f32 / s, y as f32 / s],
            [(x + width) as f32 / s, (y + height) as f32 / s],
        )
    }
}
