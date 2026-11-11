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
    Stretch,
    Px(f32),
    Percent(f32),
    MinContent,
    MaxContent,
    FitContentPx(f32),
    FitContentPercent(f32),
    FitContent,
    Content,
}

impl Val {
    #[inline]
    #[must_use]
    pub fn auto() -> Self {
        Self::Auto
    }

    #[inline]
    #[must_use]
    pub fn fill() -> Self {
        Self::Stretch
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

    #[inline]
    #[must_use]
    pub const fn all(v: f32) -> Self {
        Self {
            width: v,
            height: v,
        }
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
            x0: f64::from(r.x),
            y0: f64::from(r.y),
            x1: f64::from(r.x + r.width),
            y1: f64::from(r.y + r.height),
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

    /// 面積が 0 より大きいか（描画領域が存在するか）
    #[inline]
    pub(crate) fn has_area(&self) -> bool {
        self.width > 0.0 && self.height > 0.0
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

#[derive(
    Debug, Clone, Copy, PartialEq, PartialOrd, Default, derive_more::Deref, derive_more::DerefMut,
)]
pub struct Pixel(pub f32);
#[cfg(feature = "serde")]
impl serde::Serialize for Pixel {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        serializer.collect_str(&format_args!("{}px", self.0))
    }
}
#[cfg(feature = "serde")]
impl<'de> serde::Deserialize<'de> for Pixel {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        struct PixelVisitor;

        impl serde::de::Visitor<'_> for PixelVisitor {
            type Value = Pixel;

            fn expecting(&self, formatter: &mut std::fmt::Formatter) -> std::fmt::Result {
                formatter.write_str("a pixel length like \"10px\"")
            }

            fn visit_str<E>(self, value: &str) -> Result<Pixel, E>
            where
                E: serde::de::Error,
            {
                let s = value.trim();
                let num_str = s
                    .strip_suffix("px")
                    .ok_or_else(|| serde::de::Error::custom("expected length ending with 'px'"))?;

                num_str
                    .trim()
                    .parse::<f32>()
                    .map(Pixel)
                    .map_err(serde::de::Error::custom)
            }
        }

        deserializer.deserialize_str(PixelVisitor)
    }
}

#[derive(
    Debug, Clone, Copy, PartialEq, PartialOrd, Default, derive_more::Deref, derive_more::DerefMut,
)]
pub struct Percent(pub f32);
#[cfg(feature = "serde")]
impl serde::Serialize for Percent {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        serializer.collect_str(&format_args!("{}%", self.0))
    }
}
#[cfg(feature = "serde")]
impl<'de> serde::Deserialize<'de> for Percent {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        struct PercentVisitor;

        impl serde::de::Visitor<'_> for PercentVisitor {
            type Value = Percent;

            fn expecting(&self, formatter: &mut std::fmt::Formatter) -> std::fmt::Result {
                formatter.write_str("a percentage like \"50%\"")
            }

            fn visit_str<E>(self, value: &str) -> Result<Percent, E>
            where
                E: serde::de::Error,
            {
                let s = value.trim();
                let num_str = s.strip_suffix('%').ok_or_else(|| {
                    serde::de::Error::custom("expected percentage ending with '%'")
                })?;

                num_str
                    .trim()
                    .parse::<f32>()
                    .map(Percent)
                    .map_err(serde::de::Error::custom)
            }
        }

        deserializer.deserialize_str(PercentVisitor)
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Auto;
#[cfg(feature = "serde")]
impl serde::Serialize for Auto {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        serializer.serialize_str("auto")
    }
}
#[cfg(feature = "serde")]
impl<'de> serde::Deserialize<'de> for Auto {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        struct AutoVisitor;

        impl serde::de::Visitor<'_> for AutoVisitor {
            type Value = Auto;

            fn expecting(&self, formatter: &mut std::fmt::Formatter) -> std::fmt::Result {
                formatter.write_str("the string \"auto\"")
            }

            fn visit_str<E>(self, value: &str) -> Result<Auto, E>
            where
                E: serde::de::Error,
            {
                if value == "auto" {
                    Ok(Auto)
                } else {
                    Err(serde::de::Error::unknown_variant(value, &["auto"]))
                }
            }
        }

        deserializer.deserialize_str(AutoVisitor)
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Stretch;
#[cfg(feature = "serde")]
impl serde::Serialize for Stretch {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        serializer.serialize_str("stretch")
    }
}
#[cfg(feature = "serde")]
impl<'de> serde::Deserialize<'de> for Stretch {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        struct StretchVisitor;

        impl serde::de::Visitor<'_> for StretchVisitor {
            type Value = Stretch;

            fn expecting(&self, formatter: &mut std::fmt::Formatter) -> std::fmt::Result {
                formatter.write_str("the string \"stretch\"")
            }

            fn visit_str<E>(self, value: &str) -> Result<Stretch, E>
            where
                E: serde::de::Error,
            {
                if value == "stretch" {
                    Ok(Stretch)
                } else {
                    Err(serde::de::Error::unknown_variant(value, &["stretch"]))
                }
            }
        }

        deserializer.deserialize_str(StretchVisitor)
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct MinContent;
#[cfg(feature = "serde")]
impl serde::Serialize for MinContent {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        serializer.serialize_str("min-content")
    }
}
#[cfg(feature = "serde")]
impl<'de> serde::Deserialize<'de> for MinContent {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        struct MinContentVisitor;

        impl serde::de::Visitor<'_> for MinContentVisitor {
            type Value = MinContent;

            fn expecting(&self, formatter: &mut std::fmt::Formatter) -> std::fmt::Result {
                formatter.write_str("the string \"min-content\"")
            }

            fn visit_str<E>(self, value: &str) -> Result<MinContent, E>
            where
                E: serde::de::Error,
            {
                if value == "min-content" {
                    Ok(MinContent)
                } else {
                    Err(serde::de::Error::unknown_variant(value, &["min-content"]))
                }
            }
        }

        deserializer.deserialize_str(MinContentVisitor)
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct MaxContent;
#[cfg(feature = "serde")]
impl serde::Serialize for MaxContent {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        serializer.serialize_str("max-content")
    }
}
#[cfg(feature = "serde")]
impl<'de> serde::Deserialize<'de> for MaxContent {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        struct MaxContentVisitor;

        impl serde::de::Visitor<'_> for MaxContentVisitor {
            type Value = MaxContent;

            fn expecting(&self, formatter: &mut std::fmt::Formatter) -> std::fmt::Result {
                formatter.write_str("the string \"max-content\"")
            }

            fn visit_str<E>(self, value: &str) -> Result<MaxContent, E>
            where
                E: serde::de::Error,
            {
                if value == "max-content" {
                    Ok(MaxContent)
                } else {
                    Err(serde::de::Error::unknown_variant(value, &["max-content"]))
                }
            }
        }

        deserializer.deserialize_str(MaxContentVisitor)
    }
}

#[derive(
    Debug, Clone, Copy, PartialEq, PartialOrd, Default, derive_more::Deref, derive_more::DerefMut,
)]
pub struct FitContentPx(pub f32);
#[cfg(feature = "serde")]
impl serde::Serialize for FitContentPx {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        serializer.collect_str(&format_args!("fit-content({}px)", self.0))
    }
}
#[cfg(feature = "serde")]
impl<'de> serde::Deserialize<'de> for FitContentPx {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        struct FitContentPxVisitor;

        impl serde::de::Visitor<'_> for FitContentPxVisitor {
            type Value = FitContentPx;

            fn expecting(&self, formatter: &mut std::fmt::Formatter) -> std::fmt::Result {
                formatter.write_str("a value like \"fit-content(100px)\"")
            }

            fn visit_str<E>(self, value: &str) -> Result<FitContentPx, E>
            where
                E: serde::de::Error,
            {
                let s = value.trim();
                let inner = s
                    .strip_prefix("fit-content(")
                    .and_then(|s| s.strip_suffix(')'))
                    .ok_or_else(|| serde::de::Error::custom("expected format fit-content(...)"))?
                    .trim();

                let num_str = inner
                    .strip_suffix("px")
                    .ok_or_else(|| serde::de::Error::custom("expected length unit 'px'"))?;

                num_str
                    .trim()
                    .parse::<f32>()
                    .map(FitContentPx)
                    .map_err(serde::de::Error::custom)
            }
        }

        deserializer.deserialize_str(FitContentPxVisitor)
    }
}

#[derive(
    Debug, Clone, Copy, PartialEq, PartialOrd, Default, derive_more::Deref, derive_more::DerefMut,
)]
pub struct FitContentPercent(pub f32);
#[cfg(feature = "serde")]
impl serde::Serialize for FitContentPercent {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        serializer.collect_str(&format_args!("fit-content({}%)", self.0))
    }
}
#[cfg(feature = "serde")]
impl<'de> serde::Deserialize<'de> for FitContentPercent {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        struct Visitor;

        impl serde::de::Visitor<'_> for Visitor {
            type Value = FitContentPercent;

            fn expecting(&self, formatter: &mut std::fmt::Formatter) -> std::fmt::Result {
                formatter.write_str("a value like \"fit-content(50%)\"")
            }

            fn visit_str<E>(self, value: &str) -> Result<FitContentPercent, E>
            where
                E: serde::de::Error,
            {
                let s = value.trim();

                let inner = s
                    .strip_prefix("fit-content(")
                    .and_then(|s| s.strip_suffix(')'))
                    .ok_or_else(|| serde::de::Error::custom("expected format fit-content(...)"))?
                    .trim();

                let num_str = inner
                    .strip_suffix('%')
                    .ok_or_else(|| serde::de::Error::custom("expected percentage unit '%'"))?;

                num_str
                    .trim()
                    .parse::<f32>()
                    .map(FitContentPercent)
                    .map_err(serde::de::Error::custom)
            }
        }

        deserializer.deserialize_str(Visitor)
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct FitContent;
#[cfg(feature = "serde")]
impl serde::Serialize for FitContent {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        serializer.serialize_str("fit-content")
    }
}
#[cfg(feature = "serde")]
impl<'de> serde::Deserialize<'de> for FitContent {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        struct FitContentVisitor;

        impl serde::de::Visitor<'_> for FitContentVisitor {
            type Value = FitContent;

            fn expecting(&self, formatter: &mut std::fmt::Formatter) -> std::fmt::Result {
                formatter.write_str("the string \"fit-content\"")
            }

            fn visit_str<E>(self, value: &str) -> Result<FitContent, E>
            where
                E: serde::de::Error,
            {
                if value == "fit-content" {
                    Ok(FitContent)
                } else {
                    Err(serde::de::Error::unknown_variant(value, &["fit-content"]))
                }
            }
        }

        deserializer.deserialize_str(FitContentVisitor)
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Content;
#[cfg(feature = "serde")]
impl serde::Serialize for Content {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        serializer.serialize_str("content")
    }
}
#[cfg(feature = "serde")]
impl<'de> serde::Deserialize<'de> for Content {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        struct ContentVisitor;

        impl serde::de::Visitor<'_> for ContentVisitor {
            type Value = Content;

            fn expecting(&self, formatter: &mut std::fmt::Formatter) -> std::fmt::Result {
                formatter.write_str("the string \"content\"")
            }

            fn visit_str<E>(self, value: &str) -> Result<Content, E>
            where
                E: serde::de::Error,
            {
                if value == "content" {
                    Ok(Content)
                } else {
                    Err(serde::de::Error::unknown_variant(value, &["content"]))
                }
            }
        }

        deserializer.deserialize_str(ContentVisitor)
    }
}
