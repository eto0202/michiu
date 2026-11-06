pub mod button;
pub mod external_texture;
pub mod external_visual;
pub mod stress_test;

use crate::{ALLOW_STRESS_TEST, app::theme::Theme};
use michiu_ui::prelude::*;

#[derive(Debug, Clone, Copy, PartialEq, Eq, strum::Display, strum::EnumIter)]
pub enum ComponentType {
    Button,
    StressTest,
    ExternalVisual,
    ExternalTexture,
}

pub(crate) fn create_component_entity(comp_type: ComponentType) -> Entity {
    match comp_type {
        ComponentType::Button => wrapper(button::container()),
        ComponentType::ExternalTexture => wrapper(external_texture::container()),
        ComponentType::ExternalVisual => wrapper(external_visual::container()),
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
            .r(3.0)
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
