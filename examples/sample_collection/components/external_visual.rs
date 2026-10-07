pub use michiu::prelude::*;
use michiu::ui::{FocusTrigger, external_visual};

use crate::{GitHubVisual, CratesVisual};

pub fn container() -> Element {
    v_flex(ts().p(10.0).r(4.0).size_full()).child_d(|github: &GitHubVisual| webview2(github))
}

fn webview2(github: &GitHubVisual) -> Element {
    let (is_active, set_is_active) = create_signal(false);
    let (is_opacity, set_is_opacity) = create_signal(false);

    let visual = external_visual(github.0.clone())
        .style({
            let base = ts()
                .size_full()
                .focusable_self(FocusTrigger::Both)
                .r(4.0)
                .resizable_bottom(true)
                .dnd_droppable(DndDropTarget::Child, DndDragPayload::Element)
                .overflow_hidden();

            is_opacity.get_else(base.clone().opacity_50(), base.opacity_100())
        })
        .on_focus(move || set_is_active.set(false))
        .child(
            v_flex({
                let base = ts()
                    .absolute()
                    .size((600.0, 500.0))
                    .p(20.0)
                    .r(3.0)
                    .inset_x(100.0)
                    .inset_y(50.0)
                    .bg_color(Color::DARK_GRAY)
                    .opacity(0.9)
                    .resizable_all(true)
                    .dnd_draggable_root(DndDragPayload::Element, true)
                    .dnd_draggable_original(ts().opacity_0())
                    .dnd_draggable_placeholder(
                        ts().size(100.0)
                            .r(3.0)
                            .bg_color(Color::DARK_GRAY)
                            .opacity(0.9),
                    );

                is_active.get_else(base.clone().flex(), base.hidden())
            })
            .child_d(|youtube: &CratesVisual| {
                external_visual(youtube.0.clone()).style(ts().r(3.0).size_full())
            }),
        );

    let btn_style = |color: Color| {
        ts().justify_center()
            .items_center()
            .r(3.0)
            .size((100.0, 40.0))
            .border_dashed(1.0)
            .border_color(color)
            .hovered(ts().border_solid(1.0))
            .actived(ts().border_solid(1.0).bg_color(color))
    };

    let overlay_btn = h_flex(move || btn_style(Color::BLUE))
        .label("Overlay", ts().font_size(14.0).text_color(Color::WHITE))
        .active(is_active)
        .on_click(move || set_is_active.set(!is_active.get()));

    let opacity_btn = h_flex(move || btn_style(Color::CYAN))
        .label("Opacity", ts().font_size(14.0).text_color(Color::WHITE))
        .active(is_opacity)
        .on_click(move || set_is_opacity.set(!is_opacity.get()));

    v_flex(
        ts().r(4.0)
            .size_full()
            .gap(8.0)
            .justify_center()
            .items_center(),
    )
    .children([
        visual,
        h_flex(
            ts().p(8.0)
                .gap(8.0)
                .w_full()
                .h_auto()
                .justify_center()
                .items_center(),
        )
        .children([overlay_btn, opacity_btn]),
    ])
}
