use crate::app::theme::Theme;
use michiu::{prelude::*, ui::a11y};

const WIDTH: f32 = 150.0;
const HEIGHT: f32 = 250.0;

pub fn container() -> Element {
    v_flex(
        ts().gap(16.0)
            .p(16.0)
            .size_full()
            .justify_center()
            .items_center(),
    )
    .on_right_click_with(|cx| {
        let pos = cx.pointer_position().unwrap_or_default();
        let mut left = pos.x;
        let mut top = pos.y;

        let size = cx.window_last_size().unwrap_or_default();

        if size.width - left - WIDTH <= 16.0 {
            left = pos.x - WIDTH;
        }
        if size.height - top - HEIGHT <= 16.0 {
            top = pos.y - HEIGHT;
        }

        let menu_style = ts().flex().top(top).left(left);

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
    .label(
        "Please right click",
        ts().text_color(dynamic(|t: &Theme| t.text)).font_size(20.0),
    )
}

fn context_menu() -> Element {
    v_flex(
        ts().absolute()
            .r(4.0)
            .p(6.0)
            .size((WIDTH, HEIGHT))
            .hidden()
            .items_center()
            .border_solid(1.0)
            .border_color(dynamic(|t: &Theme| t.border))
            .bg_color(dynamic(|t: &Theme| t.background_hover)),
    )
    .tag::<a11y::AContextMenu>()
    .children(menu_item())
}

fn menu_item() -> [Element; 8] {
    let item = |label: &'static str| {
        flex(
            ts().r(3.0)
                .size_full()
                .p_l(8.0)
                .pressed(ts().bg_color(dynamic(|t: &Theme| t.border_hover)))
                .hovered(ts().bg_color(dynamic(|t: &Theme| t.border))),
        )
        .tag::<a11y::AMenuItem>()
        .on_click_with(|cx| {
            let _ = cx.query_first::<a11y::AContextMenu>().style(ts().hidden());
        })
        .label(
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
