use std::{ops::Range, time::Instant};

use crate::{
    ActiveEntitiesVec, ActiveMasksSecondary, ActiveTransitionsSparseSecondary,
    ActiveWebviewsHashSet, BaseVisualPropertiesSecondary, BasicLayoutsSecondary, BatchType,
    BoxSizing, ChildrenSecondary, Color, ComponentMask, ContentStore, Context, CornerRadius,
    DirtyLayoutEntitiesVec, DirtyRenderEntitiesVec, DrawBatch, DwriteLayoutsSparseSecondary,
    EdgeInsets, EffectiveTransformsSecondary, EffectiveZindicesSecondary, EntityId, EventStore,
    FlatDfsSequenceVec, FlexLayoutsSecondary, GridLayoutsSecondary, InputContents,
    InputContentsSparseSecondary, InteractionPropertiesSecondary, InteractionStates, LayoutPoint,
    LayoutRect, LayoutSize, LayoutStore, ParentsSecondary, PointerEvents, Position, QuadInstance,
    RenderData, RenderStore, STATE_QUEUED_LAYOUT, STYLE_TEXT_SPANS, ScrollbarStylesSecondary,
    SortedEntitiesVec, SystemStore, TaffyNodesSecondary, TaffyTreeEntityId, TextAlign,
    TextContentsSparseSecondary, TextEngine, TextSpansSparseSecondary, TopologyStore, UserSelect,
    Val, VisualPropertiesSecondary, VisualProperty, WindowStore,
};
use slotmap::{SecondaryMap, SparseSecondaryMap};
use windows::Win32::Graphics::DirectWrite::{DWRITE_HIT_TEST_METRICS, IDWriteTextLayout};

pub(crate) type RectsSecondary = SecondaryMap<EntityId, LayoutRect>;
pub(crate) type ClipRectsSecondary = SecondaryMap<EntityId, LayoutRect>;
pub(crate) type ScrollOffsetsSecondary = SecondaryMap<EntityId, LayoutPoint>;
pub(crate) type PrevRectsSecondary = SecondaryMap<EntityId, LayoutRect>;
pub(crate) type PrevClipRectsSecondary = SecondaryMap<EntityId, LayoutRect>;
pub(crate) type SelectedRectsSparseSecondary = SparseSecondaryMap<EntityId, Vec<LayoutRect>>;
pub(crate) type TextSelectionsSparseSecondary = SparseSecondaryMap<EntityId, Range<usize>>;
pub(crate) type SelectionStartIndexSparseSecondary = SparseSecondaryMap<EntityId, usize>;

#[allow(clippy::struct_field_names)]
pub struct OutputStore {
    pub(crate) out_rects: RectsSecondary,
    pub(crate) out_clip_rects: ClipRectsSecondary,
    pub(crate) out_scroll_offsets: ScrollOffsetsSecondary,
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
        topo_parents: &ParentsSecondary,
        topo_active_masks: &ActiveMasksSecondary,
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

    #[allow(clippy::too_many_arguments)]
    pub(crate) fn calc_local_rect(
        id: EntityId,
        lay_taffy_nodes: &TaffyNodesSecondary,
        lay_taffy: &TaffyTreeEntityId,
        topo_parents: &ParentsSecondary,
        out_rects: &RectsSecondary,
        out_clip_rects: &ClipRectsSecondary,
        lay_basic: &BasicLayoutsSecondary,
        out_scroll_offsets: &ScrollOffsetsSecondary,
        window_size: LayoutSize,
    ) -> (LayoutRect, LayoutRect) {
        let initial_clip = LayoutRect::new(0.0, 0.0, window_size.width, window_size.height);
        let local_rect = LayoutStore::local_rect_from_taffy(id, lay_taffy_nodes, lay_taffy);

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
        topo_parents: &ParentsSecondary,
        out_rects: &RectsSecondary,
        win_last_size: Option<&LayoutSize>,
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
        out_clip_rects: &ClipRectsSecondary,
        ren_visual: &VisualPropertiesSecondary,
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

        let user_select = ren_visual
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
    #[must_use]
    pub fn get_selected_text(
        events: &EventStore,
        renders: &RenderStore,
        outputs: &OutputStore,
        contents: &ContentStore,
    ) -> Option<String> {
        let focused_id = events.evt_interaction_states.focused?;
        let user_select = renders
            .ren_visual
            .get(focused_id)
            .and_then(|v| v.user_select)
            .unwrap_or_default();

        if user_select == UserSelect::Text {
            let range = outputs.out_text_selections.get(focused_id)?;
            if range.start < range.end {
                let text = contents.cont_text_contents.get(focused_id)?;
                let u16_text: Vec<u16> = text.encode_utf16().collect();
                let slice =
                    &u16_text[range.start.min(u16_text.len())..range.end.min(u16_text.len())];
                return String::from_utf16(slice).ok();
            }
        }
        None
    }

    pub(crate) fn inject_paste_internal(
        focused_id: EntityId,
        text: &str,
        outputs: &mut OutputStore,
        contents: &mut InputContents,
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
        outputs
            .out_text_selections
            .insert(focused_id, new_caret..new_caret);
        outputs.out_selected_rects.remove(focused_id);
        contents.text.1.set(new_text);
    }

    pub(crate) fn inject_undo_internal(
        focused_id: EntityId,
        prev_sel: Range<usize>,
        prev_text: String,
        outputs: &mut OutputStore,
        contents: &mut InputContents,
    ) {
        let current_text = contents.text.0.get();
        let current_sel = contents.selected_range.clone();
        contents.redo_stack.push((current_text, current_sel)); // 現在の状態を Redo 用にセーブ

        contents.selected_range = prev_sel.clone();
        outputs.out_text_selections.insert(focused_id, prev_sel);
        outputs.out_selected_rects.remove(focused_id);
        contents.text.1.set(prev_text);
    }

    pub(crate) fn inject_redo_internal(
        focused_id: EntityId,
        next_sel: Range<usize>,
        next_text: String,
        outputs: &mut OutputStore,
        contents: &mut InputContents,
    ) {
        let current_text = contents.text.0.get();
        let current_sel = contents.selected_range.clone();
        contents.undo_stack.push((current_text, current_sel)); // 現在の状態を Undo 用に退避

        contents.selected_range = next_sel.clone();
        outputs.out_text_selections.insert(focused_id, next_sel);
        outputs.out_selected_rects.remove(focused_id);
        contents.text.1.set(next_text);
    }

    pub(crate) fn inject_cut_internal(
        focused_id: EntityId,
        range: Range<usize>,
        outputs: &mut OutputStore,
        contents: &mut InputContents,
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
        outputs
            .out_text_selections
            .insert(focused_id, range.start..range.start);
        outputs.out_selected_rects.remove(focused_id);
        contents.text.1.set(new_text);
    }

    #[inline]
    pub(crate) fn truncate_unconfirmed_text(
        text_val: &str,
        cont_input_contents: &InputContents,
        max: usize,
        filtered_comp_text: String,
    ) -> String {
        let text_u16: Vec<u16> = text_val.encode_utf16().collect();
        let range = &cont_input_contents.selected_range;
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
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn get_scroll_size(
        id: EntityId,
        topo_active_masks: &ActiveMasksSecondary,
        cont_input_contents: &InputContentsSparseSecondary,
        sys_text_engine: &TextEngine,
        cont_text_contents: &TextContentsSparseSecondary,
        ren_visual: &VisualPropertiesSecondary,
        cont_text_spans: &TextSpansSparseSecondary,
        sys_dwrite_layouts: &DwriteLayoutsSparseSecondary,
        lay_basic: &BasicLayoutsSecondary,
        lay_flex: &FlexLayoutsSecondary,
        lay_grid: &GridLayoutsSecondary,
        ren_active_transitions: &ActiveTransitionsSparseSecondary,
        topo_parents: &ParentsSecondary,
        topo_children: &ChildrenSecondary,
        ren_interaction: &InteractionPropertiesSecondary,
        out_rects: &RectsSecondary,
        lay_scrollbar_styles: &ScrollbarStylesSecondary,
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
                cont_text_contents,
                ren_visual,
                sys_dwrite_layouts,
                cont_text_spans,
                sys_text_engine,
            )
        {
            let size = sys_text_engine.get_layout_size(&dw_layout);
            max_x = size.width;
            max_y = size.height;
        }

        // 親要素自体のボーダー・パディング厚を取得
        let (basic, _, _) = LayoutStore::resolve_active_layouts(
            id,
            lay_basic,
            lay_flex,
            lay_grid,
            topo_active_masks,
            ren_active_transitions,
            topo_parents,
            ren_interaction,
            ren_visual,
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

    #[allow(clippy::too_many_lines, clippy::too_many_arguments)]
    #[inline]
    pub(crate) fn scroll_ime_info(
        id: EntityId,
        cont_input_contents: &mut InputContentsSparseSecondary,
        cont_text_contents: &mut TextContentsSparseSecondary,
        out_text_selections: &mut TextSelectionsSparseSecondary,
        cont_text_spans: &TextSpansSparseSecondary,
        sys_text_engine: &TextEngine,
        ren_visual: &mut VisualPropertiesSecondary,
        ren_base_visual: &BaseVisualPropertiesSecondary,
    ) -> Option<(LayoutRect, f32, bool)> {
        // (caret_x, caret_y, caret_h, caret_w, caret_offset, is_multiline)
        let mut scroll_ime_info: Option<(LayoutRect, f32, bool)> = None;

        if let Some(contents) = cont_input_contents.get_mut(id) {
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
                RenderStore::get_font_propery(id, ren_visual);

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
            contents.last_layout =
                Some(LayoutRect::new(0.0, 0.0, text_size.width, text_size.height));

            // キャレット位置測定用のレイアウトをプレースホルダー抜きで作成
            let caret_layout = sys_text_engine.create_layout(
                &caret_text,
                font_size,
                font_family,
                font_weight,
                font_style,
                None,
                spans,
            );

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
                sys_text_engine.get_caret_position(&caret_layout, caret_index, u16_len_caret);

            contents.measured_caret_x = cx_offset;
            contents.measured_caret_y = cy_offset;
            contents.caret_line_height = ch_height;

            let (curr_line, tot_lines) = crate::calculate_line_indices(&display_text, caret_index);
            contents.current_line_index = curr_line;
            contents.total_lines = tot_lines;

            // 最終表示用テキストを Context 側に反映
            cont_text_contents.insert(id, display_text.into());

            if let Some(visual) = ren_visual.get_mut(id) {
                let is_ime_active = contents
                    .ime_state
                    .as_ref()
                    .is_some_and(|ime| !ime.composition_text.is_empty());

                if text_val.is_empty() && !is_ime_active {
                    // 確定文字列が空で、かつ未確定文字列も存在しない状態のみグレー表示
                    visual.text_color = contents.placeholder_color;
                } else {
                    let base_color = ren_base_visual
                        .get(id)
                        .and_then(|v| v.text_color)
                        .unwrap_or(Color::WHITE);
                    visual.text_color = Some(base_color);
                }
            }

            scroll_ime_info = Some((
                LayoutRect {
                    x: cx_offset,
                    y: cy_offset,
                    width: contents.caret_width.unwrap_or(contents.default_caret_width),
                    height: ch_height,
                },
                contents.caret_offset,
                contents.is_multiline,
            ));
        }
        scroll_ime_info
    }

    #[inline]
    pub(crate) fn clear_selection_highlight_rect(
        id: EntityId,
        topo_active_masks: &mut ActiveMasksSecondary,
        out_text_selections: &mut TextSelectionsSparseSecondary,
        out_selected_rects: &mut SelectedRectsSparseSecondary,
        cont_input_contents: &mut InputContentsSparseSecondary,
        cont_text_spans: &mut TextSpansSparseSecondary,
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
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn scroll_to(
        id: EntityId,
        mut x: f32,
        mut y: f32,
        topo_active_masks: &mut ActiveMasksSecondary,
        cont_input_contents: &InputContentsSparseSecondary,
        sys_text_engine: &TextEngine,
        cont_text_contents: &TextContentsSparseSecondary,
        ren_visual: &VisualPropertiesSecondary,
        cont_text_spans: &TextSpansSparseSecondary,
        sys_dwrite_layouts: &DwriteLayoutsSparseSecondary,
        lay_basic: &BasicLayoutsSecondary,
        lay_flex: &FlexLayoutsSecondary,
        lay_grid: &GridLayoutsSecondary,
        ren_active_transitions: &ActiveTransitionsSparseSecondary,
        topo_parents: &ParentsSecondary,
        topo_children: &ChildrenSecondary,
        ren_interaction: &InteractionPropertiesSecondary,
        out_rects: &RectsSecondary,
        lay_scrollbar_styles: &mut ScrollbarStylesSecondary,
        out_scroll_offsets: &mut ScrollOffsetsSecondary,
        win_last_size: Option<LayoutSize>,
        lay_taffy_nodes: &TaffyNodesSecondary,
        lay_taffy: &mut TaffyTreeEntityId,
        lay_dirty_entities: &mut DirtyLayoutEntitiesVec,
    ) -> bool {
        let Some(rect) = OutputStore::rect(id, out_rects) else {
            return false;
        };

        let scroll_size = OutputStore::get_scroll_size(
            id,
            topo_active_masks,
            cont_input_contents,
            sys_text_engine,
            cont_text_contents,
            ren_visual,
            cont_text_spans,
            sys_dwrite_layouts,
            lay_basic,
            lay_flex,
            lay_grid,
            ren_active_transitions,
            topo_parents,
            topo_children,
            ren_interaction,
            out_rects,
            lay_scrollbar_styles,
            out_scroll_offsets,
        );

        // 親コンテナのボーダーおよびパディング厚を取得
        let (basic, _, _) = LayoutStore::resolve_active_layouts(
            id,
            lay_basic,
            lay_flex,
            lay_grid,
            topo_active_masks,
            ren_active_transitions,
            topo_parents,
            ren_interaction,
            ren_visual,
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
                lay_taffy_nodes,
                lay_taffy,
                topo_active_masks,
                lay_dirty_entities,
                topo_parents,
            );
            true
        } else {
            false
        }
    }

    #[allow(clippy::too_many_arguments)]
    pub(crate) fn scroll_by(
        id: EntityId,
        dx: f32,
        dy: f32,
        topo_active_masks: &mut ActiveMasksSecondary,
        cont_input_contents: &InputContentsSparseSecondary,
        sys_text_engine: &TextEngine,
        cont_text_contents: &TextContentsSparseSecondary,
        ren_visual: &VisualPropertiesSecondary,
        cont_text_spans: &TextSpansSparseSecondary,
        sys_dwrite_layouts: &DwriteLayoutsSparseSecondary,
        lay_basic: &BasicLayoutsSecondary,
        lay_flex: &FlexLayoutsSecondary,
        lay_grid: &GridLayoutsSecondary,
        ren_active_transitions: &ActiveTransitionsSparseSecondary,
        topo_parents: &ParentsSecondary,
        topo_children: &ChildrenSecondary,
        ren_interaction: &InteractionPropertiesSecondary,
        out_rects: &RectsSecondary,
        lay_scrollbar_styles: &mut ScrollbarStylesSecondary,
        out_scroll_offsets: &mut ScrollOffsetsSecondary,
        win_last_size: Option<LayoutSize>,
        lay_taffy_nodes: &TaffyNodesSecondary,
        lay_taffy: &mut TaffyTreeEntityId,
        lay_dirty_entities: &mut DirtyLayoutEntitiesVec,
    ) -> bool {
        let current = out_scroll_offsets.get(id).copied().unwrap_or_default();

        OutputStore::scroll_to(
            id,
            current.x + dx,
            current.y + dy,
            topo_active_masks,
            cont_input_contents,
            sys_text_engine,
            cont_text_contents,
            ren_visual,
            cont_text_spans,
            sys_dwrite_layouts,
            lay_basic,
            lay_flex,
            lay_grid,
            ren_active_transitions,
            topo_parents,
            topo_children,
            ren_interaction,
            out_rects,
            lay_scrollbar_styles,
            out_scroll_offsets,
            win_last_size,
            lay_taffy_nodes,
            lay_taffy,
            lay_dirty_entities,
        )
    }

    /// 現在のテキスト・IME状態・フォントサイズから、
    /// キャレットの物理座標や最終表示テキスト、レイアウト矩形を正確に再計算して `SoA` を更新。
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn update_input_caret_position(
        id: EntityId,
        out_rects: &RectsSecondary,
        sys_dwrite_layouts: &DwriteLayoutsSparseSecondary,
        cont_input_contents: &mut InputContentsSparseSecondary,
        cont_text_contents: &mut TextContentsSparseSecondary,
        out_text_selections: &mut TextSelectionsSparseSecondary,
        cont_text_spans: &TextSpansSparseSecondary,
        sys_text_engine: &TextEngine,
        lay_basic: &BasicLayoutsSecondary,
        lay_flex: &FlexLayoutsSecondary,
        lay_grid: &GridLayoutsSecondary,
        topo_active_masks: &mut ActiveMasksSecondary,
        topo_children: &ChildrenSecondary,
        ren_interaction: &InteractionPropertiesSecondary,
        lay_scrollbar_styles: &mut ScrollbarStylesSecondary,
        out_scroll_offsets: &mut ScrollOffsetsSecondary,
        win_last_size: Option<LayoutSize>,
        lay_taffy_nodes: &TaffyNodesSecondary,
        lay_taffy: &mut TaffyTreeEntityId,
        lay_dirty_entities: &mut DirtyLayoutEntitiesVec,
        ren_active_transitions: &ActiveTransitionsSparseSecondary,
        topo_parents: &ParentsSecondary,
        ren_visual: &mut VisualPropertiesSecondary,
        ren_base_visual: &BaseVisualPropertiesSecondary,
        win_scale_factor: f32,
    ) {
        // IMEやタイピング中の古いキャッシュを破棄
        SystemStore::clear_layout_cache(id, sys_dwrite_layouts);

        let scroll_ime_info = OutputStore::scroll_ime_info(
            id,
            cont_input_contents,
            cont_text_contents,
            out_text_selections,
            cont_text_spans,
            sys_text_engine,
            ren_visual,
            ren_base_visual,
        );

        let Some((caret, caret_offset, is_multiline)) = scroll_ime_info else {
            return;
        };
        let (basic, flex, _) = LayoutStore::resolve_active_layouts(
            id,
            lay_basic,
            lay_flex,
            lay_grid,
            topo_active_masks,
            ren_active_transitions,
            topo_parents,
            ren_interaction,
            ren_visual,
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
                topo_active_masks,
                cont_input_contents,
                sys_text_engine,
                cont_text_contents,
                ren_visual,
                cont_text_spans,
                sys_dwrite_layouts,
                lay_basic,
                lay_flex,
                lay_grid,
                ren_active_transitions,
                topo_parents,
                topo_children,
                ren_interaction,
                out_rects,
                lay_scrollbar_styles,
                out_scroll_offsets,
                win_last_size,
                lay_taffy_nodes,
                lay_taffy,
                lay_dirty_entities,
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
        out_text_selections: &TextSelectionsSparseSecondary,
        out_selected_rects: &mut SelectedRectsSparseSecondary,
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

    #[allow(clippy::too_many_arguments, clippy::too_many_lines)]
    pub(crate) fn sync_scrollbar_drag(
        logical_pos: LayoutPoint,
        topo_active_masks: &mut ActiveMasksSecondary,
        cont_input_contents: &InputContentsSparseSecondary,
        sys_text_engine: &TextEngine,
        cont_text_contents: &TextContentsSparseSecondary,
        ren_visual: &VisualPropertiesSecondary,
        ren_dirty_entities: &mut DirtyRenderEntitiesVec,
        cont_text_spans: &TextSpansSparseSecondary,
        sys_dwrite_layouts: &DwriteLayoutsSparseSecondary,
        lay_basic: &BasicLayoutsSecondary,
        lay_flex: &FlexLayoutsSecondary,
        lay_grid: &GridLayoutsSecondary,
        ren_active_transitions: &ActiveTransitionsSparseSecondary,
        topo_parents: &ParentsSecondary,
        topo_children: &ChildrenSecondary,
        lay_taffy_nodes: &TaffyNodesSecondary,
        lay_taffy: &mut TaffyTreeEntityId,
        lay_dirty_entities: &mut DirtyLayoutEntitiesVec,
        ren_interaction: &InteractionPropertiesSecondary,
        out_rects: &RectsSecondary,
        lay_scrollbar_styles: &mut ScrollbarStylesSecondary,
        out_scroll_offsets: &mut ScrollOffsetsSecondary,
        win_last_size: Option<LayoutSize>,
    ) {
        #[derive(Clone, Copy, PartialEq, Eq)]
        pub(crate) enum DragDirection {
            Vertical,
            Horizontal,
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
            let sb_state = lay_scrollbar_styles.get(current_id).cloned().unwrap();
            let container_rect = OutputStore::rect(current_id, out_rects).unwrap_or_default();
            let scroll_size = OutputStore::get_scroll_size(
                current_id,
                topo_active_masks,
                cont_input_contents,
                sys_text_engine,
                cont_text_contents,
                ren_visual,
                cont_text_spans,
                sys_dwrite_layouts,
                lay_basic,
                lay_flex,
                lay_grid,
                ren_active_transitions,
                topo_parents,
                topo_children,
                ren_interaction,
                out_rects,
                lay_scrollbar_styles,
                out_scroll_offsets,
            );
            (sb_state, container_rect, scroll_size)
        };

        let visible_size = WindowStore::calculate_visible_size(win_last_size, container_rect);

        let (
            track_id,
            thumb_id,
            track_len,
            thumb_len,
            margin_start,
            margin_end,
            delta_mouse,
            max_scroll_len,
            start_scroll_offset,
        ) = match direction {
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

                (
                    track_id,
                    thumb_id,
                    track_rect.height,
                    thumb_rect.height,
                    margin_top,
                    margin_bottom,
                    logical_pos.y - sb_state.drag_start_mouse.y,
                    scroll_size.height - visible_size.height,
                    sb_state.drag_start_offset.y,
                )
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

                (
                    track_id,
                    thumb_id,
                    track_rect.width,
                    thumb_rect.width,
                    margin_left,
                    margin_right,
                    logical_pos.x - sb_state.drag_start_mouse.x,
                    scroll_size.width - visible_size.width,
                    sb_state.drag_start_offset.x,
                )
            }
        };

        // スクロール可動域と割合
        let track_range = track_len - thumb_len - margin_start - margin_end;
        if track_range > 0.0 {
            let ratio = max_scroll_len / track_range;
            let target_scroll = start_scroll_offset + delta_mouse * ratio;

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
                topo_active_masks,
                cont_input_contents,
                sys_text_engine,
                cont_text_contents,
                ren_visual,
                cont_text_spans,
                sys_dwrite_layouts,
                lay_basic,
                lay_flex,
                lay_grid,
                ren_active_transitions,
                topo_parents,
                topo_children,
                ren_interaction,
                out_rects,
                lay_scrollbar_styles,
                out_scroll_offsets,
                win_last_size,
                lay_taffy_nodes,
                lay_taffy,
                lay_dirty_entities,
            );
        }

        RenderStore::mark_render_dirty(current_id, topo_active_masks, ren_dirty_entities);
    }

    /// 階層的な境界判定ヘルパー
    pub(crate) fn hit_test_recursive(
        id: EntityId,
        point: LayoutPoint,
        out_rects: &RectsSecondary,
        out_clip_rects: &ClipRectsSecondary,
        topo_children: &ChildrenSecondary,
        ren_visual: &VisualPropertiesSecondary,
        ren_base_visual: &BaseVisualPropertiesSecondary,
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
                    out_rects,
                    out_clip_rects,
                    topo_children,
                    ren_visual,
                    ren_base_visual,
                ) {
                    return Some(hit);
                }
            }
        }

        // pointer_events: none の場合は、自分自身の矩形判定のみをスルーする (子要素は上を辿れるため除外しない)
        // ren_visual に無ければ ren_base_visual を見に行く
        if let Some(rect) = out_rects.get(id)
            && rect.contains(point)
        {
            let pointer_events = ren_visual
                .get(id)
                .and_then(|v| v.pointer_events)
                .or_else(|| {
                    ren_base_visual
                        .get(id)
                        .and_then(|v| v.pointer_events)
                })
                .unwrap_or_default();

            if pointer_events != PointerEvents::None {
                return Some(id);
            }
        }

        None
    }

    // 溜まっているインスタンスを DrawBatch としてフラッシュ
    #[inline]
    fn flush_batch(
        batches: &mut Vec<DrawBatch>,
        instances: &mut Vec<QuadInstance>,
        ids: &mut Vec<EntityId>,
        scissor_rect: LayoutRect,
        batch_type: BatchType,
    ) {
        if instances.is_empty() {
            return;
        }
        batches.push(DrawBatch {
            scissor_rect,
            instances: std::mem::take(instances),
            entity_ids: std::mem::take(ids),
            batch_type,
        });
    }

    /// 現在の全アクティブ要素から、wgpu 用の前面・背面描画バッチを生成します
    // TOTO: フラットバッファ ＋ インデックス範囲に変更
    #[allow(clippy::too_many_arguments, clippy::too_many_lines)]
    pub(crate) fn collect_render_data(
        out_rects: &RectsSecondary,
        out_clip_rects: &ClipRectsSecondary,
        lay_basic: &BasicLayoutsSecondary,
        lay_flex: &FlexLayoutsSecondary,
        lay_grid: &GridLayoutsSecondary,
        topo_active_masks: &ActiveMasksSecondary,
        ren_active_transitions: &ActiveTransitionsSparseSecondary,
        topo_parents: &ParentsSecondary,
        topo_flat_dfs_sequence: &FlatDfsSequenceVec,
        topo_active_entities: &ActiveEntitiesVec,
        ren_interaction: &InteractionPropertiesSecondary,
        ren_base_visual: &BaseVisualPropertiesSecondary,
        ren_visual: &VisualPropertiesSecondary,
        ren_active_webviews: &ActiveWebviewsHashSet,
        out_selected_rects: &SelectedRectsSparseSecondary,
        out_scroll_offsets: &ScrollOffsetsSecondary,
        cont_input_contents: &InputContentsSparseSecondary,
        win_scale_factor: f32,
        evt_interaction_states: &InteractionStates,
        topo_sorted_entities: &mut SortedEntitiesVec,
        topo_effective_transforms: &mut EffectiveTransformsSecondary,
        topo_effective_z_indices: &mut EffectiveZindicesSecondary,
    ) -> RenderData {
        let mut batches = Vec::new();
        let mut current_instances = Vec::new();
        let mut current_ids = Vec::new();
        let mut last_clip = None;

        // 現在のバッチの種類 (通常)
        let mut current_batch_type = BatchType::Normal;

        // 静的なデフォルト値（一度だけ確保して使い回す）
        let default_visual = VisualProperty::default();

        // 各要素の実効トランスフォーム行列を DFS 順にカスケード累積
        RenderStore::accumulate_transform_matrix(
            topo_flat_dfs_sequence,
            ren_visual,
            topo_parents,
            topo_active_entities,
            topo_effective_transforms,
        );

        // 実効 z_index の計算とソートを一括実行
        TopologyStore::prepare_sorted_entities(
            topo_active_entities,
            topo_flat_dfs_sequence,
            ren_visual,
            topo_parents,
            topo_effective_z_indices,
            topo_sorted_entities,
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
            let is_webview_ready = is_webview && ren_active_webviews.contains(&id);

            let (basic, _, _) = LayoutStore::resolve_active_layouts(
                id,
                lay_basic,
                lay_flex,
                lay_grid,
                topo_active_masks,
                ren_active_transitions,
                topo_parents,
                ren_interaction,
                ren_visual,
            );
            let visual = ren_visual.get(id).unwrap_or(&default_visual);

            // 共通パラメータの展開
            let (packed_transform, origin) =
                RenderStore::get_transform_and_origin(id, visual, &topo_effective_transforms);
            let (o_width, o_color, o_lengths, outline_offset_and_flags) =
                RenderStore::get_outline_params(visual);

            // WebView (アクティブ) の個別処理
            if is_webview_ready {
                // 溜まっている通常（Normal）のバッチがあれば一旦フラッシュ
                OutputStore::flush_batch(
                    &mut batches,
                    &mut current_instances,
                    &mut current_ids,
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
                current_instances.push(punchout_instance);
                current_ids.push(id);

                // くり抜き用のバッチとして即座にフラッシュ
                OutputStore::flush_batch(
                    &mut batches,
                    &mut current_instances,
                    &mut current_ids,
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
                current_instances.push(border_instance);
                current_ids.push(id);

                current_batch_type = BatchType::Normal;
                last_clip = Some(clip);
                continue;
            }

            // WebView (非アクティブ・静止キャッシュ) の処理
            let is_webview_static = is_webview && !is_webview_ready;
            if is_webview_static {
                // 一般UIインスタンスがあれば強制フラッシュ
                OutputStore::flush_batch(
                    &mut batches,
                    &mut current_instances,
                    &mut current_ids,
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
                current_instances.push(static_instance);
                current_ids.push(id);

                OutputStore::flush_batch(
                    &mut batches,
                    &mut current_instances,
                    &mut current_ids,
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
                current_instances.push(border_instance);
                current_ids.push(id);

                OutputStore::flush_batch(
                    &mut batches,
                    &mut current_instances,
                    &mut current_ids,
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
                        &mut batches,
                        &mut current_instances,
                        &mut current_ids,
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
                let (_, flex, _) = LayoutStore::resolve_active_layouts(
                    id,
                    lay_basic,
                    lay_flex,
                    lay_grid,
                    topo_active_masks,
                    ren_active_transitions,
                    topo_parents,
                    ren_interaction,
                    ren_visual,
                );

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
                    current_instances.push(sel_instance);
                    current_ids.push(id);
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
                current_instances.push(bg_instance);
                current_ids.push(id);
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

            current_instances.push(instance);
            current_ids.push(id);

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
                let (_, flex, _) = LayoutStore::resolve_active_layouts(
                    id,
                    lay_basic,
                    lay_flex,
                    lay_grid,
                    topo_active_masks,
                    ren_active_transitions,
                    topo_parents,
                    ren_interaction,
                    ren_visual,
                );

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
                    .or(ren_base_visual.get(id).and_then(|v| v.text_color))
                    .or(visual.text_color)
                    .unwrap_or(Color::WHITE);

                let caret_instance = QuadInstance {
                    rect: caret_rect,
                    transform: packed_transform,
                    color: c_color,
                    opacity_mode_sizing: [visual.opacity.unwrap_or(1.0), -1.0, 0.0, 0.0],
                    ..Default::default()
                };

                current_instances.push(caret_instance);
                current_ids.push(id);
            }
        }

        // 走査終了後、最後に残ったバッチをフラッシュ
        OutputStore::flush_batch(
            &mut batches,
            &mut current_instances,
            &mut current_ids,
            last_clip.unwrap_or_default(),
            current_batch_type,
        );

        RenderData { batches }
    }
}

impl Context {
    #[inline]
    pub(crate) fn swap_output_rect(&mut self) {
        let OutputStore {
            out_rects,
            out_clip_rects,
            out_prev_rects,
            out_prev_clip_rects,
            ..
        } = &mut self.outputs;

        OutputStore::swap_output_rect(
            out_rects,
            out_prev_rects,
            out_clip_rects,
            out_prev_clip_rects,
        );
    }

    #[inline]
    pub(crate) fn parent_changed(&self, id: EntityId) -> bool {
        let TopologyStore {
            topo_parents,
            topo_active_masks,
            ..
        } = &self.topology;
        let OutputStore {
            out_rects,
            out_clip_rects,
            out_prev_rects,
            out_prev_clip_rects,
            ..
        } = &self.outputs;

        OutputStore::has_parent_changed(
            id,
            topo_parents,
            topo_active_masks,
            out_rects,
            out_prev_rects,
            out_clip_rects,
            out_prev_clip_rects,
        )
    }

    #[inline]
    pub(crate) fn calc_local_rect(
        &self,
        id: EntityId,
        window_size: LayoutSize,
    ) -> (LayoutRect, LayoutRect) {
        let LayoutStore {
            lay_taffy_nodes,
            lay_taffy,
            lay_basic,
            ..
        } = &self.layouts;
        let TopologyStore { topo_parents, .. } = &self.topology;
        let OutputStore {
            out_rects,
            out_clip_rects,
            out_scroll_offsets,
            ..
        } = &self.outputs;

        OutputStore::calc_local_rect(
            id,
            lay_taffy_nodes,
            lay_taffy,
            topo_parents,
            out_rects,
            out_clip_rects,
            lay_basic,
            out_scroll_offsets,
            window_size,
        )
    }

    /// `現在の選択範囲（out_text_selections）に基づき`、
    /// `描画用の物理選択矩形（out_selected_rects）を自動再計算して` `SoA` キャッシュを更新します。
    #[inline]
    pub(crate) fn update_selection_rects(&mut self, id: EntityId, layout: &IDWriteTextLayout) {
        let OutputStore {
            out_text_selections,
            out_selected_rects,
            ..
        } = &mut self.outputs;

        OutputStore::update_selection_rects(id, layout, out_text_selections, out_selected_rects);
    }

    /// スクロールオフセットを目標位置へクランプした上で代入。
    /// オフセットに変化が生じた場合は true を返し、レイアウトのDirtyマークを打つ。
    pub(crate) fn scroll_to(&mut self, id: EntityId, mut x: f32, mut y: f32) -> bool {
        let TopologyStore {
            topo_entities: topo_entities,
            topo_parents,
            topo_children,
            topo_active_masks,
            topo_active_entities,
            ..
        } = &mut self.topology;
        let LayoutStore {
            lay_basic,
            lay_base_basic,
            lay_flex,
            lay_grid,
            lay_scrollbar_styles,
            lay_taffy_nodes,
            lay_taffy,
            lay_dirty_entities,
        } = &mut self.layouts;
        let RenderStore {
            ren_visual,
            ren_interaction,
            ren_base_visual,
            ren_dirty_entities,
            ren_active_transitions,
            ren_active_animations,
            ..
        } = &mut self.renders;
        let OutputStore {
            out_rects,
            out_scroll_offsets,
            out_selected_rects,
            out_text_selections,
            ..
        } = &mut self.outputs;
        let ContentStore {
            cont_text_contents,
            cont_text_spans,
            cont_input_contents,
            ..
        } = &mut self.contents;
        let WindowStore {
            win_last_size, ..
        } = &mut self.window;
        let SystemStore {
            sys_text_engine,
            sys_dwrite_layouts,
            ..
        } = &mut self.system;

        OutputStore::scroll_to(
            id,
            x,
            y,
            topo_active_masks,
            cont_input_contents,
            sys_text_engine,
            cont_text_contents,
            ren_visual,
            cont_text_spans,
            sys_dwrite_layouts,
            lay_basic,
            lay_flex,
            lay_grid,
            ren_active_transitions,
            topo_parents,
            topo_children,
            ren_interaction,
            out_rects,
            lay_scrollbar_styles,
            out_scroll_offsets,
            *win_last_size,
            lay_taffy_nodes,
            lay_taffy,
            lay_dirty_entities,
        )
    }

    /// 階層的な境界判定ヘルパー（非対象のブランチをまるごとスキップ）
    pub(crate) fn hit_test_recursive(&self, id: EntityId, point: LayoutPoint) -> Option<EntityId> {
        let OutputStore {
            out_rects,
            out_clip_rects,
            ..
        } = &self.outputs;
        let TopologyStore { topo_children, .. } = &self.topology;
        let RenderStore {
            ren_visual,
            ren_base_visual,
            ..
        } = &self.renders;

        OutputStore::hit_test_recursive(
            id,
            point,
            out_rects,
            out_clip_rects,
            topo_children,
            ren_visual,
            ren_base_visual,
        )
    }

    /// 現在の全アクティブ要素から、wgpu 用の前面・背面描画バッチを生成します
    #[inline]
    pub(crate) fn collect_render_data(&mut self) -> RenderData {
        let OutputStore {
            out_rects,
            out_clip_rects,
            out_selected_rects,
            out_scroll_offsets,
            ..
        } = &self.outputs;
        let TopologyStore {
            topo_active_masks,
            topo_active_entities,
            topo_parents,
            topo_children,
            topo_flat_dfs_sequence,
            topo_sorted_entities,
            topo_effective_transforms,
            topo_effective_z_indices,
            ..
        } = &mut self.topology;
        let LayoutStore {
            lay_basic,
            lay_flex,
            lay_grid,
            ..
        } = &self.layouts;
        let RenderStore {
            ren_visual,
            ren_base_visual,
            ren_active_webviews,
            ren_active_transitions,
            ren_interaction,
            ..
        } = &self.renders;
        let ContentStore {
            cont_input_contents,
            ..
        } = &self.contents;
        let EventStore {
            evt_interaction_states,
            ..
        } = &self.events;
        let WindowStore { win_scale_factor, .. } = &self.window;

        OutputStore::collect_render_data(
            out_rects,
            out_clip_rects,
            lay_basic,
            lay_flex,
            lay_grid,
            topo_active_masks,
            ren_active_transitions,
            topo_parents,
            topo_flat_dfs_sequence,
            topo_active_entities,
            ren_interaction,
            ren_base_visual,
            ren_visual,
            ren_active_webviews,
            out_selected_rects,
            out_scroll_offsets,
            cont_input_contents,
            *win_scale_factor,
            evt_interaction_states,
            topo_sorted_entities,
            topo_effective_transforms,
            topo_effective_z_indices,
        )
    }
}
