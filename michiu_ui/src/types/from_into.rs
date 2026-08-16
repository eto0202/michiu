#![allow(clippy::cast_precision_loss)]

use std::{borrow::Cow, path::PathBuf};

use crate::{
    AlignContent, AlignItems, AlignSelf, Auto, BoxSizing, CornerRadius, Direction, Display,
    Element, FlexDirection, FlexWrap, FocusTrigger, Focusable, GridAutoFlow, GridLine,
    GridPlacement, ImageSource, InputContents, JustifyContent, LayoutOverflow, LayoutPoint, Length,
    LinearGradient, MovieProperty, MovieSource, Overflow, Percent, Pixel, Point, Position, Prop,
    ReadSignal, Rect, Size, StyleValue, TextAlign, ThisStyle, Transform, UiaValue, Val,
    WebView2Contents,
};

impl<T, U> From<Size<T>> for taffy::Size<U>
where
    U: From<T>,
{
    #[inline]
    fn from(size: Size<T>) -> Self {
        Self {
            width: U::from(size.width),
            height: U::from(size.height),
        }
    }
}

impl<T, U> From<taffy::Size<T>> for Size<U>
where
    U: From<T>,
{
    #[inline]
    fn from(size: taffy::Size<T>) -> Self {
        Self {
            width: U::from(size.width),
            height: U::from(size.height),
        }
    }
}

impl<T, U> From<Rect<T>> for taffy::Rect<U>
where
    U: From<T>,
{
    #[inline]
    fn from(rect: Rect<T>) -> Self {
        Self {
            top: U::from(rect.top),
            right: U::from(rect.right),
            bottom: U::from(rect.bottom),
            left: U::from(rect.left),
        }
    }
}

impl<T, U> From<taffy::Rect<T>> for Rect<U>
where
    U: From<T>,
{
    #[inline]
    fn from(rect: taffy::Rect<T>) -> Self {
        Self {
            top: U::from(rect.top),
            right: U::from(rect.right),
            bottom: U::from(rect.bottom),
            left: U::from(rect.left),
        }
    }
}

impl<T, U> From<Point<T>> for taffy::Point<U>
where
    U: From<T>,
{
    #[inline]
    fn from(point: Point<T>) -> Self {
        Self {
            x: U::from(point.x),
            y: U::from(point.y),
        }
    }
}

impl<T, U> From<taffy::Point<T>> for Point<U>
where
    U: From<T>,
{
    #[inline]
    fn from(point: taffy::Point<T>) -> Self {
        Self {
            x: U::from(point.x),
            y: U::from(point.y),
        }
    }
}

impl<T, U> From<GridLine<T>> for taffy::Line<U>
where
    U: From<T>,
{
    #[inline]
    fn from(line: GridLine<T>) -> Self {
        Self {
            start: U::from(line.start),
            end: U::from(line.end),
        }
    }
}

impl<T, U> From<taffy::Line<T>> for GridLine<U>
where
    U: From<T>,
{
    #[inline]
    fn from(line: taffy::Line<T>) -> Self {
        Self {
            start: U::from(line.start),
            end: U::from(line.end),
        }
    }
}

impl From<Length> for f32 {
    fn from(length: Length) -> Self {
        match length {
            Length::Px(val) | Length::Percent(val) => val,
        }
    }
}

impl From<f32> for Length {
    #[inline]
    fn from(val: f32) -> Self {
        Self::Px(val)
    }
}

impl From<Length> for taffy::LengthPercentage {
    #[inline]
    fn from(len: Length) -> Self {
        match len {
            Length::Px(val) => Self::length(val),
            Length::Percent(val) => Self::percent(val / 100.0),
        }
    }
}

impl From<taffy::LengthPercentage> for Length {
    #[inline]
    fn from(t: taffy::style::LengthPercentage) -> Self {
        let raw = t.into_raw();
        match raw.tag() {
            2 => Self::Percent(raw.value() * 100.0),
            _ => Self::Px(raw.value()), // calc等未対応のものはPxにフォールバック
        }
    }
}

impl From<f32> for Val {
    #[inline]
    fn from(val: f32) -> Self {
        Self::Px(val)
    }
}

impl From<Val> for taffy::Dimension {
    #[inline]
    fn from(val: Val) -> Self {
        match val {
            Val::Auto => Self::auto(),
            Val::Px(v) => Self::length(v),
            Val::Percent(v) => Self::percent(v / 100.0),
        }
    }
}

impl From<taffy::Dimension> for Val {
    #[inline]
    fn from(t: taffy::Dimension) -> Self {
        let raw = t.into_raw();
        if raw.is_auto() {
            Self::Auto
        } else {
            match raw.tag() {
                2 => Self::Percent(raw.value() * 100.0),
                _ => Self::Px(raw.value()),
            }
        }
    }
}

impl From<Val> for taffy::LengthPercentage {
    #[inline]
    fn from(val: Val) -> Self {
        match val {
            Val::Auto => Self::length(0.0),
            Val::Px(v) => Self::length(v),
            Val::Percent(v) => Self::percent(v / 100.0),
        }
    }
}

impl From<taffy::LengthPercentage> for Val {
    #[inline]
    fn from(t: taffy::LengthPercentage) -> Self {
        let raw = t.into_raw();
        if raw.is_auto() {
            Self::Auto
        } else {
            match raw.tag() {
                2 => Self::Percent(raw.value() * 100.0),
                _ => Self::Px(raw.value()),
            }
        }
    }
}

impl From<Val> for taffy::LengthPercentageAuto {
    #[inline]
    fn from(val: Val) -> Self {
        match val {
            Val::Auto => Self::auto(),
            Val::Px(v) => Self::length(v),
            Val::Percent(v) => Self::percent(v / 100.0),
        }
    }
}

impl From<taffy::LengthPercentageAuto> for Val {
    #[inline]
    fn from(t: taffy::LengthPercentageAuto) -> Self {
        let raw = t.into_raw();
        if raw.is_auto() {
            Self::Auto
        } else {
            match raw.tag() {
                2 => Self::Percent(raw.value() * 100.0),
                _ => Self::Px(raw.value()),
            }
        }
    }
}

impl From<Display> for taffy::Display {
    #[inline]
    fn from(d: Display) -> Self {
        match d {
            Display::Flex => Self::Flex,
            Display::Grid => Self::Grid,
            Display::Block => Self::Block,
            Display::None => Self::None,
        }
    }
}

impl From<taffy::Display> for Display {
    #[inline]
    fn from(t: taffy::Display) -> Self {
        match t {
            taffy::Display::Flex => Self::Flex,
            taffy::Display::Grid => Self::Grid,
            taffy::Display::Block => Self::Block,
            taffy::Display::None => Self::None,
        }
    }
}

impl From<Position> for taffy::Position {
    #[inline]
    fn from(p: Position) -> Self {
        match p {
            Position::Relative => Self::Relative,
            Position::Absolute => Self::Absolute,
        }
    }
}

impl From<taffy::Position> for Position {
    #[inline]
    fn from(t: taffy::Position) -> Self {
        match t {
            taffy::Position::Relative => Self::Relative,
            taffy::Position::Absolute => Self::Absolute,
        }
    }
}

impl From<BoxSizing> for taffy::BoxSizing {
    #[inline]
    fn from(b: BoxSizing) -> Self {
        match b {
            BoxSizing::BorderBox => Self::BorderBox,
            BoxSizing::ContentBox => Self::ContentBox,
        }
    }
}

impl From<taffy::BoxSizing> for BoxSizing {
    #[inline]
    fn from(t: taffy::BoxSizing) -> Self {
        match t {
            taffy::BoxSizing::BorderBox => Self::BorderBox,
            taffy::BoxSizing::ContentBox => Self::ContentBox,
        }
    }
}

impl From<Direction> for taffy::Direction {
    #[inline]
    fn from(d: Direction) -> Self {
        match d {
            Direction::Ltr => Self::Ltr,
            Direction::Rtl => Self::Rtl,
        }
    }
}

impl From<taffy::Direction> for Direction {
    #[inline]
    fn from(t: taffy::Direction) -> Self {
        match t {
            taffy::Direction::Ltr => Self::Ltr,
            taffy::Direction::Rtl => Self::Rtl,
        }
    }
}

impl From<Overflow> for taffy::Overflow {
    #[inline]
    fn from(o: Overflow) -> Self {
        match o {
            Overflow::Visible => Self::Visible,
            Overflow::Hidden => Self::Hidden,
            Overflow::Scroll => Self::Scroll,
            Overflow::Clip => Self::Clip,
        }
    }
}

impl From<taffy::Overflow> for Overflow {
    #[inline]
    fn from(t: taffy::Overflow) -> Self {
        match t {
            taffy::Overflow::Visible => Self::Visible,
            taffy::Overflow::Hidden => Self::Hidden,
            taffy::Overflow::Scroll => Self::Scroll,
            taffy::Overflow::Clip => Self::Clip,
        }
    }
}

impl From<LayoutOverflow> for taffy::Point<taffy::Overflow> {
    #[inline]
    fn from(lo: LayoutOverflow) -> Self {
        Self {
            x: lo.x.into(),
            y: lo.y.into(),
        }
    }
}

impl From<taffy::Point<taffy::Overflow>> for LayoutOverflow {
    #[inline]
    fn from(t: taffy::Point<taffy::Overflow>) -> Self {
        Self {
            x: t.x.into(),
            y: t.y.into(),
        }
    }
}

impl From<FlexDirection> for taffy::FlexDirection {
    #[inline]
    fn from(fd: FlexDirection) -> Self {
        match fd {
            FlexDirection::Row => Self::Row,
            FlexDirection::Column => Self::Column,
            FlexDirection::RowReverse => Self::RowReverse,
            FlexDirection::ColumnReverse => Self::ColumnReverse,
        }
    }
}

impl From<taffy::FlexDirection> for FlexDirection {
    #[inline]
    fn from(t: taffy::FlexDirection) -> Self {
        match t {
            taffy::FlexDirection::Row => Self::Row,
            taffy::FlexDirection::Column => Self::Column,
            taffy::FlexDirection::RowReverse => Self::RowReverse,
            taffy::FlexDirection::ColumnReverse => Self::ColumnReverse,
        }
    }
}

impl From<FlexWrap> for taffy::FlexWrap {
    #[inline]
    fn from(fw: FlexWrap) -> Self {
        match fw {
            FlexWrap::NoWrap => Self::NoWrap,
            FlexWrap::Wrap => Self::Wrap,
            FlexWrap::WrapReverse => Self::WrapReverse,
        }
    }
}

impl From<taffy::FlexWrap> for FlexWrap {
    #[inline]
    fn from(t: taffy::FlexWrap) -> Self {
        match t {
            taffy::FlexWrap::NoWrap => Self::NoWrap,
            taffy::FlexWrap::Wrap => Self::Wrap,
            taffy::FlexWrap::WrapReverse => Self::WrapReverse,
        }
    }
}

impl From<AlignItems> for taffy::AlignItems {
    #[inline]
    fn from(ai: AlignItems) -> Self {
        match ai {
            AlignItems::Start => Self::START,
            AlignItems::End => Self::END,
            AlignItems::FlexStart => Self::FLEX_START,
            AlignItems::FlexEnd => Self::FLEX_END,
            AlignItems::Center => Self::CENTER,
            AlignItems::Baseline => Self::BASELINE,
            AlignItems::Stretch => Self::STRETCH,
            AlignItems::SafeStart => Self::SAFE_START,
            AlignItems::SafeEnd => Self::SAFE_END,
            AlignItems::SafeFlexStart => Self::SAFE_FLEX_START,
            AlignItems::SafeFlexEnd => Self::SAFE_FLEX_END,
            AlignItems::SafeCenter => Self::SAFE_CENTER,
        }
    }
}

impl From<taffy::AlignItems> for AlignItems {
    #[inline]
    fn from(t: taffy::AlignItems) -> Self {
        match t {
            taffy::AlignItems::START => Self::Start,
            taffy::AlignItems::END => Self::End,
            taffy::AlignItems::FLEX_START => Self::FlexStart,
            taffy::AlignItems::FLEX_END => Self::FlexEnd,
            taffy::AlignItems::CENTER => Self::Center,
            taffy::AlignItems::BASELINE => Self::Baseline,
            taffy::AlignItems::SAFE_START => Self::SafeStart,
            taffy::AlignItems::SAFE_END => Self::SafeEnd,
            taffy::AlignItems::SAFE_FLEX_START => Self::SafeFlexStart,
            taffy::AlignItems::SAFE_FLEX_END => Self::SafeFlexEnd,
            taffy::AlignItems::SAFE_CENTER => Self::SafeCenter,
            _ => Self::Stretch,
        }
    }
}

impl From<AlignSelf> for taffy::AlignSelf {
    #[inline]
    fn from(asf: AlignSelf) -> Self {
        match asf {
            AlignSelf::Start => Self::START,
            AlignSelf::End => Self::END,
            AlignSelf::Center => Self::CENTER,
            AlignSelf::Baseline => Self::BASELINE,
            AlignSelf::Stretch => Self::STRETCH,
            AlignSelf::FlexStart => Self::FLEX_START,
            AlignSelf::FlexEnd => Self::FLEX_END,
            AlignSelf::SafeStart => Self::SAFE_START,
            AlignSelf::SafeEnd => Self::SAFE_END,
            AlignSelf::SafeFlexStart => Self::SAFE_FLEX_START,
            AlignSelf::SafeFlexEnd => Self::SAFE_FLEX_END,
            AlignSelf::SafeCenter => Self::SAFE_CENTER,
        }
    }
}

impl From<taffy::style::AlignSelf> for AlignSelf {
    #[inline]
    fn from(t: taffy::style::AlignSelf) -> Self {
        match t {
            taffy::AlignSelf::START => Self::Start,
            taffy::AlignSelf::END => Self::End,
            taffy::AlignSelf::CENTER => Self::Center,
            taffy::AlignSelf::BASELINE => Self::Baseline,
            taffy::AlignSelf::FLEX_START => Self::FlexStart,
            taffy::AlignSelf::FLEX_END => Self::FlexEnd,
            taffy::AlignSelf::SAFE_START => Self::SafeStart,
            taffy::AlignSelf::SAFE_END => Self::SafeEnd,
            taffy::AlignSelf::SAFE_FLEX_START => Self::SafeFlexStart,
            taffy::AlignSelf::SAFE_FLEX_END => Self::SafeFlexEnd,
            taffy::AlignSelf::SAFE_CENTER => Self::SafeCenter,
            _ => Self::Stretch,
        }
    }
}

impl From<JustifyContent> for taffy::JustifyContent {
    #[inline]
    fn from(jc: JustifyContent) -> Self {
        match jc {
            JustifyContent::Start => Self::START,
            JustifyContent::End => Self::END,
            JustifyContent::Center => Self::CENTER,
            JustifyContent::Stretch => Self::STRETCH,
            JustifyContent::SpaceBetween => Self::SPACE_BETWEEN,
            JustifyContent::SpaceAround => Self::SPACE_AROUND,
            JustifyContent::SpaceEvenly => Self::SPACE_EVENLY,
            JustifyContent::FlexStart => Self::FLEX_START,
            JustifyContent::FlexEnd => Self::FLEX_END,
            JustifyContent::SafeStart => Self::SAFE_START,
            JustifyContent::SafeEnd => Self::SAFE_END,
            JustifyContent::SafeFlexStart => Self::SAFE_FLEX_START,
            JustifyContent::SafeFlexEnd => Self::SAFE_FLEX_END,
            JustifyContent::SafeCenter => Self::SAFE_CENTER,
        }
    }
}

impl From<taffy::JustifyContent> for JustifyContent {
    #[inline]
    fn from(t: taffy::JustifyContent) -> Self {
        match t {
            taffy::JustifyContent::START => Self::Start,
            taffy::JustifyContent::END => Self::End,
            taffy::JustifyContent::CENTER | taffy::JustifyContent::SAFE_CENTER => Self::Center,
            taffy::JustifyContent::SPACE_BETWEEN => Self::SpaceBetween,
            taffy::JustifyContent::SPACE_AROUND => Self::SpaceAround,
            taffy::JustifyContent::SPACE_EVENLY => Self::SpaceEvenly,
            taffy::JustifyContent::SAFE_START => Self::SafeStart,
            taffy::JustifyContent::SAFE_END => Self::SafeEnd,
            taffy::JustifyContent::SAFE_FLEX_START => Self::SafeFlexStart,
            taffy::JustifyContent::SAFE_FLEX_END => Self::SafeFlexEnd,
            taffy::JustifyContent::FLEX_START => Self::FlexStart,
            taffy::JustifyContent::FLEX_END => Self::FlexEnd,
            _ => Self::Stretch,
        }
    }
}

impl From<AlignContent> for taffy::AlignContent {
    #[inline]
    fn from(ac: AlignContent) -> Self {
        match ac {
            AlignContent::Start => Self::START,
            AlignContent::End => Self::END,
            AlignContent::Center => Self::CENTER,
            AlignContent::Stretch => Self::STRETCH,
            AlignContent::SpaceBetween => Self::SPACE_BETWEEN,
            AlignContent::SpaceAround => Self::SPACE_AROUND,
            AlignContent::SpaceEvenly => Self::SPACE_EVENLY,
            AlignContent::FlexStart => Self::FLEX_START,
            AlignContent::FlexEnd => Self::FLEX_END,
            AlignContent::SafeStart => Self::SAFE_START,
            AlignContent::SafeEnd => Self::SAFE_END,
            AlignContent::SafeFlexStart => Self::SAFE_FLEX_START,
            AlignContent::SafeFlexEnd => Self::SAFE_FLEX_END,
            AlignContent::SafeCenter => Self::SAFE_CENTER,
        }
    }
}

impl From<taffy::AlignContent> for AlignContent {
    #[inline]
    fn from(t: taffy::AlignContent) -> Self {
        match t {
            taffy::AlignContent::START => Self::Start,
            taffy::AlignContent::END => Self::End,
            taffy::AlignContent::CENTER | taffy::AlignContent::SAFE_CENTER => Self::Center,
            taffy::AlignContent::SPACE_BETWEEN => Self::SpaceBetween,
            taffy::AlignContent::SPACE_AROUND => Self::SpaceAround,
            taffy::AlignContent::SPACE_EVENLY => Self::SpaceEvenly,
            taffy::AlignContent::SAFE_START => Self::SafeStart,
            taffy::AlignContent::SAFE_END => Self::SafeEnd,
            taffy::AlignContent::SAFE_FLEX_START => Self::SafeFlexStart,
            taffy::AlignContent::SAFE_FLEX_END => Self::SafeFlexEnd,
            taffy::AlignContent::FLEX_START => Self::FlexStart,
            taffy::AlignContent::FLEX_END => Self::FlexEnd,
            _ => Self::Stretch,
        }
    }
}

impl From<TextAlign> for taffy::TextAlign {
    #[inline]
    fn from(ta: TextAlign) -> Self {
        match ta {
            TextAlign::Auto => Self::Auto,
            TextAlign::Left => Self::LegacyLeft,
            TextAlign::Right => Self::LegacyRight,
            TextAlign::Center => Self::LegacyCenter,
        }
    }
}

impl From<taffy::TextAlign> for TextAlign {
    #[inline]
    fn from(t: taffy::TextAlign) -> Self {
        match t {
            taffy::TextAlign::Auto => Self::Auto,
            taffy::TextAlign::LegacyLeft => Self::Left,
            taffy::TextAlign::LegacyRight => Self::Right,
            taffy::TextAlign::LegacyCenter => Self::Center,
        }
    }
}

impl From<GridAutoFlow> for taffy::GridAutoFlow {
    #[inline]
    fn from(gaf: GridAutoFlow) -> Self {
        match gaf {
            GridAutoFlow::Row => Self::Row,
            GridAutoFlow::Column => Self::Column,
            GridAutoFlow::RowDense => Self::RowDense,
            GridAutoFlow::ColumnDense => Self::ColumnDense,
        }
    }
}

impl From<taffy::GridAutoFlow> for GridAutoFlow {
    #[inline]
    fn from(t: taffy::GridAutoFlow) -> Self {
        match t {
            taffy::GridAutoFlow::Row => Self::Row,
            taffy::GridAutoFlow::Column => Self::Column,
            taffy::GridAutoFlow::RowDense => Self::RowDense,
            taffy::GridAutoFlow::ColumnDense => Self::ColumnDense,
        }
    }
}

impl<S> From<GridPlacement<S>> for taffy::GridPlacement<String>
where
    S: ToString,
{
    #[inline]
    fn from(gp: GridPlacement<S>) -> Self {
        match gp {
            GridPlacement::Auto => Self::Auto,
            GridPlacement::Line(s) => {
                let s_str = s.to_string();
                if let Ok(val) = s_str.parse::<i16>() {
                    Self::Line(val.into())
                } else {
                    Self::NamedLine(s_str, 1)
                }
            }
            GridPlacement::NamedLine(s) => Self::NamedLine(s.to_string(), 1),
            GridPlacement::Span(s) => {
                let s_str = s.to_string();
                if let Ok(val) = s_str.parse::<u16>() {
                    Self::Span(val)
                } else {
                    Self::Auto
                }
            }
            GridPlacement::NamedSpan(s) => Self::NamedSpan(s.to_string(), 1),
        }
    }
}

impl From<taffy::GridPlacement<String>> for GridPlacement<String> {
    #[inline]
    fn from(t: taffy::GridPlacement<String>) -> Self {
        match t {
            taffy::GridPlacement::Auto => Self::Auto,
            taffy::GridPlacement::Line(grid_line) => Self::Line(grid_line.as_i16().to_string()),
            taffy::GridPlacement::NamedLine(s, _val) => Self::NamedLine(s),
            taffy::GridPlacement::Span(val) => Self::Span(val.to_string()),
            taffy::GridPlacement::NamedSpan(s, _val) => Self::NamedSpan(s),
        }
    }
}

impl From<PathBuf> for ImageSource {
    fn from(path: PathBuf) -> Self {
        Self::Path(path)
    }
}

impl From<&'static str> for ImageSource {
    fn from(s: &'static str) -> Self {
        Self::Path(PathBuf::from(s))
    }
}

impl From<PathBuf> for MovieSource {
    fn from(path: PathBuf) -> Self {
        Self::Path(path)
    }
}

impl From<&'static str> for MovieSource {
    fn from(s: &'static str) -> Self {
        Self::Path(PathBuf::from(s))
    }
}

impl From<&'static str> for UiaValue {
    fn from(s: &'static str) -> Self {
        Self::String(String::from(s))
    }
}

impl From<String> for UiaValue {
    fn from(s: String) -> Self {
        Self::String(s)
    }
}

impl From<bool> for UiaValue {
    fn from(b: bool) -> Self {
        Self::Bool(b)
    }
}

impl From<i32> for UiaValue {
    fn from(i: i32) -> Self {
        Self::Int(i)
    }
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

/// StyleValue（スレッド安全なクロージャ付き）から Prop への透過変換をサポート
impl<T: 'static> From<StyleValue<T>> for Prop<T> {
    #[inline]
    fn from(val: StyleValue<T>) -> Self {
        match val {
            StyleValue::Static(v) => Prop::Static(v),
            StyleValue::Dynamic(getter) => {
                // スレッド安全な動的ゲッターを実行時に評価する Prop::Dynamic に変換
                Prop::Dynamic(Box::new(getter))
            }
        }
    }
}

impl From<bool> for Prop<bool> {
    #[inline]
    fn from(b: bool) -> Self {
        Self::Static(b)
    }
}

impl From<ReadSignal<bool>> for Prop<bool> {
    #[inline]
    fn from(sig: ReadSignal<bool>) -> Self {
        Self::Dynamic(Box::new(move || sig.get()))
    }
}

impl<F> From<F> for Prop<bool>
where
    F: Fn() -> bool + 'static,
{
    #[inline]
    fn from(f: F) -> Self {
        Self::Dynamic(Box::new(f))
    }
}

pub trait IntoStyleValue<T> {
    fn into_style_value(self) -> StyleValue<T>;
}

impl<T: Send + Sync + 'static> IntoStyleValue<T> for T {
    fn into_style_value(self) -> StyleValue<T> {
        StyleValue::Static(self)
    }
}

impl<T: Send + Sync + 'static> IntoStyleValue<T> for StyleValue<T> {
    fn into_style_value(self) -> StyleValue<T> {
        self
    }
}

pub trait IntoStyleRect<T> {
    fn into_style_rect(self) -> StyleValue<Rect<T>>;
}

impl<T: Send + Sync + 'static> IntoStyleRect<T> for StyleValue<Rect<T>> {
    fn into_style_rect(self) -> StyleValue<Rect<T>> {
        self
    }
}

impl<T: Clone + Send + Sync + 'static> IntoStyleRect<T> for Rect<T> {
    fn into_style_rect(self) -> StyleValue<Rect<T>> {
        StyleValue::Static(self)
    }
}

impl<T: Clone + Send + Sync + 'static> IntoStyleRect<T> for f32
where
    f32: IntoRect<T>,
{
    fn into_style_rect(self) -> StyleValue<Rect<T>> {
        StyleValue::Static(self.into_rect())
    }
}

impl<T: Clone + Send + Sync + 'static> IntoStyleRect<T> for i32
where
    i32: IntoRect<T>,
{
    fn into_style_rect(self) -> StyleValue<Rect<T>> {
        StyleValue::Static(self.into_rect())
    }
}

impl<T: Clone + Send + Sync + 'static> IntoStyleRect<T> for Pixel
where
    Pixel: IntoRect<T>,
{
    fn into_style_rect(self) -> StyleValue<Rect<T>> {
        StyleValue::Static(self.into_rect())
    }
}

impl<T: Clone + Send + Sync + 'static> IntoStyleRect<T> for Percent
where
    Percent: IntoRect<T>,
{
    fn into_style_rect(self) -> StyleValue<Rect<T>> {
        StyleValue::Static(self.into_rect())
    }
}

impl<T: Clone + Send + Sync + 'static> IntoStyleRect<T> for Auto
where
    Auto: IntoRect<T>,
{
    fn into_style_rect(self) -> StyleValue<Rect<T>> {
        StyleValue::Static(self.into_rect())
    }
}

impl<V, H, T> IntoStyleRect<T> for (V, H)
where
    (V, H): IntoRect<T> + Send + Sync + 'static,
    T: Send + Sync + 'static,
{
    fn into_style_rect(self) -> StyleValue<Rect<T>> {
        StyleValue::Static(self.into_rect())
    }
}

impl<Top, Right, Bottom, Left, T> IntoStyleRect<T> for (Top, Right, Bottom, Left)
where
    (Top, Right, Bottom, Left): IntoRect<T> + Send + Sync + 'static,
    T: Send + Sync + 'static,
{
    fn into_style_rect(self) -> StyleValue<Rect<T>> {
        StyleValue::Static(self.into_rect())
    }
}

pub trait IntoStyleSize<T> {
    fn into_style_size(self) -> StyleValue<Size<T>>;
}

impl<T: Send + Sync + 'static> IntoStyleSize<T> for StyleValue<Size<T>> {
    fn into_style_size(self) -> StyleValue<Size<T>> {
        self
    }
}

impl<T: Clone + Send + Sync + 'static> IntoStyleSize<T> for Size<T> {
    fn into_style_size(self) -> StyleValue<Size<T>> {
        StyleValue::Static(self)
    }
}

impl<T: Clone + Send + Sync + 'static> IntoStyleSize<T> for f32
where
    f32: IntoSize<T>,
{
    fn into_style_size(self) -> StyleValue<Size<T>> {
        StyleValue::Static(self.into_size())
    }
}

impl<T: Clone + Send + Sync + 'static> IntoStyleSize<T> for i32
where
    i32: IntoSize<T>,
{
    fn into_style_size(self) -> StyleValue<Size<T>> {
        StyleValue::Static(self.into_size())
    }
}

impl<T: Clone + Send + Sync + 'static> IntoStyleSize<T> for Pixel
where
    Pixel: IntoSize<T>,
{
    fn into_style_size(self) -> StyleValue<Size<T>> {
        StyleValue::Static(self.into_size())
    }
}

impl<T: Clone + Send + Sync + 'static> IntoStyleSize<T> for Percent
where
    Percent: IntoSize<T>,
{
    fn into_style_size(self) -> StyleValue<Size<T>> {
        StyleValue::Static(self.into_size())
    }
}

impl<T: Clone + Send + Sync + 'static> IntoStyleSize<T> for Auto
where
    Auto: IntoSize<T>,
{
    fn into_style_size(self) -> StyleValue<Size<T>> {
        StyleValue::Static(self.into_size())
    }
}

impl<W, H, T> IntoStyleSize<T> for (W, H)
where
    (W, H): IntoSize<T> + Send + Sync + 'static,
    T: Send + Sync + 'static,
{
    fn into_style_size(self) -> StyleValue<Size<T>> {
        StyleValue::Static(self.into_size())
    }
}

pub trait IntoStylePoint<T> {
    fn into_style_point(self) -> StyleValue<Point<T>>;
}

impl<T: Send + Sync + 'static> IntoStylePoint<T> for StyleValue<Point<T>> {
    fn into_style_point(self) -> StyleValue<Point<T>> {
        self
    }
}

impl<T: Clone + Send + Sync + 'static> IntoStylePoint<T> for Point<T> {
    fn into_style_point(self) -> StyleValue<Point<T>> {
        StyleValue::Static(self)
    }
}

impl<T: Clone + Send + Sync + 'static> IntoStylePoint<T> for f32
where
    f32: IntoPoint<T>,
{
    fn into_style_point(self) -> StyleValue<Point<T>> {
        StyleValue::Static(self.into_point())
    }
}

impl<T: Clone + Send + Sync + 'static> IntoStylePoint<T> for i32
where
    i32: IntoPoint<T>,
{
    fn into_style_point(self) -> StyleValue<Point<T>> {
        StyleValue::Static(self.into_point())
    }
}

impl<T: Clone + Send + Sync + 'static> IntoStylePoint<T> for Pixel
where
    Pixel: IntoPoint<T>,
{
    fn into_style_point(self) -> StyleValue<Point<T>> {
        StyleValue::Static(self.into_point())
    }
}

impl<T: Clone + Send + Sync + 'static> IntoStylePoint<T> for Percent
where
    Percent: IntoPoint<T>,
{
    fn into_style_point(self) -> StyleValue<Point<T>> {
        StyleValue::Static(self.into_point())
    }
}

impl<T: Clone + Send + Sync + 'static> IntoStylePoint<T> for Auto
where
    Auto: IntoPoint<T>,
{
    fn into_style_point(self) -> StyleValue<Point<T>> {
        StyleValue::Static(self.into_point())
    }
}

impl<W, H, T> IntoStylePoint<T> for (W, H)
where
    (W, H): IntoPoint<T> + Send + Sync + 'static,
    T: Send + Sync + 'static,
{
    fn into_style_point(self) -> StyleValue<Point<T>> {
        StyleValue::Static(self.into_point())
    }
}

pub trait IntoStyleCornerRadius {
    fn into_style_corner_radius(self) -> StyleValue<CornerRadius>;
}

impl IntoStyleCornerRadius for StyleValue<CornerRadius> {
    fn into_style_corner_radius(self) -> StyleValue<CornerRadius> {
        self
    }
}

impl IntoStyleCornerRadius for CornerRadius {
    fn into_style_corner_radius(self) -> StyleValue<CornerRadius> {
        StyleValue::Static(self)
    }
}

impl IntoStyleCornerRadius for f32 {
    fn into_style_corner_radius(self) -> StyleValue<CornerRadius> {
        StyleValue::Static(self.into_corner_radius())
    }
}

impl IntoStyleCornerRadius for i32 {
    fn into_style_corner_radius(self) -> StyleValue<CornerRadius> {
        StyleValue::Static(self.into_corner_radius())
    }
}

impl<V, H> IntoStyleCornerRadius for (V, H)
where
    (V, H): IntoCornerRadius + Send + Sync + 'static,
{
    fn into_style_corner_radius(self) -> StyleValue<CornerRadius> {
        StyleValue::Static(self.into_corner_radius())
    }
}

impl<TL, TR, BR, BL> IntoStyleCornerRadius for (TL, TR, BR, BL)
where
    (TL, TR, BR, BL): IntoCornerRadius + Send + Sync + 'static,
{
    fn into_style_corner_radius(self) -> StyleValue<CornerRadius> {
        StyleValue::Static(self.into_corner_radius())
    }
}

pub trait IntoStyleConvert<T> {
    fn into_style_convert(self) -> StyleValue<T>;
}

impl IntoStyleConvert<Val> for StyleValue<Val> {
    fn into_style_convert(self) -> StyleValue<Val> {
        self
    }
}
impl IntoStyleConvert<Val> for Val {
    fn into_style_convert(self) -> StyleValue<Val> {
        StyleValue::Static(self)
    }
}
impl IntoStyleConvert<Val> for f32 {
    fn into_style_convert(self) -> StyleValue<Val> {
        StyleValue::Static(<Self as Convert<Val>>::convert(self))
    }
}
impl IntoStyleConvert<Val> for i32 {
    fn into_style_convert(self) -> StyleValue<Val> {
        StyleValue::Static(<Self as Convert<Val>>::convert(self))
    }
}
impl IntoStyleConvert<Val> for Pixel {
    fn into_style_convert(self) -> StyleValue<Val> {
        StyleValue::Static(<Self as Convert<Val>>::convert(self))
    }
}
impl IntoStyleConvert<Val> for Percent {
    fn into_style_convert(self) -> StyleValue<Val> {
        StyleValue::Static(<Self as Convert<Val>>::convert(self))
    }
}
impl IntoStyleConvert<Val> for Auto {
    fn into_style_convert(self) -> StyleValue<Val> {
        StyleValue::Static(<Self as Convert<Val>>::convert(self))
    }
}

impl IntoStyleConvert<Length> for StyleValue<Length> {
    fn into_style_convert(self) -> StyleValue<Length> {
        self
    }
}
impl IntoStyleConvert<Length> for Length {
    fn into_style_convert(self) -> StyleValue<Length> {
        StyleValue::Static(self)
    }
}
impl IntoStyleConvert<Length> for f32 {
    fn into_style_convert(self) -> StyleValue<Length> {
        StyleValue::Static(<Self as Convert<Length>>::convert(self))
    }
}
impl IntoStyleConvert<Length> for i32 {
    fn into_style_convert(self) -> StyleValue<Length> {
        StyleValue::Static(<Self as Convert<Length>>::convert(self))
    }
}
impl IntoStyleConvert<Length> for Pixel {
    fn into_style_convert(self) -> StyleValue<Length> {
        StyleValue::Static(<Self as Convert<Length>>::convert(self))
    }
}
impl IntoStyleConvert<Length> for Percent {
    fn into_style_convert(self) -> StyleValue<Length> {
        StyleValue::Static(<Self as Convert<Length>>::convert(self))
    }
}

impl IntoStyleConvert<f32> for StyleValue<f32> {
    fn into_style_convert(self) -> StyleValue<f32> {
        self
    }
}
impl IntoStyleConvert<f32> for f32 {
    fn into_style_convert(self) -> StyleValue<f32> {
        StyleValue::Static(self)
    }
}
impl IntoStyleConvert<f32> for i32 {
    fn into_style_convert(self) -> StyleValue<f32> {
        StyleValue::Static(<Self as Convert<f32>>::convert(self))
    }
}
impl IntoStyleConvert<f32> for bool {
    fn into_style_convert(self) -> StyleValue<f32> {
        StyleValue::Static(<Self as Convert<f32>>::convert(self))
    }
}

impl IntoStyleValue<Option<AlignItems>> for AlignItems {
    fn into_style_value(self) -> StyleValue<Option<AlignItems>> {
        StyleValue::Static(Some(self))
    }
}

impl IntoStyleValue<Option<AlignSelf>> for AlignSelf {
    fn into_style_value(self) -> StyleValue<Option<AlignSelf>> {
        StyleValue::Static(Some(self))
    }
}

impl IntoStyleValue<Option<AlignContent>> for AlignContent {
    fn into_style_value(self) -> StyleValue<Option<AlignContent>> {
        StyleValue::Static(Some(self))
    }
}

impl IntoStyleValue<Option<JustifyContent>> for JustifyContent {
    fn into_style_value(self) -> StyleValue<Option<JustifyContent>> {
        StyleValue::Static(Some(self))
    }
}

impl IntoStyleValue<Cow<'static, str>> for &'static str {
    #[inline]
    fn into_style_value(self) -> StyleValue<Cow<'static, str>> {
        StyleValue::Static(Cow::Borrowed(self))
    }
}

impl IntoStyleValue<Cow<'static, str>> for String {
    #[inline]
    fn into_style_value(self) -> StyleValue<Cow<'static, str>> {
        StyleValue::Static(Cow::Owned(self))
    }
}

pub trait IntoStyleResizable {
    fn into_style_resizable(self) -> StyleValue<[bool; 4]>;
}

impl IntoStyleResizable for StyleValue<[bool; 4]> {
    #[inline]
    fn into_style_resizable(self) -> StyleValue<[bool; 4]> {
        self
    }
}

impl IntoStyleResizable for [bool; 4] {
    #[inline]
    fn into_style_resizable(self) -> StyleValue<[bool; 4]> {
        StyleValue::Static(self)
    }
}

impl IntoStyleResizable for (bool, bool, bool, bool) {
    #[inline]
    fn into_style_resizable(self) -> StyleValue<[bool; 4]> {
        StyleValue::Static([self.0, self.1, self.2, self.3])
    }
}

impl IntoStyleResizable for ReadSignal<[bool; 4]> {
    #[inline]
    fn into_style_resizable(self) -> StyleValue<[bool; 4]> {
        StyleValue::Dynamic(Box::new(move || self.get()))
    }
}

impl IntoStyleResizable for ReadSignal<(bool, bool, bool, bool)> {
    #[inline]
    fn into_style_resizable(self) -> StyleValue<[bool; 4]> {
        StyleValue::Dynamic(Box::new(move || {
            let t = self.get();
            [t.0, t.1, t.2, t.3]
        }))
    }
}

impl<T: Clone + Send + Sync + 'static> IntoStyleValue<T> for ReadSignal<T> {
    #[inline]
    fn into_style_value(self) -> StyleValue<T> {
        StyleValue::Dynamic(Box::new(move || self.get()))
    }
}

impl<T: Clone + Send + Sync + 'static> IntoStyleRect<T> for ReadSignal<Rect<T>> {
    #[inline]
    fn into_style_rect(self) -> StyleValue<Rect<T>> {
        StyleValue::Dynamic(Box::new(move || self.get()))
    }
}

impl<T: Clone + Send + Sync + 'static> IntoStyleRect<T> for ReadSignal<Val>
where
    Val: IntoRect<T>,
{
    #[inline]
    fn into_style_rect(self) -> StyleValue<Rect<T>> {
        StyleValue::Dynamic(Box::new(move || self.get().into_rect()))
    }
}

impl<T: Clone + Send + Sync + 'static> IntoStyleRect<T> for ReadSignal<Length>
where
    Length: IntoRect<T>,
{
    #[inline]
    fn into_style_rect(self) -> StyleValue<Rect<T>> {
        StyleValue::Dynamic(Box::new(move || self.get().into_rect()))
    }
}

impl<T: Clone + Send + Sync + 'static> IntoStyleRect<T> for ReadSignal<f32>
where
    f32: IntoRect<T>,
{
    #[inline]
    fn into_style_rect(self) -> StyleValue<Rect<T>> {
        StyleValue::Dynamic(Box::new(move || self.get().into_rect()))
    }
}

impl<V, H, T> IntoStyleRect<T> for ReadSignal<(V, H)>
where
    (V, H): IntoRect<T> + Clone + Send + Sync + 'static,
    T: Send + Sync + 'static,
{
    #[inline]
    fn into_style_rect(self) -> StyleValue<Rect<T>> {
        StyleValue::Dynamic(Box::new(move || self.get().into_rect()))
    }
}

impl<Top, Right, Bottom, Left, T> IntoStyleRect<T> for ReadSignal<(Top, Right, Bottom, Left)>
where
    (Top, Right, Bottom, Left): IntoRect<T> + Clone + Send + Sync + 'static,
    T: Send + Sync + 'static,
{
    #[inline]
    fn into_style_rect(self) -> StyleValue<Rect<T>> {
        StyleValue::Dynamic(Box::new(move || self.get().into_rect()))
    }
}

impl<T: Clone + Send + Sync + 'static> IntoStyleSize<T> for ReadSignal<Size<T>> {
    #[inline]
    fn into_style_size(self) -> StyleValue<Size<T>> {
        StyleValue::Dynamic(Box::new(move || self.get()))
    }
}

impl<T: Clone + Send + Sync + 'static> IntoStyleSize<T> for ReadSignal<Val>
where
    Val: IntoSize<T>,
{
    #[inline]
    fn into_style_size(self) -> StyleValue<Size<T>> {
        StyleValue::Dynamic(Box::new(move || self.get().into_size()))
    }
}

impl<T: Clone + Send + Sync + 'static> IntoStyleSize<T> for ReadSignal<Length>
where
    Length: IntoSize<T>,
{
    #[inline]
    fn into_style_size(self) -> StyleValue<Size<T>> {
        StyleValue::Dynamic(Box::new(move || self.get().into_size()))
    }
}

impl<T: Clone + Send + Sync + 'static> IntoStyleSize<T> for ReadSignal<f32>
where
    f32: IntoSize<T>,
{
    #[inline]
    fn into_style_size(self) -> StyleValue<Size<T>> {
        StyleValue::Dynamic(Box::new(move || self.get().into_size()))
    }
}

impl<W, H, T> IntoStyleSize<T> for ReadSignal<(W, H)>
where
    (W, H): IntoSize<T> + Clone + Send + Sync + 'static,
    T: Send + Sync + 'static,
{
    #[inline]
    fn into_style_size(self) -> StyleValue<Size<T>> {
        StyleValue::Dynamic(Box::new(move || self.get().into_size()))
    }
}

impl IntoStyleCornerRadius for ReadSignal<CornerRadius> {
    #[inline]
    fn into_style_corner_radius(self) -> StyleValue<CornerRadius> {
        StyleValue::Dynamic(Box::new(move || self.get()))
    }
}

impl IntoStyleCornerRadius for ReadSignal<f32> {
    #[inline]
    fn into_style_corner_radius(self) -> StyleValue<CornerRadius> {
        StyleValue::Dynamic(Box::new(move || self.get().into_corner_radius()))
    }
}

impl IntoStyleCornerRadius for ReadSignal<i32> {
    #[inline]
    fn into_style_corner_radius(self) -> StyleValue<CornerRadius> {
        StyleValue::Dynamic(Box::new(move || self.get().into_corner_radius()))
    }
}

impl<V, H> IntoStyleCornerRadius for ReadSignal<(V, H)>
where
    (V, H): IntoCornerRadius + Clone + Send + Sync + 'static,
{
    #[inline]
    fn into_style_corner_radius(self) -> StyleValue<CornerRadius> {
        StyleValue::Dynamic(Box::new(move || self.get().into_corner_radius()))
    }
}

impl<TL, TR, BR, BL> IntoStyleCornerRadius for ReadSignal<(TL, TR, BR, BL)>
where
    (TL, TR, BR, BL): IntoCornerRadius + Clone + Send + Sync + 'static,
{
    #[inline]
    fn into_style_corner_radius(self) -> StyleValue<CornerRadius> {
        StyleValue::Dynamic(Box::new(move || self.get().into_corner_radius()))
    }
}

impl<S, T> IntoStyleConvert<T> for ReadSignal<S>
where
    S: Convert<T> + Clone + Send + Sync + 'static,
    T: Send + Sync + 'static,
{
    #[inline]
    fn into_style_convert(self) -> StyleValue<T> {
        StyleValue::Dynamic(Box::new(move || self.get().convert()))
    }
}

pub trait IntoFocusable {
    fn into_focusable(self) -> Focusable;
}

impl IntoFocusable for Focusable {
    #[inline]
    fn into_focusable(self) -> Focusable {
        self
    }
}

impl IntoFocusable for bool {
    #[inline]
    fn into_focusable(self) -> Focusable {
        if self {
            Focusable::Inherit(FocusTrigger::Both)
        } else {
            Focusable::None
        }
    }
}

// 単位の相互キャスト用の中間トレイト
pub trait Convert<T> {
    fn convert(self) -> T;
}

// 具象型からターゲット単位へのキャスト実装
impl Convert<Val> for f32 {
    #[inline]
    fn convert(self) -> Val {
        Val::Px(self)
    }
}
impl Convert<Val> for i32 {
    #[inline]
    fn convert(self) -> Val {
        Val::Px(self as f32)
    }
}
impl Convert<Val> for Pixel {
    #[inline]
    fn convert(self) -> Val {
        Val::Px(self.0)
    }
}
impl Convert<Val> for Percent {
    #[inline]
    fn convert(self) -> Val {
        Val::Percent(self.0)
    }
}
impl Convert<Val> for Auto {
    #[inline]
    fn convert(self) -> Val {
        Val::Auto
    }
}

impl Convert<Length> for f32 {
    #[inline]
    fn convert(self) -> Length {
        Length::Px(self)
    }
}
impl Convert<Length> for i32 {
    #[inline]
    fn convert(self) -> Length {
        Length::Px(self as f32)
    }
}
impl Convert<Length> for Pixel {
    #[inline]
    fn convert(self) -> Length {
        Length::Px(self.0)
    }
}
impl Convert<Length> for Percent {
    #[inline]
    fn convert(self) -> Length {
        Length::Percent(self.0)
    }
}

impl Convert<f32> for f32 {
    #[inline]
    fn convert(self) -> f32 {
        self
    }
}
impl Convert<f32> for i32 {
    #[inline]
    fn convert(self) -> f32 {
        self as f32
    }
}

// Size<T> 用
pub trait IntoSize<T> {
    fn into_size(self) -> Size<T>;
}

// 具象型ごとの単一値(all)実装
impl<T> IntoSize<T> for f32
where
    f32: Convert<T>,
    T: Clone,
{
    #[inline]
    fn into_size(self) -> Size<T> {
        let v = self.convert();
        Size {
            width: v.clone(),
            height: v,
        }
    }
}
impl<T> IntoSize<T> for i32
where
    i32: Convert<T>,
    T: Clone,
{
    #[inline]
    fn into_size(self) -> Size<T> {
        let v = self.convert();
        Size {
            width: v.clone(),
            height: v,
        }
    }
}
impl<T> IntoSize<T> for Pixel
where
    Pixel: Convert<T>,
    T: Clone,
{
    #[inline]
    fn into_size(self) -> Size<T> {
        let v = self.convert();
        Size {
            width: v.clone(),
            height: v,
        }
    }
}
impl<T> IntoSize<T> for Percent
where
    Percent: Convert<T>,
    T: Clone,
{
    #[inline]
    fn into_size(self) -> Size<T> {
        let v = self.convert();
        Size {
            width: v.clone(),
            height: v,
        }
    }
}
impl<T> IntoSize<T> for Auto
where
    Auto: Convert<T>,
    T: Clone,
{
    #[inline]
    fn into_size(self) -> Size<T> {
        let v = self.convert();
        Size {
            width: v.clone(),
            height: v,
        }
    }
}

// 2連タプル (width, height)
impl<W, H, T> IntoSize<T> for (W, H)
where
    W: Convert<T>,
    H: Convert<T>,
{
    #[inline]
    fn into_size(self) -> Size<T> {
        Size {
            width: self.0.convert(),
            height: self.1.convert(),
        }
    }
}

// Rect<T> 用
pub trait IntoRect<T> {
    fn into_rect(self) -> Rect<T>;
}

// 具象型ごとの単一値(all)実装
impl<T> IntoRect<T> for f32
where
    f32: Convert<T>,
    T: Clone,
{
    #[inline]
    fn into_rect(self) -> Rect<T> {
        let v = self.convert();
        Rect {
            top: v.clone(),
            right: v.clone(),
            bottom: v.clone(),
            left: v,
        }
    }
}
impl<T> IntoRect<T> for i32
where
    i32: Convert<T>,
    T: Clone,
{
    #[inline]
    fn into_rect(self) -> Rect<T> {
        let v = self.convert();
        Rect {
            top: v.clone(),
            right: v.clone(),
            bottom: v.clone(),
            left: v,
        }
    }
}
impl<T> IntoRect<T> for Pixel
where
    Pixel: Convert<T>,
    T: Clone,
{
    #[inline]
    fn into_rect(self) -> Rect<T> {
        let v = self.convert();
        Rect {
            top: v.clone(),
            right: v.clone(),
            bottom: v.clone(),
            left: v,
        }
    }
}
impl<T> IntoRect<T> for Percent
where
    Percent: Convert<T>,
    T: Clone,
{
    #[inline]
    fn into_rect(self) -> Rect<T> {
        let v = self.convert();
        Rect {
            top: v.clone(),
            right: v.clone(),
            bottom: v.clone(),
            left: v,
        }
    }
}

// Auto 単一値 ➔ Rect (4方向すべてを Auto 一括設定)
impl<T> IntoRect<T> for Auto
where
    Auto: Convert<T> + Clone,
    T: Clone,
{
    #[inline]
    fn into_rect(self) -> Rect<T> {
        let val = self.convert();
        Rect {
            top: val.clone(),
            right: val.clone(),
            bottom: val.clone(),
            left: val,
        }
    }
}

// 2連タプル (vertical, horizontal)
impl<V, H, T> IntoRect<T> for (V, H)
where
    V: Convert<T> + Clone,
    H: Convert<T> + Clone,
    T: Clone,
{
    #[inline]
    fn into_rect(self) -> Rect<T> {
        let vert = self.0.convert();
        let horiz = self.1.convert();
        Rect {
            top: vert.clone(),
            right: horiz.clone(),
            bottom: vert,
            left: horiz,
        }
    }
}

// 4連タプル (top, right, bottom, left)
impl<Top, Right, Bottom, Left, T> IntoRect<T> for (Top, Right, Bottom, Left)
where
    Top: Convert<T>,
    Right: Convert<T>,
    Bottom: Convert<T>,
    Left: Convert<T>,
{
    #[inline]
    fn into_rect(self) -> Rect<T> {
        Rect {
            top: self.0.convert(),
            right: self.1.convert(),
            bottom: self.2.convert(),
            left: self.3.convert(),
        }
    }
}

// Point<T> 用
pub trait IntoPoint<T> {
    fn into_point(self) -> Point<T>;
}

impl<T> IntoPoint<T> for f32
where
    f32: Convert<T>,
    T: Clone,
{
    #[inline]
    fn into_point(self) -> Point<T> {
        let v = self.convert();
        Point { x: v.clone(), y: v }
    }
}
impl<T> IntoPoint<T> for i32
where
    i32: Convert<T>,
    T: Clone,
{
    #[inline]
    fn into_point(self) -> Point<T> {
        let v = self.convert();
        Point { x: v.clone(), y: v }
    }
}
impl<T> IntoPoint<T> for Pixel
where
    Pixel: Convert<T>,
    T: Clone,
{
    #[inline]
    fn into_point(self) -> Point<T> {
        let v = self.convert();
        Point { x: v.clone(), y: v }
    }
}
impl<T> IntoPoint<T> for Percent
where
    Percent: Convert<T>,
    T: Clone,
{
    #[inline]
    fn into_point(self) -> Point<T> {
        let v = self.convert();
        Point { x: v.clone(), y: v }
    }
}

impl<X, Y, T> IntoPoint<T> for (X, Y)
where
    X: Convert<T>,
    Y: Convert<T>,
{
    #[inline]
    fn into_point(self) -> Point<T> {
        Point {
            x: self.0.convert(),
            y: self.1.convert(),
        }
    }
}

// CornerRadius 用
pub trait IntoCornerRadius {
    fn into_corner_radius(self) -> CornerRadius;
}

impl IntoCornerRadius for f32 {
    #[inline]
    fn into_corner_radius(self) -> CornerRadius {
        CornerRadius::all(self)
    }
}
impl IntoCornerRadius for i32 {
    #[inline]
    fn into_corner_radius(self) -> CornerRadius {
        CornerRadius::all(self as f32)
    }
}

impl<V, H> IntoCornerRadius for (V, H)
where
    V: Convert<f32>,
    H: Convert<f32>,
{
    #[inline]
    fn into_corner_radius(self) -> CornerRadius {
        CornerRadius::symmetric(self.0.convert(), self.1.convert())
    }
}

impl<TL, TR, BR, BL> IntoCornerRadius for (TL, TR, BR, BL)
where
    TL: Convert<f32>,
    TR: Convert<f32>,
    BR: Convert<f32>,
    BL: Convert<f32>,
{
    #[inline]
    fn into_corner_radius(self) -> CornerRadius {
        CornerRadius {
            top_left: self.0.convert(),
            top_right: self.1.convert(),
            bottom_right: self.2.convert(),
            bottom_left: self.3.convert(),
        }
    }
}

// LayoutPoint（BoxShadow 等のオフセット）用
pub trait IntoLayoutPoint {
    fn into_layout_point(self) -> LayoutPoint;
}

impl IntoLayoutPoint for f32 {
    #[inline]
    fn into_layout_point(self) -> LayoutPoint {
        LayoutPoint { x: self, y: self }
    }
}
impl IntoLayoutPoint for i32 {
    #[inline]
    fn into_layout_point(self) -> LayoutPoint {
        LayoutPoint {
            x: self as f32,
            y: self as f32,
        }
    }
}

impl<X, Y> IntoLayoutPoint for (X, Y)
where
    X: Convert<f32>,
    Y: Convert<f32>,
{
    #[inline]
    fn into_layout_point(self) -> LayoutPoint {
        LayoutPoint {
            x: self.0.convert(),
            y: self.1.convert(),
        }
    }
}

// Val 自身から Val への同一変換を実装
impl Convert<Val> for Val {
    #[inline]
    fn convert(self) -> Val {
        self
    }
}

// Length 自身から Length への同一変換を実装
impl Convert<Length> for Length {
    #[inline]
    fn convert(self) -> Length {
        self
    }
}

// bool から f32 への変換 (true ➔ 1.0f32, false ➔ 0.0f32)
impl Convert<f32> for bool {
    #[inline]
    fn convert(self) -> f32 {
        if self { 1.0 } else { 0.0 }
    }
}
