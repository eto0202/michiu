use crate::{
    Context, CpuTextureView, Entity, ExternalTexture, ExternalTextureMetadata, LayoutPoint,
    LayoutRect, LayoutSize, MichiuSoA, PropertyList,
};
use std::sync::Arc;
use windows::Win32::Foundation::{HWND, LPARAM, POINT, WPARAM};
use windows::Win32::Graphics::DirectComposition::{
    IDCompositionDesktopDevice, IDCompositionVisual2,
};
use windows::Win32::Graphics::Imaging::IWICImagingFactory;
use windows_numerics::Matrix3x2;

/// An `ExternalTexture` implementation that holds a single static image in a `TextureView`
#[derive(Debug, Clone)]
pub struct StaticExternalTexture {
    pub view: wgpu::TextureView,
    pub metadata: ExternalTextureMetadata,
}

impl ExternalTexture for StaticExternalTexture {
    fn resolve_view(&self, _device: &wgpu::Device, _queue: &wgpu::Queue) -> wgpu::TextureView {
        self.view.clone()
    }

    fn resolve_cpu(&self, _f: &mut dyn FnMut(CpuTextureView<'_>)) {
        todo!()
    }

    fn metadata(&self) -> ExternalTextureMetadata {
        self.metadata
    }
}

// ====================================================================================
// ====================================================================================

/// A trait for providing `IDCompositionVisual2`
pub trait ExternalVisual: Send + Sync {
    /// Returns an `IDCompositionVisual2` to be placed in the `DirectComposition` tree.
    fn resolve_visual(&self) -> IDCompositionVisual2;

    /// If a static texture exists, it is returned as an `ExternalTexture`.
    /// When set to `Some`, it is rendered as a normal texture, and the `DComp` side is made invisible.
    /// When set to `None`, it is considered active, and `DComp` mounting and punchout are performed.
    fn static_texture(&self) -> Option<Arc<dyn ExternalTexture>> {
        None
    }

    /// Layout and State Update Hooks for Each Frame
    fn update(&self, _cx: &VisualUpdateContext) {}

    /// A generic hook that transparently forwards raw input from the OS.
    /// Returns `true` if the event is consumed, and `false` if it is passed through
    ///  (for normal processing by the library).
    fn handle_raw_input(
        &self,
        _msg: u32,
        _wparam: WPARAM,
        _lparam: LPARAM,
        _local_phys_pos: POINT,
    ) -> bool {
        false
    }

    /// Retrieves metadata.
    fn metadata(&self) -> ExternalVisualMetadata;
}

impl<T: ExternalVisual + ?Sized> ExternalVisual for Arc<T> {
    fn resolve_visual(&self) -> IDCompositionVisual2 {
        (**self).resolve_visual()
    }

    fn static_texture(&self) -> Option<Arc<dyn ExternalTexture>> {
        (**self).static_texture()
    }

    fn update(&self, cx: &VisualUpdateContext) {
        (**self).update(cx);
    }

    fn handle_raw_input(
        &self,
        msg: u32,
        wparam: WPARAM,
        lparam: LPARAM,
        local_phys_pos: POINT,
    ) -> bool {
        (**self).handle_raw_input(msg, wparam, lparam, local_phys_pos)
    }

    fn metadata(&self) -> ExternalVisualMetadata {
        (**self).metadata()
    }
}

/// The context passed to `ExternalVisual` at each frame update
pub struct VisualUpdateContext<'a> {
    pub hwnd: HWND,
    pub rect: LayoutRect,
    pub scale_factor: f32,
    pub is_interactive: bool,
    pub is_stable: bool,
    pub device: Option<&'a wgpu::Device>,
    pub queue: Option<&'a wgpu::Queue>,
    pub wic_factory: &'a IWICImagingFactory,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ExternalVisualMetadata {
    /// Recommended Initial Size
    pub size: LayoutSize,
    /// Whether to leave the rounded-corner clipping on the Direct Composition side to the engine
    pub auto_clip: bool,
    /// Whether to leave the affine transformation on the Direct Composition side to the engine
    pub auto_transform: bool,
}

impl Default for ExternalVisualMetadata {
    fn default() -> Self {
        Self {
            size: LayoutSize::ZERO,
            auto_clip: true,
            auto_transform: true,
        }
    }
}

#[track_caller]
pub fn sync_external_visual_layout_and_clip(
    cx: &mut Context,
    scale_factor: f32,
    e: Entity,
    ex_visual: &IDCompositionVisual2,
    metadata: ExternalVisualMetadata,
    dcomp: &IDCompositionDesktopDevice,
) {
    let rect = cx.outputs.out_rects.find_or_default(e, &mut cx.debug);
    let clip_rect = cx.outputs.out_clip_rects.find_or_default(e, &mut cx.debug);
    let prev_rect = cx.outputs.out_prev_rects.find_or_default(e, &mut cx.debug);
    let prev_clip = cx
        .outputs
        .out_prev_clip_rects
        .find_or_default(e, &mut cx.debug);

    let has_active_transform = cx
        .renders
        .rnd_active_transitions
        .find(e)
        .is_some_and(|list| {
            list.iter()
                .any(|t| t.property_list == PropertyList::Transform)
        });

    // 変化がなければ早期リターン
    // sync_layout で prev_rects が既にスワップされているケースを考慮し、
    // 初回またはリサイズ時は確実に同期を通す
    if !cx.window_is_resized()
        && rect == prev_rect
        && clip_rect == prev_clip
        && !has_active_transform
    {
        return;
    }

    // 位置同期 (物理ピクセル)
    let phys_x = rect.x * scale_factor;
    let phys_y = rect.y * scale_factor;
    unsafe {
        let _ = ex_visual.SetOffsetX2(phys_x);
        let _ = ex_visual.SetOffsetY2(phys_y);
    }

    let Some(visual_prop) = cx.renders.rnd_visual.find(e) else {
        return;
    };

    // 2D アフィン変換 (Transform2) の同期
    if metadata.auto_transform {
        if let Some(m) = visual_prop.transform {
            let m11 = m[0][0];
            let m12 = m[0][1];
            let m21 = m[1][0];
            let m22 = m[1][1];
            let tx = m[3][0] * scale_factor;
            let ty = m[3][1] * scale_factor;

            let origin = visual_prop
                .transform_origin
                .map_or([0.5, 0.5], |p| [p.x, p.y]);
            let origin_x = origin[0] * rect.width * scale_factor;
            let origin_y = origin[1] * rect.height * scale_factor;

            let m31 = (1.0 - m11) * origin_x - m21 * origin_y + tx;
            let m32 = -m12 * origin_x + (1.0 - m22) * origin_y + ty;

            let dcomp_matrix = Matrix3x2 {
                M11: m11,
                M12: m12,
                M21: m21,
                M22: m22,
                M31: m31,
                M32: m32,
            };
            let _ = unsafe { ex_visual.SetTransform2(&raw const dcomp_matrix) };
        } else {
            let identity = Matrix3x2 {
                M11: 1.0,
                M12: 0.0,
                M21: 0.0,
                M22: 1.0,
                M31: 0.0,
                M32: 0.0,
            };
            let _ = unsafe { ex_visual.SetTransform2(&raw const identity) };
        }
    }

    // 角丸・Scissor クリップ (RectangleClip) の同期
    if !metadata.auto_clip {
        return;
    }
    unsafe {
        let Ok(rectangle_clip) = dcomp.CreateRectangleClip() else {
            return;
        };
        let clip_left = (clip_rect.x - rect.x).max(0.0) * scale_factor;
        let clip_top = (clip_rect.y - rect.y).max(0.0) * scale_factor;
        let clip_right = ((clip_rect.x + clip_rect.width) - rect.x).min(rect.width) * scale_factor;
        let clip_bottom =
            ((clip_rect.y + clip_rect.height) - rect.y).min(rect.height) * scale_factor;

        let _ = rectangle_clip.SetLeft2(clip_left);
        let _ = rectangle_clip.SetTop2(clip_top);
        let _ = rectangle_clip.SetRight2(clip_right);
        let _ = rectangle_clip.SetBottom2(clip_bottom);

        if let Some(radius) = visual_prop.corner_radius {
            let r = radius.top_left * scale_factor;
            let _ = rectangle_clip.SetTopLeftRadiusX2(r);
            let _ = rectangle_clip.SetTopLeftRadiusY2(r);
            let _ = rectangle_clip.SetTopRightRadiusX2(r);
            let _ = rectangle_clip.SetTopRightRadiusY2(r);
            let _ = rectangle_clip.SetBottomLeftRadiusX2(r);
            let _ = rectangle_clip.SetBottomLeftRadiusY2(r);
            let _ = rectangle_clip.SetBottomRightRadiusX2(r);
            let _ = rectangle_clip.SetBottomRightRadiusY2(r);
        } else {
            let _ = rectangle_clip.SetTopLeftRadiusX2(0.0);
            let _ = rectangle_clip.SetTopLeftRadiusY2(0.0);
        }

        let _ = ex_visual.SetClip(&rectangle_clip);
    }
}

/// Identifies the topmost `ExternalVisual` based on physical pixel coordinates and forwards the raw input.
/// Returns `true` if the event has been consumed.
pub fn dispatch_raw_input_to_external_visual(
    cx: &mut Context,
    msg: u32,
    wparam: WPARAM,
    lparam: LPARAM,
    window_phys_pos: LayoutPoint,
    scale_factor: f32,
) -> bool {
    let logical_pos = LayoutPoint::new(
        window_phys_pos.x / scale_factor,
        window_phys_pos.y / scale_factor,
    );

    // プレス中要素があればそちらを優先、なければヒット要素
    let target_id = cx
        .events
        .evt_interaction_states
        .pressed
        .or_else(|| cx.hit_test(logical_pos));

    let Some(id) = target_id else {
        return false;
    };

    if !cx
        .topology
        .topo_active_masks
        .at(id)
        .has_external_visual_content()
    {
        return false;
    }

    let Some(visual) = cx.contents.cont_external_visual.find(id).cloned() else {
        return false;
    };

    let rect = cx.outputs.out_rects.find_or_default(id, &mut cx.debug);
    #[expect(clippy::cast_possible_truncation)]
    let local_phys_pos = POINT {
        x: (window_phys_pos.x - rect.x * scale_factor).round() as i32,
        y: (window_phys_pos.y - rect.y * scale_factor).round() as i32,
    };

    visual.handle_raw_input(msg, wparam, lparam, local_phys_pos)
}
