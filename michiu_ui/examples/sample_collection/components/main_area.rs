use michiu_ui::Element;
pub use michiu_ui::prelude::*;

pub fn main_area() -> Element {
    h_flex(ts().h_full().grow()).child(text("Main Area"))
}
