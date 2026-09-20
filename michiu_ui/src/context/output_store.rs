use crate::{
    ActiveInteractionStates, ActiveMasksSecondary, AlignItems, BasicLayoutsSecondary,
    CapacityConfig, DEFAULT_BASIC, DebugStore, EdgeInsets, EntityId, LayoutPoint, LayoutRect,
    LayoutSize, LayoutStore, MichiuSoA, ParentsSecondary, Position, ResolvedBasicSecondary,
    ScrollOffsetsSecondary, TaffyNodesSecondary, TaffyTreeEntityId, TextAlign, TextLayoutSize,
    UserSelect, Val, VisualPropertiesSecondary, define_secondary,
};
use slotmap::SecondaryMap;

define_secondary!(pub struct RectsSecondary(LayoutRect));
define_secondary!(pub struct ClipRectsSecondary(LayoutRect));
define_secondary!(pub struct PrevRectsSecondary(LayoutRect));
define_secondary!(pub struct PrevClipRectsSecondary(LayoutRect));

pub struct OutputStore {
    pub(crate) out_rects: RectsSecondary,
    pub(crate) out_clip_rects: ClipRectsSecondary,
    pub(crate) out_prev_rects: PrevRectsSecondary,
    pub(crate) out_prev_clip_rects: PrevClipRectsSecondary,
}

impl Default for OutputStore {
    fn default() -> Self {
        Self::new()
    }
}

impl OutputStore {
    #[inline]
    #[must_use]
    pub(crate) fn new() -> Self {
        Self {
            out_rects: RectsSecondary(SecondaryMap::new()),
            out_clip_rects: ClipRectsSecondary(SecondaryMap::new()),
            out_prev_rects: PrevRectsSecondary(SecondaryMap::new()),
            out_prev_clip_rects: PrevClipRectsSecondary(SecondaryMap::new()),
        }
    }

    #[inline]
    #[must_use]
    pub(crate) fn with_capacity(c: &CapacityConfig) -> Self {
        Self {
            out_rects: RectsSecondary(SecondaryMap::with_capacity(c.out_rects)),
            out_clip_rects: ClipRectsSecondary(SecondaryMap::with_capacity(c.out_clip_rects)),
            out_prev_rects: PrevRectsSecondary(SecondaryMap::with_capacity(c.out_prev_rects)),
            out_prev_clip_rects: PrevClipRectsSecondary(SecondaryMap::with_capacity(
                c.out_prev_clip_rects,
            )),
        }
    }

    #[inline]
    pub(crate) fn clear(&mut self) {
        self.out_rects.clear();
        self.out_clip_rects.clear();
        self.out_prev_rects.clear();
        self.out_prev_clip_rects.clear();
    }

    #[inline]
    pub(crate) fn despawn(&mut self, id: EntityId) {
        self.out_rects.remove(id);
        self.out_clip_rects.remove(id);
        self.out_prev_rects.remove(id);
        self.out_prev_clip_rects.remove(id);
    }
}

impl OutputStore {
    pub(crate) fn has_parent_changed(
        id: EntityId,
        topo_active_masks: &ActiveMasksSecondary,
        topo_parents: &ParentsSecondary,
        out_rects: &RectsSecondary,
        out_clip_rects: &ClipRectsSecondary,
        out_prev_rects: &PrevRectsSecondary,
        out_prev_clip_rects: &PrevClipRectsSecondary,
    ) -> bool {
        let Some(parent_id) = *topo_parents.at(id) else {
            return false;
        };

        out_prev_rects.find(parent_id) != out_rects.find(parent_id)
            || out_prev_clip_rects.find(parent_id) != out_clip_rects.find(parent_id)
            || topo_active_masks.at(parent_id).has_queued_layout()
    }

    pub(crate) fn calc_local_rect(
        id: EntityId,
        window_size: LayoutSize,
        topo_parents: &ParentsSecondary,
        lay_taffy_tree: &TaffyTreeEntityId,
        lay_taffy_nodes: &TaffyNodesSecondary,
        lay_basic: &BasicLayoutsSecondary,
        out_rects: &RectsSecondary,
        out_clip_rects: &ClipRectsSecondary,
        sc_offsets: &ScrollOffsetsSecondary,
        debug: &mut DebugStore,
    ) -> (LayoutRect, LayoutRect) {
        let initial_clip = LayoutRect::new(0.0, 0.0, window_size.width, window_size.height);
        let local_rect = LayoutStore::local_rect_from_taffy(id, lay_taffy_tree, lay_taffy_nodes);

        let Some(parent_id) = *topo_parents.at(id) else {
            return (local_rect, initial_clip);
        };

        let (Some(&parent_rect), Some(&parent_clip)) =
            (out_rects.find(parent_id), out_clip_rects.find(parent_id))
        else {
            return (local_rect, initial_clip);
        };

        let s_offsets = sc_offsets.find_or_default(parent_id, debug);
        // データが無い要素が Absolute になることは絶対にない
        let is_absolute = lay_basic
            .find(id)
            .is_some_and(|l| l.position == Position::Absolute);

        let parent_scroll = if is_absolute {
            LayoutPoint::ZERO
        } else {
            s_offsets
        };

        let abs_x = parent_rect.x + local_rect.x - parent_scroll.x;
        let abs_y = parent_rect.y + local_rect.y - parent_scroll.y;

        let abs_rect = LayoutRect::new(abs_x, abs_y, local_rect.width, local_rect.height);

        (abs_rect, parent_clip)
    }

    /// 単位（Px, Percent, Auto）を親要素のサイズまたはウィンドウ基準をベースに物理ピクセルへ解決します。
    pub(crate) fn val_to_px(
        id: EntityId,
        val: Val,
        is_width: bool,
        win_last_size: Option<LayoutSize>,
        topo_parents: &ParentsSecondary,
        out_rects: &RectsSecondary,
        debug: &mut DebugStore,
    ) -> f32 {
        match val {
            Val::Px(v) => v,
            Val::Percent(p) => {
                // 親要素の確定サイズを優先取得
                let parent_size = topo_parents
                    .at(id)
                    .and_then(|p_id| out_rects.find(p_id))
                    .map(|r| LayoutSize::new(r.width, r.height));

                // 親要素が未確定または存在しない場合は、最終ウィンドウ寸法を基準にする
                let ref_size = parent_size.or(win_last_size).unwrap_or_default();
                let ref_val = if is_width {
                    ref_size.width
                } else {
                    ref_size.height
                };

                ref_val * (p / 100.0)
            }
            Val::Auto => {
                // Auto の場合は前フレームで確定している Taffy のレイアウト結果を実数値の基準値とする
                let r = out_rects.find_or_default(id, debug);
                if is_width { r.width } else { r.height }
            }
        }
    }

    // 累積計算用の行列乗算
    #[inline]
    pub(crate) fn mul_4x4(a: &[[f32; 4]; 4], b: &[[f32; 4]; 4]) -> [[f32; 4]; 4] {
        let mut out = [[0.0; 4]; 4];
        for i in 0..4 {
            for j in 0..4 {
                out[i][j] =
                    a[i][0] * b[0][j] + a[i][1] * b[1][j] + a[i][2] * b[2][j] + a[i][3] * b[3][j];
            }
        }
        out
    }

    /// 矩形にアフィン変換行列を適用した後の座標軸に平行な AABB を求める
    #[inline]
    pub(crate) fn calc_aabb(rect: LayoutRect, matrix: &[[f32; 4]; 4]) -> LayoutRect {
        // 矩形の4頂点
        let p0 = OutputStore::mul_vector(0.0, 0.0, matrix);
        let p1 = OutputStore::mul_vector(rect.width, 0.0, matrix);
        let p2 = OutputStore::mul_vector(rect.width, rect.height, matrix);
        let p3 = OutputStore::mul_vector(0.0, rect.height, matrix);

        let min_x = p0.0.min(p1.0).min(p2.0).min(p3.0);
        let max_x = p0.0.max(p1.0).max(p2.0).max(p3.0);
        let min_y = p0.1.min(p1.1).min(p2.1).min(p3.1);
        let max_y = p0.1.max(p1.1).max(p2.1).max(p3.1);

        // 親要素の原点からの絶対座標にアライメント
        LayoutRect::new(rect.x + min_x, rect.y + min_y, max_x - min_x, max_y - min_y)
    }

    /// 2D頂点に行列を適用
    #[inline]
    fn mul_vector(x: f32, y: f32, m: &[[f32; 4]; 4]) -> (f32, f32) {
        let out_x = m[0][0] * x + m[1][0] * y + m[3][0];
        let out_y = m[0][1] * x + m[1][1] * y + m[3][1];
        (out_x, out_y)
    }

    #[inline]
    pub(crate) fn drag_overhang_distance(
        pointer_pos: LayoutPoint,
        clip: LayoutRect,
    ) -> LayoutPoint {
        let mut dx = 0.0f32;
        let mut dy = 0.0f32;

        // はみ出し距離
        if pointer_pos.x < clip.x {
            dx = pointer_pos.x - clip.x; // 左はみ出し：負値
        } else if pointer_pos.x > clip.x + clip.width {
            dx = pointer_pos.x - (clip.x + clip.width); // 右はみ出し：正値
        }

        if pointer_pos.y < clip.y {
            dy = pointer_pos.y - clip.y;
        } else if pointer_pos.y > clip.y + clip.height {
            dy = pointer_pos.y - (clip.y + clip.height);
        }

        LayoutPoint { x: dx, y: dy }
    }

    /// 現在テキスト選択ドラッグ中かつ、マウスポインタが要素の可視境界外にあるかを判定
    pub(crate) fn is_drag_autoscroll_active(
        evt_interaction_states: &ActiveInteractionStates,
        evt_current_pointer_position: Option<&LayoutPoint>,
        rnd_visual: &VisualPropertiesSecondary,
        out_clip_rects: &ClipRectsSecondary,
    ) -> bool {
        let Some(id) = evt_interaction_states.pressed else {
            return false;
        };

        let Some(pointer_pos) = evt_current_pointer_position else {
            return false;
        };

        let clip = out_clip_rects.at(id);

        let user_select = rnd_visual.user_select(id);

        if user_select != UserSelect::Text {
            return false;
        }

        // ポインタが可視クリップ範囲の上下左右からはみ出しているか検証
        let is_out_x = pointer_pos.x < clip.x || pointer_pos.x > clip.x + clip.width;
        let is_out_y = pointer_pos.y < clip.y || pointer_pos.y > clip.y + clip.height;
        is_out_x || is_out_y
    }

    #[inline]
    pub fn clip_rects_mut(&mut self) -> &mut ClipRectsSecondary {
        &mut self.out_clip_rects
    }

    #[inline]
    pub fn prev_clip_rects_mut(&mut self) -> &mut PrevClipRectsSecondary {
        &mut self.out_prev_clip_rects
    }

    #[inline]
    pub fn prev_rects_mut(&mut self) -> &mut PrevRectsSecondary {
        &mut self.out_prev_rects
    }

    #[inline]
    pub fn rects_mut(&mut self) -> &mut RectsSecondary {
        &mut self.out_rects
    }
}

pub(crate) struct ResolvedGeometry {
    pub(crate) rect: LayoutRect,
    pub(crate) border: EdgeInsets,
    pub(crate) padding: EdgeInsets,
    pub(crate) scroll: LayoutPoint,
}

impl ResolvedGeometry {
    #[inline]
    pub(crate) fn resolved(
        id: EntityId,
        sc_offsets: &ScrollOffsetsSecondary,
        lay_resolved_basic: &ResolvedBasicSecondary,
        out_rects: &RectsSecondary,
        debug: &mut DebugStore,
    ) -> Self {
        let basic = lay_resolved_basic.find_or(id, &DEFAULT_BASIC, debug);
        let rect = out_rects.find_or_default(id, debug);
        let scroll = sc_offsets.find_or_default(id, debug);
        let (border, padding) =
            LayoutStore::get_physical_border_padding(rect, basic.border, basic.padding);
        Self {
            rect,
            border,
            padding,
            scroll,
        }
    }

    #[inline]
    pub(crate) fn content_width(&self) -> f32 {
        self.rect.width
            - self.border.right
            - self.border.left
            - self.padding.right
            - self.padding.left
    }

    #[inline]
    pub(crate) fn viewport_size(&self) -> LayoutSize {
        let rect = self.rect;
        let border = self.border;
        let padding = self.padding;
        let content_w =
            (rect.width - border.left - border.right - padding.left - padding.right).max(0.0);
        let content_h =
            (rect.height - border.top - border.bottom - padding.top - padding.bottom).max(0.0);

        LayoutSize::new(content_w, content_h)
    }

    /// 実際の可視サイズから、物理ボーダーとパディングの厚みを引いた内枠の有効表示可能サイズを算出。
    #[inline]
    pub(crate) fn inner_content_size(&self, visible_size: LayoutSize) -> LayoutSize {
        let border = self.border;
        let padding = self.padding;

        let content_w =
            (visible_size.width - border.left - border.right - padding.left - padding.right)
                .max(0.0);
        let content_h =
            (visible_size.height - border.top - border.bottom - padding.top - padding.bottom)
                .max(0.0);

        LayoutSize::new(content_w, content_h)
    }

    pub(crate) fn pressed_local_point(
        &self,
        logical_pos: LayoutPoint,
        text_align: TextAlign,
        align_items: Option<AlignItems>,
        text_size: TextLayoutSize,
    ) -> LayoutPoint {
        let align_offset = self.calc_align_offset(text_size, text_align, align_items);
        let rect = self.rect;
        let border = self.border;
        let padding = self.padding;
        let scroll = self.scroll;

        let local_x =
            logical_pos.x - (rect.x + border.left + padding.left + align_offset.x) + scroll.x;
        let local_y =
            logical_pos.y - (rect.y + border.top + padding.top + align_offset.y) + scroll.y;

        LayoutPoint {
            x: local_x,
            y: local_y,
        }
    }

    #[inline]
    pub(crate) fn calc_max_width(&self, auto_wrap: bool) -> Option<f32> {
        let rect = self.rect;
        let border = self.border;
        let padding = self.padding;
        let content_width = rect.width - border.right - border.left - padding.right - padding.left;

        (auto_wrap && content_width > 0.0).then_some(content_width)
    }

    #[inline]
    pub(crate) fn calc_align_offset(
        &self,
        text_size: TextLayoutSize,
        text_align: TextAlign,
        align_items: Option<AlignItems>,
    ) -> LayoutPoint {
        let rect = self.rect;
        let border = self.border;
        let padding = self.padding;

        let content_w =
            (rect.width - border.left - border.right - padding.left - padding.right).max(0.0);
        let align_offset_x = match text_align {
            TextAlign::Center => ((content_w - text_size.width) * 0.5).max(0.0),
            TextAlign::Right => (content_w - text_size.width).max(0.0),
            _ => 0.0,
        };

        let content_h =
            (rect.height - border.top - border.bottom - padding.top - padding.bottom).max(0.0);

        // 複数行入力時は標準で上端揃え、単一行は標準で中央揃えにフォールバック
        let resolved_multiline = if let Some(is_multiline) = text_size.is_multiline {
            if is_multiline {
                AlignItems::Start
            } else {
                AlignItems::Center
            }
        } else {
            AlignItems::Center
        };
        let align_items_resolved = align_items.unwrap_or(resolved_multiline);

        let align_offset_y = match align_items_resolved {
            AlignItems::Start
            | AlignItems::FlexStart
            | AlignItems::SafeStart
            | AlignItems::SafeFlexStart => 0.0,
            AlignItems::End
            | AlignItems::FlexEnd
            | AlignItems::SafeEnd
            | AlignItems::SafeFlexEnd => (content_h - text_size.height).max(0.0),
            _ => ((content_h - text_size.height) * 0.5).max(0.0), // Center 等
        };

        LayoutPoint {
            x: align_offset_x,
            y: align_offset_y,
        }
    }
}

#[cfg(test)]
mod tests;
