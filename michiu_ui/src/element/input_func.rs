use windows::Win32::Graphics::DirectWrite::IDWriteTextLayout;

use crate::{
    COMP_INPUT_CONTENT, COMP_TEXT_CONTENT, Context, EffectCategory, Element, ElementState,
    EntityId, EventStore, ImeState, InputContents, Modifiers, MouseButton, Prop, STYLE_TEXT_SPANS,
    SelectedRectsSparseSecondary, SelectionStartIndexSparseSecondary, SystemStore, TextEngine,
    TextSelectionsSparseSecondary, TextSpan, UnderlineStyle, VirtualKey, VisualProperty,
    with_context,
};

impl Element {
    /// このコンテナを入力フィールド（テキストボックス）化し、IME制御や入力ロジックをバインドします。
    #[must_use]
    pub fn input(self, contents: impl Into<Prop<InputContents>>) -> Self {
        match contents.into() {
            Prop::None => {}
            Prop::Static(c) => {
                with_context(|cx| self.input_internal(cx, c));
            }
            Prop::Dynamic(f) => {
                let id = self.id;
                with_context(|cx| {
                    cx.create_element_effect(id, EffectCategory::Input, move |cx| {
                        let c = f();
                        let el = Element { id };
                        el.input_internal(cx, c);
                    });
                });
            }
        }
        self
    }

    /// プロバイダー `P` から動的に設定を読み込んで入力フィールド化します。
    #[must_use]
    #[inline]
    pub fn input_d<P, F>(self, f: F) -> Self
    where
        P: Clone + 'static,
        F: Fn(&P) -> InputContents + Send + Sync + 'static,
    {
        let dynamic_prop = Prop::Dynamic(Box::new(move || {
            let signal = with_context(|cx| cx.use_provided::<P>());
            let val = signal.get();
            f(&val)
        }));
        self.input(dynamic_prop)
    }

    /// 複数行入力（テキストエリア）をバインドします。
    #[must_use]
    pub fn input_area(self, contents: impl Into<Prop<InputContents>>) -> Self {
        match contents.into() {
            Prop::None => {}
            Prop::Static(mut c) => {
                c.is_multiline = true; // マルチライン化を強制
                with_context(|cx| self.input_internal(cx, c));
            }
            Prop::Dynamic(f) => {
                let id = self.id;
                with_context(|cx| {
                    cx.create_element_effect(id, EffectCategory::Input, move |cx| {
                        let mut c = f();
                        c.is_multiline = true;
                        let el = Element { id };
                        el.input_internal(cx, c);
                    });
                });
            }
        }
        self
    }

    /// プロバイダー `P` から動的に設定を読み込んで複数行入力フィールド化します。
    #[must_use]
    #[inline]
    pub fn input_area_d<P, F>(self, f: F) -> Self
    where
        P: Clone + 'static,
        F: Fn(&P) -> InputContents + Send + Sync + 'static,
    {
        let dynamic_prop = Prop::Dynamic(Box::new(move || {
            let signal = with_context(|cx| cx.use_provided::<P>());
            let val = signal.get();
            f(&val)
        }));
        self.input_area(dynamic_prop)
    }

    fn sync_existing_input_properties(existing: &mut InputContents, c: InputContents) {
        existing.placeholder = c.placeholder;
        existing.placeholder_color = c.placeholder_color;
        existing.caret_color = c.caret_color;
        existing.caret_width = c.caret_width;
        existing.caret_height = c.caret_height;
        existing.caret_offset = c.caret_offset;
        existing.is_blink = c.is_blink;
        existing.blink_frequency = c.blink_frequency;
        existing.has_caret = c.has_caret;
        existing.placeholder_select = c.placeholder_select;
        existing.is_multiline = c.is_multiline;
        existing.is_password = c.is_password;
        existing.mask_text = c.mask_text;
        existing.auto_wrap = c.auto_wrap;

        // 動的なテキスト長の変更に伴い、既存の選択範囲が枠外へ飛び出さないようクランプ
        let current_text = existing.text.0.get();
        let u16_len = current_text.encode_utf16().count();
        existing.selected_range.start = existing.selected_range.start.min(u16_len);
        existing.selected_range.end = existing.selected_range.end.min(u16_len);
    }

    /// キャレット位置を単一の点に設定する
    /// 範囲選択をリセットして解除する処理
    #[inline]
    fn set_caret_position(
        id: EntityId,
        contents: &mut InputContents,
        caret: usize,
        out_text_selections: &mut TextSelectionsSparseSecondary,
        out_selection_start_index: &mut SelectionStartIndexSparseSecondary,
        out_selected_rects: Option<&mut SelectedRectsSparseSecondary>,
    ) {
        contents.selected_range = caret..caret;
        contents.selection_reversed = false;

        out_text_selections.insert(id, caret..caret);
        out_selection_start_index.insert(id, caret);
        if let Some(rects) = out_selected_rects {
            rects.remove(id);
        }
    }

    /// 範囲選択を更新
    #[inline]
    fn set_selection_range(
        id: EntityId,
        contents: &mut InputContents,
        range: std::ops::Range<usize>,
        selection_reversed: bool,
        out_text_selections: &mut TextSelectionsSparseSecondary,
    ) {
        contents.selected_range = range.clone();
        contents.selection_reversed = selection_reversed;
        out_text_selections.insert(id, range);
    }

    fn handle_input_mouse_pressed(
        cx: &mut Context,
        id: EntityId,
        btn: MouseButton,
        mods: Modifiers,
        state: ElementState,
    ) {
        if btn != MouseButton::Left || state != ElementState::Pressed {
            return;
        }
        let Some(pointer_pos) = cx.events.evt_current_pointer_position else {
            return;
        };

        let local = EventStore::pressed_local_point(
            id,
            pointer_pos,
            &cx.contents.cont_input_contents,
            &cx.topology.topo_active_masks,
            &cx.topology.topo_parents,
            &cx.layouts.lay_resolved_basic,
            &cx.layouts.lay_resolved_flex,
            &cx.layouts.lay_resolved_grid,
            &cx.renders.rnd_active_transitions,
            &cx.renders.rnd_interaction,
            &cx.renders.rnd_visual,
            &cx.outputs.out_scroll_offsets,
            &cx.outputs.out_rects,
        );

        let Some(contents) = cx.contents.cont_input_contents.get_mut(id) else {
            return;
        };

        let mut update_rects_needed = false;

        let text_val = contents.text.0.get();

        // プレースホルダーが表示状態にあるか
        let is_placeholder = text_val.is_empty()
            && contents
                .ime_state
                .as_ref()
                .is_none_or(|s| s.composition_text.is_empty());

        // プレースホルダー選択が不許可かつプレースホルダー表示中なら、ヒットテストをスキップして 0 をセット
        if is_placeholder && !contents.placeholder_select {
            Element::set_caret_position(
                id,
                contents,
                0,
                &mut cx.outputs.out_text_selections,
                &mut cx.outputs.out_selection_start_index,
                Some(&mut cx.outputs.out_selected_rects),
            );
        } else {
            let Some(dw_layout) = SystemStore::get_or_create_layout(
                id,
                &cx.system.sys_text_engine,
                &cx.system.sys_dwrite_layouts,
                &cx.contents.cont_text_contents,
                &cx.contents.cont_text_spans,
                &cx.renders.rnd_visual,
            ) else {
                return;
            };
            // キャッシュ済みのレイアウトをそのまま使って高速にヒットテスト
            let (new_caret, is_trailing) = cx
                .system
                .sys_text_engine
                .hit_test_point(&dw_layout, local.x, local.y);
            let final_caret = if is_trailing {
                new_caret + 1
            } else {
                new_caret
            };

            let editable_len = if is_placeholder {
                contents
                    .placeholder
                    .as_ref()
                    .map_or(0, |p| p.encode_utf16().count())
            } else {
                // 通常の文字列長
                text_val.encode_utf16().count()
            };
            let final_caret_clamped = final_caret.min(editable_len);

            if mods.shift {
                let anchor = cx
                    .outputs
                    .out_selection_start_index
                    .entry(id)
                    .map_or(contents.selected_range.start, |e| {
                        *e.or_insert(contents.selected_range.start)
                    });

                let (range, reversed) = if anchor <= final_caret_clamped {
                    (anchor..final_caret_clamped, false)
                } else {
                    (final_caret_clamped..anchor, true)
                };
                Element::set_selection_range(
                    id,
                    contents,
                    range,
                    reversed,
                    &mut cx.outputs.out_text_selections,
                );
                update_rects_needed = true;
            } else {
                Element::set_caret_position(
                    id,
                    contents,
                    final_caret_clamped,
                    &mut cx.outputs.out_text_selections,
                    &mut cx.outputs.out_selection_start_index,
                    Some(&mut cx.outputs.out_selected_rects),
                );
            }
        }
        contents.last_interacted_time = Some(std::time::Instant::now());
        // キャレットの絶対座標と表示情報を一括更新
        cx.update_input_caret_position(id);

        if update_rects_needed && let Some(layout) = cx.get_or_create_layout(id) {
            cx.update_selection_rects(id, &layout);
        }
        cx.mark_render_dirty(id);
    }

    fn handle_input_focus_gained(cx: &mut Context, id: EntityId) {
        if let Some(contents) = cx.contents.cont_input_contents.get_mut(id) {
            contents.is_selecting = false;
            // フォーカス獲得時も操作時刻を記録して即座にキャレットを表示
            contents.last_interacted_time = Some(std::time::Instant::now());
        }
        cx.mark_render_dirty(id);
    }

    fn handle_input_char_typed(cx: &mut Context, id: EntityId, ch: &mut char) {
        // IME未変換の入力中 (composition_textがある間) は文字入力を無視
        let is_ime_active = cx
            .contents
            .cont_input_contents
            .get(id)
            .and_then(|c| c.ime_state.as_ref())
            .is_some_and(|s| !s.composition_text.is_empty());

        if is_ime_active {
            return;
        }

        let mut is_allowed = !ch.is_control();
        let is_multiline = cx
            .contents
            .cont_input_contents
            .get(id)
            .is_some_and(|c| c.is_multiline);

        // 複数行入力時に、Enterキー（'\r' / '\n'）が押された場合は改行コードとして許可
        if is_multiline && (*ch == '\r' || *ch == '\n') {
            *ch = '\n';
            is_allowed = true;
        }

        if !is_allowed {
            return;
        }

        let Some(contents) = cx.contents.cont_input_contents.get_mut(id) else {
            return;
        };

        contents.last_interacted_time = Some(std::time::Instant::now());

        let text_val = contents.text.0.get();

        // 数値制限フィルター
        if contents.numeric_only && !ch.is_numeric() && *ch != '.' && *ch != '-' {
            return;
        }

        let range = contents.selected_range.clone();

        // 変更発生前に現在の状態をセーブ
        contents.record_undo(text_val.clone(), range.clone());

        let u16_text: Vec<u16> = text_val.encode_utf16().collect();

        // 選択範囲が削除された後の長さ
        let u16_len_after_delete =
            u16_text.len() - (range.end.min(u16_text.len()) - range.start.min(u16_text.len()));
        let mut buf = [0u16; 2];
        let ch_u16_slice = ch.encode_utf16(&mut buf);

        // 文字数制限
        if let Some(max) = contents.max_length
            && u16_len_after_delete + ch_u16_slice.len() > max
        {
            return; // 制限を超えるため入力を中断
        }

        contents.last_interacted_time = Some(std::time::Instant::now());

        let mut left = u16_text[..range.start.min(u16_text.len())].to_vec();
        let right = u16_text[range.end.min(u16_text.len())..].to_vec();

        left.extend_from_slice(ch_u16_slice);
        left.extend_from_slice(&right);

        let new_text = String::from_utf16_lossy(&left);
        let new_caret = range.start + ch_u16_slice.len();

        Element::set_caret_position(
            id,
            contents,
            new_caret,
            &mut cx.outputs.out_text_selections,
            &mut cx.outputs.out_selection_start_index,
            Some(&mut cx.outputs.out_selected_rects),
        );
        contents.text.1.set(new_text);
        cx.mark_render_dirty(id);
    }

    fn pressed_back(
        id: EntityId,
        contents: &mut InputContents,
        caret: &mut usize,
        text_val: &str,
        out_text_selections: &mut TextSelectionsSparseSecondary,
        out_selection_start_index: &mut SelectionStartIndexSparseSecondary,
    ) {
        let range = contents.selected_range.clone();
        contents.record_undo(text_val.to_string(), range.clone());

        if range.start < range.end {
            // 選択範囲を一撃で消去
            let u16_text: Vec<u16> = text_val.encode_utf16().collect();
            let mut left = u16_text[..range.start.min(u16_text.len())].to_vec();
            let right = u16_text[range.end.min(u16_text.len())..].to_vec();
            left.extend_from_slice(&right);

            let new_text = String::from_utf16_lossy(&left);
            Element::set_caret_position(
                id,
                contents,
                range.start,
                out_text_selections,
                out_selection_start_index,
                None,
            );
            contents.text.1.set(new_text);
        } else {
            // 通常の1文字バックスペース
            let new_text = InputContents::input_backspace(text_val, caret);
            Element::set_caret_position(
                id,
                contents,
                *caret,
                out_text_selections,
                out_selection_start_index,
                None,
            );
            contents.text.1.set(new_text);
        }
        contents.last_interacted_time = Some(std::time::Instant::now());
    }

    fn pressed_delete(
        id: EntityId,
        contents: &mut InputContents,
        caret: usize,
        text_val: &str,
        out_text_selections: &mut TextSelectionsSparseSecondary,
        out_selection_start_index: &mut SelectionStartIndexSparseSecondary,
    ) {
        let range = contents.selected_range.clone();
        contents.record_undo(text_val.to_string(), range.clone());
        if range.start < range.end {
            let u16_text: Vec<u16> = text_val.encode_utf16().collect();
            let mut left = u16_text[..range.start.min(u16_text.len())].to_vec();
            let right = u16_text[range.end.min(u16_text.len())..].to_vec();
            left.extend_from_slice(&right);

            let new_text = String::from_utf16_lossy(&left);
            Element::set_caret_position(
                id,
                contents,
                range.start,
                out_text_selections,
                out_selection_start_index,
                None,
            );
            contents.text.1.set(new_text);
        } else {
            // 通常の1文字デリート
            let new_text = InputContents::input_delete(text_val, caret);
            Element::set_caret_position(
                id,
                contents,
                caret,
                out_text_selections,
                out_selection_start_index,
                None,
            );
            contents.text.1.set(new_text);
        }
        contents.last_interacted_time = Some(std::time::Instant::now());
    }

    fn pressed_left(
        id: EntityId,
        contents: &mut InputContents,
        caret: usize,
        mods: Modifiers,
        out_text_selections: &mut TextSelectionsSparseSecondary,
        out_selection_start_index: &mut SelectionStartIndexSparseSecondary,
        out_selected_rects: &mut SelectedRectsSparseSecondary,
    ) -> bool {
        let range = contents.selected_range.clone();
        // 選択範囲が存在し、かつ Shiftキーが押されていない通常移動時
        if range.start < range.end && !mods.shift {
            // 選択範囲をすべて解除し、キャレットを左端（start）に収束
            let new_caret = range.start;
            Element::set_caret_position(
                id,
                contents,
                new_caret,
                out_text_selections,
                out_selection_start_index,
                Some(out_selected_rects),
            );
            contents.last_interacted_time = Some(std::time::Instant::now());
            return true;
        } else if caret > 0 {
            let new_caret = caret - 1;

            if mods.shift {
                // Shiftキー押下中：選択の拡張
                let anchor = out_selection_start_index.get(id).copied().unwrap_or(caret);
                if !out_selection_start_index.contains_key(id) {
                    out_selection_start_index.insert(id, caret);
                }
                let (range, reversed) = if anchor <= new_caret {
                    (anchor..new_caret, false)
                } else {
                    (new_caret..anchor, true)
                };
                Element::set_selection_range(id, contents, range, reversed, out_text_selections);
            } else {
                // Shiftキー非押下：選択解除して単なる移動
                Element::set_caret_position(
                    id,
                    contents,
                    new_caret,
                    out_text_selections,
                    out_selection_start_index,
                    Some(out_selected_rects),
                );
            }
            contents.last_interacted_time = Some(std::time::Instant::now());
            return true;
        }
        false
    }

    fn pressed_right(
        id: EntityId,
        contents: &mut InputContents,
        caret: usize,
        u16_len: usize,
        mods: Modifiers,
        out_text_selections: &mut TextSelectionsSparseSecondary,
        out_selection_start_index: &mut SelectionStartIndexSparseSecondary,
        out_selected_rects: &mut SelectedRectsSparseSecondary,
    ) -> bool {
        let range = contents.selected_range.clone();
        // 選択範囲が存在し、かつ Shiftキーが押されていない通常移動時（全選択中での右移動に完全対応）
        if range.start < range.end && !mods.shift {
            let new_caret = range.end;
            Element::set_caret_position(
                id,
                contents,
                new_caret,
                out_text_selections,
                out_selection_start_index,
                Some(out_selected_rects),
            );
            contents.last_interacted_time = Some(std::time::Instant::now());
            return true;
        } else if caret < u16_len {
            let new_caret = caret + 1;

            if mods.shift {
                let anchor = out_selection_start_index.get(id).copied().unwrap_or(caret);
                if !out_selection_start_index.contains_key(id) {
                    out_selection_start_index.insert(id, caret);
                }
                let (range, reversed) = if anchor <= new_caret {
                    (anchor..new_caret, false)
                } else {
                    (new_caret..anchor, true)
                };
                Element::set_selection_range(id, contents, range, reversed, out_text_selections);
            } else {
                Element::set_caret_position(
                    id,
                    contents,
                    new_caret,
                    out_text_selections,
                    out_selection_start_index,
                    Some(out_selected_rects),
                );
            }
            contents.last_interacted_time = Some(std::time::Instant::now());
            return true;
        }
        false
    }

    fn pressed_up(
        id: EntityId,
        contents: &mut InputContents,
        dw_layout: &IDWriteTextLayout,
        font_size: f32,
        caret: usize,
        u16_len: usize,
        mods: Modifiers,
        sys_text_engine: &TextEngine,
        out_text_selections: &mut TextSelectionsSparseSecondary,
        out_selection_start_index: &mut SelectionStartIndexSparseSecondary,
    ) -> bool {
        if !contents.is_multiline {
            return false;
        }

        let (cx_offset, cy_offset, _) =
            sys_text_engine.get_caret_position(dw_layout, caret, u16_len);

        let line_height = font_size * 1.3;
        let target_y = (cy_offset - line_height * 1.1).max(0.0); // 1行分＋マージン

        let (new_caret, is_trailing) =
            sys_text_engine.hit_test_point(dw_layout, cx_offset, target_y);
        let final_caret = if is_trailing {
            new_caret + 1
        } else {
            new_caret
        };

        if mods.shift {
            let anchor = out_selection_start_index.get(id).copied().unwrap_or(caret);
            if !out_selection_start_index.contains_key(id) {
                out_selection_start_index.insert(id, caret);
            }
            let (range, reversed) = if anchor <= final_caret {
                (anchor..final_caret, false)
            } else {
                (final_caret..anchor, true)
            };
            Element::set_selection_range(id, contents, range, reversed, out_text_selections);
        } else {
            Element::set_caret_position(
                id,
                contents,
                final_caret,
                out_text_selections,
                out_selection_start_index,
                None,
            );
        }

        contents.last_interacted_time = Some(std::time::Instant::now());
        true
    }

    fn pressed_down(
        id: EntityId,
        contents: &mut InputContents,
        dw_layout: &IDWriteTextLayout,
        font_size: f32,
        caret: usize,
        u16_len: usize,
        mods: Modifiers,
        sys_text_engine: &TextEngine,
        out_text_selections: &mut TextSelectionsSparseSecondary,
        out_selection_start_index: &mut SelectionStartIndexSparseSecondary,
    ) -> bool {
        if !contents.is_multiline {
            return false;
        }

        let (cx_offset, cy_offset, _) =
            sys_text_engine.get_caret_position(dw_layout, caret, u16_len);

        let line_height = font_size * 1.3;
        let target_y = cy_offset + line_height * 1.5;
        let (new_caret, is_trailing) =
            sys_text_engine.hit_test_point(dw_layout, cx_offset, target_y);
        let final_caret = if is_trailing {
            new_caret + 1
        } else {
            new_caret
        };

        if mods.shift {
            let anchor = out_selection_start_index.get(id).copied().unwrap_or(caret);
            if !out_selection_start_index.contains_key(id) {
                out_selection_start_index.insert(id, caret);
            }
            let (range, reversed) = if anchor <= final_caret {
                (anchor..final_caret, false)
            } else {
                (final_caret..anchor, true)
            };
            Element::set_selection_range(id, contents, range, reversed, out_text_selections);
        } else {
            Element::set_caret_position(
                id,
                contents,
                final_caret,
                out_text_selections,
                out_selection_start_index,
                None,
            );
        }

        contents.last_interacted_time = Some(std::time::Instant::now());
        true
    }

    fn handle_input_key_pressed(
        cx: &mut Context,
        id: EntityId,
        key: VirtualKey,
        mods: Modifiers,
        state: ElementState,
    ) {
        if state != ElementState::Pressed {
            return;
        }

        let Some(dw_layout) = cx.get_or_create_layout(id) else {
            return;
        };

        let Some(contents) = cx.contents.cont_input_contents.get_mut(id) else {
            return;
        };

        let default_visual = VisualProperty::default();
        let visual = cx.renders.rnd_visual.get(id).unwrap_or(&default_visual);
        let font_size = visual.font_size.unwrap_or(16.0);
        let text_val = contents.text.0.get();
        let u16_len = text_val.encode_utf16().count();

        contents.selected_range = (contents.selected_range.start.min(u16_len))
            ..(contents.selected_range.end.min(u16_len));

        let raw_caret = if contents.selection_reversed {
            contents.selected_range.start
        } else {
            contents.selected_range.end
        };
        let mut caret = raw_caret.min(u16_len);
        let mut changed = false; // 状態変更フラグ

        match key {
            VirtualKey::BACK => {
                Element::pressed_back(
                    id,
                    contents,
                    &mut caret,
                    &text_val,
                    &mut cx.outputs.out_text_selections,
                    &mut cx.outputs.out_selection_start_index,
                );
                changed = true;
            }
            VirtualKey::DELETE => {
                Element::pressed_delete(
                    id,
                    contents,
                    caret,
                    &text_val,
                    &mut cx.outputs.out_text_selections,
                    &mut cx.outputs.out_selection_start_index,
                );
                changed = true;
            }
            VirtualKey::LEFT => {
                changed = Element::pressed_left(
                    id,
                    contents,
                    caret,
                    mods,
                    &mut cx.outputs.out_text_selections,
                    &mut cx.outputs.out_selection_start_index,
                    &mut cx.outputs.out_selected_rects,
                );
            }
            VirtualKey::RIGHT => {
                changed = Element::pressed_right(
                    id,
                    contents,
                    caret,
                    u16_len,
                    mods,
                    &mut cx.outputs.out_text_selections,
                    &mut cx.outputs.out_selection_start_index,
                    &mut cx.outputs.out_selected_rects,
                );
            }
            VirtualKey::UP => {
                changed = Element::pressed_up(
                    id,
                    contents,
                    &dw_layout,
                    font_size,
                    caret,
                    u16_len,
                    mods,
                    &cx.system.sys_text_engine,
                    &mut cx.outputs.out_text_selections,
                    &mut cx.outputs.out_selection_start_index,
                );
            }
            VirtualKey::DOWN => {
                changed = Element::pressed_down(
                    id,
                    contents,
                    &dw_layout,
                    font_size,
                    caret,
                    u16_len,
                    mods,
                    &cx.system.sys_text_engine,
                    &mut cx.outputs.out_text_selections,
                    &mut cx.outputs.out_selection_start_index,
                );
            }
            _ => {}
        }

        if changed {
            if let Some(layout) = cx.get_or_create_layout(id) {
                cx.update_selection_rects(id, &layout);
            }
            cx.update_input_caret_position(id);
            cx.mark_render_dirty(id);
        }
    }

    fn handle_input_ime_updated(cx: &mut Context, id: EntityId, ime: &ImeState) {
        let Some(contents) = cx.contents.cont_input_contents.get_mut(id) else {
            return;
        };

        contents.last_interacted_time = Some(std::time::Instant::now());
        contents.ime_state = Some(ime.clone());

        let mut text_val = contents.text.0.get();
        let range = contents.selected_range.clone();

        // 選択範囲が存在し、かつIME入力（未変換または確定）が開始される場合、
        // 文字入力の直前に選択範囲の文字列をあらかじめ消去・置換する
        if range.start < range.end
            && (!ime.composition_text.is_empty() || !ime.result_text.is_empty())
        {
            // Undo履歴に削除前の状態を記録
            contents.record_undo(text_val.clone(), range.clone());

            let u16_text: Vec<u16> = text_val.encode_utf16().collect();
            let mut left = u16_text[..range.start.min(u16_text.len())].to_vec();
            let right = u16_text[range.end.min(u16_text.len())..].to_vec();
            left.extend_from_slice(&right);

            let new_text = String::from_utf16_lossy(&left);
            let caret = range.start;

            // キャレット・選択範囲を消去開始位置に一度リセットして同期
            Element::set_caret_position(
                id,
                contents,
                caret,
                &mut cx.outputs.out_text_selections,
                &mut cx.outputs.out_selection_start_index,
                Some(&mut cx.outputs.out_selected_rects),
            );
            contents.text.1.set(new_text.clone());
            text_val = new_text;
        }

        // IME 確定文字の書き込み
        if !ime.result_text.is_empty() {
            let mut caret = contents.selected_range.start;

            // 確定した文字列を1文字ずつ安全に挿入
            let mut temp_text = text_val;
            for ch in ime.result_text.chars() {
                temp_text = InputContents::input_insert_char(
                    &temp_text,
                    &mut caret,
                    ch,
                    contents.max_length,
                    contents.numeric_only,
                );
            }

            // 確定したキャレット位置で SoA 側の選択状態と開始アンカーを同期
            Element::set_caret_position(
                id,
                contents,
                caret,
                &mut cx.outputs.out_text_selections,
                &mut cx.outputs.out_selection_start_index,
                Some(&mut cx.outputs.out_selected_rects),
            );

            contents.text.1.set(temp_text);
            contents.marked_range = None;
        } else if !ime.composition_text.is_empty() {
            // IME 未変換中
            let caret = contents.selected_range.start;
            let comp_len = ime.composition_text.encode_utf16().count();
            contents.marked_range = Some(caret..(caret + comp_len));
        } else {
            contents.marked_range = None;
        }

        // IME の未確定状態（未確定波線、変換フォーカス太線/細線）を TextSpan に自動マッピング
        if ime.composition_text.is_empty() {
            cx.contents.cont_text_spans.remove(id);
            cx.topology.topo_active_masks[id].unset(STYLE_TEXT_SPANS);
        } else {
            let mut spans = Vec::new();
            let caret = contents.selected_range.start;

            if ime.composition_attrs.is_empty() {
                // 属性が取得できない場合のフォールバック（全体を未確定波線に設定）
                let comp_len = ime.composition_text.encode_utf16().count();
                spans.push(TextSpan {
                    range: caret..(caret + comp_len),
                    underline: Some(UnderlineStyle::Wave),
                    ..Default::default()
                });
            } else {
                let attrs = &ime.composition_attrs;
                let mut start_idx = 0;

                // 同一のIME属性が連続する境界ごとに TextSpan を分割
                while start_idx < attrs.len() {
                    let attr = attrs[start_idx];
                    let mut end_idx = start_idx + 1;
                    while end_idx < attrs.len() && attrs[end_idx] == attr {
                        end_idx += 1;
                    }

                    // Windows IME 属性定数:
                    // ATTR_INPUT (0): 未変換入力 ➔ 波線 (Wave)
                    // ATTR_TARGET_CONVERTED (1): フォーカス（ターゲット）文節 ➔ 太実線 (Thick)
                    // ATTR_CONVERTED (2): 変換済み非フォーカス文節 ➔ 細実線 (Solid)
                    let underline_style = match attr {
                        1 => Some(UnderlineStyle::Thick),
                        2 => Some(UnderlineStyle::Solid),
                        _ => Some(UnderlineStyle::Wave),
                    };

                    spans.push(TextSpan {
                        range: (caret + start_idx)..(caret + end_idx),
                        color: None,
                        bg_color: None,
                        font_size: None,
                        font_family: None,
                        font_weight: None,
                        font_style: None,
                        underline: underline_style,
                        underline_color: None,
                        strikethrough: None,
                        strikethrough_color: None,
                        link_id: None,
                    });

                    start_idx = end_idx;
                }
            }

            cx.contents.cont_text_spans.insert(id, spans);
            cx.topology.topo_active_masks[id].set(STYLE_TEXT_SPANS);
        }

        // IMEイベント終了（または変換中）に表示テキストとキャレット位置を再計算・同期させる
        cx.update_input_caret_position(id);
        cx.mark_render_dirty(id);
    }

    /// 入力イベント（キー、IME、文字入力、フォーカス）を自動的にマッピングして代行するロジック
    #[allow(clippy::too_many_lines)]
    fn input_internal(self, cx: &mut Context, mut c: InputContents) {
        let id = self.id;

        // シグナル更新やテーマ変更、親コンポーネントの再レンダリングによる
        // キャレット位置（selected_range）や Undo/Redo 履歴の末尾への強制初期化を防止
        // 既存の状態を検知した場合はデザイン設定のみを上書き
        if let Some(existing) = cx.contents.cont_input_contents.get_mut(id) {
            Element::sync_existing_input_properties(existing, c);
            // 早期リターンを抜ける前に、最新の文字列状態を SoA / DWrite 側へ即座に同期・反映
            cx.update_input_caret_position(id);
            cx.mark_dirty(id);
            // これ以降の初期化を完全にスキップして早期リターン
            return;
        }

        // 最初のロード時、シグナルから現在値を取得して内部カーソルを末尾に合わせる
        let current_text = c.text.0.get();
        let current_len = current_text.encode_utf16().count();
        c.selected_range = current_len..current_len;

        cx.contents.cont_input_contents.insert(id, c);
        cx.topology.topo_active_masks[id].set(COMP_INPUT_CONTENT | COMP_TEXT_CONTENT);

        self.get_or_create_listeners(|l| {
            let mut existing_mouse = l.on_mouse_input.take();
            l.on_mouse_input = Some(Box::new(move |cx, btn, mods, state| {
                Element::handle_input_mouse_pressed(cx, id, btn, mods, state);
                if let Some(ref mut ext) = existing_mouse {
                    ext(cx, btn, mods, state);
                }
            }));

            let mut existing_focus = l.on_focus.take();
            // フォーカス取得（点滅カーソルの有効化等）
            l.on_focus = Some(Box::new(move |cx| {
                Element::handle_input_focus_gained(cx, id);
                if let Some(ref mut ext) = existing_focus {
                    ext(cx);
                }
            }));

            let mut existing_char = l.on_char_input.take();
            // 確定した1文字の文字入力 (WM_CHAR)
            l.on_char_input = Some(Box::new(move |cx, mut ch| {
                Element::handle_input_char_typed(cx, id, &mut ch);
                if let Some(ref mut ext) = existing_char {
                    ext(cx, ch);
                }
            }));

            let mut existing_keyboard = l.on_keyboard_input.take();
            // 物理キーボード操作 (Backspace, Delete, 矢印キー)
            l.on_keyboard_input = Some(Box::new(move |cx, key, mods, state| {
                Element::handle_input_key_pressed(cx, id, key, mods, state);
                if let Some(ref mut ext) = existing_keyboard {
                    ext(cx, key, mods, state);
                }
            }));

            let mut existing_ime = l.on_ime.take();
            // IME連動
            l.on_ime = Some(Box::new(move |cx, ime| {
                Element::handle_input_ime_updated(cx, id, &ime);
                if let Some(ref mut ext) = existing_ime {
                    ext(cx, ime);
                }
            }));
        });

        cx.create_element_effect(id, EffectCategory::Text, move |cx| {
            if let Some(contents) = cx.contents.cont_input_contents.get(id) {
                let _base_text_val = contents.text.0.get();
            }
            cx.update_input_caret_position(id);
            cx.mark_dirty(id);
        });
    }
}
