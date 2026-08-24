pub mod header;
pub mod main_area;
pub mod sidebar;
pub mod theme;

use crate::app::theme::Theme;
use michiu_ui::{DssSet, prelude::*};

pub fn create_root(s: ReadSignal<DssSet>) -> Element {
    let (t, _) = create_signal(Theme::dark());
    let (c, _) = create_signal(ComponentType::Div);

    let (search_text, _) = create_signal(SearchText(String::new()));
    let (sort_order, _) = create_signal(SidebarSortOrder::Ascending);

    v_flex(move || ts().size_full().bg_color(t.get().background))
        .provide(t)
        .provide(c)
        .provide(s)
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
}
