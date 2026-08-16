use std::ops::Range;
use std::{borrow::Cow, time::Duration};

use crate::{Color, ImeState, IntoSize, LayoutRect, ReadSignal, WriteSignal};

/// リッチテキスト用の下線の描画スタイル
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UnderlineStyle {
    /// 通常の細い下線 (一般の <u> タグ、または IME 変換中・非フォーカス文節)
    Solid,
    /// 太い下線 (IME 変換フォーカス文節)
    Thick,
    /// 波線 (スペルミス、または IME 未変換・非確定文字列全体)
    Wave,
    /// 二重下線 (強調等)
    Double,
}

/// 部分的な打ち消し線（取り消し線）のスタイル
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StrikethroughStyle {
    /// 通常の細い取り消し線
    Solid,
    /// 太い取り消し線
    Thick,
}

/// 文字列の特定範囲に部分的なスタイリングを施すための、完成されたリッチテキストスパン
#[derive(Debug, Clone, PartialEq)]
pub struct TextSpan {
    /// この装飾が適用される文字インデックスの範囲 (UTF-16 単位)
    pub range: std::ops::Range<usize>,

    /// 部分的な文字色の上書き (例: リンクの青色や強調の赤色)
    pub color: Option<Color>,
    /// 部分的な文字背景色の上書き (例: マーカーの黄色や、コードブロック `` `code` `` の背景グレー)
    pub bg_color: Option<Color>,

    /// 部分的なフォントサイズの上書き
    pub font_size: Option<f32>,
    /// 部分的なフォントファミリーの上書き
    pub font_family: Option<Cow<'static, str>>,
    /// 部分的な太さ (Bold = 700等) の上書き (`DWRITE_FONT_WEIGHT` 相当)
    pub font_weight: Option<u32>,
    /// 部分的な斜体 (Normal=0, Italic=2等) の上書き (`DWRITE_FONT_STYLE` 相当)
    pub font_style: Option<u32>,

    /// 下線の種類 (標準下線、太下線、波下線)
    pub underline: Option<UnderlineStyle>,
    /// 下線の色
    pub underline_color: Option<Color>,
    /// 打ち消し線 (取り消し線) の種類
    pub strikethrough: Option<StrikethroughStyle>,
    /// 打ち消し線の色
    pub strikethrough_color: Option<Color>,

    /// リンクとしてクリック可能にする場合、識別子を入れておき
    /// ヒットテスト時にイベントをフック可能にします
    pub link_id: Option<Cow<'static, str>>,
}

impl Default for TextSpan {
    fn default() -> Self {
        Self::new(Range { start: 0, end: 0 })
    }
}

impl TextSpan {
    /// 空のデフォルトスパンを生成します
    #[must_use]
    pub fn new(range: std::ops::Range<usize>) -> Self {
        Self {
            range,
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
    #[inline]
    #[must_use]
    pub fn range(mut self, range: std::ops::Range<usize>) -> Self {
        self.range = range;
        self
    }
    #[inline]
    #[must_use]
    pub fn color(mut self, color: Color) -> Self {
        self.color = Some(color);
        self
    }
    #[inline]
    #[must_use]
    pub fn bg_color(mut self, color: Color) -> Self {
        self.bg_color = Some(color);
        self
    }
    #[inline]
    #[must_use]
    pub fn font_size(mut self, size: f32) -> Self {
        self.font_size = Some(size);
        self
    }
    #[must_use]
    #[inline]
    pub fn font_family(mut self, family: impl Into<Cow<'static, str>>) -> Self {
        self.font_family = Some(family.into());
        self
    }
    #[inline]
    #[must_use]
    pub fn font_weight(mut self, weight: u32) -> Self {
        self.font_weight = Some(weight);
        self
    }
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

/// 数値入力における小数点以下の丸め（クランプ）モード
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum RoundingMode {
    /// 切り捨て
    #[default]
    Floor,
    /// 切り上げ
    Ceil,
    /// 四捨五入
    Round,
}

/// 入力ロジックプロパティ
#[allow(clippy::struct_excessive_bools)]
#[derive(Debug, Clone, PartialEq)]
pub struct InputContents {
    pub text: (ReadSignal<String>, WriteSignal<String>),
    pub placeholder: Option<Cow<'static, str>>,
    pub placeholder_color: Option<Color>,
    pub max_length: Option<usize>,
    pub is_password: bool,
    pub mask_text: Option<Cow<'static, str>>,
    pub numeric_only: bool,
    pub decimal_places: Option<usize>,
    pub rounding_mode: Option<RoundingMode>,
    pub is_multiline: bool,
    pub placeholder_select: bool,
    pub auto_wrap: bool,

    // キャレットデザイン
    pub has_caret: bool,
    pub caret_width: Option<f32>,
    pub default_caret_width: f32,
    pub caret_height: Option<f32>,
    pub caret_offset: f32,
    pub caret_color: Option<Color>,
    pub is_blink: bool,
    pub blink_frequency: Option<Duration>,

    /// 部分的なスタイリング (IMEの下線等に使用)
    pub(crate) rich_text: Option<TextSpan>,
    // IME制御
    pub is_ime: bool,
    pub ime_state: Option<ImeState>,

    /// 選択範囲。カーソル位置は start == end で表現
    pub(crate) selected_range: Range<usize>,
    pub(crate) selection_reversed: bool,
    pub(crate) marked_range: Option<Range<usize>>,
    pub(crate) is_selecting: bool,
    pub(crate) last_layout: Option<LayoutRect>,
    pub(crate) last_bounds: Option<LayoutRect>,
    pub(crate) measured_caret_x: f32,
    pub(crate) measured_caret_y: f32,
    pub(crate) caret_line_height: f32,
    /// キャレットの移動・タイピングなどの最終操作時刻
    pub(crate) last_interacted_time: Option<std::time::Instant>,
    /// 現在のキャレットが位置する行番号 (0始まり)
    pub current_line_index: usize,
    /// 入力文字列全体の総行数
    pub total_lines: usize,
    pub total_len: usize,

    // Undo / Redo 用履歴スタック
    pub(crate) undo_stack: Vec<(String, Range<usize>)>,
    pub(crate) redo_stack: Vec<(String, Range<usize>)>,
    pub undo_limit: usize,
}

impl InputContents {
    /// 新規に入力コンテンツの起点を作成します（不足フィールドの初期化を完全修正）。
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
            auto_wrap: true,
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
            selected_range: 0..0,
            selection_reversed: false,
            marked_range: None,
            is_selecting: false,
            last_layout: None,
            last_bounds: None,
            measured_caret_x: 0.0,
            measured_caret_y: 0.0,
            caret_line_height: 0.0,
            last_interacted_time: None,
            current_line_index: 0,
            total_lines: 1,
            total_len: 0,
            undo_stack: Vec::new(),
            redo_stack: Vec::new(),
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
        self.max_length = Some(max);
        self
    }
    #[inline]
    #[must_use]
    pub fn total_len(&self) -> usize {
        self.total_len
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
        self.decimal_places = Some(places);
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
    pub fn auto_wrap(mut self, enabled: bool) -> Self {
        self.auto_wrap = enabled;
        self
    }
    #[inline]
    #[must_use]
    pub fn has_caret(mut self, enabled: bool) -> Self {
        self.has_caret = enabled;
        self
    }
    /// キャレットの太さと高さを一括設定します。単一値(f32)またはタプル(f32, f32)を受け入れます。
    #[inline]
    #[must_use]
    pub fn caret_size(mut self, size: impl IntoSize<f32>) -> Self {
        let s = size.into_size();
        self.caret_width = Some(s.width);
        self.caret_height = Some(s.height);
        self
    }

    /// キャレットの太さ（幅）のみを個別設定します。
    #[inline]
    #[must_use]
    pub fn caret_width(mut self, width: f32) -> Self {
        self.caret_width = Some(width);
        self
    }

    /// キャレットの高さのみを個別設定します。
    #[inline]
    #[must_use]
    pub fn caret_height(mut self, height: f32) -> Self {
        self.caret_height = Some(height);
        self
    }

    /// キャレットの表示位置を垂直方向に微調整します。プラスは下、マイナスは上にスライドします。
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

    /// 現在の状態を Undo 履歴に記録し、Redo スタックをクリアします
    pub(crate) fn record_undo(&mut self, text: String, selection: Range<usize>) {
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
}

impl InputContents {
    /// 指定されたUTF-16の範囲（Range）を文字列から安全に削除し、新しい文字列を返します
    #[inline]
    pub(crate) fn remove_utf16_range(text: &str, range: &Range<usize>) -> String {
        let u16_text: Vec<u16> = text.encode_utf16().collect();
        let mut left = u16_text[..range.start.min(u16_text.len())].to_vec();
        let right = u16_text[range.end.min(u16_text.len())..].to_vec();
        left.extend_from_slice(&right);
        String::from_utf16_lossy(&left)
    }

    /// キー入力（文字）の挿入をマルチバイト対応で安全に行います
    pub(crate) fn input_insert_char(
        text: &str,
        caret_offset: &mut usize,
        ch: char,
        max_len: Option<usize>,
        numeric_only: bool,
    ) -> String {
        let mut u16_text: Vec<u16> = text.encode_utf16().collect();

        // 数値限定フィルタ
        if numeric_only && !ch.is_numeric() && ch != '.' && ch != '-' {
            return text.to_string();
        }
        // 文字数制限
        if let Some(max) = max_len
            && u16_text.len() >= max
        {
            return text.to_string();
        }

        // 挿入位置を確定
        let mut insert_pos = (*caret_offset).min(u16_text.len());

        // 挿入位置が下位サロゲートの開始位置だった場合、
        // サロゲートペアを破壊しないよう、位置を1つ前にずらす（または後ろにずらす）
        if insert_pos > 0 && insert_pos < u16_text.len() {
            let target_unit = u16_text[insert_pos];
            // 0xDC00 ～ 0xDFFF は下位サロゲート
            if (0xDC00..=0xDFFF).contains(&target_unit) {
                insert_pos -= 1;
            }
        }

        let mut buf = [0u16; 2];
        let ch_u16_slice = ch.encode_utf16(&mut buf);

        u16_text.splice(insert_pos..insert_pos, ch_u16_slice.iter().copied());
        *caret_offset = insert_pos + ch_u16_slice.len();

        String::from_utf16(&u16_text).unwrap_or_else(|_| text.to_string())
    }

    /// Backspace（一文字削除）を実行
    #[inline]
    pub(crate) fn input_backspace(text: &str, caret_offset: &mut usize) -> String {
        let mut u16_text: Vec<u16> = text.encode_utf16().collect();
        *caret_offset = (*caret_offset).min(u16_text.len());
        if *caret_offset > 0 && !u16_text.is_empty() {
            let remove_pos = *caret_offset - 1;
            u16_text.remove(remove_pos);
            *caret_offset -= 1;
        }
        String::from_utf16(&u16_text).unwrap_or_else(|_| text.to_string())
    }

    /// Delete（カーソル右側一文字削除）を実行
    #[inline]
    pub(crate) fn input_delete(text: &str, caret_offset: usize) -> String {
        let mut u16_text: Vec<u16> = text.encode_utf16().collect();
        let safe_offset = caret_offset.min(u16_text.len());
        if safe_offset < u16_text.len() {
            u16_text.remove(caret_offset);
        }
        String::from_utf16(&u16_text).unwrap_or_else(|_| text.to_string())
    }

    /// IME未確定テキストをカーソル位置にマージした画面表示用テキストを合成します
    pub(crate) fn input_get_display_text(
        base_text: &str,
        caret_offset: usize,
        composition: &str,
    ) -> String {
        let u16_base: Vec<u16> = base_text.encode_utf16().collect();
        let u16_comp: Vec<u16> = composition.encode_utf16().collect();

        let mut final_u16 = Vec::with_capacity(u16_base.len() + u16_comp.len());
        let split = caret_offset.min(u16_base.len());

        final_u16.extend_from_slice(&u16_base[..split]);
        final_u16.extend_from_slice(&u16_comp);
        final_u16.extend_from_slice(&u16_base[split..]);

        String::from_utf16(&final_u16).unwrap_or_default()
    }

    /// 文字列の改行文字 '\n' を数えて、現在のキャレットの行番号 (0始まり) と総行数を算出するヘルパー
    pub(crate) fn calculate_line_indices(text: &str, caret_offset: usize) -> (usize, usize) {
        let u16_text: Vec<u16> = text.encode_utf16().collect();
        let caret_clamped = caret_offset.min(u16_text.len());

        // 現在のカーソル位置よりも前にある '\n' の数が、現在の行インデックスになる
        let current_line = u16_text[..caret_clamped]
            .iter()
            .filter(|&&c| c == '\n' as u16)
            .count();

        let total_lines = u16_text.iter().filter(|&&c| c == '\n' as u16).count() + 1;

        (current_line, total_lines)
    }

    /// 文字種を判定してクラスIDを返します（UTF-16単位）
    fn get_char_class(ch: u16) -> u8 {
        if ch == ' ' as u16 || ch == '\t' as u16 || ch == '\n' as u16 || ch == '\r' as u16 {
            0 // 空白文字・改行
        } else if (0x3040..=0x309F).contains(&ch) {
            1 // ひらがな
        } else if (0x30A0..=0x30FF).contains(&ch) {
            2 // カタカナ
        } else if (0x4E00..=0x9FFF).contains(&ch) {
            3 // 漢字 (CJK 統合漢字)
        } else if (ch >= 'a' as u16 && ch <= 'z' as u16)
            || (ch >= 'A' as u16 && ch <= 'Z' as u16)
            || (ch >= '0' as u16 && ch <= '9' as u16)
            || ch == '_' as u16
        {
            4 // 英数字・アンダースコア
        } else {
            5 // 記号・その他
        }
    }

    /// 指定した文字インデックス周辺の文節・単語境界（Range）を安全にスキャンします
    pub(crate) fn find_word_boundaries(text: &[u16], index: usize) -> Range<usize> {
        if text.is_empty() {
            return 0..0;
        }
        let mut index = index.min(text.len() - 1);

        // サロゲートペア（下位サロゲート）に着地した場合は上位サロゲートへ1つ戻す保護
        if index > 0 && (0xDC00..=0xDFFF).contains(&text[index]) {
            index -= 1;
        }

        let target_class = InputContents::get_char_class(text[index]);

        // 左方向へ同じ文字種が続く限りスキャン
        let mut start = index;
        while start > 0 {
            if InputContents::get_char_class(text[start - 1]) != target_class {
                break;
            }
            start -= 1;
        }
        // 左端サロゲートペア分断防止
        if start > 0 && (0xDC00..=0xDFFF).contains(&text[start]) {
            start -= 1;
        }

        // 右方向へ同じ文字種が続く限りスキャン
        let mut end = index;
        while end < text.len() {
            if InputContents::get_char_class(text[end]) != target_class {
                break;
            }
            end += 1;
        }
        // 右端サロゲートペア分断防止
        if end < text.len() && (0xDC00..=0xDFFF).contains(&text[end]) {
            end += 1;
        }

        start..end
    }
}
