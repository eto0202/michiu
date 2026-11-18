use std::sync::Arc;

use crate::{MichiuRendererResult, Renderer, RendererUserConfig, dcomp::PlatformContext};
use michiu_ui::{
    Atlases, BatchType, Context, Entity, ExternalTexture, LayoutRect, LayoutSize, QuadInstance,
    RenderData, RenderQueue, RendererView,
};
use rustc_hash::FxHashMap;
use tiny_skia::{BlendMode, Mask, Paint, Pixmap};
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
            provider.resolve_cpu(&mut |tex| {
                todo!();
            });
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

            match batch.batch_type {
                BatchType::Punchout => {
                    let clear_paint = Paint {
                        blend_mode: BlendMode::Clear,
                        anti_alias: true,
                        ..Default::default()
                    };

                    for inst in instances {
                        todo!();
                    }
                }
                BatchType::Normal => {
                    for inst in instances {
                        todo!();
                    }
                }
            }
        }

        let _ = self.present();

        self.render_data = render_data;
    }
}

fn prepare_scissor_mask(
    clip_mask: &mut Mask,
    clip: LayoutRect,
    scale_factor: f32,
    p_width: u32,
    p_height: u32,
) -> Option<&Mask> {
    let x0 = (clip.x * scale_factor).round().clamp(0.0, p_width as f32) as u32;
    let y0 = (clip.y * scale_factor).round().clamp(0.0, p_height as f32) as u32;
    let x1 = ((clip.x + clip.width) * scale_factor)
        .round()
        .clamp(0.0, p_width as f32) as u32;
    let y1 = ((clip.y + clip.height) * scale_factor)
        .round()
        .clamp(0.0, p_height as f32) as u32;

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
