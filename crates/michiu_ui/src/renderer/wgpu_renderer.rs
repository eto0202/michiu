//! # `WgpuRenderer`
//!
//! The core UI renderer for `michiu_ui`, powered by wgpu (DirectX 12).
//! It achieves minimal draw calls through **instanced rendering** against a shared 1x1 quad mesh, combined with separate batch pipelines for normal and punch-out passes.
//!
//! ---
//!
//! ## Rendering Pipeline Lifecycle ([`WgpuRenderer::render`])
//!
//! A single frame progresses linearly through the following seven stages:
//!
//! ```text
//! [ Context ]
//!      │ 1. collect_render_data
//!      ▼
//! [ RenderData ] ─── 2. build_quad_instance_for_entity ───► [ instance_staging ]
//!                                                                 │ 3. queue.write_buffer
//! ┌───────────────────────────────────────────────────────────────┘
//! ▼
//! [ VRAM Instance Buffer ]
//!      │
//!      │ 4. begin_render_pass (Bind 1x1 Quad & Config Uniform)
//!      │
//!      │ 5. Batch Loop (for batch in render_data.batches)
//!      │     ├─ Pipeline Selection (Normal: PMA / Punchout: Attenuation Blend)
//!      │     ├─ Texture Binding (Atlas / ExternalTexture)
//!      │     ├─ set_scissor_rect (Clipping bounds)
//!      │     └─ draw_indexed (Instanced draw call)
//!      │
//!      ▼
//! 6. queue.submit & surface.present
//!      │
//!      ▼
//! 7. Return render_data to self (Zero-allocation memory reuse)
//! ```
//!
//! ---
//!
//! ### Pipeline Stage Breakdown
//!
//! #### Step 1: Collecting Render Data (`collect_render_data`)
//! Pulls entity data scheduled for the current frame from the `Context`'s SoA via `collect_render_data`.
//! Updates the dynamic texture atlas and text layout cache through `RendererView`, generating batches segregated into foreground and background passes.
//!
//! #### Step 2: Building the Instance Buffer
//! `michiu_ui` avoids per-entity vertex buffer allocations.
//! All UI primitives (rectangles, rounded panels, borders, text quads) are transformed into [`QuadInstance`] data mapped over a single 1x1 quad mesh. Properties such as bounds, corners, SDF parameters, and colors are packed into `instance_staging`.
//!
//! #### Step 3: VRAM Upload
//! Uploads the packed instance array to the GPU's `instance_buffer` in a single contiguous transfer via `queue.write_buffer`.
//!
//! #### Steps 4–5: Render Pass and Batch Dispatch
//! Instantiates a command encoder, binds the shared quad buffers and viewport uniforms (`config_bind_group`), and iterates through render batches:
//!
//! * **Dynamic Pipeline Switching**:
//!   * `BatchType::Normal`: Standard UI entities using Pre-Multiplied Alpha (PMA) compositing.
//!   * `BatchType::Punchout`: Transparent cutouts for `ExternalVisual`s using `Zero` + `OneMinusSrcAlpha` blending.
//!   * *Note: Pipeline bindings are cached and only re-bound on batch type transitions to minimize driver state changes.*
//! * **Dynamic Texture Binding**:
//!   Switches between the glyph atlas and cached static captures (`ExternalTexture` in `external_bind_groups`) according to batch requirements.
//! * **Hardware Scissor Clipping**:
//!   Constrains layout overflows (`overflow: hidden`) and scroll viewports via physical-pixel `set_scissor_rect` calls.
//! * **Draw Invocation**:
//!   Executes instanced draw calls via `draw_indexed` across all quads within the active batch.
//!
//! #### Steps 6–7: Submission and Zero-Allocation Reclamation
//! Submits encoded command buffers to the GPU queue and presents the swap chain surface.
//! Reclaims `render_data` back into `self.render_data` to preserve vector capacities for subsequent frames, eliminating recurrent heap allocations.
//!
//! ---
//!
//! ## Key Struct Fields
//!
//! | Field | Type | Responsibility |
//! | :--- | :--- | :--- |
//! | **`surface` / `device` / `queue`** | wgpu core types | DirectX 12 logical device, command queue, and presentation surface |
//! | **`pipeline`** | `wgpu::RenderPipeline` | Standard UI rendering pipeline (Pre-Multiplied Alpha) |
//! | **`punchout_pipeline`** | `wgpu::RenderPipeline` | Cutout pipeline for ExternalVisuals (attenuation blending) |
//! | **`vertex_buffer` / `index_buffer`** | `wgpu::Buffer` | Shared 1x1 quad unit mesh used across all primitives |
//! | **`instance_buffer`** | `wgpu::Buffer` | GPU instance storage holding quad transforms, bounds, and SDF data |
//! | **`atlas`** | `TextureAtlas` | Dynamic font glyph and icon atlas |
//! | **`text_cache`** | `FxHashMap` | Cache for shaped text layouts and positioned glyph clusters |
//! | **`external_bind_groups`** | `FxHashMap` | Cached bind groups for static image fallbacks (`ExternalTexture`) |
//!
//! ---
//!
//! ## Low-Level Operations and Escape Hatches ([`RawWgpuRenderer<'a>`] & [`RawMichiuRenderer<'a>`])
//!
//! While structural fields are encapsulated via `pub(crate)`, advanced extensions can bypass abstractions through [`raw_wgpu_renderer_mut`](WgpuRenderer::raw_wgpu_renderer_mut) and [`raw_michiu_renderer_mut`](MichiuRenderer::raw_michiu_renderer_mut).
//!
//! ### Internal Design: Split Borrowing via Raw Renderers
//!
//! [`MichiuRenderer`] mirrors `Context`'s split-borrow design. [`RawWgpuRenderer`] can be acquired exclusively through [`MichiuRenderer::raw_wgpu_renderer_mut`](MichiuRenderer::raw_wgpu_renderer_mut):
//!
//! ```rust,ignore
//! // Deconstruct renderer fields into independent mutable borrows
//! let mut raw_michiu = renderer.raw_michiu_renderer_mut();
//! let mut raw_wgpu = raw_michiu.raw_wgpu_renderer_mut();
//!
//! // Safely dispatch distinct GPU resources across disparate functions concurrently
//! upload_resources(raw_wgpu.device, raw_wgpu.queue, raw_wgpu.instance_buffer);
//! update_atlas(raw_wgpu.atlas, raw_wgpu.temp_uv_map);
//! ```
//!
//! For consumer applications, safe read access is provided through immutable getters such as [`d3d11_device(&self)`](MichiuRenderer::d3d11_device) and [`surface(&self)`](WgpuRenderer::surface).

use crate::{
    BatchType, BorderAlignment, BorderStyle, BoxSizing, Color, Context, DEFAULT_BASIC,
    DEFAULT_FLEX, DrawBatch, EdgeInsets, Entity, LayoutSize, MichiuError, MichiuSoA, Pipeline,
    QuadInstance, RenderData, RendererView, TextCacheKey, TextCacheValue, TextureAtlas, Vertex,
    VisualProperty,
};
use rustc_hash::FxHashMap;
use slotmap::SecondaryMap;
use wgpu::util::DeviceExt;

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

    pub(crate) atlas: TextureAtlas,
    pub(crate) temp_uv_map: SecondaryMap<Entity, [f32; 4]>,
    pub(crate) text_cache: FxHashMap<TextCacheKey, TextCacheValue>,

    pub(crate) render_data: RenderData,

    // 外部テクスチャ用の `BindGroup` キャッシュ
    pub(crate) external_bind_groups: FxHashMap<Entity, (wgpu::TextureView, wgpu::BindGroup)>,
}

#[repr(C)]
#[derive(Copy, Clone, bytemuck::Pod, bytemuck::Zeroable)]
struct GlobalConfig {
    pub(crate) screen_size: [f32; 2],
    pub(crate) scale: f32,
    pub(crate) _padding: f32, // std140のアライメント（16バイト境界）に合わせるためのパディング
}

impl WgpuRenderer {
    /// Initializes wgpu.
    ///
    /// # Safety
    ///
    /// The caller must satisfy the following safety requirements:
    /// - `visual` must be a raw pointer to a valid `IDCompositionVisual` (COM object).
    /// - The object pointed to by `visual` must not be released or destroyed until the created instance is destroyed.。
    ///
    /// # Errors
    ///
    /// Returns an error if the initialization pipeline fails:
    /// - [`MichiuError::CreateSurfaceError`]: Failed to create a surface from `visual` (e.g., invalid pointer or DirectComposition inconsistency).
    /// - [`MichiuError::RequestAdapterError`]: When a GPU (adapter) that meets the requirements cannot be found.
    /// - [`MichiuError::RequestDeviceError`]: When creating a logical device or queue fails (e.g., insufficient VRAM or driver crash).
    pub async unsafe fn new(
        visual: *mut std::ffi::c_void, // wgpu ビジュアル (前面)
        size: LayoutSize,
        scale_factor: f32,
    ) -> crate::Result<Self> {
        // インスタンス生成（DX12を明示的に指定）
        let instance = WgpuRenderer::create_instance();

        // Surface の作成 (CompositionVisual を使用)
        // create_surface_unsafe を利用して渡された生ポインタから Surface を構築
        let target = wgpu::SurfaceTargetUnsafe::CompositionVisual(visual);
        let surface = unsafe {
            instance
                .create_surface_unsafe(target)
                .map_err(|e| MichiuError::CreateSurfaceError { source: e })?
        };

        // アダプターの取得
        let adapter = instance
            .request_adapter(&wgpu::RequestAdapterOptions {
                power_preference: wgpu::PowerPreference::default(),
                compatible_surface: Some(&surface),
                force_fallback_adapter: false,
                apply_limit_buckets: false,
            })
            .await
            .map_err(|e| MichiuError::RequestAdapterError { source: e })?;

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
            .await
            .map_err(|e| MichiuError::RequestDeviceError { source: e })?;

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
            atlas,
            temp_uv_map: SecondaryMap::new(),
            text_cache: FxHashMap::default(),
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

        #[expect(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
        wgpu::SurfaceConfiguration {
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
            format: surface_format,
            width: size.width.max(1.0) as u32,
            height: size.height.max(1.0) as u32,
            present_mode: wgpu::PresentMode::Fifo, // VSync 有効
            alpha_mode: wgpu::CompositeAlphaMode::PreMultiplied,
            view_formats: vec![],
            desired_maximum_frame_latency: 2,
            color_space: wgpu::SurfaceColorSpace::Auto,
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
                buffers: &[Some(wgpu::VertexBufferLayout {
                    array_stride: std::mem::size_of::<Vertex>() as wgpu::BufferAddress,
                    step_mode: wgpu::VertexStepMode::Vertex,
                    attributes: &[wgpu::VertexAttribute {
                        format: wgpu::VertexFormat::Float32x2,
                        offset: 0,
                        shader_location: 0,
                    }],
                })],
                compilation_options: wgpu::PipelineCompilationOptions::default(),
            },
            fragment: Some(wgpu::FragmentState {
                module: shader,
                entry_point: Some("fs_main"),
                targets: &[Some(wgpu::ColorTargetState {
                    format: config.format,
                    blend: Some(wgpu::BlendState::PREMULTIPLIED_ALPHA_BLENDING),
                    write_mask: wgpu::ColorWrites::ALL,
                })],
                compilation_options: wgpu::PipelineCompilationOptions::default(),
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
                buffers: &[Some(wgpu::VertexBufferLayout {
                    array_stride: std::mem::size_of::<Vertex>() as wgpu::BufferAddress,
                    step_mode: wgpu::VertexStepMode::Vertex,
                    attributes: &[wgpu::VertexAttribute {
                        format: wgpu::VertexFormat::Float32x2,
                        offset: 0,
                        shader_location: 0,
                    }],
                })],
                compilation_options: wgpu::PipelineCompilationOptions::default(),
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
                compilation_options: wgpu::PipelineCompilationOptions::default(),
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
    #[expect(clippy::cast_precision_loss)]
    pub fn resize(&mut self, physical_width: u32, physical_height: u32, scale_factor: f32) {
        if physical_width > 0 && physical_height > 0 {
            self.config.width = physical_width;
            self.config.height = physical_height;

            self.surface.configure(&self.device, &self.config);

            let logical_width = physical_width as f32 / scale_factor;
            let logical_height = physical_height as f32 / scale_factor;

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

    pub fn render(&mut self, cx: &mut Context, scale_factor: f32) {
        let _context_guard = crate::bind_context(cx);
        // 破棄された要素のキャッシュを解放
        for id in cx.topology.topo_despawned_queue.drain(..) {
            self.external_bind_groups.remove(&id);
        }

        // 前面と背面に分類されたバッチを Context から引き出す
        Pipeline::collect_render_data(
            cx,
            &mut RendererView {
                render_data: &mut self.render_data,
                atlas: &mut self.atlas,
                text_cache: &mut self.text_cache,
                queue: &self.queue,
            },
        );

        if self.render_data.batches.is_empty() {
            return;
        }

        let render_data = std::mem::take(&mut self.render_data);

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

        #[cfg(feature = "trace-lifecycle")]
        crate::trace_lifecycle!(None, &mut cx.debug, || crate::MichiuTrace::WriteBuffer {
            staging: std::sync::Arc::from(self.instance_staging.clone()),
            add: None,
        });

        // 背面の描画実行
        let mut encoder = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor::default());
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
            // let mut instance_offset = 0;
            // 現在アクティブなパイプラインを記録
            let mut current_pipeline = None;

            #[expect(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
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

                // 静止 ExternalTexture 描画時のバインディングの切り替え
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
        self.queue.present(surface_texture);

        #[cfg(feature = "trace-lifecycle")]
        crate::trace_lifecycle!(None, &mut cx.debug, || crate::MichiuTrace::Present {
            add: None
        });

        // wgpuのデバイスを明示的にポーリングし、未解決のフェンスやリソースをフラッシュ
        let _ = self.device.poll(wgpu::PollType::Poll);

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

    /// バッチ内に外部テクスチャが含まれる場合、バインドグループを動的に切り替える
    fn bind_texture_for_batch<'a>(
        &'a self,
        rpass: &mut wgpu::RenderPass<'a>,
        batch: &DrawBatch,
        entity_ids: &[Entity],
    ) {
        if let Some(&first_id) = entity_ids.get(batch.instance_offset) {
            // 外部テクスチャのキャッシュが存在する場合
            if let Some((_, bind_group)) = self.external_bind_groups.get(&first_id) {
                rpass.set_bind_group(0, bind_group, &[]);
                return;
            }
        }

        // 通常のアトラス
        rpass.set_bind_group(0, &self.config_bind_group, &[]);
    }

    /// `QuadInstance` に静的バインドする
    #[track_caller]
    fn build_quad_instance_for_entity(
        cx: &mut Context,
        entity_id: Entity,
        instance: &QuadInstance,
    ) -> (QuadInstance, bool) {
        let basic =
            &cx.layouts
                .lay_resolved_basic
                .find_or(entity_id, &DEFAULT_BASIC, &mut cx.debug);
        let _flex = &cx
            .layouts
            .lay_resolved_flex
            .find_or(entity_id, &DEFAULT_FLEX, &mut cx.debug);
        let _grid = &cx
            .layouts
            .lay_resolved_grid
            .find_or_default(entity_id, &mut cx.debug);
        let default_visual = VisualProperty::default();
        let visual = cx
            .renders
            .rnd_visual
            .find(entity_id)
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
        // 元インスタンスで意図的に影が無効化されている (TRANSPARENT) 場合は上書きしない
        let shadow_color =
            if instance.shadow_color != Color::TRANSPARENT && visual.shadow_params.is_some() {
                let mut color = visual.shadow_color.unwrap_or_default();
                // DCompブレンド時の濃さの補正
                let mut has_active_visual_parent = false;
                let mut curr_id = entity_id;
                while let Some(parent_id) = *cx.topology.topo_parents.at(curr_id) {
                    if cx
                        .topology
                        .topo_active_masks
                        .at(parent_id)
                        .has_external_visual_content()
                        && cx.renders.rnd_active_external_visual.contains(&parent_id)
                    {
                        has_active_visual_parent = true;
                        break;
                    }
                    curr_id = parent_id;
                }
                if has_active_visual_parent {
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
            #[expect(clippy::cast_precision_loss)]
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
                alpha_mode_y_flip_srgb_gamma: instance.alpha_mode_y_flip_srgb_gamma,
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

    #[inline]
    #[must_use]
    pub fn surface(&self) -> &wgpu::Surface<'_> {
        &self.surface
    }

    #[inline]
    #[must_use]
    pub fn device(&self) -> &wgpu::Device {
        &self.device
    }

    #[inline]
    #[must_use]
    pub fn queue(&self) -> &wgpu::Queue {
        &self.queue
    }

    #[inline]
    #[must_use]
    pub fn config(&self) -> &wgpu::SurfaceConfiguration {
        &self.config
    }

    #[inline]
    #[must_use]
    pub fn pipeline(&self) -> &wgpu::RenderPipeline {
        &self.pipeline
    }

    #[inline]
    #[must_use]
    pub fn punchout_pipeline(&self) -> &wgpu::RenderPipeline {
        &self.punchout_pipeline
    }

    #[inline]
    #[must_use]
    pub fn vertex_buffer(&self) -> &wgpu::Buffer {
        &self.vertex_buffer
    }

    #[inline]
    #[must_use]
    pub fn index_buffer(&self) -> &wgpu::Buffer {
        &self.index_buffer
    }

    #[inline]
    #[must_use]
    pub fn instance_buffer(&self) -> &wgpu::Buffer {
        &self.instance_buffer
    }

    #[inline]
    #[must_use]
    pub fn instance_buffer_capacity(&self) -> &usize {
        &self.instance_buffer_capacity
    }

    #[inline]
    #[must_use]
    pub fn config_buffer(&self) -> &wgpu::Buffer {
        &self.config_buffer
    }

    #[inline]
    #[must_use]
    pub fn config_bind_group(&self) -> &wgpu::BindGroup {
        &self.config_bind_group
    }

    #[inline]
    #[must_use]
    pub fn config_bind_group_layout(&self) -> &wgpu::BindGroupLayout {
        &self.config_bind_group_layout
    }

    #[inline]
    #[must_use]
    pub fn atlas(&self) -> &TextureAtlas {
        &self.atlas
    }

    #[inline]
    #[must_use]
    pub fn temp_uv_map(&self) -> &SecondaryMap<Entity, [f32; 4]> {
        &self.temp_uv_map
    }

    #[inline]
    #[must_use]
    pub fn text_cache(&self) -> &FxHashMap<TextCacheKey, TextCacheValue> {
        &self.text_cache
    }

    #[inline]
    #[must_use]
    pub fn render_data(&self) -> &RenderData {
        &self.render_data
    }

    #[inline]
    #[must_use]
    pub fn external_bind_groups(&self) -> &FxHashMap<Entity, (wgpu::TextureView, wgpu::BindGroup)> {
        &self.external_bind_groups
    }

    #[inline]
    pub fn raw_wgpu_renderer_mut(&mut self) -> RawWgpuRenderer<'_> {
        RawWgpuRenderer {
            surface: &mut self.surface,
            device: &mut self.device,
            queue: &mut self.queue,
            config: &mut self.config,
            pipeline: &mut self.pipeline,
            punchout_pipeline: &mut self.punchout_pipeline,
            vertex_buffer: &mut self.vertex_buffer,
            index_buffer: &mut self.index_buffer,
            instance_buffer: &mut self.instance_buffer,
            instance_buffer_capacity: &mut self.instance_buffer_capacity,
            instance_staging: &mut self.instance_staging,
            config_buffer: &mut self.config_buffer,
            config_bind_group: &mut self.config_bind_group,
            config_bind_group_layout: &mut self.config_bind_group_layout,
            atlas: &mut self.atlas,
            temp_uv_map: &mut self.temp_uv_map,
            text_cache: &mut self.text_cache,
            render_data: &mut self.render_data,
            external_bind_groups: &mut self.external_bind_groups,
        }
    }
}

pub struct RawWgpuRenderer<'a> {
    pub surface: &'a mut wgpu::Surface<'static>,
    pub device: &'a mut wgpu::Device,
    pub queue: &'a mut wgpu::Queue,
    pub config: &'a mut wgpu::SurfaceConfiguration,
    pub pipeline: &'a mut wgpu::RenderPipeline,
    pub punchout_pipeline: &'a mut wgpu::RenderPipeline,
    pub vertex_buffer: &'a mut wgpu::Buffer,
    pub index_buffer: &'a mut wgpu::Buffer,
    pub instance_buffer: &'a mut wgpu::Buffer,
    pub instance_buffer_capacity: &'a mut usize,
    pub instance_staging: &'a mut Vec<QuadInstance>,
    pub config_buffer: &'a mut wgpu::Buffer,
    pub config_bind_group: &'a mut wgpu::BindGroup,
    pub config_bind_group_layout: &'a mut wgpu::BindGroupLayout,
    pub atlas: &'a mut TextureAtlas,
    pub temp_uv_map: &'a mut SecondaryMap<Entity, [f32; 4]>,
    pub text_cache: &'a mut FxHashMap<TextCacheKey, TextCacheValue>,
    pub render_data: &'a mut RenderData,
    pub external_bind_groups: &'a mut FxHashMap<Entity, (wgpu::TextureView, wgpu::BindGroup)>,
}
