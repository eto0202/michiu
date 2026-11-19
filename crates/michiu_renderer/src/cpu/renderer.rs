#![expect(clippy::too_many_lines, clippy::many_single_char_names)]

use crate::{MichiuRendererResult, Renderer, RendererUserConfig, dcomp::PlatformContext};
use michiu_ui::{
    AtlasFormat, AtlasStorage, Atlases, BatchType, Context, Entity, ExternalTexture, LayoutRect,
    LayoutSize, QuadInstance, RenderData, RenderQueue, RendererView, TextureAtlas,
};
use rustc_hash::FxHashMap;
use std::sync::Arc;
use tiny_skia::{BlendMode, FilterQuality, Mask, Paint, Pattern, Pixmap, PixmapRef, SpreadMode};
use windows::{
    Win32::Graphics::{
        Direct3D11::{ID3D11DeviceContext, ID3D11Texture2D},
        DirectComposition::IDCompositionVisual2,
        Dxgi::{
            Common::{DXGI_ALPHA_MODE_PREMULTIPLIED, DXGI_FORMAT_B8G8R8A8_UNORM, DXGI_SAMPLE_DESC},
            DXGI_PRESENT, DXGI_SCALING_STRETCH, DXGI_SWAP_CHAIN_DESC1, DXGI_SWAP_CHAIN_FLAG,
            DXGI_SWAP_EFFECT_FLIP_SEQUENTIAL, DXGI_USAGE_RENDER_TARGET_OUTPUT, IDXGIAdapter,
            IDXGIDevice, IDXGIFactory2, IDXGISwapChain1,
        },
    },
    core::Interface,
};

pub struct CpuRenderer {
    d3d11_context: ID3D11DeviceContext,
    swap_chain: IDXGISwapChain1,
    pixmap: Pixmap,
    clip_mask: Mask,
    bgra_staging: Vec<u8>,
    p_width: u32,
    p_height: u32,
    instance_staging: Vec<QuadInstance>,
    atlases: Atlases,
    render_data: RenderData,
    external_textures: FxHashMap<Entity, Arc<dyn ExternalTexture>>,
    scratch_buf: Vec<u8>,
}

// shader.wgsl の定数に対応
const MODE_DECORATION_THRESHOLD: f32 = -0.5;
const MODE_ATLAS: f32 = 2.0;
const MODE_EXTERNAL_TEXTURE: f32 = 4.0;

const ATLAS_COLOR_EMOJI: f32 = 0.5;
const ATLAS_TEXT_MASK: f32 = 1.5;

impl CpuRenderer {
    #[inline]
    pub fn new(
        visual: &IDCompositionVisual2,
        size: LayoutSize,
        scale_factor: f32,
    ) -> MichiuRendererResult<Self> {
        Self::with_config(visual, size, scale_factor, &RendererUserConfig::default())
    }

    pub fn with_config(
        visual: &IDCompositionVisual2,
        size: LayoutSize,
        scale_factor: f32,
        _user_config: &RendererUserConfig,
    ) -> MichiuRendererResult<Self> {
        let platform = PlatformContext::new()?;
        let d3d11 = &platform.d3d11;

        // 0x0 だと Pixmap::new / CreateSwapChainForComposition が失敗するため 1 以上にクランプ
        let p_width = ((size.width * scale_factor).round() as u32).max(1);
        let p_height = ((size.height * scale_factor).round() as u32).max(1);

        let d3d11_context = unsafe { d3d11.GetImmediateContext()? };
        let dxgi_device: IDXGIDevice = d3d11.cast()?;
        let adapter: IDXGIAdapter = unsafe { dxgi_device.GetAdapter()? };
        let factory: IDXGIFactory2 = unsafe { adapter.GetParent()? };

        let desc = DXGI_SWAP_CHAIN_DESC1 {
            Width: p_width,
            Height: p_height,
            Format: DXGI_FORMAT_B8G8R8A8_UNORM,
            Stereo: false.into(),
            SampleDesc: DXGI_SAMPLE_DESC {
                Count: 1,
                Quality: 0,
            },
            BufferUsage: DXGI_USAGE_RENDER_TARGET_OUTPUT,
            BufferCount: 2,
            Scaling: DXGI_SCALING_STRETCH,
            SwapEffect: DXGI_SWAP_EFFECT_FLIP_SEQUENTIAL,
            AlphaMode: DXGI_ALPHA_MODE_PREMULTIPLIED, // Backdrop 透過対応
            Flags: 0,
        };

        let swap_chain = unsafe { factory.CreateSwapChainForComposition(d3d11, &desc, None)? };

        // DComp の Visual にアタッチ（呼び出し元の platform.commit() で確定）
        unsafe { visual.SetContent(&swap_chain)? };

        let pixmap = Pixmap::new(p_width, p_height).expect("non-zero dimensions");
        let clip_mask = Mask::new(p_width, p_height).expect("non-zero dimensions");
        let bgra_staging = vec![0u8; (p_width * p_height * 4) as usize];

        Ok(Self {
            d3d11_context,
            swap_chain,
            pixmap,
            clip_mask,
            bgra_staging,
            p_width,
            p_height,
            instance_staging: Vec::with_capacity(64),
            atlases: Atlases::new_cpu(),
            render_data: RenderData::new(),
            external_textures: FxHashMap::default(),
            scratch_buf: Vec::new(),
        })
    }

    fn present(&mut self) -> MichiuRendererResult<()> {
        // RGBA (tiny-skia) -> BGRA (DXGI) 変換
        self.bgra_staging
            .as_chunks_mut::<4>()
            .0
            .iter_mut()
            .zip(self.pixmap.data().as_chunks::<4>().0)
            .for_each(|(dst, src)| {
                dst[0] = src[2]; // B
                dst[1] = src[1]; // G
                dst[2] = src[0]; // R
                dst[3] = src[3]; // A
            });

        unsafe {
            let back_buffer: ID3D11Texture2D = self.swap_chain.GetBuffer(0)?;
            self.d3d11_context.UpdateSubresource(
                &back_buffer,
                0,
                None,
                self.bgra_staging.as_ptr().cast(),
                self.p_width * 4,
                0,
            );
            self.swap_chain.Present(1, DXGI_PRESENT(0)).ok()?;
        }
        Ok(())
    }

    #[inline]
    pub(crate) fn update_external_texture(&mut self, cx: &mut Context) {
        for (e, provider) in &cx.external_textures() {
            provider.resolve_cpu(&mut |tex| {});
        }
    }
}

impl Renderer for CpuRenderer {
    fn resize(&mut self, p_width: u32, p_height: u32, _scale_factor: f32) {
        let w = p_width.max(1);
        let h = p_height.max(1);
        if self.p_width == w && self.p_height == h {
            return;
        }

        self.p_width = w;
        self.p_height = h;
        self.pixmap = Pixmap::new(w, h).expect("non-zero dimensions");
        self.clip_mask = Mask::new(w, h).expect("non-zero dimensions");
        self.bgra_staging.resize((w * h * 4) as usize, 0);

        unsafe {
            let _ = self.swap_chain.ResizeBuffers(
                2,
                w,
                h,
                DXGI_FORMAT_B8G8R8A8_UNORM,
                DXGI_SWAP_CHAIN_FLAG::default(),
            );
        }
    }

    fn render(&mut self, cx: &mut Context, scale_factor: f32) {
        let _context_guard = cx.bind_context();

        // 破棄された要素のキャッシュを解放
        for e in cx.external_visual_despawned_queue().drain(..) {
            self.external_textures.remove(&e);
        }

        // 前面と背面に分類されたバッチを Context から引き出す
        cx.collect_render_data(&mut RendererView {
            render_data: &mut self.render_data,
            atlases: &mut self.atlases,
            queue: &RenderQueue::Cpu,
        });

        if self.render_data.batches.is_empty() {
            self.pixmap.fill(tiny_skia::Color::TRANSPARENT);
            let _ = self.present();
            return;
        }

        let render_data = std::mem::take(&mut self.render_data);

        self.instance_staging.clear();
        for (i, instance) in render_data.instances.iter().enumerate() {
            let e = render_data.entity_ids[i];
            let inst = cx.build_quad_instance_for_entity(e, instance);
            self.instance_staging.push(inst);
        }

        self.update_external_texture(cx);

        self.pixmap.fill(tiny_skia::Color::TRANSPARENT);

        for batch in &render_data.batches {
            if batch.instance_count == 0 {
                continue;
            }

            let start = batch.instance_offset;
            let end = start + batch.instance_count;
            let instances = &self.instance_staging[start..end];

            // シザー矩形から Mask を準備
            let mask = prepare_scissor_mask(
                &mut self.clip_mask,
                batch.scissor_rect,
                scale_factor,
                self.p_width,
                self.p_height,
            );

            let blend_mode = match batch.batch_type {
                BatchType::Normal => BlendMode::SourceOver,
                BatchType::Punchout => BlendMode::DestinationOut,
            };

            for instance in instances {
                draw_instance(
                    &mut self.pixmap,
                    instance,
                    &self.atlases,
                    &mut self.scratch_buf,
                    scale_factor,
                    blend_mode,
                    mask,
                );
            }
        }

        let _ = self.present();

        self.render_data = render_data;
    }
}

#[expect(
    clippy::cast_precision_loss,
    clippy::cast_sign_loss,
    clippy::cast_possible_truncation
)]
fn prepare_scissor_mask(
    clip_mask: &mut Mask,
    clip: LayoutRect,
    scale_factor: f32,
    p_width: u32,
    p_height: u32,
) -> Option<&Mask> {
    let w = p_width as f32;
    let h = p_height as f32;

    let round_clamp = |clip: f32, max: f32| (clip * scale_factor).round().clamp(0.0, max) as u32;

    let x0 = round_clamp(clip.x, w);
    let y0 = round_clamp(clip.y, h);
    let x1 = round_clamp(clip.x + clip.width, w);
    let y1 = round_clamp(clip.y + clip.height, h);

    if x0 == 0 && y0 == 0 && x1 >= p_width && y1 >= p_height {
        return None;
    }

    clip_mask.clear();

    if x0 < x1 && y0 < y1 {
        let w = p_width as usize;
        let data = clip_mask.data_mut();

        for y in y0 as usize..y1 as usize {
            let row_start = y * w + x0 as usize;
            let row_end = y * w + x1 as usize;

            data[row_start..row_end].fill(255);
        }
    }
    Some(clip_mask)
}

fn draw_instance(
    pixmap: &mut Pixmap,
    instance: &QuadInstance,
    atlases: &Atlases,
    scratch_buf: &mut Vec<u8>,
    scale_factor: f32,
    blend_mode: BlendMode,
    mask: Option<&Mask>,
) {
    let mode = instance.opacity_mode_sizing_border[1];

    if (mode - MODE_ATLAS).abs() < 0.1 {
        draw_atlas_quad(
            pixmap,
            instance,
            atlases,
            scratch_buf,
            scale_factor,
            blend_mode,
            mask,
        );
        return;
    }

    if (mode - MODE_EXTERNAL_TEXTURE).abs() < 0.1 {
        return;
    }

    draw_solid_rect(pixmap, instance, scale_factor, blend_mode, mask);
}

#[expect(
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss,
    clippy::cast_precision_loss
)]
fn draw_atlas_quad(
    pixmap: &mut Pixmap,
    instance: &QuadInstance,
    atlases: &Atlases,
    scratch_buf: &mut Vec<u8>,
    scale_factor: f32,
    blend_mode: BlendMode,
    mask: Option<&Mask>,
) {
    let opacity = instance.opacity_mode_sizing_border[0].clamp(0.0, 1.0);
    if opacity <= 0.0 {
        return;
    }

    let Some(shape_transform) = build_instance_transform(instance, scale_factor) else {
        return;
    };
    let Some(local_rect) =
        tiny_skia::Rect::from_xywh(0.0, 0.0, instance.rect.width, instance.rect.height)
    else {
        return;
    };

    let atlas_type = instance.is_color_atlas_pad[0];
    let atlas: &TextureAtlas = if atlas_type < ATLAS_COLOR_EMOJI {
        &atlases.color_atlas
    } else if atlas_type < ATLAS_TEXT_MASK {
        &atlases.mask_atlas
    } else {
        &atlases.user_atlas
    };

    let AtlasStorage::Cpu { data } = &atlas.storage else {
        return;
    };

    let atlas_size = atlas.size as f32;
    let max_coord = atlas.size.cast_signed();

    let uv_min_clamped = |i: usize| {
        (instance.uv_min[i] * atlas_size)
            .round()
            .clamp(0.0, atlas_size) as i32
    };
    let uv_max_clamped = |i: usize| {
        (instance.uv_max[i] * atlas_size)
            .round()
            .clamp(0.0, atlas_size) as i32
    };

    let tex_x0 = uv_min_clamped(0);
    let tex_y0 = uv_min_clamped(1);

    let tex_x1 = uv_max_clamped(0);
    let tex_y1 = uv_max_clamped(1);

    if tex_x1 <= tex_x0 || tex_y1 <= tex_y0 || tex_x1 > max_coord || tex_y1 > max_coord {
        return;
    }

    let tex_w = (tex_x1 - tex_x0) as u32;
    let tex_h = (tex_y1 - tex_y0) as u32;
    let required_bytes = tex_w * tex_h * 4;
    scratch_buf.resize(required_bytes as usize, 0);

    let x0 = tex_x0 as usize;
    let y0 = tex_y0 as usize;
    let w = tex_w as usize;

    let pattern_opacity = match atlas.atlas_types.to_format() {
        AtlasFormat::Rgba8 => {
            // color_atlas: Straight RGBA8 -> Premultiplied RGBA8
            let src_stride = atlas.size as usize * 4;

            for row in 0..tex_h as usize {
                let src_row_start = (y0 + row) * src_stride + x0 * 4;
                let dst_row_start = row * w * 4;

                for col in 0..w {
                    let s = src_row_start + col * 4;
                    let d = dst_row_start + col * 4;
                    let sa = data[s + 3];
                    match sa {
                        0 => {
                            scratch_buf[d..d + 4].fill(0);
                        }
                        255 => {
                            scratch_buf[d..d + 4].copy_from_slice(&data[s..s + 4]);
                        }
                        _ => {
                            let a16 = u16::from(sa);
                            scratch_buf[d] = ((u16::from(data[s]) * a16 + 127) / 255) as u8;
                            scratch_buf[d + 1] = ((u16::from(data[s + 1]) * a16 + 127) / 255) as u8;
                            scratch_buf[d + 2] = ((u16::from(data[s + 2]) * a16 + 127) / 255) as u8;
                            scratch_buf[d + 3] = sa;
                        }
                    }
                }
            }
            opacity
        }
        AtlasFormat::R8 => {
            // mask_atlas / user_atlas: R8 マスク × instance.color (sRGB) -> Premultiplied RGBA8
            let [r, g, b, a]: [f32; 4] = bytemuck::cast(instance.color);
            let final_a = (a * opacity).clamp(0.0, 1.0);
            if final_a <= 0.0 {
                return;
            }
            let r_clamped = r.clamp(0.0, 1.0);
            let g_clamped = g.clamp(0.0, 1.0);
            let b_clamped = b.clamp(0.0, 1.0);

            let src_stride = atlas.size as usize;

            for row in 0..tex_h as usize {
                let src_row_start = (y0 + row) * src_stride + x0;
                let dst_row_start = row * w * 4;

                for col in 0..w {
                    let mask_val = data[src_row_start + col];
                    let d = dst_row_start + col * 4;
                    if mask_val == 0 {
                        scratch_buf[d..d + 4].fill(0);
                    } else {
                        let cov = (f32::from(mask_val) / 255.0) * final_a;
                        scratch_buf[d] = (r_clamped * cov * 255.0 + 0.5) as u8;
                        scratch_buf[d + 1] = (g_clamped * cov * 255.0 + 0.5) as u8;
                        scratch_buf[d + 2] = (b_clamped * cov * 255.0 + 0.5) as u8;
                        scratch_buf[d + 3] = (cov * 255.0 + 0.5) as u8;
                    }
                }
            }
            1.0
        }
    };

    let Some(sub_pixmap) = PixmapRef::from_bytes(scratch_buf, tex_w, tex_h) else {
        return;
    };

    // テクセル空間 [0..tex_w, 0..tex_h] -> ローカル論理空間 [0..rect.width, 0..rect.height]
    let pattern_transform = tiny_skia::Transform::from_scale(
        instance.rect.width / tex_w as f32,
        instance.rect.height / tex_h as f32,
    );

    let paint = Paint {
        shader: Pattern::new(
            sub_pixmap,
            SpreadMode::Pad,
            FilterQuality::Bilinear,
            pattern_opacity,
            pattern_transform,
        ),
        blend_mode,
        anti_alias: false, // 無いほうが見やすいかも
        ..Default::default()
    };

    pixmap.fill_rect(local_rect, &paint, shape_transform, mask);
}

fn draw_solid_rect(
    pixmap: &mut Pixmap,
    instance: &QuadInstance,
    scale_factor: f32,
    blend_mode: BlendMode,
    mask: Option<&Mask>,
) {
    let opacity = instance.opacity_mode_sizing_border[0];
    let mode = instance.opacity_mode_sizing_border[1];

    // アトラス・外部テクスチャは一旦スキップ
    if (mode - MODE_ATLAS).abs() < 0.1 || (mode - MODE_EXTERNAL_TEXTURE).abs() < 0.1 {
        return;
    }

    let [r, g, b, a]: [f32; 4] = bytemuck::cast(instance.color);
    let final_alpha = (a * opacity).clamp(0.0, 1.0);

    // Normal 時で完全透明ならスキップ（Punchout は alpha > 0 の領域をくり抜くため同じ判定）
    if final_alpha <= 0.0 {
        return;
    }

    let Some(transform) = build_instance_transform(instance, scale_factor) else {
        return;
    };

    let Some(local_rect) =
        tiny_skia::Rect::from_xywh(0.0, 0.0, instance.rect.width, instance.rect.height)
    else {
        return;
    };

    let Some(color) = tiny_skia::Color::from_rgba(
        r.clamp(0.0, 1.0),
        g.clamp(0.0, 1.0),
        b.clamp(0.0, 1.0),
        final_alpha,
    ) else {
        return;
    };

    let mut paint = Paint::default();
    paint.set_color(color);
    paint.blend_mode = blend_mode;
    paint.anti_alias = mode >= MODE_DECORATION_THRESHOLD;

    pixmap.fill_rect(local_rect, &paint, transform, mask);
}

fn build_instance_transform(
    instance: &QuadInstance,
    scale_factor: f32,
) -> Option<tiny_skia::Transform> {
    let rect = instance.rect;
    if rect.width <= 0.0 || rect.height <= 0.0 {
        return None;
    }

    let m = instance.transform;
    let sx = m[0][0];
    let ky = m[0][1];
    let kx = m[1][0];
    let sy = m[1][1];
    let m_tx = m[2][0];
    let m_ty = m[2][1];

    // 面積が 0 になる変換行列（未初期化ゼロ行列や scale(0,0)）は描画不可としてスキップ
    let det = sx * sy - kx * ky;
    if det.abs() <= f32::EPSILON {
        return None;
    }

    let ox = instance.transform_origin[0] * rect.width;
    let oy = instance.transform_origin[1] * rect.height;

    let local_tx = ox - (sx * ox + kx * oy) + m_tx;
    let local_ty = oy - (ky * ox + sy * oy) + m_ty;

    let scaled = |val: f32| val * scale_factor;

    Some(tiny_skia::Transform::from_row(
        scaled(sx),
        scaled(ky),
        scaled(kx),
        scaled(sy),
        scaled(rect.x + local_tx),
        scaled(rect.y + local_ty),
    ))
}
