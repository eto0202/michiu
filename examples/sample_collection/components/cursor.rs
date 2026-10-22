use crate::{
    app::theme::Theme,
    components::{label_style, section_title},
};
pub use michiu::prelude::*;
use michiu::ui::GlobalCursorIcon;

pub fn container() -> Entity {
    v_flex(ii().gap(20.0).p(16.0)).children([
        cursor_container(),
        resize_container(),
        global_container(),
        custom_container(),
    ])
}

fn section_style() -> Modifier {
    ii().gap(12.0)
}

fn wrapper_style() -> Modifier {
    ii().w_full().gap((20.0, 10.0)).p(6.0)
}

fn item(label: &'static str, style: Modifier) -> Entity {
    flex(
        style
            .p(10.0)
            .r(4.0)
            .size((150.0, 60.0))
            .justify_center()
            .items_center()
            .border_dashed(2.0)
            .border_color(dynamic(|t: &Theme| t.border)),
    )
    .label(label, label_style())
}

fn cursor_container() -> Entity {
    let default = item("Default", ii().cursor_default());
    let grab = item("Grab", ii().cursor_grab());
    let grabbing = item("Grabbing", ii().cursor_grabbing());
    let not_allowed = item("Not Allowed", ii().cursor_not_allowed());
    let pointer = item("Pointer", ii().cursor_pointer());

    v_flex(section_style()).children([
        section_title("Cursor"),
        h_flex(wrapper_style()).children([default, grab, grabbing, not_allowed, pointer]),
    ])
}

fn resize_container() -> Entity {
    let item = item(
        "Resizable",
        ii().min_size((150.0, 60.0))
            .resizable_all(true)
            .resizable_cursor_default(),
    );

    v_flex(section_style()).children([
        section_title("Resize Cursor"),
        h_flex(wrapper_style()).child(item),
    ])
}

fn global_container() -> Entity {
    let item1 = item("Not Style", ii());
    let item2 = item("Local Text", ii().cursor_text());

    let wrapper = h_flex(
        ii().w_full()
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

fn custom_container() -> Entity {
    let grab_path = concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/sample_collection/assets/cursor_grab.png"
    );
    let grabbing_path = concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/sample_collection/assets/cursor_grabbing.png"
    );

    let grab_icon = CursorIcon::create_from_path(grab_path, 16, 16).ok();
    let grabbing_icon = CursorIcon::create_from_path(grabbing_path, 16, 16).ok();

    let wrapper = h_flex(
        ii().w_full()
            .gap((20.0, 10.0))
            .p(8.0)
            .r(4.0)
            .border_color(dynamic(|t: &Theme| t.border))
            .border_solid(1.0)
            .cursor_global(GlobalCursorIcon::Grab(grab_icon)),
    );

    let grabbing = item("Grabbing", ii().cursor(CursorIcon::Grabbing(grabbing_icon)));

    v_flex(section_style()).children([
        section_title("Custom Cursor"),
        wrapper.children([
            flex(ii().p(10.0).r(4.0).justify_center().items_center()).label("Grab", label_style()),
            grabbing,
        ]),
    ])
}
