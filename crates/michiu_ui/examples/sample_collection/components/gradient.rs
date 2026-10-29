use crate::app::theme::Theme;
use michiu_ui::{LinearGradient, prelude::*};

pub fn container() -> Entity {
    v_flex(ii().p(16.0).gap(16.0)).children([section_1()])
}

fn input_btn(label: Label) -> Entity {
    let default_val = label.default_value();
    let s = create_signal(format!("{:.1}", default_val));

    h_flex(ii().size((130.0, 35.0))).children([
        text(label.name()).modify(
            ii().shrink_0()
                .h_full()
                .w(65.0)
                .p_l(6.0)
                .r_left(3.0)
                .font_size(15.0)
                .bg_color(dynamic(|t: &Theme| t.header))
                .text_color(dynamic(|t: &Theme| t.text)),
        ),
        input(InputContents::new(s).numeric_only(true).is_ime(false))
            .modify(
                ii().size_full()
                    .p_l(6.0)
                    .r_right(3.0)
                    .font_size(15.0)
                    .cursor_text()
                    .select_text()
                    .bg_color(dynamic(|t: &Theme| t.surface))
                    .text_color(dynamic(|t: &Theme| t.text)),
            )
            .on_char_input_with(move |cx, _| {
                let read_grad = cx.use_provided::<Grad>().get();
                let write_grad = cx.use_provided_setter::<Grad>();

                let read_string = s.0.get();
                let read_f32 = read_string.parse::<f32>().unwrap_or(default_val);
                s.1.set(read_string);

                write_grad.set(read_grad.with_updated(label, read_f32));
            }),
    ])
}

fn section_1() -> Entity {
    let (grad, _) = create_signal(Grad::default());

    v_flex(ii().gap(18.0).w_full()).provide(grad).children([
        text("Linear").modify(ii().font_size(18.0).text_color(dynamic(|t: &Theme| t.text))),
        h_flex(ii().gap(12.0)).children([
            input_btn(Label::Start),
            input_btn(Label::Center),
            input_btn(Label::End),
            input_btn(Label::Angle),
        ]),
        div(ii()
            .size((600.0, 200.0))
            .m(12.0)
            .r(3.0)
            .bg_gradient(dynamic(|g: &Grad| g.g))),
    ])
}

#[derive(Debug, Clone)]
struct Grad {
    g: LinearGradient,
    start: f32,
    center: f32,
    end: f32,
    angle: f32,
}

impl Grad {
    fn new(start: f32, center: f32, end: f32, angle: f32) -> Self {
        let g = LinearGradient::new(
            &[
                (Color::RED, start),
                (Color::ORANGE, center),
                (Color::YELLOW, end),
            ],
            angle,
        );
        Self {
            g,
            start,
            center,
            end,
            angle,
        }
    }

    /// 特定のフィールドのみを更新した新しい `Grad` を返す
    fn with_updated(&self, label: Label, val: f32) -> Self {
        let (mut start, mut center, mut end, mut angle) =
            (self.start, self.center, self.end, self.angle);

        match label {
            Label::Start => start = val,
            Label::Center => center = val,
            Label::End => end = val,
            Label::Angle => angle = val,
        }

        Self::new(start, center, end, angle)
    }
}

impl Default for Grad {
    fn default() -> Self {
        Self::new(0.0, 0.5, 1.0, 90.0)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, PartialOrd)]
enum Label {
    Start,
    Center,
    End,
    Angle,
}

impl Label {
    fn name(&self) -> &'static str {
        match self {
            Self::Start => "Start",
            Self::Center => "Center",
            Self::End => "End",
            Self::Angle => "Angle",
        }
    }

    fn default_value(&self) -> f32 {
        match self {
            Self::Start => 0.0,
            Self::Center => 0.5,
            Self::End => 1.0,
            Self::Angle => 90.0,
        }
    }
}
