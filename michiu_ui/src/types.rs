use bytemuck::{Pod, Zeroable};
use std::{borrow::Cow, path::PathBuf, sync::Arc, time::Duration};

use crate::{
    AnimationCurve, Context, Convert, EntityId, IntoLayoutPoint, KeyframeAnimation, VirtualKey,
    bitmap::*, style::ThisStyle,
};

#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Default, Pod, Zeroable)]
pub struct Color {
    pub r: f32,
    pub g: f32,
    pub b: f32,
    pub a: f32,
}

impl Color {
    pub const TRANSPARENT: Self = Self {
        r: 0.0,
        g: 0.0,
        b: 0.0,
        a: 0.0,
    };
    pub const WHITE: Self = Self {
        r: 1.0,
        g: 1.0,
        b: 1.0,
        a: 1.0,
    };
    pub const BLACK: Self = Self {
        r: 0.0,
        g: 0.0,
        b: 0.0,
        a: 1.0,
    };

    pub const RED: Self = Self {
        r: 1.0,
        g: 0.0,
        b: 0.0,
        a: 1.0,
    };
    pub const GREEN: Self = Self {
        r: 0.0,
        g: 1.0,
        b: 0.0,
        a: 1.0,
    };
    pub const BLUE: Self = Self {
        r: 0.0,
        g: 0.0,
        b: 1.0,
        a: 1.0,
    };
    pub const YELLOW: Self = Self {
        r: 1.0,
        g: 1.0,
        b: 0.0,
        a: 1.0,
    };
    pub const ORANGE: Self = Self {
        r: 1.0,
        g: 0.5,
        b: 0.0,
        a: 1.0,
    };
    pub const PURPLE: Self = Self {
        r: 0.5,
        g: 0.0,
        b: 0.5,
        a: 1.0,
    };
    pub const CYAN: Self = Self {
        r: 0.0,
        g: 1.0,
        b: 1.0,
        a: 1.0,
    };
    pub const MAGENTA: Self = Self {
        r: 1.0,
        g: 0.0,
        b: 1.0,
        a: 1.0,
    };
    pub const GRAY: Self = Self {
        r: 0.5,
        g: 0.5,
        b: 0.5,
        a: 1.0,
    };
    pub const LIGHT_GRAY: Self = Self {
        r: 0.75,
        g: 0.75,
        b: 0.75,
        a: 1.0,
    };
    pub const DARK_GRAY: Self = Self {
        r: 0.25,
        g: 0.25,
        b: 0.25,
        a: 1.0,
    };

    /// GPU/シェーダー用の 0.0~1.0 (f32) 値から直接生成します
    #[inline]
    pub const fn rgb_f32(r: f32, g: f32, b: f32) -> Self {
        Self { r, g, b, a: 1.0 }
    }

    /// GPU/シェーダー用の 0.0~1.0 (f32) 値から直接生成します
    #[inline]
    pub const fn rgba_f32(r: f32, g: f32, b: f32, a: f32) -> Self {
        Self { r, g, b, a }
    }

    /// 色味を維持したまま、不透明度（アルファ）だけを動的に書き換えます
    #[inline]
    pub const fn with_alpha(self, a: f32) -> Self {
        Self { a, ..self }
    }

    /// HSL モデル（Hue: 0..360, Saturation: 0..1, Lightness: 0..1）から Color を生成します
    #[inline]
    pub fn hsl(h: f32, s: f32, l: f32) -> Self {
        Self::hsla(h, s, l, 1.0)
    }

    /// HSL モデルにアルファ（0.0..1.0）を付与して Color を生成します
    pub fn hsla(h: f32, s: f32, l: f32, a: f32) -> Self {
        // 色相（h）を 0..360 の範囲に正規化
        let h_mod = (h % 360.0 + 360.0) % 360.0;
        let s = s.clamp(0.0, 1.0);
        let l = l.clamp(0.0, 1.0);

        let c = (1.0 - (2.0 * l - 1.0).abs()) * s;
        let x = c * (1.0 - ((h_mod / 60.0) % 2.0 - 1.0).abs());
        let m = l - c / 2.0;

        let (r, g, b) = if h_mod < 60.0 {
            (c, x, 0.0)
        } else if h_mod < 120.0 {
            (x, c, 0.0)
        } else if h_mod < 180.0 {
            (0.0, c, x)
        } else if h_mod < 240.0 {
            (0.0, x, c)
        } else if h_mod < 300.0 {
            (x, 0.0, c)
        } else {
            (c, 0.0, x)
        };

        Self {
            r: r + m,
            g: g + m,
            b: b + m,
            a,
        }
    }
}

/// 0~255 の整数値（u8）で、不透明な RGB カラーを生成します
#[inline]
pub fn rgb(r: u8, g: u8, b: u8) -> Color {
    Color {
        r: r as f32 / 255.0,
        g: g as f32 / 255.0,
        b: b as f32 / 255.0,
        a: 1.0,
    }
}

/// 0~255 の整数値（u8）でRGBを、0.0~1.0（f32）で不透明度（Alpha）を指定して RGBA カラーを生成します
#[inline]
pub fn rgba(r: u8, g: u8, b: u8, a: f32) -> Color {
    Color {
        r: r as f32 / 255.0,
        g: g as f32 / 255.0,
        b: b as f32 / 255.0,
        a,
    }
}

/// 6桁（RRGGBB、例：0x191919）または8桁（RRGGBBAA、例：0x19191980）のHEX値からカラーを自動解析して生成します
#[inline]
pub fn hex(value: u32) -> Color {
    // 0xFFFFFF (最大白の6桁) 以下であるかどうかで、6桁か8桁かを自動判定
    if value <= 0xFFFFFF {
        // 6桁カラー (RRGGBB): アルファ 1.0 固定
        let r = ((value >> 16) & 0xFF) as f32 / 255.0;
        let g = ((value >> 8) & 0xFF) as f32 / 255.0;
        let b = (value & 0xFF) as f32 / 255.0;
        Color { r, g, b, a: 1.0 }
    } else {
        // 8桁カラー (RRGGBBAA): 末尾のAAをアルファにマッピング
        let r = ((value >> 24) & 0xFF) as f32 / 255.0;
        let g = ((value >> 16) & 0xFF) as f32 / 255.0;
        let b = ((value >> 8) & 0xFF) as f32 / 255.0;
        let a = (value & 0xFF) as f32 / 255.0;
        Color { r, g, b, a }
    }
}

/// HSL（Hue: 0..360, Saturation: 0.0..1.0, Lightness: 0.0..1.0）カラーを生成するショートハンド
#[inline]
pub fn hsl(h: f32, s: f32, l: f32) -> Color {
    Color::hsl(h, s, l)
}

/// HSL にアルファ（0.0..1.0）を付与して HSLA カラーを生成するショートハンド
#[inline]
pub fn hsla(h: f32, s: f32, l: f32, a: f32) -> Color {
    Color::hsla(h, s, l, a)
}

#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Default, Pod, Zeroable)]
pub struct LayoutPoint {
    pub x: f32,
    pub y: f32,
}

impl LayoutPoint {
    pub const ZERO: Self = Self { x: 0.0, y: 0.0 };

    #[inline]
    pub const fn new(x: f32, y: f32) -> Self {
        Self { x, y }
    }
}

#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Default, Pod, Zeroable)]
pub struct LayoutSize {
    pub width: f32,
    pub height: f32,
}

impl LayoutSize {
    pub const ZERO: Self = Self {
        width: 0.0,
        height: 0.0,
    };

    #[inline]
    pub const fn new(width: f32, height: f32) -> Self {
        Self { width, height }
    }
}

#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Default, Pod, Zeroable)]
pub struct LayoutRect {
    pub x: f32,
    pub y: f32,
    pub width: f32,
    pub height: f32,
}

impl LayoutRect {
    pub const ZERO: Self = Self {
        x: 0.0,
        y: 0.0,
        width: 0.0,
        height: 0.0,
    };

    #[inline]
    pub const fn new(x: f32, y: f32, width: f32, height: f32) -> Self {
        Self {
            x,
            y,
            width,
            height,
        }
    }

    /// Determine if the mouse coordinates, etc., are included within this rectangle.
    #[inline]
    pub fn contains(&self, point: LayoutPoint) -> bool {
        // 幅または高さが 0 以下の場合は、当たり判定を即座に却下する
        if self.width <= 0.0 || self.height <= 0.0 {
            return false;
        }

        point.x >= self.x
            && point.x <= self.x + self.width
            && point.y >= self.y
            && point.y <= self.y + self.height
    }

    /// 2つの矩形が交差する領域（共通部分）を計算して返す。
    ///
    /// 共通部分が存在しない（はみ出している・離れている）場合は、
    /// 幅（width）と高さ（height）が `0.0` の空の `Rect` を返す。
    #[inline]
    pub fn intersect(&self, other: &Self) -> Self {
        // 交差領域の左上座標（最大値をとる）
        let x1 = self.x.max(other.x);
        let y1 = self.y.max(other.y);

        // 交差領域の右下座標（最小値をとる）
        let x2 = (self.x + self.width).min(other.x + other.width);
        let y2 = (self.y + self.height).min(other.y + other.height);

        // 幅と高さが負数（離れている状態）になった場合は max(0.0) で 0.0 に丸める
        let width = (x2 - x1).max(0.0);
        let height = (y2 - y1).max(0.0);

        Self {
            x: x1,
            y: y1,
            width,
            height,
        }
    }
}

#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Default, Pod, Zeroable)]
pub struct EdgeInsets {
    pub top: f32,
    pub right: f32,
    pub bottom: f32,
    pub left: f32,
}

impl EdgeInsets {
    pub const ZERO: Self = Self {
        top: 0.0,
        right: 0.0,
        bottom: 0.0,
        left: 0.0,
    };

    /// 4方向それぞれを物理ピクセルで個別に指定して生成します
    #[inline]
    pub const fn px(top: f32, right: f32, bottom: f32, left: f32) -> Self {
        Self {
            top,
            right,
            bottom,
            left,
        }
    }

    /// 4方向すべてを一括で同じ物理ピクセルに指定します
    #[inline]
    pub const fn px_all(value: f32) -> Self {
        Self {
            top: value,
            right: value,
            bottom: value,
            left: value,
        }
    }

    /// 上下・左右をそれぞれ物理ピクセルで対称指定します
    #[inline]
    pub const fn px_sym(vertical: f32, horizontal: f32) -> Self {
        Self {
            top: vertical,
            right: horizontal,
            bottom: vertical,
            left: horizontal,
        }
    }
}

#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Default, Pod, Zeroable)]
pub struct CornerRadius {
    pub top_left: f32,
    pub top_right: f32,
    pub bottom_right: f32,
    pub bottom_left: f32,
}

impl CornerRadius {
    pub const ZERO: Self = Self {
        top_left: 0.0,
        top_right: 0.0,
        bottom_right: 0.0,
        bottom_left: 0.0,
    };

    #[inline]
    pub const fn radius(
        top_left: f32,
        top_right: f32,
        bottom_right: f32,
        bottom_left: f32,
    ) -> Self {
        Self {
            top_left,
            top_right,
            bottom_right,
            bottom_left,
        }
    }

    /// 上下対称、または左右対称に角丸を生成します
    /// (例: symmetric(12.0, 4.0) で上が大きく、下が穏やかな丸みになります)
    #[inline]
    pub const fn symmetric(vertical: f32, horizontal: f32) -> Self {
        Self {
            top_left: vertical,
            top_right: vertical,
            bottom_right: horizontal,
            bottom_left: horizontal,
        }
    }

    /// All four corners have the same rounded shape.
    #[inline]
    pub const fn all(radius: f32) -> Self {
        Self {
            top_left: radius,
            top_right: radius,
            bottom_right: radius,
            bottom_left: radius,
        }
    }
}

/// 影（BoxShadow）の表現
#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Pod, Zeroable)]
pub struct BoxShadow {
    pub offset: LayoutPoint,
    pub blur: f32,
    pub spread: f32,
    pub color: Color,
}

impl Default for BoxShadow {
    #[inline]
    fn default() -> Self {
        Self {
            offset: LayoutPoint::ZERO,
            blur: 0.0,
            spread: 0.0,
            color: Color::BLACK, // デフォルトは黒
        }
    }
}

impl BoxShadow {
    /// 新しいデフォルトの影設定を生成します。
    #[inline]
    pub const fn new() -> Self {
        Self {
            offset: LayoutPoint::ZERO,
            blur: 0.0,
            spread: 0.0,
            color: Color::BLACK,
        }
    }

    /// 影のオフセット（x, y）を設定します。単一値（例: 5）やタプル（例: (0, 4)）を受け入れます。
    #[inline]
    pub fn offset(mut self, value: impl IntoLayoutPoint) -> Self {
        self.offset = value.into_layout_point();
        self
    }

    /// 影のぼかし（blur）幅を設定します。
    #[inline]
    pub fn blur(mut self, value: impl Convert<f32>) -> Self {
        self.blur = value.convert();
        self
    }

    /// 影の広がり（spread）幅を設定します。
    #[inline]
    pub fn spread(mut self, value: impl Convert<f32>) -> Self {
        self.spread = value.convert();
        self
    }

    /// 影のカラーを設定します。
    #[inline]
    pub fn color(mut self, value: Color) -> Self {
        self.color = value;
        self
    }

    /// 控えめな極小のソフトシャドウ
    pub fn sm() -> Self {
        BoxShadow::new()
            .blur(2)
            .color(rgba(0, 0, 0, 0.05))
            .offset((0, 1))
    }

    /// 標準的な中程度のソフトシャドウ
    pub fn md() -> Self {
        BoxShadow::new()
            .blur(6)
            .spread(-1)
            .color(rgba(0, 0, 0, 0.1))
            .offset((0, 4))
    }

    /// やや浮き上がって見える大きめのソフトシャドウ
    pub fn lg() -> Self {
        BoxShadow::new()
            .blur(15)
            .spread(-3)
            .color(rgba(0, 0, 0, 0.1))
            .offset((0, 10))
    }

    pub fn none() -> Self {
        Self {
            offset: LayoutPoint::ZERO,
            blur: 0.0,
            spread: 0.0,
            color: Color::TRANSPARENT,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Hash)]
pub struct Size<T> {
    pub width: T,
    pub height: T,
}

impl<T> Size<T> {
    #[inline]
    pub const fn new(width: T, height: T) -> Self {
        Self { width, height }
    }
}

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

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Hash)]
pub struct Rect<T> {
    pub top: T,
    pub right: T,
    pub bottom: T,
    pub left: T,
}

impl<T> Rect<T> {
    #[inline]
    pub const fn new(top: T, right: T, bottom: T, left: T) -> Self {
        Self {
            top,
            right,
            bottom,
            left,
        }
    }
}

impl<T: Clone> Rect<T> {
    #[inline]
    pub fn all(value: T) -> Self {
        Self {
            top: value.clone(),
            right: value.clone(),
            bottom: value.clone(),
            left: value,
        }
    }

    #[inline]
    pub fn symmetric(vertical: T, horizontal: T) -> Self {
        Self {
            top: vertical.clone(),
            right: horizontal.clone(),
            bottom: vertical,
            left: horizontal,
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

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Hash)]
pub struct Point<T> {
    pub x: T,
    pub y: T,
}

impl<T> Point<T> {
    #[inline]
    pub const fn new(x: T, y: T) -> Self {
        Self { x, y }
    }
}

impl<T: Clone> Point<T> {
    #[inline]
    pub fn all(value: T) -> Self {
        Self {
            x: value.clone(),
            y: value,
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

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Length {
    Px(f32),
    Percent(f32),
}

impl From<Length> for f32 {
    fn from(length: Length) -> Self {
        match length {
            Length::Px(val) => val,
            Length::Percent(val) => val,
        }
    }
}

impl Length {
    #[inline]
    pub fn px(px: f32) -> Self {
        Self::Px(px)
    }

    #[inline]
    pub fn pct(percent: f32) -> Self {
        Self::Percent(percent)
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

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Val {
    Auto,
    Px(f32),
    Percent(f32),
}

impl Val {
    #[inline]
    pub fn auto() -> Self {
        Self::Auto
    }

    #[inline]
    pub fn px(px: f32) -> Self {
        Self::Px(px)
    }

    #[inline]
    pub fn pct(percent: f32) -> Self {
        Self::Percent(percent)
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

#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub enum Display {
    #[default]
    Flex,
    Grid,
    Block,
    None,
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

#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub enum Position {
    #[default]
    Relative,
    Absolute,
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

#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub enum BoxSizing {
    #[default]
    BorderBox,
    ContentBox,
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

#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub enum Direction {
    #[default]
    Ltr,
    Rtl,
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

#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub enum Overflow {
    #[default]
    Visible,
    Hidden,
    Scroll,
    Clip,
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

#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct LayoutOverflow {
    pub x: Overflow,
    pub y: Overflow,
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

#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub enum FlexDirection {
    #[default]
    Row,
    Column,
    RowReverse,
    ColumnReverse,
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

#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub enum FlexWrap {
    #[default]
    NoWrap,
    Wrap,
    WrapReverse,
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
            taffy::AlignItems::STRETCH => Self::Stretch,
            taffy::AlignItems::SAFE_START => Self::SafeStart,
            taffy::AlignItems::SAFE_END => Self::SafeEnd,
            taffy::AlignItems::SAFE_FLEX_START => Self::SafeFlexStart,
            taffy::AlignItems::SAFE_FLEX_END => Self::SafeFlexEnd,
            taffy::AlignItems::SAFE_CENTER => Self::SafeCenter,
            _ => Self::Stretch,
        }
    }
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
            taffy::AlignSelf::STRETCH => Self::Stretch,
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
            taffy::JustifyContent::CENTER => Self::Center,
            taffy::JustifyContent::STRETCH => Self::Stretch,
            taffy::JustifyContent::SPACE_BETWEEN => Self::SpaceBetween,
            taffy::JustifyContent::SPACE_AROUND => Self::SpaceAround,
            taffy::JustifyContent::SPACE_EVENLY => Self::SpaceEvenly,
            taffy::JustifyContent::SAFE_START => Self::SafeStart,
            taffy::JustifyContent::SAFE_END => Self::SafeEnd,
            taffy::JustifyContent::SAFE_FLEX_START => Self::SafeFlexStart,
            taffy::JustifyContent::SAFE_FLEX_END => Self::SafeFlexEnd,
            taffy::JustifyContent::FLEX_START => Self::FlexStart,
            taffy::JustifyContent::FLEX_END => Self::FlexEnd,
            taffy::JustifyContent::SAFE_CENTER => Self::Center,
            _ => Self::Stretch,
        }
    }
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
            taffy::AlignContent::CENTER => Self::Center,
            taffy::AlignContent::STRETCH => Self::Stretch,
            taffy::AlignContent::SPACE_BETWEEN => Self::SpaceBetween,
            taffy::AlignContent::SPACE_AROUND => Self::SpaceAround,
            taffy::AlignContent::SPACE_EVENLY => Self::SpaceEvenly,
            taffy::AlignContent::SAFE_START => Self::SafeStart,
            taffy::AlignContent::SAFE_END => Self::SafeEnd,
            taffy::AlignContent::SAFE_FLEX_START => Self::SafeFlexStart,
            taffy::AlignContent::SAFE_FLEX_END => Self::SafeFlexEnd,
            taffy::AlignContent::FLEX_START => Self::FlexStart,
            taffy::AlignContent::FLEX_END => Self::FlexEnd,
            taffy::AlignContent::SAFE_CENTER => Self::Center,
            _ => Self::Stretch,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub enum TextAlign {
    #[default]
    Auto,
    Left,
    Right,
    Center,
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

#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub enum GridAutoFlow {
    #[default]
    Row,
    Column,
    RowDense,
    ColumnDense,
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
                    // GridLine の直接インポートを避けるため型推論に委ねる
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
            // Taffy 0.11 の NamedSpan(S, u16) の引数順序に適合
            GridPlacement::NamedSpan(s) => Self::NamedSpan(s.to_string(), 1),
        }
    }
}

impl From<taffy::GridPlacement<String>> for GridPlacement<String> {
    #[inline]
    fn from(t: taffy::GridPlacement<String>) -> Self {
        match t {
            taffy::GridPlacement::Auto => Self::Auto,
            // 【解決策】型名を明示せず、grid_line に実装されている .as_i16() を直接呼び出し
            taffy::GridPlacement::Line(grid_line) => Self::Line(grid_line.as_i16().to_string()),
            taffy::GridPlacement::NamedLine(s, _val) => Self::NamedLine(s),
            taffy::GridPlacement::Span(val) => Self::Span(val.to_string()),
            taffy::GridPlacement::NamedSpan(s, _val) => Self::NamedSpan(s),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct GridLine<T> {
    pub start: T,
    pub end: T,
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

// types.rs に追加

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Transform {
    pub(crate) matrix: [[f32; 4]; 4],
}

impl Default for Transform {
    fn default() -> Self {
        Self::new()
    }
}

impl Transform {
    /// 単位行列（初期状態）を生成
    #[inline]
    pub fn new() -> Self {
        Self {
            matrix: [
                [1.0, 0.0, 0.0, 0.0],
                [0.0, 1.0, 0.0, 0.0],
                [0.0, 0.0, 1.0, 0.0],
                [0.0, 0.0, 0.0, 1.0],
            ],
        }
    }

    /// 平行移動
    #[inline]
    pub fn translate(self, x: f32, y: f32) -> Self {
        let mut t = Self::new();
        t.matrix[3][0] = x;
        t.matrix[3][1] = y;
        self.mul(&t)
    }

    /// 拡大縮小
    #[inline]
    pub fn scale(self, x: f32, y: f32) -> Self {
        let mut s = Self::new();
        s.matrix[0][0] = x;
        s.matrix[1][1] = y;
        self.mul(&s)
    }

    /// Z軸（2D平面上）の回転（ラジアン）
    #[inline]
    pub fn rotate(self, radians: f32) -> Self {
        let mut r = Self::new();
        let cos = radians.cos();
        let sin = radians.sin();
        r.matrix[0][0] = cos;
        r.matrix[0][1] = sin;
        r.matrix[1][0] = -sin;
        r.matrix[1][1] = cos;
        self.mul(&r)
    }

    /// 4x4 行列の乗算処理（列優先 / Column-Major 対応）
    #[allow(clippy::needless_range_loop)]
    fn mul(&self, other: &Self) -> Self {
        let mut out = [[0.0; 4]; 4];
        for i in 0..4 {
            for j in 0..4 {
                out[i][j] = self.matrix[i][0] * other.matrix[0][j]
                    + self.matrix[i][1] * other.matrix[1][j]
                    + self.matrix[i][2] * other.matrix[2][j]
                    + self.matrix[i][3] * other.matrix[3][j];
            }
        }
        Self { matrix: out }
    }
}

/// 特定のスタイル変更を滑らかに補間する設定
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Transition {
    /// 状態遷移を設定できる ComponentMask (例: STYLE_BG_COLOR | STYLE_OPACITY)
    pub property_list: PropertyList,
    /// アニメーションの時間
    pub duration: Duration,
    /// イージングカーブ
    pub curve: AnimationCurve,
}

impl Transition {
    pub fn new(property_list: PropertyList, duration: Duration, curve: AnimationCurve) -> Self {
        Self {
            property_list,
            duration,
            curve,
        }
    }

    pub fn property_list(mut self, property_list: PropertyList) -> Self {
        self.property_list = property_list;
        self
    }

    pub fn duration(mut self, duration: Duration) -> Self {
        self.duration = duration;
        self
    }

    pub fn curve(mut self, curve: AnimationCurve) -> Self {
        self.curve = curve;
        self
    }
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
    pub fn new(start_color: Color, end_color: Color, angle_degrees: f32) -> Self {
        Self {
            start_color,
            end_color,
            angle: angle_degrees.to_radians(),
            _padding: [0.0; 3],
        }
    }
}

/// ポインターイベント（マウスインタラクション）の透過制御
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum PointerEvents {
    /// 通常通りポインターイベントを受け取り下にある要素に透過させない
    #[default]
    Auto,
    /// ポインターイベントを無視し、下にある要素へ透過させる (CSS の pointer-events: none 相当)
    None,
}

// 1. BasicLayout (基本レイアウト：18プロパティ) - ホットデータ
/// 要素がほぼ必ず持つ、基本のレイアウト情報。
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct BasicLayout {
    pub display: Display,
    pub item_is_table: bool,
    pub item_is_replaced: bool,
    pub box_sizing: BoxSizing,
    pub direction: Direction,
    pub overflow: LayoutOverflow,
    pub position: Position,
    pub inset: Rect<Val>,
    pub size: Size<Val>,
    pub min_size: Size<Val>,
    pub max_size: Size<Val>,
    pub aspect_ratio: Option<f32>,
    pub margin: Rect<Val>,
    pub padding: Rect<Length>,
    pub border: Rect<Length>,
}

impl Default for BasicLayout {
    fn default() -> Self {
        // Taffy のデフォルトの Style から基本設定値をコピーして初期化
        let default_style: taffy::Style = taffy::Style::default();
        Self {
            display: default_style.display.into(),
            item_is_table: default_style.item_is_table,
            item_is_replaced: default_style.item_is_replaced,
            box_sizing: default_style.box_sizing.into(),
            direction: default_style.direction.into(),
            overflow: default_style.overflow.into(),
            position: default_style.position.into(),
            inset: default_style.inset.into(),
            size: default_style.size.into(),
            min_size: default_style.min_size.into(),
            max_size: default_style.max_size.into(),
            aspect_ratio: default_style.aspect_ratio,
            margin: default_style.margin.into(),
            padding: default_style.padding.into(),
            border: default_style.border.into(),
        }
    }
}

impl BasicLayout {
    /// 指定されたプロパティマスクに基づいて、自身を別のレイアウトデータで上書きします。
    pub(crate) fn override_with(&mut self, other: &Self, mask: ComponentMask) {
        if mask.has(STYLE_DISPLAY) {
            self.display = other.display;
        }
        if mask.has(STYLE_ITEM_IS_TABLE) {
            self.item_is_table = other.item_is_table;
        }
        if mask.has(STYLE_ITEM_IS_REPLACED) {
            self.item_is_replaced = other.item_is_replaced;
        }
        if mask.has(STYLE_BOX_SIZING) {
            self.box_sizing = other.box_sizing;
        }
        if mask.has(STYLE_DIRECTION) {
            self.direction = other.direction;
        }
        if mask.has(STYLE_OVERFLOW) {
            self.overflow = other.overflow;
        }
        if mask.has(STYLE_POSITION) {
            self.position = other.position;
        }
        if mask.has(STYLE_INSET) {
            self.inset = other.inset;
        }
        if mask.has(STYLE_SIZE) {
            self.size = other.size;
        }
        if mask.has(STYLE_MIN_SIZE) {
            self.min_size = other.min_size;
        }
        if mask.has(STYLE_MAX_SIZE) {
            self.max_size = other.max_size;
        }
        if mask.has(STYLE_ASPECT_RATIO) {
            self.aspect_ratio = other.aspect_ratio;
        }
        if mask.has(STYLE_MARGIN) {
            self.margin = other.margin;
        }
        if mask.has(STYLE_PADDING) {
            self.padding = other.padding;
        }
        if mask.has(STYLE_BORDER) {
            self.border = other.border;
        }
    }
}

// 2. FlexLayout (Flexboxレイアウト：13プロパティ) - ホットデータ
/// Flexboxコンテナ、またはその子要素に適用される情報。
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct FlexLayout {
    pub align_items: Option<AlignItems>,
    pub align_self: Option<AlignSelf>,
    pub justify_items: Option<AlignItems>,
    pub justify_self: Option<AlignSelf>,
    pub align_content: Option<AlignContent>,
    pub justify_content: Option<JustifyContent>,
    pub gap: Size<Val>,
    pub text_align: TextAlign,
    pub flex_direction: FlexDirection,
    pub flex_wrap: FlexWrap,
    pub flex_basis: Val,
    pub flex_grow: f32,
    pub flex_shrink: f32,
}

impl FlexLayout {
    /// 指定されたプロパティマスクに基づいて、自身を別のFlexレイアウトデータで上書きします。
    pub(crate) fn override_with(&mut self, other: &Self, mask: ComponentMask) {
        if mask.has(STYLE_ALIGN_ITEMS) {
            self.align_items = other.align_items;
        }
        if mask.has(STYLE_ALIGN_SELF) {
            self.align_self = other.align_self;
        }
        if mask.has(STYLE_JUSTIFY_ITEMS) {
            self.justify_items = other.justify_items;
        }
        if mask.has(STYLE_JUSTIFY_SELF) {
            self.justify_self = other.justify_self;
        }
        if mask.has(STYLE_ALIGN_CONTENT) {
            self.align_content = other.align_content;
        }
        if mask.has(STYLE_JUSTIFY_CONTENT) {
            self.justify_content = other.justify_content;
        }
        if mask.has(STYLE_GAP) {
            self.gap = other.gap;
        }
        if mask.has(STYLE_TEXT_ALIGN) {
            self.text_align = other.text_align;
        }
        if mask.has(STYLE_FLEX_DIRECTION) {
            self.flex_direction = other.flex_direction;
        }
        if mask.has(STYLE_FLEX_WRAP) {
            self.flex_wrap = other.flex_wrap;
        }
        if mask.has(STYLE_FLEX_BASIS) {
            self.flex_basis = other.flex_basis;
        }
        if mask.has(STYLE_FLEX_GROW) {
            self.flex_grow = other.flex_grow;
        }
        if mask.has(STYLE_FLEX_SHRINK) {
            self.flex_shrink = other.flex_shrink;
        }
    }
}

impl Default for FlexLayout {
    fn default() -> Self {
        let default_style: taffy::Style = taffy::Style::default();
        Self {
            align_items: default_style.align_items.map(|a| a.into()),
            align_self: default_style.align_self.map(|a| a.into()),
            justify_items: default_style.justify_items.map(|a| a.into()),
            justify_self: default_style.justify_self.map(|a| a.into()),
            align_content: default_style.align_content.map(|a| a.into()),
            justify_content: default_style.justify_content.map(|a| a.into()),
            gap: default_style.gap.into(),
            text_align: default_style.text_align.into(),
            flex_direction: default_style.flex_direction.into(),
            flex_wrap: default_style.flex_wrap.into(),
            flex_basis: default_style.flex_basis.into(),
            flex_grow: default_style.flex_grow,
            flex_shrink: default_style.flex_shrink,
        }
    }
}

// 3. GridLayout (Gridレイアウト：10プロパティ) - コールドデータ
/// Gridコンテナ、およびGrid子要素に適用される複雑な情報。
/// `Vec`（動的配列）を多数含み、メモリ上で重いため `Copy` は不可能（`Clone` のみ）。
/// 滅多に使われないため、`SparseSecondaryMap` で隔離し、未使用の要素には一切メモリを消費させない。
#[derive(Debug, Clone, PartialEq)]
pub struct GridLayout {
    pub grid_template_rows: Vec<taffy::GridTemplateComponent<String>>,
    pub grid_template_columns: Vec<taffy::GridTemplateComponent<String>>,
    pub grid_auto_rows: Vec<taffy::TrackSizingFunction>,
    pub grid_auto_columns: Vec<taffy::TrackSizingFunction>,
    pub grid_auto_flow: GridAutoFlow,
    pub grid_template_areas: Vec<taffy::GridTemplateArea<String>>,
    pub grid_template_column_names: Vec<Vec<String>>,
    pub grid_template_row_names: Vec<Vec<String>>,
    pub grid_row: GridLine<GridPlacement<String>>,
    pub grid_column: GridLine<GridPlacement<String>>,
}

impl Default for GridLayout {
    fn default() -> Self {
        // Taffyデフォルト値と完全に一致するように空のVecで初期化
        Self {
            grid_template_rows: Vec::new(),
            grid_template_columns: Vec::new(),
            grid_auto_rows: Vec::new(),
            grid_auto_columns: Vec::new(),
            grid_auto_flow: GridAutoFlow::Row,
            grid_template_areas: Vec::new(),
            grid_template_column_names: Vec::new(),
            grid_template_row_names: Vec::new(),
            grid_row: GridLine {
                start: GridPlacement::Auto,
                end: GridPlacement::Auto,
            },
            grid_column: GridLine {
                start: GridPlacement::Auto,
                end: GridPlacement::Auto,
            },
        }
    }
}

/// 要素の描画（ペイント）に関する一括情報。
/// wgpu への高速インスタンスバッファ転送時に、時間・空間的キャッシュ局所性を最大化するために一塊で管理。
#[derive(Debug, Clone, Default, PartialEq)]
pub struct VisualProperty {
    pub bg_color: Option<Color>,
    pub border_color: Option<Color>,
    pub border_lengths: Option<EdgeInsets>,
    pub border_styles: Option<[BorderStyle; 4]>,
    pub border_alignments: Option<[BorderAlignment; 4]>,
    pub corner_radius: Option<CornerRadius>,
    pub opacity: Option<f32>,
    pub shadow_params: Option<BoxShadow>,
    pub shadow_color: Option<Color>,
    pub transform: Option<[[f32; 4]; 4]>,
    pub transform_origin: Option<Point<f32>>,
    pub z_index: Option<i32>,
    pub cursor: Option<CursorIcon>,
    pub backdrop: Backdrop,
    pub text_color: Option<Color>,
    pub font_size: Option<f32>,
    pub font_family: Option<Cow<'static, str>>,
    pub font_weight: Option<u32>,
    pub font_style: Option<u32>,
    pub bg_gradient: Option<LinearGradient>,
    pub transitions: Vec<Transition>,
    pub keyframe_animations: Vec<KeyframeAnimation>,
    pub pointer_events: Option<PointerEvents>,
    pub user_select: Option<UserSelect>,
    pub select_bg_color: Option<Color>,
    pub select_text_color: Option<Color>,
}

/// 子要素から伝播して解決可能なインタラクション定義
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum InteractionName {
    Hover,
    Focus,
    Press,
    Disable,
    Active,
    Select,
    Drag,
    All,
}

/// インタラクション（動的状態）ごとにオーバーライドして適用される、追加のスタイル表現。
/// 滅多に使われない、かつ再帰的な構造を持つため、StyleInner の直下ではなくこの構造体に隠蔽して管理。
#[derive(Debug, Clone, Default)]
pub(crate) struct InteractionStyles {
    pub(crate) hovered: Option<ThisStyle>,
    pub(crate) focused: Option<ThisStyle>,
    pub(crate) pressed: Option<ThisStyle>,
    pub(crate) disabled: Option<ThisStyle>,
    pub(crate) actived: Option<ThisStyle>,
    pub(crate) selected: Option<ThisStyle>,
    pub(crate) dragged: Option<ThisStyle>,

    pub(crate) hovered_within: Option<ThisStyle>,
    pub(crate) focused_within: Option<ThisStyle>,
    pub(crate) pressed_within: Option<ThisStyle>,
    pub(crate) disabled_within: Option<ThisStyle>,
    pub(crate) actived_within: Option<ThisStyle>,
    pub(crate) selected_within: Option<ThisStyle>,
    pub(crate) dragged_within: Option<ThisStyle>,
    pub(crate) any_within: Option<ThisStyle>, // All（いずれかのインタラクションがあればON）
}

/// 実行時にウィンドウ内で現在アクティブ（排他的）になっている、各状態の対象要素（EntityId）を管理します。
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct InteractionStates {
    pub hovered: Option<EntityId>,
    pub focused: Option<EntityId>,
    pub pressed: Option<EntityId>,
    pub dragged: Option<EntityId>,
}

impl InteractionStates {
    pub fn new() -> Self {
        Self::default()
    }
}

pub type ClickCallback = Box<dyn FnMut(&mut Context) + 'static>;
pub type MouseCallback =
    Box<dyn FnMut(&mut Context, MouseButton, Modifiers, ElementState) + 'static>;
pub type CursorMovedCallback = Box<dyn FnMut(&mut Context, LayoutPoint) + 'static>;
pub type MouseWheelCallback = Box<dyn FnMut(&mut Context, f32, f32) + 'static>;
pub type DragCallback = Box<dyn FnMut(&mut Context, LayoutPoint) + 'static>;
pub type KeyCallback = Box<dyn FnMut(&mut Context, VirtualKey, Modifiers, ElementState) + 'static>;
pub type CharCallback = Box<dyn FnMut(&mut Context, char) + 'static>;
pub type ImeCallback = Box<dyn FnMut(&mut Context, ImeState) + 'static>;
pub type FileDropCallback = Box<dyn FnMut(&mut Context, Vec<PathBuf>) + 'static>;
pub type FileDragCallback = Box<dyn FnMut(&mut Context) + 'static>;
pub type ImageLoadedCallback = Box<dyn FnMut(&mut Context, ImageMetadata) + 'static>;
pub type MediaOpenedCallback = Box<dyn FnMut(&mut Context, MovieMetadata) + 'static>;
pub type SimpleCallback = Box<dyn FnMut(&mut Context) + 'static>;

/// 要素ごとにバインドされる、検証済みイベントのハンドラ群。
#[derive(Default)]
pub(crate) struct EventListeners {
    /// 要素がクリックされた（マウスダウン ➔ 同一要素上でマウスアップされた）際のコールバック
    pub(crate) on_click: Option<ClickCallback>,

    /// 右クリックされた際のコールバック（コンテキストメニューの起動用など）
    pub(crate) on_right_click: Option<SimpleCallback>,

    /// マウスボタンの押し下げ・離しを直接受け取るハンドラ
    /// 引数: (ボタンの種類, 装飾キーの状態, 押し下げ/離し状態)
    pub(crate) on_mouse_input: Option<MouseCallback>,

    /// マウスカーソルがこの要素の可視境界（clip_rects）に入った際のイベント
    pub(crate) on_mouse_enter: Option<SimpleCallback>,

    /// マウスカーソルがこの要素の可視境界から外に出た際のイベント
    pub(crate) on_mouse_leave: Option<SimpleCallback>,

    /// マウスカーソルが要素内で動いた際のイベント。
    /// 引数: 要素の左上を (0.0, 0.0) とする、論理スケーリング済みの相対座標 `Point`
    pub(crate) on_cursor_moved: Option<CursorMovedCallback>,

    /// マウスホイールが回された際のイベント（WM_MOUSEWHEEL / WM_MOUSEHWHEEL 互換）
    /// 引数: 前方向ならプラス、後方向ならマイナスの移動量（delta）
    pub(crate) on_mouse_wheel: Option<MouseWheelCallback>,

    /// 要素がドラッグされている最中のイベント（スライダーノブやDND用）
    /// 引数: ドラッグによる移動量 `Point(delta_x, delta_y)`
    pub(crate) on_drag: Option<DragCallback>,

    /// 物理キーボードが押された、または離された際のイベント
    /// 引数: 検証済みの仮想キーコード, 装飾キー, 状態
    pub(crate) on_keyboard_input: Option<KeyCallback>,

    /// IMEなどを介さない、確定した1文字の文字入力イベント（WM_CHAR 互換）
    pub(crate) on_char_input: Option<CharCallback>,

    /// IME（TSF / Input Method）による未確定文字の入力や確定が行われた際のイベント
    /// 引数: 検証済みのIME状態アップデート情報
    pub(crate) on_ime: Option<ImeCallback>,

    /// 外部のファイルやフォルダがこの要素の上にドラッグ＆ドロップされた際のイベント
    /// 引数: 検証済みのファイルパスの配列
    pub(crate) on_file_dropped: Option<FileDropCallback>,
    /// ファイルが要素の可視境界内にドラッグされて入ってきた瞬間に発火します（ドロップゾーンの強調表示用）
    pub(crate) on_file_drag_enter: Option<FileDragCallback>,
    /// ドラッグされていたファイルが要素の外に出た、またはドラッグがキャンセルされた瞬間に発火します
    pub(crate) on_file_drag_leave: Option<FileDragCallback>,
    /// 画像のデコード・ロードが完了し、メタデータが確定した瞬間に発火します
    pub(crate) on_image_loaded: Option<ImageLoadedCallback>,
    /// 動画等のメディアファイルのロードが完了し、メタデータが確定した瞬間に発火します
    pub(crate) on_media_loaded: Option<MediaOpenedCallback>,

    pub(crate) on_hover: Option<SimpleCallback>,
    pub(crate) on_focus: Option<SimpleCallback>,
    pub(crate) on_blur: Option<SimpleCallback>,
    pub(crate) on_disable: Option<SimpleCallback>,
    pub(crate) on_active: Option<SimpleCallback>,
    pub(crate) on_select: Option<SimpleCallback>,
}

impl std::fmt::Debug for EventListeners {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("EventListeners")
            .field("on_click", &self.on_click.as_ref().map(|_| "FnMut"))
            .field(
                "on_right_click",
                &self.on_right_click.as_ref().map(|_| "FnMut"),
            )
            .field(
                "on_mouse_input",
                &self.on_mouse_input.as_ref().map(|_| "MouseCallback"),
            )
            .field(
                "on_mouse_enter",
                &self.on_mouse_enter.as_ref().map(|_| "FnMut"),
            )
            .field(
                "on_mouse_leave",
                &self.on_mouse_leave.as_ref().map(|_| "FnMut"),
            )
            .field(
                "on_cursor_moved",
                &self.on_cursor_moved.as_ref().map(|_| "FnMut(Point)"),
            )
            .field(
                "on_mouse_wheel",
                &self.on_mouse_wheel.as_ref().map(|_| "FnMut(f32)"),
            )
            .field("on_drag", &self.on_drag.as_ref().map(|_| "FnMut(Point)"))
            .field(
                "on_keyboard_input",
                &self.on_keyboard_input.as_ref().map(|_| "KeyCallback"),
            )
            .field(
                "on_char_input",
                &self.on_char_input.as_ref().map(|_| "FnMut(char)"),
            )
            .field("on_ime", &self.on_ime.as_ref().map(|_| "FnMut(ImeState)"))
            .field(
                "on_file_dropped",
                &self.on_file_dropped.as_ref().map(|_| "FnMut(Vec<PathBuf>)"),
            )
            .field(
                "on_media_loaded",
                &self
                    .on_media_loaded
                    .as_ref()
                    .map(|_| "FnMut(MediaLoadedCallback)"),
            )
            .field("on_hover", &self.on_hover.as_ref().map(|_| "FnMut"))
            .field("on_focus", &self.on_focus.as_ref().map(|_| "FnMut"))
            .field("on_blur", &self.on_blur.as_ref().map(|_| "FnMut"))
            .field("on_disable", &self.on_disable.as_ref().map(|_| "FnMut"))
            .field("on_active", &self.on_active.as_ref().map(|_| "FnMut"))
            .field("on_select", &self.on_select.as_ref().map(|_| "FnMut"))
            .finish()
    }
}

/// マウスクラスのアイコン指定
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum CursorIcon {
    #[default]
    Default,
    Pointer,
    Text,
    Grab,
    Grabbing,
    NotAllowed,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum MouseButton {
    Left,
    Right,
    Middle,
    X1,
    X2,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ElementState {
    Pressed,
    Released,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Hash)]
pub struct Modifiers {
    pub shift: bool,
    pub ctrl: bool,
    pub alt: bool,
    pub logo: bool, // Windowsキー
}

#[derive(Debug, Clone, PartialEq, Default)]
pub struct ImeState {
    pub is_open: bool,
    pub conversion_mode: u32,
    pub sentence_mode: u32,
    pub keyboard_layout_id: u32,
    pub composition_text: String,
    pub result_text: String,
    pub caret_position: Option<LayoutPoint>,
    pub composition_cursor: usize,
    pub composition_attrs: Vec<u8>,
}

/// 画像のデータソースの表現
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum ImageSource {
    /// ローカルファイルやアセットへのパス
    Path(PathBuf),
    /// インターネット上のURL
    Url(String),
    /// メモリ上に直接読み込まれたバイト列 (埋め込みアセットなど)
    Bytes(Arc<[u8]>),
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

/// 画像ファイルがロードされた際に取得できるメタデータ
#[derive(Debug, Clone, PartialEq)]
pub struct ImageMetadata {
    /// 画像自体の本来の縦横サイズ（解像度）
    pub resolution: LayoutSize,
    /// フォーマット名 (例: "png", "jpeg", "gif", "webp" など)
    pub format: String,
    /// アニメーション画像（GIFやAPNGなど）であるかどうか
    pub is_animated: bool,
}

/// 動画のデータソースの表現
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum MovieSource {
    Path(PathBuf),
    Url(String),
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

/// 動画要素が保持する再生設定プロパティ
#[derive(Debug, Clone, PartialEq)]
pub struct MovieProperty {
    pub source: Option<MovieSource>,
    pub autoplay: bool,
    pub loop_playback: bool,
    pub muted: bool,
    pub volume: f32,
    pub fast_forward: f32,
    pub rewind: f32,
    pub playback_rate: f32,
    pub current_time: Option<f32>,
}

impl MovieProperty {
    pub fn new(source: Option<MovieSource>) -> Self {
        Self {
            source,
            autoplay: false,
            loop_playback: false,
            muted: false,
            volume: 1.0,
            fast_forward: 0.0,
            rewind: 0.0,
            playback_rate: 1.0,
            current_time: None,
        }
    }
}

impl Default for MovieProperty {
    fn default() -> Self {
        Self {
            source: None,
            autoplay: true,
            loop_playback: true,
            muted: false,
            volume: 1.0,
            fast_forward: 0.0,
            rewind: 0.0,
            playback_rate: 1.0,
            current_time: None,
        }
    }
}

/// 動画ファイルがロードされた際に取得できるメタデータ
#[derive(Debug, Clone, PartialEq)]
pub struct MovieMetadata {
    /// 動画自体の本来の縦横サイズ（解像度）
    pub resolution: LayoutSize,
    /// フレームレート (fps)
    pub frame_rate: f32,
    /// ビットレート (bps)
    pub bitrate: u32,
    /// 動画の総再生時間（秒）
    pub duration: f32,
}

/// UI Automation (UIA) のプロパティ値の安全な表現
#[derive(Debug, Clone, PartialEq)]
pub enum UiaValue {
    String(String),
    Bool(bool),
    Int(i32),
    Double(f64),
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

/// Windows 11 のネイティブシステムバックドロップ（ウィンドウ背景ぼかし）効果
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
#[repr(i32)]
pub enum Backdrop {
    #[default]
    None = 0, // 透過無効（通常の wgpu 背景）
    Mica = 2,    // Mica（デスクトップ壁紙をサンプリングする不透明調）
    Acrylic = 3, // Acrylic（背後の他アプリ・デスクトップを半透明にぼかす）
    MicaAlt = 4, // Mica Alt (Tabbed)（ダーク調向けの濃いMica）
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum UserSelect {
    #[default]
    None,
    Text,
    All,
}

/// 枠線のスタイル
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
#[repr(u32)]
pub enum BorderStyle {
    #[default]
    Solid = 0,
    Dotted = 1,
    Dashed = 2,
    Double = 3,
}

/// 枠線描画の基準方向
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
#[repr(u32)]
pub enum BorderAlignment {
    #[default]
    Start = 0, // 左・上が基準 (左から右、上から下へ伸びる)
    End = 1,    // 右・下が基準 (右から左、下から上へ伸びる)
    Center = 2, // 中心が基準 (中心から両方向へ対称に広がる)
}

/// スクロールバーを表示する配置モード
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ScrollbarMode {
    /// コンテンツの横/下にレイアウト領域を確保して配置（コンテンツが狭まる）
    Layout,
    /// コンテンツの最前面に重ねて配置（コンテンツ領域を侵食しない）
    #[default]
    Overlay,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ScrollbarDisplay {
    None,
    Always,
    #[default]
    Auto,
    Transient,
}

/// スクロールコンテナが保持するスタイリング設定
#[derive(Debug, Clone)]
pub struct ScrollbarStyle {
    /// スクロールバーの太さ（縦スクロールバー時は幅、横スクロールバー時は高さ）
    pub width: f32,
    /// 表示
    pub display: ScrollbarDisplay,
    /// スクロールバーの表示モード
    pub mode: ScrollbarMode,
    /// スクロールバーのレール（トラック背景）部分のスタイル
    pub track: Option<ThisStyle>,
    /// つまみ（サム）部分のスタイル
    pub thumb: Option<ThisStyle>,
}

impl Default for ScrollbarStyle {
    fn default() -> Self {
        Self {
            width: 10.0,
            display: ScrollbarDisplay::Auto,
            mode: ScrollbarMode::Overlay,
            track: None,
            thumb: None,
        }
    }
}

impl ScrollbarStyle {
    pub fn new(width: f32) -> Self {
        Self {
            width,
            display: ScrollbarDisplay::Auto,
            mode: ScrollbarMode::Overlay,
            track: None,
            thumb: None,
        }
    }

    pub fn mode(mut self, mode: ScrollbarMode) -> Self {
        self.mode = mode;
        self
    }

    pub fn display(mut self, display: ScrollbarDisplay) -> Self {
        self.display = display;
        self
    }

    pub fn track(mut self, style: ThisStyle) -> Self {
        self.track = Some(style);
        self
    }

    pub fn thumb(mut self, style: ThisStyle) -> Self {
        self.thumb = Some(style);
        self
    }
}

#[cfg(test)]
mod tests;
