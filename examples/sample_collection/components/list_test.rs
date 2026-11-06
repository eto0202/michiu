use crate::app::{sidebar, theme::Theme};
use michiu::prelude::*;

pub fn container() -> Entity {
    v_flex_d(|t: &Theme| sidebar::scrollbar(t).size_full().overflow_scroll()).children(entities_1())
}

#[inline]
fn entities_1() -> Vec<Entity> {
    let mut children = Vec::with_capacity(2000);

    for _ in 0..=2000 {
        let e = h_flex(
            ii().h(30.0)
                .w(5000.0)
                .shrink_0()
                .static_()
                .visibility_auto()
                .border_bottom(BorderStyle::Solid, 1.0)
                .border_color(dynamic(|t: &Theme| t.border)),
        )
        .children(entities_2());
        children.push(e);
    }
    children
}

#[inline]
fn entities_2() -> Vec<Entity> {
    let mut children = Vec::with_capacity(100);

    for _ in 0..=100 {
        let e = div(ii()
            .h_full()
            .w(50.0)
            .static_()
            .visibility_auto()
            .border_left(BorderStyle::Solid, 1.0)
            .border_color(dynamic(|t: &Theme| t.border))
            .hovered(ii().bg_color(dynamic(|t: &Theme| t.accent))))
        .on_click_with(|cx| println!("Current ID: {:?}", cx.current()));
        children.push(e);
    }
    children
}
