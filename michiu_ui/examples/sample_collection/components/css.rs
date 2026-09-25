pub use michiu_ui::prelude::*;
use michiu_ui::{CssMapSet, CssSetSignalExt};

use crate::app::theme::Theme;

pub fn container() -> Element {
    v_flex(ts().gap(16.0).p(16.0).r(4.0)).children([css_flex(), merge_border()])
}

// flex_1 と flex_2 は同じ ThisStyle を持つ
pub fn css_flex() -> Element {
    div_n().child(move || {
        let css = use_provided::<CssMapSet>().sheet("global");
        let flex_1 = css.class("flex-1");
        let flex_2 = css.class( "flex-2");

        div(flex_1).children([
            h_flex(&flex_2),
            h_flex(&flex_2).children([h_flex(&flex_2), h_flex(&flex_2)]),
        ])
    })
}

pub fn merge_border() -> Element {
    div_n().child(move || {
        let css = use_provided::<CssMapSet>();
        let border = css.class("global", "merge-border");
        let cursor = css.class("global", "merge-cursor");

        div(border.border_dashed(2.0)) // マージ
            .child(div(ts()
                // width: 100%; 追加
                .p(16.0) // 追加
                .border_bottom(BorderStyle::Dashed, 2.0) // マージ
                .border_color(Color::BLACK) // マージ
                .bg_color(dynamic(|t: &Theme| t.background)) // 追加
                .cursor_text() // マージ
                .merge(&cursor)
                .border_bottom(BorderStyle::Dashed, 2.0))) // 再マージ
    })
}
