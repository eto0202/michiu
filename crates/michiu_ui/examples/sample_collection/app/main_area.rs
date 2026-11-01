use crate::{app::ComponentType, components::create_component_entity};
use michiu_ui::a11y;
use michiu_ui::prelude::*;
use strum::IntoEnumIterator;

pub fn main_area() -> Entity {
    let main_contents = v_flex(ii().size_full().overflow_hidden().p(20.0)).tag::<a11y::AMain>();

    // 全ての ComponentType の要素を起動時に一度だけ spawn してマウント
    let mut children = Vec::new();
    for comp_type in ComponentType::iter() {
        let child_el = create_component_entity(comp_type);

        // 現在の ComponentType と一致しているか否かを動的に解決
        let styled_child = flex_d(move |active: &ComponentType| {
            if *active == comp_type {
                // 表示状態のスタイル
                ii().size_full().flex() // hidden() と対応させる
            } else {
                // Display::None を適用して Context 内に常駐
                ii().hidden()
            }
        })
        .child(child_el);

        children.push(styled_child);
    }

    main_contents.children(children)
}
