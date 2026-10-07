use crate::app::theme::Theme;
pub use michiu::prelude::*;
use michiu::ui::{AlignContent, AlignItems, AlignSelf, FlexDirection, FlexWrap, JustifyContent};

#[derive(Clone, Copy)]
struct Gap(i32);
impl Default for Gap {
    fn default() -> Self {
        Gap(8)
    }
}

pub fn container() -> Element {
    let (gap, _) = create_signal(Gap::default());
    let (flex_direction, _) = create_signal(FlexDirection::default());
    let (flex_wrap, _) = create_signal(FlexWrap::default());
    let (align_self, _) = create_signal(AlignSelf::default());
    let (align_items, _) = create_signal(AlignItems::default());
    let (align_content, _) = create_signal(AlignContent::default());
    let (justify_content, _) = create_signal(JustifyContent::default());
    v_flex(ts().gap(12.0).p(12.0).max_width(1000.0).size_full())
        .provide(gap)
        .provide(flex_direction)
        .provide(flex_wrap)
        .provide(align_self)
        .provide(align_items)
        .provide(align_content)
        .provide(justify_content)
        .children([main_aria(), setting_aria()])
}

fn common_inner() -> [Element; 9] {
    let inner = |label: &'static str| {
        flex(
            ts().border_solid(1.0)
                .r(4.0)
                .size((80.0, 40.0))
                .shrink_0()
                .justify_center()
                .border_color(dynamic(|t: &Theme| t.border)),
        )
        .label(
            label,
            ts().text_color(dynamic(|t: &Theme| t.text)).font_size(14.0),
        )
    };

    [
        inner("1"),
        inner("2"),
        inner("3"),
        inner("4"),
        flex(
            ts().border_solid(1.0)
                .r(4.0)
                .min_size((80.0, 40.0))
                .justify_center()
                .align_self(dynamic(|f: &AlignSelf| (*f).into()))
                .border_color(dynamic(|t: &Theme| t.border)),
        )
        .label(
            "AlignSelf 5",
            ts().text_color(dynamic(|t: &Theme| t.text)).font_size(14.0),
        ),
        inner("6"),
        inner("7"),
        inner("8"),
        inner("9"),
    ]
}

fn main_aria() -> Element {
    v_flex(ts().gap(8.0).shrink().min_height(0.0).overflow_hidden())
        .label(
            "Results",
            ts().text_color(dynamic(|t: &Theme| t.text)).font_size(16.0),
        )
        .child(|| {
            flex(
                ts().gap(use_provided::<Gap>().get().0)
                    .flex_direction(use_provided::<FlexDirection>().get())
                    .flex_wrap_internal(use_provided::<FlexWrap>().get())
                    .align_self(use_provided::<AlignSelf>().get())
                    .align_items(use_provided::<AlignItems>().get())
                    .align_content(use_provided::<AlignContent>().get())
                    .justify_content(use_provided::<JustifyContent>().get()),
            )
            .style_d(|t: &Theme| {
                ts().p(8.0)
                    .r(4.0)
                    .min_size((400.0, 200.0))
                    .max_size((950.0, 400.0))
                    .resizable_right(true)
                    .resizable_bottom(true)
                    .border_dashed(1.0)
                    .border_color(t.border)
                    .overflow_y_hidden()
                    .scrollbar_always()
                    .scrollbar_mode(ScrollbarMode::Overlay)
                    .scrollbar(
                        ScrollbarStyle::new(10.0).v_thumb(
                            ts().m_y(4.0)
                                .p_r(5.0)
                                .width(7.0)
                                .r_full()
                                .bg_color(t.border)
                                .hovered(ts().bg_color(t.border_hover)),
                        ),
                    )
            })
            .children(common_inner())
        })
}

fn setting_aria() -> Element {
    v_flex(ts().gap(8.0).m_t(auto()).shrink_0())
        .label(
            "Setting",
            ts().text_color(dynamic(|t: &Theme| t.text)).font_size(16.0),
        )
        .child(table())
}

fn table() -> Element {
    let (gap, set_gap) = create_signal(String::from("8"));

    let label = ts().font_size(12.0).text_color(dynamic(|t: &Theme| t.text));

    let row_1 = ts()
        .w_full()
        .border_right(BorderStyle::Dashed, 1.0)
        .border_color(dynamic(|t: &Theme| t.border));

    let row_2 = ts().w_full();

    let col_1 = ts()
        .p_x(4.0)
        .h(40.0)
        .justify_start()
        .selected(ts().bg_color(dynamic(|t: &Theme| t.primary)));
    let col_2 = col_1
        .clone()
        .bg_color(dynamic(|t: &Theme| t.background_hover));

    h_flex_d(|t: &Theme| {
        ts().r(4.0)
            .justify_evenly()
            .border_dashed(1.0)
            .border_color(t.border)
            .overflow_x_scroll()
            .scrollbar_always()
            .scrollbar_mode(ScrollbarMode::Overlay)
            .scrollbar(
                ScrollbarStyle::new(10.0).h_thumb(
                    ts().m_x(4.0)
                        .p_b(5.0)
                        .h(7.0)
                        .r_full()
                        .bg_color(t.border)
                        .hovered(ts().bg_color(t.border_hover)),
                ),
            )
    })
    .children([
        v_flex(&row_1).children({
            let col_1 = ts().p_x(4.0).h(40.0).justify_start();
            let col_2 = col_1
                .clone()
                .bg_color(dynamic(|t: &Theme| t.background_hover));
            [
                flex(&col_1),
                flex(&col_2).label("Gap", &label),
                flex(&col_1).label("FlexDirection", &label),
                flex(&col_2).label("FlexWrap", &label),
                flex(&col_1).label("AlignSelf", &label),
                flex(&col_2).label("AlignItems", &label),
                flex(&col_1).label("AlignContent", &label),
                flex(&col_2).label("JustifyContent", &label),
            ]
        }),
        v_flex(&row_1).children({
            let default = ts()
                .p_x(4.0)
                .h(40.0)
                .justify_start()
                .hovered(ts().bg_color(dynamic(|t: &Theme| t.border)));
            [
                flex(default).label("Default", &label).on_click(move || {
                    use_provided_setter::<Gap>().set(Gap::default());
                    set_gap.set("8".to_string());
                    use_provided_setter::<FlexDirection>().set(FlexDirection::default());
                    use_provided_setter::<FlexWrap>().set(FlexWrap::default());
                    use_provided_setter::<AlignSelf>().set(AlignSelf::default());
                    use_provided_setter::<AlignItems>().set(AlignItems::default());
                    use_provided_setter::<AlignContent>().set(AlignContent::default());
                    use_provided_setter::<JustifyContent>().set(JustifyContent::default());
                }),
                flex(&col_2)
                    .input(move || {
                        InputContents::new((gap, set_gap))
                            .numeric_only(true)
                            .is_ime(false)
                            .placeholder("0 ~ 30")
                    })
                    .style(col_2.clone().merge(&label))
                    .on_char_input(move |_| {
                        let mut i = gap.get().parse::<i32>().unwrap_or(0);
                        i = i.clamp(0, 30);
                        let gap = if i <= 0 { String::new() } else { i.to_string() };
                        set_gap.set(gap);
                        use_provided_setter().set(Gap(i));
                    }),
                flex(&col_1)
                    .label("Row", &label)
                    .select(|| use_provided::<FlexDirection>().get() == FlexDirection::Row)
                    .on_click(|| {
                        use_provided_setter::<FlexDirection>().set(FlexDirection::Row);
                    }),
                flex(&col_2)
                    .label("NoWrap", &label)
                    .select(|| use_provided::<FlexWrap>().get() == FlexWrap::NoWrap)
                    .on_click(|| {
                        use_provided_setter::<FlexWrap>().set(FlexWrap::NoWrap);
                    }),
                flex(&col_1)
                    .label("Stretch", &label)
                    .select(|| use_provided::<AlignSelf>().get() == AlignSelf::Stretch)
                    .on_click(|| {
                        use_provided_setter::<AlignSelf>().set(AlignSelf::Stretch);
                    }),
                flex(&col_2)
                    .label("Stretch", &label)
                    .select(|| use_provided::<AlignItems>().get() == AlignItems::Stretch)
                    .on_click(|| {
                        use_provided_setter::<AlignItems>().set(AlignItems::Stretch);
                    }),
                flex(&col_1)
                    .label("Stretch", &label)
                    .select(|| use_provided::<AlignContent>().get() == AlignContent::Stretch)
                    .on_click(|| {
                        use_provided_setter::<AlignContent>().set(AlignContent::Stretch);
                    }),
                flex(&col_2)
                    .label("Stretch", &label)
                    .select(|| use_provided::<JustifyContent>().get() == JustifyContent::Stretch)
                    .on_click(|| {
                        use_provided_setter::<JustifyContent>().set(JustifyContent::Stretch);
                    }),
            ]
        }),
        v_flex(&row_1).children({
            [
                flex(&col_1),
                flex(&col_2),
                flex(&col_1)
                    .label("Column", &label)
                    .select(|| use_provided::<FlexDirection>().get() == FlexDirection::Column)
                    .on_click(|| {
                        use_provided_setter::<FlexDirection>().set(FlexDirection::Column);
                    }),
                flex(&col_2)
                    .label("Wrap", &label)
                    .select(|| use_provided::<FlexWrap>().get() == FlexWrap::Wrap)
                    .on_click(|| {
                        use_provided_setter::<FlexWrap>().set(FlexWrap::Wrap);
                    }),
                flex(&col_1)
                    .label("Start", &label)
                    .select(|| use_provided::<AlignSelf>().get() == AlignSelf::Start)
                    .on_click(|| {
                        use_provided_setter::<AlignSelf>().set(AlignSelf::Start);
                    }),
                flex(&col_2)
                    .label("Start", &label)
                    .select(|| use_provided::<AlignItems>().get() == AlignItems::Start)
                    .on_click(|| {
                        use_provided_setter::<AlignItems>().set(AlignItems::Start);
                    }),
                flex(&col_1)
                    .label("Start", &label)
                    .select(|| use_provided::<AlignContent>().get() == AlignContent::Start)
                    .on_click(|| {
                        use_provided_setter::<AlignContent>().set(AlignContent::Start);
                    }),
                flex(&col_2)
                    .label("Start", &label)
                    .select(|| use_provided::<JustifyContent>().get() == JustifyContent::Start)
                    .on_click(|| {
                        use_provided_setter::<JustifyContent>().set(JustifyContent::Start);
                    }),
            ]
        }),
        v_flex(&row_1).children({
            [
                flex(&col_1),
                flex(&col_2),
                flex(&col_1)
                    .label("RowReverse", &label)
                    .select(|| use_provided::<FlexDirection>().get() == FlexDirection::RowReverse)
                    .on_click(|| {
                        use_provided_setter::<FlexDirection>().set(FlexDirection::RowReverse);
                    }),
                flex(&col_2)
                    .label("WrapReverse", &label)
                    .select(|| use_provided::<FlexWrap>().get() == FlexWrap::WrapReverse)
                    .on_click(|| {
                        use_provided_setter::<FlexWrap>().set(FlexWrap::WrapReverse);
                    }),
                flex(&col_1)
                    .label("End", &label)
                    .select(|| use_provided::<AlignSelf>().get() == AlignSelf::End)
                    .on_click(|| {
                        use_provided_setter::<AlignSelf>().set(AlignSelf::End);
                    }),
                flex(&col_2)
                    .label("End", &label)
                    .select(|| use_provided::<AlignItems>().get() == AlignItems::End)
                    .on_click(|| {
                        use_provided_setter::<AlignItems>().set(AlignItems::End);
                    }),
                flex(&col_1)
                    .label("End", &label)
                    .select(|| use_provided::<AlignContent>().get() == AlignContent::End)
                    .on_click(|| {
                        use_provided_setter::<AlignContent>().set(AlignContent::End);
                    }),
                flex(&col_2)
                    .label("End", &label)
                    .select(|| use_provided::<JustifyContent>().get() == JustifyContent::End)
                    .on_click(|| {
                        use_provided_setter::<JustifyContent>().set(JustifyContent::End);
                    }),
            ]
        }),
        v_flex(&row_1).children({
            [
                flex(&col_1),
                flex(&col_2),
                flex(&col_1)
                    .label("ColumnReverse", &label)
                    .select(|| {
                        use_provided::<FlexDirection>().get() == FlexDirection::ColumnReverse
                    })
                    .on_click(|| {
                        use_provided_setter::<FlexDirection>().set(FlexDirection::ColumnReverse);
                    }),
                flex(&col_2)
                    .label("Balance", &label)
                    .select(|| use_provided::<FlexWrap>().get() == FlexWrap::Balance)
                    .on_click(|| {
                        use_provided_setter::<FlexWrap>().set(FlexWrap::Balance);
                    }),
                flex(&col_1)
                    .label("FlexStart", &label)
                    .select(|| use_provided::<AlignSelf>().get() == AlignSelf::FlexStart)
                    .on_click(|| {
                        use_provided_setter::<AlignSelf>().set(AlignSelf::FlexStart);
                    }),
                flex(&col_2)
                    .label("FlexStart", &label)
                    .select(|| use_provided::<AlignItems>().get() == AlignItems::FlexStart)
                    .on_click(|| {
                        use_provided_setter::<AlignItems>().set(AlignItems::FlexStart);
                    }),
                flex(&col_1)
                    .label("FlexStart", &label)
                    .select(|| use_provided::<AlignContent>().get() == AlignContent::FlexStart)
                    .on_click(|| {
                        use_provided_setter::<AlignContent>().set(AlignContent::FlexStart);
                    }),
                flex(&col_2)
                    .label("FlexStart", &label)
                    .select(|| use_provided::<JustifyContent>().get() == JustifyContent::FlexStart)
                    .on_click(|| {
                        use_provided_setter::<JustifyContent>().set(JustifyContent::FlexStart);
                    }),
            ]
        }),
        v_flex(&row_1).children({
            [
                flex(&col_1),
                flex(&col_2),
                flex(&col_1),
                flex(&col_2)
                    .label("BalanceReverse", &label)
                    .select(|| use_provided::<FlexWrap>().get() == FlexWrap::BalanceReverse)
                    .on_click(|| {
                        use_provided_setter::<FlexWrap>().set(FlexWrap::BalanceReverse);
                    }),
                flex(&col_1)
                    .label("FlexEnd", &label)
                    .select(|| use_provided::<AlignSelf>().get() == AlignSelf::FlexEnd)
                    .on_click(|| {
                        use_provided_setter::<AlignSelf>().set(AlignSelf::FlexEnd);
                    }),
                flex(&col_2)
                    .label("FlexEnd", &label)
                    .select(|| use_provided::<AlignItems>().get() == AlignItems::FlexEnd)
                    .on_click(|| {
                        use_provided_setter::<AlignItems>().set(AlignItems::FlexEnd);
                    }),
                flex(&col_1)
                    .label("FlexEnd", &label)
                    .select(|| use_provided::<AlignContent>().get() == AlignContent::FlexEnd)
                    .on_click(|| {
                        use_provided_setter::<AlignContent>().set(AlignContent::FlexEnd);
                    }),
                flex(&col_2)
                    .label("FlexEnd", &label)
                    .select(|| use_provided::<JustifyContent>().get() == JustifyContent::FlexEnd)
                    .on_click(|| {
                        use_provided_setter::<JustifyContent>().set(JustifyContent::FlexEnd);
                    }),
            ]
        }),
        v_flex(&row_1).children({
            [
                flex(&col_1),
                flex(&col_2),
                flex(&col_1),
                flex(&col_2),
                flex(&col_1)
                    .label("Center", &label)
                    .select(|| use_provided::<AlignSelf>().get() == AlignSelf::Center)
                    .on_click(|| {
                        use_provided_setter::<AlignSelf>().set(AlignSelf::Center);
                    }),
                flex(&col_2)
                    .label("Center", &label)
                    .select(|| use_provided::<AlignItems>().get() == AlignItems::Center)
                    .on_click(|| {
                        use_provided_setter::<AlignItems>().set(AlignItems::Center);
                    }),
                flex(&col_1)
                    .label("Center", &label)
                    .select(|| use_provided::<AlignContent>().get() == AlignContent::Center)
                    .on_click(|| {
                        use_provided_setter::<AlignContent>().set(AlignContent::Center);
                    }),
                flex(&col_2)
                    .label("Center", &label)
                    .select(|| use_provided::<JustifyContent>().get() == JustifyContent::Center)
                    .on_click(|| {
                        use_provided_setter::<JustifyContent>().set(JustifyContent::Center);
                    }),
            ]
        }),
        v_flex(&row_1).children({
            [
                flex(&col_1),
                flex(&col_2),
                flex(&col_1),
                flex(&col_2),
                flex(&col_1)
                    .label("Baseline", &label)
                    .select(|| use_provided::<AlignSelf>().get() == AlignSelf::Baseline)
                    .on_click(|| {
                        use_provided_setter::<AlignSelf>().set(AlignSelf::Baseline);
                    }),
                flex(&col_2)
                    .label("Baseline", &label)
                    .select(|| use_provided::<AlignItems>().get() == AlignItems::Baseline)
                    .on_click(|| {
                        use_provided_setter::<AlignItems>().set(AlignItems::Baseline);
                    }),
                flex(&col_1)
                    .label("Between", &label)
                    .select(|| use_provided::<AlignContent>().get() == AlignContent::SpaceBetween)
                    .on_click(|| {
                        use_provided_setter::<AlignContent>().set(AlignContent::SpaceBetween);
                    }),
                flex(&col_2)
                    .label("Between", &label)
                    .select(|| {
                        use_provided::<JustifyContent>().get() == JustifyContent::SpaceBetween
                    })
                    .on_click(|| {
                        use_provided_setter::<JustifyContent>().set(JustifyContent::SpaceBetween);
                    }),
            ]
        }),
        v_flex(&row_1).children({
            [
                flex(&col_1),
                flex(&col_2),
                flex(&col_1),
                flex(&col_2),
                flex(&col_1)
                    .label("SelfStart", &label)
                    .select(|| use_provided::<AlignSelf>().get() == AlignSelf::SelfStart)
                    .on_click(|| {
                        use_provided_setter::<AlignSelf>().set(AlignSelf::SelfStart);
                    }),
                flex(&col_2)
                    .label("SelfStart", &label)
                    .select(|| use_provided::<AlignItems>().get() == AlignItems::SelfStart)
                    .on_click(|| {
                        use_provided_setter::<AlignItems>().set(AlignItems::SelfStart);
                    }),
                flex(&col_1)
                    .label("Around", &label)
                    .select(|| use_provided::<AlignContent>().get() == AlignContent::SpaceAround)
                    .on_click(|| {
                        use_provided_setter::<AlignContent>().set(AlignContent::SpaceAround);
                    }),
                flex(&col_2)
                    .label("Around", &label)
                    .select(|| {
                        use_provided::<JustifyContent>().get() == JustifyContent::SpaceAround
                    })
                    .on_click(|| {
                        use_provided_setter::<JustifyContent>().set(JustifyContent::SpaceAround);
                    }),
            ]
        }),
        v_flex(&row_2).children({
            [
                flex(&col_1),
                flex(&col_2),
                flex(&col_1),
                flex(&col_2),
                flex(&col_1)
                    .label("SelfEnd", &label)
                    .select(|| use_provided::<AlignSelf>().get() == AlignSelf::SelfEnd)
                    .on_click(|| {
                        use_provided_setter::<AlignSelf>().set(AlignSelf::SelfEnd);
                    }),
                flex(&col_2)
                    .label("SelfEnd", &label)
                    .select(|| use_provided::<AlignItems>().get() == AlignItems::SelfEnd)
                    .on_click(|| {
                        use_provided_setter::<AlignItems>().set(AlignItems::SelfEnd);
                    }),
                flex(&col_1)
                    .label("Evenly", &label)
                    .select(|| use_provided::<AlignContent>().get() == AlignContent::SpaceEvenly)
                    .on_click(|| {
                        use_provided_setter::<AlignContent>().set(AlignContent::SpaceEvenly);
                    }),
                flex(&col_2)
                    .label("Evenly", &label)
                    .select(|| {
                        use_provided::<JustifyContent>().get() == JustifyContent::SpaceEvenly
                    })
                    .on_click(|| {
                        use_provided_setter::<JustifyContent>().set(JustifyContent::SpaceEvenly);
                    }),
            ]
        }),
    ])
}
