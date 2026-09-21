use crate::{
    ByteIndex, CharIndex, Color, IntoSize, LayoutPoint, LayoutRect, MichiuString, ReadSignal,
    UsizeRangeExt, WriteSignal,
};
use std::ops::Range;
use std::{borrow::Cow, time::Duration};

#[derive(Debug, Clone, PartialEq, Default)]
pub struct ImeState {
    pub is_open: bool,
    pub conversion_mode: u32,
    pub sentence_mode: u32,
    pub keyboard_layout_id: u32,
    pub composition_text: MichiuString,
    pub result_text: MichiuString,
    pub caret_position: Option<LayoutPoint>,
    pub composition_cursor: CharIndex,
    pub composition_attrs: Vec<u8>,
}

impl ImeState {
    #[inline]
    #[must_use]
    pub fn new() -> Self {
        Self {
            ..Default::default()
        }
    }

    #[inline]
    #[must_use]
    pub fn is_open(mut self, enable: bool) -> Self {
        self.is_open = enable;
        self
    }

    #[inline]
    #[must_use]
    pub fn conversion_mode(mut self, mode: u32) -> Self {
        self.conversion_mode = mode;
        self
    }

    #[inline]
    #[must_use]
    pub fn sentence_mode(mut self, mode: u32) -> Self {
        self.sentence_mode = mode;
        self
    }

    #[inline]
    #[must_use]
    pub fn keyboard_layout_id(mut self, id: u32) -> Self {
        self.keyboard_layout_id = id;
        self
    }

    #[inline]
    #[must_use]
    pub fn composition_text(mut self, text: impl Into<Cow<'static, str>>) -> Self {
        self.composition_text = MichiuString(text.into());
        self
    }

    #[inline]
    #[must_use]
    pub fn result_text(mut self, text: impl Into<Cow<'static, str>>) -> Self {
        self.result_text = MichiuString(text.into());
        self
    }

    #[inline]
    #[must_use]
    pub fn caret_position(mut self, pos: Option<LayoutPoint>) -> Self {
        self.caret_position = pos;
        self
    }

    #[inline]
    #[must_use]
    pub fn composition_cursor(mut self, index: usize) -> Self {
        self.composition_cursor = index.into();
        self
    }

    #[inline]
    #[must_use]
    pub fn composition_attrs(mut self, attrs: Vec<u8>) -> Self {
        self.composition_attrs = attrs;
        self
    }
}

/// Underline Drawing Style
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UnderlineStyle {
    Solid,
    Thick,
    Wave,
    Double,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StrikethroughStyle {
    Solid,
    Thick,
}

/// Text span for applying partial styling to a specific range of a string
#[derive(Debug, Clone, PartialEq)]
pub struct TextSpan {
    /// The range of character indices to which this decoration is applied
    pub(crate) range: Range<ByteIndex>,

    /// Partial Text Color Override
    pub(crate) color: Option<Color>,
    /// Overriding Partial Text Background Colors
    pub(crate) bg_color: Option<Color>,

    /// Partial Font Size Override
    pub(crate) font_size: Option<f32>,
    /// Partial Font Family Override
    pub(crate) font_family: Option<Cow<'static, str>>,
    /// Partial Font Weight Override
    pub(crate) font_weight: Option<u32>,
    /// Partial Font Style Override
    pub(crate) font_style: Option<u32>,

    pub(crate) underline: Option<UnderlineStyle>,
    pub(crate) underline_color: Option<Color>,
    pub(crate) strikethrough: Option<StrikethroughStyle>,
    pub(crate) strikethrough_color: Option<Color>,
    pub(crate) link_id: Option<Cow<'static, str>>,
}

impl Default for TextSpan {
    fn default() -> Self {
        Self::new(0..0)
    }
}

impl TextSpan {
    /// Generates an empty default span.
    #[must_use]
    pub fn new(range: Range<usize>) -> Self {
        Self {
            range: range.to_byte_range(),
            color: None,
            bg_color: None,
            font_size: None,
            font_family: None,
            font_weight: None,
            font_style: None,
            underline: None,
            underline_color: None,
            strikethrough: None,
            strikethrough_color: None,
            link_id: None,
        }
    }

    /// The range of character indices to which this decoration is applied
    #[inline]
    #[must_use]
    pub fn range(mut self, range: Range<usize>) -> Self {
        self.range = range.to_byte_range();
        self
    }

    /// Partial Text Color Override
    #[inline]
    #[must_use]
    pub fn color(mut self, color: Color) -> Self {
        self.color = Some(color);
        self
    }

    /// Overriding Partial Text Background Colors
    #[inline]
    #[must_use]
    pub fn bg_color(mut self, color: Color) -> Self {
        self.bg_color = Some(color);
        self
    }

    /// Partial Font Size Override
    #[inline]
    #[must_use]
    pub fn font_size(mut self, size: f32) -> Self {
        self.font_size = Some(size);
        self
    }

    /// Partial Font Family Override
    #[must_use]
    #[inline]
    pub fn font_family(mut self, family: impl Into<Cow<'static, str>>) -> Self {
        self.font_family = Some(family.into());
        self
    }

    /// Partial Font Weight Override
    #[inline]
    #[must_use]
    pub fn font_weight(mut self, weight: u32) -> Self {
        self.font_weight = Some(weight);
        self
    }

    /// Partial Font Style Override
    #[inline]
    #[must_use]
    pub fn font_style(mut self, style: u32) -> Self {
        self.font_style = Some(style);
        self
    }

    #[inline]
    #[must_use]
    pub fn underline(mut self, style: UnderlineStyle) -> Self {
        self.underline = Some(style);
        self
    }

    #[inline]
    #[must_use]
    pub fn underline_color(mut self, color: Color) -> Self {
        self.underline_color = Some(color);
        self
    }

    #[inline]
    #[must_use]
    pub fn strikethrough(mut self, style: StrikethroughStyle) -> Self {
        self.strikethrough = Some(style);
        self
    }

    #[inline]
    #[must_use]
    pub fn strikethrough_color(mut self, color: Color) -> Self {
        self.strikethrough_color = Some(color);
        self
    }

    #[must_use]
    #[inline]
    pub fn link_id(mut self, link: impl Into<Cow<'static, str>>) -> Self {
        self.link_id = Some(link.into());
        self
    }
}

/// Rounding Mode for Decimal Places in Numeric Input
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum RoundingMode {
    #[default]
    Floor,
    Ceil,
    Round,
}

/// 入力ロジックプロパティ
#[allow(clippy::struct_excessive_bools)]
#[derive(Debug, Clone, PartialEq)]
pub struct InputContents {
    pub(crate) text: (ReadSignal<String>, WriteSignal<String>),
    pub(crate) placeholder: Option<Cow<'static, str>>,
    pub(crate) placeholder_color: Option<Color>,
    pub(crate) max_length: Option<CharIndex>,
    pub(crate) is_password: bool,
    pub(crate) mask_text: Option<Cow<'static, str>>,
    pub(crate) numeric_only: bool,
    pub(crate) decimal_places: Option<CharIndex>,
    pub(crate) rounding_mode: Option<RoundingMode>,
    pub(crate) is_multiline: bool,
    pub(crate) placeholder_select: bool,
    // キャレットデザイン
    pub(crate) has_caret: bool,
    pub(crate) caret_width: Option<f32>,
    pub(crate) default_caret_width: f32,
    pub(crate) caret_height: Option<f32>,
    pub(crate) caret_offset: f32,
    pub(crate) caret_color: Option<Color>,
    pub(crate) is_blink: bool,
    pub(crate) blink_frequency: Option<Duration>,

    // 部分的なスタイリング (IMEの下線等に使用)
    pub(crate) rich_text: Option<TextSpan>,
    // IME制御
    pub(crate) is_ime: bool,
    pub(crate) ime_state: Option<ImeState>,

    // 選択範囲。カーソル位置は start == end で表現
    pub(crate) selected_range: Range<ByteIndex>,
    pub(crate) selection_reversed: bool,
    pub(crate) marked_range: Option<Range<ByteIndex>>,
    pub(crate) is_selecting: bool,
    pub(crate) last_layout: Option<LayoutRect>,
    pub(crate) last_bounds: Option<LayoutRect>,
    pub(crate) measured_caret: LayoutPoint,
    pub(crate) caret_line_height: f32,
    pub(crate) needs_scroll_to_caret: bool,
    // キャレットの移動・タイピングなどの最終操作時刻
    pub(crate) last_interacted_time: Option<std::time::Instant>,
    // 現在のキャレットが位置する行番号 (0始まり)
    pub(crate) current_line_index: usize,
    // 入力文字列全体の総行数
    pub(crate) total_lines: usize,

    // Undo / Redo 用履歴スタック
    pub(crate) undo_stack: Vec<(MichiuString, Range<ByteIndex>)>,
    pub(crate) redo_stack: Vec<(MichiuString, Range<ByteIndex>)>,
    pub(crate) undo_limit: usize,
}

impl InputContents {
    /// Generate new input content.
    ///
    /// # Examples
    /// ```rust
    /// use crate::{InputContents, div_n, create_signal};
    ///
    /// let (text, set_text) = create_signal(String::new());
    /// div_n().input(InputContents::new((text, set_text)))
    ///
    /// ```
    ///
    /// For Newtype patterns, you can use `bi_map`.
    /// ```rust
    /// use crate::{InputContents, div_n, create_signal};
    ///
    /// struct SearchText(String);
    /// let (reader, writer) = create_signal(SearchText(String::new()));
    /// let (text, set_text) =
    ///     reader.bi_map(writer, |t| t.0.clone(), SearchText);
    ///
    /// div_n().input(InputContents::new((text, set_text)))
    ///
    /// ```
    #[must_use]
    pub fn new(text: (ReadSignal<String>, WriteSignal<String>)) -> Self {
        Self {
            text,
            placeholder: None,
            placeholder_color: None,
            max_length: None,
            is_password: false,
            mask_text: None,
            numeric_only: false,
            decimal_places: None,
            rounding_mode: None,
            is_multiline: false,
            placeholder_select: false,
            has_caret: true,
            caret_width: None,
            default_caret_width: 1.5,
            caret_height: None,
            caret_offset: 0.0,
            caret_color: None,
            is_blink: true,
            blink_frequency: None,
            rich_text: None,
            is_ime: true,
            ime_state: None,
            selected_range: ByteIndex(0)..ByteIndex(0),
            selection_reversed: false,
            marked_range: None,
            is_selecting: false,
            last_layout: None,
            last_bounds: None,
            measured_caret: LayoutPoint::ZERO,
            caret_line_height: 0.0,
            needs_scroll_to_caret: false,
            last_interacted_time: None,
            current_line_index: 0,
            total_lines: 1,
            undo_stack: Vec::with_capacity(32),
            redo_stack: Vec::with_capacity(32),
            undo_limit: 100,
        }
    }
    #[inline]
    #[must_use]
    pub fn text(mut self, text: (ReadSignal<String>, WriteSignal<String>)) -> Self {
        self.text = text;
        self
    }
    #[inline]
    #[must_use]
    pub fn placeholder(mut self, placeholder: impl Into<Cow<'static, str>>) -> Self {
        self.placeholder = Some(placeholder.into());
        self
    }
    #[inline]
    #[must_use]
    pub fn placeholder_color(mut self, color: Color) -> Self {
        self.placeholder_color = Some(color);
        self
    }
    #[inline]
    #[must_use]
    pub fn max_length(mut self, max: usize) -> Self {
        self.max_length = Some(max.into());
        self
    }
    #[inline]
    #[must_use]
    pub fn password(mut self, enabled: bool) -> Self {
        self.is_password = enabled;
        self
    }
    #[inline]
    #[must_use]
    pub fn mask_text(mut self, mask: impl Into<Cow<'static, str>>) -> Self {
        self.mask_text = Some(mask.into());
        self
    }
    #[inline]
    #[must_use]
    pub fn numeric_only(mut self, enabled: bool) -> Self {
        self.numeric_only = enabled;
        self
    }
    #[inline]
    #[must_use]
    pub fn decimal_places(mut self, places: usize) -> Self {
        self.decimal_places = Some(places.into());
        self
    }
    #[inline]
    #[must_use]
    pub fn rounding_mode(mut self, mode: RoundingMode) -> Self {
        self.rounding_mode = Some(mode);
        self
    }
    #[inline]
    #[must_use]
    pub fn multiline(mut self, enabled: bool) -> Self {
        self.is_multiline = enabled;
        self
    }
    #[inline]
    #[must_use]
    pub fn has_caret(mut self, enabled: bool) -> Self {
        self.has_caret = enabled;
        self
    }
    #[inline]
    #[must_use]
    pub fn caret_size(mut self, size: impl IntoSize<f32>) -> Self {
        let s = size.into_size();
        self.caret_width = Some(s.width);
        self.caret_height = Some(s.height);
        self
    }
    #[inline]
    #[must_use]
    pub fn caret_width(mut self, width: f32) -> Self {
        self.caret_width = Some(width);
        self
    }
    #[inline]
    #[must_use]
    pub fn caret_height(mut self, height: f32) -> Self {
        self.caret_height = Some(height);
        self
    }

    /// Fine-tune the vertical position of the caret.
    /// Slide the plus sign down to move it lower, and the minus sign up to move it higher.
    #[inline]
    #[must_use]
    pub fn caret_offset(mut self, offset: f32) -> Self {
        self.caret_offset = offset;
        self
    }
    #[inline]
    #[must_use]
    pub fn caret_color(mut self, color: Color) -> Self {
        self.caret_color = Some(color);
        self
    }
    #[inline]
    #[must_use]
    pub fn is_blink(mut self, enabled: bool) -> Self {
        self.is_blink = enabled;
        self
    }
    #[inline]
    #[must_use]
    pub fn blink_frequency(mut self, value: Duration) -> Self {
        self.blink_frequency = Some(value);
        self
    }
    #[inline]
    #[must_use]
    pub fn is_ime(mut self, enabled: bool) -> Self {
        self.is_ime = enabled;
        self
    }
    #[inline]
    #[must_use]
    pub fn ime_state(mut self, state: ImeState) -> Self {
        self.ime_state = Some(state);
        self
    }

    #[inline]
    #[must_use]
    pub fn placeholder_select(mut self, enabled: bool) -> Self {
        self.placeholder_select = enabled;
        self
    }

    #[inline]
    #[must_use]
    pub fn undo_limit(mut self, limit: usize) -> Self {
        self.undo_limit = limit;
        self
    }

    /// 現在の状態を Undo 履歴に記録し、Redo スタックをクリア
    #[inline]
    pub(crate) fn record_undo(&mut self, text: MichiuString, selection: Range<ByteIndex>) {
        if let Some((last_text, _)) = self.undo_stack.last()
            && *last_text == text
        {
            return;
        }

        self.undo_stack.push((text, selection));
        self.redo_stack.clear(); // 新規タイピング発生時は Redo を破棄

        if self.undo_stack.len() > self.undo_limit {
            self.undo_stack.remove(0);
        }
    }

    /// Redoを実行し、現在の状態をUndoスタックへ退避させつつテキストと選択範囲を復元する
    #[inline]
    pub(crate) fn apply_redo(&mut self, next_text: MichiuString, next_sel: Range<ByteIndex>) {
        let current_text = self.to_michiu();
        let current_sel = self.selected_range.clone();
        self.undo_stack.push((current_text, current_sel));

        self.selected_range = next_sel;
        self.text.1.set(next_text.into());
    }

    /// Undoを実行し、現在の状態をRedoスタックへ退避させつつテキストと選択範囲を復元する
    #[inline]
    pub(crate) fn apply_undo(&mut self, prev_text: MichiuString, prev_sel: Range<ByteIndex>) {
        let current_text = self.to_michiu();
        let current_sel = self.selected_range.clone();
        self.redo_stack.push((current_text, current_sel));

        self.selected_range = prev_sel;
        self.text.1.set(prev_text.into());
    }

    /// パスワードモードが有効なら文字数分マスクした文字列を返し、無効ならそのままのテキストを返す
    #[inline]
    #[must_use]
    pub(crate) fn mask_if_password(&self, text: &MichiuString) -> MichiuString {
        if self.is_password {
            let mask = self.mask_text.as_deref().unwrap_or("●");
            mask.repeat(text.char_count().0).into()
        } else {
            text.clone()
        }
    }

    /// 表示テキスト（マスク後）のバイト位置を、生テキストのバイト位置に変換する
    pub(crate) fn display_byte_to_raw_byte(
        &self,
        display_byte: ByteIndex,
        raw_text: &MichiuString,
    ) -> ByteIndex {
        if self.is_password {
            let mask = self.mask_text.as_deref().unwrap_or("●");
            let mask_len = mask.len().max(1);

            // 表示上のバイト位置からCharIndexを逆算
            let char_idx = CharIndex(display_byte.0 / mask_len);

            // その文字数を生テキスト側のバイト位置にマッピング
            raw_text.to_byte_index(char_idx)
        } else {
            raw_text.clamp_to_boundary(display_byte)
        }
    }

    /// 生テキストの選択範囲を表示テキスト（マスク後）のバイト範囲に変換する
    pub(crate) fn raw_range_to_display_range(
        &self,
        raw_range: Range<ByteIndex>,
        raw_text: &MichiuString,
    ) -> Range<ByteIndex> {
        if self.is_password {
            let mask = self.mask_text.as_deref().unwrap_or("●");
            let mask_len = mask.len();

            // 生テキスト側のバイト位置から文字数を取り出す
            let start_char = raw_text.to_char_index(raw_range.start);
            let end_char = raw_text.to_char_index(raw_range.end);

            // マスク文字のバイト数を掛けて表示テキスト側の正しいバイト範囲にする
            ByteIndex(start_char.0 * mask_len)..ByteIndex(end_char.0 * mask_len)
        } else {
            raw_range
        }
    }

    #[inline]
    #[must_use]
    pub(crate) fn to_michiu(&self) -> MichiuString {
        MichiuString::from(self.text.0.get())
    }

    /// `MichiuString` で編集し、変更結果を自動的にシグナルへ書き戻すヘルパー
    #[inline]
    #[must_use]
    pub(crate) fn update_michiu<R>(&self, f: impl FnOnce(&mut MichiuString) -> R) -> R {
        let mut m = self.to_michiu();
        let r = f(&mut m);
        self.text.1.set(m.into());
        r
    }

    #[inline]
    pub(crate) fn text_empty(&self) -> bool {
        self.text.0.with(std::string::String::is_empty)
    }

    /// キャレットの点滅と描画を行うかを判定
    #[inline]
    pub(crate) fn should_show_caret(&self) -> bool {
        let now_instant = std::time::Instant::now();
        if let Some(last) = self.last_interacted_time
            && now_instant.duration_since(last) < Duration::from_millis(300)
        {
            return true; // キー入力や移動の操作から 300ms 未満のときは常時表示
        }

        // 点滅しない場合はキャレットの有無をそのまま返す
        if !self.is_blink {
            return self.has_caret;
        }

        let freq = self
            .blink_frequency
            .unwrap_or(Duration::from_millis(530))
            .as_millis();
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis();
        (now / freq).is_multiple_of(2)
    }
}
