use std::borrow::Cow;

#[derive(Debug, Clone, PartialEq)]
pub struct FontDate {
    pub size: Option<f32>,
    pub family: Option<Cow<'static, str>>,
    pub weight: Option<u32>,
    pub style: Option<u32>,
}

impl Default for FontDate {
    fn default() -> Self {
        Self {
            size: Some(FontDate::FONT_SIZE),
            family: None,
            weight: None,
            style: None,
        }
    }
}

impl FontDate {
    pub(crate) const FONT_SIZE: f32 = 16.0;
}
