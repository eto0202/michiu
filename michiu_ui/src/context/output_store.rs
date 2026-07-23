use std::ops::Range;

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
}
