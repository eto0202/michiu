use crate::{
    ALLOW_STRESS_TEST,
    app::{ComponentType, theme::Theme},
    components::{
        accessibility, background, border, button, color, context_menu, css, cursor, div,
        draggable, external_texture, external_visual, flexbox, focusable, gradient, hover, input,
        outline, resizable, scrollbar, stress_test, transform,
    },
};
use michiu::prelude::*;
use michiu::ui::a11y;
use strum::IntoEnumIterator;

pub fn main_area() -> Entity {
    let main_contents = v_flex(ii().size_full().overflow_hidden().p(20.0)).tag::<a11y::AMain>();

    // 全ての ComponentType の要素を起動時に一度だけ spawn してマウント
    // Spawn and mount all elements of each ComponentType once at startup
    let mut children = Vec::new();
    for comp_type in ComponentType::iter() {
        let child_el = create_component_entity(comp_type);

        // 現在の ComponentType と一致しているか否か
        // Whether it matches the current ComponentType
        let styled_child = flex_d(move |active: &ComponentType| {
            if *active == comp_type {
                // 表示状態のスタイル
                // Display State Styles
                ii().size_full().flex()
            } else {
                // Display::None を適用して Context 内に常駐
                // Apply `Display::None` to keep it resident within the Context
                ii().hidden()
            }
        })
        .child(child_el);

        children.push(styled_child);
    }

    main_contents.children(children)
}

fn create_component_entity(comp_type: ComponentType) -> Entity {
    match comp_type {
        ComponentType::Button => wrapper(button::container()),
        ComponentType::Input => wrapper(input::container()),
        ComponentType::Background => wrapper(background::container()),
        ComponentType::Div => wrapper(div::container()),
        ComponentType::Flexbox => wrapper(flexbox::container()),
        ComponentType::Border => wrapper(border::container()),
        ComponentType::ContextMenu => wrapper(context_menu::container()),
        ComponentType::Scrollbar => wrapper(scrollbar::container()),
        ComponentType::Hover => wrapper(hover::container()),
        ComponentType::Color => wrapper(color::container()),
        ComponentType::Resizable => wrapper(resizable::container()),
        ComponentType::Draggable => wrapper(draggable::container()),
        ComponentType::Css => wrapper(css::container()),
        ComponentType::Gradient => wrapper(gradient::container()),
        ComponentType::Cursor => wrapper(cursor::container()),
        ComponentType::Outline => wrapper(outline::container()),
        ComponentType::Focusable => wrapper(focusable::container()),
        ComponentType::ExternalTexture => wrapper(external_texture::container()),
        ComponentType::ExternalVisual => wrapper(external_visual::container()),
        ComponentType::Accessibility => wrapper(accessibility::container()),
        ComponentType::Size => text("Size"),
        ComponentType::Transform => wrapper(transform::container()),
        ComponentType::StressTest => {
            if ALLOW_STRESS_TEST {
                wrapper(stress_test::container())
            } else {
                text("StressTest")
            }
        }
    }
}

fn wrapper(e: Entity) -> Entity {
    v_flex_d(|t: &Theme| {
        ii().size_full()
            .r(4.0)
            .overflow_hidden()
            .border_dashed(1.0)
            .border_color(t.border)
    })
    .child(e)
}
