//! # `MichiuRenderer`
//!
//! The top-level orchestrator integrating Windows DirectComposition with wgpu (DirectX 12).
//! It coordinates the foreground UI layer ([`WgpuRenderer`]) with the background composition layer (DirectComposition visual tree) hosting zero-copy [`ExternalVisual`](crate::ExternalVisual).
//!
//! ---
//!
//! ## Visual Tree Hierarchy
//!
//! `MichiuRenderer` constructs and maintains the following tree hierarchy over the target window (`HWND`):
//!
//! ```text
//! IDCompositionTarget (Window association)
//!   └─ root_visual
//!        ├─ cont_external_visual (Underlay: ExternalVisual collection)
//!        └─ wgpu_visual          (Overlay: wgpu UI surface + punch-outs)
//! ```
//!
//! ## Execution Lifecycle (`update_composition_tree` → `draw`)
//!
//! Frame progression within `MichiuRenderer` operates across two discrete phases:
//! **DirectComposition tree synchronization (state evaluation and cutout dispatch)** followed by **wgpu execution and commit**.
//!
//! ### Phase 1: `update_composition_tree`
//!
//! Evaluated immediately following layout finalization (`sync_layout`), synchronizing `ExternalVisual` states with the DComp tree.
//!
//! 1. **Backdrop Synchronization**:
//!    Applies system-level materials (Mica, Acrylic) to the target window.
//!
//! 2. **Delayed Release Queue Processing**:
//!    Advances `pending_dcomp_releases`, safely unlinking Visuals whose replacement wgpu pixels have completed presentation (`RemoveVisual`).
//!
//! 3. **Stability Pre-Evaluation**:
//!    Inspects each `ExternalVisual` to determine stability (e.g., active resizing, transition cooldowns, or animation state). The evaluated flag governs freeze-capture eligibility.
//!
//! 4. **External Visual State Hook ([`ExternalVisual::update`](crate::ExternalVisual::update))**:
//!    Supplies stability flags and device handles via [`VisualUpdateContext`](crate::VisualUpdateContext) to invoke entity update callbacks:
//!
//!    ```rust,ignore
//!    pub struct VisualUpdateContext<'a> {
//!        pub hwnd: HWND,
//!        pub rect: LayoutRect,
//!        pub scale_factor: f32,
//!        pub is_interactive: bool,
//!        pub is_stable: bool,
//!        pub device: &'a wgpu::Device,
//!        pub queue: &'a wgpu::Queue,
//!        pub wic_factory: &'a IWICImagingFactory,
//!    }
//!    ```
//!    *`ExternalVisual` instances use this context to autonomously capture static fallbacks during stable frames or decode assets via WIC.*
//!
//! 5. **Transform and Clip Synchronization (`sync_visual_layout_and_clip`)**:
//!    Propagates resolved screen coordinates, scaling metrics, and viewport clipping bounds to the underlying DComp Visuals.
//!
//! 6. **Static Capture vs. Live Visual Branching**:
//!    Determines rendering mode based on the return value of [`static_texture`](crate::ExternalVisual::static_texture):
//!    * **Static Texture Available (`Some`)**: Unmounts the live Visual from the DComp tree, falling back to lightweight texture rendering inside `WgpuRenderer`.
//!    * **Live Native Visual (`None`)**: Mounts the Visual directly into the DComp underlay and flags the wgpu render pass to punch out the corresponding bounds (`cx.renders.rnd_active_external_visual.insert(id)`).
//!
//! 7. **Dormant Entity Cleanup**:
//!    Detaches (`RemoveVisual`) and frees despawned or inactive `ExternalVisual` nodes from the composition tree.
//!
//! ---
//!
//! ### Phase 2: `draw`
//!
//! Finalizes pixel output, presents composition changes, and flushes diagnostics.
//!
//! ```rust,ignore
//! pub(crate) fn draw(&mut self, cx: &mut Context) {
//!     // 1. Render foreground UI via wgpu
//!     self.wgpu_renderer.render(cx, self.scale_factor);
//!
//!     // 2. Commit atomic DirectComposition state to DWM
//!     self.platform.commit().unwrap_or_trace(None, &mut cx.debug);
//!
//!     // 3. Clear dirty flags across the Context upon successful GPU submission
//!     cx.clear_dirty();
//!
//!     // 4. Frame terminus: Flush profiling traces
//!     flush_trace!(cx, self);
//! }
//! ```
//!
//! 1. **`wgpu_renderer.render`**:
//!    Executes punch-out cutouts and composites UI primitives onto the surface via instanced rendering.
//! 2. **`Commit()`**:
//!    Atomically presents the foreground wgpu surface alongside background `ExternalVisual` trees to the Desktop Window Manager (DWM).
//! 3. **`cx.clear_dirty()`**:
//!    Resets dirty tracking across the `Context`, acknowledging that all pending mutations have been submitted to the GPU.
//! 4. **`flush_trace!`**:
//!    Flushes telemetry and performance traces to external inspectors ([`MichiuInspector`](crate::MichiuInspector)).
//!
//! ---
//!
//! ## Key Fields
//!
//! | Field | Type | Responsibility |
//! | :--- | :--- | :--- |
//! | **`hwnd`** | `HWND` | Target Win32 window handle |
//! | **`sync`** | `WindowSynchronizer` | Window state synchronization (`window_size`, `scale_factor`, `resize_cooldown_frames`) |
//! | **`platform`** | `PlatformContext` | Immutable graphics runtime and WinRT foundations: <br>• **`d3d11` / `dcomp`**: Core D3D11 & DirectComposition devices <br>• **`compositor` / `controller`**: WinRT Compositor and DispatcherQueue lifecycle <br>• **`wic`**: WIC imaging factory for texture loading |
//! | **`workspace`** | `DCompWorkspace` | Visual tree and lifecycle management: <br>• **`target` / `root_visual`**: Root composition target and tree container <br>• **`wgpu_visual`**: Visual hosting the wgpu swapchain surface <br>• **`promoted_visuals`**: Active external visuals attached to the tree <br>• **`pending_releases`**: Deferred destruction queue preventing frame flickers during updates |
//! | **`wgpu_renderer`** | [`WgpuRenderer`] | Subsystem driving instanced UI rendering and punch-out operations |
//! | **`current_backdrop`** | `Backdrop` | Active system material configuration |
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
    ActiveBackend, CpuRenderer, CurrentBackend, MichiuRendererResult, RawWgpuRenderer,
    RendererBackend, RendererUserConfig, WgpuRenderer,
    dcomp::{
        DCompWorkspace, PendingDcompRelease, PlatformContext, PromotedVisual, WindowSynchronizer,
    },
};
use michiu_ui::{Backdrop, Context, ExternalVisual, LayoutPoint, LayoutSize, VisualUpdateContext};
use windows::{
    System::DispatcherQueueController,
    UI::Composition::Compositor,
    Win32::{
        Foundation::{HWND, LPARAM, WPARAM},
        Graphics::{
            Direct3D11::ID3D11Device,
            DirectComposition::{
                IDCompositionDesktopDevice, IDCompositionDevice, IDCompositionTarget,
                IDCompositionVisual2,
            },
            Imaging::IWICImagingFactory,
        },
    },
    core::Interface,
};

pub struct MichiuRenderer {
    pub(crate) hwnd: HWND,
    pub(crate) sync: WindowSynchronizer,
    pub(crate) platform: PlatformContext,
    pub(crate) workspace: DCompWorkspace,
    pub(crate) active_backend: ActiveBackend,
    pub(crate) current_backend: CurrentBackend,
    // 現在ウィンドウに適用中のバックドロップ状態
    pub(crate) current_backdrop: Backdrop,
}

impl MichiuRenderer {
    /// Initialize the renderer.
    ///
    /// # Errors
    ///
    /// Returns an error if graphics initialization fails:
    /// - **DirectComposition / D3D11 failure**: Failed to create the D3D11 device, or compositor resources (targets, visuals) could not be configured (e.g., unsupported GPU or invalid HWND).
    /// - **WIC initialization failure**: Failed to instantiate the WIC imaging factory (e.g., COM is not initialized on this thread).
    /// - **wgpu initialization failure**: Failed to request a suitable GPU adapter or create the logical device.
    #[inline]
    pub async fn new(
        hwnd: HWND,
        window_size: LayoutSize,
        scale_factor: f32,
    ) -> MichiuRendererResult<Self> {
        Self::with_config(
            hwnd,
            window_size,
            scale_factor,
            &RendererUserConfig::default(),
        )
        .await
    }

    /// Initialize the renderer.
    ///
    /// # Errors
    ///
    /// Returns an error if graphics initialization fails:
    /// - **DirectComposition / D3D11 failure**: Failed to create the D3D11 device, or compositor resources (targets, visuals) could not be configured (e.g., unsupported GPU or invalid HWND).
    /// - **WIC initialization failure**: Failed to instantiate the WIC imaging factory (e.g., COM is not initialized on this thread).
    /// - **wgpu initialization failure**: Failed to request a suitable GPU adapter or create the logical device.
    #[expect(clippy::print_stderr)]
    pub async fn with_config(
        hwnd: HWND,
        window_size: LayoutSize,
        scale_factor: f32,
        user_config: &RendererUserConfig,
    ) -> MichiuRendererResult<Self> {
        let sync = WindowSynchronizer::new(window_size, scale_factor);
        let platform = PlatformContext::new(user_config.backend)?;
        let workspace = DCompWorkspace::new(hwnd, &platform.dcomp, user_config.target_layer)?;

        let active_backend = match user_config.backend {
            RendererBackend::Dx12 => {
                let gpu = unsafe {
                    WgpuRenderer::with_config(
                        workspace.wgpu_visual.as_raw(),
                        window_size,
                        scale_factor,
                        user_config,
                    )
                }
                .await?;
                ActiveBackend::Gpu(Box::new(gpu))
            }
            RendererBackend::Cpu => {
                let cpu = CpuRenderer::with_config(
                    &workspace.wgpu_visual,
                    window_size,
                    scale_factor,
                    user_config,
                )?;
                ActiveBackend::Cpu(Box::new(cpu))
            }
            RendererBackend::Auto => {
                let result = unsafe {
                    WgpuRenderer::with_config(
                        workspace.wgpu_visual.as_raw(),
                        window_size,
                        scale_factor,
                        user_config,
                    )
                }
                .await;

                match result {
                    Ok(gpu) => ActiveBackend::Gpu(Box::new(gpu)),
                    Err(e) => {
                        #[cfg(debug_assertions)]
                        eprintln!(
                            "GPU initialization failed ({e:?}), falling back to CPU renderer."
                        );

                        let cpu = CpuRenderer::with_config(
                            &workspace.wgpu_visual,
                            window_size,
                            scale_factor,
                            user_config,
                        )?;
                        ActiveBackend::Cpu(Box::new(cpu))
                    }
                }
            }
        };

        let current_backend = active_backend.kind();

        platform.commit()?;

        Ok(Self {
            hwnd,
            sync,
            current_backdrop: Backdrop::None,
            platform: platform.clone(),
            workspace,
            active_backend,
            current_backend,
        })
    }

    /// Updates the overall settings and resizes wgpu when the window size changes.
    ///
    /// `new_physical_size`: (width, height)
    #[inline]
    pub fn resize(
        &mut self,
        p_width: u32,
        p_height: u32,
        scale_factor: f32,
    ) -> MichiuRendererResult<()> {
        self.sync.resize(p_width, p_height, scale_factor);
        self.active_backend
            .resize(p_width, p_height, scale_factor)?;
        self.platform.commit()?;
        Ok(())
    }

    /// Drawing Triggers
    #[track_caller]
    #[inline]
    pub fn draw(&mut self, cx: &mut Context) -> MichiuRendererResult<()> {
        self.active_backend.render(cx, self.sync.scale_factor)?;

        self.platform.commit()?;

        // ダーティフラグをクリア
        cx.clear_dirty();
        // ログを送信
        cx.flush_trace();

        Ok(())
    }

    #[track_caller]
    pub fn update_composition_tree(&mut self, cx: &mut Context) {
        if self.sync.resize_cooldown_frames > 0 {
            self.sync.resize_cooldown_frames -= 1;
        }

        // スタイル変更があった場合のみ、拡張した apply_system_backdrop を即時更新
        let target_backdrop = cx.target_backdrop();
        if self.current_backdrop != target_backdrop {
            apply_system_backdrop(self.hwnd, target_backdrop);
            self.current_backdrop = target_backdrop;
        }

        // 遅延キューの消化処理（1〜3フレーム目）
        self.workspace.processing_delay_queue();

        // 外部 Visual の一括更新 ＆ アクティブ判定
        let mut current_promoted_ids = Vec::new();
        // ループの前にスナップショットを取得
        let visual_entries = cx.external_visual_entries();

        for (e, visual_entry) in visual_entries {
            let rect = cx.rect(e);
            let ex_visual = visual_entry.resolve_visual();
            let metadata = visual_entry.metadata();

            // 安定状態の判定
            let is_interactive = cx.is_entity_interactive(e);
            let is_transitioning = cx.is_entity_transitioning(e);
            let prev_rect = cx.prev_rect(e);
            let is_size_changing = (rect.width - prev_rect.width).abs() > 0.01
                || (rect.height - prev_rect.height).abs() > 0.01;
            let is_stable = !cx.window_is_resized()
                && self.sync.resize_cooldown_frames == 0
                && !is_transitioning
                && !is_size_changing;

            // 各 Visual の状態更新フックを実行
            visual_entry.update(&VisualUpdateContext {
                hwnd: self.hwnd,
                rect,
                scale_factor: self.sync.scale_factor,
                is_interactive,
                is_stable,
                device: self.active_backend.gpu().map(|f| &f.device),
                queue: self.active_backend.gpu().map(|f| &f.queue),
                wic_factory: &self.platform.wic,
            });

            // DComp Visual のレイアウト（位置・クリップ・トランスフォーム）を同期
            self.sync.sync_external_visual_layout_and_clip(
                cx,
                e,
                &ex_visual,
                metadata,
                &self.platform.dcomp,
            );

            // 静止画テクスチャがあるかどうかで分岐
            if let Some(static_tex) = visual_entry.static_texture() {
                self.workspace.processing_static_texture(cx, e, static_tex);
            } else {
                // 実体表示
                current_promoted_ids.push(e);
                self.workspace.processing_punchout_visual(cx, e, ex_visual);
            }
        }

        // アクティブから消えた（要素がデスポーンした）Visual をクリーンアップ
        self.workspace.cleanup_visuals(cx, &current_promoted_ids);
    }

    /// Identifies the topmost `ExternalVisual` based on physical pixel coordinates
    /// and forwards the raw input.
    ///
    ///  Returns true if the event has been consumed.
    #[inline]
    pub fn dispatch_raw_input_to_external_visual(
        &self,
        cx: &mut Context,
        msg: u32,
        wparam: WPARAM,
        lparam: LPARAM,
        window_phys_pos: LayoutPoint,
    ) -> bool {
        self.sync
            .dispatch_raw_input_to_external_visual(cx, msg, wparam, lparam, window_phys_pos)
    }

    #[inline]
    #[must_use]
    pub fn hwnd(&self) -> HWND {
        self.hwnd
    }

    #[inline]
    #[must_use]
    pub fn window_size(&self) -> LayoutSize {
        self.sync.window_size
    }

    #[inline]
    #[must_use]
    pub fn scale_factor(&self) -> f32 {
        self.sync.scale_factor
    }

    #[inline]
    #[must_use]
    pub fn wic_factory(&self) -> &IWICImagingFactory {
        &self.platform.wic
    }

    #[inline]
    #[must_use]
    pub fn d3d11_device(&self) -> &ID3D11Device {
        &self.platform.d3d11
    }

    #[inline]
    #[must_use]
    pub fn desktop_device(&self) -> &IDCompositionDesktopDevice {
        &self.platform.dcomp
    }

    #[inline]
    #[must_use]
    pub fn dispatcher_queue_controller(&self) -> &DispatcherQueueController {
        &self.platform.controller
    }

    #[inline]
    #[must_use]
    pub fn compositor(&self) -> &Compositor {
        &self.platform.compositor
    }

    /// # Errors
    ///
    /// Failed to obtain the DirectComposition device.
    #[inline]
    pub fn composition_device(&self) -> windows_core::Result<IDCompositionDevice> {
        let result: IDCompositionDevice = self.platform.dcomp.clone().cast()?;
        Ok(result)
    }

    #[inline]
    #[must_use]
    pub fn target(&self) -> &IDCompositionTarget {
        &self.workspace.target
    }

    #[inline]
    #[must_use]
    pub fn root_visual(&self) -> &IDCompositionVisual2 {
        &self.workspace.root_visual
    }

    #[inline]
    #[must_use]
    pub fn wgpu_visual(&self) -> &IDCompositionVisual2 {
        &self.workspace.wgpu_visual
    }

    #[inline]
    #[must_use]
    pub fn promoted_visuals(&self) -> &[PromotedVisual] {
        &self.workspace.promoted_visuals
    }

    #[inline]
    #[must_use]
    pub fn pending_dcomp_releases(&self) -> &[PendingDcompRelease] {
        &self.workspace.pending_releases
    }

    #[inline]
    #[must_use]
    pub fn current_backdrop(&self) -> &Backdrop {
        &self.current_backdrop
    }

    #[inline]
    #[must_use]
    pub fn resize_cooldown_frames(&self) -> u32 {
        self.sync.resize_cooldown_frames
    }

    #[inline]
    #[must_use]
    pub fn active_backend(&self) -> &ActiveBackend {
        &self.active_backend
    }

    #[inline]
    #[must_use]
    pub fn current_backend(&self) -> &CurrentBackend {
        &self.current_backend
    }

    #[inline]
    #[must_use]
    pub fn raw_wgpu_renderer_mut(&mut self) -> Option<RawWgpuRenderer<'_>> {
        self.active_backend
            .gpu_mut()
            .map(|f| f.raw_wgpu_renderer_mut())
    }

    #[inline]
    #[must_use]
    pub fn raw_michiu_renderer_mut(&mut self) -> RawMichiuRenderer<'_> {
        RawMichiuRenderer {
            hwnd: &mut self.hwnd,
            window_size: &mut self.sync.window_size,
            scale_factor: &mut self.sync.scale_factor,
            wic_factory: &mut self.platform.wic,
            d3d11_device: &mut self.platform.d3d11,
            desktop_device: &mut self.platform.dcomp,
            controller: &mut self.platform.controller,
            compositor: &mut self.platform.compositor,
            target: &mut self.workspace.target,
            root_visual: &mut self.workspace.root_visual,
            wgpu_visual: &mut self.workspace.wgpu_visual,
            promoted_visuals: &mut self.workspace.promoted_visuals,
            pending_dcomp_releases: &mut self.workspace.pending_releases,
            current_backdrop: &mut self.current_backdrop,
            resize_cooldown_frames: &mut self.sync.resize_cooldown_frames,
        }
    }
}

#[allow(clippy::print_stderr, unused)]
impl Drop for MichiuRenderer {
    fn drop(&mut self) {
        if let Err(e) = self.platform.controller.ShutdownQueueAsync() {
            #[cfg(debug_assertions)]
            eprintln!("Failed to request termination of the DispatcherQueue: {e:?}");
        }
    }
}

/// # Notes
/// - `MichiuRenderer` does not manage the WinRT Composition visual tree.
/// - The exposed `Compositor` and `DispatcherQueueController` are intended for user extension.
/// - It is the user’s responsibility to create a Composition Target for an `HWND`.
/// - michiu manages ownership and shutdown processing (`ShutdownQueueAsync`) for the `DispatcherQueueController`.
pub struct RawMichiuRenderer<'a> {
    pub hwnd: &'a mut HWND,
    pub window_size: &'a mut LayoutSize,
    pub scale_factor: &'a mut f32,
    pub wic_factory: &'a mut IWICImagingFactory,
    pub d3d11_device: &'a mut ID3D11Device,
    pub desktop_device: &'a mut IDCompositionDesktopDevice,
    pub controller: &'a mut DispatcherQueueController,
    pub compositor: &'a mut Compositor,
    pub target: &'a mut IDCompositionTarget,
    pub root_visual: &'a mut IDCompositionVisual2,
    pub wgpu_visual: &'a mut IDCompositionVisual2,
    pub promoted_visuals: &'a mut Vec<PromotedVisual>,
    pub pending_dcomp_releases: &'a mut Vec<PendingDcompRelease>,
    pub current_backdrop: &'a mut Backdrop,
    pub resize_cooldown_frames: &'a mut u32,
}

// フレーム拡張用構造体
#[repr(C)]
#[derive(Copy, Clone, Debug)]
pub struct Margins {
    pub left: i32,
    pub right: i32,
    pub top: i32,
    pub bottom: i32,
}

#[link(name = "dwmapi")]
unsafe extern "system" {
    fn DwmSetWindowAttribute(
        hwnd: HWND,
        dwattribute: u32,
        pvattribute: *const std::ffi::c_void,
        cbattribute: u32,
    ) -> windows::core::HRESULT;

    fn DwmExtendFrameIntoClientArea(
        hwnd: HWND,
        pMarInset: *const Margins,
    ) -> windows::core::HRESULT;
}

// apply_system_backdrop を拡張してダークモードとフレーム拡張を統合
#[expect(clippy::cast_possible_truncation)]
pub(crate) fn apply_system_backdrop(hwnd: HWND, backdrop: Backdrop) {
    unsafe {
        // DWMWA_USE_HOSTBACKDROPBRUSH (17) を TRUE に設定
        // DComp（NOREDIRECTIONBITMAP）ウィンドウでアクリル・Micaを透かすため
        let enable_host_backdrop: i32 = 1; // 1 = TRUE
        let _ = DwmSetWindowAttribute(
            hwnd,
            17, // DWMWA_USE_HOSTBACKDROPBRUSH
            (&raw const enable_host_backdrop).cast(),
            std::mem::size_of::<i32>() as u32,
        );
        // DWMWA_USE_IMMERSIVE_DARK_MODE (20) を true に設定（ダークアクリル下地を強制）
        let dark_mode: i32 = 1; // 1 = true
        let _ = DwmSetWindowAttribute(
            hwnd,
            20, // DWMWA_USE_IMMERSIVE_DARK_MODE
            (&raw const dark_mode).cast(),
            std::mem::size_of::<i32>() as u32,
        );

        // DWMWA_USE_HOSTBACKDROPBRUSH (36) を true に設定
        // WS_EX_NOREDIRECTIONBITMAP ウィンドウの背後に
        // DWM が自動でアクリル用のぼかし背景ブラシ（BackdropBrush）を合成
        let use_host_backdrop: i32 = 1; // 1 = TRUE
        let _ = DwmSetWindowAttribute(
            hwnd,
            36, // DWMWA_USE_HOSTBACKDROPBRUSH
            (&raw const use_host_backdrop).cast(),
            std::mem::size_of::<i32>() as u32,
        );

        // DWMWA_SYSTEMBACKDROP_TYPE (38) の設定（アクリル/Micaの適用）
        let backdrop_val = backdrop as i32;
        let _ = DwmSetWindowAttribute(
            hwnd,
            38, // DWMWA_SYSTEMBACKDROP_TYPE
            (&raw const backdrop_val).cast(),
            std::mem::size_of::<i32>() as u32,
        );

        // クライアント領域全体にアクリル・Micaを拡張 (DwmExtendFrameIntoClientArea)
        if backdrop == Backdrop::None {
            // 通常時はフレーム拡張をクリア (0)
            let margins = Margins {
                left: 0,
                right: 0,
                top: 0,
                bottom: 0,
            };
            let _ = DwmExtendFrameIntoClientArea(hwnd, &raw const margins);
        } else {
            // margins に -1 を指定することで、ウィンドウ全体にアクリルを浸透
            let margins = Margins {
                left: -1,
                right: -1,
                top: -1,
                bottom: -1,
            };
            let _ = DwmExtendFrameIntoClientArea(hwnd, &raw const margins);
        }
    }
}
