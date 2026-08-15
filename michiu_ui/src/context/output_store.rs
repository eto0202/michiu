use std::{cell::RefCell, collections::HashSet, ops::Range, time::Instant};

use crate::{
    ActiveEntitiesVec, ActiveMasksSecondary, ActiveTransitionsSparseSecondary,
    ActiveWebviewsHashSet, BaseVisualPropertiesSecondary, BasicLayoutsSecondary, BatchType,
    BoxSizing, ChildrenSecondary, Color, ComponentMask, ContentStore, Context, CornerRadius,
    DfsIndicesSecondary, DirtyLayoutEntitiesVec, DirtyRenderEntitiesVec, DrawBatch,
    DwriteLayoutsSparseSecondary, EdgeInsets, EffectiveTransformsSecondary,
    EffectiveZindicesSecondary, EntityId, EventStore, FlatDfsSequenceVec, FlexLayoutsSecondary,
    GridLayoutsSecondary, InputContents, InputContentsSparseSecondary,
    InteractionPropertiesSecondary, InteractionStates, LayoutPoint, LayoutRect, LayoutSize,
    LayoutStore, ParentsSecondary, PointerEvents, Position, PropertyList, QuadInstance,
    ReactiveStore, RenderData, RenderStore, STATE_QUEUED_LAYOUT, STYLE_OVERFLOW, STYLE_TEXT_SPANS,
    ScrollbarStylesSecondary, SortedEntitiesVec, SystemStore, TaffyNodesSecondary,
    TaffyTreeEntityId, TextAlign, TextContentsSparseSecondary, TextEngine,
    TextSpansSparseSecondary, TopoSortCacheVec, TopologyStore, UserSelect, Val,
    VisualPropertiesSecondary, VisualProperty, WindowStore, bind_context, with_context,
};
use slotmap::{SecondaryMap, SparseSecondaryMap};
use windows::Win32::Graphics::DirectWrite::{DWRITE_HIT_TEST_METRICS, IDWriteTextLayout};

pub(crate) type RectsSecondary = SecondaryMap<EntityId, LayoutRect>;
pub(crate) type ClipRectsSecondary = SecondaryMap<EntityId, LayoutRect>;
pub(crate) type ScrollOffsetsSecondary = SecondaryMap<EntityId, LayoutPoint>;
pub(crate) type ScrollSizesSecondary = SecondaryMap<EntityId, LayoutSize>;
pub(crate) type PrevRectsSecondary = SecondaryMap<EntityId, LayoutRect>;
pub(crate) type PrevClipRectsSecondary = SecondaryMap<EntityId, LayoutRect>;
pub(crate) type SelectedRectsSparseSecondary = SparseSecondaryMap<EntityId, Vec<LayoutRect>>;
pub(crate) type TextSelectionsSparseSecondary = SparseSecondaryMap<EntityId, Range<usize>>;
pub(crate) type SelectionStartIndexSparseSecondary = SparseSecondaryMap<EntityId, usize>;

pub struct OutputStore {
    pub(crate) out_rects: RectsSecondary,
    pub(crate) out_clip_rects: ClipRectsSecondary,
    pub(crate) out_scroll_offsets: ScrollOffsetsSecondary,
    pub(crate) out_scroll_sizes: ScrollSizesSecondary,
    pub(crate) out_prev_rects: PrevRectsSecondary,
    pub(crate) out_prev_clip_rects: PrevClipRectsSecondary,
    pub(crate) out_selected_rects: SelectedRectsSparseSecondary,
    pub(crate) out_text_selections: TextSelectionsSparseSecondary,
    pub(crate) out_selection_start_index: SelectionStartIndexSparseSecondary,
}

impl Default for OutputStore {
    fn default() -> Self {
        Self::new()
    }
}

impl OutputStore {
    #[inline]
    #[must_use]
    pub fn new() -> Self {
        Self {
            out_rects: SecondaryMap::new(),
            out_clip_rects: SecondaryMap::new(),
            out_scroll_offsets: SecondaryMap::new(),
            out_scroll_sizes: SecondaryMap::new(),
            out_prev_rects: SecondaryMap::new(),
            out_prev_clip_rects: SecondaryMap::new(),
            out_selected_rects: SparseSecondaryMap::new(),
            out_text_selections: SparseSecondaryMap::new(),
            out_selection_start_index: SparseSecondaryMap::new(),
        }
    }

    #[inline]
    pub fn clear(&mut self) {
        self.out_rects.clear();
        self.out_clip_rects.clear();
        self.out_scroll_offsets.clear();
        self.out_scroll_sizes.clear();
        self.out_prev_rects.clear();
        self.out_prev_clip_rects.clear();
        self.out_selected_rects.clear();
        self.out_text_selections.clear();
        self.out_selection_start_index.clear();
    }

    #[inline]
    pub fn despawn(&mut self, id: EntityId) {
        self.out_rects.remove(id);
        self.out_clip_rects.remove(id);
        self.out_scroll_offsets.remove(id);
        self.out_scroll_sizes.remove(id);
        self.out_prev_rects.remove(id);
        self.out_prev_clip_rects.remove(id);
        self.out_selected_rects.remove(id);
        self.out_text_selections.remove(id);
        self.out_selection_start_index.remove(id);
    }
}

impl OutputStore {
    pub(crate) fn swap_output_rect(
        out_rects: &mut RectsSecondary,
        out_prev_rects: &mut PrevRectsSecondary,
        out_clip_rects: &mut ClipRectsSecondary,
        out_prev_clip_rects: &mut PrevClipRectsSecondary,
    ) {
        std::mem::swap(out_rects, out_prev_rects);
        std::mem::swap(out_clip_rects, out_prev_clip_rects);

        out_rects.clear();
        out_clip_rects.clear();
    }

    pub(crate) fn has_parent_changed(
        id: EntityId,
        topo_active_masks: &ActiveMasksSecondary,
        topo_parents: &ParentsSecondary,
        out_rects: &RectsSecondary,
        out_prev_rects: &PrevRectsSecondary,
        out_clip_rects: &ClipRectsSecondary,
        out_prev_clip_rects: &PrevClipRectsSecondary,
    ) -> bool {
        let Some(parent_id) = topo_parents.get(id).copied().flatten() else {
            return false;
        };

        out_prev_rects.get(parent_id) != out_rects.get(parent_id)
            || out_prev_clip_rects.get(parent_id) != out_clip_rects.get(parent_id)
            || topo_active_masks
                .get(parent_id)
                .is_some_and(|a| a.has(STATE_QUEUED_LAYOUT))
    }

    pub(crate) fn calc_local_rect(
        id: EntityId,
        window_size: LayoutSize,
        topo_parents: &ParentsSecondary,
        lay_taffy: &TaffyTreeEntityId,
        lay_taffy_nodes: &TaffyNodesSecondary,
        lay_basic: &BasicLayoutsSecondary,
        out_rects: &RectsSecondary,
        out_clip_rects: &ClipRectsSecondary,
        out_scroll_offsets: &ScrollOffsetsSecondary,
    ) -> (LayoutRect, LayoutRect) {
        let initial_clip = LayoutRect::new(0.0, 0.0, window_size.width, window_size.height);
        let local_rect = LayoutStore::local_rect_from_taffy(id, lay_taffy, lay_taffy_nodes);

        let parent_info = topo_parents.get(id).copied().flatten().and_then(|p_id| {
            let rect = out_rects.get(p_id).copied()?;
            let clip = out_clip_rects.get(p_id).copied()?;
            Some((p_id, rect, clip))
        });

        let Some((parent_id, parent_rect, parent_clip)) = parent_info else {
            return (
                LayoutRect::new(
                    local_rect.x,
                    local_rect.y,
                    local_rect.width,
                    local_rect.height,
                ),
                initial_clip,
            );
        };

        let s_offsets = out_scroll_offsets
            .get(parent_id)
            .copied()
            .unwrap_or_default();
        let is_absolute = lay_basic
            .get(id)
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
        win_last_size: Option<&LayoutSize>,
        topo_parents: &ParentsSecondary,
        out_rects: &RectsSecondary,
    ) -> Option<f32> {
        match val {
            Val::Px(v) => Some(v),
            Val::Percent(p) => {
                // 親要素の確定サイズを優先取得
                let parent_size = topo_parents
                    .get(id)
                    .copied()
                    .flatten()
                    .and_then(|p_id| out_rects.get(p_id))
                    .map(|r| LayoutSize::new(r.width, r.height));

                // 親要素が未確定または存在しない場合は、最終ウィンドウ寸法を基準にする
                let ref_size = parent_size.or(win_last_size.copied())?;
                let ref_val = if is_width {
                    ref_size.width
                } else {
                    ref_size.height
                };

                Some(ref_val * (p / 100.0))
            }
            Val::Auto => {
                // Auto の場合は前フレームで確定している Taffy のレイアウト結果を実数値の基準値とする
                let r = out_rects.get(id)?;
                Some(if is_width { r.width } else { r.height })
            }
        }
    }

    /// 指定した要素の画面上の絶対座標（LayoutRect）を取得します。
    #[inline]
    pub(crate) fn rect(id: EntityId, out_rects: &RectsSecondary) -> Option<LayoutRect> {
        out_rects.get(id).copied()
    }

    /// 指定した要素の画面上のクリップ境界（LayoutRect）を取得します。
    #[inline]
    #[must_use]
    pub fn clip_rect(id: EntityId, out_clip_rects: &ClipRectsSecondary) -> Option<LayoutRect> {
        out_clip_rects.get(id).copied()
    }

    pub(crate) fn calculate_caret_rect(
        rect: LayoutRect,
        border: EdgeInsets,
        padding: EdgeInsets,
        contents: &InputContents,
        scale: f32,
        scroll: LayoutPoint,
        align_offset: LayoutPoint,
    ) -> LayoutRect {
        let logical_x =
            rect.x + border.left + padding.left + align_offset.x + contents.measured_caret_x
                - scroll.x;
        let aligned_x = (logical_x * scale).round() / scale;

        let line_height = contents.caret_line_height;
        let caret_width = contents.caret_width.unwrap_or(contents.default_caret_width);
        let caret_height = contents.caret_height.unwrap_or(line_height);

        let vertical_center_offset = if contents.caret_height.is_some() {
            (line_height - caret_height) * 0.5
        } else {
            0.0
        };

        let logical_y = rect.y
            + border.top
            + padding.top
            + align_offset.y
            + contents.measured_caret_y
            + contents.caret_offset
            - scroll.y;

        let aligned_y = ((logical_y + vertical_center_offset) * scale).round() / scale;
        let aligned_width = (caret_width * scale).round().max(1.0) / scale;
        let aligned_height = (caret_height * scale).round().max(1.0) / scale;

        LayoutRect::new(aligned_x, aligned_y, aligned_width, aligned_height)
    }

    /// 現在テキスト選択ドラッグ中かつ、マウスポインタが要素の可視境界外にあるかを判定
    pub(crate) fn is_drag_autoscroll_active(
        evt_interaction_states: &InteractionStates,
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

        let Some(clip) = out_clip_rects.get(id) else {
            return false;
        };

        let user_select = rnd_visual
            .get(id)
            .and_then(|v| v.user_select)
            .unwrap_or_default();

        if user_select != UserSelect::Text {
            return false;
        }

        // ポインタが可視クリップ範囲の上下左右からはみ出しているか検証
        let is_out_x = pointer_pos.x < clip.x || pointer_pos.x > clip.x + clip.width;
        let is_out_y = pointer_pos.y < clip.y || pointer_pos.y > clip.y + clip.height;
        is_out_x || is_out_y
    }

    /// `現在の選択範囲（out_text_selections）に基づき`、
    /// `描画用の物理選択矩形（out_selected_rects）を自動再計算して` `SoA` キャッシュを更新します。
    pub(crate) fn calc_selection_rects(
        id: EntityId,
        layout: &IDWriteTextLayout,
        range: Range<usize>,
    ) -> Vec<LayoutRect> {
        let mut hit_test_metrics = vec![DWRITE_HIT_TEST_METRICS::default(); 16];
        let mut actual_count: u32 = 0;
        let res = unsafe {
            layout.HitTestTextRange(
                range.start.try_into().expect("Value fits in u32"),
                (range.end - range.start)
                    .try_into()
                    .expect("Value fits in u32"),
                0.0,
                0.0,
                Some(&mut hit_test_metrics),
                &raw mut actual_count,
            )
        };

        if res.is_ok() && actual_count as usize > hit_test_metrics.len() {
            hit_test_metrics.resize(actual_count as usize, DWRITE_HIT_TEST_METRICS::default());
            let _ = unsafe {
                layout.HitTestTextRange(
                    range.start.try_into().expect("Value fits in u32"),
                    (range.end - range.start)
                        .try_into()
                        .expect("Value fits in u32"),
                    0.0,
                    0.0,
                    Some(&mut hit_test_metrics),
                    &raw mut actual_count,
                )
            };
        }

        let mut out_rects = Vec::with_capacity(actual_count as usize);
        (0..actual_count as usize).for_each(|m_idx| {
            let metric = &hit_test_metrics[m_idx];
            out_rects.push(LayoutRect::new(
                metric.left,
                metric.top,
                metric.width,
                metric.height,
            ));
        });

        out_rects
    }

    /// 現在フォーカスされている要素で範囲選択されている文字列を取得します。
    pub(crate) fn get_selected_text(
        evt_interaction_states: &InteractionStates,
        cont_text_contents: &TextContentsSparseSecondary,
        rnd_visual: &VisualPropertiesSecondary,
        out_text_selections: &TextSelectionsSparseSecondary,
    ) -> Option<String> {
        let focused_id = evt_interaction_states.focused?;
        let user_select = rnd_visual
            .get(focused_id)
            .and_then(|v| v.user_select)
            .unwrap_or_default();

        if user_select == UserSelect::Text {
            let range = out_text_selections.get(focused_id)?;
            if range.start < range.end {
                let text = cont_text_contents.get(focused_id)?;
                let u16_text: Vec<u16> = text.encode_utf16().collect();
                let slice =
                    &u16_text[range.start.min(u16_text.len())..range.end.min(u16_text.len())];
                return String::from_utf16(slice).ok();
            }
        }
        None
    }

    pub(crate) fn handle_paste(
        focused_id: EntityId,
        text: &str,
        contents: &mut InputContents,
        out_text_selections: &mut TextSelectionsSparseSecondary,
        out_selected_rects: &mut SelectedRectsSparseSecondary,
    ) {
        let text_val = contents.text.0.get();
        let range = contents.selected_range.clone();

        let u16_text: Vec<u16> = text_val.encode_utf16().collect();
        let mut left = u16_text[..range.start.min(u16_text.len())].to_vec();
        let right = u16_text[range.end.min(u16_text.len())..].to_vec();

        let mut pasted_u16: Vec<u16> = text.encode_utf16().collect();

        // ペーストテキストに対する数値制限フィルターの適用
        if contents.numeric_only {
            pasted_u16.retain(|&ch_u16| {
                if let Ok(ch_char) = String::from_utf16(&[ch_u16])
                    && let Some(c) = ch_char.chars().next()
                {
                    return c.is_numeric() || c == '.' || c == '-';
                }

                false
            });
        }

        // ペーストテキストに対する文字数制限の適用（制限限界位置で自動カット）
        if let Some(max) = contents.max_length {
            let current_after_range_deleted =
                u16_text.len() - (range.end.min(u16_text.len()) - range.start.min(u16_text.len()));
            if current_after_range_deleted >= max {
                return; // すでに限界文字数に達しているため無視
            }
            let allowed_len = max - current_after_range_deleted;
            if pasted_u16.len() > allowed_len {
                pasted_u16.truncate(allowed_len); // 限界位置で足し合わせをカット
            }
        }

        left.extend_from_slice(&pasted_u16);
        left.extend_from_slice(&right);

        let new_text = String::from_utf16_lossy(&left);
        let new_caret = range.start + pasted_u16.len();

        // 変更履歴（Undo）をセーブ
        contents.record_undo(text_val.clone(), range.clone());

        contents.selected_range = new_caret..new_caret;
        out_text_selections.insert(focused_id, new_caret..new_caret);
        out_selected_rects.remove(focused_id);
        contents.text.1.set(new_text);
    }

    pub(crate) fn handle_undo(
        focused_id: EntityId,
        prev_sel: Range<usize>,
        prev_text: String,
        contents: &mut InputContents,
        out_text_selections: &mut TextSelectionsSparseSecondary,
        out_selected_rects: &mut SelectedRectsSparseSecondary,
    ) {
        let current_text = contents.text.0.get();
        let current_sel = contents.selected_range.clone();
        contents.redo_stack.push((current_text, current_sel)); // 現在の状態を Redo 用にセーブ

        contents.selected_range = prev_sel.clone();
        out_text_selections.insert(focused_id, prev_sel);
        out_selected_rects.remove(focused_id);
        contents.text.1.set(prev_text);
    }

    pub(crate) fn handle_redo(
        focused_id: EntityId,
        next_sel: Range<usize>,
        next_text: String,
        contents: &mut InputContents,
        out_text_selections: &mut TextSelectionsSparseSecondary,
        out_selected_rects: &mut SelectedRectsSparseSecondary,
    ) {
        let current_text = contents.text.0.get();
        let current_sel = contents.selected_range.clone();
        contents.undo_stack.push((current_text, current_sel)); // 現在の状態を Undo 用に退避

        contents.selected_range = next_sel.clone();
        out_text_selections.insert(focused_id, next_sel);
        out_selected_rects.remove(focused_id);
        contents.text.1.set(next_text);
    }

    pub(crate) fn inject_cut_internal(
        focused_id: EntityId,
        range: Range<usize>,
        contents: &mut InputContents,
        out_text_selections: &mut TextSelectionsSparseSecondary,
        out_selected_rects: &mut SelectedRectsSparseSecondary,
    ) {
        // 削除前の履歴セーブ
        let current_text = contents.text.0.get();
        let current_range = contents.selected_range.clone();
        contents.record_undo(current_text, current_range);

        let u16_input: Vec<u16> = contents.text.0.get().encode_utf16().collect();
        let mut left = u16_input[..range.start.min(u16_input.len())].to_vec();
        let right = u16_input[range.end.min(u16_input.len())..].to_vec();
        left.extend_from_slice(&right);

        let new_text = String::from_utf16_lossy(&left);
        contents.selected_range = range.start..range.start;
        out_text_selections.insert(focused_id, range.start..range.start);
        out_selected_rects.remove(focused_id);
        contents.text.1.set(new_text);
    }

    #[inline]
    pub(crate) fn truncate_unconfirmed_text(
        text_val: &str,
        contents: &InputContents,
        max: usize,
        filtered_comp_text: String,
    ) -> String {
        let text_u16: Vec<u16> = text_val.encode_utf16().collect();
        let range = &contents.selected_range;
        let range_start = range.start.min(text_u16.len());
        let range_end = range.end.min(text_u16.len());
        let deleted_len = range_end - range_start;
        let current_len_after_delete = text_u16.len() - deleted_len;

        if current_len_after_delete >= max {
            // すでに確定文字数が制限に達している場合は未確定文字を一切受け入れない
            String::new()
        } else {
            let allowed_comp_len = max - current_len_after_delete;
            let comp_u16: Vec<u16> = filtered_comp_text.encode_utf16().collect();
            if comp_u16.len() > allowed_comp_len {
                // サロゲートペア文字の途中でぶつ切りになるのを防ぐ
                let mut limit = allowed_comp_len;
                if limit > 0 && (0xD800..=0xDBFF).contains(&comp_u16[limit - 1]) {
                    limit -= 1;
                }

                // 許容文字数に収まるようUTF-16単位で正確に切り詰め
                String::from_utf16_lossy(&comp_u16[..allowed_comp_len])
            } else {
                filtered_comp_text
            }
        }
    }

    /// 指定された要素の子要素全体のスクロール領域を親ローカル座標系で算出します。
    pub(crate) fn get_scroll_size(
        id: EntityId,
        sys_text_engine: &TextEngine,
        sys_dwrite_layouts: &DwriteLayoutsSparseSecondary,
        cont_input_contents: &InputContentsSparseSecondary,
        cont_text_contents: &TextContentsSparseSecondary,
        cont_text_spans: &TextSpansSparseSecondary,
        topo_active_masks: &ActiveMasksSecondary,
        topo_parents: &ParentsSecondary,
        topo_children: &ChildrenSecondary,
        lay_basic: &BasicLayoutsSecondary,
        lay_flex: &FlexLayoutsSecondary,
        lay_grid: &GridLayoutsSecondary,
        lay_scrollbar_styles: &ScrollbarStylesSecondary,
        rnd_visual: &VisualPropertiesSecondary,
        rnd_interaction: &InteractionPropertiesSecondary,
        rnd_active_transitions: &ActiveTransitionsSparseSecondary,
        out_rects: &RectsSecondary,
        out_scroll_offsets: &ScrollOffsetsSecondary,
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
            && let Some(dw_layout) = SystemStore::get_or_create_layout(
                id,
                sys_text_engine,
                sys_dwrite_layouts,
                cont_text_contents,
                cont_text_spans,
                rnd_visual,
            )
        {
            let size = sys_text_engine.get_layout_size(&dw_layout);
            max_x = size.width;
            max_y = size.height;
        }

        // 親要素自体のボーダー・パディング厚を取得
        let (basic, _, _) = LayoutStore::resolve_active_layouts(
            id,
            topo_active_masks,
            topo_parents,
            lay_basic,
            lay_flex,
            lay_grid,
            rnd_interaction,
            rnd_visual,
            rnd_active_transitions,
        );

        let rect = OutputStore::rect(id, out_rects).unwrap_or_default();
        let (border, padding) =
            LayoutStore::get_physical_border_padding(rect, basic.border, basic.padding);

        let offset_x = border.left + padding.left;
        let offset_y = border.top + padding.top;

        // スクロールバー要素のIDを取得して除外対象にする
        let (v_track_opt, h_track_opt) = if let Some(sb_state) = lay_scrollbar_styles.get(id) {
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
                let is_absolute = lay_basic
                    .get(child_id)
                    .is_some_and(|l| l.position == Position::Absolute);
                if is_absolute {
                    continue;
                }

                if let Some(&rect) = out_rects.get(child_id) {
                    let parent_rect = OutputStore::rect(id, out_rects).unwrap_or_default();
                    let scroll_offset = out_scroll_offsets.get(id).copied().unwrap_or_default();

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

    #[inline]
    pub(crate) fn scroll_ime_info(
        id: EntityId,
        sys_text_engine: &TextEngine,
        cont_input_contents: &mut InputContentsSparseSecondary,
        cont_text_contents: &mut TextContentsSparseSecondary,
        cont_text_spans: &TextSpansSparseSecondary,
        rnd_visual: &mut VisualPropertiesSecondary,
        rnd_base_visual: &BaseVisualPropertiesSecondary,
        out_text_selections: &mut TextSelectionsSparseSecondary,
    ) -> Option<(LayoutRect, f32, bool)> {
        let contents = cont_input_contents.get_mut(id)?;
        // 入力エンジン側の最新カーソル位置を描画SoA側に同期
        out_text_selections.insert(id, contents.selected_range.clone());

        let text_val = contents.text.0.get();
        contents.total_len = text_val.chars().count();

        // IME未確定文字列が入力されている際、numeric_only が有効であれば数値を事前にフィルタリング
        // is_password が有効であればマスク処理を適用した中間文字列を生成
        let mut filtered_comp_text = if let Some(ref ime) = contents.ime_state
            && !ime.composition_text.is_empty()
        {
            if contents.numeric_only {
                let mut s = String::new();
                for c in ime.composition_text.chars() {
                    if c.is_numeric() || c == '.' || c == '-' {
                        s.push(c);
                    }
                }
                s
            } else {
                ime.composition_text.clone()
            }
        } else {
            String::new()
        };

        // 文字数制限（max_length）による未確定文字列の事前切り詰め
        if let Some(max) = contents.max_length
            && !filtered_comp_text.is_empty()
        {
            filtered_comp_text = OutputStore::truncate_unconfirmed_text(
                &text_val,
                contents,
                max,
                filtered_comp_text,
            );
        }

        if contents.is_password && !filtered_comp_text.is_empty() {
            let mask = contents.mask_text.as_deref().unwrap_or("●");
            filtered_comp_text = mask.repeat(filtered_comp_text.chars().count());
        }

        // is_password が true の場合、未確定中であっても
        // すでに確定されている文字列部分が一時的に生テキストとして露出してしまわないよう
        // マスクを維持した一時文字列を生成してベースとして使用
        let text_val_for_display = if contents.is_password {
            let mask = contents.mask_text.as_deref().unwrap_or("●");
            mask.repeat(text_val.chars().count())
        } else {
            text_val.clone()
        };

        // 描画表示用テキスト（IME未確定文字列の有無を最優先で判定）
        let display_text = if !filtered_comp_text.is_empty() {
            crate::input_get_display_text(
                &text_val_for_display,
                contents.selected_range.start,
                &filtered_comp_text,
            )
        } else if text_val.is_empty() {
            contents
                .placeholder
                .as_ref()
                .map(std::string::ToString::to_string)
                .unwrap_or_default()
        } else if contents.is_password {
            let mask = contents.mask_text.as_deref().unwrap_or("●");
            mask.repeat(text_val.chars().count())
        } else {
            text_val.clone()
        };

        let caret_text = if !filtered_comp_text.is_empty() {
            crate::input_get_display_text(
                &text_val_for_display,
                contents.selected_range.start,
                &filtered_comp_text,
            )
        } else if text_val.is_empty() {
            String::new()
        } else if contents.is_password {
            let mask = contents.mask_text.as_deref().unwrap_or("●");
            mask.repeat(text_val.chars().count())
        } else {
            text_val.clone()
        };

        let (font_size, font_family, font_weight, font_style) =
            RenderStore::get_font_propery(id, rnd_visual);

        let spans = cont_text_spans.get(id).map_or(&[][..], Vec::as_slice);

        // 描画テキスト全体のレイアウトサイズを Taffy 測定用に設定
        let display_layout = sys_text_engine.create_layout(
            &display_text,
            font_size,
            font_family,
            font_weight,
            font_style,
            None,
            spans,
        );
        let text_size = sys_text_engine.get_layout_size(&display_layout);
        contents.last_layout = Some(LayoutRect::new(0.0, 0.0, text_size.width, text_size.height));

        // キャレット位置測定用のレイアウトをプレースホルダー抜きで作成
        // 文字列が同一であればレイアウトの再生成をスキップ
        let caret_layout = if display_text == caret_text {
            &display_layout
        } else {
            &sys_text_engine.create_layout(
                &caret_text,
                font_size,
                font_family,
                font_weight,
                font_style,
                None,
                spans,
            )
        };

        let composition_offset = if let Some(ref ime) = contents.ime_state
            && !ime.composition_text.is_empty()
        {
            // 組成文字全体の文字数をオフセットとして適用
            ime.composition_text.encode_utf16().count()
        } else {
            0
        };

        // ドラッグの方向を判定しマウス位置にキャレットを固定
        let current_caret_relative = if contents.selection_reversed {
            contents.selected_range.start // 逆方向（左ドラッグ）時は左端がマウス位置
        } else {
            contents.selected_range.end // 順方向（右ドラッグ）時は右端がマウス位置
        };

        let caret_index = current_caret_relative + composition_offset;
        let u16_len_caret = caret_text.encode_utf16().count();

        // プレースホルダーに干渉されない純粋なキャレット位置を算出
        let (cx_offset, cy_offset, ch_height) =
            sys_text_engine.get_caret_position(caret_layout, caret_index, u16_len_caret);

        contents.measured_caret_x = cx_offset;
        contents.measured_caret_y = cy_offset;
        contents.caret_line_height = ch_height;

        let (curr_line, tot_lines) = crate::calculate_line_indices(&display_text, caret_index);
        contents.current_line_index = curr_line;
        contents.total_lines = tot_lines;

        // 最終表示用テキストを Context 側に反映
        cont_text_contents.insert(id, display_text.into());

        let visual = rnd_visual.get_mut(id)?;
        let is_ime_active = contents
            .ime_state
            .as_ref()
            .is_some_and(|ime| !ime.composition_text.is_empty());

        if text_val.is_empty() && !is_ime_active {
            // 確定文字列が空で、かつ未確定文字列も存在しない状態のみグレー表示
            visual.text_color = contents.placeholder_color;
        } else {
            let base_color = rnd_base_visual
                .get(id)
                .and_then(|v| v.text_color)
                .unwrap_or(Color::WHITE);
            visual.text_color = Some(base_color);
        }

        Some((
            LayoutRect {
                x: cx_offset,
                y: cy_offset,
                width: contents.caret_width.unwrap_or(contents.default_caret_width),
                height: ch_height,
            },
            contents.caret_offset,
            contents.is_multiline,
        ))
    }

    #[inline]
    pub(crate) fn clear_selection_highlight_rect(
        id: EntityId,
        topo_active_masks: &mut ActiveMasksSecondary,
        cont_input_contents: &mut InputContentsSparseSecondary,
        cont_text_spans: &mut TextSpansSparseSecondary,
        out_text_selections: &mut TextSelectionsSparseSecondary,
        out_selected_rects: &mut SelectedRectsSparseSecondary,
    ) {
        out_text_selections.remove(id);
        out_selected_rects.remove(id);
        if let Some(contents) = cont_input_contents.get_mut(id) {
            contents.selected_range = 0..0;
            // 進行中の IME コンポジションをリセットして波線を消去
            contents.ime_state = None;
            contents.marked_range = None;
        }
        cont_text_spans.remove(id);
        if let Some(i) = topo_active_masks.get_mut(id) {
            i.unset(STYLE_TEXT_SPANS);
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

    #[inline]
    pub(crate) fn calc_align_offset(
        rect: LayoutRect,
        border: EdgeInsets,
        padding: EdgeInsets,
        text_size: LayoutSize,
        text_align: TextAlign,
    ) -> LayoutPoint {
        let content_w =
            (rect.width - border.left - border.right - padding.left - padding.right).max(0.0);
        let align_offset_x = match text_align {
            TextAlign::Center => ((content_w - text_size.width) * 0.5).max(0.0),
            TextAlign::Right => (content_w - text_size.width).max(0.0),
            _ => 0.0,
        };

        let content_h =
            (rect.height - border.top - border.bottom - padding.top - padding.bottom).max(0.0);
        let align_offset_y = ((content_h - text_size.height) * 0.5).max(0.0);

        LayoutPoint {
            x: align_offset_x,
            y: align_offset_y,
        }
    }

    /// スクロールオフセットを目標位置へクランプした上で代入。
    /// オフセットに変化が生じた場合は true を返し、レイアウトのDirtyマークを打つ。
    pub(crate) fn scroll_to(
        id: EntityId,
        mut x: f32,
        mut y: f32,
        win_last_size: Option<LayoutSize>,
        topo_active_masks: &mut ActiveMasksSecondary,
        topo_parents: &ParentsSecondary,
        lay_taffy: &mut TaffyTreeEntityId,
        lay_dirty_entities: &mut DirtyLayoutEntitiesVec,
        lay_scrollbar_styles: &mut ScrollbarStylesSecondary,
        lay_taffy_nodes: &TaffyNodesSecondary,
        lay_basic: &BasicLayoutsSecondary,
        lay_flex: &FlexLayoutsSecondary,
        lay_grid: &GridLayoutsSecondary,
        rnd_visual: &VisualPropertiesSecondary,
        rnd_interaction: &InteractionPropertiesSecondary,
        rnd_active_transitions: &ActiveTransitionsSparseSecondary,
        out_rects: &RectsSecondary,
        out_scroll_offsets: &mut ScrollOffsetsSecondary,
        out_scroll_sizes: &ScrollSizesSecondary,
    ) -> bool {
        let Some(rect) = OutputStore::rect(id, out_rects) else {
            return false;
        };

        let scroll_size = out_scroll_sizes.get(id).copied().unwrap_or_default();

        // 親コンテナのボーダーおよびパディング厚を取得
        let (basic, _, _) = LayoutStore::resolve_active_layouts(
            id,
            topo_active_masks,
            topo_parents,
            lay_basic,
            lay_flex,
            lay_grid,
            rnd_interaction,
            rnd_visual,
            rnd_active_transitions,
        );
        let (border, padding) =
            LayoutStore::get_physical_border_padding(rect, basic.border, basic.padding);

        let visible_size = WindowStore::calculate_visible_size(win_last_size, rect);
        let content_size = LayoutStore::calculate_inner_content_size(visible_size, border, padding);

        // コンテンツサイズと内枠表示領域サイズの差分として、正確な最大スクロール量を算出
        let max_scroll_x = (scroll_size.width - content_size.width).max(0.0);
        let max_scroll_y = (scroll_size.height - content_size.height).max(0.0);

        x = x.clamp(0.0, max_scroll_x);
        y = y.clamp(0.0, max_scroll_y);

        // スロットが存在しない場合はあらかじめ挿入して初期化
        if !out_scroll_offsets.contains_key(id) {
            out_scroll_offsets.insert(id, LayoutPoint::ZERO);
        }

        let current = out_scroll_offsets.get_mut(id).unwrap();
        if (current.x - x).abs() > 0.01 || (current.y - y).abs() > 0.01 {
            current.x = x;
            current.y = y;

            // スクロールバー状態の最終スクロール時刻を更新
            if let Some(sb_state) = lay_scrollbar_styles.get_mut(id) {
                sb_state.last_scroll_time = Some(Instant::now());
            }

            // オフセット変化に伴い、子孫全体の絶対座標を再同期させる
            LayoutStore::mark_layout_dirty(
                id,
                topo_active_masks,
                topo_parents,
                lay_taffy,
                lay_dirty_entities,
                lay_taffy_nodes,
            );
            true
        } else {
            false
        }
    }

    pub(crate) fn scroll_by(
        id: EntityId,
        dx: f32,
        dy: f32,
        win_last_size: Option<LayoutSize>,
        topo_active_masks: &mut ActiveMasksSecondary,
        topo_parents: &ParentsSecondary,
        lay_taffy: &mut TaffyTreeEntityId,
        lay_dirty_entities: &mut DirtyLayoutEntitiesVec,
        lay_scrollbar_styles: &mut ScrollbarStylesSecondary,
        lay_taffy_nodes: &TaffyNodesSecondary,
        lay_basic: &BasicLayoutsSecondary,
        lay_flex: &FlexLayoutsSecondary,
        lay_grid: &GridLayoutsSecondary,
        rnd_visual: &VisualPropertiesSecondary,
        rnd_interaction: &InteractionPropertiesSecondary,
        rnd_active_transitions: &ActiveTransitionsSparseSecondary,
        out_scroll_offsets: &mut ScrollOffsetsSecondary,
        out_rects: &RectsSecondary,
        out_scroll_sizes: &ScrollSizesSecondary,
    ) -> bool {
        let current = out_scroll_offsets.get(id).copied().unwrap_or_default();

        OutputStore::scroll_to(
            id,
            current.x + dx,
            current.y + dy,
            win_last_size,
            topo_active_masks,
            topo_parents,
            lay_taffy,
            lay_dirty_entities,
            lay_scrollbar_styles,
            lay_taffy_nodes,
            lay_basic,
            lay_flex,
            lay_grid,
            rnd_visual,
            rnd_interaction,
            rnd_active_transitions,
            out_rects,
            out_scroll_offsets,
            out_scroll_sizes,
        )
    }

    /// 現在のテキスト・IME状態・フォントサイズから、
    /// キャレットの物理座標や最終表示テキスト、レイアウト矩形を正確に再計算して `SoA` を更新。
    pub(crate) fn update_input_caret_position(
        id: EntityId,
        win_last_size: Option<LayoutSize>,
        win_scale_factor: f32,
        sys_text_engine: &TextEngine,
        sys_dwrite_layouts: &DwriteLayoutsSparseSecondary,
        cont_input_contents: &mut InputContentsSparseSecondary,
        cont_text_contents: &mut TextContentsSparseSecondary,
        cont_text_spans: &TextSpansSparseSecondary,
        topo_active_masks: &mut ActiveMasksSecondary,
        topo_parents: &ParentsSecondary,
        lay_taffy: &mut TaffyTreeEntityId,
        lay_dirty_entities: &mut DirtyLayoutEntitiesVec,
        lay_scrollbar_styles: &mut ScrollbarStylesSecondary,
        lay_taffy_nodes: &TaffyNodesSecondary,
        lay_basic: &BasicLayoutsSecondary,
        lay_flex: &FlexLayoutsSecondary,
        lay_grid: &GridLayoutsSecondary,
        rnd_visual: &mut VisualPropertiesSecondary,
        rnd_base_visual: &BaseVisualPropertiesSecondary,
        rnd_interaction: &InteractionPropertiesSecondary,
        rnd_active_transitions: &ActiveTransitionsSparseSecondary,
        out_scroll_offsets: &mut ScrollOffsetsSecondary,
        out_text_selections: &mut TextSelectionsSparseSecondary,
        out_rects: &RectsSecondary,
        out_scroll_sizes: &ScrollSizesSecondary,
    ) {
        // IMEやタイピング中の古いキャッシュを破棄
        SystemStore::clear_layout_cache(id, sys_dwrite_layouts);

        let scroll_ime_info = OutputStore::scroll_ime_info(
            id,
            sys_text_engine,
            cont_input_contents,
            cont_text_contents,
            cont_text_spans,
            rnd_visual,
            rnd_base_visual,
            out_text_selections,
        );

        let Some((caret, caret_offset, is_multiline)) = scroll_ime_info else {
            return;
        };
        let (basic, flex, _) = LayoutStore::resolve_active_layouts(
            id,
            topo_active_masks,
            topo_parents,
            lay_basic,
            lay_flex,
            lay_grid,
            rnd_interaction,
            rnd_visual,
            rnd_active_transitions,
        );
        let rect = OutputStore::rect(id, out_rects).unwrap_or_default();
        let (border, padding) =
            LayoutStore::get_physical_border_padding(rect, basic.border, basic.padding);

        let mut scroll = out_scroll_offsets.get(id).copied().unwrap_or_default();

        if rect.width > 0.0 && rect.height > 0.0 {
            let viewport = LayoutStore::calculate_viewport_size(rect, border, padding);

            let text_size = if let Some(contents) = cont_input_contents.get(id)
                && let Some(layout_rect) = contents.last_layout
            {
                LayoutSize::new(layout_rect.width, layout_rect.height)
            } else {
                LayoutSize::ZERO
            };

            let align_offset =
                OutputStore::calc_align_offset(rect, border, padding, text_size, flex.text_align);

            let aligned_caret_x = caret.x + align_offset.x;
            let aligned_caret_y = caret.y + align_offset.y;

            // マージンを設定するとキー移動時にキャレット位置がずれるため削除
            // let margin_x = 0.0; // 左右端のあそび（マージン）

            // 1. 横方向スクロール (X軸)
            if aligned_caret_x < scroll.x {
                scroll.x = aligned_caret_x.max(0.0);
            } else if aligned_caret_x + caret.width > scroll.x + viewport.width {
                scroll.x = (aligned_caret_x + caret.width - viewport.width).max(0.0);
            }

            // 2. 縦方向スクロール (Y軸 - マルチラインのみ)
            if is_multiline {
                if aligned_caret_y < scroll.y {
                    scroll.y = aligned_caret_y.max(0.0);
                } else if aligned_caret_y + caret.height > scroll.y + viewport.height {
                    scroll.y = (aligned_caret_y + caret.height - viewport.height).max(0.0);
                }
            } else {
                scroll.y = 0.0;
            }

            OutputStore::scroll_to(
                id,
                scroll.x,
                scroll.y,
                win_last_size,
                topo_active_masks,
                topo_parents,
                lay_taffy,
                lay_dirty_entities,
                lay_scrollbar_styles,
                lay_taffy_nodes,
                lay_basic,
                lay_flex,
                lay_grid,
                rnd_visual,
                rnd_interaction,
                rnd_active_transitions,
                out_rects,
                out_scroll_offsets,
                out_scroll_sizes,
            );
        }

        // IMM32 による IME 変換候補ウィンドウの位置同期を自動実行
        SystemStore::sync_imm_window_position(
            rect,
            win_scale_factor,
            border,
            padding,
            caret,
            caret_offset,
            scroll,
        );
    }

    /// `現在の選択範囲（out_text_selections）に基づき`、
    /// `描画用の物理選択矩形（out_selected_rects）を自動再計算して` `SoA` キャッシュを更新します。
    #[inline]
    pub(crate) fn update_selection_rects(
        id: EntityId,
        layout: &IDWriteTextLayout,
        out_selected_rects: &mut SelectedRectsSparseSecondary,
        out_text_selections: &TextSelectionsSparseSecondary,
    ) {
        if let Some(range) = out_text_selections.get(id).cloned()
            && range.start < range.end
        {
            let out_rects = OutputStore::calc_selection_rects(id, layout, range);
            out_selected_rects.insert(id, out_rects);
            return;
        }
        // 範囲が 0、または選択なしの時は自動クリーンアップ
        out_selected_rects.remove(id);
    }

    pub(crate) fn sync_scrollbar_drag(
        logical_pos: LayoutPoint,
        win_last_size: Option<LayoutSize>,
        topo_active_masks: &mut ActiveMasksSecondary,
        topo_parents: &ParentsSecondary,
        lay_taffy: &mut TaffyTreeEntityId,
        lay_dirty_entities: &mut DirtyLayoutEntitiesVec,
        lay_scrollbar_styles: &mut ScrollbarStylesSecondary,
        lay_taffy_nodes: &TaffyNodesSecondary,
        lay_basic: &BasicLayoutsSecondary,
        lay_flex: &FlexLayoutsSecondary,
        lay_grid: &GridLayoutsSecondary,
        rnd_dirty_entities: &mut DirtyRenderEntitiesVec,
        rnd_visual: &VisualPropertiesSecondary,
        rnd_interaction: &InteractionPropertiesSecondary,
        rnd_active_transitions: &ActiveTransitionsSparseSecondary,
        out_scroll_offsets: &mut ScrollOffsetsSecondary,
        out_rects: &RectsSecondary,
        out_scroll_sizes: &ScrollSizesSecondary,
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

        let active_drag_target = lay_scrollbar_styles.iter().find_map(|(id, state)| {
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
            let sb_state = lay_scrollbar_styles
                .get(current_id)
                .cloned()
                .unwrap_or_default();
            let container_rect = OutputStore::rect(current_id, out_rects).unwrap_or_default();
            let scroll_size = out_scroll_sizes
                .get(current_id)
                .copied()
                .unwrap_or_default();
            (sb_state, container_rect, scroll_size)
        };

        let visible_size = WindowStore::calculate_visible_size(win_last_size, container_rect);

        let date = match direction {
            DragDirection::Vertical => {
                let track_id = sb_state.v_track_id.unwrap();
                let thumb_id = sb_state.v_thumb_id.unwrap();
                let track_rect = OutputStore::rect(track_id, out_rects).unwrap_or_default();
                let thumb_rect = OutputStore::rect(thumb_id, out_rects).unwrap_or_default();

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
                let track_rect = OutputStore::rect(track_id, out_rects).unwrap_or_default();
                let thumb_rect = OutputStore::rect(thumb_id, out_rects).unwrap_or_default();

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
                    let current_x = out_scroll_offsets.get(current_id).map_or(0.0, |o| o.x);
                    (current_x, target_scroll)
                }
                DragDirection::Horizontal => {
                    let current_y = out_scroll_offsets.get(current_id).map_or(0.0, |o| o.y);
                    (target_scroll, current_y)
                }
            };

            OutputStore::scroll_to(
                current_id,
                target_x,
                target_y,
                win_last_size,
                topo_active_masks,
                topo_parents,
                lay_taffy,
                lay_dirty_entities,
                lay_scrollbar_styles,
                lay_taffy_nodes,
                lay_basic,
                lay_flex,
                lay_grid,
                rnd_visual,
                rnd_interaction,
                rnd_active_transitions,
                out_rects,
                out_scroll_offsets,
                out_scroll_sizes,
            );
        }

        RenderStore::mark_render_dirty(current_id, topo_active_masks, rnd_dirty_entities);
    }

    /// 階層的な境界判定ヘルパー
    pub(crate) fn hit_test_recursive(
        id: EntityId,
        point: LayoutPoint,
        topo_children: &ChildrenSecondary,
        rnd_visual: &VisualPropertiesSecondary,
        rnd_base_visual: &BaseVisualPropertiesSecondary,
        out_rects: &RectsSecondary,
        out_clip_rects: &ClipRectsSecondary,
    ) -> Option<EntityId> {
        // 親などの overflow: hidden 等でクリップされている表示範囲をチェック
        // クリップ領域外であれば、この要素もそのすべての子孫要素も画面上に見えていないためをスキップ
        if let Some(clip) = OutputStore::clip_rect(id, out_clip_rects)
            && !clip.contains(point)
        {
            return None;
        }

        // 子要素を逆順で再帰降下
        if let Some(c) = topo_children.get(id) {
            for &child_id in c.iter().rev() {
                if let Some(hit) = OutputStore::hit_test_recursive(
                    child_id,
                    point,
                    topo_children,
                    rnd_visual,
                    rnd_base_visual,
                    out_rects,
                    out_clip_rects,
                ) {
                    return Some(hit);
                }
            }
        }

        // pointer_events: none の場合は、自分自身の矩形判定のみをスルーする (子要素は上を辿れるため除外しない)
        // rnd_visual に無ければ rnd_base_visual を見に行く
        if let Some(rect) = out_rects.get(id)
            && rect.contains(point)
        {
            let pointer_events = rnd_visual
                .get(id)
                .and_then(|v| v.pointer_events)
                .or_else(|| rnd_base_visual.get(id).and_then(|v| v.pointer_events))
                .unwrap_or_default();

            if pointer_events != PointerEvents::None {
                return Some(id);
            }
        }

        None
    }

    /// Taffy永続ツリーへのスタイル差分同期
    fn sync_dirty_styles_to_taffy(
        scrollbar_el_ids: &HashSet<EntityId>,
        topo_active_masks: &ActiveMasksSecondary,
        topo_parents: &ParentsSecondary,
        lay_taffy: &mut TaffyTreeEntityId,
        lay_dirty_entities: &DirtyLayoutEntitiesVec,
        lay_taffy_nodes: &TaffyNodesSecondary,
        lay_basic: &BasicLayoutsSecondary,
        lay_flex: &FlexLayoutsSecondary,
        lay_grid: &GridLayoutsSecondary,
        lay_scrollbar_styles: &ScrollbarStylesSecondary,
        rnd_interaction: &InteractionPropertiesSecondary,
        rnd_visual: &VisualPropertiesSecondary,
        rnd_active_transitions: &ActiveTransitionsSparseSecondary,
    ) {
        for &id in lay_dirty_entities {
            if scrollbar_el_ids.contains(&id) {
                continue;
            }

            let (mut basic, flex, grid) = LayoutStore::resolve_active_layouts(
                id,
                topo_active_masks,
                topo_parents,
                lay_basic,
                lay_flex,
                lay_grid,
                rnd_interaction,
                rnd_visual,
                rnd_active_transitions,
            );

            // トランジション（アニメーション）中プロパティの現在値による上書き
            if let Some(active_list) = rnd_active_transitions.get(id) {
                for t_state in active_list {
                    match t_state.property_list {
                        PropertyList::Width => {
                            if let Some(layout) = lay_basic.get(id) {
                                basic.size.width = layout.size.width;
                            }
                        }
                        PropertyList::Height => {
                            if let Some(layout) = lay_basic.get(id) {
                                basic.size.height = layout.size.height;
                            }
                        }
                        _ => {}
                    }
                }
            }

            let taffy_style = LayoutStore::resolve_taffy_style(
                id,
                &basic,
                &flex,
                grid.as_ref(),
                lay_scrollbar_styles,
            );

            if let Some(taffy_node) = lay_taffy_nodes.get(id) {
                lay_taffy.set_style(*taffy_node, taffy_style).unwrap();
            }
        }
    }

    /// 物理位置を算出して、rects / `clip_rects` と入力状態へマウント
    fn update_element_output_rect_and_clip(
        id: EntityId,
        window_size: LayoutSize,
        cont_input_contents: &mut InputContentsSparseSecondary,
        topo_active_entities: &mut ActiveEntitiesVec,
        topo_active_masks: &ActiveMasksSecondary,
        topo_parents: &ParentsSecondary,
        lay_taffy: &TaffyTreeEntityId,
        lay_taffy_nodes: &TaffyNodesSecondary,
        lay_basic: &BasicLayoutsSecondary,
        out_rects: &mut RectsSecondary,
        out_clip_rects: &mut ClipRectsSecondary,
        out_scroll_offsets: &ScrollOffsetsSecondary,
    ) {
        let (abs_rect, parent_clip) = OutputStore::calc_local_rect(
            id,
            window_size,
            topo_parents,
            lay_taffy,
            lay_taffy_nodes,
            lay_basic,
            out_rects,
            out_clip_rects,
            out_scroll_offsets,
        );

        out_rects.insert(id, abs_rect);

        let mask = topo_active_masks.get(id).copied().unwrap_or_default();

        if mask.has_input_content()
            && let Some(contents) = cont_input_contents.get_mut(id)
        {
            contents.last_bounds = Some(abs_rect);
        }

        let current_clip = if mask.has(STYLE_OVERFLOW) {
            parent_clip.intersect(&abs_rect)
        } else {
            parent_clip
        };

        out_clip_rects.insert(id, current_clip);
        topo_active_entities.push(id);
    }

    /// 1回目の出力領域決定（静的キャッシュバイパス判定含む）
    fn resolve_first_pass_rects(
        scrollbar_el_ids: &HashSet<EntityId>,
        window_size: LayoutSize,
        window_resized: bool,
        cont_input_contents: &mut InputContentsSparseSecondary,
        topo_active_entities: &mut ActiveEntitiesVec,
        topo_active_masks: &ActiveMasksSecondary,
        topo_parents: &ParentsSecondary,
        topo_flat_dfs_sequence: &FlatDfsSequenceVec,
        lay_taffy: &TaffyTreeEntityId,
        lay_taffy_nodes: &TaffyNodesSecondary,
        lay_basic: &BasicLayoutsSecondary,
        out_rects: &mut RectsSecondary,
        out_clip_rects: &mut ClipRectsSecondary,
        out_prev_rects: &PrevRectsSecondary,
        out_prev_clip_rects: &PrevClipRectsSecondary,
        out_scroll_offsets: &ScrollOffsetsSecondary,
    ) {
        topo_active_entities.clear();

        for &id in topo_flat_dfs_sequence {
            // スクロールバー専用子要素は手動で物理座標を強制更新するため、この走査ループから完全にスルー
            if scrollbar_el_ids.contains(&id) {
                continue;
            }

            let parent_changed = OutputStore::has_parent_changed(
                id,
                topo_active_masks,
                topo_parents,
                out_rects,
                out_prev_rects,
                out_clip_rects,
                out_prev_clip_rects,
            );

            let has_style_changed = topo_active_masks
                .get(id)
                .is_some_and(|m| m.has(STATE_QUEUED_LAYOUT));

            // 静的キャッシュの判定と適用
            // 自分自身のスタイルが変わっておらず、親も動いていない、かつモニターリサイズもされていないならキャッシュ利用
            if !window_resized
                && !has_style_changed
                && !parent_changed
                && let Some(&cached_rect) = out_prev_rects.get(id)
                && let Some(&cached_clip) = out_prev_clip_rects.get(id)
            {
                out_rects.insert(id, cached_rect);
                out_clip_rects.insert(id, cached_clip);
                topo_active_entities.push(id);
                continue;
            }

            // キャッシュが無効な場合は、共通ヘルパーで再計算
            OutputStore::update_element_output_rect_and_clip(
                id,
                window_size,
                cont_input_contents,
                topo_active_entities,
                topo_active_masks,
                topo_parents,
                lay_taffy,
                lay_taffy_nodes,
                lay_basic,
                out_rects,
                out_clip_rects,
                out_scroll_offsets,
            );
        }
    }

    /// 最終的な出力領域決定（スクロールバー要素を含む一括同期）
    fn resolve_final_pass_rects(
        window_size: LayoutSize,
        cont_input_contents: &mut InputContentsSparseSecondary,
        topo_active_entities: &mut ActiveEntitiesVec,
        topo_active_masks: &ActiveMasksSecondary,
        topo_parents: &ParentsSecondary,
        topo_flat_dfs_sequence: &FlatDfsSequenceVec,
        lay_taffy: &TaffyTreeEntityId,
        lay_taffy_nodes: &TaffyNodesSecondary,
        lay_basic: &BasicLayoutsSecondary,
        out_rects: &mut RectsSecondary,
        out_clip_rects: &mut ClipRectsSecondary,
        out_scroll_offsets: &ScrollOffsetsSecondary,
    ) {
        topo_active_entities.clear();

        for &id in topo_flat_dfs_sequence {
            OutputStore::update_element_output_rect_and_clip(
                id,
                window_size,
                cont_input_contents,
                topo_active_entities,
                topo_active_masks,
                topo_parents,
                lay_taffy,
                lay_taffy_nodes,
                lay_basic,
                out_rects,
                out_clip_rects,
                out_scroll_offsets,
            );
        }
    }

    /// 全アクティブコンテナのスクロールオフセットの自動クランプ同期
    fn auto_clamp_scroll_offsets(
        win_last_size: Option<LayoutSize>,
        topo_active_masks: &mut ActiveMasksSecondary,
        topo_parents: &ParentsSecondary,
        topo_flat_dfs_sequence: &FlatDfsSequenceVec,
        lay_taffy: &mut TaffyTreeEntityId,
        lay_dirty_entities: &mut DirtyLayoutEntitiesVec,
        lay_scrollbar_styles: &mut ScrollbarStylesSecondary,
        lay_taffy_nodes: &TaffyNodesSecondary,
        lay_basic: &BasicLayoutsSecondary,
        lay_flex: &FlexLayoutsSecondary,
        lay_grid: &GridLayoutsSecondary,
        rnd_visual: &VisualPropertiesSecondary,
        rnd_interaction: &InteractionPropertiesSecondary,
        rnd_active_transitions: &ActiveTransitionsSparseSecondary,
        out_scroll_offsets: &mut ScrollOffsetsSecondary,
        out_rects: &RectsSecondary,
        out_scroll_sizes: &ScrollSizesSecondary,
    ) {
        for &id in topo_flat_dfs_sequence {
            let Some(current) = out_scroll_offsets.get(id).copied() else {
                continue;
            };

            // 枠サイズの変更など、現在のスクロール位置からはみ出していれば自動クランプ調整
            OutputStore::scroll_to(
                id,
                current.x,
                current.y,
                win_last_size,
                topo_active_masks,
                topo_parents,
                lay_taffy,
                lay_dirty_entities,
                lay_scrollbar_styles,
                lay_taffy_nodes,
                lay_basic,
                lay_flex,
                lay_grid,
                rnd_visual,
                rnd_interaction,
                rnd_active_transitions,
                out_rects,
                out_scroll_offsets,
                out_scroll_sizes,
            );
        }
    }

    pub(crate) fn sync_layout_and_render_list_internal(
        cx: &mut Context,
        root: EntityId,
        window_size: LayoutSize,
    ) {
        // 同期処理の開始時に自身をバインドする
        let _context_guard = bind_context(cx);

        // レイアウトが再計算される前に、溜まっているすべてのエフェクトを評価完了させる
        ReactiveStore::evaluate_pending_element_effects(
            &mut cx.reactive.react_effects,
            &mut cx.reactive.react_pending_element_effects,
        );
        // ウィンドウサイズの変更検知
        let window_resized =
            WindowStore::window_resize_detection(window_size, &mut cx.window.win_last_size);

        // 構造変更がなく、スタイル変更（レイアウト変更要求）もなく、ウィンドウサイズも変わっていないなら、
        // すべてスキップして早期リターン。
        if cx.layouts.lay_dirty_entities.is_empty()
            && !cx.topology.topo_is_structure_dirty
            && !window_resized
            && !cx.outputs.out_rects.is_empty()
        {
            return;
        }

        // DFSツリーシーケンスの再構築
        if cx.topology.topo_is_structure_dirty {
            TopologyStore::rebuild_dfs_sequence(
                root,
                &mut cx.topology.topo_flat_dfs_sequence,
                &mut cx.topology.topo_is_structure_dirty,
                &cx.topology.topo_children,
            );
        }

        // 全スクロールバー関連IDを一括抽出
        let scrollbar_el_ids = LayoutStore::scrollbar_el_ids(&cx.layouts.lay_scrollbar_styles);

        // Taffy永続ツリーへの差分同期
        OutputStore::sync_dirty_styles_to_taffy(
            &scrollbar_el_ids,
            &cx.topology.topo_active_masks,
            &cx.topology.topo_parents,
            &mut cx.layouts.lay_taffy,
            &cx.layouts.lay_dirty_entities,
            &cx.layouts.lay_taffy_nodes,
            &cx.layouts.lay_basic,
            &cx.layouts.lay_flex,
            &cx.layouts.lay_grid,
            &cx.layouts.lay_scrollbar_styles,
            &cx.renders.rnd_interaction,
            &cx.renders.rnd_visual,
            &cx.renders.rnd_active_transitions,
        );

        // Taffy 1回目レイアウト計算
        if let Some(&root_node) = cx.layouts.lay_taffy_nodes.get(root) {
            // 計測関数をクロージャとして定義
            let measure_func = |known_dims: taffy::Size<Option<f32>>,
                                available_space: taffy::Size<taffy::AvailableSpace>,
                                _node_id: taffy::NodeId,
                                context: Option<&mut EntityId>,
                                _style: &taffy::Style|
             -> taffy::Size<f32> {
                // 幅と高さの両方がすでにスタイル（known_dims）として解決されている場合はそれを最優先する
                if let (Some(w), Some(h)) = (known_dims.width, known_dims.height) {
                    return taffy::Size {
                        width: w,
                        height: h,
                    };
                }

                // テキスト内容を持っているかチェック
                // クロージャの外側の Context は直接キャプチャできないため、
                //  一時的に bind_context されているスレッドローカル経由で取得
                context.as_deref().copied().map_or(taffy::Size::ZERO, |id| {
                    with_context(|cx| {
                        ContentStore::measure_content(
                            id,
                            known_dims,
                            &cx.system.sys_text_engine,
                            &mut cx.contents.cont_input_contents,
                            &cx.contents.cont_text_contents,
                            &cx.contents.cont_text_spans,
                            &cx.topology.topo_active_masks,
                            &cx.renders.rnd_visual,
                        )
                    })
                })
            };

            let _ = cx.layouts.lay_taffy.compute_layout_with_measure(
                root_node,
                taffy::Size {
                    width: taffy::AvailableSpace::Definite(window_size.width),
                    height: taffy::AvailableSpace::Definite(window_size.height),
                },
                measure_func,
            );
        }

        // ダブルバッファをスワップし、1回目の出力座標を決定
        // scroll_size を正しく算出するため、スワップおよび一旦コンテンツの out_rects のみを確定
        OutputStore::swap_output_rect(
            &mut cx.outputs.out_rects,
            &mut cx.outputs.out_prev_rects,
            &mut cx.outputs.out_clip_rects,
            &mut cx.outputs.out_prev_clip_rects,
        );
        OutputStore::resolve_first_pass_rects(
            &scrollbar_el_ids,
            window_size,
            window_resized,
            &mut cx.contents.cont_input_contents,
            &mut cx.topology.topo_active_entities,
            &cx.topology.topo_active_masks,
            &cx.topology.topo_parents,
            &cx.topology.topo_flat_dfs_sequence,
            &cx.layouts.lay_taffy,
            &cx.layouts.lay_taffy_nodes,
            &cx.layouts.lay_basic,
            &mut cx.outputs.out_rects,
            &mut cx.outputs.out_clip_rects,
            &cx.outputs.out_prev_rects,
            &cx.outputs.out_prev_clip_rects,
            &cx.outputs.out_scroll_offsets,
        );

        // 全スクロールコンテナの scroll_size を事前計算
        cx.outputs.out_scroll_sizes.clear();
        for &id in &cx.topology.topo_flat_dfs_sequence {
            if cx
                .topology
                .topo_active_masks
                .get(id)
                .is_some_and(|m| m.has(STYLE_OVERFLOW))
            {
                let size = OutputStore::get_scroll_size(
                    id,
                    &cx.system.sys_text_engine,
                    &cx.system.sys_dwrite_layouts,
                    &cx.contents.cont_input_contents,
                    &cx.contents.cont_text_contents,
                    &cx.contents.cont_text_spans,
                    &cx.topology.topo_active_masks,
                    &cx.topology.topo_parents,
                    &cx.topology.topo_children,
                    &cx.layouts.lay_basic,
                    &cx.layouts.lay_flex,
                    &cx.layouts.lay_grid,
                    &cx.layouts.lay_scrollbar_styles,
                    &cx.renders.rnd_visual,
                    &cx.renders.rnd_interaction,
                    &cx.renders.rnd_active_transitions,
                    &cx.outputs.out_rects,
                    &cx.outputs.out_scroll_offsets,
                );
                cx.outputs.out_scroll_sizes.insert(id, size);
            }
        }

        // スクロールバー要素（Track & Thumb）のサイズ・配置・不透明度を一括同期更新
        LayoutStore::sync_scrollbar_styles(
            cx.window.win_last_size,
            &cx.system.sys_text_engine,
            &cx.system.sys_dwrite_layouts,
            &cx.contents.cont_input_contents,
            &cx.contents.cont_text_contents,
            &cx.contents.cont_text_spans,
            &cx.topology.topo_active_masks,
            &cx.topology.topo_parents,
            &cx.topology.topo_children,
            &mut cx.layouts.lay_taffy,
            &mut cx.layouts.lay_basic,
            &mut cx.layouts.lay_base_basic,
            &cx.layouts.lay_flex,
            &cx.layouts.lay_grid,
            &cx.layouts.lay_taffy_nodes,
            &cx.layouts.lay_scrollbar_styles,
            &mut cx.renders.rnd_visual,
            &mut cx.renders.rnd_base_visual,
            &cx.renders.rnd_active_transitions,
            &cx.renders.rnd_interaction,
            &cx.outputs.out_rects,
            &cx.outputs.out_scroll_offsets,
            &cx.outputs.out_scroll_sizes,
        );

        // Taffy の 2回目レイアウト計算（スクロールバー配置確定後）
        if let Some(&root_node) = cx.layouts.lay_taffy_nodes.get(root) {
            let _ = cx.layouts.lay_taffy.compute_layout_with_measure(
                root_node,
                taffy::Size {
                    width: taffy::AvailableSpace::Definite(window_size.width),
                    height: taffy::AvailableSpace::Definite(window_size.height),
                },
                |known_dims: taffy::Size<Option<f32>>,
                 _available_space: taffy::Size<taffy::AvailableSpace>,
                 _node_id: taffy::NodeId,
                 context: Option<&mut EntityId>,
                 _style: &taffy::Style|
                 -> taffy::Size<f32> {
                    context.as_deref().copied().map_or(taffy::Size::ZERO, |id| {
                        with_context(|cx| {
                            let is_input = cx
                                .topology
                                .topo_active_masks
                                .get(id)
                                .is_some_and(ComponentMask::has_input_content);

                            if is_input
                                && let Some(contents) = cx.contents.cont_input_contents.get(id)
                                && let Some(layout_rect) = contents.last_layout
                            {
                                return taffy::Size {
                                    width: known_dims.width.unwrap_or(layout_rect.width),
                                    height: known_dims.height.unwrap_or(layout_rect.height),
                                };
                            }

                            // 2回目パスはキャッシュサイズを即時引き出して高速マッピング
                            cx.outputs
                                .out_rects
                                .get(id)
                                .map_or(taffy::Size::ZERO, |rect| taffy::Size {
                                    width: known_dims.width.unwrap_or(rect.width),
                                    height: known_dims.height.unwrap_or(rect.height),
                                })
                        })
                    })
                },
            );
        }

        // スクロールバーも加えた、最終的な出力座標の決定
        OutputStore::resolve_final_pass_rects(
            window_size,
            &mut cx.contents.cont_input_contents,
            &mut cx.topology.topo_active_entities,
            &cx.topology.topo_active_masks,
            &cx.topology.topo_parents,
            &cx.topology.topo_flat_dfs_sequence,
            &cx.layouts.lay_taffy,
            &cx.layouts.lay_taffy_nodes,
            &cx.layouts.lay_basic,
            &mut cx.outputs.out_rects,
            &mut cx.outputs.out_clip_rects,
            &cx.outputs.out_scroll_offsets,
        );

        // 全アクティブコンテナのスクロールオフセット自動クランプ同期
        OutputStore::auto_clamp_scroll_offsets(
            cx.window.win_last_size,
            &mut cx.topology.topo_active_masks,
            &cx.topology.topo_parents,
            &cx.topology.topo_flat_dfs_sequence,
            &mut cx.layouts.lay_taffy,
            &mut cx.layouts.lay_dirty_entities,
            &mut cx.layouts.lay_scrollbar_styles,
            &cx.layouts.lay_taffy_nodes,
            &cx.layouts.lay_basic,
            &cx.layouts.lay_flex,
            &cx.layouts.lay_grid,
            &cx.renders.rnd_visual,
            &cx.renders.rnd_interaction,
            &cx.renders.rnd_active_transitions,
            &mut cx.outputs.out_scroll_offsets,
            &cx.outputs.out_rects,
            &cx.outputs.out_scroll_sizes,
        );

        // 全ての座標確定と絶対クリップ範囲の同期が完了した最末尾で、
        // 一括して Dirty フラグの完全クリアおよびキューリストのリセットを実行
        LayoutStore::clear_layout_dirty(
            &mut cx.topology.topo_active_masks,
            &mut cx.layouts.lay_dirty_entities,
        );
    }

    // 溜まっているインスタンスを DrawBatch としてフラッシュ
    #[inline]
    fn flush_batch(
        batches: &mut Vec<DrawBatch>,
        instances_len: usize,
        last_flushed_offset: &mut usize,
        scissor_rect: LayoutRect,
        batch_type: BatchType,
    ) {
        let count = instances_len - *last_flushed_offset;
        if count == 0 {
            return;
        }

        batches.push(DrawBatch {
            scissor_rect,
            instance_offset: *last_flushed_offset,
            instance_count: count,
            batch_type,
        });

        // 次のバッチのために、現在の末尾位置を記録しておく
        *last_flushed_offset = instances_len;
    }

    /// 現在の全アクティブ要素から、wgpu 用の前面・背面描画バッチを生成します
    pub(crate) fn collect_render_data(
        render_data: &mut RenderData,
        win_scale_factor: f32,
        cont_input_contents: &InputContentsSparseSecondary,
        evt_interaction_states: &InteractionStates,
        topo_sorted_entities: &mut SortedEntitiesVec,
        topo_effective_transforms: &mut EffectiveTransformsSecondary,
        topo_effective_z_indices: &mut EffectiveZindicesSecondary,
        topo_dfs_indices: &mut DfsIndicesSecondary,
        topo_sort_cache: &mut TopoSortCacheVec,
        topo_active_entities: &ActiveEntitiesVec,
        topo_active_masks: &ActiveMasksSecondary,
        topo_parents: &ParentsSecondary,
        topo_flat_dfs_sequence: &FlatDfsSequenceVec,
        lay_basic: &BasicLayoutsSecondary,
        lay_flex: &FlexLayoutsSecondary,
        lay_grid: &GridLayoutsSecondary,
        rnd_visual: &VisualPropertiesSecondary,
        rnd_base_visual: &BaseVisualPropertiesSecondary,
        rnd_interaction: &InteractionPropertiesSecondary,
        rnd_active_transitions: &ActiveTransitionsSparseSecondary,
        rnd_active_webviews: &ActiveWebviewsHashSet,
        out_rects: &RectsSecondary,
        out_clip_rects: &ClipRectsSecondary,
        out_selected_rects: &SelectedRectsSparseSecondary,
        out_scroll_offsets: &ScrollOffsetsSecondary,
    ) {
        let mut last_clip = None;

        render_data.clear();

        // フラッシュ済みのオフセット位置を追跡する変数
        let mut last_flushed_offset = 0;

        // 現在のバッチの種類 (通常)
        let mut current_batch_type = BatchType::Normal;

        // 静的なデフォルト値（一度だけ確保して使い回す）
        let default_visual = VisualProperty::default();

        // 各要素の実効トランスフォーム行列を DFS 順にカスケード累積
        RenderStore::accumulate_transform_matrix(
            topo_effective_transforms,
            topo_active_entities,
            topo_parents,
            topo_flat_dfs_sequence,
            rnd_visual,
        );

        // 実効 z_index の計算とソートを一括実行
        TopologyStore::prepare_sorted_entities(
            topo_sorted_entities,
            topo_effective_z_indices,
            topo_dfs_indices,
            topo_sort_cache,
            topo_active_entities,
            topo_parents,
            topo_flat_dfs_sequence,
            rnd_visual,
        );

        for &id in &*topo_sorted_entities {
            let rect = OutputStore::rect(id, out_rects).unwrap_or_default();
            if rect.width <= 0.0 || rect.height <= 0.0 {
                continue;
            }

            let clip = OutputStore::clip_rect(id, out_clip_rects).unwrap_or_default();
            let is_webview = topo_active_masks
                .get(id)
                .is_some_and(ComponentMask::has_webveiw2_content);

            // コントローラーがまだ初期化されていない場合は通常通り背景を描画し透過を防止
            let is_webview_ready = is_webview && rnd_active_webviews.contains(&id);

            let (basic, flex, grid) = LayoutStore::resolve_active_layouts(
                id,
                topo_active_masks,
                topo_parents,
                lay_basic,
                lay_flex,
                lay_grid,
                rnd_interaction,
                rnd_visual,
                rnd_active_transitions,
            );
            let visual = rnd_visual.get(id).unwrap_or(&default_visual);

            // 共通パラメータの展開
            let (packed_transform, origin) =
                RenderStore::get_transform_and_origin(id, visual, topo_effective_transforms);
            let (o_width, o_color, o_lengths, outline_offset_and_flags) =
                RenderStore::get_outline_params(visual);

            // WebView (アクティブ) の個別処理
            if is_webview_ready {
                // 溜まっている通常（Normal）のバッチがあれば一旦フラッシュ
                OutputStore::flush_batch(
                    &mut render_data.batches,
                    render_data.instances.len(),
                    &mut last_flushed_offset,
                    last_clip.unwrap_or_default(),
                    current_batch_type,
                );

                let punchout_opacity = visual.opacity.unwrap_or(1.0);
                let punchout_instance = QuadInstance {
                    rect,
                    transform: packed_transform,
                    transform_origin: origin,
                    color: Color::WHITE,
                    corner_radius: visual.corner_radius.unwrap_or_default(),
                    opacity_mode_sizing: [punchout_opacity, 0.0, 0.0, 0.0],
                    ..Default::default()
                };
                render_data.instances.push(punchout_instance);
                render_data.entity_ids.push(id);

                // くり抜き用のバッチとして即座にフラッシュ
                OutputStore::flush_batch(
                    &mut render_data.batches,
                    render_data.instances.len(),
                    &mut last_flushed_offset,
                    clip,
                    BatchType::Punchout,
                );

                // 前面装飾（通常）用のインスタンス
                let border_instance = QuadInstance {
                    rect,
                    transform: packed_transform,
                    transform_origin: origin,
                    corner_radius: visual.corner_radius.unwrap_or_default(),
                    border_width: EdgeInsets {
                        top: basic.border.top.into(),
                        right: basic.border.right.into(),
                        bottom: basic.border.bottom.into(),
                        left: basic.border.left.into(),
                    },
                    border_color: visual.border_color.unwrap_or_default(),
                    border_lengths: visual.border_lengths.unwrap_or(EdgeInsets::px_all(1.0)),
                    opacity_mode_sizing: [visual.opacity.unwrap_or(1.0), 0.0, 0.0, 0.0],
                    outline_width: o_width,
                    outline_color: o_color,
                    outline_lengths: o_lengths,
                    outline_offset_and_flags,
                    ..Default::default()
                };
                render_data.instances.push(border_instance);
                render_data.entity_ids.push(id);

                current_batch_type = BatchType::Normal;
                last_clip = Some(clip);
                continue;
            }

            // WebView (非アクティブ・静止キャッシュ) の処理
            let is_webview_static = is_webview && !is_webview_ready;
            if is_webview_static {
                // 一般UIインスタンスがあれば強制フラッシュ
                OutputStore::flush_batch(
                    &mut render_data.batches,
                    render_data.instances.len(),
                    &mut last_flushed_offset,
                    last_clip.unwrap_or_default(),
                    current_batch_type,
                );

                let static_instance = QuadInstance {
                    rect,
                    transform: packed_transform,
                    transform_origin: origin,
                    corner_radius: visual.corner_radius.unwrap_or_default(),
                    border_width: EdgeInsets {
                        top: basic.border.top.into(),
                        right: basic.border.right.into(),
                        bottom: basic.border.bottom.into(),
                        left: basic.border.left.into(),
                    },
                    border_color: visual.border_color.unwrap_or_default(),
                    border_lengths: visual.border_lengths.unwrap_or(EdgeInsets::px_all(1.0)),
                    opacity_mode_sizing: [visual.opacity.unwrap_or(1.0), 0.0, 0.0, 0.0],
                    ..Default::default()
                };
                render_data.instances.push(static_instance);
                render_data.entity_ids.push(id);

                OutputStore::flush_batch(
                    &mut render_data.batches,
                    render_data.instances.len(),
                    &mut last_flushed_offset,
                    clip,
                    BatchType::Normal,
                );

                let border_instance = QuadInstance {
                    rect,
                    transform: packed_transform,
                    transform_origin: origin,
                    corner_radius: visual.corner_radius.unwrap_or_default(),
                    border_width: EdgeInsets {
                        top: basic.border.top.into(),
                        right: basic.border.right.into(),
                        bottom: basic.border.bottom.into(),
                        left: basic.border.left.into(),
                    },
                    border_color: visual.border_color.unwrap_or_default(),
                    border_lengths: visual.border_lengths.unwrap_or(EdgeInsets::px_all(1.0)),
                    opacity_mode_sizing: [visual.opacity.unwrap_or(1.0), 0.0, 0.0, 0.0],
                    shadow_color: Color::WHITE,
                    outline_width: o_width,
                    outline_color: o_color,
                    outline_lengths: o_lengths,
                    outline_offset_and_flags,
                    ..Default::default()
                };
                render_data.instances.push(border_instance);
                render_data.entity_ids.push(id);

                OutputStore::flush_batch(
                    &mut render_data.batches,
                    render_data.instances.len(),
                    &mut last_flushed_offset,
                    clip,
                    BatchType::Normal,
                );

                last_clip = Some(clip);
                continue;
            }

            // 一般要素
            if let Some(prev_clip) = last_clip {
                if clip != prev_clip {
                    OutputStore::flush_batch(
                        &mut render_data.batches,
                        render_data.instances.len(),
                        &mut last_flushed_offset,
                        prev_clip,
                        current_batch_type,
                    );
                    last_clip = Some(clip);
                }
            } else {
                last_clip = Some(clip);
            }

            // 選択ハイライト背景のwgpu側への差し込み
            if let Some(out_rects) = out_selected_rects.get(id) {
                let (border, padding) =
                    LayoutStore::get_physical_border_padding(rect, basic.border, basic.padding);

                let sel_bg = visual
                    .select_bg_color
                    .unwrap_or(Color::rgba_f32(0.0, 0.47, 0.84, 0.35));

                let scroll = out_scroll_offsets.get(id).copied().unwrap_or_default();

                let text_size = if let Some(contents) = cont_input_contents.get(id)
                    && let Some(layout_rect) = contents.last_layout
                {
                    LayoutSize::new(layout_rect.width, layout_rect.height)
                } else {
                    LayoutSize::ZERO
                };

                let align_offset = OutputStore::calc_align_offset(
                    rect,
                    border,
                    padding,
                    text_size,
                    flex.text_align,
                );

                for metric_rect in out_rects {
                    let sel_rect = LayoutRect::new(
                        rect.x + border.left + padding.left + align_offset.x + metric_rect.x
                            - scroll.x,
                        rect.y + border.top + padding.top + align_offset.y + metric_rect.y
                            - scroll.y,
                        metric_rect.width,
                        metric_rect.height,
                    );

                    let sel_instance = QuadInstance {
                        rect: sel_rect,
                        transform: packed_transform,
                        color: sel_bg,
                        opacity_mode_sizing: [visual.opacity.unwrap_or(1.0), -1.0, 0.0, 0.0],
                        ..Default::default()
                    };
                    render_data.instances.push(sel_instance);
                    render_data.entity_ids.push(id);
                }
            }

            // 背景色とテキストの多重描画の解決
            let is_text = topo_active_masks[id].has_text_content();
            let has_bg = visual.bg_color.is_some()
                || visual.bg_gradient.is_some()
                || visual.border_color.is_some()
                || visual.shadow_params.is_some();

            let box_sizing_val = match basic.box_sizing {
                BoxSizing::BorderBox => 0.0f32,
                BoxSizing::ContentBox => 1.0f32,
            };

            if is_text && has_bg {
                let bg_color = visual.bg_color.unwrap_or_default();
                let (gradient_end_color, gradient_angle, bg_mode) = match visual.bg_gradient {
                    Some(g) => (g.end_color, g.angle, 1.0f32),
                    None => (bg_color, 0.0, 0.0f32),
                };

                let bg_instance = QuadInstance {
                    rect,
                    transform: packed_transform,
                    transform_origin: origin,
                    color: bg_color,
                    corner_radius: visual.corner_radius.unwrap_or_default(),
                    border_width: EdgeInsets {
                        top: basic.border.top.into(),
                        right: basic.border.right.into(),
                        bottom: basic.border.bottom.into(),
                        left: basic.border.left.into(),
                    },
                    border_color: visual.border_color.unwrap_or_default(),
                    border_lengths: visual.border_lengths.unwrap_or(EdgeInsets::px_all(1.0)),
                    opacity_mode_sizing: [
                        visual.opacity.unwrap_or(1.0),
                        bg_mode,
                        box_sizing_val,
                        0.0,
                    ],
                    gradient_end_color,
                    gradient_angle,
                    shadow_color: Color::WHITE,
                    shadow_params: [0.0; 4],
                    outline_width: o_width,
                    outline_color: o_color,
                    outline_lengths: o_lengths,
                    outline_offset_and_flags,
                    ..Default::default()
                };
                render_data.instances.push(bg_instance);
                render_data.entity_ids.push(id);
            }

            // 通常のテキスト / 背景のレンダリング
            let color = if is_text {
                visual.text_color.unwrap_or(Color::BLACK)
            } else {
                visual.bg_color.unwrap_or_default()
            };

            let (gradient_end_color, gradient_angle, mut mode) = match visual.bg_gradient {
                Some(g) => (g.end_color, g.angle, 1.0f32),
                None => (color, 0.0, 0.0f32),
            };

            if is_text {
                mode = 2.0;
            }

            // テキスト要素で背景を分離描画した場合、テキストレイヤー側の装飾をクリア
            let bypass_decorations = is_text && has_bg;
            let border_width = if bypass_decorations {
                EdgeInsets::ZERO
            } else {
                EdgeInsets {
                    top: basic.border.top.into(),
                    right: basic.border.right.into(),
                    bottom: basic.border.bottom.into(),
                    left: basic.border.left.into(),
                }
            };
            let border_lengths = if bypass_decorations {
                EdgeInsets::ZERO
            } else {
                visual.border_lengths.unwrap_or(EdgeInsets::px_all(1.0))
            };
            let border_color = if bypass_decorations {
                Color::TRANSPARENT
            } else {
                visual.border_color.unwrap_or_default()
            };
            let shadow_color = if bypass_decorations {
                Color::TRANSPARENT
            } else {
                Color::WHITE
            };
            let outline_width = if bypass_decorations {
                EdgeInsets::ZERO
            } else {
                o_width
            };
            let outline_color = if bypass_decorations {
                Color::TRANSPARENT
            } else {
                o_color
            };
            let outline_lengths = if bypass_decorations {
                EdgeInsets::ZERO
            } else {
                o_lengths
            };
            let outline_offset_and_flags = if bypass_decorations {
                [0.0; 4]
            } else {
                outline_offset_and_flags
            };

            let instance = QuadInstance {
                rect,
                transform: packed_transform,
                transform_origin: origin,
                color,
                corner_radius: visual.corner_radius.unwrap_or_default(),
                border_width,
                border_color,
                border_lengths,
                opacity_mode_sizing: [visual.opacity.unwrap_or(1.0), mode, 0.0, 0.0],
                gradient_end_color,
                gradient_angle,
                shadow_color,
                outline_width,
                outline_color,
                outline_lengths,
                outline_offset_and_flags,
                ..Default::default()
            };

            render_data.instances.push(instance);
            render_data.entity_ids.push(id);

            // インプット要素のキャレット描画
            let is_input = topo_active_masks
                .get(id)
                .is_some_and(ComponentMask::has_input_content);
            let is_focused = evt_interaction_states.focused == Some(id);

            if is_input
                && is_focused
                && let Some(contents) = cont_input_contents.get(id)
                && ContentStore::should_show_caret(contents)
            {
                let (border, padding) =
                    LayoutStore::get_physical_border_padding(rect, basic.border, basic.padding);
                let scroll = out_scroll_offsets.get(id).copied().unwrap_or_default();

                let text_size = if let Some(layout_rect) = contents.last_layout {
                    LayoutSize::new(layout_rect.width, layout_rect.height)
                } else {
                    LayoutSize::ZERO
                };

                let align_offset = OutputStore::calc_align_offset(
                    rect,
                    border,
                    padding,
                    text_size,
                    flex.text_align,
                );

                let caret_rect = OutputStore::calculate_caret_rect(
                    rect,
                    border,
                    padding,
                    contents,
                    win_scale_factor,
                    scroll,
                    align_offset,
                );
                let c_color = contents
                    .caret_color
                    .or(rnd_base_visual.get(id).and_then(|v| v.text_color))
                    .or(visual.text_color)
                    .unwrap_or(Color::WHITE);

                let caret_instance = QuadInstance {
                    rect: caret_rect,
                    transform: packed_transform,
                    color: c_color,
                    opacity_mode_sizing: [visual.opacity.unwrap_or(1.0), -1.0, 0.0, 0.0],
                    ..Default::default()
                };

                render_data.instances.push(caret_instance);
                render_data.entity_ids.push(id);
            }
        }

        // 走査終了後、最後に残ったバッチをフラッシュ
        OutputStore::flush_batch(
            &mut render_data.batches,
            render_data.instances.len(),
            &mut last_flushed_offset,
            last_clip.unwrap_or_default(),
            current_batch_type,
        );
    }
}

impl Context {
    /// `現在の選択範囲（out_text_selections）に基づき`、
    /// `描画用の物理選択矩形（out_selected_rects）を自動再計算して` `SoA` キャッシュを更新します。
    #[inline]
    pub(crate) fn update_selection_rects(&mut self, id: EntityId, layout: &IDWriteTextLayout) {
        OutputStore::update_selection_rects(
            id,
            layout,
            &mut self.outputs.out_selected_rects,
            &self.outputs.out_text_selections,
        );
    }

    /// スクロールオフセットを目標位置へクランプした上で代入。
    /// オフセットに変化が生じた場合は true を返し、レイアウトのDirtyマークを打つ。
    pub(crate) fn scroll_to(&mut self, id: EntityId, mut x: f32, mut y: f32) -> bool {
        OutputStore::scroll_to(
            id,
            x,
            y,
            self.window.win_last_size,
            &mut self.topology.topo_active_masks,
            &self.topology.topo_parents,
            &mut self.layouts.lay_taffy,
            &mut self.layouts.lay_dirty_entities,
            &mut self.layouts.lay_scrollbar_styles,
            &self.layouts.lay_taffy_nodes,
            &self.layouts.lay_basic,
            &self.layouts.lay_flex,
            &self.layouts.lay_grid,
            &self.renders.rnd_visual,
            &self.renders.rnd_interaction,
            &self.renders.rnd_active_transitions,
            &self.outputs.out_rects,
            &mut self.outputs.out_scroll_offsets,
            &self.outputs.out_scroll_sizes,
        )
    }

    /// 階層的な境界判定ヘルパー（非対象のブランチをまるごとスキップ）
    pub(crate) fn hit_test_recursive(&self, id: EntityId, point: LayoutPoint) -> Option<EntityId> {
        OutputStore::hit_test_recursive(
            id,
            point,
            &self.topology.topo_children,
            &self.renders.rnd_visual,
            &self.renders.rnd_base_visual,
            &self.outputs.out_rects,
            &self.outputs.out_clip_rects,
        )
    }

    /// 現在のテキスト・IME状態・フォントサイズから、
    /// キャレットの物理座標や最終表示テキスト、レイアウト矩形を正確に再計算して `SoA` を更新。
    #[inline]
    pub(crate) fn update_input_caret_position(&mut self, id: EntityId) {
        OutputStore::update_input_caret_position(
            id,
            self.window.win_last_size,
            self.window.win_scale_factor,
            &self.system.sys_text_engine,
            &self.system.sys_dwrite_layouts,
            &mut self.contents.cont_input_contents,
            &mut self.contents.cont_text_contents,
            &self.contents.cont_text_spans,
            &mut self.topology.topo_active_masks,
            &self.topology.topo_parents,
            &mut self.layouts.lay_taffy,
            &mut self.layouts.lay_dirty_entities,
            &mut self.layouts.lay_scrollbar_styles,
            &self.layouts.lay_taffy_nodes,
            &self.layouts.lay_basic,
            &self.layouts.lay_flex,
            &self.layouts.lay_grid,
            &mut self.renders.rnd_visual,
            &self.renders.rnd_base_visual,
            &self.renders.rnd_interaction,
            &self.renders.rnd_active_transitions,
            &mut self.outputs.out_scroll_offsets,
            &mut self.outputs.out_text_selections,
            &self.outputs.out_rects,
            &self.outputs.out_scroll_sizes,
        );
    }

    /// 現在の全アクティブ要素から、wgpu 用の前面・背面描画バッチを生成します
    #[inline]
    pub(crate) fn collect_render_data(&mut self, render_data: &mut RenderData) {
        OutputStore::collect_render_data(
            render_data,
            self.window.win_scale_factor,
            &self.contents.cont_input_contents,
            &self.events.evt_interaction_states,
            &mut self.topology.topo_sorted_entities,
            &mut self.topology.topo_effective_transforms,
            &mut self.topology.topo_effective_z_indices,
            &mut self.topology.topo_dfs_indices,
            &mut self.topology.topo_sort_cache,
            &self.topology.topo_active_entities,
            &self.topology.topo_active_masks,
            &self.topology.topo_parents,
            &self.topology.topo_flat_dfs_sequence,
            &self.layouts.lay_basic,
            &self.layouts.lay_flex,
            &self.layouts.lay_grid,
            &self.renders.rnd_visual,
            &self.renders.rnd_base_visual,
            &self.renders.rnd_interaction,
            &self.renders.rnd_active_transitions,
            &self.renders.rnd_active_webviews,
            &self.outputs.out_rects,
            &self.outputs.out_clip_rects,
            &self.outputs.out_selected_rects,
            &self.outputs.out_scroll_offsets,
        );
    }
}
