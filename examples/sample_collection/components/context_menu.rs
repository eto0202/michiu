use crate::app::theme::Theme;
use michiu::{prelude::*, ui::a11y};

const WIDTH: f32 = 150.0;
const HEIGHT: f32 = 250.0;

pub fn container() -> Entity {
    v_flex(
        ii().gap(16.0)
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

        let menu_style = ii().flex().top(top).left(left);

        let Some(e) = cx.try_query_first::<a11y::AContextMenu>() else {
            let root = cx.find_root_entity().expect("Not Found Root Entity");
            let menu = context_menu().modify(menu_style);
            let _ = root.child(menu);
            return;
        };
        let _ = e.modify(menu_style);
    })
    .on_click_with(|cx| {
        let _ = cx.query_first::<a11y::AContextMenu>().modify(ii().hidden());
    })
    .label(
        "Please right click",
        ii().text_color(dynamic(|t: &Theme| t.text)).font_size(20.0),
    )
}

fn context_menu() -> Entity {
    v_flex(
        ii().absolute()
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

fn menu_item() -> [Entity; 8] {
    let item = |label: &'static str| {
        flex(
            ii().r(3.0)
                .size_full()
                .p_l(8.0)
                .pressed(ii().bg_color(dynamic(|t: &Theme| t.border_hover)))
                .hovered(ii().bg_color(dynamic(|t: &Theme| t.border))),
        )
        .tag::<a11y::AMenuItem>()
        .on_click_with(|cx| {
            let _ = cx.query_first::<a11y::AContextMenu>().modify(ii().hidden());
        })
        .label(
            label,
            ii().font_size(14.0).text_color(dynamic(|t: &Theme| t.text)),
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
