pub mod animation;
pub mod callback;
pub mod color;
pub mod cursor;
pub mod font;
pub mod from_into;
pub mod geometry;
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
}

#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub enum AlignItems {
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
}

#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub enum JustifyContent {
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

#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub enum GridAutoFlow {
    #[default]
    Row,
    Column,
    RowDense,
    ColumnDense,
}

#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub enum GridPlacement<S>
where
    S: ToString,
{
    #[default]
    Auto,
    Line(S),
    NamedLine(S),
    Span(S),
    NamedSpan(S),
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct GridLine<T> {
    pub start: T,
    pub end: T,
}

#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Default, bytemuck::Pod, bytemuck::Zeroable)]
pub struct LinearGradient {
    pub start_color: Color,
    pub end_color: Color,
    pub angle: f32,                // ラジアン単位の角度
    pub(crate) _padding: [f32; 3], // アライメント
}

impl LinearGradient {
    #[inline]
    #[must_use]
    pub fn new(start_color: Color, end_color: Color, angle_degrees: f32) -> Self {
        Self {
            start_color,
            end_color,
            angle: angle_degrees.to_radians(),
            _padding: [0.0; 3],
        }
    }
}

/// Not Implemented
#[derive(Debug, Clone, PartialEq)]
pub enum UiaValue {
    String(String),
    Bool(bool),
    Int(i32),
    Double(f64),
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

#[cfg(test)]
mod tests;
