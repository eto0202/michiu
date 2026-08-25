use std::time::Duration;

use crate::{app::theme::Theme, components::section_title};
use michiu_ui::VirtualKey;
pub use michiu_ui::prelude::*;

pub fn container() -> Element {
    v_flex(ts().gap(16.0).p(16.0)).children([
        basic_container(),
        restrict_container(),
        btn_container(),
        multiline_container(),
    ])
}

fn section_style() -> ThisStyle {
    ts().gap(12.0)
}

fn wrapper_style() -> ThisStyle {
    ts().w_full().gap((16.0, 10.0)).p(6.0)
}

fn input_item(d: &'static str, el: Element) -> Element {
    let desc = div_n().label(
        d,
        ts().text_color(dynamic(|t: &Theme| t.text))
            .font_size(14.0)
            .font_weight(400),
    );

    let wrapper = div(ts()
        .flex()
        .items_center()
        .p((6.0, 4.0))
        .r(4.0)
        .size((160.0, 40.0))
        .bg_color(dynamic(|t: &Theme| t.background_hover))
        .border_solid(1.0)
        .border_color(dynamic(|t: &Theme| t.border)));

    v_flex(ts().gap(6.0).justify_center().items_center()).children([desc, wrapper.child(el)])
}

fn input_style(family: &'static str) -> ThisStyle {
    ts().h_auto()
        .w_full()
        .font_size(16.0)
        .font_family(family)
        .text_color(dynamic(|t: &Theme| t.text))
        .select_text()
        .cursor_text()
        .overflow_hidden()
}

fn basic_container() -> Element {
    let (read_1, write_1) = create_signal(String::new());
    let basic_1 = input_item(
        "Font Family : Segoe UI",
        input(InputContents::new((read_1, write_1))).style(input_style("Segoe UI")),
    );

    let (read_2, write_2) = create_signal(String::new());
    let basic_2 = input_item(
        "Font Family : Arial",
        input(
            InputContents::new((read_2, write_2))
                .caret_color(Color::BLUE)
                .caret_height(16.0)
                .caret_width(20.0)
                .caret_offset(1.0)
                .is_blink(true)
                .blink_frequency(Duration::from_millis(500)),
        )
        .style(input_style("Arial")),
    );

    let (read_3, write_3) = create_signal(String::new());
    let basic_3 = input_item(
        "Font Family : Consolas",
        input(
            InputContents::new((read_3, write_3))
                .placeholder("placeholder")
                .placeholder_color(Color::BLACK),
        )
        .style(input_style("Consolas")),
    );

    let (read_4, write_4) = create_signal("Yu Gothic UI".to_string());
    let basic_4 = input_item(
        "Font Family : Yu Gothic UI",
        input(InputContents::new((read_4, write_4))).style(input_style("Yu Gothic UI")),
    );

    v_flex(section_style()).children([
        section_title("Input Basic"),
        h_flex(wrapper_style()).children([basic_1, basic_2, basic_3, basic_4]),
    ])
}

fn restrict_container() -> Element {
    let (read_1, write_1) = create_signal(String::new());
    let basic_1 = input_item(
        "Numeric only",
        input(InputContents::new((read_1, write_1)).numeric_only(true))
            .style(input_style("Segoe UI")),
    );

    let (read_2, write_2) = create_signal(String::new());
    let basic_2 = input_item(
        "Password",
        input(
            InputContents::new((read_2, write_2))
                .password(true)
                .mask_text("*"),
        )
        .style(input_style("Segoe UI")),
    );

    let (read_3, write_3) = create_signal(String::new());
    let basic_3 = input_item(
        "Max length",
        input(InputContents::new((read_3, write_3)).max_length(10)).style(input_style("Segoe UI")),
    );

    let (read_4, write_4) = create_signal(String::new());
    let basic_4 = input_item(
        "No IME",
        input(InputContents::new((read_4, write_4)).is_ime(false)).style(input_style("Segoe UI")),
    );

    v_flex(section_style()).children([
        section_title("Input Restrict"),
        h_flex(wrapper_style()).children([basic_1, basic_2, basic_3, basic_4]),
    ])
}

fn btn_container() -> Element {
    v_flex(section_style()).children([
        section_title("Input with button"),
        h_flex(wrapper_style()).children([spin_box(), password_box(), search_box()]),
    ])
}

fn spin_box() -> Element {
    let btn = ts()
        .justify_center()
        .items_center()
        .size(40.0)
        .hovered(ts().bg_color(dynamic(|t: &Theme| t.border)));
    let label = ts()
        .text_color(dynamic(|t: &Theme| t.secondary))
        .font_size(25.0)
        .font_weight(400)
        .pressed_parent(ts().transform_scale(0.9, 0.9));

    let (read_1, write_1) = create_signal(String::new());

    let btn_left = div(btn.clone().r_left(3.0))
        .label("-", &label)
        .on_click(move || {
            let mut i = read_1.get().parse::<i32>().unwrap_or(0);
            if i <= -99 {
                i = -99;
            } else {
                i -= 1;
            }
            write_1.set(i.to_string());
        });

    let btn_right = div(btn.clone().r_right(3.0))
        .label("+", &label)
        .on_click(move || {
            let mut i = read_1.get().parse::<i32>().unwrap_or(0);
            if i >= 99 {
                i = 99;
            } else {
                i += 1;
            }
            write_1.set(i.to_string());
        });

    let input_center = h_flex(
        ts().items_center()
            .justify_center()
            .p((6.0, 4.0))
            .size((80.0, 40.0))
            .border_solid((0.0, 1.0))
            .border_color(dynamic(|t: &Theme| t.border))
            .focused_visible_within(
                ts().outline_solid(1.0)
                    .outline_color(dynamic(|t: &Theme| t.text)),
            ),
    )
    .child(
        input_d(move |t: &Theme| {
            InputContents::new((read_1, write_1))
                .is_ime(false)
                .numeric_only(true)
                .max_length(3)
                .placeholder("-99~99")
                .placeholder_color(t.text_muted)
        })
        .style(
            ts().flex()
                .text_center()
                .size_full()
                .font_size(16.0)
                .text_color(dynamic(|t: &Theme| t.text))
                .select_text()
                .cursor_text()
                .overflow_hidden(),
        ),
    );

    h_flex(
        ts().r(4.0)
            .bg_color(dynamic(|t: &Theme| t.background_hover))
            .border_solid(1.0)
            .border_color(dynamic(|t: &Theme| t.border)),
    )
    .children([btn_left, input_center, btn_right])
}

fn password_box() -> Element {
    let (read_text, write_text) = create_signal(String::new());
    let (is_mask, set_is_mask) = create_signal(true);

    let suffix_element = text("👁")
        .style_d(move |t: &Theme| {
            let base_style = ts()
                .text_color(t.text_muted)
                .font_size(12.0)
                .p_r(6.0)
                .h_full()
                .hovered(ts().text_color(t.text));

            if read_text.get().is_empty() {
                base_style.hidden()
            } else {
                base_style.block()
            }
        })
        .on_click(move || {
            set_is_mask.set(!is_mask.get());
        });

    h_flex_d(|t: &Theme| {
        ts().r(4.0)
            .bg_color(t.background_hover)
            .border_solid(1.0)
            .border_color(t.border)
    })
    .child(
        h_flex(
            ts().items_center()
                .justify_center()
                .gap(2.0)
                .p((6.0, 4.0))
                .size((160.0, 40.0)),
        )
        .children([
            input_d(move |t: &Theme| {
                InputContents::new((read_text, write_text))
                    .is_ime(false)
                    .password(is_mask.get())
                    .max_length(8)
                    .placeholder("Password")
                    .placeholder_color(t.text_muted)
            })
            .style(
                ts().flex()
                    .text_center()
                    .size_full()
                    .font_size(16.0)
                    .text_color(dynamic(|t: &Theme| t.text))
                    .select_text()
                    .cursor_text()
                    .overflow_hidden(),
            ),
            suffix_element,
        ]),
    )
}

#[allow(clippy::too_many_lines)]
fn search_box() -> Element {
    let (read_text, write_text) = create_signal(String::new());
    let (menu_open, set_menu_open) = create_signal(false);
    let (read_history, write_history) = create_signal::<Vec<String>>(vec![]);

    let wrote_history = move || {
        let input_val = read_text.get();
        if !input_val.trim().is_empty() {
            let mut current = read_history.get();
            if let Some(pos) = current.iter().position(|x| x == &input_val) {
                current.remove(pos);
            }
            current.insert(0, input_val);
            write_history.set(current);
        }
    };

    let input_left = h_flex(
        ts().items_center()
            .justify_center()
            .p((4.0, 6.0, 4.0, 10.0))
            .size((200.0, 40.0))
            .outline_solid((0.0, 1.0, 0.0, 0.0))
            .outline_align(BorderAlignment::Center)
            .outline_lengths(0.6)
            .outline_color(dynamic(|t: &Theme| t.border)),
    )
    .child(
        input_d(move |t: &Theme| {
            InputContents::new((read_text, write_text))
                .placeholder("Search")
                .placeholder_color(t.text_muted)
        })
        .style(
            ts().flex()
                .size_full()
                .font_size(16.0)
                .text_color(dynamic(|t: &Theme| t.text))
                .select_text()
                .cursor_text()
                .overflow_hidden(),
        )
        .on_char_input(move |_| {
            if !menu_open.get() && !read_history.get().is_empty() {
                set_menu_open.set(true);
            }
            if read_text.get().is_empty() && read_history.get().is_empty() {
                set_menu_open.set(false);
            }
        })
        .on_keyboard_input(move |key, _mods, state| {
            if key == VirtualKey::RETURN && state == ElementState::Pressed {
                wrote_history();
            }
        })
        .on_focus(move || {
            if !read_history.get().is_empty() {
                set_menu_open.set(true);
            }
        })
        .on_blur(move || set_menu_open.set(false)),
    );

    let btn_right = h_flex(
        ts().justify_center()
            .items_center()
            .r_right(3.0)
            .size(40.0)
            .prevent_focus_steal(true)
            .hovered(ts().bg_color(dynamic(|t: &Theme| t.border))),
    )
    .label(
        "🔎",
        ts().text_color(dynamic(|t: &Theme| t.text_muted))
            .font_size(12.0)
            .font_weight(200),
    )
    .on_click(wrote_history);

    let candidate = v_flex_d(move |t: &Theme| {
        let base = ts()
            .absolute()
            .r_bottom(4.0)
            .z_1()
            .w(242.0)
            .top(41.0)
            .left(-1.0)
            .border_solid((0.0, 1.0, 1.0, 1.0))
            .border_color(t.primary)
            .bg_color(t.background_hover);

        if menu_open.get() {
            base.block()
        } else {
            base.hidden()
        }
    })
    .child(move || {
        let mut list = Vec::new();
        let history = read_history.get();
        let len = history.len();

        for (idx, h) in history.iter().enumerate() {
            let (hovered, set_hovered) = create_signal(false);
            let (steal_focus, set_steal_focus) = create_signal(true);

            let h_val = h.clone();
            let h_to_delete = h.clone();
            let is_last = idx == len - 1;

            let mut item_style = ts()
                .w_full()
                .hovered_within(ts().bg_color(dynamic(|t: &Theme| t.border_hover)));
            if is_last {
                item_style = item_style.r_bottom(4.0);
            }

            let item = h_flex(item_style).children([
                text(h.clone())
                    .style_d(move |t: &Theme| {
                        let mut s = ts()
                            .w_full()
                            .p_y(6.0)
                            .p_l(10.0)
                            .font_size(14.0)
                            .text_color(t.text)
                            .prevent_focus_steal(true)
                            .overflow_hidden();
                        if is_last {
                            s = s.r((0.0, 0.0, 0.0, 4.0));
                        }
                        s
                    })
                    .on_mouse_enter(move || set_hovered.set(true))
                    .on_mouse_leave(move || set_hovered.set(false))
                    .on_click(move || write_text.set(h_val.clone())),
                text("✕")
                    .style_d(move |t: &Theme| {
                        let mut base_style = ts()
                            .h_full()
                            .w(40.0)
                            .p((6.0, 14.0))
                            .text_center()
                            .text_color(t.text_muted)
                            .font_size(12.0)
                            .prevent_focus_steal(steal_focus.get())
                            .hovered(ts().text_color(t.primary));

                        if is_last {
                            base_style = base_style.r((0.0, 0.0, 4.0, 0.0));
                        }

                        if hovered.get() {
                            base_style.opacity_100()
                        } else {
                            base_style.opacity_0()
                        }
                    })
                    .on_mouse_enter(move || {
                        set_hovered.set(true);

                        if len == 1 {
                            set_steal_focus.set(false);
                        }
                    })
                    .on_mouse_leave(move || set_hovered.set(false))
                    .on_click(move || {
                        if hovered.get() {
                            let h_del = h_to_delete.clone();
                            let mut current = read_history.get();
                            if let Some(pos) = current.iter().position(|x| x == &h_del) {
                                current.remove(pos);
                                write_history.set(current);
                            }
                        }
                    }),
            ]);

            list.push(item);
        }
        v_flex(ts().w_full()).children(list)
    });

    h_flex_d(move |t: &Theme| {
        let base = ts()
            .r(4.0)
            .bg_color(t.background_hover)
            .border_solid(1.0)
            .border_color(t.border)
            .focused_within(ts().border_color(t.primary));

        if menu_open.get() {
            base.r_bottom(0.0).border_bottom(BorderStyle::Solid, 0.0)
        } else {
            base
        }
    })
    .children([input_left, btn_right, candidate])
}

fn multiline_container() -> Element {
    let (read, write) = create_signal(String::new());
    let area = div(ts()
        .flex()
        .p((6.0, 6.0))
        .r(4.0)
        .size((400.0, 100.0))
        .min_size((200.0, 40.0))
        .resizable_right(true)
        .resizable_bottom(true)
        .bg_color(dynamic(|t: &Theme| t.background_hover))
        .border_solid(1.0)
        .border_color(dynamic(|t: &Theme| t.border)))
    .child(
        input_area(InputContents::new((read, write))).style(
            ts().size_full()
                .font_size(16.0)
                .text_auto_wrap(true)
                .text_color(dynamic(|t: &Theme| t.text))
                .select_text()
                .cursor_text()
                .overflow_hidden(),
        ),
    );

    v_flex(section_style()).children([
        section_title("Multiline"),
        h_flex(wrapper_style()).children([area]),
    ])
}
