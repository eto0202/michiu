#![expect(
    clippy::too_many_lines,
    clippy::many_single_char_names,
    clippy::too_many_arguments
)]

use crate::{MichiuRendererResult, Renderer, RendererUserConfig, dcomp::PlatformContext};
use michiu_ui::{
    AtlasFormat, AtlasStorage, Atlases, BatchType, Context, Entity, ExternalTexture, LayoutRect,
    LayoutSize, QuadInstance, RenderData, RenderQueue, RendererView, TextureAtlas,
};
use rustc_hash::FxHashMap;
use std::sync::Arc;
use tiny_skia::{
    BlendMode, FillRule, FilterQuality, Mask, Paint, PathBuilder, Pattern, Pixmap, PixmapRef,
    PremultipliedColorU8, SpreadMode,
};
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

const KAPPA: f32 = 0.552_284_8;

const EDGE_TOP: usize = 0;
const EDGE_RIGHT: usize = 1;
const EDGE_BOTTOM: usize = 2;
const EDGE_LEFT: usize = 3;

// const STROKE_SOLID: u32 = 0;
const STROKE_DOTTED: u32 = 1;
const STROKE_DASHED: u32 = 2;
const STROKE_DOUBLE: u32 = 3;

const ALIGN_START: u32 = 0;
const ALIGN_END: u32 = 1;

const INVALID_EDGE_DIST: f32 = 10000.0;

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
                    batch.scissor_rect,
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
    scissor_rect: LayoutRect,
    scale_factor: f32,
    blend_mode: BlendMode,
    mask: Option<&Mask>,
) {
    let mode = instance.opacity_mode_sizing_border[1];
    let has_stroke = has_border_or_outline(instance);

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
        if has_stroke {
            draw_sdf_quad(
                pixmap,
                instance,
                scissor_rect,
                scale_factor,
                blend_mode,
                true, // entity 本体は既に描画済みなので枠線/アウトラインのみ重ねる
            );
        }
        return;
    }

    if (mode - MODE_EXTERNAL_TEXTURE).abs() < 0.1 {
        return;
    }

    if has_stroke {
        draw_sdf_quad(
            pixmap,
            instance,
            scissor_rect,
            scale_factor,
            blend_mode,
            false,
        );
    } else {
        draw_solid_rect(pixmap, instance, scale_factor, blend_mode, mask);
    }
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
        anti_alias: true,
        ..Default::default()
    };

    fill_instance_shape(pixmap, instance, &paint, shape_transform, mask);
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

    fill_instance_shape(pixmap, instance, &paint, transform, mask);
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

fn build_rounded_rect_path(width: f32, height: f32, radii: [f32; 4]) -> Option<tiny_skia::Path> {
    let max_radius = (width.min(height) * 0.5).max(0.0);
    let tl = radii[0].clamp(0.0, max_radius);
    let tr = radii[1].clamp(0.0, max_radius);
    let br = radii[2].clamp(0.0, max_radius);
    let bl = radii[3].clamp(0.0, max_radius);

    let mut pb = PathBuilder::new();
    pb.move_to(tl, 0.0);

    let kappa = |val: f32| val * KAPPA;

    // Top edge -> Top-Right
    pb.line_to(width - tr, 0.0);
    if tr > 0.0 {
        let k = kappa(tr);
        pb.cubic_to(width - tr + k, 0.0, width, tr - k, width, tr);
    }

    // Right edge -> Bottom-Right
    pb.line_to(width, height - br);
    if br > 0.0 {
        let k = kappa(br);
        pb.cubic_to(
            width,
            height - br + k,
            width - br + k,
            height,
            width - br,
            height,
        );
    }

    // Bottom edge -> Bottom-Left
    pb.line_to(bl, height);
    if bl > 0.0 {
        let k = kappa(bl);
        pb.cubic_to(bl - k, height, 0.0, height - bl + k, 0.0, height - bl);
    }

    // Left edge -> Top-Left
    pb.line_to(0.0, tl);
    if tl > 0.0 {
        let k = kappa(tl);
        pb.cubic_to(0.0, tl - k, tl - k, 0.0, tl, 0.0);
    }

    pb.close();
    pb.finish()
}

fn fill_instance_shape(
    pixmap: &mut Pixmap,
    instance: &QuadInstance,
    paint: &Paint<'_>,
    transform: tiny_skia::Transform,
    mask: Option<&Mask>,
) {
    let w = instance.rect.width;
    let h = instance.rect.height;
    let mode = instance.opacity_mode_sizing_border[1];
    let radii: [f32; 4] = bytemuck::cast(instance.corner_radius);

    let has_radius = mode >= MODE_DECORATION_THRESHOLD && radii.iter().any(|&r| r > 0.0);

    if has_radius {
        if let Some(path) = build_rounded_rect_path(w, h, radii) {
            pixmap.fill_path(&path, paint, FillRule::Winding, transform, mask);
        }
    } else if let Some(local_rect) = tiny_skia::Rect::from_xywh(0.0, 0.0, w, h) {
        pixmap.fill_rect(local_rect, paint, transform, mask);
    }
}

fn has_border_or_outline(instance: &QuadInstance) -> bool {
    let b_width: [f32; 4] = bytemuck::cast(instance.border_width);
    let b_color: [f32; 4] = bytemuck::cast(instance.border_color);
    let has_border = b_color[3] > 0.0 && (b_width[0] + b_width[1] + b_width[2] + b_width[3]) > 0.0;

    let o_width: [f32; 4] = bytemuck::cast(instance.outline_width);
    let o_color: [f32; 4] = bytemuck::cast(instance.outline_color);
    let has_outline = o_color[3] > 0.0 && (o_width[0] + o_width[1] + o_width[2] + o_width[3]) > 0.0;

    has_border || has_outline
}

#[inline]
fn wgsl_fract(x: f32) -> f32 {
    x - x.floor()
}

#[inline]
fn smoothstep(edge0: f32, edge1: f32, x: f32) -> f32 {
    let denom = edge1 - edge0;
    if denom.abs() <= f32::EPSILON {
        return if x >= edge1 { 1.0 } else { 0.0 };
    }
    let t = ((x - edge0) / denom).clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

#[inline]
fn sd_rounded_box(px: f32, py: f32, bx: f32, by: f32, r: [f32; 4]) -> f32 {
    let rad = if px >= 0.0 && py < 0.0 {
        r[1] // top_right
    } else if px >= 0.0 && py >= 0.0 {
        r[2] // bottom_right
    } else if px < 0.0 && py >= 0.0 {
        r[3] // bottom_left
    } else {
        r[0] // top_left
    };
    let qx = px.abs() - bx + rad;
    let qy = py.abs() - by + rad;
    qx.max(qy).min(0.0) + qx.max(0.0).hypot(qy.max(0.0)) - rad
}

#[inline]
fn select_closest_edge(
    local_cx: f32,
    local_cy: f32,
    half_w: f32,
    half_h: f32,
    widths: [f32; 4],
    skip_zero_width: bool,
) -> usize {
    let d_top = if !skip_zero_width || widths[EDGE_TOP] > 0.0 {
        local_cy + half_h
    } else {
        INVALID_EDGE_DIST
    };
    let d_right = if !skip_zero_width || widths[EDGE_RIGHT] > 0.0 {
        half_w - local_cx
    } else {
        INVALID_EDGE_DIST
    };
    let d_bottom = if !skip_zero_width || widths[EDGE_BOTTOM] > 0.0 {
        half_h - local_cy
    } else {
        INVALID_EDGE_DIST
    };
    let d_left = if !skip_zero_width || widths[EDGE_LEFT] > 0.0 {
        local_cx + half_w
    } else {
        INVALID_EDGE_DIST
    };

    let min_dist = d_top.min(d_bottom).min(d_left.min(d_right));
    if (min_dist - d_right).abs() == 0.0 {
        EDGE_RIGHT
    } else if (min_dist - d_bottom).abs() == 0.0 {
        EDGE_BOTTOM
    } else if (min_dist - d_left).abs() == 0.0 {
        EDGE_LEFT
    } else {
        EDGE_TOP
    }
}

#[inline]
fn compute_length_alpha(t: f32, len_limit: f32, alignment: u32, edge_fade: f32) -> f32 {
    if len_limit >= 0.999 {
        return 1.0;
    }

    match alignment {
        ALIGN_START => 1.0 - smoothstep(len_limit - edge_fade, len_limit + edge_fade, t),
        ALIGN_END => {
            let lower_bound = 1.0 - len_limit;
            smoothstep(lower_bound - edge_fade, lower_bound + edge_fade, t)
        }
        _ => {
            let half_len = len_limit * 0.5;
            let dist_from_center = (t - 0.5).abs();
            1.0 - smoothstep(half_len - edge_fade, half_len + edge_fade, dist_from_center)
        }
    }
}

#[inline]
fn compute_style_alpha(style: u32, w: f32, pos_edge: f32, thick_t: f32) -> f32 {
    match style {
        STROKE_DOTTED => {
            let period = w * 2.2;
            let cycle_t = wgsl_fract(pos_edge / period) * period - (period * 0.5);
            let radial_dist = cycle_t.hypot((thick_t - 0.5) * w);
            1.0 - smoothstep(w * 0.4 - 0.5, w * 0.4 + 0.5, radial_dist)
        }
        STROKE_DASHED => {
            let period = w * 5.0;
            let cycle_t = wgsl_fract(pos_edge / period) * period;
            1.0 - smoothstep(w * 3.0 - 0.5, w * 3.0 + 0.5, cycle_t)
        }
        STROKE_DOUBLE => {
            let is_double_void =
                smoothstep(0.30, 0.33, thick_t) * (1.0 - smoothstep(0.67, 0.70, thick_t));
            1.0 - is_double_void
        }
        _ => 1.0, // STROKE_SOLID
    }
}

#[inline]
fn compute_stroke_pattern_alpha(
    local_cx: f32,
    local_cy: f32,
    half_w: f32,
    half_h: f32,
    w_size: f32,
    h_size: f32,
    edge_idx: usize,
    w: f32,
    flags: u32,
    len_limit: f32,
    thick_t: f32,
) -> f32 {
    let shift = edge_idx * 4;
    let style = (flags >> shift) & 3;
    let alignment = (flags >> (shift + 2)) & 3;

    let (pos_edge, t) = if edge_idx == EDGE_TOP || edge_idx == EDGE_BOTTOM {
        let pe = local_cx + half_w;
        (pe, pe / (half_w * 2.0))
    } else {
        let pe = local_cy + half_h;
        (pe, pe / (half_h * 2.0))
    };

    let edge_fade = 0.5 / w_size.max(h_size);
    let length_alpha = compute_length_alpha(t, len_limit, alignment, edge_fade);
    let style_alpha = compute_style_alpha(style, w, pos_edge, thick_t);

    length_alpha * style_alpha
}

#[expect(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
fn compute_border_alpha(
    instance: &QuadInstance,
    local_cx: f32,
    local_cy: f32,
    half_w: f32,
    half_h: f32,
    w_size: f32,
    h_size: f32,
    clamped_radius: [f32; 4],
    dist_to_box: f32,
    box_alpha: f32,
) -> f32 {
    let b_width: [f32; 4] = bytemuck::cast(instance.border_width);
    let b_color: [f32; 4] = bytemuck::cast(instance.border_color);
    let total_width = b_width[0] + b_width[1] + b_width[2] + b_width[3];
    if b_color[3] <= 0.0 || total_width <= 0.0 {
        return 0.0;
    }

    let shift_x = (b_width[EDGE_LEFT] - b_width[EDGE_RIGHT]) * 0.5;
    let shift_y = (b_width[EDGE_TOP] - b_width[EDGE_BOTTOM]) * 0.5;
    let inner_bw = half_w - f32::midpoint(b_width[EDGE_LEFT], b_width[EDGE_RIGHT]);
    let inner_bh = half_h - f32::midpoint(b_width[EDGE_TOP], b_width[EDGE_BOTTOM]);

    let calc = |i: usize| (clamped_radius[i] - b_width[i]).max(0.0);
    let inner_radius = [calc(0), calc(1), calc(2), calc(3)];

    let dist_to_inner = sd_rounded_box(
        local_cx - shift_x,
        local_cy - shift_y,
        inner_bw,
        inner_bh,
        inner_radius,
    );

    let edge_idx = select_closest_edge(local_cx, local_cy, half_w, half_h, b_width, true);
    let w = b_width[edge_idx];
    let denom = -dist_to_box + dist_to_inner;
    let thick_t = if denom.abs() > 1e-6 {
        (-dist_to_box / denom).clamp(0.0, 1.0)
    } else {
        0.0
    };

    let b_lengths: [f32; 4] = bytemuck::cast(instance.border_lengths);
    let flags = instance.opacity_mode_sizing_border[3].max(0.0) as u32;

    let pattern_alpha = compute_stroke_pattern_alpha(
        local_cx,
        local_cy,
        half_w,
        half_h,
        w_size,
        h_size,
        edge_idx,
        w,
        flags,
        b_lengths[edge_idx],
        thick_t,
    );

    let dist_to_inner_clamped = dist_to_inner.min(dist_to_box + w);
    let raw_alpha = box_alpha * smoothstep(-0.5, 0.5, dist_to_inner_clamped) * pattern_alpha;

    raw_alpha * w.clamp(0.0, 1.0)
}

#[expect(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
fn compute_outline_alpha(
    instance: &QuadInstance,
    local_cx: f32,
    local_cy: f32,
    half_w: f32,
    half_h: f32,
    w_size: f32,
    h_size: f32,
    clamped_radius: [f32; 4],
) -> f32 {
    let o_width: [f32; 4] = bytemuck::cast(instance.outline_width);
    let o_color: [f32; 4] = bytemuck::cast(instance.outline_color);
    let total_width = o_width[0] + o_width[1] + o_width[2] + o_width[3];
    if o_color[3] <= 0.0 || total_width <= 0.0 {
        return 0.0;
    }

    let edge_idx = select_closest_edge(local_cx, local_cy, half_w, half_h, o_width, false);
    let o_w = o_width[edge_idx];
    let o_offset = instance.outline_offset_and_flags[0];

    let calc_i = |i: usize| clamped_radius[i] + o_offset;
    let r_inner = [calc_i(0), calc_i(1), calc_i(2), calc_i(3)];

    let calc_o = |i: usize| clamped_radius[i] + o_offset + o_w;
    let r_outer = [calc_o(0), calc_o(1), calc_o(2), calc_o(3)];

    let dist_to_inner = sd_rounded_box(
        local_cx,
        local_cy,
        half_w + o_offset,
        half_h + o_offset,
        r_inner,
    );
    let dist_to_outer = sd_rounded_box(
        local_cx,
        local_cy,
        half_w + o_offset + o_w,
        half_h + o_offset + o_w,
        r_outer,
    );

    let base_alpha =
        smoothstep(-0.5, 0.5, dist_to_inner) * (1.0 - smoothstep(-0.5, 0.5, dist_to_outer));
    let denom = dist_to_inner - dist_to_outer;
    let thick_t = if denom.abs() > 1e-6 {
        (dist_to_inner / denom).clamp(0.0, 1.0)
    } else {
        0.0
    };

    let o_lengths: [f32; 4] = bytemuck::cast(instance.outline_lengths);
    let flags = instance.outline_offset_and_flags[1].max(0.0) as u32;

    let pattern_alpha = compute_stroke_pattern_alpha(
        local_cx,
        local_cy,
        half_w,
        half_h,
        w_size,
        h_size,
        edge_idx,
        o_w,
        flags,
        o_lengths[edge_idx],
        thick_t,
    );

    base_alpha * pattern_alpha * o_w.clamp(0.0, 1.0)
}

fn compute_sdf_margin(instance: &QuadInstance) -> f32 {
    let mut margin = 0.0_f32;

    let o_width: [f32; 4] = bytemuck::cast(instance.outline_width);
    let o_color: [f32; 4] = bytemuck::cast(instance.outline_color);
    let total_o_width = o_width[0] + o_width[1] + o_width[2] + o_width[3];
    if o_color[3] > 0.0 && total_o_width > 0.0 {
        let max_o_width = o_width[0].max(o_width[1]).max(o_width[2]).max(o_width[3]);
        let o_offset = instance.outline_offset_and_flags[0];
        let outline_margin = max_o_width + o_offset.max(0.0);
        margin = margin.max(outline_margin + 1.5);
    }

    margin
}

#[inline]
fn to_pma_srgb(rgba: [f32; 4], opacity: f32) -> [f32; 4] {
    let a = (rgba[3] * opacity).clamp(0.0, 1.0);
    let clamped = |i: usize| rgba[i].clamp(0.0, 1.0) * a;
    [clamped(0), clamped(1), clamped(2), a]
}

#[inline]
fn floor_min<F>(corners: &mut [tiny_skia::Point; 4], f: F) -> f32
where
    F: Fn(&tiny_skia::Point) -> f32,
{
    corners.iter().map(f).fold(f32::INFINITY, f32::min).floor()
}

#[inline]
fn ceil_max<F>(corners: &mut [tiny_skia::Point; 4], f: F) -> f32
where
    F: Fn(&tiny_skia::Point) -> f32,
{
    corners
        .iter()
        .map(f)
        .fold(f32::NEG_INFINITY, f32::max)
        .ceil()
}

#[expect(
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss,
    clippy::cast_precision_loss
)]
fn draw_sdf_quad(
    pixmap: &mut Pixmap,
    instance: &QuadInstance,
    scissor_rect: LayoutRect,
    scale_factor: f32,
    blend_mode: BlendMode,
    skip_entity: bool,
) {
    let opacity = instance.opacity_mode_sizing_border[0].clamp(0.0, 1.0);
    if opacity <= 0.0 {
        return;
    }

    let Some(shape_transform) = build_instance_transform(instance, scale_factor) else {
        return;
    };
    let Some(inv_transform) = shape_transform.invert() else {
        return;
    };

    let w_size = instance.rect.width;
    let h_size = instance.rect.height;
    let half_w = w_size * 0.5;
    let half_h = h_size * 0.5;
    let mode = instance.opacity_mode_sizing_border[1];

    // 変換後4隅から物理バウンディングボックスを算出（AA 用に論理 1px 余裕を持たせる）
    let margin = compute_sdf_margin(instance) + (1.0 / scale_factor.max(0.01));
    let mut corners = [
        tiny_skia::Point::from_xy(-margin, -margin),
        tiny_skia::Point::from_xy(w_size + margin, -margin),
        tiny_skia::Point::from_xy(w_size + margin, h_size + margin),
        tiny_skia::Point::from_xy(-margin, h_size + margin),
    ];
    shape_transform.map_points(&mut corners);

    let min_px = floor_min(&mut corners, |p| p.x);
    let min_py = floor_min(&mut corners, |p| p.y);
    let max_px = ceil_max(&mut corners, |p| p.x);
    let max_py = ceil_max(&mut corners, |p| p.y);

    let pw_u32 = pixmap.width();
    let ph_u32 = pixmap.height();
    let pw = pw_u32 as f32;
    let ph = ph_u32 as f32;

    let rect_clamped = |val: f32, max: f32| (val * scale_factor).round().clamp(0.0, max) as usize;
    // prepare_scissor_mask と同じ丸め規則でシザー矩形を適用
    let sc_x0 = rect_clamped(scissor_rect.x, pw);
    let sc_y0 = rect_clamped(scissor_rect.y, ph);
    let sc_x1 = rect_clamped(scissor_rect.x + scissor_rect.width, pw);
    let sc_y1 = rect_clamped(scissor_rect.y + scissor_rect.height, ph);

    let x0 = (min_px.max(0.0) as usize).max(sc_x0);
    let y0 = (min_py.max(0.0) as usize).max(sc_y0);
    let x1 = (max_px.max(0.0).min(pw) as usize).min(sc_x1);
    let y1 = (max_py.max(0.0).min(ph) as usize).min(sc_y1);

    if x0 >= x1 || y0 >= y1 {
        return;
    }

    let raw_radii: [f32; 4] = bytemuck::cast(instance.corner_radius);
    let max_radius = half_w.min(half_h);
    let radii = |i: usize| raw_radii[i].min(max_radius).max(0.0);
    let clamped_radius = [radii(0), radii(1), radii(2), radii(3)];

    let entity_pma = if skip_entity {
        [0.0; 4]
    } else {
        to_pma_srgb(bytemuck::cast(instance.color), opacity)
    };
    let border_pma = to_pma_srgb(bytemuck::cast(instance.border_color), opacity);
    let outline_pma = to_pma_srgb(bytemuck::cast(instance.outline_color), opacity);

    let stride = pw_u32 as usize;
    let pixels = pixmap.pixels_mut();

    for py in y0..y1 {
        let row_offset = py * stride;
        let fy = py as f32 + 0.5;

        for px in x0..x1 {
            let mut local_pt = tiny_skia::Point::from_xy(px as f32 + 0.5, fy);
            inv_transform.map_point(&mut local_pt);

            let local_cx = local_pt.x - half_w;
            let local_cy = local_pt.y - half_h;

            let dist_to_box = sd_rounded_box(local_cx, local_cy, half_w, half_h, clamped_radius);
            let box_alpha = if mode < MODE_DECORATION_THRESHOLD {
                1.0
            } else {
                1.0 - smoothstep(-0.5, 0.5, dist_to_box)
            };

            let border_alpha = compute_border_alpha(
                instance,
                local_cx,
                local_cy,
                half_w,
                half_h,
                w_size,
                h_size,
                clamped_radius,
                dist_to_box,
                box_alpha,
            );
            let outline_alpha = compute_outline_alpha(
                instance,
                local_cx,
                local_cy,
                half_w,
                half_h,
                w_size,
                h_size,
                clamped_radius,
            );

            let mut base = if skip_entity {
                let calc = |i: usize| border_pma[i] * border_alpha * box_alpha;
                [calc(0), calc(1), calc(2), calc(3)]
            } else {
                let mut e = entity_pma;
                if border_alpha > 0.0 {
                    let inv_b = 1.0 - border_alpha;
                    let mut calc = |i: usize| e[i] = e[i] * inv_b + border_pma[i] * border_alpha;
                    calc(0);
                    calc(1);
                    calc(2);
                    calc(3);
                }
                let calc = |i: usize| e[i] * box_alpha;
                [calc(0), calc(1), calc(2), calc(3)]
            };

            if outline_alpha > 0.0 {
                let factor = outline_alpha * (1.0 - base[3]);
                let mut calc = |i: usize| base[i] += outline_pma[i] * factor;
                calc(0);
                calc(1);
                calc(2);
                calc(3);
            }

            let sa = base[3].clamp(0.0, 1.0);
            if sa <= 0.0 {
                continue;
            }

            let dst = &mut pixels[row_offset + px];
            let (nr, ng, nb, na) = match blend_mode {
                BlendMode::DestinationOut => {
                    let inv_a = 1.0 - sa;
                    let calc = |c: u8| f32::from(c) * inv_a;
                    (
                        calc(dst.red()),
                        calc(dst.green()),
                        calc(dst.blue()),
                        calc(dst.alpha()),
                    )
                }
                _ => {
                    // SourceOver
                    if sa >= 1.0 {
                        let calc = |i: usize| base[i].clamp(0.0, 1.0) * 255.0;
                        (calc(0), calc(1), calc(2), 255.0)
                    } else {
                        let inv_a = 1.0 - sa;
                        let calc = |i: usize, c: u8| {
                            base[i].clamp(0.0, 1.0) * 255.0 + f32::from(c) * inv_a
                        };
                        (
                            calc(0, dst.red()),
                            calc(1, dst.green()),
                            calc(2, dst.blue()),
                            sa * 255.0 + f32::from(dst.alpha()) * inv_a,
                        )
                    }
                }
            };

            let clamped = |v: f32| (v + 0.5).clamp(0.0, 255.0) as u8;

            let out_a = clamped(na);
            let out_r = (clamped(nr)).min(out_a);
            let out_g = (clamped(ng)).min(out_a);
            let out_b = (clamped(nb)).min(out_a);
            if let Some(p) = PremultipliedColorU8::from_rgba(out_r, out_g, out_b, out_a) {
                *dst = p;
            }
        }
    }
}
