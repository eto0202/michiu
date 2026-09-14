use crate::{
    ByteIndex, ComponentMask, Context, DebugStore, EffectCategory, Element, ElementState, EntityId,
    ImeState, InputContents, InputOp, LayoutPoint, MichiuSoA, MichiuString, Modifiers, MouseButton,
    OutputStore, Prop, SelectedRectsSparse, SelectionStartIndexSparse, SystemStore, TextEngine,
    TextSelectionsSparse, TextSpan, UnderlineStyle, VirtualKey, with_context,
};
use cosmic_text::Buffer;
use std::{ops::Range, time::Instant};

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

        // 動的なテキスト長の変更に伴い、既存の選択範囲が枠外へ飛び出さないようクランプ
        let current_text = existing.to_michiu();
        let text_len = current_text.byte_len();
        existing.selected_range.start = existing.selected_range.start.min(text_len);
        existing.selected_range.end = existing.selected_range.end.min(text_len);
    }

    /// キャレット位置を単一の点に設定する
    /// 範囲選択をリセットして解除する処理
    #[inline]
    fn set_caret_position(
        id: EntityId,
        contents: &mut InputContents,
        caret: ByteIndex,
        edit_selections: &mut TextSelectionsSparse,
        edit_selection_start_index: &mut SelectionStartIndexSparse,
        edit_selected_rects: Option<&mut SelectedRectsSparse>,
    ) {
        contents.selected_range = caret..caret;
        contents.selection_reversed = false;

        edit_selections.insert(id, caret..caret);
        edit_selection_start_index.insert(id, caret);
        if let Some(rects) = edit_selected_rects {
            rects.remove(id);
        }
    }

    /// 範囲選択を更新
    #[inline]
    fn set_selection_range(
        id: EntityId,
        contents: &mut InputContents,
        range: Range<ByteIndex>,
        selection_reversed: bool,
        edit_selections: &mut TextSelectionsSparse,
    ) {
        contents.selected_range = range.clone();
        contents.selection_reversed = selection_reversed;
        edit_selections.insert(id, range);
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

        let buffer = SystemStore::get_or_create_layout(
            id,
            &mut cx.system.sys_text_engine,
            &cx.system.sys_text_buffers,
            &cx.contents.cont_text_contents,
            &cx.contents.cont_text_spans,
            &cx.layouts.lay_resolved_basic,
            &cx.layouts.lay_resolved_flex,
            &cx.renders.rnd_visual,
            &cx.outputs.out_rects,
            &mut cx.debug,
        );

        let local = OutputStore::pressed_local_point(
            id,
            pointer_pos,
            &buffer,
            &cx.contents.cont_input_contents,
            &cx.layouts.lay_resolved_basic,
            &cx.layouts.lay_resolved_flex,
            &cx.layouts.lay_resolved_grid,
            &cx.outputs.out_rects,
            &cx.states.scroll.sc_offsets,
            &mut cx.debug,
        );

        let Some(contents) = cx.contents.cont_input_contents.find_mut(id) else {
            return;
        };

        let text_val = contents.to_michiu();

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
                ByteIndex(0),
                &mut cx.states.edit.edit_selections,
                &mut cx.states.edit.edit_selection_start_index,
                Some(&mut cx.states.edit.edit_selected_rects),
            );
        } else {
            // 表示テキストを取得
            let display_text = cx
                .contents
                .cont_text_contents
                .find(id)
                .cloned()
                .unwrap_or_default();

            // 表示バッファ上でヒットテスト
            // 返ってくるのは表示テキスト上のバイト位置
            let (display_caret, is_trailing) = TextEngine::hit_test_point(&buffer, local);

            // 表示テキスト基準で次の文字境界へ進める
            let final_display_caret = if is_trailing {
                display_text.next_char_boundary(display_caret)
            } else {
                display_caret
            };

            // 表示テキスト上のバイト位置を生テキストのバイト位置に変換
            let final_caret = contents.display_byte_to_raw_byte(final_display_caret, &text_val);

            // 許容される最大長
            let editable_len = if is_placeholder {
                contents
                    .placeholder
                    .as_ref()
                    .map_or(ByteIndex(0), |p| ByteIndex(p.len()))
            } else {
                text_val.byte_len()
            };
            let final_caret_clamped = final_caret.min(editable_len);

            if mods.shift {
                let anchor = cx
                    .states
                    .edit
                    .edit_selection_start_index
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
                    &mut cx.states.edit.edit_selections,
                );
            } else {
                Element::set_caret_position(
                    id,
                    contents,
                    final_caret_clamped,
                    &mut cx.states.edit.edit_selections,
                    &mut cx.states.edit.edit_selection_start_index,
                    Some(&mut cx.states.edit.edit_selected_rects),
                );
            }
        }
        contents.last_interacted_time = Some(Instant::now());
        // キャレットの絶対座標と表示情報を一括更新
        cx.apply_input_update(id, InputOp::MousePress);
    }

    fn handle_input_focus_gained(cx: &mut Context, id: EntityId) {
        if let Some(contents) = cx.contents.cont_input_contents.find_mut(id) {
            contents.is_selecting = false;
            // フォーカス獲得時も操作時刻を記録して即座にキャレットを表示
            contents.last_interacted_time = Some(Instant::now());
        }
        cx.mark_render_dirty(id);
    }

    fn handle_input_char_typed(cx: &mut Context, id: EntityId, ch: &mut char) {
        let Some(contents) = cx.contents.cont_input_contents.find_mut(id) else {
            return;
        };

        // IME未変換の入力中 (composition_textがある間) は文字入力を無視
        if contents
            .ime_state
            .as_ref()
            .is_some_and(|s| !s.composition_text.is_empty())
        {
            return;
        }

        let mut is_allowed = !ch.is_control();

        // 複数行入力時に、Enterキー（'\r' / '\n'）が押された場合は改行コードとして許可
        if contents.is_multiline && (*ch == '\r' || *ch == '\n') {
            *ch = '\n';
            is_allowed = true;
        }

        if !is_allowed {
            return;
        }

        // 数値制限フィルター
        if contents.numeric_only && !ch.is_numeric() && *ch != '.' && *ch != '-' {
            return;
        }

        let text_val = contents.to_michiu();
        let range = contents.selected_range.clone();

        // 文字数制限のチェック
        if let Some(max) = contents.max_length {
            let selected_chars = text_val.slice(range.clone()).chars().count();
            let chars_after_delete = text_val.char_count() - selected_chars;
            if chars_after_delete + 1 > max {
                return; // 制限を超えるため入力を中断
            }
        }

        // 変更発生前に現在の状態をセーブ
        contents.record_undo(text_val, range.clone());
        contents.last_interacted_time = Some(Instant::now());

        let mut buf = [0u8; 4];
        let ch_str = ch.encode_utf8(&mut buf);
        let new_caret = contents.update_michiu(|m| m.replace_range(range, ch_str));

        Element::set_caret_position(
            id,
            contents,
            new_caret,
            &mut cx.states.edit.edit_selections,
            &mut cx.states.edit.edit_selection_start_index,
            Some(&mut cx.states.edit.edit_selected_rects),
        );
        cx.apply_input_update(id, InputOp::CharTyped);
    }

    fn pressed_back(
        id: EntityId,
        contents: &mut InputContents,
        caret: ByteIndex,
        text_val: &MichiuString,
        edit_selections: &mut TextSelectionsSparse,
        edit_selection_start_index: &mut SelectionStartIndexSparse,
    ) {
        let range = contents.selected_range.clone();
        contents.record_undo(text_val.clone(), range.clone());

        // 範囲選択があれば範囲削除、なければ通常の1文字バックスペース
        let new_caret = if range.start < range.end {
            contents.update_michiu(|m| m.remove_range(range))
        } else {
            contents.update_michiu(|m| m.backspace(caret))
        };

        Element::set_caret_position(
            id,
            contents,
            new_caret,
            edit_selections,
            edit_selection_start_index,
            None,
        );
        contents.last_interacted_time = Some(Instant::now());
    }

    fn pressed_delete(
        id: EntityId,
        contents: &mut InputContents,
        caret: ByteIndex,
        text_val: &MichiuString,
        edit_selections: &mut TextSelectionsSparse,
        edit_selection_start_index: &mut SelectionStartIndexSparse,
    ) {
        let range = contents.selected_range.clone();
        contents.record_undo(text_val.clone(), range.clone());

        // 範囲選択があれば範囲削除、なければ通常の1文字デリート
        let new_caret = if range.start < range.end {
            contents.update_michiu(|m| m.remove_range(range))
        } else {
            contents.update_michiu(|m| m.delete(caret))
        };

        Element::set_caret_position(
            id,
            contents,
            new_caret,
            edit_selections,
            edit_selection_start_index,
            None,
        );
        contents.last_interacted_time = Some(Instant::now());
    }

    fn pressed_left(
        id: EntityId,
        contents: &mut InputContents,
        text_val: &MichiuString,
        caret: ByteIndex,
        mods: Modifiers,
        edit_selections: &mut TextSelectionsSparse,
        edit_selection_start_index: &mut SelectionStartIndexSparse,
        edit_selected_rects: &mut SelectedRectsSparse,
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
                edit_selections,
                edit_selection_start_index,
                Some(edit_selected_rects),
            );
            contents.last_interacted_time = Some(Instant::now());
            return true;
        } else if caret.0 > 0 {
            let new_caret = text_val.prev_char_boundary(caret);

            if mods.shift {
                // Shiftキー押下中：選択の拡張
                let anchor = edit_selection_start_index
                    .find(id)
                    .copied()
                    .unwrap_or(new_caret);
                if !edit_selection_start_index.contains_key(id) {
                    edit_selection_start_index.insert(id, new_caret);
                }
                let (range, reversed) = if anchor <= new_caret {
                    (anchor..new_caret, false)
                } else {
                    (new_caret..anchor, true)
                };
                Element::set_selection_range(id, contents, range, reversed, edit_selections);
            } else {
                // Shiftキー非押下：選択解除して単なる移動
                Element::set_caret_position(
                    id,
                    contents,
                    new_caret,
                    edit_selections,
                    edit_selection_start_index,
                    Some(edit_selected_rects),
                );
            }
            contents.last_interacted_time = Some(Instant::now());
            return true;
        }
        false
    }

    fn pressed_right(
        id: EntityId,
        contents: &mut InputContents,
        text_val: &MichiuString,
        caret: ByteIndex,
        mods: Modifiers,
        edit_selections: &mut TextSelectionsSparse,
        edit_selection_start_index: &mut SelectionStartIndexSparse,
        edit_selected_rects: &mut SelectedRectsSparse,
        debug: &mut DebugStore,
    ) -> bool {
        let range = contents.selected_range.clone();
        // 選択範囲が存在し、かつ Shiftキーが押されていない通常移動時（全選択中での右移動に完全対応）
        if range.start < range.end && !mods.shift {
            Element::set_caret_position(
                id,
                contents,
                range.end,
                edit_selections,
                edit_selection_start_index,
                Some(edit_selected_rects),
            );
            contents.last_interacted_time = Some(Instant::now());
            return true;
        } else if caret < text_val.byte_len() {
            // 現在位置にある文字を取得し、その文字のバイト数分だけキャレットを進める
            let new_caret = text_val.next_char_boundary(caret);

            if mods.shift {
                let anchor = *edit_selection_start_index.find_or(id, &caret, debug);
                if !edit_selection_start_index.contains_key(id) {
                    edit_selection_start_index.insert(id, caret);
                }
                let (range, reversed) = if anchor <= new_caret {
                    (anchor..new_caret, false)
                } else {
                    (new_caret..anchor, true)
                };
                Element::set_selection_range(id, contents, range, reversed, edit_selections);
            } else {
                Element::set_caret_position(
                    id,
                    contents,
                    new_caret,
                    edit_selections,
                    edit_selection_start_index,
                    Some(edit_selected_rects),
                );
            }
            contents.last_interacted_time = Some(Instant::now());
            return true;
        }
        false
    }

    fn pressed_up(
        id: EntityId,
        contents: &mut InputContents,
        buffer: &Buffer,
        caret: ByteIndex,
        text_val: &MichiuString,
        mods: Modifiers,
        edit_selections: &mut TextSelectionsSparse,
        edit_selection_start_index: &mut SelectionStartIndexSparse,
        debug: &mut DebugStore,
    ) -> bool {
        if !contents.is_multiline {
            return false;
        }

        let (cx_offset, cy_offset, ch_height) = TextEngine::get_caret_position(buffer, caret);

        let target_y = (cy_offset - ch_height * 0.5).max(0.0);

        let (new_caret, is_trailing) =
            TextEngine::hit_test_point(buffer, LayoutPoint::new(cx_offset, target_y));

        let final_caret = if is_trailing {
            text_val.next_char_boundary(new_caret)
        } else {
            new_caret
        };

        if mods.shift {
            let anchor = *edit_selection_start_index.find_or(id, &caret, debug);
            if !edit_selection_start_index.contains_key(id) {
                edit_selection_start_index.insert(id, caret);
            }
            let (range, reversed) = if anchor <= final_caret {
                (anchor..final_caret, false)
            } else {
                (final_caret..anchor, true)
            };
            Element::set_selection_range(id, contents, range, reversed, edit_selections);
        } else {
            Element::set_caret_position(
                id,
                contents,
                final_caret,
                edit_selections,
                edit_selection_start_index,
                None,
            );
        }

        contents.last_interacted_time = Some(Instant::now());
        true
    }

    fn pressed_down(
        id: EntityId,
        contents: &mut InputContents,
        buffer: &Buffer,
        caret: ByteIndex,
        text_val: &MichiuString,
        mods: Modifiers,
        edit_selections: &mut TextSelectionsSparse,
        edit_selection_start_index: &mut SelectionStartIndexSparse,
        debug: &mut DebugStore,
    ) -> bool {
        if !contents.is_multiline {
            return false;
        }

        let (cx_offset, cy_offset, ch_height) = TextEngine::get_caret_position(buffer, caret);

        let target_y = cy_offset + ch_height * 1.5;

        let (new_caret, is_trailing) =
            TextEngine::hit_test_point(buffer, LayoutPoint::new(cx_offset, target_y));

        let final_caret = if is_trailing {
            text_val.next_char_boundary(new_caret)
        } else {
            new_caret
        };

        if mods.shift {
            let anchor = *edit_selection_start_index.find_or(id, &caret, debug);
            if !edit_selection_start_index.contains_key(id) {
                edit_selection_start_index.insert(id, caret);
            }
            let (range, reversed) = if anchor <= final_caret {
                (anchor..final_caret, false)
            } else {
                (final_caret..anchor, true)
            };
            Element::set_selection_range(id, contents, range, reversed, edit_selections);
        } else {
            Element::set_caret_position(
                id,
                contents,
                final_caret,
                edit_selections,
                edit_selection_start_index,
                None,
            );
        }

        contents.last_interacted_time = Some(Instant::now());
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

        let buffer = cx.get_or_create_layout(id);

        let Some(contents) = cx.contents.cont_input_contents.find_mut(id) else {
            return;
        };

        let text_val = contents.to_michiu();
        let text_len = text_val.byte_len();

        contents.selected_range =
            contents.selected_range.start.min(text_len)..contents.selected_range.end.min(text_len);

        let raw_caret = if contents.selection_reversed {
            contents.selected_range.start
        } else {
            contents.selected_range.end
        };
        let caret = raw_caret.min(text_len);
        let mut changed = false; // 状態変更フラグ

        match key {
            VirtualKey::BACK => {
                Element::pressed_back(
                    id,
                    contents,
                    caret,
                    &text_val,
                    &mut cx.states.edit.edit_selections,
                    &mut cx.states.edit.edit_selection_start_index,
                );
                cx.apply_input_update(id, InputOp::Backspace);
            }
            VirtualKey::DELETE => {
                Element::pressed_delete(
                    id,
                    contents,
                    caret,
                    &text_val,
                    &mut cx.states.edit.edit_selections,
                    &mut cx.states.edit.edit_selection_start_index,
                );
                cx.apply_input_update(id, InputOp::Delete);
            }
            VirtualKey::LEFT => {
                changed = Element::pressed_left(
                    id,
                    contents,
                    &text_val,
                    caret,
                    mods,
                    &mut cx.states.edit.edit_selections,
                    &mut cx.states.edit.edit_selection_start_index,
                    &mut cx.states.edit.edit_selected_rects,
                );
            }
            VirtualKey::RIGHT => {
                changed = Element::pressed_right(
                    id,
                    contents,
                    &text_val,
                    caret,
                    mods,
                    &mut cx.states.edit.edit_selections,
                    &mut cx.states.edit.edit_selection_start_index,
                    &mut cx.states.edit.edit_selected_rects,
                    &mut cx.debug,
                );
            }
            VirtualKey::UP => {
                changed = Element::pressed_up(
                    id,
                    contents,
                    &buffer,
                    caret,
                    &text_val,
                    mods,
                    &mut cx.states.edit.edit_selections,
                    &mut cx.states.edit.edit_selection_start_index,
                    &mut cx.debug,
                );
            }
            VirtualKey::DOWN => {
                changed = Element::pressed_down(
                    id,
                    contents,
                    &buffer,
                    caret,
                    &text_val,
                    mods,
                    &mut cx.states.edit.edit_selections,
                    &mut cx.states.edit.edit_selection_start_index,
                    &mut cx.debug,
                );
            }
            _ => {}
        }

        if changed {
            cx.apply_input_update(id, InputOp::ArrowMove);
        }
    }

    fn handle_input_ime_updated(cx: &mut Context, id: EntityId, ime: &ImeState) {
        let Some(contents) = cx.contents.cont_input_contents.find_mut(id) else {
            return;
        };

        contents.last_interacted_time = Some(Instant::now());
        contents.ime_state = Some(ime.clone());

        let mut text_val = contents.to_michiu();
        let range = contents.selected_range.clone();

        // 選択範囲が存在し、かつIME入力（未変換または確定）が開始される場合、
        // 文字入力の直前に選択範囲の文字列をあらかじめ消去・置換する
        if range.start < range.end
            && (!ime.composition_text.is_empty() || !ime.result_text.is_empty())
        {
            // Undo履歴に削除前の状態を記録
            contents.record_undo(text_val.clone(), range.clone());

            let caret = contents.update_michiu(|m| m.remove_range(range));

            // キャレット・選択範囲を消去開始位置に一度リセットして同期
            Element::set_caret_position(
                id,
                contents,
                caret,
                &mut cx.states.edit.edit_selections,
                &mut cx.states.edit.edit_selection_start_index,
                Some(&mut cx.states.edit.edit_selected_rects),
            );

            // 後続の処理で使うため、最新の文字列を取得しておく
            text_val = contents.to_michiu();
        }

        // IME 確定文字の書き込み
        if !ime.result_text.is_empty() {
            // 文字数制限の計算
            let allowed_len = if let Some(max) = contents.max_length {
                let current_chars = text_val.char_count().0;
                if current_chars >= max.0 {
                    // すでに制限いっぱいなら確定文字を破棄して終了
                    contents.marked_range = None;
                    return;
                }
                max.0 - current_chars
            } else {
                usize::MAX
            };

            // 確定文字のフィルタリング＆切り詰め
            let mut filtered = String::new();
            let mut chars_added = 0;

            for c in ime.result_text.chars() {
                // 数値制限フィルタ
                if contents.numeric_only && !c.is_numeric() && c != '.' && c != '-' {
                    continue;
                }
                // 文字数制限カット
                if chars_added >= allowed_len {
                    break;
                }
                filtered.push(c);
                chars_added += 1;
            }

            if !filtered.is_empty() {
                let caret = contents.selected_range.start;
                let new_caret = contents.update_michiu(|m| m.insert_str(caret, &filtered));

                Element::set_caret_position(
                    id,
                    contents,
                    new_caret,
                    &mut cx.states.edit.edit_selections,
                    &mut cx.states.edit.edit_selection_start_index,
                    Some(&mut cx.states.edit.edit_selected_rects),
                );
            }
            contents.marked_range = None;
        } else if !ime.composition_text.is_empty() {
            // IME 未変換中
            let caret = contents.selected_range.start;
            let comp_len = ime.composition_text.byte_len();
            contents.marked_range = Some(caret..(caret + comp_len));
        } else {
            contents.marked_range = None;
        }

        // IME の未確定状態（未確定波線、変換フォーカス太線/細線）を TextSpan に自動マッピング
        if ime.composition_text.is_empty() {
            cx.contents.cont_text_spans.remove(id);
            cx.topology
                .topo_active_masks
                .at_mut(id)
                .unset(ComponentMask::STYLE_TEXT_SPANS);
        } else {
            let mut spans = Vec::new();
            let caret = contents.selected_range.start;

            // Windowsから送られてくる属性情報のインデックス（start_idx と end_idx）はバイト数ではなく文字単位
            // UTF-8のときは文字単位インデックスをバイト単位にマッピングするためのリストを作成
            let char_byte_offsets: Vec<ByteIndex> = {
                let mut offsets = Vec::with_capacity(ime.composition_text.char_count().0 + 1);
                let mut curr_byte = ByteIndex(0);
                for c in ime.composition_text.chars() {
                    offsets.push(curr_byte);
                    curr_byte += c.len_utf8();
                }
                offsets.push(curr_byte);
                offsets
            };

            if ime.composition_attrs.is_empty() {
                // 属性が取得できない場合のフォールバック（全体を未確定波線に設定）
                let comp_len = ime.composition_text.byte_len();
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

                    let span_start = caret + char_byte_offsets[start_idx];
                    let span_end = caret + char_byte_offsets[end_idx];

                    spans.push(TextSpan {
                        range: span_start..span_end,
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
            cx.topology
                .topo_active_masks
                .at_mut(id)
                .set(ComponentMask::STYLE_TEXT_SPANS);
        }

        // IMEイベント終了（または変換中）に表示テキストとキャレット位置を再計算・同期させる
        cx.apply_input_update(id, InputOp::ImeUpdated);
    }

    /// 入力イベント（キー、IME、文字入力、フォーカス）を自動的にマッピングして代行するロジック
    fn input_internal(self, cx: &mut Context, mut c: InputContents) {
        let id = self.id;

        // シグナル更新やテーマ変更、親コンポーネントの再レンダリングによる
        // キャレット位置（selected_range）や Undo/Redo 履歴の末尾への強制初期化を防止
        // 既存の状態を検知した場合はデザイン設定のみを上書き
        if let Some(existing) = cx.contents.cont_input_contents.find_mut(id) {
            Element::sync_existing_input_properties(existing, c);
            // 早期リターンを抜ける前に、最新の文字列状態を SoA / DWrite 側へ即座に同期・反映
            cx.apply_input_update(id, InputOp::Init);
            // これ以降の初期化を完全にスキップして早期リターン
            return;
        }

        // 最初のロード時、シグナルから現在値を取得して内部カーソルを末尾に合わせる
        let current_text = c.to_michiu();
        let current_len = current_text.byte_len();
        c.selected_range = current_len..current_len;

        cx.contents.cont_input_contents.insert(id, c);
        cx.topology
            .topo_active_masks
            .at_mut(id)
            .set(ComponentMask::COMP_INPUT_CONTENT | ComponentMask::COMP_TEXT_CONTENT);

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
            if let Some(contents) = cx.contents.cont_input_contents.find(id) {
                let _base_text_val = contents.text.0.get();
            }
            cx.apply_input_update(id, InputOp::TextEffect);
        });
    }
}
