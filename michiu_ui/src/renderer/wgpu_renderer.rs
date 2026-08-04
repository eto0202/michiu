#![allow(dead_code)]
use crate::{
    BatchType, BorderAlignment, BorderStyle, BoxSizing, Color, Context, CornerRadius, DrawBatch,
    EdgeInsets, EntityId, LayoutPoint, LayoutRect, LayoutSize, LayoutStore, Length, OutputStore,
    QuadInstance, TextAlign, TextCacheKey, TextCacheValue, TextRasterizer, TextSpan, TextureAtlas,
    Vertex, VisualProperty,
};
use raw_window_handle::{
    RawDisplayHandle, RawWindowHandle, Win32WindowHandle, WindowsDisplayHandle,
};
use slotmap::SecondaryMap;
use std::collections::HashMap;
use std::num::NonZeroIsize;
use wgpu::util::DeviceExt;
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
    pub(crate) text_cache: HashMap<TextCacheKey, TextCacheValue>,
    // 非アクティブ状態の WebView2 の静止画キャッシュ
    pub(crate) webview_static_caches: HashMap<EntityId, wgpu::TextureView>,
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
        // 1. インスタンス生成（DX12を明示的に指定）
        let instance = wgpu::Instance::new(wgpu::InstanceDescriptor {
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
        });

        // 2. Surface の作成 (CompositionVisual を使用)
        // create_surface_unsafe を利用して、渡された生ポインタから Surface を構築します
        let target = unsafe { wgpu::SurfaceTargetUnsafe::CompositionVisual(visual) };
        let surface = unsafe { instance.create_surface_unsafe(target)? };

        // 3. アダプター（GPU）の取得
        let adapter = instance
            .request_adapter(&wgpu::RequestAdapterOptions {
                power_preference: wgpu::PowerPreference::default(),
                compatible_surface: Some(&surface),
                force_fallback_adapter: false,
            })
            .await
            .map_err(|_| "Failed to find an appropriate adapter")?;

        // 4. デバイスとキューの取得
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

        // 5. Surface 設定
        let caps = surface.get_capabilities(&adapter);
        let surface_format = caps
            .formats
            .iter()
            .copied()
            .find(wgpu::TextureFormat::is_srgb) // SRGBを優先
            .unwrap_or(caps.formats[0]);

        let config = wgpu::SurfaceConfiguration {
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
            format: surface_format,
            width: size.width.max(1.0) as u32,
            height: size.height.max(1.0) as u32,
            present_mode: wgpu::PresentMode::Fifo, // VSync 有効
            alpha_mode: wgpu::CompositeAlphaMode::PreMultiplied,
            view_formats: vec![],
            desired_maximum_frame_latency: 2,
        };

        // Surface に設定をアタッチしてスワップチェーンを初期化
        surface.configure(&device, &config);

        // 6. シェーダーの読み込み
        let shader = device.create_shader_module(wgpu::include_wgsl!("shader.wgsl"));

        // 7. ユニフォーム(Config)バッファ
        let config_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("Global Config Buffer"),
            contents: bytemuck::cast_slice(&[GlobalConfig {
                screen_size: [size.width, size.height],
                scale: scale_factor,
                _padding: 0.0,
            }]),
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        });

        // アトラス初期化
        let atlas = TextureAtlas::new(&device, 2048);

        let config_bind_group_layout =
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
            });

        // 8. パイプライン
        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("Render Pipeline Layout"),
            bind_group_layouts: &[Some(&config_bind_group_layout)],
            immediate_size: 0,
        });

        let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("Render Pipeline"),
            layout: Some(&pipeline_layout),
            vertex: wgpu::VertexState {
                module: &shader,
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
                module: &shader,
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
        });

        // くり抜き用のパイプライン
        let punchout_pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("Punchout Render Pipeline"),
            layout: Some(&pipeline_layout),
            vertex: wgpu::VertexState {
                module: &shader,
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
                module: &shader,
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
        });

        // 9. 頂点/インデックスバッファの作成 (1x1 Quad)
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

        // 10. 初期インスタンスバッファ
        let instance_buffer_capacity = 64;
        let instance_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("Instance Storage Buffer"),
            size: (instance_buffer_capacity * std::mem::size_of::<QuadInstance>())
                as wgpu::BufferAddress,
            usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        let config_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            layout: &config_bind_group_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: config_buffer.as_entire_binding(),
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
                    resource: instance_buffer.as_entire_binding(),
                },
            ],
            label: None,
        });

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
            instance_buffer_capacity,
            instance_staging: Vec::with_capacity(64),
            config_buffer,
            config_bind_group,
            config_bind_group_layout,
            text_rasterizer: TextRasterizer::new(),
            atlas,
            temp_uv_map: SecondaryMap::new(),
            text_cache: HashMap::new(),
            webview_static_caches: HashMap::new(),
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

    pub(crate) fn render(&mut self, cx: &Context, scale_factor: f32) {
        let _context_guard = crate::bind_context(cx);
        // 1. 前面と背面に分類されたバッチを Context から引き出す
        let render_data = cx.collect_render_data();
        if render_data.batches.is_empty() {
            return;
        }

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

        // インスタンスの組み立て
        for batch in &render_data.batches {
            for (i, instance) in batch.instances.iter().enumerate() {
                let entity_id = batch.entity_ids[i];
                let inst = self.build_quad_instance_for_entity(cx, entity_id, instance);
                self.instance_staging.push(inst);
            }
        }

        // C. VRAM インスタンスバッファへの一括転送
        self.ensure_instance_buffer_capacity(self.instance_staging.len());
        self.queue.write_buffer(
            &self.instance_buffer,
            0,
            bytemuck::cast_slice(&self.instance_staging),
        );

        // PASS 1: 背面 (Background) の描画実行
        let mut encoder = self.device.create_command_encoder(&Default::default());
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
            for batch in &render_data.batches {
                let count = batch.instances.len() as u32;
                if count == 0 {
                    continue;
                }

                // くり抜きバッチであれば punchout_pipeline (REPLACEブレンド)、
                // 通常バッチであれば通常の pipeline (PMAブレンド) を設定します。
                match batch.batch_type {
                    BatchType::Normal => rpass.set_pipeline(&self.pipeline),
                    BatchType::Punchout => rpass.set_pipeline(&self.punchout_pipeline),
                }

                // 静止 WebView2 描画時のバインディングの切り替え
                self.bind_texture_for_batch(&mut rpass, batch);

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
                // 描画が不可能であるため、検証エラーを避けるためにこのバッチの描画を安全にスキップ（バイパス）します。
                if phys_x >= target_w || phys_y >= target_h {
                    // オフセットだけはスキップされた数分確実に進めます。
                    instance_offset += count;
                    continue;
                }

                // 開始位置 + 幅/高さがレンダーターゲットをはみ出さないように厳密にクランプ
                let clamped_w = phys_w.min(target_w - phys_x);
                let clamped_h = phys_h.min(target_h - phys_y);

                // wgpu の制約回避のため、クランプ後の幅・高さが 0 の場合も描画をスキップ
                if clamped_w == 0 || clamped_h == 0 {
                    instance_offset += count;
                    continue;
                }

                // 完全に境界内に収まるように安全化された Scissor Rect を適用
                rpass.set_scissor_rect(phys_x, phys_y, clamped_w, clamped_h);

                rpass.draw_indexed(0..6, 0, instance_offset..(instance_offset + count));
                instance_offset += count;
            }
        }
        self.queue.submit(Some(encoder.finish()));
        surface_texture.present();

        // wgpuのデバイスを明示的にポーリングし、未解決のフェンスやリソースをフラッシュする
        self.device.poll(wgpu::PollType::Poll);
    }

    /// ヘルパー: バッチ内に静止 `WebView2` テクスチャが含まれる場合、バインドグループを動的に切り替える
    fn bind_texture_for_batch<'a>(&'a self, rpass: &mut wgpu::RenderPass<'a>, batch: &DrawBatch) {
        // バッチに含まれる最初の要素が静止 WebView2 キャッシュを持っているか
        if let Some(&first_id) = batch.entity_ids.first()
            && let Some(cached_view) = self.webview_static_caches.get(&first_id)
        {
            // 動的にそのテクスチャビューを割り当てたバインドグループを構築
            let temp_bind_group = self.device.create_bind_group(&wgpu::BindGroupDescriptor {
                layout: &self.config_bind_group_layout,
                entries: &[
                    wgpu::BindGroupEntry {
                        binding: 0,
                        resource: self.config_buffer.as_entire_binding(),
                    },
                    wgpu::BindGroupEntry {
                        binding: 1,
                        resource: wgpu::BindingResource::TextureView(cached_view), // アトラスの代わりに静止画を割り当て
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
                label: Some("Dynamic WebView2 Bind Group"),
            });
            // このドローコールの間だけ一時バインドグループをセット
            rpass.set_bind_group(0, &temp_bind_group, &[]);
        } else {
            // 通常はグローバル（標準アトラス）のバインドグループを使用
            rpass.set_bind_group(0, &self.config_bind_group, &[]);
        }
    }

    /// ヘルパー: 各 `EntityId` の属性から GPU 用の `QuadInstance` を正確に構築
    fn build_quad_instance_for_entity(
        &mut self,
        cx: &Context,
        entity_id: EntityId,
        instance: &QuadInstance,
    ) -> QuadInstance {
        let (basic, flex, _) = cx.resolve_active_layouts(entity_id);
        let default_visual = VisualProperty::default();
        let visual = cx
            .renders
            .visual_properties
            .get(entity_id)
            .unwrap_or(&default_visual);

        let origin = visual.transform_origin.map_or([0.5, 0.5], |p| [p.x, p.y]);
        let opacity = visual.opacity.unwrap_or(1.0);

        let box_sizing_val = match basic.box_sizing {
            BoxSizing::BorderBox => 0.0f32,
            BoxSizing::ContentBox => 1.0f32,
        };

        // 四辺個別長さの抽出（設定が無ければ 1.0 (100% 描画) とする）
        let border_lengths = visual.border_lengths.unwrap_or(EdgeInsets::px_all(1.0));
        let styles = visual.border_styles.unwrap_or([BorderStyle::Solid; 4]);
        let aligns = visual
            .border_alignments
            .unwrap_or([BorderAlignment::Start; 4]);

        let mut border_flags = 0u32;
        for i in 0..4 {
            let s_val = styles[i] as u32; // 0..3 (2ビット)
            let a_val = aligns[i] as u32; // 0..2 (2ビット)

            border_flags |= s_val << (i * 4); // スタイル用： bit 0, 4, 8, 12 起点
            border_flags |= a_val << (i * 4 + 2); // アライメント用： bit 2, 6, 10, 14 起点
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

        // 影 (BoxShadow) のデータを選別して適用
        // テンプレート側が影なしを指定している場合は SoA を無視して完全透明にする
        let mut shadow_color =
            if instance.shadow_color != Color::TRANSPARENT && visual.shadow_params.is_some() {
                visual.shadow_color.unwrap_or_default()
            } else {
                Color::TRANSPARENT // テンプレートが透明を指定、または SoA に形状が無いなら影を完全無効化
            };

        // WebView2 がアクティブ（昇格表示）の時は、
        // 透過スナップショットを突き抜けて DComp コンポジターでブレンドされるため、
        // 影の黒さが非線形（ガンマ空間）で強調されて濃く見えてしまう。
        // これを防ぐため、親要素に COMP_WEBVIEW_CONTENT があり、かつそれが active_webviews (準備完了) に
        // 入っている場合は、影のアルファを 45% に補正して、静止画キャッシュ時と視覚的な濃さを統一。
        if shadow_color != Color::TRANSPARENT {
            let mut has_active_webview_parent = false;
            let mut curr_id = entity_id;
            while let Some(Some(parent_id)) = cx.topology.parents.get(curr_id) {
                if cx.topology.active_masks[*parent_id].has_webveiw2_content()
                    && cx.renders.active_webviews.contains(parent_id)
                {
                    has_active_webview_parent = true;
                    break;
                }
                curr_id = *parent_id;
            }

            if has_active_webview_parent {
                shadow_color.a *= 0.45;
            }
        }

        // 影のパラメータ（形状）も上記カラーが透明なら 0 に落とす
        let shadow_params = if shadow_color == Color::TRANSPARENT {
            [0.0; 4]
        } else {
            match visual.shadow_params {
                Some(shadow) => [shadow.offset.x, shadow.offset.y, shadow.blur, shadow.spread],
                None => [0.0; 4],
            }
        };

        let is_decorator = instance.opacity_mode_sizing[1] < -0.5; // mode == -1.0 なら true
        let is_text_body = (instance.opacity_mode_sizing[1] - 2.0).abs() < 0.01; // mode == 2.0 なら true

        let mut current_mode = if is_decorator {
            -1.0f32
        } else if is_text_body {
            2.0f32
        } else if visual.bg_gradient.is_some() {
            1.0f32
        } else {
            0.0f32
        };
        let mut uv_min = instance.uv_min; // collect_render_data 側での指定値を維持
        let mut uv_max = instance.uv_max;

        let mut final_rect = instance.rect;
        let mut final_color = instance.color;

        if is_text_body && cx.topology.active_masks[entity_id].has_text_content() {
            let spans = cx
                .contents
                .text_spans
                .get(entity_id)
                .map_or(&[][..], Vec::as_slice);

            let text_size = if cx.topology.active_masks[entity_id].has_input_content()
                && let Some(contents) = cx.contents.input_contents.get(entity_id)
                && let Some(layout_rect) = contents.last_layout
            {
                LayoutSize::new(layout_rect.width, layout_rect.height)
            } else {
                let text = &cx.contents.text_contents[entity_id];
                let visual = cx
                    .renders
                    .visual_properties
                    .get(entity_id)
                    .unwrap_or(&default_visual);
                let layout = cx.system.text_engine.create_layout(
                    text,
                    visual.font_size.unwrap_or(16.0),
                    visual.font_family.as_deref(),
                    visual.font_weight,
                    visual.font_style,
                    None,
                    spans,
                );
                cx.system.text_engine.get_layout_size(&layout)
            };

            let rect = OutputStore::rect(entity_id, &cx.outputs.rects).unwrap_or_default();
            let (border, padding) =
                LayoutStore::get_physical_border_padding(rect, basic.border, basic.padding);

            // スクロールオフセット
            let scroll = cx
                .outputs
                .scroll_offsets
                .get(entity_id)
                .copied()
                .unwrap_or(LayoutPoint::ZERO);

            let align_offset =
                OutputStore::calc_align_offset(rect, border, padding, text_size, flex.text_align);

            // 完全に整数ピクセルサイズにスナップし、にじみとピクピク揺れを完全に阻止
            final_rect = LayoutRect::new(
                instance.rect.x + border.left + padding.left + align_offset.x - scroll.x,
                instance.rect.y + border.top + padding.top + align_offset.y - scroll.y,
                text_size.width.ceil(),
                text_size.height.ceil(),
            );
        }

        // 静止 WebView2 キャッシュの引き当て判定
        if !is_decorator && let Some(_cached_view) = self.webview_static_caches.get(&entity_id) {
            // 描画モードを 3.0f32 (静止 WebView2 サンプリング) にスイッチ
            current_mode = 3.0;
            uv_min = [0.0, 0.0];
            uv_max = [1.0, 1.0];
        } else if is_text_body && cx.topology.active_masks[entity_id].has_text_content() {
            // テキスト要素である場合
            let text = cx
                .contents
                .text_contents
                .get(entity_id)
                .cloned()
                .unwrap_or_else(|| "".into());

            let font_size = cx
                .renders
                .visual_properties
                .get(entity_id)
                .and_then(|v| v.font_size)
                .unwrap_or(16.0);

            // IME 未確定テキストが入力中か否かを判定
            let is_ime_active = cx
                .contents
                .input_contents
                .get(entity_id)
                .and_then(|c| c.ime_state.as_ref())
                .is_some_and(|ime| !ime.composition_text.is_empty());

            let base_text_empty = cx
                .contents
                .input_contents
                .get(entity_id)
                .is_some_and(|c| c.text.0.get().is_empty());

            let placeholder_color = cx
                .contents
                .input_contents
                .get(entity_id)
                .and_then(|p| p.placeholder_color)
                .unwrap_or(Color::rgb_f32(0.5, 0.5, 0.5));

            let resolved_color = if base_text_empty && !is_ime_active {
                // プレースホルダー時は半透明の薄いグレー
                // 確定文字列が空、かつ IME 未変換も空の場合のみプレースホルダー色
                placeholder_color
            } else {
                // 通常文字入力中はユーザー指定色、無ければ不透明白
                cx.renders
                    .visual_properties
                    .get(entity_id)
                    .and_then(|v| v.text_color)
                    .unwrap_or(Color::WHITE)
            };

            let mut spans = cx
                .contents
                .text_spans
                .get(entity_id)
                .cloned()
                .unwrap_or_else(Vec::new);

            // 選択範囲がある場合、ハイライトスパンをキャッシュ判定の前にマージ
            if let Some(selection) = cx.outputs.text_selections.get(entity_id)
                && selection.start < selection.end
                && let Some(sel_text) = visual.select_text_color
            {
                spans.push(TextSpan {
                    range: selection.clone(),
                    color: Some(sel_text), // 文字色の変更がある時だけアトラス側でラスタライズ
                    bg_color: None,        // 背景色は wgpu-Quad 側に描画させるためここでは None
                    underline: None,
                    ..Default::default()
                });
            }

            // マージされたスパン全体から正確なキャッシュ用ハッシュ値を算出
            let spans_hash = crate::hash_text_spans(&spans);

            let text_clone = text.clone();
            let key = TextCacheKey {
                text,
                font_size_bits: (font_size * cx.window.scale_factor).to_bits(),
                font_style: cx
                    .renders
                    .visual_properties
                    .get(entity_id)
                    .and_then(|v| v.font_style),
                font_family: cx
                    .renders
                    .visual_properties
                    .get(entity_id)
                    .and_then(|f| f.font_family.clone()),
                font_weight: cx
                    .renders
                    .visual_properties
                    .get(entity_id)
                    .and_then(|v| v.font_weight),
                spans_hash,
            };

            let uv = if let Some(cached) = self.text_cache.get(&key) {
                (cached.uv_min, cached.uv_max)
            } else {
                let physical_font_size = font_size * cx.window.scale_factor;

                let mut spans = cx
                    .contents
                    .text_spans
                    .get(entity_id)
                    .cloned()
                    .unwrap_or_else(Vec::new);

                // 選択範囲が存在する場合、カラーハイライト用の TextSpan を動的にマージ
                if let Some(selection) = cx.outputs.text_selections.get(entity_id)
                    && selection.start < selection.end
                    && let Some(sel_text) = visual.select_text_color
                {
                    spans.push(TextSpan {
                        range: selection.clone(),
                        color: Some(sel_text),
                        bg_color: None,
                        underline: None,
                        ..Default::default()
                    });
                }

                let physical_layout = cx.system.text_engine.create_layout(
                    &text_clone,
                    physical_font_size,
                    cx.renders
                        .visual_properties
                        .get(entity_id)
                        .and_then(|v| v.font_family.as_deref()),
                    cx.renders
                        .visual_properties
                        .get(entity_id)
                        .and_then(|v| v.font_weight),
                    cx.renders
                        .visual_properties
                        .get(entity_id)
                        .and_then(|v| v.font_style),
                    None,
                    &spans,
                );

                let size = cx.system.text_engine.get_layout_size(&physical_layout);
                let r8_pixels = self.text_rasterizer.rasterize(
                    &physical_layout,
                    size,
                    &spans,
                    &cx.system.text_engine.rendering_params,
                );

                let width = size.width.ceil() as u32;
                let height = size.height.ceil() as u32;

                let mut alloc_res = self.atlas.allocate(width, height);

                if alloc_res.is_none() {
                    self.atlas.clear();
                    self.text_cache.clear();
                    alloc_res = self.atlas.allocate(width, height);
                }

                let (x, y) = alloc_res.expect("Text exceeds maximum atlas size!");

                self.queue.write_texture(
                    wgpu::TexelCopyTextureInfo {
                        texture: &self.atlas.texture,
                        mip_level: 0,
                        origin: wgpu::Origin3d { x, y, z: 0 },
                        aspect: wgpu::TextureAspect::All,
                    },
                    &r8_pixels,
                    wgpu::TexelCopyBufferLayout {
                        offset: 0,
                        bytes_per_row: Some(width), // 1ピクセルあたり1バイト
                        rows_per_image: Some(height),
                    },
                    wgpu::Extent3d {
                        width,
                        height,
                        depth_or_array_layers: 1,
                    },
                );

                let (uv_min, uv_max) = self.atlas.texel_to_uv(x, y, width, height);
                self.text_cache
                    .insert(key, TextCacheValue { uv_min, uv_max });
                (uv_min, uv_max)
            };

            current_mode = 2.0; // テキストモード（2.0f32）
            uv_min = uv.0;
            uv_max = uv.1;

            final_color = resolved_color;
        }

        // 解決済みの累積トランスフォーム行列
        let packed_transform = instance.transform;

        // 完全に 16B 境界にアラインされたインスタンス構造体をビルド
        QuadInstance {
            rect: final_rect,
            transform: packed_transform,
            transform_origin: origin,
            color: final_color,
            corner_radius: visual.corner_radius.unwrap_or(CornerRadius::ZERO),
            border_width: instance.border_width,
            border_color: visual.border_color.unwrap_or(Color::TRANSPARENT),
            border_lengths,
            opacity_mode_sizing: [opacity, current_mode, box_sizing_val, border_flags as f32],
            uv_min,
            uv_max,
            gradient_end_color: instance.gradient_end_color,
            gradient_angle: instance.gradient_angle,
            _padding: 0.0,
            shadow_color,
            shadow_params,
            outline_width: o_width,
            outline_color: o_color,
            outline_lengths: o_lengths,
            outline_offset_and_flags: [o_offset, outline_flags as f32, 0.0, 0.0],
        }
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
