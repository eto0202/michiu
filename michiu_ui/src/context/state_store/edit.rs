use std::{ops::Range, rc::Rc};

use crate::{
    ActiveInteractionStates, ActiveMasksSecondary, ActiveTransitionsSparseSecondary,
    BaseVisualPropertiesSecondary, ByteIndex, CapacityConfig, CharIndex, ChildrenSecondary, Color,
    ComponentMask, Context, DirtyLayoutEntitiesVec, DirtyRenderEntitiesVec, EdgeInsets, EntityId,
    EventStore, InputContents, InputContentsSparse, InteractionPropertiesSecondary, LayoutPoint,
    LayoutRect, LayoutSize, LayoutStore, MichiuSoA, MichiuString, OutputStore, ParentsSecondary,
    RangeExt, RectsSecondary, RenderStore, ResolvedBasicSecondary, ResolvedFlexSecondary,
    ResolvedGridSparse, ScrollOffsetsSecondary, ScrollSizesSecondary, ScrollStore,
    ScrollbarStylesSecondary, SystemStore, TaffyNodesSecondary, TaffyTreeEntityId,
    TextBufferSparseSecondary, TextContentsSparse, TextEngine, TextSpansSparse, TopologyStore,
    UserSelect, UsizeRangeExt, VisualPropertiesSecondary,
};
use cosmic_text::Buffer;
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
pub(crate) type TextSelectionsSparseSecondary = SparseSecondaryMap<EntityId, Range<ByteIndex>>;
pub(crate) type SelectionStartIndexSparseSecondary = SparseSecondaryMap<EntityId, ByteIndex>;

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
        text_val: &MichiuString,
        contents: &InputContents,
        max: CharIndex,
        filtered_comp_text: MichiuString,
    ) -> MichiuString {
        let range = &contents.selected_range;

        // 選択範囲削除後の文字数
        let selected_chars = text_val.slice(range.clone()).chars().count();
        let current_len_after_delete = text_val.char_count() - selected_chars;

        if current_len_after_delete >= max {
            // すでに確定文字数が制限に達している場合は空の MichiuString を返す
            MichiuString::default()
        } else {
            // 許容文字数
            let allowed_chars = max - current_len_after_delete;

            if filtered_comp_text.char_count().0 > allowed_chars {
                // 制限に収まるよう安全に文字数で切り詰め
                let truncated: String = filtered_comp_text.chars().take(allowed_chars).collect();
                MichiuString::from(truncated)
            } else {
                filtered_comp_text
            }
        }
    }

    #[inline]
    pub(crate) fn clear_selection_highlight_rect(
        id: EntityId,
        topo_active_masks: &mut ActiveMasksSecondary,
        cont_input_contents: &mut InputContentsSparse,
        cont_text_spans: &mut TextSpansSparse,
        edit_selections: &mut TextSelectionsSparseSecondary,
        edit_selected_rects: &mut SelectedRectsSparseSecondary,
    ) {
        edit_selections.remove(id);
        edit_selected_rects.remove(id);
        if let Some(contents) = cont_input_contents.get_mut(id) {
            contents.selected_range = ByteIndex(0)..ByteIndex(0);
            // 進行中の IME コンポジションをリセットして波線を消去
            contents.ime_state = None;
            contents.marked_range = None;
        }
        cont_text_spans.remove(id);
        let mask = topo_active_masks.at_mut(id);
        mask.unset(ComponentMask::STYLE_TEXT_SPANS);
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
            rect.x + border.left + padding.left + align_offset.x + contents.measured_caret.x
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
            + contents.measured_caret.y
            + contents.caret_offset
            - scroll.y;

        let aligned_y = ((logical_y + vertical_center_offset) * scale).round() / scale;
        let aligned_width = (caret_width * scale).round().max(1.0) / scale;
        let aligned_height = (caret_height * scale).round().max(1.0) / scale;

        LayoutRect::new(aligned_x, aligned_y, aligned_width, aligned_height)
    }

    pub(crate) fn calc_selection_rects(
        id: EntityId,
        buffer: &Buffer,
        range: Range<ByteIndex>,
    ) -> Vec<LayoutRect> {
        // 範囲が空（選択なし）の場合は即座に空Vecを返す
        if range.start == range.end {
            return Vec::new();
        }

        let (start_idx, end_idx) = if range.start <= range.end {
            (range.start, range.end)
        } else {
            (range.end, range.start)
        };

        let mut cursor_start = TextEngine::flat_idx_to_cursor(buffer, start_idx);
        let mut cursor_end = TextEngine::flat_idx_to_cursor(buffer, end_idx);

        if (cursor_start.line > cursor_end.line)
            || (cursor_start.line == cursor_end.line && cursor_start.index > cursor_end.index)
        {
            std::mem::swap(&mut cursor_start, &mut cursor_end);
        }

        let mut out_rects = Vec::new();
        for run in buffer.layout_runs() {
            let line_i = run.line_i;

            // 選択範囲より前の行、または後の行はスキップ
            if line_i < cursor_start.line || line_i > cursor_end.line {
                continue;
            }

            // この行（run）における選択の始点と終点
            let line_len = buffer.lines.get(line_i).map_or(0, |l| l.text().len());

            // 開始行なら cursor_start.index、それ以降の行なら行頭（0）
            let run_cursor_start = if line_i == cursor_start.line {
                cursor_start
            } else {
                cosmic_text::Cursor {
                    line: line_i,
                    index: 0,
                    affinity: cosmic_text::Affinity::Before,
                }
            };

            // 終了行なら cursor_end.index、それ以前の行なら行末（line_len）
            let run_cursor_end = if line_i == cursor_end.line {
                cursor_end
            } else {
                cosmic_text::Cursor {
                    line: line_i,
                    index: line_len,
                    affinity: cosmic_text::Affinity::Before,
                }
            };

            // highlight を計算
            for (x, w) in run.highlight(run_cursor_start, run_cursor_end) {
                out_rects.push(LayoutRect::new(x, run.line_top, w, run.line_height));
            }
        }

        out_rects
    }

    /// 現在の選択範囲に基づき描画用の物理選択矩形を計算してキャッシュを更新
    #[inline]
    pub(crate) fn update_selection_rects(
        id: EntityId,
        buffer: &Rc<Buffer>,
        edit_selected_rects: &mut SelectedRectsSparseSecondary,
        edit_selections: &TextSelectionsSparseSecondary,
        cont_input_contents: &InputContentsSparse,
    ) {
        if let Some(range) = edit_selections.get(id).cloned()
            && range.start < range.end
        {
            // 生テキストの選択範囲を、表示テキスト（マスク文字）の選択範囲へ変換
            let display_range = if let Some(contents) = cont_input_contents.get(id) {
                let raw_text = contents.to_michiu();
                contents.raw_range_to_display_range(range, &raw_text)
            } else {
                range
            };

            let out_rects = TextEditStore::calc_selection_rects(id, buffer, display_range);
            edit_selected_rects.insert(id, out_rects);
            return;
        }
        // 範囲が 0、または選択なしの時は自動クリーンアップ
        edit_selected_rects.remove(id);
    }

    /// 現在フォーカスされている要素で範囲選択されている文字列を取得します。
    pub(crate) fn get_selected_text(
        evt_interaction_states: &ActiveInteractionStates,
        cont_text_contents: &TextContentsSparse,
        rnd_visual: &VisualPropertiesSecondary,
        edit_selections: &TextSelectionsSparseSecondary,
    ) -> Option<MichiuString> {
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
                let text = cont_text_contents.at(target_id);
                let byte_range = range.clone();
                return Some(text.slice(byte_range).to_string().into());
            }
        }
        None
    }

    #[inline]
    pub(crate) fn calculate_text_selection(
        start_pos: ByteIndex,
        local: LayoutPoint,
        buffer: &Rc<Buffer>,
        sys_text_engine: &mut TextEngine,
    ) -> (Range<ByteIndex>, bool) {
        let (current_index, is_trailing) = sys_text_engine.hit_test_point(buffer, local);

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
        sys_text_buffers: &TextBufferSparseSecondary,
        cont_text_contents: &TextContentsSparse,
        cont_text_spans: &TextSpansSparse,
        cont_input_contents: &InputContentsSparse,
        topo_active_masks: &mut ActiveMasksSecondary,
        topo_parents: &ParentsSecondary,
        lay_resolved_basic: &ResolvedBasicSecondary,
        lay_resolved_flex: &ResolvedFlexSecondary,
        lay_resolved_grid: &ResolvedGridSparse,
        rnd_dirty_entities: &mut DirtyRenderEntitiesVec,
        rnd_visual: &VisualPropertiesSecondary,
        rnd_interaction: &InteractionPropertiesSecondary,
        rnd_active_transitions: &ActiveTransitionsSparseSecondary,
        edit_selections: &mut TextSelectionsSparseSecondary,
        edit_selection_start_index: &mut SelectionStartIndexSparseSecondary,
        edit_selected_rects: &mut SelectedRectsSparseSecondary,
        out_rects: &RectsSecondary,
        sc_offsets: &ScrollOffsetsSecondary,
    ) {
        let Some(buffer) = SystemStore::get_or_create_layout(
            id,
            sys_text_engine,
            sys_text_buffers,
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
            Some(&buffer),
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
        let (clicked_index, is_trailing) = sys_text_engine.hit_test_point(&buffer, local);

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
                &buffer,
                edit_selected_rects,
                edit_selections,
                cont_input_contents,
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
        start_pos: ByteIndex,
        local: LayoutPoint,
        buffer: Option<&Rc<Buffer>>,
        win_scale_factor: f32,
        win_last_size: Option<LayoutSize>,
        sys_text_engine: &mut TextEngine,
        sys_text_buffers: &TextBufferSparseSecondary,
        cont_text_contents: &mut TextContentsSparse,
        cont_input_contents: &mut InputContentsSparse,
        cont_text_spans: &TextSpansSparse,
        topo_active_masks: &mut ActiveMasksSecondary,
        topo_parents: &ParentsSecondary,
        topo_children: &ChildrenSecondary,
        lay_dirty_entities: &mut DirtyLayoutEntitiesVec,
        lay_taffy_tree: &mut TaffyTreeEntityId,
        bar_styles: &mut ScrollbarStylesSecondary,
        lay_taffy_nodes: &TaffyNodesSecondary,
        lay_resolved_basic: &ResolvedBasicSecondary,
        lay_resolved_flex: &ResolvedFlexSecondary,
        lay_resolved_grid: &ResolvedGridSparse,
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
    ) {
        let Some(buffer) = buffer else {
            return;
        };

        let (display_index, is_trailing) = sys_text_engine.hit_test_point(buffer, local);

        // 表示テキストの文字境界を進める
        let display_text = cont_text_contents.at(id);
        let final_display_index = if is_trailing {
            display_text.next_char_boundary(display_index)
        } else {
            display_index
        };

        // 生テキストのバイト位置に逆算
        let final_raw_index = if let Some(contents) = cont_input_contents.get(id) {
            let raw_text = contents.to_michiu();
            contents.display_byte_to_raw_byte(final_display_index, &raw_text)
        } else {
            final_display_index
        };

        // 生テキスト基準で選択範囲を作成
        let (range, is_reversed) = if start_pos <= final_raw_index {
            (start_pos..final_raw_index, false)
        } else {
            (final_raw_index..start_pos, true)
        };

        edit_selections.insert(id, range.clone());

        TextEditStore::update_selection_rects(
            id,
            buffer,
            edit_selected_rects,
            edit_selections,
            cont_input_contents,
        );

        if let Some(contents) = cont_input_contents.get_mut(id) {
            contents.selection_reversed = is_reversed;
            contents.selected_range = range;

            TextEditStore::apply_input_update(
                id,
                InputOp::MousePress,
                win_scale_factor,
                win_last_size,
                sys_text_engine,
                sys_text_buffers,
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
        sys_text_buffers: &TextBufferSparseSecondary,
        cont_text_contents: &mut TextContentsSparse,
        cont_input_contents: &mut InputContentsSparse,
        cont_text_spans: &TextSpansSparse,
        topo_active_masks: &mut ActiveMasksSecondary,
        topo_parents: &ParentsSecondary,
        topo_children: &ChildrenSecondary,
        lay_dirty_entities: &mut DirtyLayoutEntitiesVec,
        lay_taffy_tree: &mut TaffyTreeEntityId,
        bar_styles: &mut ScrollbarStylesSecondary,
        lay_taffy_nodes: &TaffyNodesSecondary,
        lay_resolved_basic: &ResolvedBasicSecondary,
        lay_resolved_flex: &ResolvedFlexSecondary,
        lay_resolved_grid: &ResolvedGridSparse,
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
    ) {
        let Some(engine) = SystemStore::get_or_create_layout(
            id,
            sys_text_engine,
            sys_text_buffers,
            cont_text_contents,
            cont_text_spans,
            lay_resolved_basic,
            lay_resolved_flex,
            rnd_visual,
            out_rects,
        ) else {
            return;
        };

        // Input 要素の場合は生テキストの長さで全選択範囲を作る
        let input_full_range = if let Some(contents) = cont_input_contents.get_mut(id) {
            let raw_text = contents.to_michiu();
            let full_range = ByteIndex(0)..raw_text.byte_len();

            contents.selected_range = full_range.clone();
            contents.selection_reversed = false;
            Some(full_range)
        } else {
            None
        };

        if let Some(full_range) = input_full_range {
            edit_selections.insert(id, full_range);

            TextEditStore::update_selection_rects(
                id,
                &engine,
                edit_selected_rects,
                edit_selections,
                cont_input_contents,
            );

            TextEditStore::apply_input_update(
                id,
                InputOp::SelectAll,
                win_scale_factor,
                win_last_size,
                sys_text_engine,
                sys_text_buffers,
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
            );
        } else {
            // 通常のテキスト要素
            let text_len = cont_text_contents.at(id).byte_len();
            let full_range = ByteIndex(0)..text_len;

            edit_selections.insert(id, full_range.clone());
            TextEditStore::update_selection_rects(
                id,
                &engine,
                edit_selected_rects,
                edit_selections,
                cont_input_contents,
            );
            RenderStore::mark_render_dirty(id, topo_active_masks, rnd_dirty_entities);
        }
    }

    pub(crate) fn apply_input_update(
        id: EntityId,
        op: InputOp,
        win_scale_factor: f32,
        win_last_size: Option<LayoutSize>,
        sys_text_engine: &mut TextEngine,
        sys_text_buffers: &TextBufferSparseSecondary,
        cont_text_contents: &mut TextContentsSparse,
        cont_input_contents: &mut InputContentsSparse,
        cont_text_spans: &TextSpansSparse,
        topo_active_masks: &mut ActiveMasksSecondary,
        topo_parents: &ParentsSecondary,
        lay_dirty_entities: &mut DirtyLayoutEntitiesVec,
        lay_taffy_tree: &mut TaffyTreeEntityId,
        bar_styles: &mut ScrollbarStylesSecondary,
        lay_taffy_nodes: &TaffyNodesSecondary,
        lay_resolved_basic: &ResolvedBasicSecondary,
        lay_resolved_flex: &ResolvedFlexSecondary,
        lay_resolved_grid: &ResolvedGridSparse,
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
    ) {
        // 直前までハイライトが描画されていたか
        let has_selection_before = edit_selected_rects.contains_key(id);

        // 現在（操作後）に範囲選択されているか
        let has_selection_after = cont_input_contents
            .get(id)
            .is_some_and(|c| c.selected_range.start != c.selected_range.end);

        // 選択範囲の描画を更新
        if (has_selection_before || has_selection_after)
            && let Some(buffer) = SystemStore::get_or_create_layout(
                id,
                sys_text_engine,
                sys_text_buffers,
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
                &buffer,
                edit_selected_rects,
                edit_selections,
                cont_input_contents,
            );
        }

        if let Some(c) = cont_input_contents.get_mut(id) {
            c.needs_scroll_to_caret = true;
        }

        match op {
            // コンテンツのサイズに変動がない（Taffyレイアウトの再計算が不要）操作
            InputOp::MousePress | InputOp::ArrowMove | InputOp::SelectAll => {
                TextEditStore::update_input_caret_position(
                    id,
                    win_scale_factor,
                    win_last_size,
                    sys_text_engine,
                    sys_text_buffers,
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
                );
                RenderStore::mark_render_dirty(id, topo_active_masks, rnd_dirty_entities);
            }

            // Taffy計算前に表示用テキストの同期が必須
            // 未確定文字の伸縮時はシグナルが更新されないため ImeUpdated を含める
            InputOp::Init | InputOp::TextEffect | InputOp::ImeUpdated => {
                // IMEやタイピング中の古いキャッシュを破棄
                SystemStore::clear_layout_cache(id, sys_text_buffers);
                TextEditStore::update_input_caret_position(
                    id,
                    win_scale_factor,
                    win_last_size,
                    sys_text_engine,
                    sys_text_buffers,
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
                // IMEやタイピング中の古いキャッシュを破棄
                SystemStore::clear_layout_cache(id, sys_text_buffers);
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
        sys_text_buffers: &TextBufferSparseSecondary,
        cont_text_contents: &mut TextContentsSparse,
        cont_input_contents: &mut InputContentsSparse,
        cont_text_spans: &TextSpansSparse,
        topo_active_masks: &mut ActiveMasksSecondary,
        topo_parents: &ParentsSecondary,
        lay_dirty_entities: &mut DirtyLayoutEntitiesVec,
        lay_taffy_tree: &mut TaffyTreeEntityId,
        bar_styles: &mut ScrollbarStylesSecondary,
        lay_taffy_nodes: &TaffyNodesSecondary,
        lay_resolved_basic: &ResolvedBasicSecondary,
        lay_resolved_flex: &ResolvedFlexSecondary,
        lay_resolved_grid: &ResolvedGridSparse,
        rnd_visual: &mut VisualPropertiesSecondary,
        rnd_base_visual: &BaseVisualPropertiesSecondary,
        rnd_interaction: &InteractionPropertiesSecondary,
        rnd_active_transitions: &ActiveTransitionsSparseSecondary,
        sc_offsets: &mut ScrollOffsetsSecondary,
        edit_selections: &mut TextSelectionsSparseSecondary,
        out_rects: &RectsSecondary,
        sc_sizes: &ScrollSizesSecondary,
    ) {
        let ime_caret_info = TextEditStore::ime_caret_info(
            id,
            sys_text_engine,
            sys_text_buffers,
            cont_text_contents,
            cont_input_contents,
            cont_text_spans,
            lay_resolved_basic,
            lay_resolved_flex,
            rnd_visual,
            rnd_base_visual,
            edit_selections,
            out_rects,
        );

        let Some((caret, caret_offset, is_multiline)) = ime_caret_info else {
            return;
        };
        let basic = lay_resolved_basic.get_or_default(id);
        let flex = lay_resolved_flex.get_or_default(id);
        let _grid = lay_resolved_grid.get_or_default(id);
        let rect = out_rects.get(id).copied().unwrap_or_default();
        let (border, padding) =
            LayoutStore::get_physical_border_padding(rect, basic.border, basic.padding);

        // キャレットがあるなら Some のはず
        let contents = cont_input_contents.at_mut(id);

        let should_scroll = contents.needs_scroll_to_caret;

        let mut scroll_offset = sc_offsets.get(id).copied().unwrap_or_default();

        if should_scroll && rect.width > 0.0 && rect.height > 0.0 {
            let viewport = OutputStore::calc_viewport_size(rect, border, padding);

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
                flex.align_items,
                is_multiline,
            );

            let aligned_caret_x = caret.x + align_offset.x;
            let aligned_caret_y = caret.y + align_offset.y;

            // マージンを設定するとキー移動時にキャレット位置がずれるため削除
            // let margin_x = 0.0; // 左右端のあそび（マージン）

            // 1. 横方向スクロール (X軸)
            if aligned_caret_x < scroll_offset.x {
                scroll_offset.x = aligned_caret_x.max(0.0);
            } else if aligned_caret_x + caret.width > scroll_offset.x + viewport.width {
                scroll_offset.x = (aligned_caret_x + caret.width - viewport.width).max(0.0);
            }

            // 2. 縦方向スクロール (Y軸 - マルチラインのみ)
            if is_multiline {
                if aligned_caret_y < scroll_offset.y {
                    scroll_offset.y = aligned_caret_y.max(0.0);
                } else if aligned_caret_y + caret.height > scroll_offset.y + viewport.height {
                    scroll_offset.y = (aligned_caret_y + caret.height - viewport.height).max(0.0);
                }
            } else {
                scroll_offset.y = 0.0;
            }

            ScrollStore::scroll_to(
                id,
                scroll_offset.x,
                scroll_offset.y,
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

            contents.needs_scroll_to_caret = false;
        }

        // IMM32 による IME 変換候補ウィンドウの位置同期を自動実行
        SystemStore::sync_imm_window_position(
            rect,
            win_scale_factor,
            border,
            padding,
            caret,
            caret_offset,
            scroll_offset,
        );
    }

    #[inline]
    pub(crate) fn ime_caret_info(
        id: EntityId,
        sys_text_engine: &mut TextEngine,
        sys_text_buffers: &TextBufferSparseSecondary,
        cont_text_contents: &mut TextContentsSparse,
        cont_input_contents: &mut InputContentsSparse,
        cont_text_spans: &TextSpansSparse,
        lay_resolved_basic: &ResolvedBasicSecondary,
        lay_resolved_flex: &ResolvedFlexSecondary,
        rnd_visual: &mut VisualPropertiesSecondary,
        rnd_base_visual: &BaseVisualPropertiesSecondary,
        edit_selections: &mut TextSelectionsSparseSecondary,
        out_rects: &RectsSecondary,
    ) -> Option<(LayoutRect, f32, bool)> {
        let contents = cont_input_contents.get_mut(id)?;
        // 入力エンジン側の最新カーソル位置を描画SoA側に同期
        edit_selections.insert(id, contents.selected_range.clone());

        let text_val = contents.to_michiu();

        // IME未確定文字列が入力されている際、numeric_only が有効であれば数値を事前にフィルタリング
        // is_password が有効であればマスク処理を適用した中間文字列を生成
        let mut filtered_comp_text = if let Some(ref ime) = contents.ime_state
            && !ime.composition_text.is_empty()
        {
            if contents.numeric_only {
                let filtered: String = ime
                    .composition_text
                    .chars()
                    .filter(|&c| c.is_numeric() || c == '.' || c == '-')
                    .collect();
                MichiuString::from(filtered)
            } else {
                ime.composition_text.clone()
            }
        } else {
            MichiuString::default()
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
            );
        }

        // 未確定テキストのマスク
        if !filtered_comp_text.is_empty() {
            filtered_comp_text = contents.mask_if_password(&filtered_comp_text);
        }

        // 確定済みテキストのマスク
        let text_val_for_display = contents.mask_if_password(&text_val);

        // 描画表示用テキストの生成
        // is_password が true の場合、未確定中であっても
        // すでに確定されている文字列部分が一時的に生テキストとして露出してしまわないよう
        // マスクを維持した一時文字列を生成してベースとして使用
        let display_text = if !filtered_comp_text.is_empty() {
            // 未確定文字がある場合：[マスク済み確定テキスト] + [マスク済み未確定文字]
            text_val_for_display.inserted(contents.selected_range.start, &filtered_comp_text)
        } else if text_val.is_empty() {
            // 文字列が完全に空の場合はプレースホルダー
            contents
                .placeholder
                .as_ref()
                .map(std::string::ToString::to_string)
                .unwrap_or_default()
                .into()
        } else {
            text_val_for_display.to_string().into()
        };

        cont_text_contents.insert(id, display_text.clone());

        let buffer = SystemStore::get_or_create_layout(
            id,
            sys_text_engine,
            sys_text_buffers,
            cont_text_contents,
            cont_text_spans,
            lay_resolved_basic,
            lay_resolved_flex,
            rnd_visual,
            out_rects,
        )?;

        let text_size = sys_text_engine.get_layout_size(&buffer);
        contents.last_layout = Some(LayoutRect::new(0.0, 0.0, text_size.width, text_size.height));

        let composition_offset = if let Some(ref ime) = contents.ime_state
            && !ime.composition_text.is_empty()
        {
            // 組成文字全体の文字数をオフセットとして適用
            ime.composition_text.byte_len()
        } else {
            ByteIndex(0)
        };

        // ドラッグの方向を判定しマウス位置にキャレットを固定
        let current_caret_relative = if contents.selection_reversed {
            contents.selected_range.start // 逆方向（左ドラッグ）時は左端がマウス位置
        } else {
            contents.selected_range.end // 順方向（右ドラッグ）時は右端がマウス位置
        };

        // キャレット位（caret_index）の計算
        let caret_index = if contents.is_password {
            let mask = contents.mask_text.as_deref().unwrap_or("●");

            let base_chars = text_val.to_char_index(current_caret_relative);
            let comp_chars = contents
                .ime_state
                .as_ref()
                .map_or(CharIndex(0), |ime| ime.composition_text.char_count());

            let total_chars = base_chars.0 + comp_chars.0;
            ByteIndex(total_chars * mask.len())
        } else {
            current_caret_relative + composition_offset
        };

        // プレースホルダーに干渉されない純粋なキャレット位置を算出
        let (cx_offset, cy_offset, ch_height) =
            sys_text_engine.get_caret_position(&buffer, caret_index);

        contents.measured_caret = LayoutPoint::new(cx_offset, cy_offset);
        contents.caret_line_height = ch_height;

        // 行情報の計算
        let (curr_line, _tot_lines) = display_text.line_indices(caret_index);
        contents.current_line_index = curr_line;

        // 最終表示用テキストを Context 側に反映
        cont_text_contents.insert(id, display_text);

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
            &self.system.sys_text_buffers,
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
        );
    }
}
