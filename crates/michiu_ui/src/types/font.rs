use std::borrow::Cow;

use cosmic_text::Family;

#[derive(Debug, Clone, PartialEq)]
pub struct FontDate {
    pub size: Option<f32>,
    pub line_height: Option<LineHeight>,
    pub family: Option<FontFamily>,
    pub weight: Option<u16>,
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
