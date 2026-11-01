pub mod accessibility;
pub mod background;
pub mod border;
pub mod button;
pub mod color;
pub mod context_menu;
pub mod css;
pub mod cursor;
pub mod div;
pub mod draggable;
pub mod external_texture;
pub mod external_visual;
pub mod flexbox;
pub mod focusable;
pub mod gradient;
pub mod hover;
pub mod input;
pub mod outline;
pub mod position;
pub mod resizable;
pub mod scrollbar;
pub mod size;
pub mod stress_test;
pub mod transform;

use crate::{ALLOW_STRESS_TEST, app::theme::Theme};
use michiu_ui::prelude::*;

#[derive(Debug, Clone, Copy, PartialEq, Eq, strum::Display, strum::EnumIter)]
pub enum ComponentType {
    Button,
    Input,
    Scrollbar,
    Border,
    ContextMenu,
    Background,
    Div,
    Flexbox,
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
    ExternalVisual,
    ExternalTexture,
    Accessibility,
    Size,
    Transform,
    Position,
}

pub(crate) fn create_component_entity(comp_type: ComponentType) -> Entity {
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
        ComponentType::Position => wrapper(position::container()),
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

pub(crate) fn section_title(text_val: &'static str) -> Entity {
    text(text_val).modify(
        ii().text_color(dynamic(|t: &Theme| t.text_muted))
            .font_size(14.0)
            .font_weight(400),
    )
}

pub(crate) fn label_style() -> Modifier {
    ii().text_color(dynamic(|t: &Theme| t.text_muted))
        .font_size(16.0)
        .font_weight(400)
}
