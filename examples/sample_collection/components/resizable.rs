use crate::app::theme::Theme;
pub use michiu::prelude::*;

pub fn container() -> Entity {
    v_flex(ii().gap(16.0).p(16.0).r(4.0))
        .children([all_section_relative().child(all_section_absolute())])
}

fn all_section_relative() -> Entity {
    // 上下: resizable_y(value)
    // 左右： resizable_x(value)
    // 四辺全て： resizable_all(value)
    // ここでは見本として一つずつ設定する
    let layout = ii()
        .p(16.0)
        .r(4.0)
        .size(400.0)
        .resizable_top(true)
        .resizable_right(true)
        .resizable_bottom(true)
        .resizable_left(true)
        // resizable_cursor_default() と同じ
        .resizable_cursor(None, None, None, None)
        .border_solid(1.0)
        .overflow_hidden();

    h_flex(layout)
        .modify_d(|t: &Theme| {
            ii().bg_color(t.background)
                .border_color(t.border)
                .hovered(ii().bg_color(t.background_hover))
        })
        .label("Relative", label_style())
}

fn all_section_absolute() -> Entity {
    let layout = ii()
        .resizable_all(true)
        .absolute()
        .top(50.0)
        .left(50.0)
        .size(200.0)
        .p(16.0)
        .r(4.0)
        .border_solid(1.0)
        .overflow_hidden();

    h_flex(layout)
        .modify_d(|t: &Theme| {
            ii().bg_color(t.background)
                .border_color(t.border)
                .hovered(ii().bg_color(t.background_hover))
        })
        .label("Absolute", label_style())
}

fn label_style() -> Modifier {
    ii().text_color(dynamic(|t: &Theme| t.text_muted))
        .font_size(16.0)
        .self_start()
}
