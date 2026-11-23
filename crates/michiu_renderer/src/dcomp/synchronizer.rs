use michiu_ui::{Context, Entity, ExternalVisualMetadata, LayoutPoint, LayoutSize};
use windows::Win32::{
    Foundation::{LPARAM, POINT, WPARAM},
    Graphics::DirectComposition::{IDCompositionDesktopDevice, IDCompositionVisual2},
};
use windows_numerics::Matrix3x2;

pub(crate) struct WindowSynchronizer {
    pub(crate) window_size: LayoutSize,
    pub(crate) scale_factor: f32,
    // リサイズが完全に安定するまでキャプチャを保留するカウンター
    pub(crate) resize_cooldown_frames: u32,
}

impl WindowSynchronizer {
    #[inline]
    pub fn new(window_size: LayoutSize, scale_factor: f32) -> Self {
        Self {
            window_size,
            scale_factor,
            resize_cooldown_frames: 0,
        }
    }

    #[expect(clippy::cast_precision_loss)]
    #[inline]
    pub fn resize(&mut self, p_width: u32, p_height: u32, scale_factor: f32) {
        self.scale_factor = scale_factor;
        self.window_size = LayoutSize::new(
            p_width as f32 / scale_factor,
            p_height as f32 / scale_factor,
        );
        // リサイズが発生したため、キャプチャクールダウンを 15 フレームに設定
        // 拡大リサイズ中およびリサイズ直後の不安定なバッファへのキャプチャを遮断
        self.resize_cooldown_frames = 15;
    }

    pub fn sync_external_visual_layout_and_clip(
        &self,
        cx: &mut Context,
        e: Entity,
        ex_visual: &IDCompositionVisual2,
        metadata: ExternalVisualMetadata,
        dcomp: &IDCompositionDesktopDevice,
    ) {
        let rect = cx.rect(e);
        let clip_rect = cx.clip_rect(e);
        let prev_rect = cx.prev_rect(e);
        let prev_clip = cx.prev_clip_rect(e);

        let has_active_transform = cx.has_active_transform(e);

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

        let scale_factor = self.scale_factor;

        // 位置同期 (物理ピクセル)
        let phys_x = rect.x * scale_factor;
        let phys_y = rect.y * scale_factor;
        unsafe {
            let _ = ex_visual.SetOffsetX2(phys_x);
            let _ = ex_visual.SetOffsetY2(phys_y);
        }

        let Some(visual_prop) = cx.try_visual_property(e) else {
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
            let clip_right =
                ((clip_rect.x + clip_rect.width) - rect.x).min(rect.width) * scale_factor;
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
        &self,
        cx: &mut Context,
        msg: u32,
        wparam: WPARAM,
        lparam: LPARAM,
        window_phys_pos: LayoutPoint,
    ) -> bool {
        let scale_factor = self.scale_factor;

        let logical_pos = LayoutPoint::new(
            window_phys_pos.x / scale_factor,
            window_phys_pos.y / scale_factor,
        );

        // プレス中要素があればそちらを優先、なければヒット要素
        let target = cx.pressed_entity().or_else(|| cx.hit_test(logical_pos));

        let Some(e) = target else {
            return false;
        };
        if !cx.has_external_visual(e) {
            return false;
        }
        let Some(visual) = cx.external_visual(e) else {
            return false;
        };

        let rect = cx.rect(e);

        #[expect(clippy::cast_possible_truncation)]
        let local_phys_pos = POINT {
            x: (window_phys_pos.x - rect.x * scale_factor).round() as i32,
            y: (window_phys_pos.y - rect.y * scale_factor).round() as i32,
        };

        visual.handle_raw_input(msg, wparam, lparam, local_phys_pos)
    }
}
