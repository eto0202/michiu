use crate::app::theme::Theme;
use michiu_ui::prelude::*;

pub fn container() -> Entity {
    v_flex(ii().p(16.0).gap(16.0).r(4.0).w_full()).children([
        text("↓↓↓ Click Section ↓↓↓").modify(common_label()),
        section_1(),
    ])
}

fn common_label() -> Modifier {
    ii().text_color(dynamic(|t: &Theme| t.text))
}

fn section_1() -> Entity {
    v_flex(
        ii().relative()
            .w(500.0)
            .gap(12.0)
            .p(12.0)
            .r(4.0)
            .border_dashed(1.0)
            .border_color(dynamic(|t: &Theme| t.border)),
    )
    .children([section_2(), section_2(), section_2().child(section_3())])
}

fn section_2() -> Entity {
    let (is_relative, set_is_relative) = create_signal(true);

    div(move || {
        let s = ii()
            .p(12.0)
            .r(4.0)
            .left(20.0) // This does not apply to Static.
            .size(160.0)
            .border_solid(1.0)
            .border_color(dynamic(|t: &Theme| t.border));

        if is_relative.get() {
            s.relative()
        } else {
            s.static_()
        }
    })
    .label(
        move || is_relative.get_else("Relative", "Static")(),
        common_label(),
    )
    .on_click(move || set_is_relative.set(!is_relative.get()))
}

fn section_3() -> Entity {
    let (is_absolute, set_is_absolute) = create_signal(true);

    // `top()`, `left()`, `right()`, `bottom()` and `inset()`
    //
    // # Based
    // `absolute()` -> Parent
    // `fixed()` -> Window
    div(move || {
        let s = ii()
            .p(4.0)
            .r(4.0)
            .size(80.0)
            .top(50.0)
            .left(30.0)
            .bg_color(dynamic(|t: &Theme| t.accent));

        if is_absolute.get() {
            s.absolute()
        } else {
            s.fixed()
        }
    })
    .label(
        move || is_absolute.get_else("Absolute", "Fixed")(),
        ii().text_color(dynamic(|t: &Theme| t.text_on_accent)),
    )
    .on_click(move || set_is_absolute.set(!is_absolute.get()))
}

fn _sticky_section() -> Entity {
    div_n()
}
