use crate::{
    ActiveEntitiesVec, ActiveMasksSecondary, ActiveTransitionsSparseSecondary,
    ActiveWebviewsHashSet, AlignItems, BaseVisualPropertiesSecondary, BasicLayout,
    BasicLayoutsSecondary, BatchType, BoxSizing, CapacityConfig, ChildrenSecondary, Color,
    ComponentMask, ContentStore, Context, CornerRadius, DfsIndicesSecondary,
    DirtyLayoutEntitiesVec, DirtyRenderEntitiesVec, DrawBatch, DwriteLayoutsSparseSecondary,
    EdgeInsets, EffectiveZindicesSecondary, EntityId, EventStore, ExternalTextureAlphaMode,
    ExternalTextureSparseSecondary, FlatDfsSequenceVec, FlexLayout, FlexLayoutsSecondary,
    GridLayoutsSparseSecondary, IDENTITY_MATRIX, InputContents, InputContentsSparseSecondary,
    InteractionPropertiesSecondary, InteractionStates, LayoutPoint, LayoutRect, LayoutSize,
    LayoutStore, ParentsSecondary, PointerEvents, Position, PropertyList, QuadInstance,
    ReactiveStore, RenderData, RenderStore, RendererView, ResolvedBasicSecondary,
    ResolvedFlexSecondary, ResolvedGridSparseSecondary, ScrollbarStylesSecondary,
    SortedEntitiesVec, StrikethroughStyle, SystemStore, TaffyNodesSecondary, TaffyTreeEntityId,
    TextAlign, TextCacheKey, TextCacheValue, TextContentsSparseSecondary, TextEngine,
    TextRasterizer, TextSpan, TextSpansSparseSecondary, TextureAtlas, TopoSortCacheVec,
    TopologyStore, Transform, UnderlineStyle, UserSelect, Val, VisualPropertiesSecondary,
    VisualProperty, WindowStore, bind_context, with_context,
};
use rustc_hash::FxHashMap;
use slotmap::{SecondaryMap, SparseSecondaryMap};
use std::{
    borrow::Cow,
    cell::RefCell,
    collections::{HashMap, HashSet},
    ops::Range,
    time::Instant,
};
use windows::Win32::Graphics::DirectWrite::{DWRITE_HIT_TEST_METRICS, IDWriteTextLayout};

pub(crate) type RectsSecondary = SecondaryMap<EntityId, LayoutRect>;
pub(crate) type ClipRectsSecondary = SecondaryMap<EntityId, LayoutRect>;
pub(crate) type PrevRectsSecondary = SecondaryMap<EntityId, LayoutRect>;
pub(crate) type PrevClipRectsSecondary = SecondaryMap<EntityId, LayoutRect>;
pub(crate) type ScrollOffsetsSecondary = SecondaryMap<EntityId, LayoutPoint>;
pub(crate) type ScrollSizesSecondary = SecondaryMap<EntityId, LayoutSize>;
pub(crate) type SelectedRectsSparseSecondary = SparseSecondaryMap<EntityId, Vec<LayoutRect>>;
pub(crate) type TextSelectionsSparseSecondary = SparseSecondaryMap<EntityId, Range<usize>>;
pub(crate) type SelectionStartIndexSparseSecondary = SparseSecondaryMap<EntityId, usize>;

pub struct OutputStore {
    pub(crate) out_rects: RectsSecondary,
    pub(crate) out_clip_rects: ClipRectsSecondary,
    pub(crate) out_prev_rects: PrevRectsSecondary,
    pub(crate) out_prev_clip_rects: PrevClipRectsSecondary,
    pub(crate) out_scroll_offsets: ScrollOffsetsSecondary,
    pub(crate) out_scroll_sizes: ScrollSizesSecondary,
    pub(crate) out_text_selections: TextSelectionsSparseSecondary,
    pub(crate) out_selection_start_index: SelectionStartIndexSparseSecondary,
    pub(crate) out_selected_rects: SelectedRectsSparseSecondary,
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
            out_prev_rects: SecondaryMap::new(),
            out_prev_clip_rects: SecondaryMap::new(),
            out_scroll_offsets: SecondaryMap::new(),
            out_scroll_sizes: SecondaryMap::new(),
            out_text_selections: SparseSecondaryMap::new(),
            out_selection_start_index: SparseSecondaryMap::new(),
            out_selected_rects: SparseSecondaryMap::new(),
        }
    }

    #[inline]
    #[must_use]
    pub fn with_capacity(c: &CapacityConfig) -> Self {
        Self {
            out_rects: SecondaryMap::with_capacity(c.out_rects),
            out_clip_rects: SecondaryMap::with_capacity(c.out_clip_rects),
            out_prev_rects: SecondaryMap::with_capacity(c.out_prev_rects),
            out_prev_clip_rects: SecondaryMap::with_capacity(c.out_prev_clip_rects),
            out_scroll_offsets: SecondaryMap::with_capacity(c.out_scroll_offsets),
            out_scroll_sizes: SecondaryMap::with_capacity(c.out_scroll_sizes),
            out_text_selections: SparseSecondaryMap::with_capacity(c.out_text_selections),
            out_selected_rects: SparseSecondaryMap::with_capacity(c.out_selected_rects),
            out_selection_start_index: SparseSecondaryMap::with_capacity(
                c.out_selection_start_index,
            ),
        }
    }

    #[inline]
    pub fn clear(&mut self) {
        self.out_rects.clear();
        self.out_clip_rects.clear();
        self.out_prev_rects.clear();
        self.out_prev_clip_rects.clear();
        self.out_scroll_offsets.clear();
        self.out_scroll_sizes.clear();
        self.out_text_selections.clear();
        self.out_selection_start_index.clear();
        self.out_selected_rects.clear();
    }

    #[inline]
    pub fn despawn(&mut self, id: EntityId) {
        self.out_rects.remove(id);
        self.out_clip_rects.remove(id);
        self.out_prev_rects.remove(id);
        self.out_prev_clip_rects.remove(id);
        self.out_scroll_offsets.remove(id);
        self.out_scroll_sizes.remove(id);
        self.out_text_selections.remove(id);
        self.out_selection_start_index.remove(id);
        self.out_selected_rects.remove(id);
    }
}

impl OutputStore {
    pub(crate) fn swap_output_rect(
        out_rects: &mut RectsSecondary,
        out_clip_rects: &mut ClipRectsSecondary,
        out_prev_rects: &mut PrevRectsSecondary,
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
        out_clip_rects: &ClipRectsSecondary,
        out_prev_rects: &PrevRectsSecondary,
        out_prev_clip_rects: &PrevClipRectsSecondary,
    ) -> bool {
        let Some(parent_id) = topo_parents.get(id).copied().flatten() else {
            return false;
        };

        out_prev_rects.get(parent_id) != out_rects.get(parent_id)
            || out_prev_clip_rects.get(parent_id) != out_clip_rects.get(parent_id)
            || topo_active_masks
                .get(parent_id)
                .is_some_and(|a| a.has(ComponentMask::STATE_QUEUED_LAYOUT))
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
        out_scroll_offsets: &ScrollOffsetsSecondary,
    ) -> (LayoutRect, LayoutRect) {
        let initial_clip = LayoutRect::new(0.0, 0.0, window_size.width, window_size.height);
        let local_rect = LayoutStore::local_rect_from_taffy(id, lay_taffy_tree, lay_taffy_nodes);

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
        // フォーカスされている要素を最優先とし、
        // 無い場合は現在有効な空ではない選択範囲を持つ最初の要素を逆引き
        let target_id = evt_interaction_states.focused.or_else(|| {
            out_text_selections
                .iter()
                .find(|(_, range)| range.start < range.end)
                .map(|(id, _)| id)
        })?;

        let user_select = rnd_visual
            .get(target_id)
            .and_then(|v| v.user_select)
            .unwrap_or_default();

        if user_select == UserSelect::Text {
            let range = out_text_selections.get(target_id)?;
            if range.start < range.end {
                let text = cont_text_contents.get(target_id)?;
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
        cont_text_contents: &TextContentsSparseSecondary,
        cont_text_spans: &TextSpansSparseSecondary,
        cont_input_contents: &InputContentsSparseSecondary,
        topo_active_masks: &ActiveMasksSecondary,
        topo_parents: &ParentsSecondary,
        topo_children: &ChildrenSecondary,
        lay_resolved_basic: &ResolvedBasicSecondary,
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
                lay_resolved_basic,
                rnd_visual,
                out_rects,
            )
        {
            let size = sys_text_engine.get_layout_size(&dw_layout);
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
                let is_absolute = lay_resolved_basic
                    .get(child_id)
                    .is_some_and(|l| l.position == Position::Absolute);
                if is_absolute {
                    continue;
                }

                if let Some(&rect) = out_rects.get(child_id) {
                    let parent_rect = out_rects.get(id).copied().unwrap_or_default();
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
        sys_dwrite_layouts: &DwriteLayoutsSparseSecondary,
        cont_text_contents: &mut TextContentsSparseSecondary,
        cont_input_contents: &mut InputContentsSparseSecondary,
        cont_text_spans: &TextSpansSparseSecondary,
        lay_resolved_basic: &ResolvedBasicSecondary,
        rnd_visual: &mut VisualPropertiesSecondary,
        rnd_base_visual: &BaseVisualPropertiesSecondary,
        out_text_selections: &mut TextSelectionsSparseSecondary,
        out_rects: &RectsSecondary,
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
            InputContents::input_get_display_text(
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

        cont_text_contents.insert(id, display_text.clone().into());

        let display_layout = SystemStore::get_or_create_layout(
            id,
            sys_text_engine,
            sys_dwrite_layouts,
            cont_text_contents,
            cont_text_spans,
            lay_resolved_basic,
            rnd_visual,
            out_rects,
        )?;

        let text_size = sys_text_engine.get_layout_size(&display_layout);
        contents.last_layout = Some(LayoutRect::new(0.0, 0.0, text_size.width, text_size.height));

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
        let u16_len_display = display_text.encode_utf16().count();

        // プレースホルダーに干渉されない純粋なキャレット位置を算出
        let (cx_offset, cy_offset, ch_height) =
            sys_text_engine.get_caret_position(&display_layout, caret_index, u16_len_display);

        contents.measured_caret_x = cx_offset;
        contents.measured_caret_y = cy_offset;
        contents.caret_line_height = ch_height;

        let (curr_line, tot_lines) =
            InputContents::calculate_line_indices(&display_text, caret_index);
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
            i.unset(ComponentMask::STYLE_TEXT_SPANS);
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
        align_items: Option<AlignItems>,
        is_multiline: bool,
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

        // 複数行入力時は標準で上端揃え、単一行は標準で中央揃えにフォールバック
        let align_items_resolved = align_items.unwrap_or(if is_multiline {
            AlignItems::Start
        } else {
            AlignItems::Center
        });

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
        lay_scrollbar_styles: &mut ScrollbarStylesSecondary,
        lay_taffy_nodes: &TaffyNodesSecondary,
        lay_resolved_basic: &ResolvedBasicSecondary,
        rnd_visual: &VisualPropertiesSecondary,
        rnd_interaction: &InteractionPropertiesSecondary,
        rnd_active_transitions: &ActiveTransitionsSparseSecondary,
        out_scroll_offsets: &mut ScrollOffsetsSecondary,
        out_rects: &RectsSecondary,
        out_scroll_sizes: &ScrollSizesSecondary,
    ) -> bool {
        let Some(rect) = out_rects.get(id).copied() else {
            return false;
        };

        let scroll_size = out_scroll_sizes.get(id).copied().unwrap_or_default();

        // 親コンテナのボーダーおよびパディング厚を取得
        let basic = lay_resolved_basic.get(id).copied().unwrap_or_default();
        let (border, padding) =
            LayoutStore::get_physical_border_padding(rect, basic.border, basic.padding);

        let visible_size = WindowStore::calculate_visible_size(rect, win_last_size);
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
                lay_dirty_entities,
                lay_taffy_tree,
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
        lay_dirty_entities: &mut DirtyLayoutEntitiesVec,
        lay_taffy_tree: &mut TaffyTreeEntityId,
        lay_scrollbar_styles: &mut ScrollbarStylesSecondary,
        lay_taffy_nodes: &TaffyNodesSecondary,
        lay_resolved_basic: &ResolvedBasicSecondary,
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
            lay_dirty_entities,
            lay_taffy_tree,
            lay_scrollbar_styles,
            lay_taffy_nodes,
            lay_resolved_basic,
            rnd_visual,
            rnd_interaction,
            rnd_active_transitions,
            out_scroll_offsets,
            out_rects,
            out_scroll_sizes,
        )
    }

    /// 現在のテキスト・IME状態・フォントサイズから、
    /// キャレットの物理座標や最終表示テキスト、レイアウト矩形を正確に再計算して `SoA` を更新。
    pub(crate) fn update_input_caret_position(
        id: EntityId,
        win_scale_factor: f32,
        win_last_size: Option<LayoutSize>,
        sys_text_engine: &TextEngine,
        sys_dwrite_layouts: &DwriteLayoutsSparseSecondary,
        cont_text_contents: &mut TextContentsSparseSecondary,
        cont_input_contents: &mut InputContentsSparseSecondary,
        cont_text_spans: &TextSpansSparseSecondary,
        topo_active_masks: &mut ActiveMasksSecondary,
        topo_parents: &ParentsSecondary,
        lay_dirty_entities: &mut DirtyLayoutEntitiesVec,
        lay_taffy_tree: &mut TaffyTreeEntityId,
        lay_scrollbar_styles: &mut ScrollbarStylesSecondary,
        lay_taffy_nodes: &TaffyNodesSecondary,
        lay_resolved_basic: &ResolvedBasicSecondary,
        lay_resolved_flex: &ResolvedFlexSecondary,
        lay_resolved_grid: &ResolvedGridSparseSecondary,
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
            sys_dwrite_layouts,
            cont_text_contents,
            cont_input_contents,
            cont_text_spans,
            lay_resolved_basic,
            rnd_visual,
            rnd_base_visual,
            out_text_selections,
            out_rects,
        );

        let Some((caret, caret_offset, is_multiline)) = scroll_ime_info else {
            return;
        };
        let basic = lay_resolved_basic.get(id).copied().unwrap_or_default();
        let flex = lay_resolved_flex.get(id).copied().unwrap_or_default();
        let _grid = lay_resolved_grid.get(id).cloned().unwrap_or_default();
        let rect = out_rects.get(id).copied().unwrap_or_default();
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

            let align_offset = OutputStore::calc_align_offset(
                rect,
                border,
                padding,
                text_size,
                flex.text_align,
                flex.align_items,
                is_multiline,
            );

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
                lay_dirty_entities,
                lay_taffy_tree,
                lay_scrollbar_styles,
                lay_taffy_nodes,
                lay_resolved_basic,
                rnd_visual,
                rnd_interaction,
                rnd_active_transitions,
                out_scroll_offsets,
                out_rects,
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

    #[inline]
    pub(crate) fn drag_overhang_distance(
        pointer_pos: LayoutPoint,
        clip: &LayoutRect,
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

    pub(crate) fn autoscroll_occurred(
        id: EntityId,
        win_last_size: Option<LayoutSize>,
        sys_text_engine: &TextEngine,
        sys_dwrite_layouts: &DwriteLayoutsSparseSecondary,
        evt_current_pointer_position: Option<LayoutPoint>,
        cont_text_contents: &TextContentsSparseSecondary,
        cont_text_spans: &TextSpansSparseSecondary,
        cont_input_contents: &InputContentsSparseSecondary,
        topo_active_masks: &mut ActiveMasksSecondary,
        topo_parents: &ParentsSecondary,
        topo_children: &ChildrenSecondary,
        lay_dirty_entities: &mut DirtyLayoutEntitiesVec,
        lay_taffy_tree: &mut TaffyTreeEntityId,
        lay_scrollbar_styles: &mut ScrollbarStylesSecondary,
        lay_taffy_nodes: &TaffyNodesSecondary,
        lay_resolved_basic: &ResolvedBasicSecondary,
        rnd_visual: &VisualPropertiesSecondary,
        rnd_interaction: &InteractionPropertiesSecondary,
        rnd_active_transitions: &ActiveTransitionsSparseSecondary,
        out_scroll_offsets: &mut ScrollOffsetsSecondary,
        out_rects: &RectsSecondary,
        out_clip_rects: &ClipRectsSecondary,
        out_scroll_sizes: &ScrollSizesSecondary,
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

        let scroll = OutputStore::scroll_by(
            id,
            dx,
            dy,
            win_last_size,
            topo_active_masks,
            topo_parents,
            lay_dirty_entities,
            lay_taffy_tree,
            lay_scrollbar_styles,
            lay_taffy_nodes,
            lay_resolved_basic,
            rnd_visual,
            rnd_interaction,
            rnd_active_transitions,
            out_scroll_offsets,
            out_rects,
            out_scroll_sizes,
        );

        if scroll {
            (true, Some(pointer_pos))
        } else {
            (false, None)
        }
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
        lay_dirty_entities: &mut DirtyLayoutEntitiesVec,
        lay_taffy_tree: &mut TaffyTreeEntityId,
        lay_scrollbar_styles: &mut ScrollbarStylesSecondary,
        lay_taffy_nodes: &TaffyNodesSecondary,
        lay_resolved_basic: &ResolvedBasicSecondary,
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
            let container_rect = out_rects.get(current_id).copied().unwrap_or_default();
            let scroll_size = out_scroll_sizes
                .get(current_id)
                .copied()
                .unwrap_or_default();
            (sb_state, container_rect, scroll_size)
        };

        let visible_size = WindowStore::calculate_visible_size(container_rect, win_last_size);

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
                lay_dirty_entities,
                lay_taffy_tree,
                lay_scrollbar_styles,
                lay_taffy_nodes,
                lay_resolved_basic,
                rnd_visual,
                rnd_interaction,
                rnd_active_transitions,
                out_scroll_offsets,
                out_rects,
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
        if let Some(clip) = out_clip_rects.get(id).copied()
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
    pub(crate) fn sync_dirty_styles_to_taffy(
        scrollbar_el_ids: &HashSet<EntityId>,
        lay_taffy_tree: &mut TaffyTreeEntityId,
        lay_dirty_entities: &DirtyLayoutEntitiesVec,
        lay_taffy_nodes: &TaffyNodesSecondary,
        lay_resolved_basic: &ResolvedBasicSecondary,
        lay_resolved_flex: &ResolvedFlexSecondary,
        lay_resolved_grid: &ResolvedGridSparseSecondary,
        lay_scrollbar_styles: &ScrollbarStylesSecondary,
    ) {
        for &id in lay_dirty_entities {
            if scrollbar_el_ids.contains(&id) {
                continue;
            }

            let basic = lay_resolved_basic.get(id).copied().unwrap_or_default();
            let flex = lay_resolved_flex.get(id).copied().unwrap_or_default();
            let grid = lay_resolved_grid.get(id).cloned();

            // トランジション（アニメーション）中プロパティの現在値による上書き
            // 削除：resolve_active_layouts の段階でアニメーション中のサイズが正しく反映されたレイアウト）が返ってくるため
            // if let Some(active_list) = rnd_active_transitions.get(id) {}

            let taffy_style = LayoutStore::resolve_taffy_style(
                id,
                &basic,
                &flex,
                grid.as_ref(),
                lay_scrollbar_styles,
            );

            if let Some(taffy_node) = lay_taffy_nodes.get(id) {
                lay_taffy_tree.set_style(*taffy_node, taffy_style).unwrap();
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
        lay_taffy_tree: &TaffyTreeEntityId,
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
            lay_taffy_tree,
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

        let current_clip = if mask.has(ComponentMask::STYLE_OVERFLOW) {
            parent_clip.intersect(&abs_rect)
        } else {
            parent_clip
        };

        out_clip_rects.insert(id, current_clip);
        topo_active_entities.push(id);
    }

    /// 1回目の出力領域決定（静的キャッシュバイパス判定含む）
    pub(crate) fn resolve_first_pass_rects(
        scrollbar_el_ids: &HashSet<EntityId>,
        window_size: LayoutSize,
        window_resized: bool,
        cont_input_contents: &mut InputContentsSparseSecondary,
        topo_active_entities: &mut ActiveEntitiesVec,
        topo_active_masks: &ActiveMasksSecondary,
        topo_parents: &ParentsSecondary,
        topo_flat_dfs_sequence: &FlatDfsSequenceVec,
        lay_taffy_tree: &TaffyTreeEntityId,
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
                out_clip_rects,
                out_prev_rects,
                out_prev_clip_rects,
            );

            let has_style_changed = topo_active_masks
                .get(id)
                .is_some_and(|m| m.has(ComponentMask::STATE_QUEUED_LAYOUT));

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
                lay_taffy_tree,
                lay_taffy_nodes,
                lay_basic,
                out_rects,
                out_clip_rects,
                out_scroll_offsets,
            );
        }
    }

    /// 最終的な出力領域決定（スクロールバー要素を含む一括同期）
    pub(crate) fn resolve_final_pass_rects(
        window_size: LayoutSize,
        cont_input_contents: &mut InputContentsSparseSecondary,
        topo_active_entities: &mut ActiveEntitiesVec,
        topo_active_masks: &ActiveMasksSecondary,
        topo_parents: &ParentsSecondary,
        topo_flat_dfs_sequence: &FlatDfsSequenceVec,
        lay_taffy_tree: &TaffyTreeEntityId,
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
                lay_taffy_tree,
                lay_taffy_nodes,
                lay_basic,
                out_rects,
                out_clip_rects,
                out_scroll_offsets,
            );
        }
    }

    /// 全アクティブコンテナのスクロールオフセットの自動クランプ同期
    pub(crate) fn auto_clamp_scroll_offsets(
        win_last_size: Option<LayoutSize>,
        topo_active_masks: &mut ActiveMasksSecondary,
        topo_parents: &ParentsSecondary,
        topo_flat_dfs_sequence: &FlatDfsSequenceVec,
        lay_dirty_entities: &mut DirtyLayoutEntitiesVec,
        lay_taffy_tree: &mut TaffyTreeEntityId,
        lay_scrollbar_styles: &mut ScrollbarStylesSecondary,
        lay_taffy_nodes: &TaffyNodesSecondary,
        lay_resolved_basic: &ResolvedBasicSecondary,
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
                lay_dirty_entities,
                lay_taffy_tree,
                lay_scrollbar_styles,
                lay_taffy_nodes,
                lay_resolved_basic,
                rnd_visual,
                rnd_interaction,
                rnd_active_transitions,
                out_scroll_offsets,
                out_rects,
                out_scroll_sizes,
            );
        }
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
            &mut self.layouts.lay_dirty_entities,
            &mut self.layouts.lay_taffy_tree,
            &mut self.layouts.lay_scrollbar_styles,
            &self.layouts.lay_taffy_nodes,
            &self.layouts.lay_resolved_basic,
            &self.renders.rnd_visual,
            &self.renders.rnd_interaction,
            &self.renders.rnd_active_transitions,
            &mut self.outputs.out_scroll_offsets,
            &self.outputs.out_rects,
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
}
