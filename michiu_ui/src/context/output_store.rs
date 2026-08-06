use std::{ops::Range, time::Instant};

use crate::{
    ActiveMasksSecondary, ActiveTransitionsSparseSecondary, BaseVisualPropertiesSecondary,
    BasicLayoutsSecondary, BatchType, BoxSizing, ChildrenSecondary, Color, ComponentMask,
    ContentStore, Context, CornerRadius, DirtyLayoutEntitiesVec, DrawBatch,
    DwriteLayoutsSparseSecondary, EdgeInsets, EntityId, EventStore, FlexLayoutsSecondary,
    GridLayoutsSecondary, InputContents, InputContentsSparseSecondary,
    InteractionPropertiesSecondary, InteractionStates, LayoutPoint, LayoutRect, LayoutSize,
    LayoutStore, ParentsSecondary, PointerEvents, Position, QuadInstance, RenderData, RenderStore,
    STATE_QUEUED_LAYOUT, STYLE_TEXT_SPANS, ScrollbarStylesSecondary, SystemStore,
    TaffyNodesSecondary, TaffyTreeEntityId, TextAlign, TextContentsSparseSecondary, TextEngine,
    TextSpansSparseSecondary, TopologyStore, UserSelect, Val, VisualPropertiesSecondary,
    VisualProperty, WindowStore,
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

pub struct OutputStore {
    pub(crate) rects: RectsSecondary,
    pub(crate) clip_rects: ClipRectsSecondary,
    pub(crate) scroll_offsets: ScrollOffsetsSecondary,
    pub(crate) prev_rects: PrevRectsSecondary,
    pub(crate) prev_clip_rects: PrevClipRectsSecondary,
    pub(crate) selected_rects: SelectedRectsSparseSecondary,
    pub(crate) text_selections: TextSelectionsSparseSecondary,
    pub(crate) selection_start_index: SelectionStartIndexSparseSecondary,
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
            rects: SecondaryMap::new(),
            clip_rects: SecondaryMap::new(),
            scroll_offsets: SecondaryMap::new(),
            prev_rects: SecondaryMap::new(),
            prev_clip_rects: SecondaryMap::new(),
            selected_rects: SparseSecondaryMap::new(),
            text_selections: SparseSecondaryMap::new(),
            selection_start_index: SparseSecondaryMap::new(),
        }
    }

    #[inline]
    pub fn clear(&mut self) {
        self.rects.clear();
        self.clip_rects.clear();
        self.scroll_offsets.clear();
        self.prev_rects.clear();
        self.prev_clip_rects.clear();
        self.selected_rects.clear();
        self.text_selections.clear();
        self.selection_start_index.clear();
    }

    #[inline]
    pub fn despawn(&mut self, id: EntityId) {
        self.rects.remove(id);
        self.clip_rects.remove(id);
        self.scroll_offsets.remove(id);
        self.prev_rects.remove(id);
        self.prev_clip_rects.remove(id);
        self.selected_rects.remove(id);
        self.text_selections.remove(id);
        self.selection_start_index.remove(id);
    }
}

impl OutputStore {
    pub(crate) fn swap_output_rect(
        rects: &mut RectsSecondary,
        prev_rects: &mut PrevRectsSecondary,
        clip_rects: &mut ClipRectsSecondary,
        prev_clip_rects: &mut PrevClipRectsSecondary,
    ) {
        std::mem::swap(rects, prev_rects);
        std::mem::swap(clip_rects, prev_clip_rects);

        rects.clear();
        clip_rects.clear();
    }

    pub(crate) fn has_parent_changed(
        id: EntityId,
        parents: &ParentsSecondary,
        active_masks: &ActiveMasksSecondary,
        rects: &RectsSecondary,
        prev_rects: &PrevRectsSecondary,
        clip_rects: &ClipRectsSecondary,
        prev_clip_rects: &PrevClipRectsSecondary,
    ) -> bool {
        let Some(parent_id) = parents.get(id).copied().flatten() else {
            return false;
        };

        prev_rects.get(parent_id) != rects.get(parent_id)
            || prev_clip_rects.get(parent_id) != clip_rects.get(parent_id)
            || active_masks
                .get(parent_id)
                .is_some_and(|a| a.has(STATE_QUEUED_LAYOUT))
    }

    #[allow(clippy::too_many_arguments)]
    pub(crate) fn calc_local_rect(
        id: EntityId,
        taffy_nodes: &TaffyNodesSecondary,
        taffy: &TaffyTreeEntityId,
        parents: &ParentsSecondary,
        rects: &RectsSecondary,
        clip_rects: &ClipRectsSecondary,
        basic_layouts: &BasicLayoutsSecondary,
        scroll_offsets: &ScrollOffsetsSecondary,
        window_size: LayoutSize,
    ) -> (LayoutRect, LayoutRect) {
        let initial_clip = LayoutRect::new(0.0, 0.0, window_size.width, window_size.height);
        let local_rect = LayoutStore::local_rect_from_taffy(id, taffy_nodes, taffy);

        let parent_info = parents.get(id).copied().flatten().and_then(|p_id| {
            let rect = rects.get(p_id).copied()?;
            let clip = clip_rects.get(p_id).copied()?;
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

        let s_offsets = scroll_offsets.get(parent_id).copied().unwrap_or_default();
        let is_absolute = basic_layouts
            .get(id)
            .is_some_and(|l| l.position == Position::default());

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
        parents: &ParentsSecondary,
        rects: &RectsSecondary,
        last_window_size: Option<&LayoutSize>,
    ) -> Option<f32> {
        match val {
            Val::Px(v) => Some(v),
            Val::Percent(p) => {
                // 親要素の確定サイズを優先取得
                let parent_size = parents
                    .get(id)
                    .copied()
                    .flatten()
                    .and_then(|p_id| rects.get(p_id))
                    .map(|r| LayoutSize::new(r.width, r.height));

                // 親要素が未確定または存在しない場合は、最終ウィンドウ寸法を基準にする
                let ref_size = parent_size.or(last_window_size.copied())?;
                let ref_val = if is_width {
                    ref_size.width
                } else {
                    ref_size.height
                };

                Some(ref_val * (p / 100.0))
            }
            Val::Auto => {
                // Auto の場合は前フレームで確定している Taffy のレイアウト結果を実数値の基準値とする
                let r = rects.get(id)?;
                Some(if is_width { r.width } else { r.height })
            }
        }
    }

    /// 指定した要素の画面上の絶対座標（LayoutRect）を取得します。
    #[inline]
    pub(crate) fn rect(id: EntityId, rects: &RectsSecondary) -> Option<LayoutRect> {
        rects.get(id).copied()
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
        interaction_states: &InteractionStates,
        current_pointer_position: Option<&LayoutPoint>,
        clip_rects: &ClipRectsSecondary,
        visual_properties: &VisualPropertiesSecondary,
    ) -> bool {
        let Some(id) = interaction_states.pressed else {
            return false;
        };

        let Some(pointer_pos) = current_pointer_position else {
            return false;
        };

        let Some(clip) = clip_rects.get(id) else {
            return false;
        };

        let user_select = visual_properties
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

    /// `現在の選択範囲（text_selections）に基づき`、
    /// `描画用の物理選択矩形（selected_rects）を自動再計算して` `SoA` キャッシュを更新します。
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

        let mut rects = Vec::with_capacity(actual_count as usize);
        (0..actual_count as usize).for_each(|m_idx| {
            let metric = &hit_test_metrics[m_idx];
            rects.push(LayoutRect::new(
                metric.left,
                metric.top,
                metric.width,
                metric.height,
            ));
        });

        rects
    }

    /// 現在フォーカスされている要素で範囲選択されている文字列を取得します。
    #[must_use]
    pub fn get_selected_text(
        events: &EventStore,
        renders: &RenderStore,
        outputs: &OutputStore,
        contents: &ContentStore,
    ) -> Option<String> {
        let focused_id = events.interaction_states.focused?;
        let user_select = renders
            .visual_properties
            .get(focused_id)
            .and_then(|v| v.user_select)
            .unwrap_or_default();

        if user_select == UserSelect::Text {
            let range = outputs.text_selections.get(focused_id)?;
            if range.start < range.end {
                let text = contents.text_contents.get(focused_id)?;
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
            .text_selections
            .insert(focused_id, new_caret..new_caret);
        outputs.selected_rects.remove(focused_id);
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
        outputs.text_selections.insert(focused_id, prev_sel);
        outputs.selected_rects.remove(focused_id);
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
        outputs.text_selections.insert(focused_id, next_sel);
        outputs.selected_rects.remove(focused_id);
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
            .text_selections
            .insert(focused_id, range.start..range.start);
        outputs.selected_rects.remove(focused_id);
        contents.text.1.set(new_text);
    }

    #[inline]
    pub(crate) fn truncate_unconfirmed_text(
        text_val: &str,
        input_contents: &InputContents,
        max: usize,
        filtered_comp_text: String,
    ) -> String {
        let text_u16: Vec<u16> = text_val.encode_utf16().collect();
        let range = &input_contents.selected_range;
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
        active_masks: &ActiveMasksSecondary,
        input_contents: &InputContentsSparseSecondary,
        text_engine: &TextEngine,
        text_contents: &TextContentsSparseSecondary,
        visual_properties: &VisualPropertiesSecondary,
        text_spans: &TextSpansSparseSecondary,
        dwrite_layouts: &DwriteLayoutsSparseSecondary,
        basic_layouts: &BasicLayoutsSecondary,
        flex_layouts: &FlexLayoutsSecondary,
        grid_layouts: &GridLayoutsSecondary,
        active_transitions: &ActiveTransitionsSparseSecondary,
        parents: &ParentsSecondary,
        children: &ChildrenSecondary,
        interaction_properties: &InteractionPropertiesSecondary,
        rects: &RectsSecondary,
        scrollbar_styles: &ScrollbarStylesSecondary,
        scroll_offsets: &ScrollOffsetsSecondary,
    ) -> LayoutSize {
        let mut max_x = 0.0f32;
        let mut max_y = 0.0f32;

        // 自身に内包されたインラインコンテンツの計測サイズを初期値とする
        if active_masks
            .get(id)
            .is_some_and(ComponentMask::has_input_content)
            && let Some(contents) = input_contents.get(id)
            && let Some(layout_rect) = contents.last_layout
        {
            max_x =
                layout_rect.width + contents.caret_width.unwrap_or(contents.default_caret_width);
            max_y = layout_rect.height;
        } else if active_masks
            .get(id)
            .is_some_and(ComponentMask::has_text_content)
            && let Some(dw_layout) = SystemStore::get_or_create_layout(
                id,
                text_contents,
                visual_properties,
                dwrite_layouts,
                text_spans,
                text_engine,
            )
        {
            let size = text_engine.get_layout_size(&dw_layout);
            max_x = size.width;
            max_y = size.height;
        }

        // 親要素自体のボーダー・パディング厚を取得
        let (basic, _, _) = LayoutStore::resolve_active_layouts(
            id,
            basic_layouts,
            flex_layouts,
            grid_layouts,
            active_masks,
            active_transitions,
            parents,
            interaction_properties,
            visual_properties,
        );

        let rect = OutputStore::rect(id, rects).unwrap_or_default();
        let (border, padding) =
            LayoutStore::get_physical_border_padding(rect, basic.border, basic.padding);

        let offset_x = border.left + padding.left;
        let offset_y = border.top + padding.top;

        // スクロールバー要素のIDを取得して除外対象にする
        let (v_track_opt, h_track_opt) = if let Some(sb_state) = scrollbar_styles.get(id) {
            (sb_state.v_track_id, sb_state.h_track_id)
        } else {
            (None, None)
        };

        if let Some(children_list) = children.get(id) {
            for &child_id in children_list {
                // スクロールバーのトラックはサイズ計算から除外
                if Some(child_id) == v_track_opt || Some(child_id) == h_track_opt {
                    continue;
                }

                // 絶対配置要素（スクロールバーのサムなど）もスクロール領域サイズ計算から除外
                let is_absolute = basic_layouts
                    .get(child_id)
                    .is_some_and(|l| l.position == Position::Absolute);
                if is_absolute {
                    continue;
                }

                if let Some(&rect) = rects.get(child_id) {
                    let parent_rect = OutputStore::rect(id, rects).unwrap_or_default();
                    let scroll_offset = scroll_offsets.get(id).copied().unwrap_or_default();

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
        input_contents: &mut InputContentsSparseSecondary,
        text_contents: &mut TextContentsSparseSecondary,
        text_selections: &mut TextSelectionsSparseSecondary,
        text_spans: &TextSpansSparseSecondary,
        text_engine: &TextEngine,
        visual_properties: &mut VisualPropertiesSecondary,
        base_visual_properties: &BaseVisualPropertiesSecondary,
    ) -> Option<(LayoutRect, f32, bool)> {
        // (caret_x, caret_y, caret_h, caret_w, caret_offset, is_multiline)
        let mut scroll_ime_info: Option<(LayoutRect, f32, bool)> = None;

        if let Some(contents) = input_contents.get_mut(id) {
            // 入力エンジン側の最新カーソル位置を描画SoA側に同期
            text_selections.insert(id, contents.selected_range.clone());

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
                RenderStore::get_font_propery(id, visual_properties);

            let spans = text_spans.get(id).map_or(&[][..], Vec::as_slice);

            // 描画テキスト全体のレイアウトサイズを Taffy 測定用に設定
            let display_layout = text_engine.create_layout(
                &display_text,
                font_size,
                font_family,
                font_weight,
                font_style,
                None,
                spans,
            );
            let text_size = text_engine.get_layout_size(&display_layout);
            contents.last_layout =
                Some(LayoutRect::new(0.0, 0.0, text_size.width, text_size.height));

            // キャレット位置測定用のレイアウトをプレースホルダー抜きで作成
            let caret_layout = text_engine.create_layout(
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
                text_engine.get_caret_position(&caret_layout, caret_index, u16_len_caret);

            contents.measured_caret_x = cx_offset;
            contents.measured_caret_y = cy_offset;
            contents.caret_line_height = ch_height;

            let (curr_line, tot_lines) = crate::calculate_line_indices(&display_text, caret_index);
            contents.current_line_index = curr_line;
            contents.total_lines = tot_lines;

            // 最終表示用テキストを Context 側に反映
            text_contents.insert(id, display_text.into());

            if let Some(visual) = visual_properties.get_mut(id) {
                let is_ime_active = contents
                    .ime_state
                    .as_ref()
                    .is_some_and(|ime| !ime.composition_text.is_empty());

                if text_val.is_empty() && !is_ime_active {
                    // 確定文字列が空で、かつ未確定文字列も存在しない状態のみグレー表示
                    visual.text_color = contents.placeholder_color;
                } else {
                    let base_color = base_visual_properties
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
        active_masks: &mut ActiveMasksSecondary,
        text_selections: &mut TextSelectionsSparseSecondary,
        selected_rects: &mut SelectedRectsSparseSecondary,
        input_contents: &mut InputContentsSparseSecondary,
        text_spans: &mut TextSpansSparseSecondary,
    ) {
        text_selections.remove(id);
        selected_rects.remove(id);
        if let Some(contents) = input_contents.get_mut(id) {
            contents.selected_range = 0..0;
            // 進行中の IME コンポジションをリセットして波線を消去
            contents.ime_state = None;
            contents.marked_range = None;
        }
        text_spans.remove(id);
        if let Some(i) = active_masks.get_mut(id) {
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
        active_masks: &mut ActiveMasksSecondary,
        input_contents: &InputContentsSparseSecondary,
        text_engine: &TextEngine,
        text_contents: &TextContentsSparseSecondary,
        visual_properties: &VisualPropertiesSecondary,
        text_spans: &TextSpansSparseSecondary,
        dwrite_layouts: &DwriteLayoutsSparseSecondary,
        basic_layouts: &BasicLayoutsSecondary,
        flex_layouts: &FlexLayoutsSecondary,
        grid_layouts: &GridLayoutsSecondary,
        active_transitions: &ActiveTransitionsSparseSecondary,
        parents: &ParentsSecondary,
        children: &ChildrenSecondary,
        interaction_properties: &InteractionPropertiesSecondary,
        rects: &RectsSecondary,
        scrollbar_styles: &mut ScrollbarStylesSecondary,
        scroll_offsets: &mut ScrollOffsetsSecondary,
        last_window_size: Option<LayoutSize>,
        taffy_nodes: &TaffyNodesSecondary,
        taffy: &mut TaffyTreeEntityId,
        dirty_layout_entities: &mut DirtyLayoutEntitiesVec,
    ) -> bool {
        let Some(rect) = OutputStore::rect(id, rects) else {
            return false;
        };

        let scroll_size = OutputStore::get_scroll_size(
            id,
            active_masks,
            input_contents,
            text_engine,
            text_contents,
            visual_properties,
            text_spans,
            dwrite_layouts,
            basic_layouts,
            flex_layouts,
            grid_layouts,
            active_transitions,
            parents,
            children,
            interaction_properties,
            rects,
            scrollbar_styles,
            scroll_offsets,
        );

        // 親コンテナのボーダーおよびパディング厚を取得
        let (basic, _, _) = LayoutStore::resolve_active_layouts(
            id,
            basic_layouts,
            flex_layouts,
            grid_layouts,
            active_masks,
            active_transitions,
            parents,
            interaction_properties,
            visual_properties,
        );
        let (border, padding) =
            LayoutStore::get_physical_border_padding(rect, basic.border, basic.padding);

        let visible_size = WindowStore::calculate_visible_size(last_window_size, rect);
        let content_size = LayoutStore::calculate_inner_content_size(visible_size, border, padding);

        // コンテンツサイズと内枠表示領域サイズの差分として、正確な最大スクロール量を算出
        let max_scroll_x = (scroll_size.width - content_size.width).max(0.0);
        let max_scroll_y = (scroll_size.height - content_size.height).max(0.0);

        x = x.clamp(0.0, max_scroll_x);
        y = y.clamp(0.0, max_scroll_y);

        // スロットが存在しない場合はあらかじめ挿入して初期化
        if !scroll_offsets.contains_key(id) {
            scroll_offsets.insert(id, LayoutPoint::ZERO);
        }

        let current = scroll_offsets.get_mut(id).unwrap();
        if (current.x - x).abs() > 0.01 || (current.y - y).abs() > 0.01 {
            current.x = x;
            current.y = y;

            // スクロールバー状態の最終スクロール時刻を更新
            if let Some(sb_state) = scrollbar_styles.get_mut(id) {
                sb_state.last_scroll_time = Some(Instant::now());
            }

            // オフセット変化に伴い、子孫全体の絶対座標を再同期させる
            LayoutStore::mark_layout_dirty(
                id,
                taffy_nodes,
                taffy,
                active_masks,
                dirty_layout_entities,
                parents,
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
        active_masks: &mut ActiveMasksSecondary,
        input_contents: &InputContentsSparseSecondary,
        text_engine: &TextEngine,
        text_contents: &TextContentsSparseSecondary,
        visual_properties: &VisualPropertiesSecondary,
        text_spans: &TextSpansSparseSecondary,
        dwrite_layouts: &DwriteLayoutsSparseSecondary,
        basic_layouts: &BasicLayoutsSecondary,
        flex_layouts: &FlexLayoutsSecondary,
        grid_layouts: &GridLayoutsSecondary,
        active_transitions: &ActiveTransitionsSparseSecondary,
        parents: &ParentsSecondary,
        children: &ChildrenSecondary,
        interaction_properties: &InteractionPropertiesSecondary,
        rects: &RectsSecondary,
        scrollbar_styles: &mut ScrollbarStylesSecondary,
        scroll_offsets: &mut ScrollOffsetsSecondary,
        last_window_size: Option<LayoutSize>,
        taffy_nodes: &TaffyNodesSecondary,
        taffy: &mut TaffyTreeEntityId,
        dirty_layout_entities: &mut DirtyLayoutEntitiesVec,
    ) -> bool {
        let current = scroll_offsets.get(id).copied().unwrap_or_default();

        OutputStore::scroll_to(
            id,
            current.x + dx,
            current.y + dy,
            active_masks,
            input_contents,
            text_engine,
            text_contents,
            visual_properties,
            text_spans,
            dwrite_layouts,
            basic_layouts,
            flex_layouts,
            grid_layouts,
            active_transitions,
            parents,
            children,
            interaction_properties,
            rects,
            scrollbar_styles,
            scroll_offsets,
            last_window_size,
            taffy_nodes,
            taffy,
            dirty_layout_entities,
        )
    }

    /// 現在のテキスト・IME状態・フォントサイズから、
    /// キャレットの物理座標や最終表示テキスト、レイアウト矩形を正確に再計算して `SoA` を更新。
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn update_input_caret_position(
        id: EntityId,
        rects: &RectsSecondary,
        dwrite_layouts: &DwriteLayoutsSparseSecondary,
        input_contents: &mut InputContentsSparseSecondary,
        text_contents: &mut TextContentsSparseSecondary,
        text_selections: &mut TextSelectionsSparseSecondary,
        text_spans: &TextSpansSparseSecondary,
        text_engine: &TextEngine,
        basic_layouts: &BasicLayoutsSecondary,
        flex_layouts: &FlexLayoutsSecondary,
        grid_layouts: &GridLayoutsSecondary,
        active_masks: &mut ActiveMasksSecondary,
        children: &ChildrenSecondary,
        interaction_properties: &InteractionPropertiesSecondary,
        scrollbar_styles: &mut ScrollbarStylesSecondary,
        scroll_offsets: &mut ScrollOffsetsSecondary,
        last_window_size: Option<LayoutSize>,
        taffy_nodes: &TaffyNodesSecondary,
        taffy: &mut TaffyTreeEntityId,
        dirty_layout_entities: &mut DirtyLayoutEntitiesVec,
        active_transitions: &ActiveTransitionsSparseSecondary,
        parents: &ParentsSecondary,
        visual_properties: &mut VisualPropertiesSecondary,
        base_visual_properties: &BaseVisualPropertiesSecondary,
        scale_factor: f32,
    ) {
        // IMEやタイピング中の古いキャッシュを破棄
        SystemStore::clear_layout_cache(id, dwrite_layouts);

        let scroll_ime_info = OutputStore::scroll_ime_info(
            id,
            input_contents,
            text_contents,
            text_selections,
            text_spans,
            text_engine,
            visual_properties,
            base_visual_properties,
        );

        let Some((caret, caret_offset, is_multiline)) = scroll_ime_info else {
            return;
        };
        let (basic, flex, _) = LayoutStore::resolve_active_layouts(
            id,
            basic_layouts,
            flex_layouts,
            grid_layouts,
            active_masks,
            active_transitions,
            parents,
            interaction_properties,
            visual_properties,
        );
        let rect = OutputStore::rect(id, rects).unwrap_or_default();
        let (border, padding) =
            LayoutStore::get_physical_border_padding(rect, basic.border, basic.padding);

        let mut scroll = scroll_offsets.get(id).copied().unwrap_or_default();

        if rect.width > 0.0 && rect.height > 0.0 {
            let viewport = LayoutStore::calculate_viewport_size(rect, border, padding);

            let text_size = if let Some(contents) = input_contents.get(id)
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
                active_masks,
                input_contents,
                text_engine,
                text_contents,
                visual_properties,
                text_spans,
                dwrite_layouts,
                basic_layouts,
                flex_layouts,
                grid_layouts,
                active_transitions,
                parents,
                children,
                interaction_properties,
                rects,
                scrollbar_styles,
                scroll_offsets,
                last_window_size,
                taffy_nodes,
                taffy,
                dirty_layout_entities,
            );
        }

        // IMM32 による IME 変換候補ウィンドウの位置同期を自動実行
        SystemStore::sync_imm_window_position(
            rect,
            scale_factor,
            border,
            padding,
            caret,
            caret_offset,
            scroll,
        );
    }

    /// `現在の選択範囲（text_selections）に基づき`、
    /// `描画用の物理選択矩形（selected_rects）を自動再計算して` `SoA` キャッシュを更新します。
    #[inline]
    pub(crate) fn update_selection_rects(
        id: EntityId,
        layout: &IDWriteTextLayout,
        text_selections: &TextSelectionsSparseSecondary,
        selected_rects: &mut SelectedRectsSparseSecondary,
    ) {
        if let Some(range) = text_selections.get(id).cloned()
            && range.start < range.end
        {
            let rects = OutputStore::calc_selection_rects(id, layout, range);
            selected_rects.insert(id, rects);
            return;
        }
        // 範囲が 0、または選択なしの時は自動クリーンアップ
        selected_rects.remove(id);
    }
}

impl Context {
    #[inline]
    pub(crate) fn swap_output_rect(&mut self) {
        let OutputStore {
            rects,
            clip_rects,
            prev_rects,
            prev_clip_rects,
            ..
        } = &mut self.outputs;

        OutputStore::swap_output_rect(rects, prev_rects, clip_rects, prev_clip_rects);
    }

    #[inline]
    pub(crate) fn parent_changed(&self, id: EntityId) -> bool {
        let TopologyStore {
            parents,
            active_masks,
            ..
        } = &self.topology;
        let OutputStore {
            rects,
            clip_rects,
            prev_rects,
            prev_clip_rects,
            ..
        } = &self.outputs;

        OutputStore::has_parent_changed(
            id,
            parents,
            active_masks,
            rects,
            prev_rects,
            clip_rects,
            prev_clip_rects,
        )
    }

    #[inline]
    pub(crate) fn calc_local_rect(
        &self,
        id: EntityId,
        window_size: LayoutSize,
    ) -> (LayoutRect, LayoutRect) {
        let LayoutStore {
            taffy_nodes,
            taffy,
            basic_layouts,
            ..
        } = &self.layouts;
        let TopologyStore { parents, .. } = &self.topology;
        let OutputStore {
            rects,
            clip_rects,
            scroll_offsets,
            ..
        } = &self.outputs;

        OutputStore::calc_local_rect(
            id,
            taffy_nodes,
            taffy,
            parents,
            rects,
            clip_rects,
            basic_layouts,
            scroll_offsets,
            window_size,
        )
    }

    /// 単位（Px, Percent, Auto）を親要素のサイズまたはウィンドウ基準をベースに f32 (物理ピクセル) へ解決します。
    #[inline]
    pub(crate) fn val_to_px(&self, id: EntityId, val: Val, is_width: bool) -> Option<f32> {
        let TopologyStore { parents, .. } = &self.topology;
        let OutputStore { rects, .. } = &self.outputs;
        let WindowStore {
            last_window_size, ..
        } = &self.window;

        OutputStore::val_to_px(id, val, is_width, parents, rects, last_window_size.as_ref())
    }

    #[allow(clippy::too_many_arguments)]
    #[inline]
    pub(crate) fn calculate_caret_rect(
        rect: LayoutRect,
        border: EdgeInsets,
        padding: EdgeInsets,
        contents: &InputContents,
        scale: f32,
        scroll: LayoutPoint,
        align_offset: LayoutPoint,
    ) -> LayoutRect {
        OutputStore::calculate_caret_rect(
            rect,
            border,
            padding,
            contents,
            scale,
            scroll,
            align_offset,
        )
    }

    /// 現在テキスト選択ドラッグ中かつ、マウスポインタが要素の可視境界外にあるかを判定
    #[inline]
    pub(crate) fn is_drag_autoscroll_active(&self) -> bool {
        let RenderStore {
            visual_properties, ..
        } = &self.renders;
        let OutputStore { clip_rects, .. } = &self.outputs;
        let EventStore {
            current_pointer_position,
            interaction_states,
            ..
        } = &self.events;

        OutputStore::is_drag_autoscroll_active(
            interaction_states,
            current_pointer_position.as_ref(),
            clip_rects,
            visual_properties,
        )
    }

    /// `現在の選択範囲（text_selections）に基づき`、
    /// `描画用の物理選択矩形（selected_rects）を自動再計算して` `SoA` キャッシュを更新します。
    #[inline]
    pub(crate) fn update_selection_rects(&mut self, id: EntityId, layout: &IDWriteTextLayout) {
        let OutputStore {
            text_selections,
            selected_rects,
            ..
        } = &mut self.outputs;

        OutputStore::update_selection_rects(id, layout, text_selections, selected_rects);
    }

    pub(crate) fn sync_scrollbar_drag(&mut self, logical_pos: LayoutPoint) {
        let mut scrollbar_dragged = false;
        let mut active_drag_target: Option<(EntityId, bool, bool)> = None;

        for (id, state) in &self.layouts.scrollbar_styles {
            if state.v_thumb_dragged {
                active_drag_target = Some((id, true, false));
                break;
            } else if state.h_thumb_dragged {
                active_drag_target = Some((id, false, true));
                break;
            }
        }

        if let Some((current_id, is_vertical, is_horiazon)) = active_drag_target {
            let (sb_state, container_rect, scroll_size) = {
                let sb_state = self
                    .layouts
                    .scrollbar_styles
                    .get(current_id)
                    .cloned()
                    .unwrap();
                let container_rect = self
                    .outputs
                    .rects
                    .get(current_id)
                    .copied()
                    .unwrap_or_default();
                let scroll_size = self.get_scroll_size(current_id);
                (sb_state, container_rect, scroll_size)
            };

            let visible_size = self.calculate_visible_size(container_rect);

            if is_vertical {
                let track_id = sb_state.v_track_id.unwrap();
                let thumb_id = sb_state.v_thumb_id.unwrap();
                let track_rect = self.outputs.rects[track_id];
                let thumb_rect = self.outputs.rects[thumb_id];

                // サムのマージンを差し引く
                let mut margin_top = 0.0;
                let mut margin_bottom = 0.0;
                if let Some(ref thumb_style) = sb_state.style.v_thumb {
                    if let Val::Px(val) = thumb_style.inner.basic_layout.margin.top {
                        margin_top = val;
                    }
                    if let Val::Px(val) = thumb_style.inner.basic_layout.margin.bottom {
                        margin_bottom = val;
                    }
                }

                // 同期処理と同じく、マージンを含めた実際の有効可動域を正確に計算
                let track_range =
                    track_rect.height - thumb_rect.height - margin_top - margin_bottom;
                if track_range > 0.0 {
                    let dy = logical_pos.y - sb_state.drag_start_mouse.y;
                    let max_scroll_y = scroll_size.height - visible_size.height;

                    if max_scroll_y > 0.0 {
                        let ratio = max_scroll_y / track_range;
                        let target_scroll_y = sb_state.drag_start_offset.y + dy * ratio;

                        let current_x = self
                            .outputs
                            .scroll_offsets
                            .get(current_id)
                            .map_or(0.0, |o| o.x);
                        self.scroll_to(current_id, current_x, target_scroll_y);
                    }
                }
            } else if is_horiazon {
                let track_id = sb_state.h_track_id.unwrap();
                let thumb_id = sb_state.h_thumb_id.unwrap();
                let track_rect = self.outputs.rects[track_id];
                let thumb_rect = self.outputs.rects[thumb_id];

                let mut margin_left = 0.0;
                let mut margin_right = 0.0;
                if let Some(ref thumb_style) = sb_state.style.h_thumb {
                    if let Val::Px(val) = thumb_style.inner.basic_layout.margin.left {
                        margin_left = val;
                    }
                    if let Val::Px(val) = thumb_style.inner.basic_layout.margin.right {
                        margin_right = val;
                    }
                }

                let track_range = track_rect.width - thumb_rect.width - margin_left - margin_right;
                if track_range > 0.0 {
                    let dx = logical_pos.x - sb_state.drag_start_mouse.x;
                    let max_scroll_x = scroll_size.width - visible_size.width;

                    if max_scroll_x > 0.0 {
                        let ratio = max_scroll_x / track_range;
                        let target_scroll_x = sb_state.drag_start_offset.x + dx * ratio;

                        let current_y = self
                            .outputs
                            .scroll_offsets
                            .get(current_id)
                            .map_or(0.0, |o| o.y);
                        self.scroll_to(current_id, target_scroll_x, current_y);
                    }
                }
            }

            self.mark_render_dirty(current_id);
            scrollbar_dragged = true;
        }
    }

    /// 指定された要素の子要素全体のスクロール領域を親ローカル座標系で算出します。
    pub(crate) fn get_scroll_size(&self, id: EntityId) -> LayoutSize {
        let mut max_x = 0.0f32;
        let mut max_y = 0.0f32;

        // 自身に内包されたインラインコンテンツの計測サイズを初期値とする
        if self.topology.active_masks[id].has_input_content()
            && let Some(contents) = self.contents.input_contents.get(id)
            && let Some(layout_rect) = contents.last_layout
        {
            max_x =
                layout_rect.width + contents.caret_width.unwrap_or(contents.default_caret_width);
            max_y = layout_rect.height;
        } else if self.topology.active_masks[id].has_text_content()
            && let Some(layout) = self.get_or_create_layout(id)
        {
            let size = self.system.text_engine.get_layout_size(&layout);
            max_x = size.width;
            max_y = size.height;
        }

        // 親要素自体のボーダー・パディング厚を取得
        let (basic, _, _) = self.resolve_active_layouts(id);
        let rect = self.rect(id).unwrap_or_default();
        let (border, padding) =
            LayoutStore::get_physical_border_padding(rect, basic.border, basic.padding);

        let offset_x = border.left + padding.left;
        let offset_y = border.top + padding.top;

        // スクロールバー要素のIDを取得して除外対象にする
        let (v_track_opt, h_track_opt) =
            if let Some(sb_state) = self.layouts.scrollbar_styles.get(id) {
                (sb_state.v_track_id, sb_state.h_track_id)
            } else {
                (None, None)
            };

        if let Some(children_list) = self.topology.children.get(id) {
            for &child_id in children_list {
                // スクロールバーのトラックはサイズ計算から除外
                if Some(child_id) == v_track_opt || Some(child_id) == h_track_opt {
                    continue;
                }

                // 絶対配置要素（スクロールバーのサムなど）もスクロール領域サイズ計算から除外
                let is_absolute = self
                    .layouts
                    .basic_layouts
                    .get(child_id)
                    .is_some_and(|l| l.position == Position::Absolute);
                if is_absolute {
                    continue;
                }

                if let Some(&rect) = self.outputs.rects.get(child_id) {
                    let parent_rect = self.outputs.rects.get(id).copied().unwrap_or_default();
                    let scroll_offset = self
                        .outputs
                        .scroll_offsets
                        .get(id)
                        .copied()
                        .unwrap_or_default();

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

    /// スクロールオフセットを目標位置へクランプした上で代入。
    /// オフセットに変化が生じた場合は true を返し、レイアウトのDirtyマークを打つ。
    pub(crate) fn scroll_to(&mut self, id: EntityId, mut x: f32, mut y: f32) -> bool {
        let TopologyStore {
            entities,
            parents,
            children,
            active_masks,
            active_entities,
            session_spawned,
            session_roots,
            flat_dfs_sequence,
            is_structure_dirty,
        } = &mut self.topology;
        let LayoutStore {
            basic_layouts,
            base_basic_layouts,
            flex_layouts,
            grid_layouts,
            scrollbar_styles,
            taffy_nodes,
            taffy,
            dirty_layout_entities,
        } = &mut self.layouts;
        let RenderStore {
            visual_properties,
            interaction_properties,
            base_visual_properties,
            dirty_render_entities,
            active_transitions,
            active_animations,
            active_webviews,
            last_tick_time,
        } = &mut self.renders;
        let OutputStore {
            rects,
            clip_rects,
            scroll_offsets,
            prev_rects,
            prev_clip_rects,
            selected_rects,
            text_selections,
            selection_start_index,
        } = &mut self.outputs;
        let ContentStore {
            text_contents,
            text_spans,
            input_contents,
            image_sources,
            movie_properties,
            webview_contents,
        } = &mut self.contents;
        let WindowStore {
            last_window_size, ..
        } = &mut self.window;
        let SystemStore {
            text_engine,
            dwrite_layouts,
            uia_properties,
            task_sender,
            task_receiver,
        } = &mut self.system;

        OutputStore::scroll_to(
            id,
            x,
            y,
            active_masks,
            input_contents,
            text_engine,
            text_contents,
            visual_properties,
            text_spans,
            dwrite_layouts,
            basic_layouts,
            flex_layouts,
            grid_layouts,
            active_transitions,
            parents,
            children,
            interaction_properties,
            rects,
            scrollbar_styles,
            scroll_offsets,
            *last_window_size,
            taffy_nodes,
            taffy,
            dirty_layout_entities,
        )
    }

    /// 階層的な境界判定ヘルパー（非対象のブランチをまるごとスキップ）
    pub(crate) fn hit_test_recursive(&self, id: EntityId, point: LayoutPoint) -> Option<EntityId> {
        // 1. 親などの overflow: hidden 等でクリップされている表示範囲をチェック
        // クリップ領域外であれば、この要素もそのすべての子孫要素も画面上に見えていないため、走査を即座にスキップ（枝刈り）
        if let Some(clip) = self.clip_rect(id)
            && !clip.contains(point)
        {
            return None;
        }

        // 2. 子要素を逆順（前面優先）で再帰降下
        if let Some(children) = self.topology.children.get(id) {
            let child_len = children.len();
            for i in (0..child_len).rev() {
                let child_id = children[i];
                if let Some(hit) = self.hit_test_recursive(child_id, point) {
                    return Some(hit);
                }
            }
        }

        // pointer_events: none の場合は、自分自身の矩形判定のみをスルーする (子要素は上を辿れるため除外しない)
        // visual_properties (動的) に無ければ base_visual_properties (静的) を見に行く
        let pointer_events = self
            .renders
            .visual_properties
            .get(id)
            .and_then(|v| v.pointer_events)
            .or_else(|| {
                self.renders
                    .base_visual_properties
                    .get(id)
                    .and_then(|v| v.pointer_events)
            })
            .unwrap_or(PointerEvents::Auto);

        if pointer_events != PointerEvents::None
            && let Some(rect) = self.outputs.rects.get(id)
            && rect.contains(point)
        {
            return Some(id);
        }

        None
    }

    #[inline]
    pub(crate) fn clear_selection_highlight_rect(&mut self, id: EntityId) {
        let TopologyStore { active_masks, .. } = &mut self.topology;
        let ContentStore {
            input_contents,
            text_spans,
            ..
        } = &mut self.contents;
        let OutputStore {
            text_selections,
            selected_rects,
            ..
        } = &mut self.outputs;

        OutputStore::clear_selection_highlight_rect(
            id,
            active_masks,
            text_selections,
            selected_rects,
            input_contents,
            text_spans,
        );
    }

    /// 現在の全アクティブ要素から、wgpu 用の前面・背面描画バッチを生成します
    pub(crate) fn collect_render_data(&self) -> RenderData {
        let mut batches = Vec::new();
        let mut current_instances = Vec::new();
        let mut current_ids = Vec::new();
        let mut last_clip = None;

        // 現在のバッチの種類 (通常)
        let mut current_batch_type = BatchType::Normal;

        // 静的なデフォルト値（一度だけ確保して使い回す）
        let default_visual = VisualProperty::default();

        // 各要素の実効トランスフォーム行列を DFS 順にカスケード累積
        let mut effective_transforms = self.accumulate_transform_matrix();

        // 各要素の実効 z_index を親から子へカスケード（伝播）して計算
        let effective_z_indices = self.compute_effective_z_indices();

        // 実効 z_index で active_entities を安定ソート
        let mut sorted_entities = self.topology.active_entities.clone();
        sorted_entities.sort_by_key(|&id| effective_z_indices.get(id).copied().unwrap_or(0));

        // 溜まっているインスタンスを DrawBatch としてフラッシュ
        fn flush_batch(
            batches: &mut Vec<DrawBatch>,
            instances: &mut Vec<QuadInstance>,
            ids: &mut Vec<EntityId>,
            scissor_rect: LayoutRect,
            batch_type: BatchType,
        ) {
            if !instances.is_empty() {
                batches.push(DrawBatch {
                    scissor_rect,
                    instances: std::mem::take(instances),
                    entity_ids: std::mem::take(ids),
                    batch_type,
                });
            }
        }

        for &id in &sorted_entities {
            let rect = self.rect(id).unwrap_or_default();
            if rect.width <= 0.0 || rect.height <= 0.0 {
                continue;
            }

            let clip = self.clip_rect(id).unwrap_or_default();
            let is_webview = self.topology.active_masks[id].has_webveiw2_content();

            // コントローラーがまだ初期化されていない場合は通常通り背景を描画し透過を防止
            let is_webview_ready = is_webview && self.renders.active_webviews.contains(&id);

            let (basic, _, _) = self.resolve_active_layouts(id);
            let visual = self
                .renders
                .visual_properties
                .get(id)
                .unwrap_or(&default_visual);

            // 共通パラメータの展開
            let (packed_transform, origin) =
                self.get_transform_and_origin(id, visual, &effective_transforms);
            let (o_width, o_color, o_lengths, outline_offset_and_flags) =
                RenderStore::get_outline_params(visual);

            // WebView (アクティブ) の個別処理
            if is_webview_ready {
                // 溜まっている通常（Normal）のバッチがあれば一旦フラッシュ
                flush_batch(
                    &mut batches,
                    &mut current_instances,
                    &mut current_ids,
                    last_clip.unwrap_or(LayoutRect::ZERO),
                    current_batch_type,
                );

                let punchout_opacity = visual.opacity.unwrap_or(1.0);
                let punchout_instance = QuadInstance {
                    rect,
                    transform: packed_transform,
                    transform_origin: origin,
                    color: Color::WHITE,
                    corner_radius: visual.corner_radius.unwrap_or(CornerRadius::ZERO),
                    opacity_mode_sizing: [punchout_opacity, 0.0, 0.0, 0.0],
                    ..Default::default()
                };
                current_instances.push(punchout_instance);
                current_ids.push(id);

                // くり抜き用のバッチとして即座にフラッシュ
                flush_batch(
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
                    corner_radius: visual.corner_radius.unwrap_or(CornerRadius::ZERO),
                    border_width: EdgeInsets {
                        top: basic.border.top.into(),
                        right: basic.border.right.into(),
                        bottom: basic.border.bottom.into(),
                        left: basic.border.left.into(),
                    },
                    border_color: visual.border_color.unwrap_or(Color::TRANSPARENT),
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
                flush_batch(
                    &mut batches,
                    &mut current_instances,
                    &mut current_ids,
                    last_clip.unwrap_or(LayoutRect::ZERO),
                    current_batch_type,
                );

                let static_instance = QuadInstance {
                    rect,
                    transform: packed_transform,
                    transform_origin: origin,
                    corner_radius: visual.corner_radius.unwrap_or(CornerRadius::ZERO),
                    border_width: EdgeInsets {
                        top: basic.border.top.into(),
                        right: basic.border.right.into(),
                        bottom: basic.border.bottom.into(),
                        left: basic.border.left.into(),
                    },
                    border_color: visual.border_color.unwrap_or(Color::TRANSPARENT),
                    border_lengths: visual.border_lengths.unwrap_or(EdgeInsets::px_all(1.0)),
                    opacity_mode_sizing: [visual.opacity.unwrap_or(1.0), 0.0, 0.0, 0.0],
                    ..Default::default()
                };
                current_instances.push(static_instance);
                current_ids.push(id);

                flush_batch(
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
                    corner_radius: visual.corner_radius.unwrap_or(CornerRadius::ZERO),
                    border_width: EdgeInsets {
                        top: basic.border.top.into(),
                        right: basic.border.right.into(),
                        bottom: basic.border.bottom.into(),
                        left: basic.border.left.into(),
                    },
                    border_color: visual.border_color.unwrap_or(Color::TRANSPARENT),
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

                flush_batch(
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
                    flush_batch(
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
            if let Some(rects) = self.outputs.selected_rects.get(id) {
                let (border, padding) =
                    LayoutStore::get_physical_border_padding(rect, basic.border, basic.padding);

                let sel_bg = visual
                    .select_bg_color
                    .unwrap_or(Color::rgba_f32(0.0, 0.47, 0.84, 0.35));

                let scroll = self
                    .outputs
                    .scroll_offsets
                    .get(id)
                    .copied()
                    .unwrap_or(LayoutPoint::ZERO);

                let text_size = if let Some(contents) = self.contents.input_contents.get(id)
                    && let Some(layout_rect) = contents.last_layout
                {
                    LayoutSize::new(layout_rect.width, layout_rect.height)
                } else {
                    LayoutSize::ZERO
                };
                let (_, flex, _) = self.resolve_active_layouts(id);
                let content_w =
                    (rect.width - border.left - border.right - padding.left - padding.right)
                        .max(0.0);
                let align_offset_x = match flex.text_align {
                    TextAlign::Center => ((content_w - text_size.width) * 0.5).max(0.0),
                    TextAlign::Right => (content_w - text_size.width).max(0.0),
                    _ => 0.0,
                };
                let content_h =
                    (rect.height - border.top - border.bottom - padding.top - padding.bottom)
                        .max(0.0);
                let align_offset_y = ((content_h - text_size.height) * 0.5).max(0.0);

                for metric_rect in rects {
                    let sel_rect = LayoutRect::new(
                        rect.x + border.left + padding.left + align_offset_x + metric_rect.x
                            - scroll.x,
                        rect.y + border.top + padding.top + align_offset_y + metric_rect.y
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
            let is_text = self.topology.active_masks[id].has_text_content();
            let has_bg = visual.bg_color.is_some()
                || visual.bg_gradient.is_some()
                || visual.border_color.is_some()
                || visual.shadow_params.is_some();

            let box_sizing_val = match basic.box_sizing {
                BoxSizing::BorderBox => 0.0f32,
                BoxSizing::ContentBox => 1.0f32,
            };

            if is_text && has_bg {
                let bg_color = visual.bg_color.unwrap_or(Color::TRANSPARENT);
                let (gradient_end_color, gradient_angle, bg_mode) = match visual.bg_gradient {
                    Some(g) => (g.end_color, g.angle, 1.0f32),
                    None => (bg_color, 0.0, 0.0f32),
                };

                let bg_instance = QuadInstance {
                    rect,
                    transform: packed_transform,
                    transform_origin: origin,
                    color: bg_color,
                    corner_radius: visual.corner_radius.unwrap_or(CornerRadius::ZERO),
                    border_width: EdgeInsets {
                        top: basic.border.top.into(),
                        right: basic.border.right.into(),
                        bottom: basic.border.bottom.into(),
                        left: basic.border.left.into(),
                    },
                    border_color: visual.border_color.unwrap_or(Color::TRANSPARENT),
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
                visual.bg_color.unwrap_or(Color::TRANSPARENT)
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
                visual.border_color.unwrap_or(Color::TRANSPARENT)
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
                corner_radius: visual.corner_radius.unwrap_or(CornerRadius::ZERO),
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
            let is_input = self.topology.active_masks[id].has_input_content();
            let is_focused = self.events.interaction_states.focused == Some(id);

            if is_input
                && is_focused
                && let Some(contents) = self.contents.input_contents.get(id)
                && ContentStore::should_show_caret(contents)
            {
                let (border, padding) =
                    LayoutStore::get_physical_border_padding(rect, basic.border, basic.padding);
                let scale = self.window.scale_factor;
                let scroll = self
                    .outputs
                    .scroll_offsets
                    .get(id)
                    .copied()
                    .unwrap_or(LayoutPoint::ZERO);

                let text_size = if let Some(layout_rect) = contents.last_layout {
                    LayoutSize::new(layout_rect.width, layout_rect.height)
                } else {
                    LayoutSize::ZERO
                };
                let (_, flex, _) = self.resolve_active_layouts(id);
                let content_w =
                    (rect.width - border.left - border.right - padding.left - padding.right)
                        .max(0.0);
                let align_offset_x = match flex.text_align {
                    TextAlign::Center => ((content_w - text_size.width) * 0.5).max(0.0),
                    TextAlign::Right => (content_w - text_size.width).max(0.0),
                    _ => 0.0,
                };
                let content_h =
                    (rect.height - border.top - border.bottom - padding.top - padding.bottom)
                        .max(0.0);
                let align_offset_y = ((content_h - text_size.height) * 0.5).max(0.0);
                let align_offset = LayoutPoint {
                    x: align_offset_x,
                    y: align_offset_y,
                };

                let caret_rect = OutputStore::calculate_caret_rect(
                    rect,
                    border,
                    padding,
                    contents,
                    scale,
                    scroll,
                    align_offset,
                );
                let c_color = contents
                    .caret_color
                    .or(self
                        .renders
                        .base_visual_properties
                        .get(id)
                        .and_then(|v| v.text_color))
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
        flush_batch(
            &mut batches,
            &mut current_instances,
            &mut current_ids,
            last_clip.unwrap_or(LayoutRect::ZERO),
            current_batch_type,
        );

        RenderData { batches }
    }
}
