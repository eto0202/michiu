use crate::app::theme::Theme;
use michiu::{prelude::*, ui::a11y};

pub fn container() -> Element {
    v_flex(ts().gap(16.0).p(16.0).size_full())
        .on_right_click_with(|cx| {
            let pos = cx.pointer_position().unwrap_or_default();
            let menu_style = ts().flex().top(pos.y).left(pos.x + 12.0);

            let Some(el) = cx.try_query_first::<a11y::AContextMenu>() else {
                let root = cx.find_root_entity().expect("Not Found Root EntityId");
                let menu = context_menu().style(menu_style);
                let _ = root.into_el().child(menu);
                return;
            };
            let _ = el.style(menu_style);
        })
        .on_click_with(|cx| {
            let _ = cx.query_first::<a11y::AContextMenu>().style(ts().hidden());
        })
}

fn context_menu() -> Element {
    v_flex(
        ts().absolute()
            .r(4.0)
            .p(8.0)
            .size((150.0, 250.0))
            .hidden()
            .items_center()
            .bg_color(Color::DARK_GRAY),
    )
    .tag::<a11y::AContextMenu>()
    .children(menu_item())
}

fn menu_item() -> [Element; 8] {
    let item = |label: &'static str| {
        flex(ts().size_full()).tag::<a11y::AMenuItem>().label(
            label,
            ts().font_size(14.0).text_color(dynamic(|t: &Theme| t.text)),
        )
    };

    [
        item("Menu 1"),
        item("Menu 2"),
        item("Menu 3"),
        item("Menu 4"),
        item("Menu 5"),
        item("Menu 6"),
        item("Menu 7"),
        item("Menu 8"),
    ]
}
