use crate::{
    ComponentMask, Context, EffectCategory, Element, ElementState, EntityId, ImeState,
    InputContents, InputOp, Modifiers, MouseButton, OutputStore, Prop,
    SelectedRectsSparseSecondary, SelectionStartIndexSparseSecondary, SystemStore, TextEngine,
    TextLayoutEngine, TextSelectionsSparseSecondary, TextSpan, UnderlineStyle, VirtualKey,
    VisualProperty, with_context,
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

    fn sync_existing_input_properties(
        existing: &mut InputContents,
        c: InputContents,
        cosmic: bool,
    ) {
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
        let current_text = existing.text.0.get();
        let text_len = if cosmic {
            current_text.len()
        } else {
            current_text.encode_utf16().count()
        };
        existing.selected_range.start = existing.selected_range.start.min(text_len);
        existing.selected_range.end = existing.selected_range.end.min(text_len);
    }

    /// キャレット位置を単一の点に設定する
    /// 範囲選択をリセットして解除する処理
    #[inline]
    fn set_caret_position(
        id: EntityId,
        contents: &mut InputContents,
        caret: usize,
        edit_selections: &mut TextSelectionsSparseSecondary,
        edit_selection_start_index: &mut SelectionStartIndexSparseSecondary,
        edit_selected_rects: Option<&mut SelectedRectsSparseSecondary>,
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
        range: std::ops::Range<usize>,
        selection_reversed: bool,
        edit_selections: &mut TextSelectionsSparseSecondary,
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

        let engine = SystemStore::get_or_create_layout(
            id,
            cx.cosmic,
            &mut cx.system.sys_text_engine,
            &cx.system.sys_dwrite_layouts,
            &cx.contents.cont_text_contents,
            &cx.contents.cont_text_spans,
            &cx.layouts.lay_resolved_basic,
            &cx.layouts.lay_resolved_flex,
            &cx.renders.rnd_visual,
            &cx.outputs.out_rects,
        );

        let local = OutputStore::pressed_local_point(
            id,
            pointer_pos,
            engine.as_ref(),
            &mut cx.system.sys_text_engine,
            &cx.contents.cont_input_contents,
            &cx.topology.topo_active_masks,
            &cx.topology.topo_parents,
            &cx.layouts.lay_resolved_basic,
            &cx.layouts.lay_resolved_flex,
            &cx.layouts.lay_resolved_grid,
            &cx.renders.rnd_interaction,
            &cx.renders.rnd_active_transitions,
            &cx.renders.rnd_visual,
            &cx.outputs.out_rects,
            &cx.states.scroll.sc_offsets,
        );

        let Some(contents) = cx.contents.cont_input_contents.get_mut(id) else {
            return;
        };

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
                &mut cx.states.edit.edit_selections,
                &mut cx.states.edit.edit_selection_start_index,
                Some(&mut cx.states.edit.edit_selected_rects),
            );
        } else {
            let Some(engine) = SystemStore::get_or_create_layout(
                id,
                cx.cosmic,
                &mut cx.system.sys_text_engine,
                &cx.system.sys_dwrite_layouts,
                &cx.contents.cont_text_contents,
                &cx.contents.cont_text_spans,
                &cx.layouts.lay_resolved_basic,
                &cx.layouts.lay_resolved_flex,
                &cx.renders.rnd_visual,
                &cx.outputs.out_rects,
            ) else {
                return;
            };
            // キャッシュ済みのレイアウトをそのまま使って高速にヒットテスト
            let (new_caret, is_trailing) = match &engine {
                TextLayoutEngine::Cosmic(buffer) => cx
                    .system
                    .sys_text_engine
                    .hit_test_point_cosmic(buffer, local.x, local.y),
                TextLayoutEngine::DWrite(dw_layout) => cx
                    .system
                    .sys_text_engine
                    .hit_test_point(dw_layout, local.x, local.y),
            };
            // UTF-8の場合は次の文字境界までバイト数分進める
            let final_caret = if cx.cosmic {
                if is_trailing && new_caret < text_val.len() {
                    let current_char = text_val[new_caret..].chars().next().unwrap_or(' ');
                    new_caret + current_char.len_utf8()
                } else {
                    new_caret
                }
            } else {
                if is_trailing {
                    new_caret + 1
                } else {
                    new_caret
                }
            };

            let editable_len = if is_placeholder {
                contents.placeholder.as_ref().map_or(0, |p| {
                    if cx.cosmic {
                        p.len()
                    } else {
                        p.encode_utf16().count()
                    }
                })
            } else {
                // 通常の文字列長
                if cx.cosmic {
                    text_val.len()
                } else {
                    text_val.encode_utf16().count()
                }
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
        contents.last_interacted_time = Some(std::time::Instant::now());
        // キャレットの絶対座標と表示情報を一括更新
        cx.apply_input_update(id, InputOp::MousePress);
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

        if cx.cosmic {
            // キャレット範囲をクランプし安全な文字境界に補正
            let mut start = range.start.min(text_val.len());
            let mut end = range.end.min(text_val.len());

            while start > 0 && !text_val.is_char_boundary(start) {
                start -= 1;
            }
            while end > 0 && !text_val.is_char_boundary(end) {
                end -= 1;
            }

            let left = &text_val[..start];
            let right = &text_val[end..];

            // 削除後の文字数 ＋ 挿入する1文字が制限を超えないか
            let chars_after_delete = left.chars().count() + right.chars().count();
            if let Some(max) = contents.max_length
                && chars_after_delete + 1 > max
            {
                return; // 制限を超えるため入力を中断
            }

            contents.last_interacted_time = Some(std::time::Instant::now());

            // 文字列の結合
            let mut new_text = String::with_capacity(left.len() + ch.len_utf8() + right.len());
            new_text.push_str(left);
            new_text.push(*ch);
            new_text.push_str(right);

            // キャレット位置を挿入した文字のバイト数分だけ進める
            let new_caret = start + ch.len_utf8();

            Element::set_caret_position(
                id,
                contents,
                new_caret,
                &mut cx.states.edit.edit_selections,
                &mut cx.states.edit.edit_selection_start_index,
                Some(&mut cx.states.edit.edit_selected_rects),
            );
            contents.text.1.set(new_text);
        } else {
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
                &mut cx.states.edit.edit_selections,
                &mut cx.states.edit.edit_selection_start_index,
                Some(&mut cx.states.edit.edit_selected_rects),
            );
            contents.text.1.set(new_text);
        }

        cx.apply_input_update(id, InputOp::CharTyped);
    }

    fn pressed_back(
        id: EntityId,
        contents: &mut InputContents,
        caret: &mut usize,
        text_val: &str,
        edit_selections: &mut TextSelectionsSparseSecondary,
        edit_selection_start_index: &mut SelectionStartIndexSparseSecondary,
        cosmic: bool,
    ) {
        let range = contents.selected_range.clone();
        contents.record_undo(text_val.to_string(), range.clone());

        if range.start < range.end {
            let new_text = if cosmic {
                InputContents::remove_range_utf8_byte(text_val, &range)
            } else {
                InputContents::remove_utf16_range(text_val, &range)
            };

            Element::set_caret_position(
                id,
                contents,
                range.start,
                edit_selections,
                edit_selection_start_index,
                None,
            );
            contents.text.1.set(new_text);
        } else {
            // 通常の1文字バックスペース
            let new_text = if cosmic {
                InputContents::input_backspace_utf8_byte(text_val, caret)
            } else {
                InputContents::input_backspace(text_val, caret)
            };
            Element::set_caret_position(
                id,
                contents,
                *caret,
                edit_selections,
                edit_selection_start_index,
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
        edit_selections: &mut TextSelectionsSparseSecondary,
        edit_selection_start_index: &mut SelectionStartIndexSparseSecondary,
        cosmic: bool,
    ) {
        let range = contents.selected_range.clone();
        contents.record_undo(text_val.to_string(), range.clone());
        if range.start < range.end {
            let new_text = if cosmic {
                InputContents::remove_range_utf8_byte(text_val, &range)
            } else {
                InputContents::remove_utf16_range(text_val, &range)
            };

            Element::set_caret_position(
                id,
                contents,
                range.start,
                edit_selections,
                edit_selection_start_index,
                None,
            );
            contents.text.1.set(new_text);
        } else {
            // 通常の1文字デリート
            let new_text = if cosmic {
                InputContents::input_delete_utf8_byte(text_val, caret)
            } else {
                InputContents::input_delete(text_val, caret)
            };
            Element::set_caret_position(
                id,
                contents,
                caret,
                edit_selections,
                edit_selection_start_index,
                None,
            );
            contents.text.1.set(new_text);
        }
        contents.last_interacted_time = Some(std::time::Instant::now());
    }

    fn pressed_left(
        id: EntityId,
        contents: &mut InputContents,
        text_val: &str,
        caret: usize,
        mods: Modifiers,
        edit_selections: &mut TextSelectionsSparseSecondary,
        edit_selection_start_index: &mut SelectionStartIndexSparseSecondary,
        edit_selected_rects: &mut SelectedRectsSparseSecondary,
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
            contents.last_interacted_time = Some(std::time::Instant::now());
            return true;
        } else if caret > 0 {
            let mut prev = caret - 1;
            // 安全な文字の境界にぶつかるまで左に
            while prev > 0 && !text_val.is_char_boundary(prev) {
                prev -= 1;
            }

            let new_caret = prev;

            if mods.shift {
                // Shiftキー押下中：選択の拡張
                let anchor = edit_selection_start_index.get(id).copied().unwrap_or(caret);
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
            contents.last_interacted_time = Some(std::time::Instant::now());
            return true;
        }
        false
    }

    fn pressed_right(
        id: EntityId,
        contents: &mut InputContents,
        text_val: &str,
        caret: usize,
        text_len: usize,
        mods: Modifiers,
        edit_selections: &mut TextSelectionsSparseSecondary,
        edit_selection_start_index: &mut SelectionStartIndexSparseSecondary,
        edit_selected_rects: &mut SelectedRectsSparseSecondary,
    ) -> bool {
        let range = contents.selected_range.clone();
        // 選択範囲が存在し、かつ Shiftキーが押されていない通常移動時（全選択中での右移動に完全対応）
        if range.start < range.end && !mods.shift {
            let new_caret = range.end;
            Element::set_caret_position(
                id,
                contents,
                new_caret,
                edit_selections,
                edit_selection_start_index,
                Some(edit_selected_rects),
            );
            contents.last_interacted_time = Some(std::time::Instant::now());
            return true;
        } else if caret < text_len {
            // 現在位置にある文字を取得
            let current_char = text_val[caret..].chars().next().unwrap_or(' ');
            // その文字のバイト数分だけキャレットを進める
            let new_caret = caret + current_char.len_utf8();

            if mods.shift {
                let anchor = edit_selection_start_index.get(id).copied().unwrap_or(caret);
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
            contents.last_interacted_time = Some(std::time::Instant::now());
            return true;
        }
        false
    }

    fn pressed_up(
        id: EntityId,
        contents: &mut InputContents,
        engine: &TextLayoutEngine,
        caret: usize,
        text_val: &str,
        text_len: usize,
        mods: Modifiers,
        sys_text_engine: &mut TextEngine,
        edit_selections: &mut TextSelectionsSparseSecondary,
        edit_selection_start_index: &mut SelectionStartIndexSparseSecondary,
        cosmic: bool,
    ) -> bool {
        if !contents.is_multiline {
            return false;
        }

        let (cx_offset, cy_offset, ch_height) = match &engine {
            TextLayoutEngine::Cosmic(buffer) => {
                sys_text_engine.get_caret_position_cosmic(buffer, caret, text_len)
            }
            TextLayoutEngine::DWrite(dw_layout) => {
                sys_text_engine.get_caret_position(dw_layout, caret, text_len)
            }
        };

        let target_y = (cy_offset - ch_height * 0.5).max(0.0);

        let (new_caret, is_trailing) = match &engine {
            TextLayoutEngine::Cosmic(buffer) => {
                sys_text_engine.hit_test_point_cosmic(buffer, cx_offset, target_y)
            }
            TextLayoutEngine::DWrite(dw_layout) => {
                sys_text_engine.hit_test_point(dw_layout, cx_offset, target_y)
            }
        };
        let final_caret = if cosmic {
            if is_trailing && new_caret < text_len {
                let current_char = text_val[new_caret..].chars().next().unwrap_or(' ');
                new_caret + current_char.len_utf8()
            } else {
                new_caret
            }
        } else {
            if is_trailing {
                new_caret + 1
            } else {
                new_caret
            }
        };

        if mods.shift {
            let anchor = edit_selection_start_index.get(id).copied().unwrap_or(caret);
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

        contents.last_interacted_time = Some(std::time::Instant::now());
        true
    }

    fn pressed_down(
        id: EntityId,
        contents: &mut InputContents,
        engine: &TextLayoutEngine,
        caret: usize,
        text_val: &str,
        text_len: usize,
        mods: Modifiers,
        sys_text_engine: &mut TextEngine,
        edit_selections: &mut TextSelectionsSparseSecondary,
        edit_selection_start_index: &mut SelectionStartIndexSparseSecondary,
        cosmic: bool,
    ) -> bool {
        if !contents.is_multiline {
            return false;
        }

        let (cx_offset, cy_offset, ch_height) = match &engine {
            TextLayoutEngine::Cosmic(buffer) => {
                sys_text_engine.get_caret_position_cosmic(buffer, caret, text_len)
            }
            TextLayoutEngine::DWrite(dw_layout) => {
                sys_text_engine.get_caret_position(dw_layout, caret, text_len)
            }
        };

        let target_y = cy_offset + ch_height * 1.5;

        let (new_caret, is_trailing) = match &engine {
            TextLayoutEngine::Cosmic(buffer) => {
                sys_text_engine.hit_test_point_cosmic(buffer, cx_offset, target_y)
            }
            TextLayoutEngine::DWrite(dw_layout) => {
                sys_text_engine.hit_test_point(dw_layout, cx_offset, target_y)
            }
        };
        let final_caret = if cosmic {
            if is_trailing && new_caret < text_len {
                let current_char = text_val[new_caret..].chars().next().unwrap_or(' ');
                new_caret + current_char.len_utf8()
            } else {
                new_caret
            }
        } else {
            if is_trailing {
                new_caret + 1
            } else {
                new_caret
            }
        };

        if mods.shift {
            let anchor = edit_selection_start_index.get(id).copied().unwrap_or(caret);
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

        let Some(engine) = cx.get_or_create_layout(id) else {
            return;
        };

        let Some(contents) = cx.contents.cont_input_contents.get_mut(id) else {
            return;
        };

        let text_val = contents.text.0.get();
        let text_len = if cx.cosmic {
            text_val.len()
        } else {
            text_val.encode_utf16().count()
        };

        contents.selected_range = (contents.selected_range.start.min(text_len))
            ..(contents.selected_range.end.min(text_len));

        let raw_caret = if contents.selection_reversed {
            contents.selected_range.start
        } else {
            contents.selected_range.end
        };
        let mut caret = raw_caret.min(text_len);
        let mut changed = false; // 状態変更フラグ

        match key {
            VirtualKey::BACK => {
                Element::pressed_back(
                    id,
                    contents,
                    &mut caret,
                    &text_val,
                    &mut cx.states.edit.edit_selections,
                    &mut cx.states.edit.edit_selection_start_index,
                    cx.cosmic,
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
                    cx.cosmic,
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
                    text_len,
                    mods,
                    &mut cx.states.edit.edit_selections,
                    &mut cx.states.edit.edit_selection_start_index,
                    &mut cx.states.edit.edit_selected_rects,
                );
            }
            VirtualKey::UP => {
                changed = Element::pressed_up(
                    id,
                    contents,
                    &engine,
                    caret,
                    &text_val,
                    text_len,
                    mods,
                    &mut cx.system.sys_text_engine,
                    &mut cx.states.edit.edit_selections,
                    &mut cx.states.edit.edit_selection_start_index,
                    cx.cosmic,
                );
            }
            VirtualKey::DOWN => {
                changed = Element::pressed_down(
                    id,
                    contents,
                    &engine,
                    caret,
                    &text_val,
                    text_len,
                    mods,
                    &mut cx.system.sys_text_engine,
                    &mut cx.states.edit.edit_selections,
                    &mut cx.states.edit.edit_selection_start_index,
                    cx.cosmic,
                );
            }
            _ => {}
        }

        if changed {
            cx.apply_input_update(id, InputOp::ArrowMove);
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

            let new_text = if cx.cosmic {
                InputContents::remove_range_utf8_byte(&text_val, &range)
            } else {
                InputContents::remove_utf16_range(&text_val, &range)
            };
            let caret = range.start;

            // キャレット・選択範囲を消去開始位置に一度リセットして同期
            Element::set_caret_position(
                id,
                contents,
                caret,
                &mut cx.states.edit.edit_selections,
                &mut cx.states.edit.edit_selection_start_index,
                Some(&mut cx.states.edit.edit_selected_rects),
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
                if cx.cosmic {
                    temp_text = InputContents::input_insert_char_utf8_byte(
                        &temp_text,
                        &mut caret,
                        ch,
                        contents.max_length,
                        contents.numeric_only,
                    );
                } else {
                    temp_text = InputContents::input_insert_char(
                        &temp_text,
                        &mut caret,
                        ch,
                        contents.max_length,
                        contents.numeric_only,
                    );
                }
            }

            // 確定したキャレット位置で SoA 側の選択状態と開始アンカーを同期
            Element::set_caret_position(
                id,
                contents,
                caret,
                &mut cx.states.edit.edit_selections,
                &mut cx.states.edit.edit_selection_start_index,
                Some(&mut cx.states.edit.edit_selected_rects),
            );

            contents.text.1.set(temp_text);
            contents.marked_range = None;
        } else if !ime.composition_text.is_empty() {
            // IME 未変換中
            let caret = contents.selected_range.start;
            let comp_len = if cx.cosmic {
                ime.composition_text.len()
            } else {
                ime.composition_text.encode_utf16().count()
            };
            contents.marked_range = Some(caret..(caret + comp_len));
        } else {
            contents.marked_range = None;
        }

        // IME の未確定状態（未確定波線、変換フォーカス太線/細線）を TextSpan に自動マッピング
        if ime.composition_text.is_empty() {
            cx.contents.cont_text_spans.remove(id);
            cx.topology.topo_active_masks[id].unset(ComponentMask::STYLE_TEXT_SPANS);
        } else {
            let mut spans = Vec::new();
            let caret = contents.selected_range.start;

            if ime.composition_attrs.is_empty() {
                // 属性が取得できない場合のフォールバック（全体を未確定波線に設定）
                let comp_len = if cx.cosmic {
                    ime.composition_text.len()
                } else {
                    ime.composition_text.encode_utf16().count()
                };
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
            cx.topology.topo_active_masks[id].set(ComponentMask::STYLE_TEXT_SPANS);
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
        if let Some(existing) = cx.contents.cont_input_contents.get_mut(id) {
            Element::sync_existing_input_properties(existing, c, cx.cosmic);
            // 早期リターンを抜ける前に、最新の文字列状態を SoA / DWrite 側へ即座に同期・反映
            cx.apply_input_update(id, InputOp::Init);
            // これ以降の初期化を完全にスキップして早期リターン
            return;
        }

        // 最初のロード時、シグナルから現在値を取得して内部カーソルを末尾に合わせる
        let current_text = c.text.0.get();
        let current_len = if cx.cosmic {
            current_text.len()
        } else {
            current_text.encode_utf16().count()
        };
        c.selected_range = current_len..current_len;

        cx.contents.cont_input_contents.insert(id, c);
        cx.topology.topo_active_masks[id]
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
            if let Some(contents) = cx.contents.cont_input_contents.get(id) {
                let _base_text_val = contents.text.0.get();
            }
            cx.apply_input_update(id, InputOp::TextEffect);
        });
    }
}
