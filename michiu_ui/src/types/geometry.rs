use bytemuck::{Pod, Zeroable};

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

    /// Returns that value if it is a Px; otherwise, returns 0.0.
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

impl From<LayoutRect> for accesskit::Rect {
    fn from(r: LayoutRect) -> Self {
        Self {
            x0: r.x as f64,
            y0: r.y as f64,
            x1: (r.x + r.width) as f64,
            y1: (r.y + r.height) as f64,
        }
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
    pub(crate) fn contains(&self, point: LayoutPoint) -> bool {
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
    pub(crate) fn intersect(&self, other: &Self) -> Self {
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

    /// Generate images by specifying each of the four directions individually in physical pixels.
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

    /// Set all four directions to the same physical pixel at once.
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

    /// Specify symmetry for the top/bottom and left/right axes in physical pixels.
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

    /// Creates rounded corners that are vertically symmetrical or horizontally symmetrical.
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
