use crate::theme::Theme;
pub use michiu_ui::prelude::*;
use std::time::Duration;

pub fn toggle_btn() -> Element {
    let mode_change = || {
        let t = use_provided::<Theme>().get();
        let set_t = use_provided_setter::<Theme>();

        if t.is_dark {
            set_t.set(Theme::light());
        } else {
            set_t.set(Theme::dark());
        }
    };

    let btn_wrapper = h_flex_c(|t: &Theme| {
        // ダーク（Active）時はアクセントカラーや hover 色を、ライト時は標準の境界色を割り当てます
        let track_color = if t.is_dark { t.surface } else { t.border };

        ts().r(4.0)
            .items_center()
            .size((50.0, 26.0))
            .bg_color(track_color)
            .absolute()
            .right(10.0)
    })
    .on_click(mode_change);

    let btn_inner = h_flex_c(|t: &Theme| {
        // ダーク（Active）時は右端（22.0pxシフト）、ライト時は左端（0.0px）
        let tx = if t.is_dark { 22.0 } else { 0.0 };

        ts().r(4.0)
            .m_l(4.0) // 左端に 4.0px の余白を設けてスタート
            .height(20.0)
            .width(20.0)
            .bg_color(t.text_muted)
            // Transform を使って X 軸方向に平行移動
            .transform_translate(tx, 0.0)
            // つまみの移動に滑らかなイージングを適用
            .trans_transform(Duration::from_millis(150), AnimationCurve::EaseInOutQuad)
    })
    .on_click(mode_change);

    btn_wrapper.child(btn_inner)
}
