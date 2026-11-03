use crate::app::theme::Theme;
use michiu_ui::{FontFamily, IntoModFamily, prelude::*};

pub fn container() -> Entity {
    v_flex(ii().p(12.0).gap(12.0).r(3.0).w_full()).children([
        size_section(),
        style_section(),
        family_section(),
        weight_section(),
        line_height_section(),
        font_file_section(),
        icon_font_section(),
    ])
}

fn section<I>(s: &'static str, e: I) -> Entity
where
    I: IntoIterator<Item = Entity>,
{
    v_flex(ii().gap(6.0).w_full().h(90.0)).children([
        text(s).modify(ii().font_size(18.0).text_color(dynamic(|t: &Theme| t.text))),
        h_flex(
            ii().gap(18.0)
                .items_end()
                .size_full()
                .r(3.0)
                .border_color(dynamic(|t: &Theme| t.border))
                .border_dashed(1.0),
        )
        .children(e),
    ])
}

fn size_section() -> Entity {
    let contents = |s: &'static str, size: f32| {
        text(s).modify(
            ii().font_size(size)
                .border_color(dynamic(|t: &Theme| t.border))
                .border_dashed(1.0)
                .text_color(dynamic(|t: &Theme| t.text)),
        )
    };
    section(
        "Size",
        [
            contents("16.0 (Default)", 16.0),
            contents("20.0", 20.0),
            contents("24.0", 25.0),
            contents("28.0", 28.0),
            contents("32.0", 32.0),
        ],
    )
}

fn style_section() -> Entity {
    section("Style", [div_n()])
}

fn item(s: &'static str, f: impl IntoModFamily) -> Entity {
    text(s).modify(
        ii().font_size(24.0)
            .border_color(dynamic(|t: &Theme| t.border))
            .border_dashed(1.0)
            .text_color(dynamic(|t: &Theme| t.text))
            .font_family(f),
    )
}

fn family_section() -> Entity {
    section(
        "Family",
        [
            item("Segoe UI", "Segoe UI"),
            item("Arial", FontFamily::name("Arial")), // -> FontFamily::Name(Cow<'static, str>)
            item("SansSerif", FontFamily::SansSerif),
            item("Serif", FontFamily::Serif),
            item("Cursive", FontFamily::Cursive),
            item("Monospace", FontFamily::Monospace),
            item("Fantasy", FontFamily::Fantasy),
        ],
    )
}

fn weight_section() -> Entity {
    section("Weight", [div_n()])
}

fn line_height_section() -> Entity {
    section("Line Height", [div_n()])
}

fn font_file_section() -> Entity {
    section("Font File", [div_n()])
}

fn icon_font_section() -> Entity {
    section("Icon Font", [div_n()])
}
