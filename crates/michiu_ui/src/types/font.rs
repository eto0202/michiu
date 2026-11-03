use std::borrow::Cow;

use cosmic_text::{Family, Weight};

#[derive(Debug, Clone, PartialEq)]
pub struct FontDate {
    pub size: Option<f32>,
    pub line_height: Option<LineHeight>,
    pub family: Option<FontFamily>,
    pub weight: Option<FontWeight>,
    pub style: Option<FontStyle>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Default)]
pub enum FontStyle {
    #[default]
    Normal,
    Italic,
    Oblique,
}

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub enum FontFamily {
    Name(Cow<'static, str>),
    Serif,
    SansSerif,
    Cursive,
    Fantasy,
    Monospace,
}

impl FontFamily {
    pub fn name(name: impl Into<Cow<'static, str>>) -> Self {
        Self::Name(name.into())
    }

    pub(crate) fn as_family(&self) -> Family<'_> {
        match self {
            FontFamily::Name(cow) => Family::Name(cow.as_ref()),
            FontFamily::Serif => Family::Serif,
            FontFamily::SansSerif => Family::SansSerif,
            FontFamily::Cursive => Family::Cursive,
            FontFamily::Fantasy => Family::Fantasy,
            FontFamily::Monospace => Family::Monospace,
        }
    }
}

#[derive(Clone, Copy, PartialOrd, Ord, PartialEq, Eq, Debug, Hash)]
pub struct FontWeight(pub u16);
impl Default for FontWeight {
    #[inline]
    fn default() -> FontWeight {
        FontWeight::NORMAL
    }
}
impl FontWeight {
    pub(crate) fn as_weight(self) -> Weight {
        Weight(self.0)
    }
    /// Thin weight (100), the thinnest value.
    pub const THIN: FontWeight = FontWeight(100);
    /// Extra light weight (200).
    pub const EXTRA_LIGHT: FontWeight = FontWeight(200);
    /// Light weight (300).
    pub const LIGHT: FontWeight = FontWeight(300);
    /// Normal (400).
    pub const NORMAL: FontWeight = FontWeight(400);
    /// Medium weight (500, higher than normal).
    pub const MEDIUM: FontWeight = FontWeight(500);
    /// Semibold weight (600).
    pub const SEMIBOLD: FontWeight = FontWeight(600);
    /// Bold weight (700).
    pub const BOLD: FontWeight = FontWeight(700);
    /// Extra-bold weight (800).
    pub const EXTRA_BOLD: FontWeight = FontWeight(800);
    /// Black weight (900), the thickest value.
    pub const BLACK: FontWeight = FontWeight(900);
}

#[derive(Debug, Clone, Copy, PartialEq, PartialOrd)]
pub enum LineHeight {
    Value(f32),
    Factor(f32),
}
impl Default for LineHeight {
    fn default() -> Self {
        Self::Factor(Self::LINE_HEIGHT_FACTOR)
    }
}
impl LineHeight {
    pub(crate) const LINE_HEIGHT_FACTOR: f32 = 1.4;

    #[inline]
    pub(crate) fn clac_height(self, font_size: f32) -> f32 {
        match self {
            LineHeight::Value(v) => v,
            LineHeight::Factor(f) => font_size * f,
        }
    }
}

impl Default for FontDate {
    fn default() -> Self {
        Self {
            size: Some(FontDate::FONT_SIZE),
            line_height: Some(LineHeight::default()),
            family: None,
            weight: None,
            style: None,
        }
    }
}

impl FontDate {
    pub(crate) const FONT_SIZE: f32 = 16.0;

    #[inline]
    pub(crate) fn line_height(&self) -> f32 {
        self.line_height
            .unwrap_or_default()
            .clac_height(self.size.unwrap_or_default())
    }
}
