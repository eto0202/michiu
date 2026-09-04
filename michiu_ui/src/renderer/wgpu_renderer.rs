#![allow(dead_code)]
use crate::{
    BatchType, BorderAlignment, BorderStyle, BoxSizing, Color, Context, CornerRadius, DrawBatch,
    EdgeInsets, EntityId, LayoutPoint, LayoutRect, LayoutSize, LayoutStore, Length, NewPipeline,
    NewRendererView, NewTextCacheKey, OutputStore, Pipeline, QuadInstance, RenderData,
    RendererView, TextAlign, TextCacheKey, TextCacheValue, TextRasterizer, TextSpan, TextureAtlas,
    Vertex, VisualProperty,
};
use raw_window_handle::{
    RawDisplayHandle, RawWindowHandle, Win32WindowHandle, WindowsDisplayHandle,
};
use rustc_hash::FxHashMap;
use slotmap::SecondaryMap;
use std::collections::HashMap;
use std::num::NonZeroIsize;
use wgpu::util::DeviceExt;
use wgpu::wgt::CommandEncoderDescriptor;
use wgpu::{CurrentSurfaceTexture, PipelineCompilationOptions};
use windows::{
    Win32::{
        Foundation::HANDLE,
        Graphics::{
            Direct3D11::{
                D3D11_BIND_RENDER_TARGET, D3D11_BIND_SHADER_RESOURCE, D3D11_RESOURCE_MISC_SHARED,
                D3D11_RESOURCE_MISC_SHARED_NTHANDLE, D3D11_TEXTURE2D_DESC, D3D11_USAGE_DEFAULT,
                ID3D11Device, ID3D11Texture2D,
            },
            Direct3D12::ID3D12Resource,
            DirectWrite::IDWriteTextLayout,
            Dxgi::{
                Common::{DXGI_FORMAT_B8G8R8A8_UNORM, DXGI_SAMPLE_DESC},
                DXGI_SHARED_RESOURCE_READ, DXGI_SHARED_RESOURCE_WRITE, IDXGIResource1,
            },
        },
    },
    core::Interface,
};

pub struct WgpuRenderer {
    pub(crate) surface: wgpu::Surface<'static>,
    pub(crate) device: wgpu::Device,
    pub(crate) queue: wgpu::Queue,
    pub(crate) config: wgpu::SurfaceConfiguration,

    pub(crate) pipeline: wgpu::RenderPipeline,
    pub(crate) punchout_pipeline: wgpu::RenderPipeline,

    // 頂点データ（共通の 1x1 矩形）
    pub(crate) vertex_buffer: wgpu::Buffer,
    pub(crate) index_buffer: wgpu::Buffer,

    // インスタンスデータ（可変長バッファ）
    pub(crate) instance_buffer: wgpu::Buffer,
    pub(crate) instance_buffer_capacity: usize,
    pub(crate) instance_staging: Vec<QuadInstance>,

    // スクリーン投影用 Uniform
    pub(crate) config_buffer: wgpu::Buffer,
    pub(crate) config_bind_group: wgpu::BindGroup,
    // ebView2 の静止画を描画する際にバインドグループを動的生成するため保持
    pub(crate) config_bind_group_layout: wgpu::BindGroupLayout,

    pub(crate) text_rasterizer: TextRasterizer,
    pub(crate) atlas: TextureAtlas,
    pub(crate) temp_uv_map: SecondaryMap<EntityId, [f32; 4]>,
    pub(crate) text_cache: FxHashMap<TextCacheKey, TextCacheValue>,
    pub(crate) new_text_cache: FxHashMap<NewTextCacheKey, TextCacheValue>,
    // 非アクティブ状態の WebView2 の静止画キャッシュ
    pub(crate) webview_static_caches: FxHashMap<EntityId, wgpu::TextureView>,

    pub(crate) render_data: RenderData,

    /// 外部テクスチャ用の `BindGroup` キャッシュ
    pub(crate) external_bind_groups: FxHashMap<EntityId, (wgpu::TextureView, wgpu::BindGroup)>,
}

#[repr(C)]
#[derive(Copy, Clone, bytemuck::Pod, bytemuck::Zeroable)]
struct GlobalConfig {
    pub(crate) screen_size: [f32; 2],
    pub(crate) scale: f32,
    pub(crate) _padding: f32, // std140のアライメント（16バイト境界）に合わせるためのパディング
}

impl WgpuRenderer {
    pub(crate) async fn new(
        visual: *mut std::ffi::c_void, // 背面ビジュアル
        size: LayoutSize,
        scale_factor: f32,
    ) -> Result<Self, Box<dyn std::error::Error>> {
        // インスタンス生成（DX12を明示的に指定）
        let instance = WgpuRenderer::create_instance();

        // Surface の作成 (CompositionVisual を使用)
        // create_surface_unsafe を利用して渡された生ポインタから Surface を構築
        let target = unsafe { wgpu::SurfaceTargetUnsafe::CompositionVisual(visual) };
        let surface = unsafe { instance.create_surface_unsafe(target)? };

        // アダプターの取得
        let adapter = instance
            .request_adapter(&wgpu::RequestAdapterOptions {
                power_preference: wgpu::PowerPreference::default(),
                compatible_surface: Some(&surface),
                force_fallback_adapter: false,
            })
            .await
            .map_err(|_| "Failed to find an appropriate adapter")?;

        // デバイスとキューの取得
        let mut custom_limits = wgpu::Limits::downlevel_defaults();
        custom_limits.max_non_sampler_bindings = 2048;
        custom_limits.max_bind_groups = 3;
        custom_limits.max_vertex_buffers = 3;
        custom_limits.max_texture_dimension_2d = 8192;

        let (device, queue) = adapter
            .request_device(&wgpu::DeviceDescriptor {
                label: Some("Michiu Renderer Device"),
                required_features: wgpu::Features::empty(),
                required_limits: custom_limits,
                memory_hints: wgpu::MemoryHints::MemoryUsage,
                experimental_features: wgpu::ExperimentalFeatures::default(),
                trace: wgpu::Trace::Off,
            })
            .await?;

        // Surface 設定
        let config = WgpuRenderer::surface_config(size, &surface, &adapter);
        // Surface に設定をアタッチしてスワップチェーンを初期化
        surface.configure(&device, &config);

        // シェーダーの読み込み
        let shader = device.create_shader_module(wgpu::include_wgsl!("shader.wgsl"));

        // ユニフォームバッファ
        let config_buffer = WgpuRenderer::create_buffer(size, scale_factor, &device);

        // アトラス初期化
        let atlas = TextureAtlas::new(&device, 2048);

        let config_bind_group_layout = WgpuRenderer::create_bind_group_layout(&device);

        // パイプライン
        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("Render Pipeline Layout"),
            bind_group_layouts: &[Some(&config_bind_group_layout)],
            immediate_size: 0,
        });
        let pipeline =
            WgpuRenderer::create_render_pipeline(&pipeline_layout, &device, &shader, &config);
        // くり抜き用のパイプライン
        let punchout_pipeline =
            WgpuRenderer::create_punchout_pipeline(&pipeline_layout, &device, &shader, &config);

        // 頂点/インデックスバッファ
        let (vertex_buffer, index_buffer) = WgpuRenderer::create_buffer_init(&device);

        // 初期インスタンスバッファ
        let buffer_capacity = 64;
        let instance_buffer = WgpuRenderer::create_instance_buffer(&device, buffer_capacity);

        let config_bind_group = WgpuRenderer::create_bind_group(
            &device,
            &config_bind_group_layout,
            &config_buffer,
            &instance_buffer,
            &atlas,
        );

        Ok(Self {
            surface,
            device,
            queue,
            config,
            pipeline,
            punchout_pipeline,
            vertex_buffer,
            index_buffer,
            instance_buffer,
            instance_buffer_capacity: buffer_capacity,
            instance_staging: Vec::with_capacity(64),
            config_buffer,
            config_bind_group,
            config_bind_group_layout,
            text_rasterizer: TextRasterizer::new(),
            atlas,
            temp_uv_map: SecondaryMap::new(),
            text_cache: FxHashMap::default(),
            new_text_cache: FxHashMap::default(),
            webview_static_caches: FxHashMap::default(),
            render_data: RenderData::new(),
            external_bind_groups: FxHashMap::default(),
        })
    }

    #[inline]
    fn create_instance() -> wgpu::Instance {
        wgpu::Instance::new(wgpu::InstanceDescriptor {
            backends: wgpu::Backends::DX12,
            flags: wgpu::InstanceFlags::empty(),
            memory_budget_thresholds: wgpu::MemoryBudgetThresholds::default(),
            backend_options: wgpu::BackendOptions {
                dx12: wgpu::Dx12BackendOptions {
                    shader_compiler: wgpu::Dx12Compiler::default(),
                    presentation_system: wgpu::Dx12SwapchainKind::DxgiFromVisual, // // DComp用スワップチェーン
                    ..Default::default()
                },
                ..Default::default()
            },
            display: None,
        })
    }

    #[inline]
    fn surface_config(
        size: LayoutSize,
        surface: &wgpu::Surface,
        adapter: &wgpu::Adapter,
    ) -> wgpu::SurfaceConfiguration {
        let caps = surface.get_capabilities(adapter);
        let surface_format = caps
            .formats
            .iter()
            .copied()
            .find(wgpu::TextureFormat::is_srgb) // SRGBを優先
            .unwrap_or(caps.formats[0]);

        wgpu::SurfaceConfiguration {
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
            format: surface_format,
            width: size.width.max(1.0) as u32,
            height: size.height.max(1.0) as u32,
            present_mode: wgpu::PresentMode::Fifo, // VSync 有効
            alpha_mode: wgpu::CompositeAlphaMode::PreMultiplied,
            view_formats: vec![],
            desired_maximum_frame_latency: 2,
        }
    }

    #[inline]
    fn create_buffer(size: LayoutSize, scale_factor: f32, device: &wgpu::Device) -> wgpu::Buffer {
        device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("Global Config Buffer"),
            contents: bytemuck::cast_slice(&[GlobalConfig {
                screen_size: [size.width, size.height],
                scale: scale_factor,
                _padding: 0.0,
            }]),
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        })
    }

    #[inline]
    fn create_bind_group_layout(device: &wgpu::Device) -> wgpu::BindGroupLayout {
        device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            entries: &[
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::VERTEX | wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 1,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Texture {
                        sample_type: wgpu::TextureSampleType::Float { filterable: true },
                        view_dimension: wgpu::TextureViewDimension::D2,
                        multisampled: false,
                    },
                    count: None,
                },
                // Atlas Sampler (Binding 2)
                wgpu::BindGroupLayoutEntry {
                    binding: 2,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                    count: None,
                },
                // Storage (読み取り専用)
                wgpu::BindGroupLayoutEntry {
                    binding: 3,
                    visibility: wgpu::ShaderStages::VERTEX | wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Storage { read_only: true },
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
            ],
            label: None,
        })
    }

    #[inline]
    fn create_render_pipeline(
        pipeline_layout: &wgpu::PipelineLayout,
        device: &wgpu::Device,
        shader: &wgpu::ShaderModule,
        config: &wgpu::SurfaceConfiguration,
    ) -> wgpu::RenderPipeline {
        device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("Render Pipeline"),
            layout: Some(pipeline_layout),
            vertex: wgpu::VertexState {
                module: shader,
                entry_point: Some("vs_main"),
                buffers: &[
                    // Vertex Buffer
                    wgpu::VertexBufferLayout {
                        array_stride: std::mem::size_of::<Vertex>() as wgpu::BufferAddress,
                        step_mode: wgpu::VertexStepMode::Vertex,
                        attributes: &[wgpu::VertexAttribute {
                            format: wgpu::VertexFormat::Float32x2,
                            offset: 0,
                            shader_location: 0,
                        }],
                    },
                ],
                compilation_options: PipelineCompilationOptions::default(),
            },
            fragment: Some(wgpu::FragmentState {
                module: shader,
                entry_point: Some("fs_main"),
                targets: &[Some(wgpu::ColorTargetState {
                    format: config.format,
                    blend: Some(wgpu::BlendState::PREMULTIPLIED_ALPHA_BLENDING),
                    write_mask: wgpu::ColorWrites::ALL,
                })],
                compilation_options: PipelineCompilationOptions::default(),
            }),
            primitive: wgpu::PrimitiveState::default(),
            depth_stencil: None,
            multisample: wgpu::MultisampleState::default(),
            multiview_mask: None,
            cache: None,
        })
    }

    #[inline]
    fn create_punchout_pipeline(
        pipeline_layout: &wgpu::PipelineLayout,
        device: &wgpu::Device,
        shader: &wgpu::ShaderModule,
        config: &wgpu::SurfaceConfiguration,
    ) -> wgpu::RenderPipeline {
        device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("Punchout Render Pipeline"),
            layout: Some(pipeline_layout),
            vertex: wgpu::VertexState {
                module: shader,
                entry_point: Some("vs_main"),
                buffers: &[wgpu::VertexBufferLayout {
                    array_stride: std::mem::size_of::<Vertex>() as wgpu::BufferAddress,
                    step_mode: wgpu::VertexStepMode::Vertex,
                    attributes: &[wgpu::VertexAttribute {
                        format: wgpu::VertexFormat::Float32x2,
                        offset: 0,
                        shader_location: 0,
                    }],
                }],
                compilation_options: PipelineCompilationOptions::default(),
            },
            fragment: Some(wgpu::FragmentState {
                module: shader,
                entry_point: Some("fs_main"),
                targets: &[Some(wgpu::ColorTargetState {
                    format: config.format,
                    // ブレンドせず、出力色（アルファ=0.0）で完全に上書きする
                    blend: Some(wgpu::BlendState {
                        color: wgpu::BlendComponent {
                            src_factor: wgpu::BlendFactor::Zero,
                            // アルファが 0.0 に消去される場所ではカラーも同時に 0.0 になるように
                            // OneMinusSrcAlpha で元の背景色を減算します。
                            dst_factor: wgpu::BlendFactor::OneMinusSrcAlpha,
                            operation: wgpu::BlendOperation::Add,
                        },
                        alpha: wgpu::BlendComponent {
                            src_factor: wgpu::BlendFactor::Zero,
                            dst_factor: wgpu::BlendFactor::OneMinusSrcAlpha, // アルファは SDF マスク分だけ消去
                            operation: wgpu::BlendOperation::Add,
                        },
                    }),
                    write_mask: wgpu::ColorWrites::ALL,
                })],
                compilation_options: PipelineCompilationOptions::default(),
            }),
            primitive: wgpu::PrimitiveState::default(),
            depth_stencil: None,
            multisample: wgpu::MultisampleState::default(),
            multiview_mask: None,
            cache: None,
        })
    }

    #[inline]
    fn create_buffer_init(device: &wgpu::Device) -> (wgpu::Buffer, wgpu::Buffer) {
        let vertices = [
            Vertex {
                position: [0.0, 0.0],
            },
            Vertex {
                position: [1.0, 0.0],
            },
            Vertex {
                position: [1.0, 1.0],
            },
            Vertex {
                position: [0.0, 1.0],
            },
        ];
        let vertex_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("Vertex Buffer"),
            contents: bytemuck::cast_slice(&vertices),
            usage: wgpu::BufferUsages::VERTEX,
        });

        let indices: [u16; 6] = [0, 1, 2, 2, 3, 0];

        let index_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("Index Buffer"),
            contents: bytemuck::cast_slice(&indices),
            usage: wgpu::BufferUsages::INDEX,
        });

        (vertex_buffer, index_buffer)
    }

    #[inline]
    fn create_instance_buffer(device: &wgpu::Device, capacity: usize) -> wgpu::Buffer {
        device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("Instance Storage Buffer"),
            size: (capacity * std::mem::size_of::<QuadInstance>()) as wgpu::BufferAddress,
            usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        })
    }

    #[inline]
    fn create_bind_group(
        device: &wgpu::Device,
        bind_group_layout: &wgpu::BindGroupLayout,
        config: &wgpu::Buffer,
        instance: &wgpu::Buffer,
        atlas: &TextureAtlas,
    ) -> wgpu::BindGroup {
        device.create_bind_group(&wgpu::BindGroupDescriptor {
            layout: bind_group_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: config.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::TextureView(&atlas.view),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: wgpu::BindingResource::Sampler(&atlas.sampler),
                },
                wgpu::BindGroupEntry {
                    binding: 3,
                    resource: instance.as_entire_binding(),
                },
            ],
            label: None,
        })
    }

    /// ウィンドウサイズが変更された際の再設定
    /// `new_physical_size`: (width, height)
    pub(crate) fn resize(&mut self, new_physical_size: (u32, u32), scale_factor: f32) {
        if new_physical_size.0 > 0 && new_physical_size.1 > 0 {
            self.config.width = new_physical_size.0;
            self.config.height = new_physical_size.1;

            self.surface.configure(&self.device, &self.config);

            let logical_width = new_physical_size.0 as f32 / scale_factor;
            let logical_height = new_physical_size.1 as f32 / scale_factor;

            // シェーダー側で位置を正しく計算できるよう、論理サイズを Uniform に再書き込み
            let config_data = GlobalConfig {
                screen_size: [logical_width, logical_height],
                scale: scale_factor,
                _padding: 0.0,
            };
            self.queue
                .write_buffer(&self.config_buffer, 0, bytemuck::cast_slice(&[config_data]));
        }
    }

    pub(crate) fn render(&mut self, cx: &mut Context, scale_factor: f32, cosmic: bool) {
        let _context_guard = crate::bind_context(cx);
        // 破棄された要素のキャッシュを解放
        for id in cx.topology.topo_despawned_queue.drain(..) {
            self.external_bind_groups.remove(&id);
            self.webview_static_caches.remove(&id);
        }

        if cosmic {
            NewPipeline::collect_render_data(
                cx,
                &mut NewRendererView {
                    render_data: &mut self.render_data,
                    atlas: &mut self.atlas,
                    text_rasterizer: &self.text_rasterizer,
                    text_cache: &mut self.new_text_cache,
                    queue: &self.queue,
                },
            );
        } else {
            // 前面と背面に分類されたバッチを Context から引き出す
            Pipeline::collect_render_data(
                cx,
                &mut RendererView {
                    render_data: &mut self.render_data,
                    atlas: &mut self.atlas,
                    text_rasterizer: &self.text_rasterizer,
                    text_cache: &mut self.text_cache,
                    queue: &self.queue,
                },
            );
        }

        if self.render_data.batches.is_empty() {
            return;
        }

        let mut render_data = std::mem::take(&mut self.render_data);

        // スワップチェーンから描画先フレームを獲得
        // TODO: エラーハンドリング
        let wgpu::CurrentSurfaceTexture::Success(surface_texture) =
            self.surface.get_current_texture()
        else {
            return;
        };

        let view = surface_texture
            .texture
            .create_view(&wgpu::TextureViewDescriptor::default());

        self.instance_staging.clear();
        for (i, instance) in render_data.instances.iter().enumerate() {
            let entity_id = render_data.entity_ids[i];
            let (inst, _) = WgpuRenderer::build_quad_instance_for_entity(cx, entity_id, instance);
            self.instance_staging.push(inst);
        }

        // 容量を確定
        self.ensure_instance_buffer_capacity(self.instance_staging.len());

        // 外部テクスチャを事前にキャッシュ
        self.update_external_texture_bind_groups(cx);

        // VRAM インスタンスバッファへの一括転送
        self.queue.write_buffer(
            &self.instance_buffer,
            0,
            bytemuck::cast_slice(&self.instance_staging),
        );

        // 背面の描画実行
        let mut encoder = self
            .device
            .create_command_encoder(&CommandEncoderDescriptor::default());
        {
            let mut rpass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("Background Render Pass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &view,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color::TRANSPARENT),
                        store: wgpu::StoreOp::Store,
                    },
                    depth_slice: None,
                })],
                depth_stencil_attachment: None,
                ..Default::default()
            });

            rpass.set_pipeline(&self.pipeline);
            rpass.set_vertex_buffer(0, self.vertex_buffer.slice(..));
            rpass.set_index_buffer(self.index_buffer.slice(..), wgpu::IndexFormat::Uint16);

            // 背面用バッチの描画 (instance_offset は 0 から開始)
            let mut instance_offset = 0;
            // 現在アクティブなパイプラインを記録
            let mut current_pipeline = None;

            for batch in &render_data.batches {
                let offset = batch.instance_offset as u32;
                let count = batch.instance_count as u32;
                if count == 0 {
                    continue;
                }

                // 次のバッチに必要なパイプラインを特定
                // くり抜きバッチであれば punchout_pipeline (REPLACEブレンド)、
                // 通常バッチであれば通常の pipeline (PMAブレンド) を設定。
                let needed_pipeline = match batch.batch_type {
                    BatchType::Normal => &self.pipeline,
                    BatchType::Punchout => &self.punchout_pipeline,
                };

                // アクティブなパイプラインと異なる場合のみ、wgpu側にコマンドを送信する
                if current_pipeline != Some(std::ptr::from_ref(needed_pipeline)) {
                    rpass.set_pipeline(needed_pipeline);
                    current_pipeline = Some(std::ptr::from_ref(needed_pipeline));
                }

                // 静止 WebView2 描画時のバインディングの切り替え
                self.bind_texture_for_batch(&mut rpass, batch, &render_data.entity_ids);

                let clip = batch.scissor_rect;

                // スケールを乗じて物理ピクセル座標を算出
                let phys_x = (clip.x * scale_factor).round().max(0.0) as u32;
                let phys_y = (clip.y * scale_factor).round().max(0.0) as u32;
                let phys_w = (clip.width * scale_factor).round().max(0.0) as u32;
                let phys_h = (clip.height * scale_factor).round().max(0.0) as u32;

                // 現在のスワップチェーンテクスチャの境界サイズを取得
                let target_w = self.config.width;
                let target_h = self.config.height;

                //  物理開始位置がすでに縮小後のバックバッファ外に押し出されている場合、
                // 描画が不可能であるため、検証エラーを避けるためにこのバッチの描画をスキップ
                if phys_x >= target_w || phys_y >= target_h {
                    continue;
                }

                // 開始位置 + 幅/高さがレンダーターゲットをはみ出さないように厳密にクランプ
                let clamped_w = phys_w.min(target_w - phys_x);
                let clamped_h = phys_h.min(target_h - phys_y);

                // wgpu の制約回避のため、クランプ後の幅・高さが 0 の場合も描画をスキップ
                if clamped_w == 0 || clamped_h == 0 {
                    continue;
                }

                // 完全に境界内に収まるように安全化された Scissor Rect を適用
                rpass.set_scissor_rect(phys_x, phys_y, clamped_w, clamped_h);

                rpass.draw_indexed(0..6, 0, offset..(offset + count));
            }
        }
        self.queue.submit(Some(encoder.finish()));
        surface_texture.present();

        // 描画が完了したため蓄積された描画Dirtyをクリア
        cx.clear_render_dirty();

        // wgpuのデバイスを明示的にポーリングし、未解決のフェンスやリソースをフラッシュ
        self.device.poll(wgpu::PollType::Poll);

        self.render_data = render_data;
    }

    /// 外部テクスチャが更新された場合のみ、描画ループの外で `BindGroup` を再構築
    fn update_external_texture_bind_groups(&mut self, cx: &Context) {
        for (id, provider) in &cx.contents.cont_external_textures {
            let view = provider.resolve_view(&self.device, &self.queue);

            let need_update = self
                .external_bind_groups
                .get(&id)
                .is_none_or(|(cached_id, _)| *cached_id != view);

            if need_update {
                let bind_group = self.device.create_bind_group(&wgpu::BindGroupDescriptor {
                    layout: &self.config_bind_group_layout,
                    entries: &[
                        wgpu::BindGroupEntry {
                            binding: 0,
                            resource: self.config_buffer.as_entire_binding(),
                        },
                        wgpu::BindGroupEntry {
                            binding: 1,
                            resource: wgpu::BindingResource::TextureView(&view),
                        },
                        wgpu::BindGroupEntry {
                            binding: 2,
                            resource: wgpu::BindingResource::Sampler(&self.atlas.sampler),
                        },
                        wgpu::BindGroupEntry {
                            binding: 3,
                            resource: self.instance_buffer.as_entire_binding(),
                        },
                    ],
                    label: Some("External Texture Bind Group"),
                });

                self.external_bind_groups.insert(id, (view, bind_group));
            }
        }
    }

    /// バッチ内に静止 `WebView2` テクスチャが含まれる場合、バインドグループを動的に切り替える
    fn bind_texture_for_batch<'a>(
        &'a self,
        rpass: &mut wgpu::RenderPass<'a>,
        batch: &DrawBatch,
        entity_ids: &[EntityId],
    ) {
        // バッチに含まれる最初の要素が静止 WebView2 キャッシュを持っているか
        // instance_offsetの位置にある要素の ID を取得
        if let Some(&first_id) = entity_ids.get(batch.instance_offset) {
            // キャッシュが存在する場合
            if let Some((_, bind_group)) = self.external_bind_groups.get(&first_id) {
                rpass.set_bind_group(0, bind_group, &[]);
                return;
            }

            // WebView2 の静止画キャッシュ
            if let Some(cached_view) = self.webview_static_caches.get(&first_id) {
                let temp_bind_group = self.device.create_bind_group(&wgpu::BindGroupDescriptor {
                    layout: &self.config_bind_group_layout,
                    entries: &[
                        wgpu::BindGroupEntry {
                            binding: 0,
                            resource: self.config_buffer.as_entire_binding(),
                        },
                        wgpu::BindGroupEntry {
                            binding: 1,
                            resource: wgpu::BindingResource::TextureView(cached_view),
                        },
                        wgpu::BindGroupEntry {
                            binding: 2,
                            resource: wgpu::BindingResource::Sampler(&self.atlas.sampler),
                        },
                        wgpu::BindGroupEntry {
                            binding: 3,
                            resource: self.instance_buffer.as_entire_binding(),
                        },
                    ],
                    label: None,
                });
                rpass.set_bind_group(0, &temp_bind_group, &[]);
                return;
            }
            // 通常のアトラス
            rpass.set_bind_group(0, &self.config_bind_group, &[]);
        }
    }

    /// `QuadInstance` に静的バインドする
    fn build_quad_instance_for_entity(
        cx: &Context,
        entity_id: EntityId,
        instance: &QuadInstance,
    ) -> (QuadInstance, bool) {
        let basic = &cx
            .layouts
            .lay_resolved_basic
            .get(entity_id)
            .copied()
            .unwrap_or_default();
        let flex = &cx
            .layouts
            .lay_resolved_flex
            .get(entity_id)
            .copied()
            .unwrap_or_default();
        let _grid = &cx
            .layouts
            .lay_resolved_grid
            .get(entity_id)
            .cloned()
            .unwrap_or_default();
        let default_visual = VisualProperty::default();
        let visual = cx
            .renders
            .rnd_visual
            .get(entity_id)
            .unwrap_or(&default_visual);

        let origin = visual.transform_origin.map_or([0.5, 0.5], |p| [p.x, p.y]);
        let opacity = visual.opacity.unwrap_or(1.0);

        let box_sizing_val = match basic.box_sizing {
            BoxSizing::BorderBox => 0.0f32,
            BoxSizing::ContentBox => 1.0f32,
        };

        // 四辺個別枠線フラグの抽出
        let border_lengths = visual.border_lengths.unwrap_or(EdgeInsets::px_all(1.0));
        let styles = visual.border_styles.unwrap_or([BorderStyle::Solid; 4]);
        let aligns = visual
            .border_alignments
            .unwrap_or([BorderAlignment::Start; 4]);

        let mut border_flags = 0u32;
        for i in 0..4 {
            let s_val = styles[i] as u32;
            let a_val = aligns[i] as u32;
            border_flags |= s_val << (i * 4);
            border_flags |= a_val << (i * 4 + 2);
        }

        let o_width = visual.outline_width.unwrap_or_default();
        let o_color = visual.outline_color.unwrap_or_default();
        let o_lengths = visual.outline_lengths.unwrap_or(EdgeInsets::px_all(1.0));
        let o_offset = visual.outline_offset.unwrap_or(0.0);
        let o_styles = visual.outline_styles.unwrap_or([BorderStyle::Solid; 4]);
        let o_aligns = visual
            .outline_alignments
            .unwrap_or([BorderAlignment::Start; 4]);

        let mut outline_flags = 0u32;
        for i in 0..4 {
            let s_val = o_styles[i] as u32;
            let a_val = o_aligns[i] as u32;
            outline_flags |= s_val << (i * 4);
            outline_flags |= a_val << (i * 4 + 2);
        }

        // 影のカラーと形状の解決
        let shadow_color =
            if instance.shadow_color != Color::TRANSPARENT && visual.shadow_params.is_some() {
                let mut color = visual.shadow_color.unwrap_or_default();
                // WebViewアクティブ（DCompブレンド時）の濃さの補正
                let mut has_active_webview_parent = false;
                let mut curr_id = entity_id;
                while let Some(Some(parent_id)) = cx.topology.topo_parents.get(curr_id) {
                    if cx.topology.topo_active_masks[*parent_id].has_webveiw2_content()
                        && cx.renders.rnd_active_webviews.contains(parent_id)
                    {
                        has_active_webview_parent = true;
                        break;
                    }
                    curr_id = *parent_id;
                }
                if has_active_webview_parent {
                    color.a *= 0.45;
                }
                color
            } else {
                Color::TRANSPARENT
            };

        let shadow_params = if shadow_color == Color::TRANSPARENT {
            [0.0; 4]
        } else {
            match visual.shadow_params {
                Some(shadow) => [shadow.offset.x, shadow.offset.y, shadow.blur, shadow.spread],
                None => [0.0; 4],
            }
        };

        let packed_transform = instance.transform;

        // モード、座標、UV は呼び出し元で既に決定されているためそのまま転送
        (
            QuadInstance {
                rect: instance.rect,
                transform: packed_transform,
                transform_origin: origin,
                color: instance.color,
                corner_radius: visual.corner_radius.unwrap_or_default(),
                border_width: instance.border_width,
                border_color: visual.border_color.unwrap_or_default(),
                border_lengths,
                opacity_mode_sizing: [
                    opacity,
                    instance.opacity_mode_sizing[1], // mode (背景=0.0/1.0, テキスト=2.0, 静止WebView=3.0 等)
                    box_sizing_val,
                    border_flags as f32,
                ],
                uv_min: instance.uv_min,
                uv_max: instance.uv_max,
                gradient_end_color: instance.gradient_end_color,
                gradient_angle: instance.gradient_angle,
                _padding: 0.0,
                shadow_color,
                shadow_params,
                outline_width: o_width,
                outline_color: o_color,
                outline_lengths: o_lengths,
                outline_offset_and_flags: [o_offset, outline_flags as f32, 0.0, 0.0],
                alpha_mode_y_flip_srgb: instance.alpha_mode_y_flip_srgb,
            },
            false, // アトラスを直接操作しないため常に false
        )
    }

    fn ensure_instance_buffer_capacity(&mut self, total_instances: usize) {
        if total_instances > self.instance_buffer_capacity {
            // 現在の 1.5 倍、または最低限必要なサイズに拡張
            let new_capacity = (self.instance_buffer_capacity * 3 / 2)
                .max(total_instances)
                .max(64);
            let size = (new_capacity * std::mem::size_of::<QuadInstance>()) as wgpu::BufferAddress;

            self.instance_buffer = self.device.create_buffer(&wgpu::BufferDescriptor {
                label: Some("Instance Storage Buffer"),
                size,
                usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST,
                mapped_at_creation: false,
            });

            self.instance_buffer_capacity = new_capacity;

            // 古いバッファをバインドしているキャッシュをクリア
            self.external_bind_groups.clear();

            // バッファのアドレスが変わったため、標準の config_bind_group も再構築してキャッシュを同期
            self.config_bind_group = self.device.create_bind_group(&wgpu::BindGroupDescriptor {
                layout: &self.config_bind_group_layout,
                entries: &[
                    wgpu::BindGroupEntry {
                        binding: 0,
                        resource: self.config_buffer.as_entire_binding(),
                    },
                    wgpu::BindGroupEntry {
                        binding: 1,
                        resource: wgpu::BindingResource::TextureView(&self.atlas.view),
                    },
                    wgpu::BindGroupEntry {
                        binding: 2,
                        resource: wgpu::BindingResource::Sampler(&self.atlas.sampler),
                    },
                    wgpu::BindGroupEntry {
                        binding: 3,
                        resource: self.instance_buffer.as_entire_binding(),
                    },
                ],
                label: None,
            });
        }
    }
}
