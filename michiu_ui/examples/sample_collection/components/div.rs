use crate::app::theme::Theme;
use michiu_ui::prelude::*;

// ここでは再利用しやすいように comsume ではなく引数で Theme を受け取る
pub fn container() -> Element {
    // false = row, true = col
    let (is_column, set_is_column) = create_signal(false);

    div_n()
        .style(move || {
            let base = ts()
                .h_full()
                .flex()
                .gap(20.0)
                .p(16.0)
                .overflow_hidden()
                .hovered(ts().bg_color(dynamic(|t: &Theme| t.background_hover)));

            // シグナル状態に応じて flex_direction を動的に切り替え
            let is_col = is_column.get();
            if is_col {
                base.flex_col()
            } else {
                base.flex_row()
            }
        })
        .label(is_column.get_else("v_flex", "h_flex"), label_style())
        .children([
            flex_container().children([div_container(), div_container()]),
            flex_container().children([div_container(), div_container()]),
        ])
        .on_click(move || {
            set_is_column.set(!is_column.get());
        })
}

fn flex_container() -> Element {
    let (is_row, set_is_row) = create_signal(false);

    div_n()
        .style(move || {
            let base = ts()
                .flex()
                .grow()
                .r(4.0)
                .p(20.0)
                .gap(20.0)
                .border_dashed(1.0)
                .border_color(dynamic(|t: &Theme| t.border))
                .overflow_hidden()
                .hovered(ts().bg_color(dynamic(|t: &Theme| t.background_hover)));

            let is_row = is_row.get();
            if is_row {
                base.flex_row()
            } else {
                base.flex_col()
            }
        })
        .label(is_row.get_else("h_flex", "v_flex"), label_style())
        .on_click(move || {
            set_is_row.set(!is_row.get());
        })
}

fn div_container() -> Element {
    div(ts()
        .grow()
        .r(4.0)
        .p(20.0)
        .gap(20.0)
        .border_dashed(1.0)
        .border_color(dynamic(|t: &Theme| t.border))
        .overflow_hidden()
        .hovered(ts().bg_color(dynamic(|t: &Theme| t.background_hover))))
    .label("div", label_style())
}

fn label_style() -> ThisStyle {
    ts().text_color(dynamic(|t: &Theme| t.text_muted))
        .font_size(30.0)
}
