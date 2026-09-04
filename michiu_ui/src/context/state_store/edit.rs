use std::{ops::Range, rc::Rc};

use crate::{
    ActiveInteractionStates, ActiveMasksSecondary, ActiveTransitionsSparseSecondary,
    BaseVisualPropertiesSecondary, CapacityConfig, ChildrenSecondary, Color, ComponentMask,
    Context, DirtyLayoutEntitiesVec, DirtyRenderEntitiesVec, EdgeInsets, EntityId, EventStore,
    InputContents, InputContentsSparseSecondary, InteractionPropertiesSecondary, LayoutPoint,
    LayoutRect, LayoutSize, LayoutStore, OutputStore, ParentsSecondary, RectsSecondary,
    RenderStore, ResolvedBasicSecondary, ResolvedFlexSecondary, ResolvedGridSparseSecondary,
    ScrollOffsetsSecondary, ScrollSizesSecondary, ScrollStore, ScrollbarStylesSecondary,
    SystemStore, TaffyNodesSecondary, TaffyTreeEntityId, TextContentsSparseSecondary, TextEngine,
    TextLayoutEngine, TextLayoutEngineSparseSecondary, TextSpansSparseSecondary, TopologyStore,
    UserSelect, VisualPropertiesSecondary,
};
use slotmap::SparseSecondaryMap;
use windows::Win32::Graphics::DirectWrite::{DWRITE_HIT_TEST_METRICS, IDWriteTextLayout};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InputOp {
    // 初期化用
    Init,
    // シグナル監視エフェクト同期用
    TextEffect,
    MousePress,
    ArrowMove,
    SelectAll,
    CharTyped,
    Backspace,
    Delete,
    ImeUpdated,
    Paste,
    Cut,
    Undo,
    Redo,
}

pub(crate) type SelectedRectsSparseSecondary = SparseSecondaryMap<EntityId, Vec<LayoutRect>>;
pub(crate) type TextSelectionsSparseSecondary = SparseSecondaryMap<EntityId, Range<usize>>;
pub(crate) type SelectionStartIndexSparseSecondary = SparseSecondaryMap<EntityId, usize>;

pub(crate) struct TextEditStore {
    pub(crate) edit_selections: TextSelectionsSparseSecondary,
    pub(crate) edit_selection_start_index: SelectionStartIndexSparseSecondary,
    pub(crate) edit_selected_rects: SelectedRectsSparseSecondary,
}

impl Default for TextEditStore {
    fn default() -> Self {
        Self::new()
    }
}

impl TextEditStore {
    #[inline]
    #[must_use]
    pub fn new() -> Self {
        Self {
            edit_selections: SparseSecondaryMap::new(),
            edit_selection_start_index: SparseSecondaryMap::new(),
            edit_selected_rects: SparseSecondaryMap::new(),
        }
    }

    #[inline]
    #[must_use]
    pub fn with_capacity(c: &CapacityConfig) -> Self {
        Self {
            edit_selections: SparseSecondaryMap::with_capacity(c.edit_selections),
            edit_selected_rects: SparseSecondaryMap::with_capacity(c.edit_selected_rects),
            edit_selection_start_index: SparseSecondaryMap::with_capacity(
                c.edit_selection_start_index,
            ),
        }
    }

    #[inline]
    pub fn clear(&mut self) {
        self.edit_selections.clear();
        self.edit_selection_start_index.clear();
        self.edit_selected_rects.clear();
    }

    #[inline]
    pub fn despawn(&mut self, id: EntityId) {
        self.edit_selections.remove(id);
        self.edit_selection_start_index.remove(id);
        self.edit_selected_rects.remove(id);
    }
}

impl TextEditStore {
    #[inline]
    pub(crate) fn truncate_unconfirmed_text(
        text_val: &str,
        contents: &InputContents,
        max: usize,
        filtered_comp_text: String,
        cosmic: bool,
    ) -> String {
        let range = &contents.selected_range;

        if cosmic {
            // キャレット範囲をクランプ
            let mut start = range.start.min(text_val.len());
            let mut end = range.end.min(text_val.len());

            while start > 0 && !text_val.is_char_boundary(start) {
                start -= 1;
            }
            while end > 0 && !text_val.is_char_boundary(end) {
                end -= 1;
            }

            // 選択範囲が削除された後の確定テキストの文字数
            let left_chars = text_val[..start].chars().count();
            let right_chars = text_val[end..].chars().count();
            let current_len_after_delete = left_chars + right_chars;

            if current_len_after_delete >= max {
                // すでに確定文字数が制限に達している場合は未確定文字を一切受け入れない
                String::new()
            } else {
                let allowed_comp_len = max - current_len_after_delete;
                let comp_char_count = filtered_comp_text.chars().count();

                if comp_char_count > allowed_comp_len {
                    filtered_comp_text.chars().take(allowed_comp_len).collect()
                } else {
                    filtered_comp_text
                }
            }
        } else {
            let text_u16: Vec<u16> = text_val.encode_utf16().collect();

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
    }

    #[inline]
    pub(crate) fn clear_selection_highlight_rect(
        id: EntityId,
        topo_active_masks: &mut ActiveMasksSecondary,
        cont_input_contents: &mut InputContentsSparseSecondary,
        cont_text_spans: &mut TextSpansSparseSecondary,
        edit_selections: &mut TextSelectionsSparseSecondary,
        edit_selected_rects: &mut SelectedRectsSparseSecondary,
    ) {
        edit_selections.remove(id);
        edit_selected_rects.remove(id);
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

    pub(crate) fn calc_selection_rects(
        id: EntityId,
        engine: &TextLayoutEngine,
        range: Range<usize>,
    ) -> Vec<LayoutRect> {
        match engine {
            TextLayoutEngine::Cosmic(buffer) => {
                // 範囲が逆転している場合の対策
                let (start_idx, end_idx) = if range.start <= range.end {
                    (range.start, range.end)
                } else {
                    (range.end, range.start)
                };

                // フラットなバイト位置から cosmic-text の Cursor への変換用クロージャ
                let flat_idx_to_cursor = |flat_idx: usize| -> cosmic_text::Cursor {
                    let mut accum = 0;
                    for (line_idx, line) in buffer.lines.iter().enumerate() {
                        let line_len = line.text().len();
                        // インデックスが現在の段落内にあるか
                        if flat_idx >= accum && flat_idx <= accum + line_len {
                            return cosmic_text::Cursor {
                                line: line_idx,
                                index: flat_idx - accum,
                                affinity: cosmic_text::Affinity::Before,
                            };
                        }
                        accum += line_len + 1; // 各段落の終わりの '\n' の分
                    }
                    // 範囲外だった場合のフォールバック
                    let last_line = buffer.lines.len().saturating_sub(1);
                    let last_line_len = buffer
                        .lines
                        .get(last_line)
                        .map(|l| l.text().len())
                        .unwrap_or(0);
                    cosmic_text::Cursor {
                        line: last_line,
                        index: last_line_len,
                        affinity: cosmic_text::Affinity::Before,
                    }
                };

                let cursor_start = flat_idx_to_cursor(start_idx);
                let cursor_end = flat_idx_to_cursor(end_idx);

                let mut out_rects = Vec::new();

                // 各行をループしてハイライト領域を計算
                for run in buffer.layout_runs() {
                    for (x, w) in run.highlight(cursor_start, cursor_end) {
                        out_rects.push(LayoutRect::new(
                            x,               // 選択範囲の左端
                            run.line_top,    // 行の上端のY座標
                            w,               // 選択範囲の幅
                            run.line_height, // 行の高さ
                        ));
                    }
                }

                out_rects
            }
            TextLayoutEngine::DWrite(dw_layout) => {
                let mut hit_test_metrics = vec![DWRITE_HIT_TEST_METRICS::default(); 16];
                let mut actual_count: u32 = 0;
                let res = unsafe {
                    dw_layout.HitTestTextRange(
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
                    hit_test_metrics
                        .resize(actual_count as usize, DWRITE_HIT_TEST_METRICS::default());
                    let _ = unsafe {
                        dw_layout.HitTestTextRange(
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
        }
    }

    /// 現在の選択範囲に基づき描画用の物理選択矩形を計算してキャッシュを更新
    #[inline]
    pub(crate) fn update_selection_rects(
        id: EntityId,
        engine: &TextLayoutEngine,
        edit_selected_rects: &mut SelectedRectsSparseSecondary,
        edit_selections: &TextSelectionsSparseSecondary,
    ) {
        if let Some(range) = edit_selections.get(id).cloned()
            && range.start < range.end
        {
            let out_rects = TextEditStore::calc_selection_rects(id, engine, range);
            edit_selected_rects.insert(id, out_rects);
            return;
        }
        // 範囲が 0、または選択なしの時は自動クリーンアップ
        edit_selected_rects.remove(id);
    }

    /// 現在フォーカスされている要素で範囲選択されている文字列を取得します。
    pub(crate) fn get_selected_text(
        evt_interaction_states: &ActiveInteractionStates,
        cont_text_contents: &TextContentsSparseSecondary,
        rnd_visual: &VisualPropertiesSecondary,
        edit_selections: &TextSelectionsSparseSecondary,
        cosmic: bool,
    ) -> Option<String> {
        // フォーカスされている要素を最優先とし、
        // 無い場合は現在有効な空ではない選択範囲を持つ最初の要素を逆引き
        let target_id = evt_interaction_states.focused.or_else(|| {
            edit_selections
                .iter()
                .find(|(_, range)| range.start < range.end)
                .map(|(id, _)| id)
        })?;

        let user_select = rnd_visual
            .get(target_id)
            .and_then(|v| v.user_select)
            .unwrap_or_default();

        if user_select == UserSelect::Text {
            let range = edit_selections.get(target_id)?;
            if range.start < range.end {
                let text = cont_text_contents.get(target_id)?;

                if cosmic {
                    let mut start = range.start.min(text.len());
                    let mut end = range.end.min(text.len());

                    while start > 0 && !text.is_char_boundary(start) {
                        start -= 1;
                    }
                    while end > 0 && !text.is_char_boundary(end) {
                        end -= 1;
                    }

                    return Some(text[start..end].to_string());
                }
                let u16_text: Vec<u16> = text.encode_utf16().collect();
                let slice =
                    &u16_text[range.start.min(u16_text.len())..range.end.min(u16_text.len())];
                return String::from_utf16(slice).ok();
            }
        }
        None
    }

    #[inline]
    pub(crate) fn calculate_text_selection(
        start_pos: usize,
        local: LayoutPoint,
        engine: &TextLayoutEngine,
        sys_text_engine: &mut TextEngine,
    ) -> (std::ops::Range<usize>, bool) {
        let (current_index, is_trailing) = match engine {
            TextLayoutEngine::Cosmic(buffer) => {
                sys_text_engine.hit_test_point_cosmic(buffer, local.x, local.y)
            }
            TextLayoutEngine::DWrite(dw_layout) => {
                sys_text_engine.hit_test_point(dw_layout, local.x, local.y)
            }
        };

        let final_index = if is_trailing {
            current_index + 1
        } else {
            current_index
        };

        if start_pos <= final_index {
            (start_pos..final_index, false)
        } else {
            (final_index..start_pos, true)
        }
    }

    pub(crate) fn handle_user_select_text(
        id: EntityId,
        pointer_pos: LayoutPoint,
        pressed_shift: bool,
        sys_text_engine: &mut TextEngine,
        sys_dwrite_layouts: &TextLayoutEngineSparseSecondary,
        cont_text_contents: &TextContentsSparseSecondary,
        cont_text_spans: &TextSpansSparseSecondary,
        cont_input_contents: &InputContentsSparseSecondary,
        topo_active_masks: &mut ActiveMasksSecondary,
        topo_parents: &ParentsSecondary,
        lay_resolved_basic: &ResolvedBasicSecondary,
        lay_resolved_flex: &ResolvedFlexSecondary,
        lay_resolved_grid: &ResolvedGridSparseSecondary,
        rnd_dirty_entities: &mut DirtyRenderEntitiesVec,
        rnd_visual: &VisualPropertiesSecondary,
        rnd_interaction: &InteractionPropertiesSecondary,
        rnd_active_transitions: &ActiveTransitionsSparseSecondary,
        edit_selections: &mut TextSelectionsSparseSecondary,
        edit_selection_start_index: &mut SelectionStartIndexSparseSecondary,
        edit_selected_rects: &mut SelectedRectsSparseSecondary,
        out_rects: &RectsSecondary,
        sc_offsets: &ScrollOffsetsSecondary,
        cosmic: bool,
    ) {
        let Some(engine) = SystemStore::get_or_create_layout(
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
        ) else {
            return;
        };

        let local = OutputStore::pressed_local_point(
            id,
            pointer_pos,
            Some(&engine),
            sys_text_engine,
            cont_input_contents,
            topo_active_masks,
            topo_parents,
            lay_resolved_basic,
            lay_resolved_flex,
            lay_resolved_grid,
            rnd_interaction,
            rnd_active_transitions,
            rnd_visual,
            out_rects,
            sc_offsets,
        );
        let (clicked_index, is_trailing) = match &engine {
            TextLayoutEngine::Cosmic(buffer) => {
                sys_text_engine.hit_test_point_cosmic(buffer, local.x, local.y)
            }
            TextLayoutEngine::DWrite(dw_layout) => {
                sys_text_engine.hit_test_point(dw_layout, local.x, local.y)
            }
        };

        let final_index = if is_trailing {
            clicked_index + 1
        } else {
            clicked_index
        };

        if pressed_shift {
            // 共通の Shift選択拡張
            let anchor = edit_selection_start_index
                .get(id)
                .copied()
                .unwrap_or(final_index);
            if !edit_selection_start_index.contains_key(id) {
                edit_selection_start_index.insert(id, final_index);
            }
            let range = if anchor <= final_index {
                anchor..final_index
            } else {
                final_index..anchor
            };
            edit_selections.insert(id, range);
            TextEditStore::update_selection_rects(
                id,
                &engine,
                edit_selected_rects,
                edit_selections,
            );
        } else {
            // 共通の通常クリックリセット
            edit_selection_start_index.insert(id, final_index);
            edit_selections.insert(id, final_index..final_index);
            edit_selected_rects.remove(id);
        }

        RenderStore::mark_render_dirty(id, topo_active_masks, rnd_dirty_entities);
    }

    pub(crate) fn handle_text_selection_click(
        id: EntityId,
        start_pos: usize,
        local: LayoutPoint,
        win_scale_factor: f32,
        win_last_size: Option<LayoutSize>,
        sys_text_engine: &mut TextEngine,
        sys_dwrite_layouts: &TextLayoutEngineSparseSecondary,
        cont_text_contents: &mut TextContentsSparseSecondary,
        cont_input_contents: &mut InputContentsSparseSecondary,
        cont_text_spans: &TextSpansSparseSecondary,
        topo_active_masks: &mut ActiveMasksSecondary,
        topo_parents: &ParentsSecondary,
        topo_children: &ChildrenSecondary,
        lay_dirty_entities: &mut DirtyLayoutEntitiesVec,
        lay_taffy_tree: &mut TaffyTreeEntityId,
        bar_styles: &mut ScrollbarStylesSecondary,
        lay_taffy_nodes: &TaffyNodesSecondary,
        lay_resolved_basic: &ResolvedBasicSecondary,
        lay_resolved_flex: &ResolvedFlexSecondary,
        lay_resolved_grid: &ResolvedGridSparseSecondary,
        rnd_dirty_entities: &mut DirtyRenderEntitiesVec,
        rnd_visual: &mut VisualPropertiesSecondary,
        rnd_base_visual: &BaseVisualPropertiesSecondary,
        rnd_interaction: &InteractionPropertiesSecondary,
        rnd_active_transitions: &ActiveTransitionsSparseSecondary,
        sc_offsets: &mut ScrollOffsetsSecondary,
        edit_selections: &mut TextSelectionsSparseSecondary,
        edit_selected_rects: &mut SelectedRectsSparseSecondary,
        out_rects: &RectsSecondary,
        sc_sizes: &ScrollSizesSecondary,
        cosmic: bool,
    ) {
        let Some(engine) = SystemStore::get_or_create_layout(
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
        ) else {
            return;
        };

        let (range, is_reversed) =
            TextEditStore::calculate_text_selection(start_pos, local, &engine, sys_text_engine);

        edit_selections.insert(id, range.clone());

        TextEditStore::update_selection_rects(id, &engine, edit_selected_rects, edit_selections);

        if let Some(contents) = cont_input_contents.get_mut(id) {
            contents.selection_reversed = is_reversed;
            contents.selected_range = range;

            TextEditStore::apply_input_update(
                id,
                InputOp::MousePress,
                win_scale_factor,
                win_last_size,
                sys_text_engine,
                sys_dwrite_layouts,
                cont_text_contents,
                cont_input_contents,
                cont_text_spans,
                topo_active_masks,
                topo_parents,
                lay_dirty_entities,
                lay_taffy_tree,
                bar_styles,
                lay_taffy_nodes,
                lay_resolved_basic,
                lay_resolved_flex,
                lay_resolved_grid,
                rnd_dirty_entities,
                rnd_visual,
                rnd_base_visual,
                rnd_interaction,
                rnd_active_transitions,
                sc_offsets,
                edit_selections,
                edit_selected_rects,
                out_rects,
                sc_sizes,
                cosmic,
            );
        } else {
            RenderStore::mark_render_dirty(id, topo_active_masks, rnd_dirty_entities);
        }
    }

    /// 指定されたテキスト要素の内容をすべて選択状態に
    pub(crate) fn handle_select_all(
        id: EntityId,
        win_scale_factor: f32,
        win_last_size: Option<LayoutSize>,
        sys_text_engine: &mut TextEngine,
        sys_dwrite_layouts: &TextLayoutEngineSparseSecondary,
        cont_text_contents: &mut TextContentsSparseSecondary,
        cont_input_contents: &mut InputContentsSparseSecondary,
        cont_text_spans: &TextSpansSparseSecondary,
        topo_active_masks: &mut ActiveMasksSecondary,
        topo_parents: &ParentsSecondary,
        topo_children: &ChildrenSecondary,
        lay_dirty_entities: &mut DirtyLayoutEntitiesVec,
        lay_taffy_tree: &mut TaffyTreeEntityId,
        bar_styles: &mut ScrollbarStylesSecondary,
        lay_taffy_nodes: &TaffyNodesSecondary,
        lay_resolved_basic: &ResolvedBasicSecondary,
        lay_resolved_flex: &ResolvedFlexSecondary,
        lay_resolved_grid: &ResolvedGridSparseSecondary,
        rnd_dirty_entities: &mut DirtyRenderEntitiesVec,
        rnd_visual: &mut VisualPropertiesSecondary,
        rnd_base_visual: &BaseVisualPropertiesSecondary,
        rnd_interaction: &InteractionPropertiesSecondary,
        rnd_active_transitions: &ActiveTransitionsSparseSecondary,
        sc_offsets: &mut ScrollOffsetsSecondary,
        edit_selections: &mut TextSelectionsSparseSecondary,
        edit_selected_rects: &mut SelectedRectsSparseSecondary,
        out_rects: &RectsSecondary,
        sc_sizes: &ScrollSizesSecondary,
        cosmic: bool,
    ) {
        let Some(engine) = SystemStore::get_or_create_layout(
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
        ) else {
            return;
        };

        let Some(text) = cont_text_contents.get(id) else {
            return;
        };

        let text_len = if cosmic {
            text.len()
        } else {
            text.encode_utf16().count()
        };
        let full_range = 0..text_len;

        edit_selections.insert(id, full_range.clone());

        TextEditStore::update_selection_rects(id, &engine, edit_selected_rects, edit_selections);

        if let Some(contents) = cont_input_contents.get_mut(id) {
            contents.selected_range = full_range;
            contents.selection_reversed = false;
            TextEditStore::apply_input_update(
                id,
                InputOp::SelectAll,
                win_scale_factor,
                win_last_size,
                sys_text_engine,
                sys_dwrite_layouts,
                cont_text_contents,
                cont_input_contents,
                cont_text_spans,
                topo_active_masks,
                topo_parents,
                lay_dirty_entities,
                lay_taffy_tree,
                bar_styles,
                lay_taffy_nodes,
                lay_resolved_basic,
                lay_resolved_flex,
                lay_resolved_grid,
                rnd_dirty_entities,
                rnd_visual,
                rnd_base_visual,
                rnd_interaction,
                rnd_active_transitions,
                sc_offsets,
                edit_selections,
                edit_selected_rects,
                out_rects,
                sc_sizes,
                cosmic,
            );
        } else {
            RenderStore::mark_render_dirty(id, topo_active_masks, rnd_dirty_entities);
        }
    }

    pub(crate) fn apply_input_update(
        id: EntityId,
        op: InputOp,
        win_scale_factor: f32,
        win_last_size: Option<LayoutSize>,
        sys_text_engine: &mut TextEngine,
        sys_dwrite_layouts: &TextLayoutEngineSparseSecondary,
        cont_text_contents: &mut TextContentsSparseSecondary,
        cont_input_contents: &mut InputContentsSparseSecondary,
        cont_text_spans: &TextSpansSparseSecondary,
        topo_active_masks: &mut ActiveMasksSecondary,
        topo_parents: &ParentsSecondary,
        lay_dirty_entities: &mut DirtyLayoutEntitiesVec,
        lay_taffy_tree: &mut TaffyTreeEntityId,
        bar_styles: &mut ScrollbarStylesSecondary,
        lay_taffy_nodes: &TaffyNodesSecondary,
        lay_resolved_basic: &ResolvedBasicSecondary,
        lay_resolved_flex: &ResolvedFlexSecondary,
        lay_resolved_grid: &ResolvedGridSparseSecondary,
        rnd_dirty_entities: &mut DirtyRenderEntitiesVec,
        rnd_visual: &mut VisualPropertiesSecondary,
        rnd_base_visual: &BaseVisualPropertiesSecondary,
        rnd_interaction: &InteractionPropertiesSecondary,
        rnd_active_transitions: &ActiveTransitionsSparseSecondary,
        sc_offsets: &mut ScrollOffsetsSecondary,
        edit_selections: &mut TextSelectionsSparseSecondary,
        edit_selected_rects: &mut SelectedRectsSparseSecondary,
        out_rects: &RectsSecondary,
        sc_sizes: &ScrollSizesSecondary,
        cosmic: bool,
    ) {
        // 直前までハイライトが描画されていたか
        let has_selection_before = edit_selected_rects.contains_key(id);

        // 現在（操作後）に範囲選択されているか
        let has_selection_after = cont_input_contents
            .get(id)
            .is_some_and(|c| c.selected_range.start != c.selected_range.end);

        // 選択範囲の描画を更新
        if (has_selection_before || has_selection_after)
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
            TextEditStore::update_selection_rects(
                id,
                &engine,
                edit_selected_rects,
                edit_selections,
            );
        }

        match op {
            // コンテンツのサイズに変動がない（Taffyレイアウトの再計算が不要）操作
            InputOp::MousePress | InputOp::ArrowMove | InputOp::SelectAll => {
                TextEditStore::update_input_caret_position(
                    id,
                    win_scale_factor,
                    win_last_size,
                    sys_text_engine,
                    sys_dwrite_layouts,
                    cont_text_contents,
                    cont_input_contents,
                    cont_text_spans,
                    topo_active_masks,
                    topo_parents,
                    lay_dirty_entities,
                    lay_taffy_tree,
                    bar_styles,
                    lay_taffy_nodes,
                    lay_resolved_basic,
                    lay_resolved_flex,
                    lay_resolved_grid,
                    rnd_visual,
                    rnd_base_visual,
                    rnd_interaction,
                    rnd_active_transitions,
                    sc_offsets,
                    edit_selections,
                    out_rects,
                    sc_sizes,
                    cosmic,
                );
                RenderStore::mark_render_dirty(id, topo_active_masks, rnd_dirty_entities);
            }

            // Taffy計算前に表示用テキストの同期が必須
            // 未確定文字の伸縮時はシグナルが更新されないため ImeUpdated を含める
            InputOp::Init | InputOp::TextEffect | InputOp::ImeUpdated => {
                TextEditStore::update_input_caret_position(
                    id,
                    win_scale_factor,
                    win_last_size,
                    sys_text_engine,
                    sys_dwrite_layouts,
                    cont_text_contents,
                    cont_input_contents,
                    cont_text_spans,
                    topo_active_masks,
                    topo_parents,
                    lay_dirty_entities,
                    lay_taffy_tree,
                    bar_styles,
                    lay_taffy_nodes,
                    lay_resolved_basic,
                    lay_resolved_flex,
                    lay_resolved_grid,
                    rnd_visual,
                    rnd_base_visual,
                    rnd_interaction,
                    rnd_active_transitions,
                    sc_offsets,
                    edit_selections,
                    out_rects,
                    sc_sizes,
                    cosmic,
                );
                TopologyStore::mark_dirty(
                    id,
                    topo_active_masks,
                    topo_parents,
                    lay_dirty_entities,
                    lay_taffy_tree,
                    lay_taffy_nodes,
                    rnd_dirty_entities,
                );
            }

            // シグナルエフェクト側で update_input_caret_position が走るが、
            // 不具合によってエフェクト自体がスキップされることも考慮してフラグを立てる
            InputOp::CharTyped
            | InputOp::Backspace
            | InputOp::Delete
            | InputOp::Paste
            | InputOp::Cut
            | InputOp::Undo
            | InputOp::Redo => {
                TopologyStore::mark_dirty(
                    id,
                    topo_active_masks,
                    topo_parents,
                    lay_dirty_entities,
                    lay_taffy_tree,
                    lay_taffy_nodes,
                    rnd_dirty_entities,
                );
            }
        }
    }

    /// 現在のテキスト・IME状態・フォントサイズから、
    /// キャレットの物理座標や最終表示テキスト、レイアウト矩形を正確に再計算して `SoA` を更新。
    pub(crate) fn update_input_caret_position(
        id: EntityId,
        win_scale_factor: f32,
        win_last_size: Option<LayoutSize>,
        sys_text_engine: &mut TextEngine,
        sys_dwrite_layouts: &TextLayoutEngineSparseSecondary,
        cont_text_contents: &mut TextContentsSparseSecondary,
        cont_input_contents: &mut InputContentsSparseSecondary,
        cont_text_spans: &TextSpansSparseSecondary,
        topo_active_masks: &mut ActiveMasksSecondary,
        topo_parents: &ParentsSecondary,
        lay_dirty_entities: &mut DirtyLayoutEntitiesVec,
        lay_taffy_tree: &mut TaffyTreeEntityId,
        bar_styles: &mut ScrollbarStylesSecondary,
        lay_taffy_nodes: &TaffyNodesSecondary,
        lay_resolved_basic: &ResolvedBasicSecondary,
        lay_resolved_flex: &ResolvedFlexSecondary,
        lay_resolved_grid: &ResolvedGridSparseSecondary,
        rnd_visual: &mut VisualPropertiesSecondary,
        rnd_base_visual: &BaseVisualPropertiesSecondary,
        rnd_interaction: &InteractionPropertiesSecondary,
        rnd_active_transitions: &ActiveTransitionsSparseSecondary,
        sc_offsets: &mut ScrollOffsetsSecondary,
        edit_selections: &mut TextSelectionsSparseSecondary,
        out_rects: &RectsSecondary,
        sc_sizes: &ScrollSizesSecondary,
        cosmic: bool,
    ) {
        // IMEやタイピング中の古いキャッシュを破棄
        SystemStore::clear_layout_cache(id, sys_dwrite_layouts);

        let ime_caret_info = TextEditStore::ime_caret_info(
            id,
            sys_text_engine,
            sys_dwrite_layouts,
            cont_text_contents,
            cont_input_contents,
            cont_text_spans,
            lay_resolved_basic,
            lay_resolved_flex,
            rnd_visual,
            rnd_base_visual,
            edit_selections,
            out_rects,
            cosmic,
        );

        let Some((caret, caret_offset, is_multiline)) = ime_caret_info else {
            return;
        };
        let basic = lay_resolved_basic.get(id).copied().unwrap_or_default();
        let flex = lay_resolved_flex.get(id).copied().unwrap_or_default();
        let _grid = lay_resolved_grid.get(id).cloned().unwrap_or_default();
        let rect = out_rects.get(id).copied().unwrap_or_default();
        let (border, padding) =
            LayoutStore::get_physical_border_padding(rect, basic.border, basic.padding);

        let mut scroll = sc_offsets.get(id).copied().unwrap_or_default();

        if rect.width > 0.0 && rect.height > 0.0 {
            let viewport = OutputStore::calc_viewport_size(rect, border, padding);

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

            ScrollStore::scroll_to(
                id,
                scroll.x,
                scroll.y,
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
    pub(crate) fn ime_caret_info(
        id: EntityId,
        sys_text_engine: &mut TextEngine,
        sys_dwrite_layouts: &TextLayoutEngineSparseSecondary,
        cont_text_contents: &mut TextContentsSparseSecondary,
        cont_input_contents: &mut InputContentsSparseSecondary,
        cont_text_spans: &TextSpansSparseSecondary,
        lay_resolved_basic: &ResolvedBasicSecondary,
        lay_resolved_flex: &ResolvedFlexSecondary,
        rnd_visual: &mut VisualPropertiesSecondary,
        rnd_base_visual: &BaseVisualPropertiesSecondary,
        edit_selections: &mut TextSelectionsSparseSecondary,
        out_rects: &RectsSecondary,
        cosmic: bool,
    ) -> Option<(LayoutRect, f32, bool)> {
        let contents = cont_input_contents.get_mut(id)?;
        // 入力エンジン側の最新カーソル位置を描画SoA側に同期
        edit_selections.insert(id, contents.selected_range.clone());

        let text_val = contents.text.0.get();

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
            filtered_comp_text = TextEditStore::truncate_unconfirmed_text(
                &text_val,
                contents,
                max,
                filtered_comp_text,
                cosmic,
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
            let char_count = text_val.chars().count();
            mask.repeat(char_count)
        } else {
            text_val.clone()
        };

        // 描画表示用テキスト（IME未確定文字列の有無を最優先で判定）
        let display_text = if !filtered_comp_text.is_empty() {
            if cosmic {
                InputContents::input_get_display_text_utf8_byte(
                    &text_val_for_display,
                    contents.selected_range.start,
                    &filtered_comp_text,
                )
            } else {
                InputContents::input_get_display_text(
                    &text_val_for_display,
                    contents.selected_range.start,
                    &filtered_comp_text,
                )
            }
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

        let engine = SystemStore::get_or_create_layout(
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
        )?;

        let text_size = match &engine {
            TextLayoutEngine::Cosmic(buffer) => sys_text_engine.get_layout_size_cosmic(buffer),
            TextLayoutEngine::DWrite(dw_layout) => sys_text_engine.get_layout_size(dw_layout),
        };
        contents.last_layout = Some(LayoutRect::new(0.0, 0.0, text_size.width, text_size.height));

        let composition_offset = if let Some(ref ime) = contents.ime_state
            && !ime.composition_text.is_empty()
        {
            // 組成文字全体の文字数をオフセットとして適用
            if cosmic {
                ime.composition_text.len()
            } else {
                ime.composition_text.encode_utf16().count()
            }
        } else {
            0
        };

        // ドラッグの方向を判定しマウス位置にキャレットを固定
        let current_caret_relative = if contents.selection_reversed {
            contents.selected_range.start // 逆方向（左ドラッグ）時は左端がマウス位置
        } else {
            contents.selected_range.end // 順方向（右ドラッグ）時は右端がマウス位置
        };

        let caret_index = if contents.is_password {
            let mask = contents.mask_text.as_deref().unwrap_or("●");

            // 確定テキストの現在のキャレット位置までの文字数
            let mut safe_caret = current_caret_relative.min(text_val.len());
            while safe_caret > 0 && !text_val.is_char_boundary(safe_caret) {
                safe_caret -= 1;
            }
            let base_char_count = text_val[..safe_caret].chars().count();

            // 未確定テキストの文字数
            let comp_char_count = if let Some(ref ime) = contents.ime_state {
                ime.composition_text.chars().count()
            } else {
                0
            };

            let total_char_caret = base_char_count + comp_char_count;
            if cosmic {
                total_char_caret * mask.len()
            } else {
                total_char_caret * mask.encode_utf16().count()
            }
        } else {
            current_caret_relative + composition_offset
        };

        let display_text_len = if cosmic {
            display_text.len()
        } else {
            display_text.encode_utf16().count()
        };

        // プレースホルダーに干渉されない純粋なキャレット位置を算出
        let (cx_offset, cy_offset, ch_height) = match &engine {
            TextLayoutEngine::Cosmic(buffer) => {
                sys_text_engine.get_caret_position_cosmic(buffer, caret_index, display_text_len)
            }
            TextLayoutEngine::DWrite(dw_layout) => {
                sys_text_engine.get_caret_position(dw_layout, caret_index, display_text_len)
            }
        };

        contents.measured_caret_x = cx_offset;
        contents.measured_caret_y = cy_offset;
        contents.caret_line_height = ch_height;

        let (curr_line, tot_lines) = if cosmic {
            InputContents::calculate_line_indices_utf8_byte(&display_text, caret_index)
        } else {
            InputContents::calculate_line_indices(&display_text, caret_index)
        };
        contents.current_line_index = curr_line;

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
}

impl Context {
    #[inline]
    pub(crate) fn apply_input_update(&mut self, id: EntityId, op: InputOp) {
        TextEditStore::apply_input_update(
            id,
            op,
            self.window.win_scale_factor,
            self.window.win_last_size,
            &mut self.system.sys_text_engine,
            &self.system.sys_dwrite_layouts,
            &mut self.contents.cont_text_contents,
            &mut self.contents.cont_input_contents,
            &self.contents.cont_text_spans,
            &mut self.topology.topo_active_masks,
            &self.topology.topo_parents,
            &mut self.layouts.lay_dirty_entities,
            &mut self.layouts.lay_taffy_tree,
            &mut self.layouts.scrollbar.bar_styles,
            &self.layouts.lay_taffy_nodes,
            &self.layouts.lay_resolved_basic,
            &self.layouts.lay_resolved_flex,
            &self.layouts.lay_resolved_grid,
            &mut self.renders.rnd_dirty_entities,
            &mut self.renders.rnd_visual,
            &self.renders.rnd_base_visual,
            &self.renders.rnd_interaction,
            &self.renders.rnd_active_transitions,
            &mut self.states.scroll.sc_offsets,
            &mut self.states.edit.edit_selections,
            &mut self.states.edit.edit_selected_rects,
            &self.outputs.out_rects,
            &self.states.scroll.sc_sizes,
            self.cosmic,
        );
    }
}
