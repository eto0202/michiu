use std::{
    fmt,
    ops::{Add, AddAssign, Deref, Range, Sub},
};

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Default, Hash)]
pub struct ByteIndex(pub usize);

// ======================================================================
//
// ======================================================================

impl Deref for ByteIndex {
    type Target = usize;
    #[inline]
    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl From<usize> for ByteIndex {
    #[inline]
    fn from(v: usize) -> Self {
        Self(v)
    }
}

impl From<ByteIndex> for usize {
    #[inline]
    fn from(v: ByteIndex) -> Self {
        v.0
    }
}

impl fmt::Display for ByteIndex {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

// ByteIndex + usize
impl Add<usize> for ByteIndex {
    type Output = Self;
    #[inline]
    fn add(self, rhs: usize) -> Self::Output {
        Self(self.0 + rhs)
    }
}

// ByteIndex + ByteIndex
impl Add<ByteIndex> for ByteIndex {
    type Output = Self;
    #[inline]
    fn add(self, rhs: ByteIndex) -> Self::Output {
        Self(self.0 + rhs.0)
    }
}

impl AddAssign<usize> for ByteIndex {
    #[inline]
    fn add_assign(&mut self, rhs: usize) {
        self.0 += rhs;
    }
}

// ByteIndex - usize
impl Sub<usize> for ByteIndex {
    type Output = Self;
    #[inline]
    fn sub(self, rhs: usize) -> Self::Output {
        Self(self.0.saturating_sub(rhs))
    }
}

// ByteIndex - ByteIndex
impl Sub<ByteIndex> for ByteIndex {
    type Output = usize;
    #[inline]
    fn sub(self, rhs: ByteIndex) -> Self::Output {
        self.0.saturating_sub(rhs.0)
    }
}

// ======================================================================
//
// ======================================================================

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Default, Hash)]
pub struct CharIndex(pub usize);

// ======================================================================
//
// ======================================================================

impl Deref for CharIndex {
    type Target = usize;
    #[inline]
    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl From<usize> for CharIndex {
    #[inline]
    fn from(v: usize) -> Self {
        Self(v)
    }
}

impl From<CharIndex> for usize {
    #[inline]
    fn from(v: CharIndex) -> Self {
        v.0
    }
}

impl fmt::Display for CharIndex {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl Add<usize> for CharIndex {
    type Output = Self;
    #[inline]
    fn add(self, rhs: usize) -> Self::Output {
        Self(self.0 + rhs)
    }
}

impl AddAssign<usize> for CharIndex {
    #[inline]
    fn add_assign(&mut self, rhs: usize) {
        self.0 += rhs;
    }
}

impl Sub<usize> for CharIndex {
    type Output = Self;
    #[inline]
    fn sub(self, rhs: usize) -> Self::Output {
        Self(self.0.saturating_sub(rhs))
    }
}

impl Sub<CharIndex> for CharIndex {
    type Output = usize;
    #[inline]
    fn sub(self, rhs: CharIndex) -> Self::Output {
        self.0.saturating_sub(rhs.0)
    }
}

// ======================================================================
//
// ======================================================================

/// A trait that provides conversion between `Range<usize>` and the dedicated `Range` type
pub trait RangeExt {
    fn to_usize_range(self) -> Range<usize>;
}

// ======================================================================
//
// ======================================================================

impl RangeExt for Range<usize> {
    #[inline]
    fn to_usize_range(self) -> Range<usize> {
        self
    }
}

impl RangeExt for Range<ByteIndex> {
    #[inline]
    fn to_usize_range(self) -> Range<usize> {
        self.start.0..self.end.0
    }
}

impl RangeExt for Range<CharIndex> {
    #[inline]
    fn to_usize_range(self) -> Range<usize> {
        self.start.0..self.end.0
    }
}

// ======================================================================
//
// ======================================================================

// Range<usize> 側から、型を明示して変換するための拡張
pub trait UsizeRangeExt {
    fn to_byte_range(self) -> Range<ByteIndex>;
    fn to_char_range(self) -> Range<CharIndex>;
}

// ======================================================================
//
// ======================================================================

impl UsizeRangeExt for Range<usize> {
    #[inline]
    fn to_byte_range(self) -> Range<ByteIndex> {
        ByteIndex(self.start)..ByteIndex(self.end)
    }

    #[inline]
    fn to_char_range(self) -> Range<CharIndex> {
        CharIndex(self.start)..CharIndex(self.end)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::ops::Range;

    #[test]
    fn test_deref_and_conversions() {
        let idx = ByteIndex::from(42);

        // From / Into
        assert_eq!(idx, ByteIndex(42));
        assert_eq!(usize::from(idx), 42);
        let as_usize: usize = idx.into();
        assert_eq!(as_usize, 42);

        // Deref
        assert_eq!(*idx, 42);
    }

    #[test]
    fn test_display() {
        let idx = ByteIndex(128);
        assert_eq!(format!("{idx}"), "128");
    }

    #[test]
    fn test_add() {
        let a = ByteIndex(10);

        // ByteIndex + usize -> ByteIndex
        assert_eq!(a + 5, ByteIndex(15));

        // ByteIndex + ByteIndex -> ByteIndex
        let b = ByteIndex(20);
        assert_eq!(a + b, ByteIndex(30));

        // AddAssign
        let mut c = ByteIndex(10);
        c += 5;
        assert_eq!(c, ByteIndex(15));
    }

    #[test]
    fn test_sub_with_saturating() {
        let a = ByteIndex(10);

        // ByteIndex - usize -> ByteIndex
        assert_eq!(a - 4, ByteIndex(6));
        // アンダーフロー時に 0 で飽和すること
        assert_eq!(a - 20, ByteIndex(0));

        // ByteIndex - ByteIndex -> usize
        let b = ByteIndex(4);
        let diff: usize = a - b; // 戻り値の型が usize であることの確認
        assert_eq!(diff, 6);

        // アンダーフロー時に 0 で飽和すること
        let c = ByteIndex(20);
        assert_eq!(a - c, 0usize);
    }

    #[test]
    fn test_ordering_and_defaults() {
        // Default
        assert_eq!(ByteIndex::default(), ByteIndex(0));

        // Ord / PartialOrd
        assert!(ByteIndex(5) < ByteIndex(10));
        assert!(ByteIndex(10) > ByteIndex(5));
        assert_eq!(ByteIndex(10), ByteIndex(10));
    }

    #[test]
    fn test_char_index_deref_and_conversions() {
        let idx = CharIndex::from(42);

        // From / Into
        assert_eq!(idx, CharIndex(42));
        assert_eq!(usize::from(idx), 42);
        let as_usize: usize = idx.into();
        assert_eq!(as_usize, 42);

        // Deref
        assert_eq!(*idx, 42);
    }

    #[test]
    fn test_char_index_display() {
        let idx = CharIndex(7);
        assert_eq!(format!("{idx}"), "7");
    }

    #[test]
    fn test_char_index_add() {
        let a = CharIndex(10);

        // CharIndex + usize -> CharIndex
        assert_eq!(a + 5, CharIndex(15));

        // AddAssign
        let mut c = CharIndex(10);
        c += 5;
        assert_eq!(c, CharIndex(15));
    }

    #[test]
    fn test_char_index_sub_with_saturating() {
        let a = CharIndex(10);

        // CharIndex - usize -> CharIndex
        assert_eq!(a - 4, CharIndex(6));
        // アンダーフロー時の飽和
        assert_eq!(a - 20, CharIndex(0));

        // CharIndex - CharIndex -> usize (インデックス間の距離)
        let b = CharIndex(4);
        let diff: usize = a - b;
        assert_eq!(diff, 6);

        // アンダーフロー時の飽和
        let c = CharIndex(20);
        assert_eq!(a - c, 0usize);
    }

    #[test]
    fn test_char_index_ordering_and_defaults() {
        // Default
        assert_eq!(CharIndex::default(), CharIndex(0));

        // Ord / PartialOrd
        assert!(CharIndex(3) < CharIndex(5));
        assert!(CharIndex(5) > CharIndex(3));
        assert_eq!(CharIndex(5), CharIndex(5));
    }

    #[test]
    fn test_range_ext_to_usize_range() {
        // Range<usize> -> Range<usize>
        let usize_range: Range<usize> = 2..8;
        assert_eq!(usize_range.to_usize_range(), 2..8);

        // Range<ByteIndex> -> Range<usize>
        let byte_range = ByteIndex(5)..ByteIndex(15);
        assert_eq!(byte_range.to_usize_range(), 5..15);

        // Range<CharIndex> -> Range<usize>
        let char_range = CharIndex(0)..CharIndex(42);
        assert_eq!(char_range.to_usize_range(), 0..42);

        // 空の範囲 (0..0, 5..5)
        let empty_byte = ByteIndex(3)..ByteIndex(3);
        assert_eq!(empty_byte.to_usize_range(), 3..3);
    }

    #[test]
    fn test_usize_range_ext() {
        let r = 10..20;

        // to_byte_range
        assert_eq!(r.clone().to_byte_range(), ByteIndex(10)..ByteIndex(20));

        // to_char_range
        assert_eq!(r.clone().to_char_range(), CharIndex(10)..CharIndex(20));
    }

    #[test]
    fn test_range_conversions_roundtrip() {
        // usize -> ByteIndex -> usize
        let orig_usize = 4..12;
        assert_eq!(
            orig_usize.clone().to_byte_range().to_usize_range(),
            orig_usize
        );

        // usize -> CharIndex -> usize
        assert_eq!(
            orig_usize.clone().to_char_range().to_usize_range(),
            orig_usize
        );

        // ByteIndex -> usize -> ByteIndex
        let orig_byte = ByteIndex(7)..ByteIndex(18);
        assert_eq!(
            orig_byte.clone().to_usize_range().to_byte_range(),
            orig_byte
        );

        // CharIndex -> usize -> CharIndex
        let orig_char = CharIndex(1)..CharIndex(99);
        assert_eq!(
            orig_char.clone().to_usize_range().to_char_range(),
            orig_char
        );
    }
}
