use crate::{ByteIndex, CharIndex};
use std::{
    borrow::Cow,
    fmt,
    ops::{Deref, Range},
    sync::Arc,
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
    pub(crate) fn byte_len(&self) -> ByteIndex {
        ByteIndex(self.0.len())
    }

    #[inline]
    #[must_use]
    pub(crate) fn char_count(&self) -> CharIndex {
        CharIndex(self.0.chars().count())
    }

    /// Converting from Character Index (`CharIndex`) to Byte Index (`ByteIndex`)
    #[inline]
    #[must_use]
    pub(crate) fn to_byte_index(&self, char_idx: CharIndex) -> ByteIndex {
        let pos = self
            .0
            .char_indices()
            .map(|(b, _)| b)
            .nth(char_idx.0)
            .unwrap_or(self.0.len());
        ByteIndex(pos)
    }

    /// Convert from Byte Index (`ByteIndex`) to Character Index (`CharIndex`)
    #[inline]
    #[must_use]
    pub fn to_char_index(&self, byte_idx: ByteIndex) -> CharIndex {
        let clamped = self.clamp_to_boundary(byte_idx).0;
        let char_pos = self.0[..clamped].chars().count();
        CharIndex(char_pos)
    }

    /// Returns the specified position adjusted to the start boundary of a safe character
    #[inline]
    #[must_use]
    pub fn clamp_to_boundary(&self, at: ByteIndex) -> ByteIndex {
        let mut pos = at.0.min(self.0.len());
        while pos > 0 && !self.0.is_char_boundary(pos) {
            pos -= 1;
        }
        ByteIndex(pos)
    }

    /// Returns a safe position one character to the left of the current position
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

    /// Returns a safe position one character to the right of the current position
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

    /// Inserts one character and returns the new cursor position after the insertion
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

    /// Inserts a string in bulk and returns the new cursor position after insertion
    #[inline]
    #[must_use]
    pub fn insert_str(&mut self, at: ByteIndex, text: &str) -> ByteIndex {
        let insert_pos = self.clamp_to_boundary(at).0;
        let s = self.0.to_mut();
        s.insert_str(insert_pos, text);
        ByteIndex(insert_pos + text.len())
    }

    /// Creates and returns a new `MichiuString` with text inserted at the specified position.
    #[inline]
    #[must_use]
    pub fn inserted(&self, at: ByteIndex, text: &str) -> Self {
        let mut cloned = self.clone();
        let _ = cloned.insert_str(at, text);
        cloned
    }

    /// Replaces the specified range with a new string and returns the safe caret position after insertion
    ///
    /// If the range is empty, it acts as a simple insertion; if the range exists, it acts as a delete-and-insert operation
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

    /// Slice the range and extract it as `&str`
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

    /// Delete the specified range and return the caret position after the deletion
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

    /// Deletes the character immediately before the cursor and returns the new cursor position
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

    /// Deletes the character immediately following the cursor and returns the cursor position.
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

    /// Calculate (current line number, total number of lines) at the cursor position
    #[inline]
    #[must_use]
    pub fn line_indices(&self, at: ByteIndex) -> (usize, usize) {
        let bytes = self.0.as_bytes();
        let clamped = at.0.min(bytes.len());
        let current_line = bytecount::count(&bytes[..clamped], b'\n');
        let total_lines = bytecount::count(bytes, b'\n') + 1;
        (current_line, total_lines)
    }

    /// Get the character class at a specified position
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

    /// Find the word boundary range
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

impl From<Arc<String>> for MichiuString {
    fn from(arc_str: Arc<String>) -> Self {
        Self((*arc_str).clone().into())
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

#[cfg(test)]
mod tests {
    use super::*;

    // テスト文字列: "A" (1B) + "あ" (3B) + "🦀" (4B)
    // インデックス構成:
    // [0..1]: 'A'
    // [1..4]: 'あ'
    // [4..8]: '🦀'
    // 8: 末尾 (EOF)
    const SAMPLE: &str = "Aあ🦀";

    #[test]
    fn test_new_and_empty() {
        let s = MichiuString::new("");
        assert_eq!(s.byte_len(), ByteIndex(0));
        assert_eq!(s.char_count(), CharIndex(0));
        assert_eq!(s.to_byte_index(CharIndex(0)), ByteIndex(0));
        assert_eq!(s.to_byte_index(CharIndex(10)), ByteIndex(0));
        assert_eq!(s.to_char_index(ByteIndex(0)), CharIndex(0));
    }

    #[test]
    fn test_byte_len_and_char_count() {
        // ASCII: 1文字1バイト
        let ascii = MichiuString::new("hello");
        assert_eq!(ascii.byte_len(), ByteIndex(5));
        assert_eq!(ascii.char_count(), CharIndex(5));

        // 日本語: 1文字3バイト
        let ja = MichiuString::new("こんにちは");
        assert_eq!(ja.byte_len(), ByteIndex(15));
        assert_eq!(ja.char_count(), CharIndex(5));

        // 絵文字: 1文字4バイト
        let emoji = MichiuString::new("🦀🦀🦀");
        assert_eq!(emoji.byte_len(), ByteIndex(12));
        assert_eq!(emoji.char_count(), CharIndex(3));
    }

    #[test]
    fn test_to_byte_index_multibyte() {
        // 'a': 1byte (0..1)
        // 'あ': 3bytes (1..4)
        // '🦀': 4bytes (4..8)
        // 'z': 1byte (8..9)
        let s = MichiuString::new("aあ🦀z");

        assert_eq!(s.to_byte_index(CharIndex(0)), ByteIndex(0)); // 'a'
        assert_eq!(s.to_byte_index(CharIndex(1)), ByteIndex(1)); // 'あ'
        assert_eq!(s.to_byte_index(CharIndex(2)), ByteIndex(4)); // '🦀'
        assert_eq!(s.to_byte_index(CharIndex(3)), ByteIndex(8)); // 'z'
        assert_eq!(s.to_byte_index(CharIndex(4)), ByteIndex(9)); // 末尾境界

        // 範囲外の CharIndex は len にフォールバックされる仕様
        assert_eq!(s.to_byte_index(CharIndex(5)), ByteIndex(9));
        assert_eq!(s.to_byte_index(CharIndex(100)), ByteIndex(9));
    }

    #[test]
    fn test_to_char_index_multibyte() {
        let s = MichiuString::new("aあ🦀z");

        // 正常な文字境界
        assert_eq!(s.to_char_index(ByteIndex(0)), CharIndex(0)); // 'a' の先頭
        assert_eq!(s.to_char_index(ByteIndex(1)), CharIndex(1)); // 'あ' の先頭
        assert_eq!(s.to_char_index(ByteIndex(4)), CharIndex(2)); // '🦀' の先頭
        assert_eq!(s.to_char_index(ByteIndex(8)), CharIndex(3)); // 'z' の先頭
        assert_eq!(s.to_char_index(ByteIndex(9)), CharIndex(4)); // 末尾

        // マルチバイト文字の途中を指定した場合
        // 'あ' の途中 (ByteIndex 2, 3) -> 1 ('あ') に丸められるか
        assert_eq!(s.to_char_index(ByteIndex(2)), CharIndex(1));
        assert_eq!(s.to_char_index(ByteIndex(3)), CharIndex(1));

        // '🦀' の途中 (ByteIndex 5, 6, 7) -> 2 ('🦀') に丸められるか
        assert_eq!(s.to_char_index(ByteIndex(5)), CharIndex(2));
        assert_eq!(s.to_char_index(ByteIndex(7)), CharIndex(2));
    }

    #[test]
    fn test_round_trip() {
        // 相互変換の可逆性テスト（CharIndex -> ByteIndex -> CharIndex は常に元に戻る）
        let s = MichiuString::new("Hello, 世界! 🦀");

        for i in 0..=s.char_count().0 {
            let char_idx = CharIndex(i);
            let byte_idx = s.to_byte_index(char_idx);
            let round_trip_char_idx = s.to_char_index(byte_idx);

            assert_eq!(
                round_trip_char_idx, char_idx,
                "Failed round-trip at char index {i}"
            );
        }
    }

    #[test]
    fn test_clamp_to_boundary() {
        let s = MichiuString::new(SAMPLE);

        // 正常な境界はそのまま
        assert_eq!(s.clamp_to_boundary(ByteIndex(0)), ByteIndex(0)); // 'A'
        assert_eq!(s.clamp_to_boundary(ByteIndex(1)), ByteIndex(1)); // 'あ'
        assert_eq!(s.clamp_to_boundary(ByteIndex(4)), ByteIndex(4)); // '🦀'
        assert_eq!(s.clamp_to_boundary(ByteIndex(8)), ByteIndex(8)); // 末尾

        // マルチバイト文字の途中 -> 先頭境界へ巻き戻る
        assert_eq!(s.clamp_to_boundary(ByteIndex(2)), ByteIndex(1)); // 'あ' の2バイト目 -> 'あ' の先頭
        assert_eq!(s.clamp_to_boundary(ByteIndex(3)), ByteIndex(1)); // 'あ' の3バイト目 -> 'あ' の先頭
        assert_eq!(s.clamp_to_boundary(ByteIndex(5)), ByteIndex(4)); // '🦀' の2バイト目 -> '🦀' の先頭
        assert_eq!(s.clamp_to_boundary(ByteIndex(7)), ByteIndex(4)); // '🦀' の4バイト目 -> '🦀' の先頭

        // 範囲外（len超え） -> len にクランプ
        assert_eq!(s.clamp_to_boundary(ByteIndex(9)), ByteIndex(8));
        assert_eq!(s.clamp_to_boundary(ByteIndex(999)), ByteIndex(8));
    }

    #[test]
    fn test_prev_char_boundary() {
        let s = MichiuString::new(SAMPLE);

        // 境界からの移動（1文字左へ）
        assert_eq!(s.prev_char_boundary(ByteIndex(8)), ByteIndex(4)); // 末尾 -> '🦀'
        assert_eq!(s.prev_char_boundary(ByteIndex(4)), ByteIndex(1)); // '🦀' -> 'あ'
        assert_eq!(s.prev_char_boundary(ByteIndex(1)), ByteIndex(0)); // 'あ' -> 'A'

        // 先頭からさらに左へ行こうとしても 0 のまま
        assert_eq!(s.prev_char_boundary(ByteIndex(0)), ByteIndex(0));

        // 文字の途中からの移動（clamp されてから 1 文字左へ行く挙動）
        // '🦀' (4..8) の途中の 6 -> clamp で 4 ('🦀') -> その prev で 1 ('あ')
        assert_eq!(s.prev_char_boundary(ByteIndex(6)), ByteIndex(1));
    }

    #[test]
    fn test_next_char_boundary() {
        let s = MichiuString::new(SAMPLE);

        // 境界からの移動（1文字右へ）
        assert_eq!(s.next_char_boundary(ByteIndex(0)), ByteIndex(1)); // 'A' -> 'あ'
        assert_eq!(s.next_char_boundary(ByteIndex(1)), ByteIndex(4)); // 'あ' -> '🦀'
        assert_eq!(s.next_char_boundary(ByteIndex(4)), ByteIndex(8)); // '🦀' -> 末尾

        // 末尾からさらに右へ行こうとしても len のまま
        assert_eq!(s.next_char_boundary(ByteIndex(8)), ByteIndex(8));

        // 文字の途中からの移動（clamp されてから 1 文字右へ行く挙動）
        // 'あ' (1..4) の途中の 2 -> clamp で 1 ('あ') -> その next で 4 ('🦀')
        assert_eq!(s.next_char_boundary(ByteIndex(2)), ByteIndex(4));
    }

    #[test]
    fn test_empty_string_boundaries() {
        let empty = MichiuString::new("");

        assert_eq!(empty.clamp_to_boundary(ByteIndex(0)), ByteIndex(0));
        assert_eq!(empty.clamp_to_boundary(ByteIndex(5)), ByteIndex(0));
        assert_eq!(empty.prev_char_boundary(ByteIndex(0)), ByteIndex(0));
        assert_eq!(empty.next_char_boundary(ByteIndex(0)), ByteIndex(0));
    }

    #[test]
    fn test_cursor_traverse_roundtrip() {
        // 先頭から next で歩いていき、末尾に着いたら prev で戻ってくるテスト
        let s = MichiuString::new("Rust🦀日本！");
        let len = s.byte_len();

        // 右へ全移動
        let mut cur = ByteIndex(0);
        let mut forward_history = vec![cur];
        while cur < len {
            let next = s.next_char_boundary(cur);
            assert!(next > cur, "Cursor must advance forward");
            cur = next;
            forward_history.push(cur);
        }
        assert_eq!(cur, len);

        // 左へ全移動（履歴の逆順と完全一致するはず）
        let mut backward_history = vec![cur];
        while cur > ByteIndex(0) {
            let prev = s.prev_char_boundary(cur);
            assert!(prev < cur, "Cursor must retreat backward");
            cur = prev;
            backward_history.push(cur);
        }
        assert_eq!(cur, ByteIndex(0));

        forward_history.reverse();
        assert_eq!(forward_history, backward_history);
    }

    #[test]
    fn test_insert_char() {
        // 先頭への挿入 (ASCII)
        let mut s = MichiuString::new("world");
        let next_pos = s.insert_char(ByteIndex(0), 'H');
        assert_eq!(s.0.as_ref(), "Hworld");
        assert_eq!(next_pos, ByteIndex(1)); // 'H' (1B) の後ろ

        // 中間への挿入 (マルチバイト: 絵文字 4B)
        let mut s = MichiuString::new("Rust!");
        // 'R', 'u', 's', 't' の後ろ (ByteIndex 4)
        let next_pos = s.insert_char(ByteIndex(4), '🦀');
        assert_eq!(s.0.as_ref(), "Rust🦀!");
        assert_eq!(next_pos, ByteIndex(8)); // 4 + 4B = 8

        // 末尾への挿入 (日本語 3B)
        let mut s = MichiuString::new("こんにちは");
        let len = s.byte_len(); // 15
        let next_pos = s.insert_char(len, '！');
        assert_eq!(s.0.as_ref(), "こんにちは！");
        assert_eq!(next_pos, ByteIndex(18)); // 15 + 3B = 18

        // マルチバイト文字の途中を指定した場合（クラッシュせず clamp されて手前に挿入）
        // "あ" (0..3) の途中の 2 を指定 -> 0 ('あ' の手前) に挿入される
        let mut s = MichiuString::new("あい");
        let next_pos = s.insert_char(ByteIndex(2), 'X');
        assert_eq!(s.0.as_ref(), "Xあい");
        assert_eq!(next_pos, ByteIndex(1));

        // 5. 範囲外（len超え） -> 末尾に挿入される
        let mut s = MichiuString::new("abc");
        let next_pos = s.insert_char(ByteIndex(999), 'd');
        assert_eq!(s.0.as_ref(), "abcd");
        assert_eq!(next_pos, ByteIndex(4));
    }

    #[test]
    fn test_insert_str() {
        // 空文字の挿入（何も変わらず、位置もそのまま）
        let mut s = MichiuString::new("test");
        let next_pos = s.insert_str(ByteIndex(2), "");
        assert_eq!(s.0.as_ref(), "test");
        assert_eq!(next_pos, ByteIndex(2));

        // まとめて文字列挿入 (マルチバイト混在)
        let mut s = MichiuString::new("Hello !");
        let next_pos = s.insert_str(ByteIndex(6), "Rust🦀");
        assert_eq!(s.0.as_ref(), "Hello Rust🦀!");
        // 6 + 4(Rust) + 4(🦀) = 14
        assert_eq!(next_pos, ByteIndex(14));

        // 空文字列に対して挿入
        let mut s = MichiuString::new("");
        let next_pos = s.insert_str(ByteIndex(0), "init");
        assert_eq!(s.0.as_ref(), "init");
        assert_eq!(next_pos, ByteIndex(4));
    }

    #[test]
    fn test_inserted_immutable() {
        let original = MichiuString::new("hello");
        let modified = original.inserted(ByteIndex(5), " world");

        // 元の文字列が変更されていないこと
        assert_eq!(original.0.as_ref(), "hello");
        // 新しい文字列が正しく挿入されていること
        assert_eq!(modified.0.as_ref(), "hello world");
    }

    #[test]
    fn test_consecutive_typing_simulation() {
        // キーボードで文字を連続入力するシミュレーションテスト
        // insert_char の戻り値をそのまま次の at に渡していく
        let mut s = MichiuString::new("");
        let mut cursor = ByteIndex(0);

        for ch in "Rust言語🦀".chars() {
            cursor = s.insert_char(cursor, ch);
        }

        assert_eq!(s.0.as_ref(), "Rust言語🦀");
        assert_eq!(cursor, s.byte_len());
    }

    #[test]
    fn test_slice() {
        // "A"(1B) + "あ"(3B) + "🦀"(4B) + "B"(1B) = 9B
        let s = MichiuString::new("Aあ🦀B");

        // 正常なスライス
        assert_eq!(s.slice(ByteIndex(0)..ByteIndex(1)), "A");
        assert_eq!(s.slice(ByteIndex(1)..ByteIndex(4)), "あ");
        assert_eq!(s.slice(ByteIndex(4)..ByteIndex(8)), "🦀");
        assert_eq!(s.slice(ByteIndex(1)..ByteIndex(8)), "あ🦀");

        // マルチバイト文字の途中を指定しても安全に clamp される
        // ByteIndex(2) -> 1 ('あ'), ByteIndex(6) -> 4 ('🦀')
        assert_eq!(s.slice(ByteIndex(2)..ByteIndex(6)), "あ");

        // start == end は空文字
        assert_eq!(s.slice(ByteIndex(4)..ByteIndex(4)), "");

        // 逆転レンジ（start > end）はクラッシュせず空文字
        assert_eq!(s.slice(ByteIndex(4)..ByteIndex(1)), "");

        // 範囲外（len超え）も安全
        assert_eq!(s.slice(ByteIndex(8)..ByteIndex(100)), "B");
    }

    #[test]
    fn test_replace_range() {
        // 通常の置換（範囲を別の文字列に置き換え）
        let mut s = MichiuString::new("Hello World");
        // "World" (6..11) を "Rust🦀" に置換
        let next_pos = s.replace_range(ByteIndex(6)..ByteIndex(11), "Rust🦀");
        assert_eq!(s.0.as_ref(), "Hello Rust🦀");
        // 6 + 4(Rust) + 4(🦀) = 14
        assert_eq!(next_pos, ByteIndex(14));

        // 空の範囲（start == end）は単なる挿入になる
        let mut s = MichiuString::new("AC");
        let next_pos = s.replace_range(ByteIndex(1)..ByteIndex(1), "B");
        assert_eq!(s.0.as_ref(), "ABC");
        assert_eq!(next_pos, ByteIndex(2));

        // 逆転レンジ（start > end）の場合もパニックせず insert 挙動になる
        let mut s = MichiuString::new("AC");
        let next_pos = s.replace_range(ByteIndex(1)..ByteIndex(0), "B");
        assert_eq!(s.0.as_ref(), "ABC");
        assert_eq!(next_pos, ByteIndex(2));
    }

    #[test]
    fn test_remove_range() {
        let mut s = MichiuString::new("Hello Rust🦀 World");
        // " Rust🦀" (5..14) を削除
        let next_pos = s.remove_range(ByteIndex(5)..ByteIndex(14));
        assert_eq!(s.0.as_ref(), "Hello World");
        assert_eq!(next_pos, ByteIndex(5));

        // 逆転レンジは何も消えない
        let mut s = MichiuString::new("Hello");
        let next_pos = s.remove_range(ByteIndex(3)..ByteIndex(1));
        assert_eq!(s.0.as_ref(), "Hello");
        assert_eq!(next_pos, ByteIndex(3));
    }

    #[test]
    fn test_backspace() {
        // "A" (0..1), "あ" (1..4), "🦀" (4..8)
        let mut s = MichiuString::new("Aあ🦀");

        // 末尾(8)で backspace -> '🦀' (4B) が丸ごと消える
        let pos = s.backspace(ByteIndex(8));
        assert_eq!(s.0.as_ref(), "Aあ");
        assert_eq!(pos, ByteIndex(4));

        // 末尾(4)で backspace -> 'あ' (3B) が丸ごと消える
        let pos = s.backspace(ByteIndex(4));
        assert_eq!(s.0.as_ref(), "A");
        assert_eq!(pos, ByteIndex(1));

        // 末尾(1)で backspace -> 'A' (1B) が消えて空文字に
        let pos = s.backspace(ByteIndex(1));
        assert_eq!(s.0.as_ref(), "");
        assert_eq!(pos, ByteIndex(0));

        // 先頭(0)で backspace -> 何も起きず 0 を返す
        let pos = s.backspace(ByteIndex(0));
        assert_eq!(s.0.as_ref(), "");
        assert_eq!(pos, ByteIndex(0));
    }

    #[test]
    fn test_delete() {
        // "A" (0..1), "あ" (1..4), "🦀" (4..8)
        let mut s = MichiuString::new("Aあ🦀");

        // 先頭(0)で delete -> 直後の 'A' (1B) が消える（カーソルは 0 のまま）
        let pos = s.delete(ByteIndex(0));
        assert_eq!(s.0.as_ref(), "あ🦀");
        assert_eq!(pos, ByteIndex(0));

        // 先頭(0)で delete -> 直後の 'あ' (3B) が消える
        let pos = s.delete(ByteIndex(0));
        assert_eq!(s.0.as_ref(), "🦀");
        assert_eq!(pos, ByteIndex(0));

        // 先頭(0)で delete -> 直後の '🦀' (4B) が消える
        let pos = s.delete(ByteIndex(0));
        assert_eq!(s.0.as_ref(), "");
        assert_eq!(pos, ByteIndex(0));

        // 末尾で delete -> 何も起きずカーソル位置のまま
        let pos = s.delete(ByteIndex(0));
        assert_eq!(s.0.as_ref(), "");
        assert_eq!(pos, ByteIndex(0));
    }

    #[test]
    #[expect(unused_assignments)]
    fn test_typing_and_backspacing() {
        // タイプしては消す、リアルな入力シミュレーション
        let mut s = MichiuString::new("");
        let mut cursor = ByteIndex(0);

        // "猫🦀" と入力
        cursor = s.insert_char(cursor, '猫');
        cursor = s.insert_char(cursor, '🦀');
        assert_eq!(s.0.as_ref(), "猫🦀");

        // 1回バックスペース（🦀が消える）
        cursor = s.backspace(cursor);
        assert_eq!(s.0.as_ref(), "猫");
        assert_eq!(cursor, ByteIndex(3));

        // '犬' を入力
        cursor = s.insert_char(cursor, '犬');
        assert_eq!(s.0.as_ref(), "猫犬");
    }

    #[test]
    fn test_line_indices() {
        // 空文字: 0行目、総行数 1
        let s = MichiuString::new("");
        assert_eq!(s.line_indices(ByteIndex(0)), (0, 1));

        // 改行なし
        let s = MichiuString::new("Hello");
        assert_eq!(s.line_indices(ByteIndex(0)), (0, 1));
        assert_eq!(s.line_indices(ByteIndex(5)), (0, 1));
        assert_eq!(s.line_indices(ByteIndex(100)), (0, 1)); // 範囲外クランプ

        // 複数行: "abc\ndef\n"
        // [0..3]: "abc", 3: '\n', [4..7]: "def", 7: '\n'
        let s = MichiuString::new("abc\ndef\n");
        // 行 0 (abc)
        assert_eq!(s.line_indices(ByteIndex(0)), (0, 3));
        assert_eq!(s.line_indices(ByteIndex(3)), (0, 3)); // '\n' の直前

        // 行 1 (def)
        assert_eq!(s.line_indices(ByteIndex(4)), (1, 3)); // '\n' の直後
        assert_eq!(s.line_indices(ByteIndex(7)), (1, 3)); // 2つ目の '\n' の直前

        // 行 2 (末尾の空行)
        assert_eq!(s.line_indices(ByteIndex(8)), (2, 3));
    }

    #[test]
    fn test_char_class_at() {
        // 各クラスの代表文字を詰め込んだ文字列
        // 0: 空白 (' ', '\t', '\n', '\r')
        // 1: 平仮名 ('あ')
        // 2: 片仮名 ('ア')
        // 3: 漢字   ('漢')
        // 4: 英数+_ ('a', 'Z', '0', '_')
        // 5: その他 ('!', '🦀')
        let s = MichiuString::new(" \t\nあア漢a_!🦀");

        assert_eq!(s.char_class_at(ByteIndex(0)), CharClassID(0)); // ' '
        assert_eq!(s.char_class_at(ByteIndex(1)), CharClassID(0)); // '\t'
        assert_eq!(s.char_class_at(ByteIndex(2)), CharClassID(0)); // '\n'

        // 3: 'あ' (3B)
        assert_eq!(s.char_class_at(ByteIndex(3)), CharClassID(1));
        // 6: 'ア' (3B)
        assert_eq!(s.char_class_at(ByteIndex(6)), CharClassID(2));
        // 9: '漢' (3B)
        assert_eq!(s.char_class_at(ByteIndex(9)), CharClassID(3));

        // 12: 'a' (1B), 13: '_' (1B)
        assert_eq!(s.char_class_at(ByteIndex(12)), CharClassID(4));
        assert_eq!(s.char_class_at(ByteIndex(13)), CharClassID(4));

        // 14: '!' (1B), 15: '🦀' (4B)
        assert_eq!(s.char_class_at(ByteIndex(14)), CharClassID(5));
        assert_eq!(s.char_class_at(ByteIndex(15)), CharClassID(5));

        // 末尾はフォールバックの ' ' により Class 0
        assert_eq!(s.char_class_at(ByteIndex(19)), CharClassID(0));
    }

    #[test]
    fn test_find_word_boundaries_basic() {
        // 空文字列
        let empty = MichiuString::new("");
        assert_eq!(
            empty.find_word_boundaries(ByteIndex(0)),
            ByteIndex(0)..ByteIndex(0)
        );

        // "hello world"
        // [0..5]: "hello", [5..6]: " ", [6..11]: "world"
        let s = MichiuString::new("hello world");

        // "hello" のどこをクリックしても 0..5 が取れる
        assert_eq!(
            s.find_word_boundaries(ByteIndex(0)),
            ByteIndex(0)..ByteIndex(5)
        );
        assert_eq!(
            s.find_word_boundaries(ByteIndex(2)),
            ByteIndex(0)..ByteIndex(5)
        );
        assert_eq!(
            s.find_word_boundaries(ByteIndex(4)),
            ByteIndex(0)..ByteIndex(5)
        );

        // 空白をクリックすると空白の範囲が取れる
        assert_eq!(
            s.find_word_boundaries(ByteIndex(5)),
            ByteIndex(5)..ByteIndex(6)
        );

        // "world" のどこをクリックしても 6..11 が取れる
        assert_eq!(
            s.find_word_boundaries(ByteIndex(6)),
            ByteIndex(6)..ByteIndex(11)
        );
        assert_eq!(
            s.find_word_boundaries(ByteIndex(10)),
            ByteIndex(6)..ByteIndex(11)
        );

        // 文字列末尾(11)をクリック -> 直前の "world" が選択される
        assert_eq!(
            s.find_word_boundaries(ByteIndex(11)),
            ByteIndex(6)..ByteIndex(11)
        );
    }

    #[test]
    fn test_find_word_boundaries_japanese_tokenization() {
        // 日本語の平仮名・片仮名・漢字の境界認識テスト
        // "東京都"(漢字: 9B) + "の"(平仮名: 3B) + "ネコ"(片仮名: 6B)
        // [0..9]: "東京都"
        // [9..12]: "の"
        // [12..18]: "ネコ"
        let s = MichiuString::new("東京都のネコ");

        // 漢字部分 ("東京都") のどこをクリックしても 0..9
        assert_eq!(
            s.find_word_boundaries(ByteIndex(0)),
            ByteIndex(0)..ByteIndex(9)
        ); // '東'
        assert_eq!(
            s.find_word_boundaries(ByteIndex(3)),
            ByteIndex(0)..ByteIndex(9)
        ); // '京'
        assert_eq!(
            s.find_word_boundaries(ByteIndex(6)),
            ByteIndex(0)..ByteIndex(9)
        ); // '都'

        // 平仮名部分 ("の") -> 9..12
        assert_eq!(
            s.find_word_boundaries(ByteIndex(9)),
            ByteIndex(9)..ByteIndex(12)
        );

        // 片仮名部分 ("ネコ") のどこをクリックしても 12..18
        assert_eq!(
            s.find_word_boundaries(ByteIndex(12)),
            ByteIndex(12)..ByteIndex(18)
        ); // 'ネ'
        assert_eq!(
            s.find_word_boundaries(ByteIndex(15)),
            ByteIndex(12)..ByteIndex(18)
        ); // 'コ'
    }

    #[test]
    fn test_find_word_boundaries_identifier() {
        // アルファベット + 数字 + アンダースコア が一塊になるか
        let s = MichiuString::new("let mut my_var_123 = 42;");
        // "my_var_123" は ByteIndex 8..18
        assert_eq!(
            s.find_word_boundaries(ByteIndex(8)),
            ByteIndex(8)..ByteIndex(18)
        );
        assert_eq!(
            s.find_word_boundaries(ByteIndex(12)),
            ByteIndex(8)..ByteIndex(18)
        );
    }

    // てんこ盛り字列
    const CHAOS_TEXT: &str = "Hello_123 ひらがな カタカナ 漢字 中文\n\
             한국어 مرحبا བོད་སྐད་ 🦀🚀✨ !@#\t\r\n\
             EOF_TOKEN";

    #[test]
    fn test_multilingual_all_byte_offsets_must_not_panic() {
        // 0 から len までのすべてのバイト位置を突く全探索テスト
        // マルチバイト文字の途中、EOFが絶対にパニックせず
        // 有効な文字境界に丸められることを証明する。
        let s = MichiuString::new(CHAOS_TEXT);
        let len = s.byte_len().0;

        for b in 0..=len {
            let at = ByteIndex(b);

            // clamp がパニックせず、かつ結果は必ずUTF-8の文字境界であること
            let clamped = s.clamp_to_boundary(at);
            assert!(
                s.0.is_char_boundary(clamped.0),
                "Clamped offset {clamped:?} at byte {b} is NOT a char boundary!"
            );
            assert!(clamped.0 <= len);

            // to_char_index がパニックしないこと
            let _char_idx = s.to_char_index(at);

            // char_class_at がパニックしないこと
            let _class = s.char_class_at(at);

            // find_word_boundaries がパニックせず、返された Range の両端が char boundary であること
            let word_range = s.find_word_boundaries(at);
            assert!(s.0.is_char_boundary(word_range.start.0));
            assert!(s.0.is_char_boundary(word_range.end.0));
            assert!(word_range.start <= word_range.end);
            assert!(word_range.end.0 <= len);

            // slice しても絶対にパニックしないこと
            let _sliced = s.slice(word_range);
        }
    }

    #[test]
    fn test_multilingual_roundtrip_navigation() {
        // 先頭から末尾まで1文字ずつ歩き、末尾から先頭まで戻ってくる往復走査
        let s = MichiuString::new(CHAOS_TEXT);
        let len = s.byte_len();

        let mut positions = vec![ByteIndex(0)];
        let mut cur = ByteIndex(0);

        // 前進
        while cur < len {
            let next = s.next_char_boundary(cur);
            assert!(next > cur, "Cursor failed to advance at {cur:?}");
            assert!(s.0.is_char_boundary(next.0));
            cur = next;
            positions.push(cur);
        }
        assert_eq!(cur, len);

        // 後退（前進の履歴と完全逆順で一致するはず）
        for &expected in positions.iter().rev() {
            assert_eq!(cur, expected);
            if cur > ByteIndex(0) {
                cur = s.prev_char_boundary(cur);
            }
        }
        assert_eq!(cur, ByteIndex(0));
    }

    #[test]
    fn test_multilingual_insert_and_backspace_integrity() {
        // 多言語が混ざった文章のあらゆる境界に文字を挿入し、
        // 直後に backspace したら完全に元の文字列に復元できるかテスト
        let s = MichiuString::new(CHAOS_TEXT);
        let mut cur = ByteIndex(0);

        while cur <= s.byte_len() {
            let mut test_s = s.clone();

            // 絵文字（4バイト）を挿入
            let next_cursor = test_s.insert_char(cur, '🦀');
            assert_eq!(next_cursor, ByteIndex(cur.0 + 4));

            // 直後に backspace
            let restored_cursor = test_s.backspace(next_cursor);
            assert_eq!(restored_cursor, cur);

            // 完全に元通りか
            assert_eq!(test_s.0.as_ref(), CHAOS_TEXT);

            if cur == s.byte_len() {
                break;
            }
            cur = s.next_char_boundary(cur);
        }
    }

    #[test]
    fn test_multilingual_word_boundaries_segmentation() {
        // 言語ごとの単語境界の判定チェック
        let s = MichiuString::new("Rust 🦀 漢字 日本語 한국어 مرحبا");

        // 英単語 "Rust"
        let r1 = s.find_word_boundaries(ByteIndex(0));
        assert_eq!(s.slice(r1), "Rust");

        // 絵文字 "🦀"（記号/その他クラス）
        let crab_pos = s.to_byte_index(CharIndex(5));
        let r2 = s.find_word_boundaries(crab_pos);
        assert_eq!(s.slice(r2), "🦀");

        // 漢字 "漢字"
        let kanji_pos = s.to_byte_index(CharIndex(7));
        let r3 = s.find_word_boundaries(kanji_pos);
        assert_eq!(s.slice(r3), "漢字");

        // アラビア語 "مرحبا"（末尾の単語）
        let arab_pos = s.byte_len(); // 末尾 EOF をクリック
        let r4 = s.find_word_boundaries(arab_pos);
        assert_eq!(s.slice(r4), "مرحبا");
    }

    #[test]
    fn test_nul_byte_containment() {
        // NUL文字 ('\0') を途中に含む文字列
        // [0..5]: "hello", 5: '\0', [6..11]: "world"
        let mut s = MichiuString::new("hello\0world");

        assert_eq!(s.byte_len(), ByteIndex(11));
        assert_eq!(s.char_count(), CharIndex(11));

        // NUL文字自体のクラス判定（Class 5: その他）
        assert_eq!(s.char_class_at(ByteIndex(5)), CharClassID(5));

        // NUL文字を backspace で安全に消せるか
        let pos = s.backspace(ByteIndex(6));
        assert_eq!(s.0.as_ref(), "helloworld");
        assert_eq!(pos, ByteIndex(5));
    }

    #[test]
    fn test_zalgo_combining_characters() {
        // 'Z' に結合アクセント記号が3つくっついた文字 (Z + \u{0300} + \u{0301} + \u{0302})
        let zalgo = "Z\u{0300}\u{0301}\u{0302}";
        let s = MichiuString::new(zalgo);

        // 各アクセント記号は2バイト
        assert_eq!(s.byte_len(), ByteIndex(1 + 2 * 3)); // 7バイト
        assert_eq!(s.char_count(), CharIndex(4));

        // すべてのバイト位置でクラッシュしないこと
        for b in 0..=s.byte_len().0 {
            let clamped = s.clamp_to_boundary(ByteIndex(b));
            assert!(s.0.is_char_boundary(clamped.0));
        }

        // 後ろから 1 スカラー値ずつ確実に消していけるか
        let mut cur = s.byte_len();
        let mut editable = s.clone();
        for _ in 0..4 {
            cur = editable.backspace(cur);
        }
        assert_eq!(editable.0.as_ref(), "");
    }

    #[test]
    fn test_zwj_emoji_sequence() {
        // 👩‍💻 = 👩 (4B) + ZWJ (3B) + 💻 (4B) = 計11バイト
        let tech_woman = "👩\u{200D}💻";
        let mut s = MichiuString::new(tech_woman);

        assert_eq!(s.byte_len(), ByteIndex(11));
        assert_eq!(s.char_count(), CharIndex(3));

        // 1回 backspace -> '💻' (4B) が消えて 👩 + ZWJ (7B) になる
        let next_pos = s.backspace(ByteIndex(11));
        assert_eq!(next_pos, ByteIndex(7));
        assert_eq!(s.0.as_ref(), "👩\u{200D}");

        // もう1回 backspace -> ZWJ (3B) が消えて '👩' (4B) になる
        let next_pos = s.backspace(next_pos);
        assert_eq!(next_pos, ByteIndex(4));
        assert_eq!(s.0.as_ref(), "👩");
    }

    #[test]
    #[expect(unused_must_use)]
    fn test_crlf_midpoint() {
        // Windows 改行 "\r\n" の間（ByteIndex 5）にカーソルを置く
        let s = MichiuString::new("line1\r\nline2");

        // line_indices の挙動: \r と \n の間は 0行目判定
        let (current_line, total) = s.line_indices(ByteIndex(6));
        assert_eq!(current_line, 0); // \n をまだ通過していないので 0行目
        assert_eq!(total, 2);

        // \r と \n の間に 'X' を挿入しても文字化け・クラッシュしないか
        let mut editable = s.clone();
        editable.insert_char(ByteIndex(6), 'X');
        assert_eq!(editable.0.as_ref(), "line1\rX\nline2");
    }

    #[test]
    fn test_extreme_indices_overflow_resilience() {
        let s = MichiuString::new("Hello");

        // usize::MAX などの極端な巨大インデックスを渡してもオーバーフローせず末尾にクランプされるか
        let extreme = ByteIndex(usize::MAX);
        assert_eq!(s.clamp_to_boundary(extreme), ByteIndex(5));
        assert_eq!(s.next_char_boundary(extreme), ByteIndex(5));
        assert_eq!(s.prev_char_boundary(extreme), ByteIndex(4)); // 'o' の位置
        assert_eq!(s.to_char_index(extreme), CharIndex(5));

        // 範囲外スライス
        assert_eq!(s.slice(ByteIndex(100)..extreme), "");

        // 巨大な範囲指定での削除
        let mut editable = s.clone();
        let caret = editable.remove_range(ByteIndex(3)..extreme);
        assert_eq!(editable.0.as_ref(), "Hel");
        assert_eq!(caret, ByteIndex(3));
    }
}
