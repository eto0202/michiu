use crate::{
    app::{ComponentType, sidebar, theme::Theme},
    components::{background, border, button, css, div, draggable, hover, outline, resizable},
};
pub use michiu_ui::prelude::*;
use strum::IntoEnumIterator;

pub fn main_area() -> Element {
    let main_contents = v_flex_d(|t: &Theme| {
        sidebar::scrollbar(t)
            .grow()
            .overflow_y_scroll()
            .overflow_x_hidden()
            .p(20.0)
    });

    // 全ての ComponentType の要素を起動時に一度だけ spawn してマウント
    let mut children = Vec::new();
    for comp_type in ComponentType::iter() {
        let child_el = create_component_element(comp_type);

        // 現在の ComponentType と一致しているか否かを動的に解決
        let styled_child = div_d(move |active: &ComponentType| {
            if *active == comp_type {
                // 表示状態のスタイル
                ts().flex() // hidden() と対応させる
                    .grow()
                    .overflow_hidden()
            } else {
                // Display::None を適用して Context 内に常駐
                ts().hidden()
            }
        })
        .child(child_el);

        children.push(styled_child);
    }

    main_contents.children(children)
}

fn create_component_element(comp_type: ComponentType) -> Element {
    match comp_type {
        ComponentType::Button => wrapper(button::container()),
        ComponentType::Input => text("Input"),
        ComponentType::InputArea => text("InputArea"),
        ComponentType::Background => wrapper(background::container()),
        ComponentType::Div => wrapper(div::container()),
        ComponentType::Dropdown => text("Dropdown"),
        ComponentType::Flexbox => text("Flexbox"),
        ComponentType::Border => wrapper(border::container()),
        ComponentType::Card => text("Card"),
        ComponentType::List => text("List"),
        ComponentType::Scrollbar => text("Scrollbar"),
        ComponentType::Hover => wrapper(hover::container()),
        ComponentType::Color => text("Color"),
        ComponentType::Resizable => wrapper(resizable::container()),
        ComponentType::Draggable => wrapper(draggable::container()),
        ComponentType::Css => wrapper(css::container()),
        ComponentType::Gradient => text("Gradient"),
        ComponentType::Cursor => text("Cursor"),
        ComponentType::Outline => wrapper(outline::container()),
    }
}

fn wrapper(el: Element) -> Element {
    v_flex_d(|t: &Theme| {
        ts().grow()
            .r(4.0)
            .border_dashed(1.0)
            .border_color(t.border)
            .overflow_hidden()
    })
    .child(el)
}
