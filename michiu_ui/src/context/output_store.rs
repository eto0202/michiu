use std::{ops::Range, time::Instant};

use crate::*;
use slotmap::{SecondaryMap, SparseSecondaryMap};
use windows::Win32::Graphics::DirectWrite::{DWRITE_HIT_TEST_METRICS, IDWriteTextLayout};

pub struct OutputStore {
    pub(crate) rects: SecondaryMap<EntityId, LayoutRect>,
    pub(crate) clip_rects: SecondaryMap<EntityId, LayoutRect>,
    pub(crate) scroll_offsets: SecondaryMap<EntityId, LayoutPoint>,
    pub(crate) prev_rects: SecondaryMap<EntityId, LayoutRect>,
    pub(crate) prev_clip_rects: SecondaryMap<EntityId, LayoutRect>,
    pub(crate) selected_rects: SparseSecondaryMap<EntityId, Vec<LayoutRect>>,
    pub(crate) text_selections: SparseSecondaryMap<EntityId, std::ops::Range<usize>>,
    pub(crate) selection_start_index: SparseSecondaryMap<EntityId, usize>,
}

impl Default for OutputStore {
    fn default() -> Self {
        Self::new()
    }
}

impl OutputStore {
    #[inline]
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
    pub(crate) fn swap_output_rect(outputs: &mut OutputStore) {
        std::mem::swap(&mut outputs.rects, &mut outputs.prev_rects);
        std::mem::swap(&mut outputs.clip_rects, &mut outputs.prev_clip_rects);

        outputs.rects.clear();
        outputs.clip_rects.clear();
    }

    pub(crate) fn parent_changed(
        id: EntityId,
        outputs: &OutputStore,
        topology: &TopologyStore,
    ) -> bool {
        let parent_id_opt = topology.parents.get(id).copied().flatten();

        let mut parent_changed = false;

        if let Some(parent_id) = parent_id_opt {
            let prev_parent_rect = outputs.prev_rects.get(parent_id);
            let curr_parent_rect = outputs.rects.get(parent_id);
            let prev_parent_clip = outputs.prev_clip_rects.get(parent_id);
            let curr_parent_clip = outputs.clip_rects.get(parent_id);
            let is_parent_dirty = topology.active_masks[parent_id].has(STATE_QUEUED_LAYOUT);

            // 親が動いた、サイズが変わった、クリップが変わった、または親にレイアウト変更がある
            if prev_parent_rect != curr_parent_rect
                || prev_parent_clip != curr_parent_clip
                || is_parent_dirty
            {
                parent_changed = true;
            }
        }

        parent_changed
    }

    pub(crate) fn calc_local_rect(
        id: EntityId,
        outputs: &OutputStore,
        layouts: &LayoutStore,
        topology: &TopologyStore,
        window_size: LayoutSize,
    ) -> (LayoutRect, LayoutRect) {
        let initial_clip = LayoutRect::new(0.0, 0.0, window_size.width, window_size.height);

        // Taffyから実データを引き出す
        let local_rect = LayoutStore::local_rect_from_taffy(id, layouts);

        let parent_id_opt = topology.parents.get(id).copied().flatten();
        let (abs_rect, parent_clip) = if let Some(parent_id) = parent_id_opt {
            let parent_rect = outputs.rects[parent_id];
            let parent_clip = outputs.clip_rects[parent_id];

            let is_absolute = layouts
                .basic_layouts
                .get(id)
                .map(|l| l.position == Position::Absolute)
                .unwrap_or(false);

            let parent_scroll = if is_absolute {
                LayoutPoint::ZERO
            } else {
                outputs
                    .scroll_offsets
                    .get(parent_id)
                    .copied()
                    .unwrap_or(LayoutPoint::ZERO)
            };

            let abs_x = parent_rect.x + local_rect.x - parent_scroll.x;
            let abs_y = parent_rect.y + local_rect.y - parent_scroll.y;

            (
                LayoutRect::new(abs_x, abs_y, local_rect.width, local_rect.height),
                parent_clip,
            )
        } else {
            (
                LayoutRect::new(
                    local_rect.x,
                    local_rect.y,
                    local_rect.width,
                    local_rect.height,
                ),
                initial_clip,
            )
        };

        (abs_rect, parent_clip)
    }

    /// 単位（Px, Percent, Auto）を親要素のサイズまたはウィンドウ基準をベースに物理ピクセルへ解決します。
    pub(crate) fn resolve_val_to_px(
        id: EntityId,
        val: Val,
        is_width: bool,
        topology: &TopologyStore,
        outputs: &OutputStore,
        window: &WindowStore,
    ) -> Option<f32> {
        match val {
            Val::Px(v) => Some(v),
            Val::Percent(p) => {
                // 親要素の確定サイズを優先取得
                let parent_size = if let Some(Some(parent_id)) = topology.parents.get(id) {
                    outputs
                        .rects
                        .get(*parent_id)
                        .map(|r| LayoutSize::new(r.width, r.height))
                } else {
                    None
                };

                // 親要素が未確定または存在しない場合は、最終ウィンドウ寸法を基準にする
                let ref_size = parent_size.or(window.last_window_size)?;
                let ref_val = if is_width {
                    ref_size.width
                } else {
                    ref_size.height
                };

                Some(ref_val * (p / 100.0))
            }
            Val::Auto => {
                // Auto の場合は前フレームで確定している Taffy のレイアウト結果を実数値の基準値とする
                outputs
                    .rects
                    .get(id)
                    .map(|r| if is_width { r.width } else { r.height })
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
    ) -> LayoutRect {
        let logical_x = rect.x + border.left + padding.left + contents.measured_caret_x - scroll.x;
        let aligned_x = (logical_x * scale).round() / scale;

        let line_height = contents.caret_line_height;
        let caret_width = contents.caret_width.unwrap_or(1.5);
        let caret_height = contents.caret_height.unwrap_or(line_height);

        let vertical_center_offset = if contents.caret_height.is_some() {
            (line_height - caret_height) * 0.5
        } else {
            0.0
        };

        let logical_y =
            rect.y + border.top + padding.top + contents.measured_caret_y + contents.caret_offset
                - scroll.y;

        let aligned_y = ((logical_y + vertical_center_offset) * scale).round() / scale;
        let aligned_width = (caret_width * scale).round().max(1.0) / scale;
        let aligned_height = (caret_height * scale).round().max(1.0) / scale;

        LayoutRect::new(aligned_x, aligned_y, aligned_width, aligned_height)
    }

    /// 現在テキスト選択ドラッグ中かつ、マウスポインタが要素の可視境界外にあるかを判定
    pub(crate) fn is_drag_autoscroll_active(
        events: &EventStore,
        outputs: &OutputStore,
        renders: &RenderStore,
    ) -> bool {
        if let Some(pressed_id) = events.interaction_states.pressed
            && let Some(pointer_pos) = events.current_pointer_position
            && let Some(clip) = outputs.clip_rects.get(pressed_id)
        {
            let user_select = renders
                .visual_properties
                .get(pressed_id)
                .and_then(|v| v.user_select)
                .unwrap_or(UserSelect::None);

            if user_select == UserSelect::Text {
                // ポインタが可視クリップ範囲の上下左右からはみ出しているか検証
                let is_out_x = pointer_pos.x < clip.x || pointer_pos.x > clip.x + clip.width;
                let is_out_y = pointer_pos.y < clip.y || pointer_pos.y > clip.y + clip.height;
                return is_out_x || is_out_y;
            }
        }
        false
    }

    /// 現在の選択範囲（text_selections）に基づき、
    /// 描画用の物理選択矩形（selected_rects）を自動再計算して SoA キャッシュを更新します。
    pub(crate) fn calc_selection_rects(
        id: EntityId,
        layout: IDWriteTextLayout,
        range: Range<usize>,
        outputs: &mut OutputStore,
    ) -> Vec<LayoutRect> {
        let mut hit_test_metrics = vec![DWRITE_HIT_TEST_METRICS::default(); 16];
        let mut actual_count: u32 = 0;
        let res = unsafe {
            layout.HitTestTextRange(
                range.start as u32,
                (range.end - range.start) as u32,
                0.0,
                0.0,
                Some(&mut hit_test_metrics),
                &mut actual_count,
            )
        };

        if res.is_ok() && actual_count as usize > hit_test_metrics.len() {
            hit_test_metrics.resize(actual_count as usize, DWRITE_HIT_TEST_METRICS::default());
            let _ = unsafe {
                layout.HitTestTextRange(
                    range.start as u32,
                    (range.end - range.start) as u32,
                    0.0,
                    0.0,
                    Some(&mut hit_test_metrics),
                    &mut actual_count,
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
            .unwrap_or(UserSelect::None);

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
    pub(crate) fn scroll_ime_info(
        id: EntityId,
        outputs: &mut OutputStore,
        contents: &mut ContentStore,
        renders: &mut RenderStore,
        system: &SystemStore,
    ) -> Option<(LayoutRect, f32, bool)> {
        // (caret_x, caret_y, caret_h, caret_w, caret_offset, is_multiline)
        let mut scroll_ime_info: Option<(LayoutRect, f32, bool)> = None;

        if let Some(input_contents) = contents.input_contents.get_mut(id) {
            // 入力エンジン側の最新カーソル位置を描画SoA側に同期
            outputs
                .text_selections
                .insert(id, input_contents.selected_range.clone());

            let text_val = input_contents.text.0.get();
            input_contents.total_len = text_val.chars().count();

            // 描画表示用テキスト（IME未確定文字列の有無を最優先で判定）
            let display_text = if let Some(ref ime) = input_contents.ime_state
                && !ime.composition_text.is_empty()
            {
                crate::input_get_display_text(
                    &text_val,
                    input_contents.selected_range.start,
                    &ime.composition_text,
                )
            } else if text_val.is_empty() {
                input_contents
                    .placeholder
                    .as_ref()
                    .map(|s| s.to_string())
                    .unwrap_or_default()
            } else if input_contents.is_password {
                let mask = input_contents.mask_text.as_deref().unwrap_or("●");
                mask.repeat(text_val.chars().count())
            } else {
                text_val.clone()
            };

            let caret_text = if let Some(ref ime) = input_contents.ime_state
                && !ime.composition_text.is_empty()
            {
                crate::input_get_display_text(
                    &text_val,
                    input_contents.selected_range.start,
                    &ime.composition_text,
                )
            } else if text_val.is_empty() {
                String::new()
            } else if input_contents.is_password {
                let mask = input_contents.mask_text.as_deref().unwrap_or("●");
                mask.repeat(text_val.chars().count())
            } else {
                text_val.clone()
            };

            let font_size = renders
                .visual_properties
                .get(id)
                .and_then(|v| v.font_size)
                .unwrap_or(16.0);
            let font_family = renders
                .visual_properties
                .get(id)
                .and_then(|v| v.font_family.as_deref());
            let font_weight = renders
                .visual_properties
                .get(id)
                .and_then(|v| v.font_weight);
            let font_style = renders.visual_properties.get(id).and_then(|v| v.font_style);

            let spans = contents
                .text_spans
                .get(id)
                .map(|s| s.as_slice())
                .unwrap_or(&[]);

            // 描画テキスト全体のレイアウトサイズを Taffy 測定用に設定
            let display_layout = system.text_engine.create_layout(
                &display_text,
                font_size,
                font_family,
                font_weight,
                font_style,
                None,
                spans,
            );
            let text_size = system.text_engine.get_layout_size(&display_layout);
            input_contents.last_layout =
                Some(LayoutRect::new(0.0, 0.0, text_size.width, text_size.height));

            // キャレット位置測定用のレイアウトをプレースホルダー抜きで作成
            let caret_layout = system.text_engine.create_layout(
                &caret_text,
                font_size,
                font_family,
                font_weight,
                font_style,
                None,
                spans,
            );

            let composition_offset = if let Some(ref ime) = input_contents.ime_state
                && !ime.composition_text.is_empty()
            {
                // 組成文字全体の文字数をオフセットとして適用
                ime.composition_text.encode_utf16().count()
            } else {
                0
            };

            // ドラッグの方向を判定しマウス位置にキャレットを固定
            let current_caret_relative = if input_contents.selection_reversed {
                input_contents.selected_range.start // 逆方向（左ドラッグ）時は左端がマウス位置
            } else {
                input_contents.selected_range.end // 順方向（右ドラッグ）時は右端がマウス位置
            };

            let caret_index = current_caret_relative + composition_offset;
            let u16_len_caret = caret_text.encode_utf16().count();

            // プレースホルダーに干渉されない純粋なキャレット位置を算出
            let (cx_offset, cy_offset, ch_height) =
                system
                    .text_engine
                    .get_caret_position(&caret_layout, caret_index, u16_len_caret);

            input_contents.measured_caret_x = cx_offset;
            input_contents.measured_caret_y = cy_offset;
            input_contents.caret_line_height = ch_height;

            let (curr_line, tot_lines) = crate::calculate_line_indices(&display_text, caret_index);
            input_contents.current_line_index = curr_line;
            input_contents.total_lines = tot_lines;

            // 最終表示用テキストを Context 側に反映
            contents.text_contents.insert(id, display_text.into());

            if let Some(visual) = renders.visual_properties.get_mut(id) {
                let is_ime_active = input_contents
                    .ime_state
                    .as_ref()
                    .map(|ime| !ime.composition_text.is_empty())
                    .unwrap_or(false);

                if text_val.is_empty() && !is_ime_active {
                    // 確定文字列が空で、かつ未確定文字列も存在しない状態のみグレー表示
                    visual.text_color = input_contents.placeholder_color;
                } else {
                    let base_color = renders
                        .base_visual_properties
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
                    width: input_contents.caret_width.unwrap_or(1.5),
                    height: ch_height,
                },
                input_contents.caret_offset,
                input_contents.is_multiline,
            ));
        }
        scroll_ime_info
    }
}

impl Context {
    /// 指定した要素の画面上の絶対座標（LayoutRect）を取得します。
    #[inline]
    pub fn rect(&self, handle: Element) -> Option<LayoutRect> {
        self.outputs.rects.get(handle.id).copied()
    }

    /// 指定した要素の画面上のクリップ境界（LayoutRect）を取得します。
    #[inline]
    pub fn clip_rect(&self, handle: Element) -> Option<LayoutRect> {
        self.outputs.clip_rects.get(handle.id).copied()
    }

    #[inline]
    pub(crate) fn swap_output_rect(&mut self) {
        OutputStore::swap_output_rect(&mut self.outputs);
    }

    #[inline]
    pub(crate) fn parent_changed(&self, id: EntityId) -> bool {
        OutputStore::parent_changed(id, &self.outputs, &self.topology)
    }

    #[inline]
    pub(crate) fn calc_local_rect(
        &self,
        id: EntityId,
        window_size: LayoutSize,
    ) -> (LayoutRect, LayoutRect) {
        OutputStore::calc_local_rect(
            id,
            &self.outputs,
            &self.layouts,
            &self.topology,
            window_size,
        )
    }

    /// 単位（Px, Percent, Auto）を親要素のサイズまたはウィンドウ基準をベースに f32 (物理ピクセル) へ解決します。
    #[inline]
    pub(crate) fn resolve_val_to_px(&self, id: EntityId, val: Val, is_width: bool) -> Option<f32> {
        OutputStore::resolve_val_to_px(
            id,
            val,
            is_width,
            &self.topology,
            &self.outputs,
            &self.window,
        )
    }

    #[inline]
    pub(crate) fn calculate_caret_rect(
        &self,
        rect: LayoutRect,
        border: EdgeInsets,
        padding: EdgeInsets,
        contents: &InputContents,
        scale: f32,
        scroll: LayoutPoint,
    ) -> LayoutRect {
        OutputStore::calculate_caret_rect(rect, border, padding, contents, scale, scroll)
    }

    /// 現在テキスト選択ドラッグ中かつ、マウスポインタが要素の可視境界外にあるかを判定
    #[inline]
    pub(crate) fn is_drag_autoscroll_active(&self) -> bool {
        OutputStore::is_drag_autoscroll_active(&self.events, &self.outputs, &self.renders)
    }

    /// 現在の選択範囲（text_selections）に基づき、
    /// 描画用の物理選択矩形（selected_rects）を自動再計算して SoA キャッシュを更新します。
    #[inline]
    pub(crate) fn update_selection_rects(&mut self, id: EntityId) {
        if let Some(range) = self.outputs.text_selections.get(id).cloned()
            && range.start < range.end
            && let Some(layout) = self.get_or_create_layout(id)
        {
            let rects = OutputStore::calc_selection_rects(id, layout, range, &mut self.outputs);
            self.outputs.selected_rects.insert(id, rects);
            return;
        }
        // 範囲が 0、または選択なしの時は自動クリーンアップ
        self.outputs.selected_rects.remove(id);
    }

    /// 現在フォーカスされている要素で範囲選択されている文字列を取得します。
    #[inline]
    pub fn get_selected_text(&self) -> Option<String> {
        OutputStore::get_selected_text(&self.events, &self.renders, &self.outputs, &self.contents)
    }

    /// 現在のスクロール位置から相対移動します。
    pub fn scroll_by(&mut self, id: EntityId, dx: f32, dy: f32) -> bool {
        let current = self
            .outputs
            .scroll_offsets
            .get(id)
            .copied()
            .unwrap_or(LayoutPoint::ZERO);
        self.scroll_to(id, current.x + dx, current.y + dy)
    }

    pub(crate) fn sync_scrollbar_drag(&mut self, logical_pos: LayoutPoint) {
        let mut scrollbar_dragged = false;
        let mut active_drag_target: Option<(EntityId, bool, bool)> = None;

        for (id, state) in self.layouts.scrollbar_styles.iter() {
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
                    .unwrap_or(LayoutRect::ZERO);
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
                            .map(|o| o.x)
                            .unwrap_or(0.0);
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
                            .map(|o| o.y)
                            .unwrap_or(0.0);
                        self.scroll_to(current_id, target_scroll_x, current_y);
                    }
                }
            }

            self.mark_render_dirty(current_id);
            scrollbar_dragged = true;
        }
    }

    /// 指定された要素の子要素全体のスクロール領域を親ローカル座標系で算出します。
    pub fn get_scroll_size(&self, id: EntityId) -> LayoutSize {
        let mut max_x = 0.0f32;
        let mut max_y = 0.0f32;

        // 自身に内包されたインラインコンテンツの計測サイズを初期値とする
        if self.topology.active_masks[id].has(COMP_INPUT_CONTENT)
            && let Some(contents) = self.contents.input_contents.get(id)
            && let Some(layout_rect) = contents.last_layout
        {
            max_x = layout_rect.width + contents.caret_width.unwrap_or(1.5);
            max_y = layout_rect.height;
        } else if self.topology.active_masks[id].has(COMP_TEXT_CONTENT)
            && let Some(layout) = self.get_or_create_layout(id)
        {
            let size = self.system.text_engine.get_layout_size(&layout);
            max_x = size.width;
            max_y = size.height;
        }

        // 親要素自体のボーダー・パディング厚を取得
        let (basic, _, _) = self.resolve_active_layouts(id);
        let border = self.get_physical_border(id, &basic);
        let padding = self.get_physical_padding(id, &basic);

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
                    .map(|l| l.position == Position::Absolute)
                    .unwrap_or(false);
                if is_absolute {
                    continue;
                }

                if let Some(&rect) = self.outputs.rects.get(child_id) {
                    let parent_rect = self
                        .outputs
                        .rects
                        .get(id)
                        .copied()
                        .unwrap_or(LayoutRect::ZERO);
                    let scroll_offset = self
                        .outputs
                        .scroll_offsets
                        .get(id)
                        .copied()
                        .unwrap_or(LayoutPoint::ZERO);

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
    pub fn scroll_to(&mut self, id: EntityId, mut x: f32, mut y: f32) -> bool {
        let rect = match self.outputs.rects.get(id).copied() {
            Some(r) => r,
            None => return false,
        };

        let scroll_size = self.get_scroll_size(id);

        // 親コンテナのボーダーおよびパディング厚を取得
        let (basic, _, _) = self.resolve_active_layouts(id);
        let border = self.get_physical_border(id, &basic);
        let padding = self.get_physical_padding(id, &basic);

        let visible_size = self.calculate_visible_size(rect);
        let content_size = self.calculate_inner_content_size(visible_size, border, padding);

        // コンテンツサイズと内枠表示領域サイズの差分として、正確な最大スクロール量を算出
        let max_scroll_x = (scroll_size.width - content_size.width).max(0.0);
        let max_scroll_y = (scroll_size.height - content_size.height).max(0.0);

        x = x.clamp(0.0, max_scroll_x);
        y = y.clamp(0.0, max_scroll_y);

        // スロットが存在しない場合はあらかじめ挿入して初期化
        if !self.outputs.scroll_offsets.contains_key(id) {
            self.outputs.scroll_offsets.insert(id, LayoutPoint::ZERO);
        }

        let current = self.outputs.scroll_offsets.get_mut(id).unwrap();
        if (current.x - x).abs() > 0.01 || (current.y - y).abs() > 0.01 {
            current.x = x;
            current.y = y;

            // スクロールバー状態の最終スクロール時刻を更新
            if let Some(sb_state) = self.layouts.scrollbar_styles.get_mut(id) {
                sb_state.last_scroll_time = Some(Instant::now());
            }

            // オフセット変化に伴い、子孫全体の絶対座標を再同期させる
            self.mark_layout_dirty(id);
            true
        } else {
            false
        }
    }

    /// マウス座標などが、要素の描画領域かつ表示枠内に収まっているかを判定。
    /// 階層的な早期枝刈りヒットテスト
    pub fn hit_test(&self, point: LayoutPoint) -> Option<EntityId> {
        // 各要素の実効 z_index を、親から子へカスケードして算出
        let mut effective_z_indices =
            SecondaryMap::with_capacity(self.topology.active_entities.len());
        for &id in &self.layouts.flat_dfs_sequence {
            let self_z = self
                .renders
                .visual_properties
                .get(id)
                .and_then(|v| v.z_index);

            let parent_z = self
                .topology
                .parents
                .get(id)
                .copied()
                .flatten()
                .and_then(|pid| effective_z_indices.get(pid).copied());

            let eff_z = self_z.or(parent_z).unwrap_or(0);
            effective_z_indices.insert(id, eff_z);
        }

        // 実効 z_index に基づいて active_entities を安定ソート
        let mut sorted_entities = self.topology.active_entities.clone();
        sorted_entities.sort_by_key(|&id| effective_z_indices.get(id).copied().unwrap_or(0));

        for &id in sorted_entities.iter().rev() {
            // ドラッグ中かつゴースト化した元の実体要素、およびプレースホルダー要素はヒットテストを強制スルーさせる
            if Some(id) == self.events.interaction_states.dragged
                || self.topology.active_masks[id].has(STATE_DRAG_OVER)
            {
                continue;
            }

            // 親などの overflow 等でクリップされている表示範囲外ならスキップ
            if let Some(clip) = self.outputs.clip_rects.get(id)
                && !clip.contains(point)
            {
                continue;
            }

            // pointer-events 設定の解決
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

            if pointer_events == PointerEvents::None {
                continue; // 透過設定
            }

            // 物理範囲にヒットしたかを検証
            if let Some(rect) = self.outputs.rects.get(id)
                && rect.contains(point)
            {
                return Some(id);
            }
        }
        None
    }

    /// 階層的な境界判定ヘルパー（非対象のブランチをまるごとスキップ）
    pub(crate) fn hit_test_recursive(&self, id: EntityId, point: LayoutPoint) -> Option<EntityId> {
        // 1. 親などの overflow: hidden 等でクリップされている表示範囲をチェック
        // クリップ領域外であれば、この要素もそのすべての子孫要素も画面上に見えていないため、走査を即座にスキップ（枝刈り）
        if let Some(clip) = self.outputs.clip_rects.get(id)
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

    /// 現在のテキスト・IME状態・フォントサイズから、
    /// キャレットの物理座標や最終表示テキスト、レイアウト矩形を正確に再計算して SoA を更新。
    pub(crate) fn update_input_caret_position(&mut self, id: EntityId) {
        self.clear_layout_cache(id); // IMEやタイピング中の古いキャッシュを破棄

        let scroll_ime_info = OutputStore::scroll_ime_info(
            id,
            &mut self.outputs,
            &mut self.contents,
            &mut self.renders,
            &self.system,
        );

        let Some((caret, caret_offset, is_multiline)) = scroll_ime_info else {
            return;
        };
        let (basic, _, _) = self.resolve_active_layouts(id);
        let border = self.get_physical_border(id, &basic);
        let padding = self.get_physical_padding(id, &basic);
        let rect = self
            .outputs
            .rects
            .get(id)
            .copied()
            .unwrap_or(LayoutRect::ZERO);
        let mut scroll = self
            .outputs
            .scroll_offsets
            .get(id)
            .copied()
            .unwrap_or(LayoutPoint::ZERO);
        let scale = self.window.scale_factor;

        if rect.width > 0.0 && rect.height > 0.0 {
            let viewport_w =
                (rect.width - border.left - border.right - padding.left - padding.right).max(0.0);
            let viewport_h =
                (rect.height - border.top - border.bottom - padding.top - padding.bottom).max(0.0);

            // マージンを設定するとキー移動時にキャレット位置がずれるため削除
            // let margin_x = 0.0; // 左右端のあそび（マージン）

            // 1. 横方向スクロール (X軸)
            if caret.x < scroll.x {
                scroll.x = caret.x.max(0.0);
            } else if caret.x + caret.width > scroll.x + viewport_w {
                scroll.x = (caret.x + caret.width - viewport_w).max(0.0);
            }

            // 2. 縦方向スクロール (Y軸 - マルチラインのみ)
            if is_multiline {
                // let margin_y = 4.0; // 上下端のあそび
                if caret.y < scroll.y {
                    scroll.y = caret.y.max(0.0);
                } else if caret.y + caret.height > scroll.y + viewport_h {
                    scroll.y = (caret.y + caret.height - viewport_h).max(0.0);
                }
            } else {
                scroll.y = 0.0;
            }

            self.scroll_to(id, scroll.x, scroll.y);
        }

        // IMM32 による IME 変換候補ウィンドウの位置同期を自動実行
        SystemStore::sync_imm_window_position(
            rect,
            scale,
            border,
            padding,
            caret,
            caret_offset,
            scroll,
        );
    }
}
