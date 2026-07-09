use crate::theme::Theme;
pub use michiu_ui::prelude::*;

pub fn main_area() -> Element {
    h_flex(ts().h_full().grow()).label_c("Main Area", |t: &Theme| ts().text_color(t.text))
}
