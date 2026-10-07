use crate::app::theme::Theme;
pub use michiu_ui::prelude::*;

pub fn container() -> Element {
    v_flex(ts().gap(16.0).p(16.0).r(4.0))
        .children([all_section_relative().child(all_section_absolute())])
}

fn all_section_relative() -> Element {
    // 上下: resizable_y(value)
    // 左右： resizable_x(value)
    // 四辺全て： resizable_all(value)
    // ここでは見本として一つずつ設定する
    let layout = ts()
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
        .style_d(|t: &Theme| {
            ts().bg_color(t.background)
                .border_color(t.border)
                .hovered(ts().bg_color(t.background_hover))
        })
        .label("Relative", label_style())
}

fn all_section_absolute() -> Element {
    let layout = ts()
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
        .style_d(|t: &Theme| {
            ts().bg_color(t.background)
                .border_color(t.border)
                .hovered(ts().bg_color(t.background_hover))
        })
        .label("Absolute", label_style())
}

fn label_style() -> ThisStyle {
    ts().text_color(dynamic(|t: &Theme| t.text_muted))
        .font_size(16.0)
        .self_start()
}
