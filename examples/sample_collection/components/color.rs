use crate::components::section_title;
use michiu::prelude::*;

pub fn container() -> Entity {
    v_flex(ii().gap(24.0).p(16.0)).children([
        rgb_container(),
        rgba_container(),
        hex_container(),
        hsl_container(),
        hsla_container(),
    ])
}

fn section_style() -> Modifier {
    ii().gap(12.0)
}

fn wrapper_style() -> Modifier {
    ii().w_full().gap((24.0, 10.0)).p(6.0)
}

fn item(label: &'static str, style: Modifier) -> Entity {
    let (show_popup, set_show_popup) = create_signal(false);
    let (popup_pos, set_popup_pos) = create_signal(LayoutPoint::ZERO);

    flex(
        style
            .p(10.0)
            .r(4.0)
            .size((70.0, 70.0))
            .justify_center()
            .items_center(),
    )
    .on_mouse_enter(move || set_show_popup.set(true))
    .on_mouse_leave(move || set_show_popup.set(false))
    .on_cursor_moved(move |pos| set_popup_pos.set(pos))
    .child(move || {
        if show_popup.get() {
            let pos = popup_pos.get();
            let offset_x = pos.x + 12.0;
            let offset_y = pos.y + 12.0;

            flex(
                ii().absolute()
                    .z_1()
                    .left(offset_x)
                    .top(offset_y)
                    .p((6.0, 10.0))
                    .r(4.0)
                    .bg_color(rgba(20, 20, 20, 0.95))
                    .pointer_events_none(),
            )
            .label(
                label,
                ii().text_color(rgba(210, 210, 210, 0.95))
                    .font_size(16.0)
                    .font_weight(400),
            )
        } else {
            div_n()
        }
    })
}

fn rgb_container() -> Entity {
    let red = item("rgb(255, 0, 0)", ii().bg_color(rgb(255, 0, 0)));
    let yellow = item_direct("rgb(255, 255, 0)", ii().bg_color(rgb(255, 255, 0)));
    let white = item_event("rgb(255, 255, 255)", ii().bg_color(rgb(255, 255, 255)));
    let lightblue = item_tag("rgb(0, 255, 255)", ii().bg_color(rgb(0, 255, 255)));

    let blue = item("rgb(0, 0, 255)", ii().bg_color(rgb(0, 0, 255)));
    let green = item("rgb(0, 255, 0)", ii().bg_color(rgb(0, 255, 0)));

    v_flex(section_style()).children([
        section_title("RGB"),
        h_flex(wrapper_style()).children([red, yellow, white, lightblue, blue, green]),
    ])
}

fn rgba_container() -> Entity {
    let black1 = item("rgba(0, 0, 0, 0.2)", ii().bg_color(rgba(0, 0, 0, 0.2)));
    let black2 = item("rgba(0, 0, 0, 0.4)", ii().bg_color(rgba(0, 0, 0, 0.4)));
    let black3 = item("rgba(0, 0, 0, 0.6)", ii().bg_color(rgba(0, 0, 0, 0.6)));
    let black4 = item("rgba(0, 0, 0, 0.8)", ii().bg_color(rgba(0, 0, 0, 0.8)));
    let black5 = item("rgba(0, 0, 0, 1.0)", ii().bg_color(rgba(0, 0, 0, 1.0)));

    v_flex(section_style()).children([
        section_title("RGBA"),
        h_flex(wrapper_style()).children([black1, black2, black3, black4, black5]),
    ])
}

fn hex_container() -> Entity {
    let red = item("hex(\"#ff0000\")", ii().bg_color(hex("#ff0000")));
    let green = item("hex(0x00FF00)", ii().bg_color(hex(0x00_FF00)));
    let blue = item("hex(0x0000ff)", ii().bg_color(hex(0x00_00ff)));
    let white = item("hex(\"00000080\")", ii().bg_color(hex("00000080")));
    let black = item("hex(\"0xFFFFFF80\")", ii().bg_color(hex("0xFFFFFF80")));

    v_flex(section_style()).children([
        section_title("HEX"),
        h_flex(wrapper_style()).children([red, green, blue, white, black]),
    ])
}

fn hsl_container() -> Entity {
    let primary = item(
        "hsl(318.0, 56.0, 59.0)",
        ii().bg_color(hsl(318.0, 56.0, 59.0)),
    );
    let hsl_1 = item(
        "hsl(318.0, 0.0, 59.0)",
        ii().bg_color(hsl(318.0, 0.0, 59.0)),
    );
    let hsl_2 = item(
        "hsl(318.0, 56.0, 0.0)",
        ii().bg_color(hsl(318.0, 56.0, 0.0)),
    );
    let hsl_3 = item("hsl(0.0, 56.0, 59.0)", ii().bg_color(hsl(0.0, 56.0, 59.0)));
    let hsl_4 = item("hsl(318.0, 0.0, 0.0)", ii().bg_color(hsl(318.0, 0.0, 0.0)));
    let hsl_5 = item(
        "hsl(318.0, 100.0, 100.0)",
        ii().bg_color(hsl(318.0, 100.0, 100.0)),
    );

    v_flex(section_style()).children([
        section_title("HSL"),
        h_flex(wrapper_style()).children([primary, hsl_1, hsl_2, hsl_3, hsl_4, hsl_5]),
    ])
}

fn hsla_container() -> Entity {
    let primary = item(
        "hsla(318.0, 56.0, 59.0, 1.0)",
        ii().bg_color(hsla(318.0, 56.0, 59.0, 1.0)),
    );
    let hsla_1 = item(
        "hsla(318.0, 56.0, 59.0, 0.8)",
        ii().bg_color(hsla(318.0, 56.0, 59.0, 0.8)),
    );
    let hsla_2 = item(
        "hsla(318.0, 56.0, 59.0, 0.6)",
        ii().bg_color(hsla(318.0, 56.0, 59.0, 0.6)),
    );
    let hsla_3 = item(
        "hsla(318.0, 56.0, 59.0, 0.4)",
        ii().bg_color(hsla(318.0, 56.0, 59.0, 0.4)),
    );
    let hsla_4 = item(
        "hsla(318.0, 56.0, 59.0, 0.2)",
        ii().bg_color(hsla(318.0, 56.0, 59.0, 0.2)),
    );

    v_flex(section_style()).children([
        section_title("HSLA"),
        h_flex(wrapper_style()).children([primary, hsla_1, hsla_2, hsla_3, hsla_4]),
    ])
}

// こういう実装方法も可能
fn item_direct(label: &'static str, style: Modifier) -> Entity {
    let e = flex(
        style
            .p(10.0)
            .r(4.0)
            .size((70.0, 70.0))
            .justify_center()
            .items_center()
            .relative(),
    );

    let e_enter = e;
    let e_move = e;
    let e_leave = e;

    e.on_mouse_enter(move || {
        e_enter.set_contents(create_tooltip(label, LayoutPoint::ZERO));
    })
    .on_cursor_moved(move |pos| {
        e_move.set_contents(create_tooltip(label, pos));
    })
    .on_mouse_leave(move || {
        e_leave.set_contents(div_n());
    })
}

fn item_event(label: &'static str, style: Modifier) -> Entity {
    flex(
        style
            .p(10.0)
            .r(4.0)
            .size((70.0, 70.0))
            .justify_center()
            .items_center()
            .relative(),
    )
    .on_mouse_enter_with(move |cx| {
        cx.current()
            .set_contents(create_tooltip(label, LayoutPoint::ZERO));
    })
    .on_cursor_moved_with(move |cx, pos| {
        cx.current().set_contents(create_tooltip(label, pos));
    })
    .on_mouse_leave_with(move |cx| {
        cx.current().set_contents(div_n());
    })
}

fn item_tag(label: &'static str, style: Modifier) -> Entity {
    struct SuperMichiu;
    struct HyperMichiu;
    struct UltraMichiu;

    flex(
        style
            .p(10.0)
            .r(4.0)
            .size((70.0, 70.0))
            .justify_center()
            .items_center()
            .relative(),
    )
    .tag::<SuperMichiu>()
    .tag::<HyperMichiu>()
    .tag::<UltraMichiu>()
    .on_mouse_enter_with(move |cx| {
        if let Some(e) = cx.try_query_first::<SuperMichiu>() {
            e.set_contents(create_tooltip(label, LayoutPoint::ZERO));
        }
    })
    .on_cursor_moved_with(move |cx, pos| {
        let e = cx.query_first::<HyperMichiu>();
        e.set_contents(create_tooltip(label, pos));
    })
    .on_mouse_leave_with(move |cx| {
        let e = cx.query_first::<UltraMichiu>();
        e.set_contents(div_n());
    })
}

fn create_tooltip(label: &'static str, pos: LayoutPoint) -> Entity {
    let offset_x = pos.x + 12.0;
    let offset_y = pos.y + 12.0;

    flex(
        ii().absolute()
            .z_1()
            .left(offset_x)
            .top(offset_y)
            .p((6.0, 10.0))
            .r(4.0)
            .bg_color(rgba(20, 20, 20, 0.95))
            .pointer_events_none(),
    )
    .label(
        label,
        ii().text_color(rgba(210, 210, 210, 0.95))
            .font_size(16.0)
            .font_weight(400),
    )
}
