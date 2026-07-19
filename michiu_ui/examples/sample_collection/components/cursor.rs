use crate::app::{
    theme::Theme,
    utils::{label_style, section_title},
};
use michiu_ui::GlobalCursorIcon;
pub use michiu_ui::prelude::*;

pub fn container() -> Element {
    v_flex(ts().gap(20.0).p(16.0)).children([
        cursor_container(),
        resize_container(),
        global_container(),
        custom_container(),
    ])
}

fn section_style() -> ThisStyle {
    ts().gap(12.0)
}

fn wrapper_style() -> ThisStyle {
    ts().w_full().gap((20.0, 10.0)).p(6.0)
}

fn item(label: &'static str, style: ThisStyle) -> Element {
    div(style
        .p(10.0)
        .r(4.0)
        .size((150.0, 60.0))
        .justify_center()
        .items_center()
        .border_dashed(2.0)
        .border_color(dynamic(|t: &Theme| t.border)))
    .label(label, label_style())
}

fn cursor_container() -> Element {
    let default = item("Default", ts().cursor_default());
    let grab = item("Grab", ts().cursor_grab());
    let grabbing = item("Grabbing", ts().cursor_grabbing());
    let not_allowed = item("Not Allowed", ts().cursor_not_allowed());
    let pointer = item("Pointer", ts().cursor_pointer());

    v_flex(section_style()).children([
        section_title("Cursor"),
        h_flex(wrapper_style()).children([default, grab, grabbing, not_allowed, pointer]),
    ])
}

fn resize_container() -> Element {
    let item = item(
        "Resizable",
        ts().min_size((150.0, 60.0))
            .resizable_all(true)
            .resizable_cursor_default(),
    );

    v_flex(section_style()).children([
        section_title("Resize Cursor"),
        h_flex(wrapper_style()).child(item),
    ])
}

fn global_container() -> Element {
    let item1 = item("Not Style", ts());
    let item2 = item("Local Text", ts().cursor_text());

    let wrapper = h_flex(
        ts().w_full()
            .gap((20.0, 10.0))
            .p(8.0)
            .r(4.0)
            .border_color(dynamic(|t: &Theme| t.border))
            .border_solid(1.0)
            .cursor_global_pointer(),
    );

    v_flex(section_style()).children([
        section_title("Global Cursor"),
        wrapper.children([item1, item2]),
    ])
}

fn custom_container() -> Element {
    let grab_path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("examples/sample_collection/assets/cursor-grab.png");
    let grabbing_path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("examples/sample_collection/assets/cursor-grabbing.png");

    let grab_icon = CursorIcon::create_from_path(grab_path, 16, 16).ok();
    let grabbing_icon = CursorIcon::create_from_path(grabbing_path, 16, 16).ok();

    let wrapper = h_flex(
        ts().w_full()
            .gap((20.0, 10.0))
            .p(8.0)
            .r(4.0)
            .border_color(dynamic(|t: &Theme| t.border))
            .border_solid(1.0)
            .cursor_global(GlobalCursorIcon::Grab(grab_icon)),
    );

    let grabbing = item("Grabbing", ts().cursor(CursorIcon::Grabbing(grabbing_icon)));

    v_flex(section_style()).children([
        section_title("Custom Cursor"),
        wrapper.children([
            div(ts().p(10.0).r(4.0).justify_center().items_center()).label("Grab", label_style()),
            grabbing,
        ]),
    ])
}
