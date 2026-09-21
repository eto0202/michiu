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
