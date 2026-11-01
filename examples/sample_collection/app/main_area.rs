use crate::app::ComponentType;
use crate::components::create_component_entity;
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
