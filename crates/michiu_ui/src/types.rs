pub mod a11y;
pub mod animation;
pub mod callback;
pub mod color;
pub mod cursor;
pub mod font;
pub mod from_into;
pub mod geometry;
pub mod gradient;
pub mod index;
pub mod input;
pub mod interaction;
pub mod key;
pub mod layout_data;
pub mod shadow;
pub mod string;
pub mod transform;

pub use animation::*;
pub use callback::*;
pub use color::*;
pub use cursor::*;
pub use font::*;
pub use from_into::*;
pub use geometry::*;
pub use gradient::*;
pub use index::*;
pub use input::*;
pub use interaction::*;
pub use key::*;
pub use layout_data::*;
pub use shadow::*;
pub use string::*;
pub use transform::*;

#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub enum Position {
    #[default]
    Relative,
    Absolute,
    Static,
    Fixed,
    Sticky,
}

#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub enum BoxSizing {
    #[default]
    BorderBox,
    ContentBox,
}

#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub enum Direction {
    #[default]
    Ltr,
    Rtl,
}

#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub enum Overflow {
    #[default]
    Visible,
    Hidden,
    Scroll,
    Clip,
}

#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct LayoutOverflow {
    pub x: Overflow,
    pub y: Overflow,
}

#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub enum FlexDirection {
    #[default]
    Row,
    Column,
    RowReverse,
    ColumnReverse,
}

impl FlexDirection {
    #[inline]
    pub(crate) fn is_row(self) -> bool {
        self == FlexDirection::Row || self == FlexDirection::RowReverse
    }

    #[allow(unused)]
    #[inline]
    pub(crate) fn is_col(self) -> bool {
        self == FlexDirection::Column || self == FlexDirection::ColumnReverse
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub enum FlexWrap {
    #[default]
    NoWrap,
    Wrap,
    WrapReverse,
    Balance,
    BalanceReverse,
}

#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub enum AlignItems {
    Normal,
    Start,
    End,
    FlexStart,
    FlexEnd,
    Center,
    Baseline,
    #[default]
    Stretch,
    SafeStart,
    SafeEnd,
    SafeFlexStart,
    SafeFlexEnd,
    SafeCenter,
    SelfStart,
    SelfEnd,
}

#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub enum AlignSelf {
    Start,
    End,
    FlexStart,
    FlexEnd,
    Center,
    Baseline,
    #[default]
    Stretch,
    SafeStart,
    SafeEnd,
    SafeFlexStart,
    SafeFlexEnd,
    SafeCenter,
    SelfStart,
    SelfEnd,
}

#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub enum JustifyContent {
    Normal,
    Start,
    End,
    Center,
    #[default]
    Stretch,
    SpaceBetween,
    SpaceAround,
    SpaceEvenly,
    FlexStart,
    FlexEnd,
    SafeStart,
    SafeEnd,
    SafeFlexStart,
    SafeFlexEnd,
    SafeCenter,
}

#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub enum AlignContent {
    Normal,
    Start,
    End,
    Center,
    #[default]
    Stretch,
    SpaceBetween,
    SpaceAround,
    SpaceEvenly,
    FlexStart,
    FlexEnd,
    SafeStart,
    SafeEnd,
    SafeFlexStart,
    SafeFlexEnd,
    SafeCenter,
}

#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub enum TextAlign {
    #[default]
    Auto,
    Left,
    Right,
    Center,
}

/// Windows 11's Native System Background Blur Effect
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
#[repr(i32)]
pub enum Backdrop {
    #[default]
    None = 0,
    Mica = 2,
    Acrylic = 3,
    MicaAlt = 4,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
#[repr(u32)]
pub enum BorderStyle {
    #[default]
    Solid = 0,
    Dotted = 1,
    Dashed = 2,
    Double = 3,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
#[repr(u32)]
pub enum BorderAlignment {
    #[default]
    Start = 0, // 左・上が基準 (左から右、上から下へ伸びる)
    End = 1,    // 右・下が基準 (右から左、下から上へ伸びる)
    Center = 2, // 中心が基準 (中心から両方向へ対称に広がる)
}

/// Controls whether to skip the layout and rendering of an element and its descendant subtree.
///
/// This is equivalent to the CSS `content-visibility` property and speeds up the rendering of large lists and tables by skipping the calculation of off-screen or hidden subtrees in one go.
///
/// # Note
/// When specifying `Auto` or `Hidden`, it is recommended to set an explicit `size` (width or height) on the element
/// to prevent layout breaks caused by the height becoming 0px when the element is off-screen.
///
/// If there is a brief rendering delay when scrolling quickly while using `Auto`,
/// set the window's visibility margin ([`set_visibility_margin`](crate::Context::set_visibility_margin) /
/// [`with_visibility_margin`](crate::Context::with_visibility_margin)),
/// so that rendering begins in advance, starting from just before the content enters the viewport (Default: 200px top and bottom).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ContentVisibility {
    /// Skips layout calculations and rendering for descendants while the element is off-screen (outside the viewport).
    ///
    /// The element is automatically made visible when it enters the viewport.
    /// It remains in the accessibility tree (e.g., for screen readers) even when off-screen.
    ///
    /// If you want to pre-render content just before it enters the screen, set the window's `visibility_margin`.
    Auto,

    /// Always layout and renders child elements as usual (default).
    #[default]
    Visible,

    /// Completely skips the layout and rendering of descendants, whether they are on-screen or off-screen. Descendants are also excluded from the accessibility tree.
    Hidden,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_is_row() {
        assert!(FlexDirection::Row.is_row());
        assert!(FlexDirection::RowReverse.is_row());
        assert!(!FlexDirection::Column.is_row());
        assert!(!FlexDirection::ColumnReverse.is_row());
    }

    #[test]
    fn test_is_col() {
        assert!(FlexDirection::Column.is_col());
        assert!(FlexDirection::ColumnReverse.is_col());
        assert!(!FlexDirection::Row.is_col());
        assert!(!FlexDirection::RowReverse.is_col());
    }

    #[test]
    fn test_row_and_col_are_mutually_exclusive() {
        let directions = [
            FlexDirection::Row,
            FlexDirection::RowReverse,
            FlexDirection::Column,
            FlexDirection::ColumnReverse,
        ];

        for dir in directions {
            assert_ne!(
                dir.is_row(),
                dir.is_col(),
                "{dir:?} should not be both or neither"
            );
        }
    }
}
