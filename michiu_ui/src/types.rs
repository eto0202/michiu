pub mod font;
pub mod from_into;
pub mod index;
pub mod layout_data;
pub mod string;

pub use font::*;
pub use from_into::*;
pub use index::*;
pub use layout_data::*;
pub use string::*;

use crate::{Context, Element, EntityId, ImeState, PropertyList, VirtualKey, rgba};
use bytemuck::{Pod, Zeroable};
use std::{path::PathBuf, time::Duration};
use windows::Win32::{
    Graphics::Gdi::{
        BITMAPINFO, BITMAPINFOHEADER, CreateBitmap, CreateDIBSection, DIB_RGB_COLORS, DeleteObject,
        GetDC, HGDIOBJ, ReleaseDC,
    },
    UI::WindowsAndMessaging::{CreateIconIndirect, HCURSOR, ICONINFO},
};

#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Pod, Zeroable)]
pub struct Color {
    pub r: f32,
    pub g: f32,
    pub b: f32,
    pub a: f32,
}

impl Default for Color {
    fn default() -> Self {
        Color::TRANSPARENT
    }
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
    #[must_use]
    pub const fn rgb_f32(r: f32, g: f32, b: f32) -> Self {
        Self { r, g, b, a: 1.0 }
    }

    /// GPU/シェーダー用の 0.0~1.0 (f32) 値から直接生成します
    #[inline]
    #[must_use]
    pub const fn rgba_f32(r: f32, g: f32, b: f32, a: f32) -> Self {
        Self { r, g, b, a }
    }

    /// 色味を維持したまま、不透明度（アルファ）だけを動的に書き換えます
    #[inline]
    #[must_use]
    pub const fn with_alpha(self, a: f32) -> Self {
        Self { a, ..self }
    }

    /// HSL モデル（Hue: 0..360, Saturation: 0..100%, Lightness: 0..100%）から Color を生成します
    #[inline]
    #[must_use]
    pub fn hsl(h: f32, s: f32, l: f32) -> Self {
        Self::hsla(h, s, l, 1.0)
    }

    /// HSL モデルにアルファ（0.0..1.0）を付与して Color を生成します
    #[must_use]
    #[allow(clippy::many_single_char_names)]
    pub fn hsla(h: f32, s: f32, l: f32, a: f32) -> Self {
        // 色相（h）を 0..360 の範囲に正規化
        let h_mod = (h % 360.0 + 360.0) % 360.0;

        // s, l を 0.0..100.0 (%) から 0.0..1.0 の比率へ安全変換
        let s = (s / 100.0).clamp(0.0, 1.0);
        let l = (l / 100.0).clamp(0.0, 1.0);

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

pub trait IntoHexColor {
    fn into_hex_color(self) -> Color;
}

impl IntoHexColor for &str {
    #[inline]
    fn into_hex_color(self) -> Color {
        let s = self.trim_start_matches('#').trim_start_matches("0x");
        if let Ok(num) = u32::from_str_radix(s, 16) {
            parse_u32_to_color(num, s.len() > 6)
        } else {
            Color::TRANSPARENT
        }
    }
}

impl IntoHexColor for String {
    #[inline]
    fn into_hex_color(self) -> Color {
        self.as_str().into_hex_color()
    }
}

impl IntoHexColor for u32 {
    #[inline]
    fn into_hex_color(self) -> Color {
        parse_u32_to_color(self, self > 0xFF_FFFF)
    }
}

#[inline]
fn parse_u32_to_color(num: u32, is_8digit: bool) -> Color {
    if is_8digit {
        let r = ((num >> 24) & 0xFF) as f32 / 255.0;
        let g = ((num >> 16) & 0xFF) as f32 / 255.0;
        let b = ((num >> 8) & 0xFF) as f32 / 255.0;
        let a = (num & 0xFF) as f32 / 255.0;
        Color { r, g, b, a }
    } else {
        let r = ((num >> 16) & 0xFF) as f32 / 255.0;
        let g = ((num >> 8) & 0xFF) as f32 / 255.0;
        let b = (num & 0xFF) as f32 / 255.0;
        Color { r, g, b, a: 1.0 }
    }
}

#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Pod, Zeroable)]
pub struct LayoutPoint {
    pub x: f32,
    pub y: f32,
}

impl Default for LayoutPoint {
    fn default() -> Self {
        LayoutPoint::ZERO
    }
}

impl LayoutPoint {
    pub const ZERO: Self = Self { x: 0.0, y: 0.0 };

    pub const ORIGIN: Self = Self { x: 0.5, y: 0.5 };

    #[inline]
    #[must_use]
    pub const fn new(x: f32, y: f32) -> Self {
        Self { x, y }
    }
}

#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Pod, Zeroable)]
pub struct LayoutSize {
    pub width: f32,
    pub height: f32,
}

impl Default for LayoutSize {
    fn default() -> Self {
        LayoutSize::ZERO
    }
}

impl LayoutSize {
    pub const ZERO: Self = Self {
        width: 0.0,
        height: 0.0,
    };

    #[inline]
    #[must_use]
    pub const fn new(width: f32, height: f32) -> Self {
        Self { width, height }
    }
}

#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Pod, Zeroable)]
pub struct LayoutRect {
    pub x: f32,
    pub y: f32,
    pub width: f32,
    pub height: f32,
}

impl Default for LayoutRect {
    fn default() -> Self {
        LayoutRect::ZERO
    }
}

impl LayoutRect {
    pub const ZERO: Self = Self {
        x: 0.0,
        y: 0.0,
        width: 0.0,
        height: 0.0,
    };

    #[inline]
    #[must_use]
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
    #[must_use]
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
    #[must_use]
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
#[derive(Debug, Clone, Copy, PartialEq, Pod, Zeroable)]
pub struct EdgeInsets {
    pub top: f32,
    pub right: f32,
    pub bottom: f32,
    pub left: f32,
}

impl Default for EdgeInsets {
    fn default() -> Self {
        EdgeInsets::ZERO
    }
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
    #[must_use]
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
    #[must_use]
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
    #[must_use]
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
    #[must_use]
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
    #[must_use]
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
    #[must_use]
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
    #[must_use]
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
    #[must_use]
    pub fn offset(mut self, value: impl IntoLayoutPoint) -> Self {
        self.offset = value.into_layout_point();
        self
    }

    /// 影のぼかし（blur）幅を設定します。
    #[inline]
    #[must_use]
    pub fn blur(mut self, value: impl Convert<f32>) -> Self {
        self.blur = value.convert();
        self
    }

    /// 影の広がり（spread）幅を設定します。
    #[inline]
    #[must_use]
    pub fn spread(mut self, value: impl Convert<f32>) -> Self {
        self.spread = value.convert();
        self
    }

    /// 影のカラーを設定します。
    #[inline]
    #[must_use]
    pub fn color(mut self, value: Color) -> Self {
        self.color = value;
        self
    }

    /// 控えめな極小のソフトシャドウ
    #[must_use]
    pub fn sm() -> Self {
        BoxShadow::new()
            .blur(2)
            .color(rgba(0, 0, 0, 0.05))
            .offset((0, 1))
    }

    /// 標準的な中程度のソフトシャドウ
    #[must_use]
    pub fn md() -> Self {
        BoxShadow::new()
            .blur(6)
            .spread(-1)
            .color(rgba(0, 0, 0, 0.1))
            .offset((0, 4))
    }

    /// やや浮き上がって見える大きめのソフトシャドウ
    #[must_use]
    pub fn lg() -> Self {
        BoxShadow::new()
            .blur(15)
            .spread(-3)
            .color(rgba(0, 0, 0, 0.1))
            .offset((0, 10))
    }

    #[must_use]
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

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Hash)]
pub struct Point<T> {
    pub x: T,
    pub y: T,
}

impl Point<f32> {
    pub const ORIGIN: Self = Self { x: 0.5, y: 0.5 };
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

#[derive(Debug, Clone, Copy, PartialEq)]
#[repr(u8)]
pub enum Length {
    Px(f32),
    Percent(f32),
}

impl Length {
    #[inline]
    #[must_use]
    pub fn px(px: f32) -> Self {
        Self::Px(px)
    }

    #[inline]
    #[must_use]
    pub fn pct(percent: f32) -> Self {
        Self::Percent(percent)
    }

    /// Px ならその値、それ以外は 0.0 を返す
    #[inline]
    #[must_use]
    pub fn to_px_or_zero(&self) -> f32 {
        match *self {
            Length::Px(v) => v,
            Length::Percent(_) => 0.0,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
#[repr(u8)]
pub enum Val {
    Auto,
    Px(f32),
    Percent(f32),
}

impl Val {
    #[inline]
    #[must_use]
    pub fn auto() -> Self {
        Self::Auto
    }

    #[inline]
    #[must_use]
    pub fn px(px: f32) -> Self {
        Self::Px(px)
    }

    #[inline]
    #[must_use]
    pub fn pct(percent: f32) -> Self {
        Self::Percent(percent)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Default)]
#[repr(u8)]
pub enum Display {
    #[default]
    Flex,
    Grid,
    Block,
    None,
}

#[derive(Debug, Clone, Copy, PartialEq, Default)]
#[repr(u8)]
pub enum Position {
    #[default]
    Relative,
    Absolute,
}

#[derive(Debug, Clone, Copy, PartialEq, Default)]
#[repr(u8)]
pub enum BoxSizing {
    #[default]
    BorderBox,
    ContentBox,
}

#[derive(Debug, Clone, Copy, PartialEq, Default)]
#[repr(u8)]
pub enum Direction {
    #[default]
    Ltr,
    Rtl,
}

#[derive(Debug, Clone, Copy, PartialEq, Default)]
#[repr(u8)]
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
#[repr(u8)]
pub enum FlexDirection {
    #[default]
    Row,
    Column,
    RowReverse,
    ColumnReverse,
}

#[derive(Debug, Clone, Copy, PartialEq, Default)]
#[repr(u8)]
pub enum FlexWrap {
    #[default]
    NoWrap,
    Wrap,
    WrapReverse,
}

#[derive(Debug, Clone, Copy, PartialEq, Default)]
#[repr(u8)]
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
#[repr(u8)]
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
#[repr(u8)]
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
#[repr(u8)]
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
#[repr(u8)]
pub enum TextAlign {
    #[default]
    Auto,
    Left,
    Right,
    Center,
}

#[derive(Debug, Clone, Copy, PartialEq, Default)]
#[repr(u8)]
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
    #[must_use]
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
    #[must_use]
    pub fn translate(self, x: f32, y: f32) -> Self {
        let mut t = Self::new();
        t.matrix[3][0] = x;
        t.matrix[3][1] = y;
        self.mul(&t)
    }

    /// 拡大縮小
    #[inline]
    #[must_use]
    pub fn scale(self, x: f32, y: f32) -> Self {
        let mut s = Self::new();
        s.matrix[0][0] = x;
        s.matrix[1][1] = y;
        self.mul(&s)
    }

    /// Z軸（2D平面上）の回転（ラジアン）
    #[inline]
    #[must_use]
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
    /// 状態遷移を設定できる `ComponentMask` (例: `STYLE_BG_COLOR` | `STYLE_OPACITY`)
    pub property_list: PropertyList,
    /// アニメーションの時間
    pub duration: Duration,
    /// イージングカーブ
    pub curve: AnimationCurve,
}

impl Transition {
    #[must_use]
    pub fn new(property_list: PropertyList, duration: Duration, curve: AnimationCurve) -> Self {
        Self {
            property_list,
            duration,
            curve,
        }
    }

    #[must_use]
    pub fn property_list(mut self, property_list: PropertyList) -> Self {
        self.property_list = property_list;
        self
    }

    #[must_use]
    pub fn duration(mut self, duration: Duration) -> Self {
        self.duration = duration;
        self
    }

    #[must_use]
    pub fn curve(mut self, curve: AnimationCurve) -> Self {
        self.curve = curve;
        self
    }
}

/// アニメーションのイージングカーブを定義する列挙型。
/// 軽量なため Clone と Copy が可能です。
// TODO: バネ物理シミュレーション Spring Physics
// 摩擦（Damping）とバネの強さ（Stiffness）のパラメータから毎フレーム物理演算
#[derive(Debug, Clone, Copy)]
pub enum AnimationCurve {
    /// イージングなし（線形 / リニア）
    Linear,
    /// 加減速をかける標準的な2次イージング
    EaseInOutQuad,
    /// 加速（2次）
    EaseInQuad,
    /// 減速（2次）
    EaseOutQuad,
    /// ユーザーが独自のイージング計算（0.0～1.0 を受け取り 0.0～1.0 を返す）を行えるエスケープハッチ
    Custom(fn(f32) -> f32),
}

impl AnimationCurve {
    /// 経過割合 t (0.0 <= t <= 1.0) に基づいて、イージングされた値を評価します
    #[must_use]
    pub fn evaluate(&self, t: f32) -> f32 {
        let t = t.clamp(0.0, 1.0);
        match *self {
            AnimationCurve::Linear => t,
            AnimationCurve::EaseInQuad => t * t,
            AnimationCurve::EaseOutQuad => t * (2.0 - t),
            AnimationCurve::EaseInOutQuad => {
                if t < 0.5 {
                    2.0 * t * t
                } else {
                    -1.0 + (4.0 - 2.0 * t) * t
                }
            }
            AnimationCurve::Custom(f) => f(t),
        }
    }
}

impl PartialEq for AnimationCurve {
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            (Self::Linear, Self::Linear)
            | (Self::EaseInOutQuad, Self::EaseInOutQuad)
            | (Self::EaseInQuad, Self::EaseInQuad)
            | (Self::EaseOutQuad, Self::EaseOutQuad) => true,
            (Self::Custom(f1), Self::Custom(f2)) => std::ptr::fn_addr_eq(*f1, *f2),
            _ => false,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum PlaybackCount {
    Infinite,
    Count(u32),
}

/// CSS Animation 相当の設定を定義
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct KeyframeAnimation {
    /// 何をアニメーションさせるか
    pub property: PropertyList,
    /// 1周（ループ）にかかる時間
    pub duration: Duration,
    /// ループ回数
    pub iteration_count: PlaybackCount,
    /// イージングカーブ
    pub curve: AnimationCurve,
}

impl KeyframeAnimation {
    #[inline]
    #[must_use]
    pub fn new(
        property: PropertyList,
        duration: Duration,
        iteration_count: PlaybackCount,
        curve: AnimationCurve,
    ) -> Self {
        Self {
            property,
            duration,
            iteration_count,
            curve,
        }
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

/// 子要素から伝播して解決可能なインタラクション定義
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(u8)]
pub enum InteractionName {
    Hover,
    Focus,
    FocusVisible,
    Press,
    Disable,
    Active,
    Select,
    Drag,
    All,
}

macro_rules! define_event_dispatchers {
    (
        $(
            $fn_name:ident, $field_name:ident $(, $arg_name:ident : $arg_type:ty)*;
        )*
    ) => {
        $(
            #[inline]
            #[allow(dead_code)]
            pub(crate) fn $fn_name(
                cx: &mut Context,
                id: EntityId,
                $($arg_name : $arg_type),*
            ) {
                // events.evt_listeners から該当のハンドラを一時的に take する
                if let Some(mut handler) = cx
                    .events
                    .evt_listeners
                    .get_mut(id)
                    .and_then(|l| l.$field_name.take())
                {
                    let _guard = crate::ActiveElementGuard::new(id);

                    // コールバックを安全に実行
                    handler(cx, $($arg_name),*);

                    // 実行後コールバックを書き戻す
                    if let Some(l) = cx.events.evt_listeners.get_mut(id) {
                        l.$field_name = Some(handler);
                    }
                }
            }
        )*
    };
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
pub type SimpleCallback = Box<dyn FnMut(&mut Context) + 'static>;

// 各コールバックの引数: (context, ドラッグ元のElement, 現在ホバーまたはドロップされた対象のElement)
// 失敗時は対象が None
pub type EntityDragCallback = Box<dyn FnMut(&mut Context, Element, Option<Element>) + 'static>;
pub type IdDragCallback = Box<dyn FnMut(&mut Context, EntityId, Option<EntityId>) + 'static>;
pub type EntityDropCallback = Box<dyn FnMut(&mut Context, Element, Option<Element>) + 'static>;
pub type IdDropCallback = Box<dyn FnMut(&mut Context, EntityId, Option<EntityId>) + 'static>;
// ドラッグ開始時のコールバック型。生成されたプレースホルダーの Element を受け取れます。
// 引数: (context, ドラッグ元のオリジナル要素, 生成されたプレースホルダー要素)
pub type DragStartCallback = Box<dyn FnMut(&mut Context, Element, Element) + 'static>;

/// 要素ごとにバインドされる、検証済みイベントのハンドラ群。
#[derive(Default)]
#[allow(clippy::struct_field_names)]
pub(crate) struct EventListeners {
    /// 要素がクリックされた（マウスダウン -> 同一要素上でマウスアップされた）際のコールバック
    pub(crate) on_click: Option<ClickCallback>,

    /// 右クリックされた際のコールバック（コンテキストメニューの起動用など）
    pub(crate) on_right_click: Option<SimpleCallback>,

    /// マウスボタンの押し下げ・離しを直接受け取るハンドラ
    /// 引数: (ボタンの種類, 装飾キーの状態, 押し下げ/離し状態)
    pub(crate) on_mouse_input: Option<MouseCallback>,

    /// `マウスカーソルがこの要素の可視境界（out_clip_rects）に入った際のイベント`
    pub(crate) on_mouse_enter: Option<SimpleCallback>,

    /// マウスカーソルがこの要素の可視境界から外に出た際のイベント
    pub(crate) on_mouse_leave: Option<SimpleCallback>,

    /// マウスカーソルが要素内で動いた際のイベント。
    /// 引数: 要素の左上を (0.0, 0.0) とする、論理スケーリング済みの相対座標 `Point`
    pub(crate) on_cursor_moved: Option<CursorMovedCallback>,

    /// `マウスホイールが回された際のイベント（WM_MOUSEWHEEL` / `WM_MOUSEHWHEEL` 互換）
    /// 引数: 前方向ならプラス、後方向ならマイナスの移動量（delta）
    pub(crate) on_mouse_wheel: Option<MouseWheelCallback>,

    /// 要素がドラッグされている最中のイベント
    /// 引数: ドラッグによる移動量 `Point(delta_x, delta_y)`
    pub(crate) on_drag: Option<DragCallback>,

    pub(crate) on_hover: Option<SimpleCallback>,
    pub(crate) on_focus: Option<SimpleCallback>,
    pub(crate) on_blur: Option<SimpleCallback>,
    pub(crate) on_disable: Option<SimpleCallback>,
    pub(crate) on_active: Option<SimpleCallback>,
    pub(crate) on_select: Option<SimpleCallback>,

    /// 物理キーボードが押された、または離された際のイベント
    /// 引数: 検証済みの仮想キーコード, 装飾キー, 状態
    pub(crate) on_keyboard_input: Option<KeyCallback>,

    /// `IMEなどを介さない、確定した1文字の文字入力イベント（WM_CHAR` 互換）
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

    // D&D 専用イベント
    pub(crate) on_dnd_entity_drag: Option<EntityDragCallback>,
    pub(crate) on_dnd_id_drag: Option<IdDragCallback>,
    pub(crate) on_dnd_entity_drop: Option<EntityDropCallback>,
    pub(crate) on_dnd_id_drop: Option<IdDropCallback>,
    pub(crate) on_dnd_drag_start: Option<DragStartCallback>,
}

define_event_dispatchers! {
    // マウス・クリック
    handle_on_click, on_click;
    handle_on_right_click, on_right_click;
    handle_on_mouse_input, on_mouse_input, button: MouseButton, modifiers: Modifiers, state: ElementState;
    handle_on_mouse_enter, on_mouse_enter;
    handle_on_mouse_leave, on_mouse_leave;
    handle_on_cursor_moved, on_cursor_moved, pos: LayoutPoint;
    handle_on_mouse_wheel, on_mouse_wheel, delta_x: f32, delta_y: f32;
    handle_on_drag, on_drag, delta: LayoutPoint;

    // ステート変化
    handle_on_hover, on_hover;
    handle_on_focus, on_focus;
    handle_on_blur, on_blur;
    handle_on_disable, on_disable;
    handle_on_active, on_active;
    handle_on_select, on_select;

    // キーボード・入力
    handle_on_keyboard_input, on_keyboard_input, key: VirtualKey, modifiers: Modifiers, state: ElementState;
    handle_on_char_input, on_char_input, ch: char;
    handle_on_ime, on_ime, info: ImeState;

    // ファイルドロップ
    handle_on_file_dropped, on_file_dropped, paths: Vec<PathBuf>;
    handle_on_file_drag_enter, on_file_drag_enter;
    handle_on_file_drag_leave, on_file_drag_leave;

    // ドラッグ＆ドロップ
    handle_on_dnd_entity_drag, on_dnd_entity_drag, origin: Element, target: Option<Element>;
    handle_on_dnd_id_drag, on_dnd_id_drag, origin_id: EntityId, target_id: Option<EntityId>;
    handle_on_dnd_entity_drop, on_dnd_entity_drop, origin: Element, target: Option<Element>;
    handle_on_dnd_id_drop, on_dnd_id_drop, origin_id: EntityId, target_id: Option<EntityId>;
    handle_on_dnd_drag_start, on_dnd_drag_start, origin: Element, placeholder: Element;
}

impl std::fmt::Debug for EventListeners {
    #[allow(clippy::too_many_lines)]
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
                "on_file_drag_enter",
                &self.on_file_drag_enter.as_ref().map(|_| "FnMut"),
            )
            .field(
                "on_file_drag_leave",
                &self.on_file_drag_leave.as_ref().map(|_| "FnMut"),
            )
            .field("on_hover", &self.on_hover.as_ref().map(|_| "FnMut"))
            .field("on_focus", &self.on_focus.as_ref().map(|_| "FnMut"))
            .field("on_blur", &self.on_blur.as_ref().map(|_| "FnMut"))
            .field("on_disable", &self.on_disable.as_ref().map(|_| "FnMut"))
            .field("on_active", &self.on_active.as_ref().map(|_| "FnMut"))
            .field("on_select", &self.on_select.as_ref().map(|_| "FnMut"))
            .field(
                "on_dnd_entity_drag",
                &self
                    .on_dnd_entity_drag
                    .as_ref()
                    .map(|_| "FnMut(EntityId, Option<EntityId>)"),
            )
            .field(
                "on_dnd_id_drag",
                &self
                    .on_dnd_id_drag
                    .as_ref()
                    .map(|_| "FnMut(EntityId, Option<EntityId>)"),
            )
            .field(
                "on_dnd_entity_drop",
                &self
                    .on_dnd_entity_drop
                    .as_ref()
                    .map(|_| "FnMut(EntityId, Option<EntityId>)"),
            )
            .field(
                "on_dnd_id_drop",
                &self
                    .on_dnd_id_drop
                    .as_ref()
                    .map(|_| "FnMut(EntityId, Option<EntityId>)"),
            )
            .field(
                "on_dnd_drag_start",
                &self
                    .on_dnd_drag_start
                    .as_ref()
                    .map(|_| "FnMut(EntityId, Element)"),
            )
            .finish()
    }
}

/// 伝播用のグローバルカーソル種別
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GlobalCursorIcon {
    Default(Option<HCURSOR>),
    Pointer(Option<HCURSOR>),
    Text(Option<HCURSOR>),
    Grab(Option<HCURSOR>),
    Grabbing(Option<HCURSOR>),
    NotAllowed(Option<HCURSOR>),
    ResizeNs(Option<HCURSOR>),
    ResizeEw(Option<HCURSOR>),
    ResizeNesw(Option<HCURSOR>),
    ResizeNwse(Option<HCURSOR>),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CursorIcon {
    Default(Option<HCURSOR>),
    Pointer(Option<HCURSOR>),
    Text(Option<HCURSOR>),
    Grab(Option<HCURSOR>),
    Grabbing(Option<HCURSOR>),
    NotAllowed(Option<HCURSOR>),
    ResizeNs(Option<HCURSOR>),
    ResizeEw(Option<HCURSOR>),
    ResizeNesw(Option<HCURSOR>),
    ResizeNwse(Option<HCURSOR>),
    Global(GlobalCursorIcon),
}

unsafe impl Send for CursorIcon {}
unsafe impl Sync for CursorIcon {}

unsafe impl Send for GlobalCursorIcon {}
unsafe impl Sync for GlobalCursorIcon {}

impl Default for CursorIcon {
    fn default() -> Self {
        CursorIcon::Default(None)
    }
}

impl CursorIcon {
    /// Windows API の HCURSOR 物理ハンドルを安全にロードして返却します。
    /// 独自の HCURSOR が指定されている場合はそれを最優先し、None の場合はOSのシステム標準をロードします。
    #[must_use]
    pub fn to_hcursor(self) -> HCURSOR {
        use windows::Win32::UI::WindowsAndMessaging::{
            IDC_ARROW, IDC_HAND, IDC_IBEAM, IDC_NO, IDC_SIZEALL, IDC_SIZENESW, IDC_SIZENS,
            IDC_SIZENWSE, IDC_SIZEWE, LoadCursorW,
        };
        unsafe {
            let idc = match self {
                // 独自カーソル指定時は即座にそのハンドルを返却
                CursorIcon::Default(Some(h))
                | CursorIcon::Pointer(Some(h))
                | CursorIcon::Text(Some(h))
                | CursorIcon::Grab(Some(h))
                | CursorIcon::Grabbing(Some(h))
                | CursorIcon::NotAllowed(Some(h))
                | CursorIcon::ResizeNs(Some(h))
                | CursorIcon::ResizeEw(Some(h))
                | CursorIcon::ResizeNesw(Some(h))
                | CursorIcon::ResizeNwse(Some(h)) => return h,
                CursorIcon::Global(global_icon) => {
                    match global_icon {
                        GlobalCursorIcon::Default(Some(h))
                        | GlobalCursorIcon::Pointer(Some(h))
                        | GlobalCursorIcon::Text(Some(h))
                        | GlobalCursorIcon::Grab(Some(h))
                        | GlobalCursorIcon::Grabbing(Some(h))
                        | GlobalCursorIcon::NotAllowed(Some(h))
                        | GlobalCursorIcon::ResizeNs(Some(h))
                        | GlobalCursorIcon::ResizeEw(Some(h))
                        | GlobalCursorIcon::ResizeNesw(Some(h))
                        | GlobalCursorIcon::ResizeNwse(Some(h)) => return h,
                        _ => {}
                    }
                    // None 時は標準システムカーソルにフォールバック
                    match global_icon {
                        GlobalCursorIcon::Default(_) => IDC_ARROW,
                        GlobalCursorIcon::Pointer(_) => IDC_HAND,
                        GlobalCursorIcon::Text(_) => IDC_IBEAM,
                        GlobalCursorIcon::Grab(_) | GlobalCursorIcon::Grabbing(_) => IDC_SIZEALL,
                        GlobalCursorIcon::NotAllowed(_) => IDC_NO,
                        GlobalCursorIcon::ResizeNs(_) => IDC_SIZENS,
                        GlobalCursorIcon::ResizeEw(_) => IDC_SIZEWE,
                        GlobalCursorIcon::ResizeNesw(_) => IDC_SIZENESW,
                        GlobalCursorIcon::ResizeNwse(_) => IDC_SIZENWSE,
                    }
                }

                // 独自カーソル未指定(None)時は、Windows 標準カーソルからロード
                CursorIcon::Default(None) => IDC_ARROW,
                CursorIcon::Pointer(None) => IDC_HAND,
                CursorIcon::Text(None) => IDC_IBEAM,
                CursorIcon::Grab(None) | CursorIcon::Grabbing(None) => IDC_SIZEALL,
                CursorIcon::NotAllowed(None) => IDC_NO,
                CursorIcon::ResizeNs(None) => IDC_SIZENS,
                CursorIcon::ResizeEw(None) => IDC_SIZEWE,
                CursorIcon::ResizeNesw(None) => IDC_SIZENESW,
                CursorIcon::ResizeNwse(None) => IDC_SIZENWSE,
            };

            LoadCursorW(None, idc).unwrap()
        }
    }

    /// メモリ上の RGBA8 ピクセルデータから指定したホットスポット座標を持つカスタム HCURSOR を生成します（アルファ透過対応）。
    pub fn create_from_rgba(
        rgba_pixels: &[u8],
        width: u32,
        height: u32,
        hotspot_x: u32,
        hotspot_y: u32,
    ) -> Result<HCURSOR, Box<dyn std::error::Error>> {
        if rgba_pixels.len() != (width * height * 4) as usize {
            return Err("Pixel buffer size mismatch for the given width and height".into());
        }

        unsafe {
            let h_dc = GetDC(None);
            if h_dc.is_invalid() {
                return Err("Failed to get DC".into());
            }

            let bmi = BITMAPINFO {
                bmiHeader: BITMAPINFOHEADER {
                    biSize: std::mem::size_of::<BITMAPINFOHEADER>() as u32,
                    biWidth: width as i32,
                    biHeight: -(height as i32),
                    biPlanes: 1,
                    biBitCount: 32,
                    biCompression: 0,
                    ..Default::default()
                },
                ..Default::default()
            };

            let mut pv_bits = std::ptr::null_mut();
            let hbm_color = CreateDIBSection(
                Some(h_dc),
                &raw const bmi,
                DIB_RGB_COLORS,
                &raw mut pv_bits,
                None,
                0,
            )?;

            if pv_bits.is_null() {
                let _ = ReleaseDC(None, h_dc);
                let _ = DeleteObject(HGDIOBJ(hbm_color.0));
                return Err("Failed to allocate DIB Section memory".into());
            }

            let dest_slice =
                std::slice::from_raw_parts_mut(pv_bits.cast::<u8>(), (width * height * 4) as usize);
            for i in (0..(width * height * 4) as usize).step_by(4) {
                dest_slice[i] = rgba_pixels[i + 2]; // B
                dest_slice[i + 1] = rgba_pixels[i + 1]; // G
                dest_slice[i + 2] = rgba_pixels[i]; // R
                dest_slice[i + 3] = rgba_pixels[i + 3]; // A
            }

            let hbm_mask = CreateBitmap(width as i32, height as i32, 1, 1, None);

            let icon_info = ICONINFO {
                fIcon: false.into(),
                xHotspot: hotspot_x,
                yHotspot: hotspot_y,
                hbmMask: hbm_mask,
                hbmColor: hbm_color,
            };

            let h_icon = CreateIconIndirect(&raw const icon_info)?;
            let h_cursor = windows::Win32::UI::WindowsAndMessaging::HCURSOR(h_icon.0);

            let _ = DeleteObject(HGDIOBJ(hbm_color.0));
            let _ = DeleteObject(HGDIOBJ(hbm_mask.0));
            let _ = ReleaseDC(None, h_dc);

            Ok(h_cursor)
        }
    }

    /// 画像ファイルのパスから指定したホットスポット座標を持つカスタム HCURSOR を生成します。
    pub fn create_from_path(
        path: impl AsRef<std::path::Path>,
        hotspot_x: u32,
        hotspot_y: u32,
    ) -> Result<HCURSOR, Box<dyn std::error::Error>> {
        let img = image::open(path)?;
        let rgba_img = img.to_rgba8();
        let (width, height) = rgba_img.dimensions();

        Self::create_from_rgba(rgba_img.as_raw(), width, height, hotspot_x, hotspot_y)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(u8)]
pub enum MouseButton {
    Left,
    Right,
    Middle,
    X1,
    X2,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(u8)]
pub enum ElementState {
    Pressed,
    Released,
}

#[allow(clippy::struct_excessive_bools)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Hash)]
pub struct Modifiers {
    pub shift: bool,
    pub ctrl: bool,
    pub alt: bool,
    pub logo: bool, // Windowsキー
}

/// UI Automation (UIA) のプロパティ値の安全な表現
#[derive(Debug, Clone, PartialEq)]
#[repr(u8)]
pub enum UiaValue {
    String(String),
    Bool(bool),
    Int(i32),
    Double(f64),
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
#[repr(u8)]
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

#[derive(Debug, Clone, Copy, PartialEq)]
#[repr(u8)]
pub enum InteractionState {
    Hovered,
    Focused,
    Pressed,
    Dragged,
}

/// 外部の動画やゲーム等からGUIへ動的にフレームを供給するためのトレイト
pub trait ExternalTexture: Send + Sync {
    /// 描画直前に呼び出され、このフレームで描画すべき最新の `TextureView` を返す。
    fn resolve_view(&self, device: &wgpu::Device, queue: &wgpu::Queue) -> wgpu::TextureView;

    /// 描画方法を制御するメタデータを同期的に取得。
    fn metadata(&self) -> ExternalTextureMetadata;
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ExternalTextureMetadata {
    pub size: LayoutSize,
    pub alpha_mode: ExternalTextureAlphaMode,
    pub y_flip: bool,
    // テクスチャが -Srgb 系統の自動色空間変換フォーマットかどうか
    pub is_srgb: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum ExternalTextureAlphaMode {
    /// 通常（Straight）アルファ。シェーダー内で自動的に PMA（乗算済みアルファ）へ変換。
    Straight,
    /// 乗算済み（Premultiplied）。シェーダー内でそのまま合成。
    Premultiplied,
}

#[cfg(test)]
mod tests;
