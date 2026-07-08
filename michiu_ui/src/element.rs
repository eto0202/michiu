use windows::Win32::UI::Input::{
    Ime::{
        CANDIDATEFORM, CFS_EXCLUDE, CFS_POINT, COMPOSITIONFORM, ImmGetContext, ImmReleaseContext,
        ImmSetCandidateWindow, ImmSetCompositionWindow,
    },
    KeyboardAndMouse::GetFocus,
};

use crate::{
    Color, Context, EffectCategory, ElementState, EntityId, EventListeners, ImageMetadata,
    ImageSource, ImeState, InputContents, LayoutPoint, LayoutRect, Length, LinearGradient,
    Modifiers, MouseButton, MovieMetadata, MovieProperty, ReadSignal, TextSpan, Transform,
    UiaValue, UnderlineStyle, VirtualKey, VisualProperty, WebView2Contents, bitmap::*,
    create_effect, div_n, style::ThisStyle,
};
use std::{borrow::Cow, cell::Cell, path::PathBuf};

thread_local! {
    // 現在構築中のUIコンテキストへの生ポインタを一時的にバインドするグローバルスレッド領域。
    // UI構築は常に単一のスレッド（メインスレッド）で行われるため、このアプローチは安全に機能。
    static ACTIVE_CONTEXT: Cell<Option<*mut Context>> = const { Cell::new(None) };
}

/// ユーザーがコンポーネントを評価する際に呼び出すグローバルラッパー
pub fn build_ui(cx: &mut Context, f: impl FnOnce() -> Element) -> Element {
    let old = ACTIVE_CONTEXT.get();
    ACTIVE_CONTEXT.set(Some(cx as *mut Context));
    let _guard = ContextGuard { old };

    let marker = cx.start_session();
    let result = f();
    // 戻り値に含まれるハンドルをルート要素として登録
    cx.register_root(result.id);
    // 親子関係に組み込まれなかった無駄な孤児を自動一掃
    cx.end_session(marker);
    result
}

/// スレッドローカルから安全にContextへのアクセスを解決する内部ヘルパー
#[inline(always)]
pub(crate) fn with_context<R>(f: impl FnOnce(&mut Context) -> R) -> R {
    let ptr = ACTIVE_CONTEXT
        .get()
        .expect("No active UI Context found in this thread context");
    // UIスレッドは単一かつ非同期にまたがらないため、ポインタの生存期間は保証される。
    unsafe { f(&mut *ptr) }
}

/// 構築が完了したUI要素を表す軽量なハンドル
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Element {
    pub id: EntityId,
}

impl Default for Element {
    fn default() -> Self {
        Self::new()
    }
}

impl Element {
    /// 新規要素の構築を開始します
    #[inline]
    pub fn new() -> Self {
        // スレッドローカルの Context から安全に要素を spawn
        let id = with_context(|cx| cx.spawn(None));
        Self { id }
    }

    #[inline]
    pub fn id(&self) -> EntityId {
        self.id
    }

    #[inline]
    pub fn from_id(id: EntityId) -> Element {
        Element { id }
    }

    /// スタイルを適用します（静的な値、Signal、またはクロージャ）。
    pub fn style(self, style: impl Into<Prop<ThisStyle>>) -> Self {
        match style.into() {
            Prop::None => {}
            Prop::Static(s) => with_context(|cx| self.style_internal(cx, s)),
            Prop::Dynamic(f) => {
                let id = self.id;
                let effect_id = create_effect(move |cx| {
                    let s = f();
                    let element = Element { id };
                    element.style_internal(cx, s);
                });
                with_context(|cx| cx.register_element_effect(id, EffectCategory::Style, effect_id));
            }
        }
        self
    }

    /// スタイルの適用（一括インライン展開）
    pub(crate) fn style_internal(self, cx: &mut Context, style: ThisStyle) {
        let inner = &style.inner;
        let mask = inner.mask;

        // active_masks にスタイル側のマスクをマージするが、
        // 動的なインタラクション状態フラグ（STYLE_INTERACTION_PROPERTY）は
        // 実行時にのみ制御されるべきなので、ここでは除外（マスクアウト）する
        let property_only_mask = mask.0 & !STYLE_INTERACTION_PROPERTY;
        cx.active_masks[self.id].0 |= property_only_mask;

        // 1. ベーススタイル（不変の基準値）として登録
        if mask.has_basic_layout() {
            cx.base_basic_layouts.insert(self.id, inner.basic_layout);
            // サイズ等の基本レイアウトが静的に指定されたことをマーク
            cx.mark_layout_dirty(self.id);
        }

        let has_visual =
            mask.has_visual_property() || inner.visual_property.border_lengths.is_some();
        if has_visual {
            cx.base_visual_properties
                .insert(self.id, inner.visual_property.clone());
        }

        // within 系スタイルが指定されている場合もSoAへ差し込む
        if mask.has_interaction_property() || mask.has(STYLE_INTERACTION_WITHIN) {
            cx.interaction_properties
                .insert(self.id, inner.interaction_styles.clone());
        }

        // 2. トランジションが関与しないプロパティは即時にマウント
        if mask.has_flex_layout() {
            if let Some(flex) = cx.flex_layouts.get_mut(self.id) {
                flex.override_with(&inner.flex_layout, mask);
            } else {
                cx.flex_layouts.insert(self.id, inner.flex_layout);
            }
            cx.mark_layout_dirty(self.id);
        }

        if mask.has_grid_layout()
            && let Some(ref grid) = inner.grid_layout
        {
            cx.grid_layouts.insert(self.id, grid.clone());
            cx.mark_layout_dirty(self.id);
        }

        if mask.has(STYLE_SCROLLBAR)
            && let Some(ref sb) = inner.scrollbar_style
        {
            // 動的 Element のマウントと生成を Context 側に委譲
            cx.ensure_scrollbar_elements(self.id, sb);
            cx.mark_layout_dirty(self.id);
        }

        // 3. 即座に「スタイル解決」を走り込ませ、トランジションやレイアウトマウントを自動処理！
        cx.resolve_element_style_state(self.id);
    }

    /// 子要素を追加します（Element単体、Signal、またはクロージャ）。
    /// 動的な値が渡された場合、自動的にスロット要素が作成され、その中身がリアクティブに切り替わります。
    #[inline]
    pub fn child(self, element: impl Into<Prop<Element>>) -> Self {
        match element.into() {
            Prop::None => {}
            Prop::Static(child) => {
                with_context(|cx| cx.add_child(self.id, child.id));
            }
            Prop::Dynamic(f) => {
                // 動的な子の場合はスロット(div)を作成して追加し、その中身を set_content で管理する
                let slot = div_n();
                with_context(|cx| cx.add_child(self.id, slot.id));
                slot.set_contents(Prop::Dynamic(f));
            }
        }
        self
    }

    /// 複数の子要素を一括して追加します。
    /// 静的な要素、シグナル、またはクロージャ（Prop<Element> に変換可能なオブジェクト）のコレクションを受け入れます。
    #[inline]
    pub fn children<I, E>(mut self, elements: I) -> Self
    where
        I: IntoIterator<Item = E>,
        E: Into<Prop<Element>>,
    {
        for el in elements {
            self = self.child(el);
        }
        self
    }

    /// 子要素として、インタラクション（クリック等）を自動的に透過するテキストラベルを挿入します。
    #[inline]
    pub fn label(
        self,
        content: impl Into<Prop<Cow<'static, str>>>,
        style: impl Into<Prop<ThisStyle>>,
    ) -> Self {
        let style = match style.into() {
            Prop::None => Prop::Static(ThisStyle::new().pointer_events_none()),
            Prop::Static(s) => Prop::Static(s.pointer_events_none()),
            Prop::Dynamic(f) => Prop::Dynamic(Box::new(move || f().pointer_events_none())),
        };
        let label_el = div_n().text(content).style(style);
        self.child(label_el)
    }

    /// このコンテナの内容を差し替えます。以前の内容はすべて破棄されます。
    pub fn set_contents(self, contents: impl Into<Prop<Element>>) -> Self {
        match contents.into() {
            Prop::None => {}
            Prop::Static(new_child) => {
                with_context(|cx| self.set_contents_internal(cx, new_child));
            }
            Prop::Dynamic(f) => {
                let id = self.id;
                let effect_id = create_effect(move |cx| {
                    let new_child = f();
                    let container = Element { id };
                    container.set_contents_internal(cx, new_child);
                });
                with_context(|cx| {
                    cx.register_element_effect(id, EffectCategory::Contents, effect_id)
                });
            }
        }
        self
    }

    /// 既存のクロージャをクリアせずに、子要素の差し替え（マウント）のみを実行する
    fn set_contents_internal(self, cx: &mut Context, new_child: Element) {
        let id = self.id;

        // 現在の子要素をすべて再帰的に despawn
        if let Some(children_list) = cx.children.get(id) {
            let old_children: Vec<EntityId> = children_list.iter().copied().collect();
            for child_id in old_children {
                cx.despawn_internal(child_id);
            }
        }

        // 新しい子要素を追加（親子トポロジーおよび Taffy ツリーの同期）
        cx.add_child(id, new_child.id);

        // レイアウトと描画の再計算を要求
        cx.mark_layout_dirty(id);
        cx.mark_render_dirty(id);
    }

    /// テキストを設定します。
    /// 引数には &str, String, ReadSignal<T>, またはクロージャを渡せます。
    #[inline]
    pub fn text(self, content: impl Into<Prop<Cow<'static, str>>>) -> Self {
        match content.into() {
            Prop::None => {}
            Prop::Static(val) => {
                with_context(|cx| {
                    cx.text_contents.insert(self.id, val);
                    cx.active_masks[self.id].set(COMP_TEXT_CONTENT);
                    cx.clear_layout_cache(self.id);
                    cx.mark_layout_dirty(self.id);
                    cx.mark_render_dirty(self.id);
                });
            }
            Prop::Dynamic(f) => {
                let id = self.id;
                let effect_id = create_effect(move |cx| {
                    let new_text = f();
                    cx.text_contents.insert(id, new_text);
                    cx.active_masks[id].set(COMP_TEXT_CONTENT);
                    cx.clear_layout_cache(self.id);
                    cx.mark_layout_dirty(id);
                    cx.mark_render_dirty(id);
                });
                with_context(|cx| cx.register_element_effect(id, EffectCategory::Text, effect_id));
            }
        }
        self
    }

    /// 画像を設定します。
    pub fn image(self, content: impl Into<Prop<ImageSource>>) -> Self {
        match content.into() {
            Prop::None => {}
            Prop::Static(src) => {
                with_context(|cx| {
                    cx.image_sources.insert(self.id, src);
                    cx.active_masks[self.id].set(COMP_IMAGE_CONTENT);
                    cx.mark_layout_dirty(self.id);
                    cx.mark_render_dirty(self.id);
                });
            }
            Prop::Dynamic(f) => {
                let id = self.id;
                let effect_id = create_effect(move |cx| {
                    let src = f();
                    cx.image_sources.insert(id, src);
                    cx.active_masks[id].set(COMP_IMAGE_CONTENT);
                    cx.mark_layout_dirty(id);
                    cx.mark_render_dirty(id);
                });
                with_context(|cx| cx.register_element_effect(id, EffectCategory::Image, effect_id));
            }
        }
        self
    }

    /// 動画を設定します。
    pub fn movie(self, content: impl Into<Prop<MovieProperty>>) -> Self {
        match content.into() {
            Prop::None => {}
            Prop::Static(p) => {
                with_context(|cx| {
                    cx.movie_properties.insert(self.id, p);
                    cx.active_masks[self.id].set(COMP_MOVIE_CONTENT);
                    cx.mark_layout_dirty(self.id);
                    cx.mark_render_dirty(self.id);
                });
            }
            Prop::Dynamic(f) => {
                let id = self.id;
                let effect_id = create_effect(move |cx| {
                    let p = f();
                    cx.movie_properties.insert(id, p);
                    cx.active_masks[id].set(COMP_MOVIE_CONTENT);
                    cx.mark_layout_dirty(id);
                    cx.mark_render_dirty(id);
                });
                with_context(|cx| cx.register_element_effect(id, EffectCategory::Movie, effect_id));
            }
        }
        self
    }

    /// WebView2 コンポーネントを配置します（静的設定、またはSignal / クロージャに対応）。
    pub fn webview2(self, contents: impl Into<Prop<WebView2Contents>>) -> Self {
        match contents.into() {
            Prop::None => {}
            Prop::Static(contents) => {
                with_context(|cx| {
                    cx.webview_contents.insert(self.id, contents);
                    cx.active_masks[self.id].set(COMP_WEBVIEW_CONTENT);
                    cx.mark_layout_dirty(self.id);
                    cx.mark_render_dirty(self.id);
                });
            }
            Prop::Dynamic(f) => {
                let id = self.id;
                let effect_id = create_effect(move |cx| {
                    let contents = f();
                    cx.webview_contents.insert(id, contents);
                    cx.active_masks[id].set(COMP_WEBVIEW_CONTENT);
                    cx.mark_layout_dirty(id);
                    cx.mark_render_dirty(id);
                });
                with_context(|cx| {
                    cx.register_element_effect(id, EffectCategory::WebView2, effect_id)
                });
            }
        }
        self
    }

    /// このコンテナを入力フィールド（テキストボックス）化し、IME制御や入力ロジックをバインドします。
    pub fn input(self, contents: impl Into<Prop<InputContents>>) -> Self {
        match contents.into() {
            Prop::None => {}
            Prop::Static(c) => {
                with_context(|cx| self.input_internal(cx, c));
            }
            Prop::Dynamic(f) => {
                let id = self.id;
                let effect_id = create_effect(move |cx| {
                    let c = f();
                    let el = Element { id };
                    el.input_internal(cx, c);
                });
                with_context(|cx| cx.register_element_effect(id, EffectCategory::Text, effect_id));
            }
        }
        self
    }

    /// 複数行入力（テキストエリア）をバインドします。
    pub fn input_area(self, contents: impl Into<Prop<InputContents>>) -> Self {
        match contents.into() {
            Prop::None => {}
            Prop::Static(mut c) => {
                c.is_multiline = true; // マルチライン化を強制
                with_context(|cx| self.input_internal(cx, c));
            }
            Prop::Dynamic(f) => {
                let id = self.id;
                let effect_id = create_effect(move |cx| {
                    let mut c = f();
                    c.is_multiline = true;
                    let el = Element { id };
                    el.input_internal(cx, c);
                });
                with_context(|cx| cx.register_element_effect(id, EffectCategory::Text, effect_id));
            }
        }
        self
    }

    /// 入力イベント（キー、IME、文字入力、フォーカス）を自動的にマッピングして代行するロジック
    fn input_internal(self, cx: &mut Context, mut c: InputContents) {
        let id = self.id;

        // 最初のロード時、シグナルから現在値を取得して内部カーソルを末尾に合わせる
        let current_text = c.text.0.get();
        let current_len = current_text.encode_utf16().count();
        c.selected_range = current_len..current_len;

        cx.input_contents.insert(id, c);
        cx.active_masks[id].set(COMP_INPUT_CONTENT);
        cx.active_masks[id].set(COMP_TEXT_CONTENT);

        self.get_or_create_listeners(|l| {
            l.on_mouse_input = Some(Box::new(move |cx, button, modifiers, state| {
                if button == MouseButton::Left
                    && state == ElementState::Pressed
                    && let Some(pointer_pos) = cx.current_pointer_position
                {
                    let rect = cx.rects[id];

                    // 要素の境界枠（border + padding）を取得してローカル座標を算出
                    let (basic, _, _) = cx.resolve_active_layouts(id);
                    let border_top = match basic.border.top {
                        Length::Px(v) => v,
                        _ => 0.0,
                    };
                    let border_left = match basic.border.left {
                        Length::Px(v) => v,
                        _ => 0.0,
                    };
                    let padding_top = match basic.padding.top {
                        Length::Px(v) => v,
                        _ => 0.0,
                    };
                    let padding_left = match basic.padding.left {
                        Length::Px(v) => v,
                        _ => 0.0,
                    };

                    // テキスト本来の描画領域に対する相対マウス座標
                    let local_x = pointer_pos.x - (rect.x + border_left + padding_left);
                    let local_y = pointer_pos.y - (rect.y + border_top + padding_top);

                    let mut update_rects_needed = false;

                    if let Some(contents) = cx.input_contents.get_mut(id) {
                        let text_val = contents.text.0.get();

                        // 逆引きレイアウト時もプレースホルダーは含まない
                        let editable_text_for_caret = if text_val.is_empty() {
                            if let Some(ref ime) = contents.ime_state
                                && !ime.composition_text.is_empty()
                            {
                                ime.composition_text.clone()
                            } else {
                                String::new()
                            }
                        } else if contents.is_password {
                            let mask = contents.mask_text.as_deref().unwrap_or("●");
                            mask.repeat(text_val.chars().count())
                        } else if let Some(ref ime) = contents.ime_state
                            && !ime.composition_text.is_empty()
                        {
                            crate::input_get_display_text(
                                &text_val,
                                contents.selected_range.start,
                                &ime.composition_text,
                            )
                        } else {
                            text_val.clone()
                        };

                        let default_visual = VisualProperty::default();
                        let visual = cx.visual_properties.get(id).unwrap_or(&default_visual);
                        let font_size = visual.font_size.unwrap_or(16.0);
                        let font_family = visual.font_family.as_deref();
                        let font_weight = visual.font_weight;
                        let font_style = visual.font_style;

                        let spans = cx.text_spans.get(id).map(|s| s.as_slice()).unwrap_or(&[]);

                        let layout = cx.text_engine.create_layout(
                            &editable_text_for_caret,
                            font_size,
                            font_family,
                            font_weight,
                            font_style,
                            None,
                            spans,
                        );

                        // 物理クリック座標から文字インデックスを逆引き
                        let (new_caret, is_trailing) =
                            cx.text_engine.hit_test_point(&layout, local_x, local_y);

                        let final_caret = if is_trailing {
                            new_caret + 1
                        } else {
                            new_caret
                        };

                        let editable_len = editable_text_for_caret.encode_utf16().count();
                        let final_caret_clamped = final_caret.min(editable_len);

                        // プレースホルダーが表示状態にあるか
                        let is_placeholder = text_val.is_empty()
                            && contents
                                .ime_state
                                .as_ref()
                                .map(|s| s.composition_text.is_empty())
                                .unwrap_or(true);

                        // プレースホルダーではない、またはプレースホルダー選択が明示許可されていること
                        let allow_selection = !is_placeholder || contents.placeholder_select;

                        if modifiers.shift && allow_selection {
                            let anchor = cx
                                .selection_start_index
                                .get(id)
                                .copied()
                                .unwrap_or(contents.selected_range.start);

                            if !cx.selection_start_index.contains_key(id) {
                                cx.selection_start_index
                                    .insert(id, contents.selected_range.start);
                            }

                            let range = if anchor <= final_caret_clamped {
                                contents.selection_reversed = false;
                                anchor..final_caret_clamped
                            } else {
                                contents.selection_reversed = true;
                                final_caret_clamped..anchor
                            };

                            contents.selected_range = range.clone();
                            cx.text_selections.insert(id, range);
                            update_rects_needed = true;
                        } else {
                            contents.selected_range = final_caret_clamped..final_caret_clamped;
                            cx.text_selections
                                .insert(id, final_caret_clamped..final_caret_clamped);
                            cx.selection_start_index.insert(id, final_caret_clamped);
                            cx.selected_rects.remove(id);
                            contents.selection_reversed = false;
                        }

                        contents.last_interacted_time = Some(std::time::Instant::now());

                        // キャレットの絶対座標と表示情報を一括更新
                        update_input_caret_position(cx, id);
                    }

                    if update_rects_needed {
                        cx.update_selection_rects(id);
                    }

                    cx.mark_render_dirty(id);
                }
            }));

            // フォーカス取得（点滅カーソルの有効化等）
            l.on_focus = Some(Box::new(move |cx| {
                if let Some(contents) = cx.input_contents.get_mut(id) {
                    contents.is_selecting = false;
                    // フォーカス獲得時も操作時刻を記録して即座にキャレットを表示
                    contents.last_interacted_time = Some(std::time::Instant::now());
                }
                cx.mark_render_dirty(id);
            }));

            // 確定した1文字の文字入力 (WM_CHAR)
            l.on_char_input = Some(Box::new(move |cx, mut ch| {
                // IME未変換の入力中 (composition_textがある間) は文字入力を無視
                let is_ime_active = cx
                    .input_contents
                    .get(id)
                    .and_then(|c| c.ime_state.as_ref())
                    .map(|s| !s.composition_text.is_empty())
                    .unwrap_or(false);

                if !is_ime_active {
                    let mut is_allowed = !ch.is_control();
                    let is_multiline = cx
                        .input_contents
                        .get(id)
                        .map(|c| c.is_multiline)
                        .unwrap_or(false);

                    // 複数行入力時に、Enterキー（'\r' / '\n'）が押された場合は改行コードとして許可
                    if is_multiline && (ch == '\r' || ch == '\n') {
                        ch = '\n';
                        is_allowed = true;
                    }

                    if is_allowed && let Some(contents) = cx.input_contents.get_mut(id) {
                        contents.last_interacted_time = Some(std::time::Instant::now());

                        let text_val = contents.text.0.get();

                        // 数値制限フィルター
                        if contents.numeric_only && !ch.is_numeric() && ch != '.' && ch != '-' {
                            return;
                        }

                        let range = contents.selected_range.clone();

                        // 変更発生前に現在の状態をセーブ
                        contents.record_undo(text_val.clone(), range.clone());

                        let u16_text: Vec<u16> = text_val.encode_utf16().collect();

                        // 選択範囲が削除された後の長さ
                        let u16_len_after_delete = u16_text.len()
                            - (range.end.min(u16_text.len()) - range.start.min(u16_text.len()));
                        let mut buf = [0u16; 2];
                        let ch_u16_slice = ch.encode_utf16(&mut buf);

                        // 文字数制限
                        if let Some(max) = contents.max_length
                            && u16_len_after_delete + ch_u16_slice.len() > max
                        {
                            return; // 制限を超えるため入力を中断
                        }

                        contents.last_interacted_time = Some(std::time::Instant::now());
                        contents.record_undo(text_val.clone(), range.clone());

                        let mut left = u16_text[..range.start.min(u16_text.len())].to_vec();
                        let right = u16_text[range.end.min(u16_text.len())..].to_vec();

                        left.extend_from_slice(ch_u16_slice);
                        left.extend_from_slice(&right);

                        let new_text = String::from_utf16_lossy(&left);
                        let new_caret = range.start + ch_u16_slice.len();

                        contents.selected_range = new_caret..new_caret;
                        cx.text_selections.insert(id, new_caret..new_caret); // 選択表示をリセット
                        cx.selected_rects.remove(id);
                        contents.text.1.set(new_text);
                        cx.mark_render_dirty(id);
                    }
                }
            }));

            // 物理キーボード操作 (Backspace, Delete, 矢印キー)
            l.on_keyboard_input = Some(Box::new(move |cx, key, modifiers, state| {
                if state == ElementState::Pressed
                    && let Some(contents) = cx.input_contents.get_mut(id)
                {
                    let default_visual = VisualProperty::default();
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
                            let range = contents.selected_range.clone();
                            contents.record_undo(text_val.clone(), range.clone());

                            if range.start < range.end {
                                // 選択範囲を一撃で消去
                                let u16_text: Vec<u16> = text_val.encode_utf16().collect();
                                let mut left = u16_text[..range.start.min(u16_text.len())].to_vec();
                                let right = u16_text[range.end.min(u16_text.len())..].to_vec();
                                left.extend_from_slice(&right);

                                let new_text = String::from_utf16_lossy(&left);
                                contents.selected_range = range.start..range.start;
                                cx.text_selections.insert(id, range.start..range.start);
                                contents.text.1.set(new_text);
                            } else {
                                // 通常の1文字バックスペース
                                let new_text = crate::input_backspace(&text_val, &mut caret);
                                contents.selected_range = caret..caret;
                                contents.text.1.set(new_text);
                            }
                            contents.last_interacted_time = Some(std::time::Instant::now());
                            changed = true;
                        }
                        VirtualKey::DELETE => {
                            let range = contents.selected_range.clone();
                            contents.record_undo(text_val.clone(), range.clone());
                            if range.start < range.end {
                                let u16_text: Vec<u16> = text_val.encode_utf16().collect();
                                let mut left = u16_text[..range.start.min(u16_text.len())].to_vec();
                                let right = u16_text[range.end.min(u16_text.len())..].to_vec();
                                left.extend_from_slice(&right);

                                let new_text = String::from_utf16_lossy(&left);
                                contents.selected_range = range.start..range.start;
                                cx.text_selections.insert(id, range.start..range.start);
                                contents.text.1.set(new_text);
                            } else {
                                // 通常の1文字デリート
                                let new_text = crate::input_delete(&text_val, caret);
                                contents.text.1.set(new_text);
                            }
                            contents.last_interacted_time = Some(std::time::Instant::now());
                            changed = true;
                        }
                        VirtualKey::LEFT => {
                            let range = contents.selected_range.clone();
                            // 選択範囲が存在し、かつ Shiftキーが押されていない通常移動時
                            if range.start < range.end && !modifiers.shift {
                                // 選択範囲をすべて解除し、キャレットを左端（start）に収束
                                let new_caret = range.start;
                                contents.selected_range = new_caret..new_caret;
                                cx.text_selections.insert(id, new_caret..new_caret);
                                cx.selected_rects.remove(id);
                                cx.selection_start_index.remove(id);
                                contents.selection_reversed = false;
                                contents.last_interacted_time = Some(std::time::Instant::now());
                                changed = true;
                            } else if caret > 0 {
                                let new_caret = caret - 1;

                                if modifiers.shift {
                                    // Shiftキー押下中：選択の拡張
                                    let anchor =
                                        cx.selection_start_index.get(id).copied().unwrap_or(caret);
                                    if !cx.selection_start_index.contains_key(id) {
                                        cx.selection_start_index.insert(id, caret);
                                    }
                                    let range = if anchor <= new_caret {
                                        contents.selection_reversed = false;
                                        anchor..new_caret
                                    } else {
                                        contents.selection_reversed = true;
                                        new_caret..anchor
                                    };
                                    contents.selected_range = range.clone();
                                    cx.text_selections.insert(id, range);
                                } else {
                                    // Shiftキー非押下：選択解除して単なる移動
                                    contents.selected_range = new_caret..new_caret;
                                    cx.text_selections.insert(id, new_caret..new_caret);
                                    cx.selected_rects.remove(id);
                                    cx.selection_start_index.remove(id);
                                }
                                contents.last_interacted_time = Some(std::time::Instant::now());
                                changed = true;
                            }
                        }
                        VirtualKey::RIGHT => {
                            let range = contents.selected_range.clone();
                            // 選択範囲が存在し、かつ Shiftキーが押されていない通常移動時（全選択中での右移動に完全対応）
                            if range.start < range.end && !modifiers.shift {
                                let new_caret = range.end;
                                contents.selected_range = new_caret..new_caret;
                                cx.text_selections.insert(id, new_caret..new_caret);
                                cx.selected_rects.remove(id);
                                cx.selection_start_index.remove(id);
                                contents.selection_reversed = false;
                                contents.last_interacted_time = Some(std::time::Instant::now());
                                changed = true;
                            } else if caret < u16_len {
                                let new_caret = caret + 1;

                                if modifiers.shift {
                                    let anchor =
                                        cx.selection_start_index.get(id).copied().unwrap_or(caret);
                                    if !cx.selection_start_index.contains_key(id) {
                                        cx.selection_start_index.insert(id, caret);
                                    }
                                    let range = if anchor <= new_caret {
                                        contents.selection_reversed = false;
                                        anchor..new_caret
                                    } else {
                                        contents.selection_reversed = true;
                                        new_caret..anchor
                                    };
                                    contents.selected_range = range.clone();
                                    cx.text_selections.insert(id, range);
                                } else {
                                    contents.selected_range = new_caret..new_caret;
                                    cx.text_selections.insert(id, new_caret..new_caret);
                                    cx.selected_rects.remove(id);
                                    cx.selection_start_index.remove(id);
                                }
                                contents.last_interacted_time = Some(std::time::Instant::now());
                                changed = true;
                            }
                        }
                        VirtualKey::UP => {
                            if contents.is_multiline {
                                let visual =
                                    cx.visual_properties.get(id).unwrap_or(&default_visual);
                                let font_size = visual.font_size.unwrap_or(16.0);
                                let font_family = visual.font_family.as_deref();
                                let font_weight = visual.font_weight;
                                let font_style = visual.font_style;

                                let spans =
                                    cx.text_spans.get(id).map(|s| s.as_slice()).unwrap_or(&[]);

                                let layout = cx.text_engine.create_layout(
                                    &text_val,
                                    font_size,
                                    font_family,
                                    font_weight,
                                    font_style,
                                    None,
                                    spans,
                                );

                                let (cx_offset, cy_offset, _) =
                                    cx.text_engine.get_caret_position(&layout, caret, u16_len);

                                let line_height = font_size * 1.3;
                                let target_y = (cy_offset - line_height * 1.1).max(0.0); // 1行分＋マージン

                                let (new_caret, is_trailing) =
                                    cx.text_engine.hit_test_point(&layout, cx_offset, target_y);
                                let final_caret = if is_trailing {
                                    new_caret + 1
                                } else {
                                    new_caret
                                };

                                if modifiers.shift {
                                    let anchor =
                                        cx.selection_start_index.get(id).copied().unwrap_or(caret);
                                    if !cx.selection_start_index.contains_key(id) {
                                        cx.selection_start_index.insert(id, caret);
                                    }
                                    let range = if anchor <= final_caret {
                                        contents.selection_reversed = false;
                                        anchor..final_caret
                                    } else {
                                        contents.selection_reversed = true;
                                        final_caret..anchor
                                    };
                                    contents.selected_range = range.clone();
                                    cx.text_selections.insert(id, range);
                                } else {
                                    contents.selected_range = final_caret..final_caret;
                                    cx.text_selections.insert(id, final_caret..final_caret);
                                    cx.selection_start_index.remove(id);
                                    contents.selection_reversed = false;
                                }

                                contents.last_interacted_time = Some(std::time::Instant::now());
                                changed = true;
                            }
                        }
                        VirtualKey::DOWN if contents.is_multiline => {
                            let visual = cx.visual_properties.get(id).unwrap_or(&default_visual);
                            let font_size = visual.font_size.unwrap_or(16.0);
                            let font_family = visual.font_family.as_deref();
                            let font_weight = visual.font_weight;
                            let font_style = visual.font_style;

                            let spans = cx.text_spans.get(id).map(|s| s.as_slice()).unwrap_or(&[]);

                            let layout = cx.text_engine.create_layout(
                                &text_val,
                                font_size,
                                font_family,
                                font_weight,
                                font_style,
                                None,
                                spans,
                            );

                            let (cx_offset, cy_offset, _) =
                                cx.text_engine.get_caret_position(&layout, caret, u16_len);

                            let line_height = font_size * 1.3;
                            let target_y = cy_offset + line_height * 1.5;
                            let (new_caret, is_trailing) =
                                cx.text_engine.hit_test_point(&layout, cx_offset, target_y);
                            let final_caret = if is_trailing {
                                new_caret + 1
                            } else {
                                new_caret
                            };

                            if modifiers.shift {
                                let anchor =
                                    cx.selection_start_index.get(id).copied().unwrap_or(caret);
                                if !cx.selection_start_index.contains_key(id) {
                                    cx.selection_start_index.insert(id, caret);
                                }
                                let range = if anchor <= final_caret {
                                    contents.selection_reversed = false;
                                    anchor..final_caret
                                } else {
                                    contents.selection_reversed = true;
                                    final_caret..anchor
                                };
                                contents.selected_range = range.clone();
                                cx.text_selections.insert(id, range);
                            } else {
                                contents.selected_range = final_caret..final_caret;
                                cx.text_selections.insert(id, final_caret..final_caret);
                                cx.selection_start_index.remove(id);
                                contents.selection_reversed = false;
                            }

                            contents.last_interacted_time = Some(std::time::Instant::now());
                            changed = true;
                        }
                        _ => {}
                    }

                    if changed {
                        cx.update_selection_rects(id);
                        update_input_caret_position(cx, id);
                        cx.mark_render_dirty(id);
                    }
                }
            }));

            // IME連動
            l.on_ime = Some(Box::new(move |cx, ime| {
                if let Some(contents) = cx.input_contents.get_mut(id) {
                    contents.last_interacted_time = Some(std::time::Instant::now());
                    contents.ime_state = Some(ime.clone());

                    // IME 確定文字の書き込み
                    if !ime.result_text.is_empty() {
                        let text_val = contents.text.0.get();
                        let mut caret = contents.selected_range.start;

                        // 確定した文字列を1文字ずつ安全に挿入
                        let mut temp_text = text_val;
                        for ch in ime.result_text.chars() {
                            temp_text = crate::input_insert_char(
                                &temp_text,
                                &mut caret,
                                ch,
                                contents.max_length,
                                contents.numeric_only,
                            );
                        }

                        contents.selected_range = caret..caret;
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
                    if !ime.composition_text.is_empty() {
                        let mut spans = Vec::new();
                        let caret = contents.selected_range.start;

                        if !ime.composition_attrs.is_empty() {
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
                        } else {
                            // 属性が取得できない場合のフォールバック（全体を未確定波線に設定）
                            let comp_len = ime.composition_text.encode_utf16().count();
                            spans.push(TextSpan {
                                range: caret..(caret + comp_len),
                                underline: Some(UnderlineStyle::Wave),
                                ..Default::default()
                            });
                        }

                        cx.text_spans.insert(id, spans);
                        cx.active_masks[id].set(STYLE_TEXT_SPANS);
                    } else {
                        cx.text_spans.remove(id);
                        cx.active_masks[id].unset(STYLE_TEXT_SPANS);
                    }

                    // IMEイベント終了（または変換中）に表示テキストとキャレット位置を再計算・同期させる
                    update_input_caret_position(cx, id);

                    cx.mark_render_dirty(id);
                }
            }));
        });

        // テキスト表示内容をシグナルやIME状態と完全同期させるエフェクト
        let text_sync_effect = create_effect(move |cx| {
            // シグナルを get() して依存関係を構築
            if let Some(contents) = cx.input_contents.get(id) {
                let _base_text_val = contents.text.0.get();
            }

            // 表示テキストとキャレット位置を完全同期
            update_input_caret_position(cx, id);

            // 物理サイズ変更や再描画を確実に要求
            cx.mark_layout_dirty(id);
            cx.mark_render_dirty(id);
        });

        // エフェクトを要素に紐付け登録
        cx.register_element_effect(id, EffectCategory::Text, text_sync_effect);
    }

    /// 内部ヘルパー：この要素に対応する `EventListeners` が SoA 上に存在しない場合は新規に作成し、
    /// 可変参照を取得して渡されたクロージャを実行します。
    #[inline(always)]
    fn get_or_create_listeners<R>(&self, f: impl FnOnce(&mut EventListeners) -> R) -> R {
        with_context(|cx| {
            // SparseSecondaryMap にキーが存在しない場合は Default (すべて None) で差し込む
            if !cx.event_listeners.contains_key(self.id) {
                cx.event_listeners
                    .insert(self.id, EventListeners::default());
            }
            let listeners = cx.event_listeners.get_mut(self.id).unwrap();
            f(listeners)
        })
    }

    /// 左クリックのリリース（押し下げ ➔ 同一要素上での離し）が成立した際に発火するイベントを登録します。
    #[inline]
    pub fn on_click<F>(self, mut f: F) -> Self
    where
        F: FnMut() + 'static,
    {
        self.on_click_with(move |_cx| f())
    }

    #[inline]
    pub fn on_click_with<F>(self, f: F) -> Self
    where
        F: FnMut(&mut Context) + 'static,
    {
        self.get_or_create_listeners(|l| {
            if let Some(mut existing) = l.on_click.take() {
                let mut f = f;
                l.on_click = Some(Box::new(move |cx| {
                    existing(cx);
                    f(cx);
                }));
            } else {
                l.on_click = Some(Box::new(f));
            }
        });
        self
    }

    /// 右クリックのリリース（押し下げ ➔ 同一要素上での離し）が成立した際に発火するイベントを登録します。
    #[inline]
    pub fn on_right_click<F>(self, mut f: F) -> Self
    where
        F: FnMut() + 'static,
    {
        self.on_right_click_with(move |_cx| f())
    }

    #[inline]
    pub fn on_right_click_with<F>(self, f: F) -> Self
    where
        F: FnMut(&mut Context) + 'static,
    {
        self.get_or_create_listeners(|l| {
            if let Some(mut existing) = l.on_right_click.take() {
                let mut f = f;
                l.on_right_click = Some(Box::new(move |cx| {
                    existing(cx);
                    f(cx);
                }));
            } else {
                l.on_right_click = Some(Box::new(f));
            }
        });
        self
    }

    /// マウスボタンの生入力（押し下げ、または離し）が発生した際に発火するイベントを登録します。
    #[inline]
    pub fn on_mouse_input<F>(self, mut f: F) -> Self
    where
        F: FnMut(MouseButton, Modifiers, ElementState) + 'static,
    {
        self.on_mouse_input_with(move |_cx, btn, mods, state| f(btn, mods, state))
    }

    #[inline]
    pub fn on_mouse_input_with<F>(self, f: F) -> Self
    where
        F: FnMut(&mut Context, MouseButton, Modifiers, ElementState) + 'static,
    {
        self.get_or_create_listeners(|l| {
            if let Some(mut existing) = l.on_mouse_input.take() {
                let mut f = f;
                l.on_mouse_input = Some(Box::new(move |cx, btn, mods, state| {
                    existing(cx, btn, mods, state);
                    f(cx, btn, mods, state);
                }));
            } else {
                l.on_mouse_input = Some(Box::new(f));
            }
        });
        self
    }

    /// マウスポインタが要素の可視境界内に入った（Enter）際に発火するイベントを登録します。
    #[inline]
    pub fn on_mouse_enter<F>(self, mut f: F) -> Self
    where
        F: FnMut() + 'static,
    {
        self.on_mouse_enter_with(move |_cx| f())
    }

    #[inline]
    pub fn on_mouse_enter_with<F>(self, f: F) -> Self
    where
        F: FnMut(&mut Context) + 'static,
    {
        self.get_or_create_listeners(|l| {
            if let Some(mut existing) = l.on_mouse_enter.take() {
                let mut f = f;
                l.on_mouse_enter = Some(Box::new(move |cx| {
                    existing(cx);
                    f(cx);
                }));
            } else {
                l.on_mouse_enter = Some(Box::new(f));
            }
        });
        self
    }

    /// マウスポインタが要素の可視境界から外に出た（Leave）際に発火するイベントを登録します。
    #[inline]
    pub fn on_mouse_leave<F>(self, mut f: F) -> Self
    where
        F: FnMut() + 'static,
    {
        self.on_mouse_leave_with(move |_cx| f())
    }

    #[inline]
    pub fn on_mouse_leave_with<F>(self, f: F) -> Self
    where
        F: FnMut(&mut Context) + 'static,
    {
        self.get_or_create_listeners(|l| {
            if let Some(mut existing) = l.on_mouse_leave.take() {
                let mut f = f;
                l.on_mouse_leave = Some(Box::new(move |cx| {
                    existing(cx);
                    f(cx);
                }));
            } else {
                l.on_mouse_leave = Some(Box::new(f));
            }
        });
        self
    }

    /// マウスポインタが要素内で移動した際に発火するイベントを登録します。
    /// コールバックには、要素の左上を原点 (0, 0) とする論理座標 `LayoutPoint` が伝播します。
    #[inline]
    pub fn on_cursor_moved<F>(self, mut f: F) -> Self
    where
        F: FnMut(LayoutPoint) + 'static,
    {
        self.on_cursor_moved_with(move |_cx, point| f(point))
    }

    #[inline]
    pub fn on_cursor_moved_with<F>(self, f: F) -> Self
    where
        F: FnMut(&mut Context, LayoutPoint) + 'static,
    {
        self.get_or_create_listeners(|l| {
            if let Some(mut existing) = l.on_cursor_moved.take() {
                let mut f = f;
                l.on_cursor_moved = Some(Box::new(move |cx, p| {
                    existing(cx, p);
                    f(cx, p);
                }));
            } else {
                l.on_cursor_moved = Some(Box::new(f));
            }
        });
        self
    }

    /// マウスホイールスクロールがこの要素上で検知された際のイベントをバインドします。
    /// コールバック引数には、論理ピクセル単位に換算された (scroll_x, scroll_y) が渡されます。
    #[inline]
    pub fn on_mouse_wheel<F>(self, mut f: F) -> Self
    where
        F: FnMut(f32, f32) + 'static,
    {
        self.on_mouse_wheel_with(move |_cx, sx, sy| f(sx, sy))
    }

    #[inline]
    pub fn on_mouse_wheel_with<F>(self, f: F) -> Self
    where
        F: FnMut(&mut Context, f32, f32) + 'static,
    {
        self.get_or_create_listeners(|l| {
            if let Some(mut existing) = l.on_mouse_wheel.take() {
                let mut f = f;
                l.on_mouse_wheel = Some(Box::new(move |cx, sx, sy| {
                    existing(cx, sx, sy);
                    f(cx, sx, sy);
                }));
            } else {
                l.on_mouse_wheel = Some(Box::new(f));
            }
        });
        self
    }

    /// スクロールコンテナの指定軸方向のオフセット（スクロール位置）を強制変更します。
    #[inline]
    pub fn scroll_to(self, x: f32, y: f32) -> Self {
        with_context(|cx| {
            cx.scroll_to(self.id, x, y);
        });
        self
    }

    /// スクロールコンテナを指定ピクセル分だけ相対移動させます。
    #[inline]
    pub fn scroll_by(self, dx: f32, dy: f32) -> Self {
        with_context(|cx| {
            cx.scroll_by(self.id, dx, dy);
        });
        self
    }

    /// このコンテナの現在のスクロール位置 (x, y) を安全に取得します。
    #[inline]
    pub fn scroll_offset(self) -> LayoutPoint {
        with_context(|cx| {
            cx.scroll_offsets
                .get(self.id)
                .copied()
                .unwrap_or(LayoutPoint::ZERO)
        })
    }

    /// 要素のドラッグ（左クリック押し下げ中のマウス移動）が発生した際に発火するイベントを登録します。
    /// コールバックには、前フレームからの移動差分である `LayoutPoint` が伝播します。
    #[inline]
    pub fn on_drag<F>(self, mut f: F) -> Self
    where
        F: FnMut(LayoutPoint) + 'static,
    {
        self.on_drag_with(move |_cx, delta| f(delta))
    }

    #[inline]
    pub fn on_drag_with<F>(self, f: F) -> Self
    where
        F: FnMut(&mut Context, LayoutPoint) + 'static,
    {
        self.get_or_create_listeners(|l| {
            if let Some(mut existing) = l.on_drag.take() {
                let mut f = f;
                l.on_drag = Some(Box::new(move |cx, p| {
                    existing(cx, p);
                    f(cx, p);
                }));
            } else {
                l.on_drag = Some(Box::new(f));
            }
        });
        self
    }

    /// マウスオーバーされた瞬間（`on_mouse_enter` と同時）に発火するイベントを登録します。
    #[inline]
    pub fn on_hover<F>(self, mut f: F) -> Self
    where
        F: FnMut() + 'static,
    {
        self.on_hover_with(move |_cx| f())
    }

    #[inline]
    pub fn on_hover_with<F>(self, f: F) -> Self
    where
        F: FnMut(&mut Context) + 'static,
    {
        self.get_or_create_listeners(|l| {
            if let Some(mut existing) = l.on_hover.take() {
                let mut f = f;
                l.on_hover = Some(Box::new(move |cx| {
                    existing(cx);
                    f(cx);
                }));
            } else {
                l.on_hover = Some(Box::new(f));
            }
        });
        self
    }

    /// 物理キーボードキーの操作が発生した際に発火するイベントを登録します（フォーカス獲得時のみ有効）。
    #[inline]
    pub fn on_keyboard_input<F>(self, mut f: F) -> Self
    where
        F: FnMut(VirtualKey, Modifiers, ElementState) + 'static,
    {
        self.on_keyboard_input_with(move |_cx, key, mods, state| f(key, mods, state))
    }

    #[inline]
    pub fn on_keyboard_input_with<F>(self, f: F) -> Self
    where
        F: FnMut(&mut Context, VirtualKey, Modifiers, ElementState) + 'static,
    {
        self.get_or_create_listeners(|l| {
            if let Some(mut existing) = l.on_keyboard_input.take() {
                let mut f = f;
                l.on_keyboard_input = Some(Box::new(move |cx, key, mods, state| {
                    existing(cx, key, mods, state);
                    f(cx, key, mods, state);
                }));
            } else {
                l.on_keyboard_input = Some(Box::new(f));
            }
        });
        self
    }

    /// ローカライズやリピート処理が適用された確定1文字が入力された際に発火するイベントを登録します。
    #[inline]
    pub fn on_char_input<F>(self, mut f: F) -> Self
    where
        F: FnMut(char) + 'static,
    {
        self.on_char_input_with(move |_cx, c| f(c))
    }

    #[inline]
    pub fn on_char_input_with<F>(self, f: F) -> Self
    where
        F: FnMut(&mut Context, char) + 'static,
    {
        self.get_or_create_listeners(|l| {
            if let Some(mut existing) = l.on_char_input.take() {
                let mut f = f;
                l.on_char_input = Some(Box::new(move |cx, c| {
                    existing(cx, c);
                    f(cx, c);
                }));
            } else {
                l.on_char_input = Some(Box::new(f));
            }
        });
        self
    }

    /// IME（入力文字プロセッサ）による変換テキスト、キャレット、確定文字列の更新を捕捉するイベントを登録します。
    #[inline]
    pub fn on_ime<F>(self, mut f: F) -> Self
    where
        F: FnMut(ImeState) + 'static,
    {
        self.on_ime_with(move |_cx, state| f(state))
    }

    #[inline]
    pub fn on_ime_with<F>(self, f: F) -> Self
    where
        F: FnMut(&mut Context, ImeState) + 'static,
    {
        self.get_or_create_listeners(|l| {
            if let Some(mut existing) = l.on_ime.take() {
                let mut f = f;
                l.on_ime = Some(Box::new(move |cx, state| {
                    existing(cx, state.clone());
                    f(cx, state);
                }));
            } else {
                l.on_ime = Some(Box::new(f));
            }
        });
        self
    }

    /// OS上からファイルやフォルダーがこの要素へドラッグ＆ドロップされた際のイベントを登録します。
    #[inline]
    pub fn on_file_dropped<F>(self, mut f: F) -> Self
    where
        F: FnMut(Vec<PathBuf>) + 'static,
    {
        self.on_file_dropped_with(move |_cx, paths| f(paths))
    }

    #[inline]
    pub fn on_file_dropped_with<F>(self, f: F) -> Self
    where
        F: FnMut(&mut Context, Vec<PathBuf>) + 'static,
    {
        self.get_or_create_listeners(|l| {
            if let Some(mut existing) = l.on_file_dropped.take() {
                let mut f = f;
                l.on_file_dropped = Some(Box::new(move |cx, paths| {
                    existing(cx, paths.clone());
                    f(cx, paths);
                }));
            } else {
                l.on_file_dropped = Some(Box::new(f));
            }
        });
        self
    }

    /// ファイルが要素上にドラッグ侵入した際のイベント（シンプル版）
    #[inline]
    pub fn on_file_drag_enter<F>(self, mut f: F) -> Self
    where
        F: FnMut() + 'static,
    {
        self.on_file_drag_enter_with(move |_cx| f())
    }

    /// ファイルが要素上にドラッグ侵入した際のイベント（エスケープハッチ版）
    #[inline]
    pub fn on_file_drag_enter_with<F>(self, f: F) -> Self
    where
        F: FnMut(&mut Context) + 'static,
    {
        self.get_or_create_listeners(|l| {
            if let Some(mut existing) = l.on_file_drag_enter.take() {
                let mut f = f;
                l.on_file_drag_enter = Some(Box::new(move |cx| {
                    existing(cx);
                    f(cx);
                }));
            } else {
                l.on_file_drag_enter = Some(Box::new(f));
            }
        });
        self
    }

    /// ファイルが要素上からドラッグ離脱した際のイベント（シンプル版）
    #[inline]
    pub fn on_file_drag_leave<F>(self, mut f: F) -> Self
    where
        F: FnMut() + 'static,
    {
        self.on_file_drag_leave_with(move |_cx| f())
    }

    /// ファイルが要素上からドラッグ離脱した際のイベント（エスケープハッチ版）
    #[inline]
    pub fn on_file_drag_leave_with<F>(self, f: F) -> Self
    where
        F: FnMut(&mut Context) + 'static,
    {
        self.get_or_create_listeners(|l| {
            if let Some(mut existing) = l.on_file_drag_leave.take() {
                let mut f = f;
                l.on_file_drag_leave = Some(Box::new(move |cx| {
                    existing(cx);
                    f(cx);
                }));
            } else {
                l.on_file_drag_leave = Some(Box::new(f));
            }
        });
        self
    }

    /// 画像ファイルのロードが完了し、
    /// メタデータ（解像度、フォーマット、アニメーションの有無等）が取得可能になった時のイベントを登録します。
    #[inline]
    pub fn on_image_loaded<F>(self, mut f: F) -> Self
    where
        F: FnMut(ImageMetadata) + 'static,
    {
        self.on_image_loaded_with(move |_cx, img| f(img))
    }

    #[inline]
    pub fn on_image_loaded_with<F>(self, f: F) -> Self
    where
        F: FnMut(&mut Context, ImageMetadata) + 'static,
    {
        self.get_or_create_listeners(|l| {
            if let Some(mut existing) = l.on_image_loaded.take() {
                let mut f = f;
                l.on_image_loaded = Some(Box::new(move |cx, img| {
                    existing(cx, img.clone());
                    f(cx, img);
                }));
            } else {
                l.on_image_loaded = Some(Box::new(f));
            }
        });
        self
    }

    /// 動画ファイルがロードされ、メタデータ（解像度、FPS、ビットレート等）が取得可能になった時のイベントを登録します。
    #[inline]
    pub fn on_media_loaded<F>(self, mut f: F) -> Self
    where
        F: FnMut(MovieMetadata) + 'static,
    {
        self.on_media_loaded_with(move |_cx, movie| f(movie))
    }

    #[inline]
    pub fn on_media_loaded_with<F>(self, f: F) -> Self
    where
        F: FnMut(&mut Context, MovieMetadata) + 'static,
    {
        self.get_or_create_listeners(|l| {
            if let Some(mut existing) = l.on_media_loaded.take() {
                let mut f = f;
                l.on_media_loaded = Some(Box::new(move |cx, movie| {
                    existing(cx, movie.clone());
                    f(cx, movie);
                }));
            } else {
                l.on_media_loaded = Some(Box::new(f));
            }
        });
        self
    }

    /// 要素が新しく入力フォーカスを獲得した際に発火するイベントを登録します。
    #[inline]
    pub fn on_focus<F>(self, mut f: F) -> Self
    where
        F: FnMut() + 'static,
    {
        self.on_focus_with(move |_cx| f())
    }

    #[inline]
    pub fn on_focus_with<F>(self, f: F) -> Self
    where
        F: FnMut(&mut Context) + 'static,
    {
        self.get_or_create_listeners(|l| {
            if let Some(mut existing) = l.on_focus.take() {
                let mut f = f;
                l.on_focus = Some(Box::new(move |cx| {
                    existing(cx);
                    f(cx);
                }));
            } else {
                l.on_focus = Some(Box::new(f));
            }
        });
        self
    }

    /// 他の要素がクリックされるなどして、フォーカスを喪失した際に発火するイベントを登録します。
    #[inline]
    pub fn on_blur<F>(self, mut f: F) -> Self
    where
        F: FnMut() + 'static,
    {
        self.on_blur_with(move |_cx| f())
    }

    #[inline]
    pub fn on_blur_with<F>(self, f: F) -> Self
    where
        F: FnMut(&mut Context) + 'static,
    {
        self.get_or_create_listeners(|l| {
            if let Some(mut existing) = l.on_blur.take() {
                let mut f = f;
                l.on_blur = Some(Box::new(move |cx| {
                    existing(cx);
                    f(cx);
                }));
            } else {
                l.on_blur = Some(Box::new(f));
            }
        });
        self
    }

    /// 要素が無効化（Disabled）された瞬間に発火するイベントを登録します。
    #[inline]
    pub fn on_disable<F>(self, mut f: F) -> Self
    where
        F: FnMut() + 'static,
    {
        self.on_disable_with(move |_cx| f())
    }

    #[inline]
    pub fn on_disable_with<F>(self, f: F) -> Self
    where
        F: FnMut(&mut Context) + 'static,
    {
        self.get_or_create_listeners(|l| {
            if let Some(mut existing) = l.on_disable.take() {
                let mut f = f;
                l.on_disable = Some(Box::new(move |cx| {
                    existing(cx);
                    f(cx);
                }));
            } else {
                l.on_disable = Some(Box::new(f));
            }
        });
        self
    }

    /// 要素がアクティブ（Actived）状態になった瞬間に発火するイベントを登録します。
    #[inline]
    pub fn on_active<F>(self, mut f: F) -> Self
    where
        F: FnMut() + 'static,
    {
        self.on_active_with(move |_cx| f())
    }

    #[inline]
    pub fn on_active_with<F>(self, f: F) -> Self
    where
        F: FnMut(&mut Context) + 'static,
    {
        self.get_or_create_listeners(|l| {
            if let Some(mut existing) = l.on_active.take() {
                let mut f = f;
                l.on_active = Some(Box::new(move |cx| {
                    existing(cx);
                    f(cx);
                }));
            } else {
                l.on_active = Some(Box::new(f));
            }
        });
        self
    }

    /// チェックボックスやラジオボタンなどで、要素が選択（Selected）された瞬間に発火するイベントを登録します。
    #[inline]
    pub fn on_select<F>(self, mut f: F) -> Self
    where
        F: FnMut() + 'static,
    {
        self.on_select_with(move |_cx| f())
    }

    #[inline]
    pub fn on_select_with<F>(self, f: F) -> Self
    where
        F: FnMut(&mut Context) + 'static,
    {
        self.get_or_create_listeners(|l| {
            if let Some(mut existing) = l.on_select.take() {
                let mut f = f;
                l.on_select = Some(Box::new(move |cx| {
                    existing(cx);
                    f(cx);
                }));
            } else {
                l.on_select = Some(Box::new(f));
            }
        });
        self
    }

    /// UI Automation のプロパティを生の ID (i32) を指定して直接登録します
    #[inline]
    pub fn uia_property(self, property_id: i32, value: impl Into<UiaValue>) -> Self {
        with_context(|cx| self.uia_property_internal(cx, property_id, value.into()));
        self
    }

    /// スクリーンリーダーが読み上げる要素の「名前」を設定します（UIA_NamePropertyId 互換）。
    pub fn uia_name(self, name: impl Into<Prop<Cow<'static, str>>>) -> Self {
        match name.into() {
            Prop::None => self,
            Prop::Static(s) => self.uia_property(30005, UiaValue::String(s.into())),
            Prop::Dynamic(f) => {
                let id = self.id;
                let effect_id = create_effect(move |cx| {
                    let s = f();
                    let el = Element { id };
                    el.uia_property_internal(cx, 30005, UiaValue::String(s.into()));
                });
                with_context(|cx| {
                    cx.register_element_effect(id, EffectCategory::UiaName, effect_id)
                });
                self
            }
        }
    }

    fn uia_property_internal(self, cx: &mut Context, property_id: i32, value: UiaValue) {
        if !cx.uia_properties.contains_key(self.id) {
            cx.uia_properties.insert(self.id, Vec::new());
        }
        let list = cx.uia_properties.get_mut(self.id).unwrap();
        if let Some(pos) = list.iter().position(|(k, _)| *k == property_id) {
            list[pos].1 = value;
        } else {
            list.push((property_id, value));
        }
        cx.active_masks[self.id].set(COMP_UIA_CONTENT);
    }

    /// 自動テストフレームワークやデバッグで要素を特定するための「Automation ID」を設定します（UIA_AutomationIdPropertyId 互換）。
    #[inline]
    pub fn uia_automation_id(self, id: impl Into<Prop<Cow<'static, str>>>) -> Self {
        match id.into() {
            Prop::None => self,
            Prop::Static(s) => self.uia_property(30011, UiaValue::String(s.into())),
            Prop::Dynamic(f) => {
                let id = self.id;
                let effect_id = create_effect(move |cx| {
                    let s = f();
                    let el = Element { id };
                    el.uia_property_internal(cx, 30011, UiaValue::String(s.into()));
                });
                with_context(|cx| {
                    cx.register_element_effect(id, EffectCategory::UiaAutomationId, effect_id)
                });
                self
            }
        }
    }

    /// この要素がどのようなコントロール（ボタン、チェックボックス、リスト等）として振る舞うかを定義します（UIA_ControlTypePropertyId 互換）。
    #[inline]
    pub fn uia_control_type(self, control_type_id: i32) -> Self {
        self.uia_property(30003, control_type_id)
    }
}

/// 内部ヘルパー: 現在のテキスト・IME状態・フォントサイズから、
/// キャレットの物理座標や最終表示テキスト、レイアウト矩形を正確に再計算して SoA を更新します。
pub(crate) fn update_input_caret_position(cx: &mut Context, id: EntityId) {
    cx.clear_layout_cache(id); // IMEやタイピング中の古いキャッシュを破棄
    if let Some(contents) = cx.input_contents.get_mut(id) {
        let text_val = contents.text.0.get();
        contents.total_len = text_val.chars().count();

        // 描画表示用テキスト（IME未確定文字列の有無を最優先で判定）
        let display_text = if let Some(ref ime) = contents.ime_state
            && !ime.composition_text.is_empty()
        {
            crate::input_get_display_text(
                &text_val,
                contents.selected_range.start,
                &ime.composition_text,
            )
        } else if text_val.is_empty() {
            contents
                .placeholder
                .as_ref()
                .map(|s| s.to_string())
                .unwrap_or_default()
        } else if contents.is_password {
            let mask = contents.mask_text.as_deref().unwrap_or("●");
            mask.repeat(text_val.chars().count())
        } else {
            text_val.clone()
        };

        let caret_text = if let Some(ref ime) = contents.ime_state
            && !ime.composition_text.is_empty()
        {
            crate::input_get_display_text(
                &text_val,
                contents.selected_range.start,
                &ime.composition_text,
            )
        } else if text_val.is_empty() {
            String::new()
        } else if contents.is_password {
            let mask = contents.mask_text.as_deref().unwrap_or("●");
            mask.repeat(text_val.chars().count())
        } else {
            text_val.clone()
        };

        let font_size = cx
            .visual_properties
            .get(id)
            .and_then(|v| v.font_size)
            .unwrap_or(16.0);
        let font_family = cx
            .visual_properties
            .get(id)
            .and_then(|v| v.font_family.as_deref());
        let font_weight = cx.visual_properties.get(id).and_then(|v| v.font_weight);
        let font_style = cx.visual_properties.get(id).and_then(|v| v.font_style);

        let spans = cx.text_spans.get(id).map(|s| s.as_slice()).unwrap_or(&[]);

        // 描画テキスト全体のレイアウトサイズを Taffy 測定用に設定
        let display_layout = cx.text_engine.create_layout(
            &display_text,
            font_size,
            font_family,
            font_weight,
            font_style,
            None,
            spans,
        );
        let text_size = cx.text_engine.get_layout_size(&display_layout);
        contents.last_layout = Some(LayoutRect::new(0.0, 0.0, text_size.width, text_size.height));

        // キャレット位置測定用のレイアウトをプレースホルダー抜きで作成
        let caret_layout = cx.text_engine.create_layout(
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
            cx.text_engine
                .get_caret_position(&caret_layout, caret_index, u16_len_caret);

        contents.caret_offset_x = cx_offset;
        contents.caret_offset_y = cy_offset;
        contents.caret_line_height = ch_height;

        let (curr_line, tot_lines) = crate::calculate_line_indices(&display_text, caret_index);
        contents.current_line_index = curr_line;
        contents.total_lines = tot_lines;

        // 最終表示用テキストを Context 側に反映
        cx.text_contents.insert(id, display_text.into());

        if let Some(visual) = cx.visual_properties.get_mut(id) {
            let is_ime_active = contents
                .ime_state
                .as_ref()
                .map(|ime| !ime.composition_text.is_empty())
                .unwrap_or(false);

            if text_val.is_empty() && !is_ime_active {
                // 確定文字列が空で、かつ未確定文字列も存在しない状態のみグレー表示
                visual.text_color = contents.placeholder_color;
            } else {
                let base_color = cx
                    .base_visual_properties
                    .get(id)
                    .and_then(|v| v.text_color)
                    .unwrap_or(Color::WHITE);
                visual.text_color = Some(base_color);
            }
        }

        // IMM32 による IME 変換候補ウィンドウの位置同期を自動実行
        unsafe {
            let hwnd = GetFocus();
            let himc = ImmGetContext(hwnd);
            if !himc.is_invalid() {
                let rect = cx.rects[id];
                let (basic, _, _) = cx.resolve_active_layouts(id);
                let border_top = match basic.border.top {
                    Length::Px(v) => v,
                    _ => 0.0,
                };
                let border_left = match basic.border.left {
                    Length::Px(v) => v,
                    _ => 0.0,
                };
                let padding_top = match basic.padding.top {
                    Length::Px(v) => v,
                    _ => 0.0,
                };
                let padding_left = match basic.padding.left {
                    Length::Px(v) => v,
                    _ => 0.0,
                };

                let scale = cx.scale_factor;
                let caret_phys_x =
                    ((rect.x + border_left + padding_left + cx_offset) * scale).round() as i32;
                let caret_phys_y =
                    ((rect.y + border_top + padding_top + cy_offset) * scale).round() as i32;
                let caret_phys_h = (ch_height * scale).round() as i32;

                // コンポジションウィンドウ位置の指定 (CFS_POINT)
                let comp_form = COMPOSITIONFORM {
                    dwStyle: CFS_POINT,
                    ptCurrentPos: windows::Win32::Foundation::POINT {
                        x: caret_phys_x,
                        y: caret_phys_y,
                    },
                    rcArea: windows::Win32::Foundation::RECT::default(),
                };
                let _ = ImmSetCompositionWindow(himc, &comp_form);

                // 候補ウィンドウ位置の指定 (CFS_EXCLUDE)
                let candidate_form = CANDIDATEFORM {
                    dwIndex: 0,
                    dwStyle: CFS_EXCLUDE,
                    ptCurrentPos: windows::Win32::Foundation::POINT {
                        x: caret_phys_x,
                        y: caret_phys_y,
                    },
                    rcArea: windows::Win32::Foundation::RECT {
                        left: caret_phys_x,
                        top: caret_phys_y,
                        right: caret_phys_x + 1,
                        bottom: caret_phys_y + caret_phys_h,
                    },
                };
                let _ = ImmSetCandidateWindow(himc, &candidate_form);
                let _ = ImmReleaseContext(hwnd, himc);
            }
        }
    }
}

// コンテキストを復元するための一時的なガード構造体
pub(crate) struct ContextGuard {
    old: Option<*mut Context>,
}

/// 現在のスレッドローカル（ACTIVE_CONTEXT）に Context を一時的にバインドします。
/// 戻り値のガードオブジェクト（ContextGuard）がスコープを抜ける際、自動的に元のコンテキストに復元されます。
#[inline(always)]
pub(crate) fn bind_context(cx: &Context) -> ContextGuard {
    let old = ACTIVE_CONTEXT.get();
    // 借用チェッカーと衝突しないよう、生ポインタキャストを行ってスレッドローカルに格納
    ACTIVE_CONTEXT.set(Some(cx as *const Context as *mut Context));
    ContextGuard { old }
}

impl Drop for ContextGuard {
    #[inline]
    fn drop(&mut self) {
        ACTIVE_CONTEXT.set(self.old);
    }
}

/// 静的な値、または動的に変化する値（Signalやクロージャ）を抽象化する型
pub enum Prop<T> {
    None,
    Static(T),
    Dynamic(Box<dyn Fn() -> T + 'static>),
}

impl<T> From<Option<T>> for Prop<T> {
    fn from(opt: Option<T>) -> Self {
        match opt {
            Some(v) => Self::Static(v),
            None => Self::None,
        }
    }
}

// 文字列リテラル用
impl From<&'static str> for Prop<Cow<'static, str>> {
    fn from(s: &'static str) -> Self {
        Self::Static(s.into())
    }
}

// String用
impl From<String> for Prop<Cow<'static, str>> {
    fn from(s: String) -> Self {
        Self::Static(s.into())
    }
}

// Cowそのもの
impl From<Cow<'static, str>> for Prop<Cow<'static, str>> {
    fn from(s: Cow<'static, str>) -> Self {
        Self::Static(s)
    }
}

// Displayを実装している型のSignal (u32, i32など)
impl<T: std::fmt::Display + Clone + Send + 'static> From<ReadSignal<T>>
    for Prop<Cow<'static, str>>
{
    fn from(sig: ReadSignal<T>) -> Self {
        Self::Dynamic(Box::new(move || sig.get().to_string().into()))
    }
}

// クロージャ用 (戻り値が Cow に変換可能なもの)
impl<F, S> From<F> for Prop<Cow<'static, str>>
where
    F: Fn() -> S + 'static,
    S: Into<Cow<'static, str>>,
{
    fn from(f: F) -> Self {
        Self::Dynamic(Box::new(move || f().into()))
    }
}

impl From<ThisStyle> for Prop<ThisStyle> {
    fn from(s: ThisStyle) -> Self {
        Self::Static(s)
    }
}

impl From<ReadSignal<ThisStyle>> for Prop<ThisStyle> {
    fn from(sig: ReadSignal<ThisStyle>) -> Self {
        Self::Dynamic(Box::new(move || sig.get()))
    }
}

impl<F> From<F> for Prop<ThisStyle>
where
    F: Fn() -> ThisStyle + 'static,
{
    fn from(f: F) -> Self {
        Self::Dynamic(Box::new(f))
    }
}

impl From<Element> for Prop<Element> {
    fn from(el: Element) -> Self {
        Self::Static(el)
    }
}

impl From<ReadSignal<Element>> for Prop<Element> {
    fn from(sig: ReadSignal<Element>) -> Self {
        Self::Dynamic(Box::new(move || sig.get()))
    }
}

impl<F> From<F> for Prop<Element>
where
    F: Fn() -> Element + 'static,
{
    fn from(f: F) -> Self {
        Self::Dynamic(Box::new(f))
    }
}

impl From<ReadSignal<ImageSource>> for Prop<ImageSource> {
    #[inline]
    fn from(sig: ReadSignal<ImageSource>) -> Self {
        Self::Dynamic(Box::new(move || sig.get()))
    }
}
impl From<ReadSignal<MovieProperty>> for Prop<MovieProperty> {
    #[inline]
    fn from(sig: ReadSignal<MovieProperty>) -> Self {
        Self::Dynamic(Box::new(move || sig.get()))
    }
}

impl<F, S> From<F> for Prop<ImageSource>
where
    F: Fn() -> S + 'static,
    S: Into<ImageSource>,
{
    #[inline]
    fn from(f: F) -> Self {
        Self::Dynamic(Box::new(move || f().into()))
    }
}
impl<F, S> From<F> for Prop<MovieProperty>
where
    F: Fn() -> S + 'static,
    S: Into<MovieProperty>,
{
    #[inline]
    fn from(f: F) -> Self {
        Self::Dynamic(Box::new(move || f().into()))
    }
}

impl From<Transform> for Prop<Transform> {
    #[inline]
    fn from(t: Transform) -> Self {
        Self::Static(t)
    }
}
impl From<[[f32; 4]; 4]> for Prop<Transform> {
    #[inline]
    fn from(m: [[f32; 4]; 4]) -> Self {
        Self::Static(Transform { matrix: m })
    }
}
impl From<ReadSignal<Transform>> for Prop<Transform> {
    #[inline]
    fn from(sig: ReadSignal<Transform>) -> Self {
        Self::Dynamic(Box::new(move || sig.get()))
    }
}
impl<F> From<F> for Prop<Transform>
where
    F: Fn() -> Transform + 'static,
{
    #[inline]
    fn from(f: F) -> Self {
        Self::Dynamic(Box::new(f))
    }
}

impl From<LinearGradient> for Prop<LinearGradient> {
    #[inline]
    fn from(g: LinearGradient) -> Self {
        Self::Static(g)
    }
}
impl From<ReadSignal<LinearGradient>> for Prop<LinearGradient> {
    #[inline]
    fn from(sig: ReadSignal<LinearGradient>) -> Self {
        Self::Dynamic(Box::new(move || sig.get()))
    }
}
impl<F> From<F> for Prop<LinearGradient>
where
    F: Fn() -> LinearGradient + 'static,
{
    #[inline]
    fn from(f: F) -> Self {
        Self::Dynamic(Box::new(f))
    }
}

impl From<ReadSignal<u32>> for Prop<u32> {
    #[inline]
    fn from(sig: ReadSignal<u32>) -> Self {
        Self::Dynamic(Box::new(move || sig.get()))
    }
}
impl<F> From<F> for Prop<u32>
where
    F: Fn() -> u32 + 'static,
{
    #[inline]
    fn from(f: F) -> Self {
        Self::Dynamic(Box::new(f))
    }
}

impl From<WebView2Contents> for Prop<WebView2Contents> {
    fn from(c: WebView2Contents) -> Self {
        Self::Static(c)
    }
}
impl From<ReadSignal<WebView2Contents>> for Prop<WebView2Contents> {
    fn from(sig: ReadSignal<WebView2Contents>) -> Self {
        Self::Dynamic(Box::new(move || sig.get()))
    }
}

impl From<InputContents> for Prop<InputContents> {
    #[inline]
    fn from(c: InputContents) -> Self {
        Self::Static(c)
    }
}

impl From<ReadSignal<InputContents>> for Prop<InputContents> {
    #[inline]
    fn from(sig: ReadSignal<InputContents>) -> Self {
        Self::Dynamic(Box::new(move || sig.get()))
    }
}

impl<F> From<F> for Prop<InputContents>
where
    F: Fn() -> InputContents + 'static,
{
    #[inline]
    fn from(f: F) -> Self {
        Self::Dynamic(Box::new(f))
    }
}

impl From<&ThisStyle> for Prop<ThisStyle> {
    #[inline]
    fn from(s: &ThisStyle) -> Self {
        Self::Static(s.clone())
    }
}

#[cfg(test)]
mod tests;
