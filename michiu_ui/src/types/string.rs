use crate::{ByteIndex, CharIndex};
use std::{
    borrow::Cow,
    fmt,
    ops::{Deref, Range},
};

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct CharClassID(pub u8);

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct MichiuString(pub Cow<'static, str>);

impl MichiuString {
    #[inline]
    pub fn new(text: impl Into<Cow<'static, str>>) -> Self {
        Self(text.into())
    }

    #[inline]
    #[must_use]
    pub fn byte_len(&self) -> ByteIndex {
        ByteIndex(self.0.len())
    }

    #[inline]
    #[must_use]
    pub fn char_count(&self) -> CharIndex {
        CharIndex(self.0.chars().count())
    }

    /// 文字数インデックス（CharIndex）からバイトインデックス（ByteIndex）へ変換
    #[inline]
    #[must_use]
    pub fn to_byte_index(&self, char_idx: CharIndex) -> ByteIndex {
        let pos = self
            .0
            .char_indices()
            .map(|(b, _)| b)
            .nth(char_idx.0)
            .unwrap_or(self.0.len());
        ByteIndex(pos)
    }

    /// バイトインデックス（ByteIndex）から、文字数インデックス（CharIndex）へ変換
    #[inline]
    #[must_use]
    pub fn to_char_index(&self, byte_idx: ByteIndex) -> CharIndex {
        let clamped = self.clamp_to_boundary(byte_idx).0;
        let char_pos = self.0[..clamped].chars().count();
        CharIndex(char_pos)
    }

    /// 指定位置を安全な文字の開始境界に補正して返す
    #[inline]
    #[must_use]
    pub fn clamp_to_boundary(&self, at: ByteIndex) -> ByteIndex {
        let mut pos = at.0.min(self.0.len());
        while pos > 0 && !self.0.is_char_boundary(pos) {
            pos -= 1;
        }
        ByteIndex(pos)
    }

    /// 現在位置から左へ1文字移動した安全な位置を返す
    #[inline]
    #[must_use]
    pub fn prev_char_boundary(&self, at: ByteIndex) -> ByteIndex {
        let current = self.clamp_to_boundary(at).0;
        if current > 0 {
            let mut prev = current - 1;
            while prev > 0 && !self.0.is_char_boundary(prev) {
                prev -= 1;
            }
            ByteIndex(prev)
        } else {
            ByteIndex(0)
        }
    }

    /// 現在位置から右へ1文字移動した安全な位置を返す
    #[inline]
    #[must_use]
    pub fn next_char_boundary(&self, at: ByteIndex) -> ByteIndex {
        let current = self.clamp_to_boundary(at).0;
        if current < self.0.len() {
            let ch = self.0[current..].chars().next().unwrap_or(' ');
            ByteIndex(current + ch.len_utf8())
        } else {
            ByteIndex(self.0.len())
        }
    }

    /// 1文字挿入し、挿入後の新しいキャレット位置を返す
    #[inline]
    #[must_use]
    pub fn insert_char(&mut self, at: ByteIndex, ch: char) -> ByteIndex {
        let insert_pos = self.clamp_to_boundary(at).0;
        let mut buf = [0u8; 4];
        let ch_str = ch.encode_utf8(&mut buf);

        let s = self.0.to_mut();
        s.insert_str(insert_pos, ch_str);

        ByteIndex(insert_pos + ch_str.len())
    }

    /// 文字列を一括挿入し、挿入後の新しいキャレット位置を返す
    #[inline]
    #[must_use]
    pub fn insert_str(&mut self, at: ByteIndex, text: &str) -> ByteIndex {
        let insert_pos = self.clamp_to_boundary(at).0;
        let s = self.0.to_mut();
        s.insert_str(insert_pos, text);
        ByteIndex(insert_pos + text.len())
    }

    /// 指定位置にテキストを挿入した、新しい `MichiuString` を生成して返す（表示用テキスト合成用）
    #[inline]
    #[must_use]
    pub fn inserted(&self, at: ByteIndex, text: &str) -> Self {
        let mut cloned = self.clone();
        let _ = cloned.insert_str(at, text);
        cloned
    }

    /// 指定範囲を新しい文字列で置き換え、挿入後の安全なキャレット位置を返す
    /// 範囲が空なら単なる挿入、範囲があれば削除＋挿入として動作
    #[inline]
    #[must_use]
    pub fn replace_range(&mut self, range: Range<ByteIndex>, text: &str) -> ByteIndex {
        let start = self.clamp_to_boundary(range.start).0;
        let end = self.clamp_to_boundary(range.end).0;

        let s = self.0.to_mut();
        if start < end {
            s.replace_range(start..end, text);
        } else {
            s.insert_str(start, text);
        }

        ByteIndex(start + text.len())
    }

    /// 範囲をスライスして `&str` として切り出す
    #[inline]
    #[must_use]
    pub fn slice(&self, range: Range<ByteIndex>) -> &str {
        let start = self.clamp_to_boundary(range.start).0;
        let end = self.clamp_to_boundary(range.end).0;
        if start <= end {
            &self.0[start..end]
        } else {
            ""
        }
    }

    /// 指定範囲を削除し、削除後のキャレット位置（範囲の始点）を返す
    #[inline]
    #[must_use]
    pub fn remove_range(&mut self, range: Range<ByteIndex>) -> ByteIndex {
        let start = self.clamp_to_boundary(range.start).0;
        let end = self.clamp_to_boundary(range.end).0;

        if start < end {
            let s = self.0.to_mut();
            s.replace_range(start..end, "");
        }
        ByteIndex(start)
    }

    /// 直前の1文字を削除し、新しいキャレット位置を返す
    #[inline]
    #[must_use]
    pub fn backspace(&mut self, at: ByteIndex) -> ByteIndex {
        let current_pos = self.clamp_to_boundary(at).0;
        if current_pos > 0 {
            let mut remove_start = current_pos - 1;
            while remove_start > 0 && !self.0.is_char_boundary(remove_start) {
                remove_start -= 1;
            }
            let s = self.0.to_mut();
            s.replace_range(remove_start..current_pos, "");
            ByteIndex(remove_start)
        } else {
            ByteIndex(0)
        }
    }

    /// 直後の1文字を削除を行い、キャレット位置（変化なし）を返す
    #[inline]
    #[must_use]
    pub fn delete(&mut self, at: ByteIndex) -> ByteIndex {
        let current_pos = self.clamp_to_boundary(at).0;
        if current_pos < self.0.len() {
            let mut remove_end = current_pos + 1;
            while remove_end < self.0.len() && !self.0.is_char_boundary(remove_end) {
                remove_end += 1;
            }
            let s = self.0.to_mut();
            s.replace_range(current_pos..remove_end, "");
        }
        ByteIndex(current_pos)
    }

    /// キャレット位置における (現在の行番号, 総行数) を算出する
    #[inline]
    #[must_use]
    pub fn line_indices(&self, at: ByteIndex) -> (usize, usize) {
        let bytes = self.0.as_bytes();
        let clamped = at.0.min(bytes.len());
        let current_line = bytecount::count(&bytes[..clamped], b'\n');
        let total_lines = bytecount::count(bytes, b'\n') + 1;
        (current_line, total_lines)
    }

    /// 指定位置の文字クラスを取得する
    #[inline]
    #[must_use]
    pub fn char_class_at(&self, at: ByteIndex) -> CharClassID {
        let pos = self.clamp_to_boundary(at).0;
        let ch = self.0[pos..].chars().next().unwrap_or(' ');

        let id = if ch == ' ' || ch == '\t' || ch == '\n' || ch == '\r' {
            0
        } else if ('\u{3040}'..='\u{309F}').contains(&ch) {
            1
        } else if ('\u{30A0}'..='\u{30FF}').contains(&ch) {
            2
        } else if ('\u{4E00}'..='\u{9FFF}').contains(&ch) {
            3
        } else if ch.is_ascii_alphanumeric() || ch == '_' {
            4
        } else {
            5
        };
        CharClassID(id)
    }

    /// 単語の境界範囲をスキャンする
    #[inline]
    #[must_use]
    pub fn find_word_boundaries(&self, at: ByteIndex) -> Range<ByteIndex> {
        if self.0.is_empty() {
            return ByteIndex(0)..ByteIndex(0);
        }

        let mut pos = self.clamp_to_boundary(at).0;
        if pos == self.0.len() {
            let mut prev = pos - 1;
            while prev > 0 && !self.0.is_char_boundary(prev) {
                prev -= 1;
            }
            pos = prev;
        }

        let target_class = self.char_class_at(ByteIndex(pos));

        // 左方向
        let mut start = pos;
        while start > 0 {
            let mut prev = start - 1;
            while prev > 0 && !self.0.is_char_boundary(prev) {
                prev -= 1;
            }
            if self.char_class_at(ByteIndex(prev)) != target_class {
                break;
            }
            start = prev;
        }

        // 右方向
        let mut end = pos;
        while end < self.0.len() {
            let current_char = self.0[end..].chars().next().unwrap_or(' ');
            if self.char_class_at(ByteIndex(end)) != target_class {
                break;
            }
            end += current_char.len_utf8();
        }

        ByteIndex(start)..ByteIndex(end)
    }
}

// ======================================================================
//
// ======================================================================

impl Deref for MichiuString {
    type Target = str;
    #[inline]
    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl AsRef<str> for MichiuString {
    #[inline]
    fn as_ref(&self) -> &str {
        &self.0
    }
}

impl From<&'static str> for MichiuString {
    #[inline]
    fn from(s: &'static str) -> Self {
        Self(Cow::Borrowed(s))
    }
}

impl From<String> for MichiuString {
    #[inline]
    fn from(s: String) -> Self {
        Self(Cow::Owned(s))
    }
}

impl From<Cow<'static, str>> for MichiuString {
    #[inline]
    fn from(cow: Cow<'static, str>) -> Self {
        Self(cow)
    }
}

impl From<MichiuString> for String {
    #[inline]
    fn from(s: MichiuString) -> Self {
        s.0.into_owned()
    }
}

impl fmt::Display for MichiuString {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl Default for MichiuString {
    #[inline]
    fn default() -> Self {
        MichiuString::new(Cow::Borrowed(""))
    }
}
