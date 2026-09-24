pub mod header;
pub mod main_area;
pub mod sidebar;
pub mod theme;

use crate::app::theme::Theme;
use michiu_ui::prelude::*;

pub fn create_root() -> Element {
    let (theme, _) = create_signal(Theme::dark());
    let (comp_type, _) = create_signal(ComponentType::Div);

    let (search_text, _) = create_signal(SearchText(String::new()));
    let (sort_order, _) = create_signal(SidebarSortOrder::Ascending);

    v_flex(move || {
        ts().size_full()
            .bg_color(theme.get().background)
            .backdrop_acrylic() // 背景色は不透明のため見た目は変化しない
    })
    .provide(theme)
    .provide(comp_type)
    .provide(search_text)
    .provide(sort_order)
    .children([
        header::header(),
        h_flex(ts().grow().size_full().overflow_hidden())
            .children([sidebar::sidebar(), main_area::main_area()]),
    ])
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SearchText(String);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SidebarSortOrder {
    Ascending,
    Descending,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, strum::Display, strum::EnumIter)]
pub enum ComponentType {
    Button,
    Input,
    InputArea,
    Scrollbar,
    Border,
    Card,
    Background,
    Div,
    Flexbox,
    Dropdown,
    List,
    Hover,
    Color,
    Resizable,
    Draggable,
    Css,
    Gradient,
    Cursor,
    Outline,
    Focusable,
    StressTest,
    Image,
    ExternalVisual,
}
