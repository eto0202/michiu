#![allow(dead_code)]
use crate::{BatchType, DrawBatch};
use crate::{
    COMP_TEXT_CONTENT, Color, Context, CornerRadius, EdgeInsets, EntityId, IDENTITY_MATRIX,
    LayoutRect, LayoutSize, QuadInstance, TextCacheKey, TextCacheValue, TextRasterizer,
    TextureAtlas, VisualProperty, renderer::Vertex,
};
use raw_window_handle::{
    RawDisplayHandle, RawWindowHandle, Win32WindowHandle, WindowsDisplayHandle,
};
use slotmap::SecondaryMap;
use std::collections::HashMap;
use std::num::NonZeroIsize;
use wgpu::CurrentSurfaceTexture;
use wgpu::util::DeviceExt;
use windows::Win32::Foundation::HANDLE;
use windows::Win32::Graphics::Direct3D11::{
    D3D11_BIND_RENDER_TARGET, D3D11_BIND_SHADER_RESOURCE, D3D11_RESOURCE_MISC_SHARED,
    D3D11_RESOURCE_MISC_SHARED_NTHANDLE, D3D11_TEXTURE2D_DESC, D3D11_USAGE_DEFAULT, ID3D11Device,
    ID3D11Texture2D,
};
use windows::Win32::Graphics::Direct3D12::ID3D12Resource;
use windows::Win32::Graphics::Dxgi::Common::{DXGI_FORMAT_B8G8R8A8_UNORM, DXGI_SAMPLE_DESC};
use windows::Win32::Graphics::Dxgi::{
    DXGI_SHARED_RESOURCE_READ, DXGI_SHARED_RESOURCE_WRITE, IDXGIResource1,
};
use windows::core::Interface;

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
            flags: wgpu::InstanceFlags::default(),
            memory_budget_thresholds: wgpu::MemoryBudgetThresholds::default(),
            backend_options: wgpu::BackendOptions {
                dx12: wgpu::Dx12BackendOptions {
                    // Dxc (最新) または Fxc (レガシー) を選択。Windows 10/11 なら Dxc 推奨
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
                power_preference: wgpu::PowerPreference::HighPerformance,
                compatible_surface: Some(&surface), // 代表として bg で適合検証
                force_fallback_adapter: false,
            })
            .await
            .map_err(|_| "Failed to find an appropriate adapter")?;

        // 4. デバイスとキューの取得
        let (device, queue) = adapter
            .request_device(&wgpu::DeviceDescriptor {
                label: Some("Michiu Renderer Device"),
                required_features: wgpu::Features::empty(),
                // DX12 ならば制限（Limits）は比較的一般的
                required_limits: wgpu::Limits::default(),
                memory_hints: wgpu::MemoryHints::default(),
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
            .find(|f| f.is_srgb()) // SRGBを優先
            .unwrap_or(caps.formats[0]);

        let config = wgpu::SurfaceConfiguration {
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
            format: surface_format, // 通常は Rgba8UnormSrgb か Bgra8UnormSrgb
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
                ],
                label: None,
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
                    // Instance Buffer (QuadInstance のレイアウト)
                    QuadInstance::desc(), // 以前定義した属性レイアウト
                ],
                compilation_options: Default::default(),
            },
            fragment: Some(wgpu::FragmentState {
                module: &shader,
                entry_point: Some("fs_main"),
                targets: &[Some(wgpu::ColorTargetState {
                    format: config.format,
                    blend: Some(wgpu::BlendState::PREMULTIPLIED_ALPHA_BLENDING),
                    write_mask: wgpu::ColorWrites::ALL,
                })],
                compilation_options: Default::default(),
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
                buffers: &[
                    wgpu::VertexBufferLayout {
                        array_stride: std::mem::size_of::<Vertex>() as wgpu::BufferAddress,
                        step_mode: wgpu::VertexStepMode::Vertex,
                        attributes: &[wgpu::VertexAttribute {
                            format: wgpu::VertexFormat::Float32x2,
                            offset: 0,
                            shader_location: 0,
                        }],
                    },
                    QuadInstance::desc(),
                ],
                compilation_options: Default::default(),
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
                            // アルファが 0.0 に消去される場所では、カラーも同時に 0.0 になるように
                            // OneMinusSrcAlpha で元の背景色を減算（消去）します。
                            dst_factor: wgpu::BlendFactor::OneMinusSrcAlpha,
                            operation: wgpu::BlendOperation::Add,
                        },
                        alpha: wgpu::BlendComponent {
                            src_factor: wgpu::BlendFactor::Zero,
                            dst_factor: wgpu::BlendFactor::OneMinusSrcAlpha, // アルファは SDF マスク (src) 分だけ消去
                            operation: wgpu::BlendOperation::Add,
                        },
                    }),
                    write_mask: wgpu::ColorWrites::ALL,
                })],
                compilation_options: Default::default(),
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
            label: Some("Instance Buffer"),
            size: (instance_buffer_capacity * std::mem::size_of::<QuadInstance>())
                as wgpu::BufferAddress,
            usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
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
    /// new_physical_size: (width, height)
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
        // 1. 前面と背面に分類されたバッチを Context から引き出す
        let render_data = cx.collect_render_data();
        if render_data.batches.is_empty() {
            return;
        }

        // スワップチェーンから描画先フレームを獲得
        let surface_texture = match self.surface.get_current_texture() {
            wgpu::CurrentSurfaceTexture::Success(t) => t,
            _ => return,
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
            rpass.set_bind_group(0, &self.config_bind_group, &[]);
            rpass.set_vertex_buffer(0, self.vertex_buffer.slice(..));
            rpass.set_vertex_buffer(1, self.instance_buffer.slice(..));
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
                rpass.set_scissor_rect(
                    (clip.x * scale_factor).round().max(0.0) as u32,
                    (clip.y * scale_factor).round().max(0.0) as u32,
                    (clip.width * scale_factor).round().max(0.0) as u32,
                    (clip.height * scale_factor).round().max(0.0) as u32,
                );

                rpass.draw_indexed(0..6, 0, instance_offset..(instance_offset + count));
                instance_offset += count;
            }
        }
        self.queue.submit(Some(encoder.finish()));
        surface_texture.present();
    }

    /// ヘルパー: バッチ内に静止 WebView2 テクスチャが含まれる場合、バインドグループを動的に切り替える
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

    /// ヘルパー: 各 EntityId の属性から GPU 用の QuadInstance を正確に構築
    fn build_quad_instance_for_entity(
        &mut self,
        cx: &Context,
        entity_id: EntityId,
        instance: &QuadInstance,
    ) -> QuadInstance {
        let (basic, _, _) = cx.resolve_active_layouts(entity_id);
        let default_visual = VisualProperty::default();
        let visual = cx
            .visual_properties
            .get(entity_id)
            .unwrap_or(&default_visual);

        let origin = visual
            .transform_origin
            .map(|p| [p.x, p.y])
            .unwrap_or([0.5, 0.5]);
        let opacity = visual.opacity.unwrap_or(1.0);

        let mut current_mode = if visual.bg_gradient.is_some() {
            1.0f32
        } else {
            0.0f32
        };
        let mut uv_min = [0.0f32; 2];
        let mut uv_max = [0.0f32; 2];

        // 静止 WebView2 キャッシュの引き当て判定
        if let Some(_cached_view) = self.webview_static_caches.get(&entity_id) {
            // 描画モードを 3.0f32 (静止 WebView2 サンプリング) にスイッチ
            current_mode = 3.0;
            uv_min = [0.0, 0.0];
            uv_max = [1.0, 1.0];
        } else if cx.active_masks[entity_id].has(COMP_TEXT_CONTENT) {
            // テキスト要素である場合
            let text = &cx.text_contents[entity_id];
            let font_size = cx
                .visual_properties
                .get(entity_id)
                .and_then(|v| v.font_size)
                .unwrap_or(16.0);

            let key = TextCacheKey {
                text: text.to_string(),
                font_size_bits: font_size.to_bits(),
                font_style: None,
                font_family: None,
                font_weight: None,
            };

            let uv = if let Some(cached) = self.text_cache.get(&key) {
                (cached.uv_min, cached.uv_max)
            } else {
                let layout = cx.text_engine.create_layout(
                    text,
                    font_size,
                    key.font_family.as_deref(),
                    key.font_weight,
                    key.font_style,
                    None,
                );

                let size = cx.text_engine.get_layout_size(&layout);
                let pixels = self.text_rasterizer.rasterize(&layout, size);

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
                    &pixels,
                    wgpu::TexelCopyBufferLayout {
                        offset: 0,
                        bytes_per_row: Some(width * 4),
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
        }

        // 完全に 16B 境界にアラインされたインスタンス構造体をビルド
        QuadInstance {
            rect: instance.rect,
            transform: visual.transform.unwrap_or(IDENTITY_MATRIX),
            transform_origin: origin,
            color: instance.color,
            corner_radius: visual.corner_radius.unwrap_or(CornerRadius::ZERO),
            border_width: EdgeInsets {
                top: basic.border.top.into(),
                right: basic.border.right.into(),
                bottom: basic.border.bottom.into(),
                left: basic.border.left.into(),
            },
            border_color: visual.border_color.unwrap_or(Color::TRANSPARENT),

            // 【パック】[opacity, mode, 0.0, 0.0] の 16B 転送
            opacity_and_mode: [opacity, current_mode, 0.0, 0.0],

            uv_min,
            uv_max,
            gradient_end_color: instance.gradient_end_color,
            gradient_angle: instance.gradient_angle,
            _padding: 0.0,
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
                label: Some("Instance Buffer"),
                size,
                usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
                mapped_at_creation: false,
            });

            self.instance_buffer_capacity = new_capacity;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::renderer::text::TextEngine;
    use crate::{Color, Context, CornerRadius, Size, build_ui, div, setup_direct_composition, ts};
    use std::borrow::Cow;
    use windows::Win32::Foundation::*;
    use windows::Win32::System::Com::{COINIT_APARTMENTTHREADED, CoInitializeEx, CoUninitialize};
    use windows::Win32::System::LibraryLoader::GetModuleHandleW;
    use windows::Win32::UI::WindowsAndMessaging::*;
    use windows::core::Interface;

    struct ComGuard {
        _private: (),
    }

    impl ComGuard {
        fn new() -> Result<Self, windows::core::Error> {
            unsafe {
                // UIスレッド用の STA アパートメントとして COM を初期化
                CoInitializeEx(None, COINIT_APARTMENTTHREADED);
            }
            Ok(Self { _private: () })
        }
    }

    impl Drop for ComGuard {
        fn drop(&mut self) {
            unsafe {
                // スコープを抜ける（テストが終了する）際に、自動でアンロードが走る
                CoUninitialize();
            }
        }
    }

    // DirectWrite と Direct2D (WIC)
    #[test]
    fn test_text_rasterizer_and_metrics() {
        // テストスレッドの COM アパートメントを初期化
        let _com = ComGuard::new().unwrap();

        // DirectWrite エンジンの初期化
        let engine = TextEngine::new();
        let rasterizer = TextRasterizer::new();

        let sample_text = "Michiu GUI テスト";
        let font_size = 24.0;

        // 1. テキストの計測を検証
        let size = engine.measure_text(sample_text, font_size, None, None, None, None);
        assert!(size.width > 0.0);
        assert!(size.height > 0.0);

        // 2. ラスタライズの実行を検証
        let layout = engine.create_layout(sample_text, font_size, None, None, None, None);
        let size = engine.get_layout_size(&layout);
        let pixels = rasterizer.rasterize(&layout, size);

        // ピクセルバッファのサイズが正しく RGBA8 (width * height * 4) になっているか検証
        let expected_width = (size.width.ceil() as u32).max(1);
        let expected_height = (size.height.ceil() as u32).max(1);
        let expected_len = (expected_width * expected_height * 4) as usize;

        assert_eq!(pixels.len(), expected_len);

        // 描画されたピクセル（テキスト部分）に不透明なピクセルが存在するか（すべて透明でないか）確認
        let has_content = pixels.iter().any(|&p| p > 0);
        assert!(
            has_content,
            "Rasterized image should not be completely empty"
        );
    }

    // wgpu (DX12) & DirectComposition

    // テスト用のウィンドウプロシージャ
    unsafe extern "system" fn dummy_wnd_proc(
        hwnd: HWND,
        msg: u32,
        wparam: WPARAM,
        lparam: LPARAM,
    ) -> LRESULT {
        unsafe { DefWindowProcW(hwnd, msg, wparam, lparam) }
    }

    // テスト用のダミーメッセージウィンドウを作成するヘルパー
    unsafe fn create_dummy_window() -> HWND {
        let h_instance = unsafe { GetModuleHandleW(None).unwrap() };
        let class_name = windows::core::w!("MichiuTestWindowClass");

        let wnd_class = WNDCLASSW {
            lpfnWndProc: Some(dummy_wnd_proc),
            hInstance: h_instance.into(),
            lpszClassName: class_name,
            ..Default::default()
        };

        unsafe { RegisterClassW(&wnd_class) };

        // 画面に表示されない、テスト用のメッセージ専用ウィンドウ
        unsafe {
            CreateWindowExW(
                WINDOW_EX_STYLE::default(),
                class_name,
                windows::core::w!("Test Window"),
                WS_OVERLAPPEDWINDOW,
                CW_USEDEFAULT,
                CW_USEDEFAULT,
                100,
                100,
                Some(HWND_MESSAGE), // メッセージウィンドウ指定
                Some(HMENU::default()),
                Some(HINSTANCE(h_instance.0)),
                None,
            )
            .unwrap()
        }
    }
}
