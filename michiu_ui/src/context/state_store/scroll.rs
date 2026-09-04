use std::{
    collections::HashSet,
    time::{Duration, Instant},
};

use crate::{
    ActiveInteractionStates, ActiveMasksSecondary, ActiveTransitionsSparseSecondary,
    BaseBasicLayoutsSecondary, BaseVisualPropertiesSecondary, BasicLayoutsSecondary,
    CapacityConfig, ChildrenSecondary, ClipRectsSecondary, ComponentMask, DirtyLayoutEntitiesVec,
    DirtyRenderEntitiesVec, Display, EntityId, FlatDfsSequenceVec, FlexLayoutsSecondary,
    GridLayoutsSparseSecondary, InputContentsSparseSecondary, InteractionPropertiesSecondary,
    LayoutPoint, LayoutSize, LayoutStore, Length, OutputStore, ParentsSecondary, Position, Rect,
    RectsSecondary, RenderStore, ResolvedBasicSecondary, ResolvedFlexSecondary,
    ResolvedGridSparseSecondary, ScrollBarState, ScrollbarStylesSecondary, Size, SystemStore,
    TaffyNodesSecondary, TaffyTreeEntityId, TextContentsSparseSecondary, TextEngine,
    TextLayoutEngine, TextLayoutEngineSparseSecondary, TextSpansSparseSecondary, ThisStyle,
    UserSelect, Val, VisualPropertiesSecondary, WindowStore,
};
use slotmap::{SecondaryMap, SparseSecondaryMap};
use smallvec::SmallVec;

pub(crate) type ScrollOffsetsSecondary = SecondaryMap<EntityId, LayoutPoint>;
pub(crate) type ScrollSizesSecondary = SecondaryMap<EntityId, LayoutSize>;

pub(crate) struct ScrollStore {
    pub(crate) sc_offsets: ScrollOffsetsSecondary,
    pub(crate) sc_sizes: ScrollSizesSecondary,
}

impl Default for ScrollStore {
    fn default() -> Self {
        Self::new()
    }
}

impl ScrollStore {
    #[inline]
    #[must_use]
    pub fn new() -> Self {
        Self {
            sc_offsets: SecondaryMap::new(),
            sc_sizes: SecondaryMap::new(),
        }
    }

    #[inline]
    #[must_use]
    pub fn with_capacity(c: &CapacityConfig) -> Self {
        Self {
            sc_offsets: SecondaryMap::with_capacity(c.sc_offsets),
            sc_sizes: SecondaryMap::with_capacity(c.sc_sizes),
        }
    }

    #[inline]
    pub fn clear(&mut self) {
        self.sc_offsets.clear();
        self.sc_sizes.clear();
    }

    #[inline]
    pub fn despawn(&mut self, id: EntityId) {
        self.sc_offsets.remove(id);
        self.sc_sizes.remove(id);
    }
}

impl ScrollStore {
    /// スクロールオフセットを目標位置へクランプした上で代入。
    /// オフセットに変化が生じた場合は true を返し、レイアウトのDirtyマークを打つ。
    pub(crate) fn scroll_to(
        id: EntityId,
        mut x: f32,
        mut y: f32,
        win_last_size: Option<LayoutSize>,
        topo_active_masks: &mut ActiveMasksSecondary,
        topo_parents: &ParentsSecondary,
        lay_dirty_entities: &mut DirtyLayoutEntitiesVec,
        lay_taffy_tree: &mut TaffyTreeEntityId,
        bar_styles: &mut ScrollbarStylesSecondary,
        lay_taffy_nodes: &TaffyNodesSecondary,
        lay_resolved_basic: &ResolvedBasicSecondary,
        rnd_visual: &VisualPropertiesSecondary,
        rnd_interaction: &InteractionPropertiesSecondary,
        rnd_active_transitions: &ActiveTransitionsSparseSecondary,
        sc_offsets: &mut ScrollOffsetsSecondary,
        out_rects: &RectsSecondary,
        sc_sizes: &ScrollSizesSecondary,
    ) -> bool {
        let Some(rect) = out_rects.get(id).copied() else {
            return false;
        };

        let scroll_size = sc_sizes.get(id).copied().unwrap_or_default();

        // 親コンテナのボーダーおよびパディング厚を取得
        let basic = lay_resolved_basic.get(id).copied().unwrap_or_default();
        let (border, padding) =
            LayoutStore::get_physical_border_padding(rect, basic.border, basic.padding);

        let visible_size = WindowStore::calc_visible_size(rect, win_last_size);
        let content_size = OutputStore::calc_inner_content_size(visible_size, border, padding);

        // コンテンツサイズと内枠表示領域サイズの差分として、正確な最大スクロール量を算出
        let max_scroll_x = (scroll_size.width - content_size.width).max(0.0);
        let max_scroll_y = (scroll_size.height - content_size.height).max(0.0);

        x = x.clamp(0.0, max_scroll_x);
        y = y.clamp(0.0, max_scroll_y);

        // スロットが存在しない場合はあらかじめ挿入して初期化
        if !sc_offsets.contains_key(id) {
            sc_offsets.insert(id, LayoutPoint::ZERO);
        }

        let current = sc_offsets.get_mut(id).unwrap();
        if (current.x - x).abs() > 0.01 || (current.y - y).abs() > 0.01 {
            current.x = x;
            current.y = y;

            // スクロールバー状態の最終スクロール時刻を更新
            if let Some(sb_state) = bar_styles.get_mut(id) {
                sb_state.last_scroll_time = Some(Instant::now());
            }

            // オフセット変化に伴い、子孫全体の絶対座標を再同期させる
            LayoutStore::mark_layout_dirty(
                id,
                topo_active_masks,
                topo_parents,
                lay_dirty_entities,
                lay_taffy_tree,
                lay_taffy_nodes,
            );
            true
        } else {
            false
        }
    }

    pub(crate) fn sync_scrollbar_drag(
        logical_pos: LayoutPoint,
        win_last_size: Option<LayoutSize>,
        topo_active_masks: &mut ActiveMasksSecondary,
        topo_parents: &ParentsSecondary,
        lay_dirty_entities: &mut DirtyLayoutEntitiesVec,
        lay_taffy_tree: &mut TaffyTreeEntityId,
        bar_styles: &mut ScrollbarStylesSecondary,
        lay_taffy_nodes: &TaffyNodesSecondary,
        lay_resolved_basic: &ResolvedBasicSecondary,
        rnd_dirty_entities: &mut DirtyRenderEntitiesVec,
        rnd_visual: &VisualPropertiesSecondary,
        rnd_interaction: &InteractionPropertiesSecondary,
        rnd_active_transitions: &ActiveTransitionsSparseSecondary,
        sc_offsets: &mut ScrollOffsetsSecondary,
        out_rects: &RectsSecondary,
        sc_sizes: &ScrollSizesSecondary,
    ) {
        #[derive(Clone, Copy, PartialEq, Eq)]
        enum DragDirection {
            Vertical,
            Horizontal,
        }

        #[derive(Clone, Copy, PartialEq)]
        struct SrcrollbarDate {
            track_id: EntityId,
            thumb_id: EntityId,
            track_len: f32,
            thumb_len: f32,
            margin_start: f32,
            margin_end: f32,
            delta_mouse: f32,
            max_scroll_len: f32,
            start_scroll_offset: f32,
        }

        let active_drag_target = bar_styles.iter().find_map(|(id, state)| {
            if state.v_thumb_dragged {
                Some((id, DragDirection::Vertical))
            } else if state.h_thumb_dragged {
                Some((id, DragDirection::Horizontal))
            } else {
                None
            }
        });

        let Some((current_id, direction)) = active_drag_target else {
            return;
        };

        let (sb_state, container_rect, scroll_size) = {
            let sb_state = bar_styles.get(current_id).cloned().unwrap_or_default();
            let container_rect = out_rects.get(current_id).copied().unwrap_or_default();
            let scroll_size = sc_sizes.get(current_id).copied().unwrap_or_default();
            (sb_state, container_rect, scroll_size)
        };

        let visible_size = WindowStore::calc_visible_size(container_rect, win_last_size);

        let date = match direction {
            DragDirection::Vertical => {
                let track_id = sb_state.v_track_id.unwrap();
                let thumb_id = sb_state.v_thumb_id.unwrap();
                let track_rect = out_rects.get(track_id).copied().unwrap_or_default();
                let thumb_rect = out_rects.get(thumb_id).copied().unwrap_or_default();

                let (mut margin_top, mut margin_bottom) = (0.0, 0.0);
                if let Some(ref thumb_style) = sb_state.style.v_thumb {
                    if let Val::Px(val) = thumb_style.inner.basic_layout.margin.top {
                        margin_top = val;
                    }
                    if let Val::Px(val) = thumb_style.inner.basic_layout.margin.bottom {
                        margin_bottom = val;
                    }
                }

                SrcrollbarDate {
                    track_id,
                    thumb_id,
                    track_len: track_rect.height,
                    thumb_len: thumb_rect.height,
                    margin_start: margin_top,
                    margin_end: margin_bottom,
                    delta_mouse: logical_pos.y - sb_state.drag_start_mouse.y,
                    max_scroll_len: scroll_size.height - visible_size.height,
                    start_scroll_offset: sb_state.drag_start_offset.y,
                }
            }
            DragDirection::Horizontal => {
                let track_id = sb_state.h_track_id.unwrap();
                let thumb_id = sb_state.h_thumb_id.unwrap();
                let track_rect = out_rects.get(track_id).copied().unwrap_or_default();
                let thumb_rect = out_rects.get(thumb_id).copied().unwrap_or_default();

                let (mut margin_left, mut margin_right) = (0.0, 0.0);
                if let Some(ref thumb_style) = sb_state.style.h_thumb {
                    if let Val::Px(val) = thumb_style.inner.basic_layout.margin.left {
                        margin_left = val;
                    }
                    if let Val::Px(val) = thumb_style.inner.basic_layout.margin.right {
                        margin_right = val;
                    }
                }

                SrcrollbarDate {
                    track_id,
                    thumb_id,
                    track_len: track_rect.width,
                    thumb_len: thumb_rect.width,
                    margin_start: margin_left,
                    margin_end: margin_right,
                    delta_mouse: logical_pos.x - sb_state.drag_start_mouse.x,
                    max_scroll_len: scroll_size.width - visible_size.width,
                    start_scroll_offset: sb_state.drag_start_offset.x,
                }
            }
        };

        // スクロール可動域と割合
        let track_range = date.track_len - date.thumb_len - date.margin_start - date.margin_end;
        if track_range > 0.0 {
            let ratio = date.max_scroll_len / track_range;
            let target_scroll = date.start_scroll_offset + date.delta_mouse * ratio;

            let (target_x, target_y) = match direction {
                DragDirection::Vertical => {
                    let current_x = sc_offsets.get(current_id).map_or(0.0, |o| o.x);
                    (current_x, target_scroll)
                }
                DragDirection::Horizontal => {
                    let current_y = sc_offsets.get(current_id).map_or(0.0, |o| o.y);
                    (target_scroll, current_y)
                }
            };

            ScrollStore::scroll_to(
                current_id,
                target_x,
                target_y,
                win_last_size,
                topo_active_masks,
                topo_parents,
                lay_dirty_entities,
                lay_taffy_tree,
                bar_styles,
                lay_taffy_nodes,
                lay_resolved_basic,
                rnd_visual,
                rnd_interaction,
                rnd_active_transitions,
                sc_offsets,
                out_rects,
                sc_sizes,
            );
        }

        RenderStore::mark_render_dirty(current_id, topo_active_masks, rnd_dirty_entities);
    }

    pub(crate) fn scroll_by(
        id: EntityId,
        dx: f32,
        dy: f32,
        win_last_size: Option<LayoutSize>,
        topo_active_masks: &mut ActiveMasksSecondary,
        topo_parents: &ParentsSecondary,
        lay_dirty_entities: &mut DirtyLayoutEntitiesVec,
        lay_taffy_tree: &mut TaffyTreeEntityId,
        bar_styles: &mut ScrollbarStylesSecondary,
        lay_taffy_nodes: &TaffyNodesSecondary,
        lay_resolved_basic: &ResolvedBasicSecondary,
        rnd_visual: &VisualPropertiesSecondary,
        rnd_interaction: &InteractionPropertiesSecondary,
        rnd_active_transitions: &ActiveTransitionsSparseSecondary,
        sc_offsets: &mut ScrollOffsetsSecondary,
        out_rects: &RectsSecondary,
        sc_sizes: &ScrollSizesSecondary,
    ) -> bool {
        let current = sc_offsets.get(id).copied().unwrap_or_default();
        ScrollStore::scroll_to(
            id,
            current.x + dx,
            current.y + dy,
            win_last_size,
            topo_active_masks,
            topo_parents,
            lay_dirty_entities,
            lay_taffy_tree,
            bar_styles,
            lay_taffy_nodes,
            lay_resolved_basic,
            rnd_visual,
            rnd_interaction,
            rnd_active_transitions,
            sc_offsets,
            out_rects,
            sc_sizes,
        )
    }

    pub(crate) fn autoscroll_occurred(
        id: EntityId,
        win_last_size: Option<LayoutSize>,
        evt_current_pointer_position: Option<LayoutPoint>,
        topo_active_masks: &mut ActiveMasksSecondary,
        topo_parents: &ParentsSecondary,
        lay_dirty_entities: &mut DirtyLayoutEntitiesVec,
        lay_taffy_tree: &mut TaffyTreeEntityId,
        bar_styles: &mut ScrollbarStylesSecondary,
        lay_taffy_nodes: &TaffyNodesSecondary,
        lay_resolved_basic: &ResolvedBasicSecondary,
        rnd_visual: &VisualPropertiesSecondary,
        rnd_interaction: &InteractionPropertiesSecondary,
        rnd_active_transitions: &ActiveTransitionsSparseSecondary,
        sc_offsets: &mut ScrollOffsetsSecondary,
        out_rects: &RectsSecondary,
        out_clip_rects: &ClipRectsSecondary,
        sc_sizes: &ScrollSizesSecondary,
    ) -> (bool, Option<LayoutPoint>) {
        // ポインタ位置、またはクリップ領域がない場合
        let Some(pointer_pos) = evt_current_pointer_position else {
            return (false, None);
        };
        let Some(clip) = out_clip_rects.get(id).copied() else {
            return (false, None);
        };

        // テキスト選択状態
        let user_select = rnd_visual
            .get(id)
            .and_then(|v| v.user_select)
            .unwrap_or_default();
        if user_select != UserSelect::Text {
            return (false, None);
        }

        // はみ出し距離
        let distance = OutputStore::drag_overhang_distance(pointer_pos, &clip);
        if distance.x.abs() <= 1.0 && distance.y.abs() <= 1.0 {
            return (false, None);
        }

        // オートスクロール実行
        let speed_factor = 0.15f32;
        let dx = distance.x * speed_factor;
        let dy = distance.y * speed_factor;

        let scroll = ScrollStore::scroll_by(
            id,
            dx,
            dy,
            win_last_size,
            topo_active_masks,
            topo_parents,
            lay_dirty_entities,
            lay_taffy_tree,
            bar_styles,
            lay_taffy_nodes,
            lay_resolved_basic,
            rnd_visual,
            rnd_interaction,
            rnd_active_transitions,
            sc_offsets,
            out_rects,
            sc_sizes,
        );

        if scroll {
            (true, Some(pointer_pos))
        } else {
            (false, None)
        }
    }

    /// 指定された要素の子要素全体のスクロール領域を親ローカル座標系で算出します。
    pub(crate) fn get_scroll_size(
        id: EntityId,
        sys_text_engine: &mut TextEngine,
        sys_dwrite_layouts: &TextLayoutEngineSparseSecondary,
        cont_text_contents: &TextContentsSparseSecondary,
        cont_text_spans: &TextSpansSparseSecondary,
        cont_input_contents: &InputContentsSparseSecondary,
        topo_active_masks: &ActiveMasksSecondary,
        topo_parents: &ParentsSecondary,
        topo_children: &ChildrenSecondary,
        lay_resolved_basic: &ResolvedBasicSecondary,
        lay_resolved_flex: &ResolvedFlexSecondary,
        bar_styles: &ScrollbarStylesSecondary,
        rnd_visual: &VisualPropertiesSecondary,
        rnd_interaction: &InteractionPropertiesSecondary,
        rnd_active_transitions: &ActiveTransitionsSparseSecondary,
        out_rects: &RectsSecondary,
        sc_offsets: &ScrollOffsetsSecondary,
        cosmic: bool,
    ) -> LayoutSize {
        let mut max_x = 0.0f32;
        let mut max_y = 0.0f32;

        // 自身に内包されたインラインコンテンツの計測サイズを初期値とする
        if topo_active_masks
            .get(id)
            .is_some_and(ComponentMask::has_input_content)
            && let Some(contents) = cont_input_contents.get(id)
            && let Some(layout_rect) = contents.last_layout
        {
            max_x =
                layout_rect.width + contents.caret_width.unwrap_or(contents.default_caret_width);
            max_y = layout_rect.height;
        } else if topo_active_masks
            .get(id)
            .is_some_and(ComponentMask::has_text_content)
            && let Some(engine) = SystemStore::get_or_create_layout(
                id,
                cosmic,
                sys_text_engine,
                sys_dwrite_layouts,
                cont_text_contents,
                cont_text_spans,
                lay_resolved_basic,
                lay_resolved_flex,
                rnd_visual,
                out_rects,
            )
        {
            let size = match engine {
                TextLayoutEngine::Cosmic(buffer) => sys_text_engine.get_layout_size_cosmic(&buffer),
                TextLayoutEngine::DWrite(dw_layout) => sys_text_engine.get_layout_size(&dw_layout),
            };
            max_x = size.width;
            max_y = size.height;
        }

        // 親要素自体のボーダー・パディング厚を取得
        let basic = lay_resolved_basic.get(id).copied().unwrap_or_default();
        let rect = out_rects.get(id).copied().unwrap_or_default();
        let (border, padding) =
            LayoutStore::get_physical_border_padding(rect, basic.border, basic.padding);

        let offset_x = border.left + padding.left;
        let offset_y = border.top + padding.top;

        // スクロールバー要素のIDを取得して除外対象にする
        let (v_track_opt, h_track_opt) = if let Some(sb_state) = bar_styles.get(id) {
            (sb_state.v_track_id, sb_state.h_track_id)
        } else {
            (None, None)
        };

        if let Some(children_list) = topo_children.get(id) {
            for &child_id in children_list {
                // スクロールバーのトラックはサイズ計算から除外
                if Some(child_id) == v_track_opt || Some(child_id) == h_track_opt {
                    continue;
                }

                // 絶対配置要素（スクロールバーのサムなど）もスクロール領域サイズ計算から除外
                let is_absolute = lay_resolved_basic
                    .get(child_id)
                    .is_some_and(|l| l.position == Position::Absolute);
                if is_absolute {
                    continue;
                }

                if let Some(&rect) = out_rects.get(child_id) {
                    let parent_rect = out_rects.get(id).copied().unwrap_or_default();
                    let scroll_offset = sc_offsets.get(id).copied().unwrap_or_default();

                    // 親の左上（border+padding除外）を原点 (0,0) とした子要素の右下端
                    let local_right =
                        rect.x - parent_rect.x + scroll_offset.x + rect.width - offset_x;
                    let local_bottom =
                        rect.y - parent_rect.y + scroll_offset.y + rect.height - offset_y;

                    max_x = max_x.max(local_right);
                    max_y = max_y.max(local_bottom);
                }
            }
        }

        LayoutSize::new(max_x, max_y)
    }
}
