use crate::app::theme::Theme;
use michiu::{
    prelude::*,
    ui::{
        FontFamily, FontStyle, FontWeight, IntoModFamily, IntoModSpacing, IntoModWeight,
        LetterSpacing, LineHeight,
    },
};

pub fn container() -> Entity {
    v_flex(ii().p(12.0).gap(12.0).r(3.0).w_full()).children([
        size_section(),
        style_section(),
        family_section(),
        weight_section(),
        line_height_section(),
        spacing_section(),
        font_file_section(),
    ])
}

fn section<I>(s: &'static str, e: I) -> Entity
where
    I: IntoIterator<Item = Entity>,
{
    v_flex(ii().gap(6.0).w_full().h(90.0)).children([
        text(s).modify(
            ii().font_size(18.0)
                .text_color(dynamic(|t: &Theme| t.accent)),
        ),
        h_flex(
            ii().gap(18.0)
                .items_end()
                .size_full()
                .r(3.0)
                .border_color(dynamic(|t: &Theme| t.border))
                .border_solid(1.0)
                .overflow_hidden(),
        )
        .children(e),
    ])
}

fn base_m() -> Modifier {
    ii().border_color(dynamic(|t: &Theme| t.border))
        .border_dashed(1.0)
        .text_color(dynamic(|t: &Theme| t.text))
}

fn size_section() -> Entity {
    let item = |s: &'static str, size: f32| text(s).modify(base_m().font_size(size));
    section(
        "Size",
        [
            item("12.0", 12.0),
            item("16.0", 16.0), // Default
            item("20.0", 20.0),
            item("24.0", 25.0),
            item("28.0", 28.0),
            item("32.0", 32.0),
        ],
    )
}

fn style_section() -> Entity {
    let item = |s: &'static str, style: FontStyle| {
        text(s).modify(base_m().font_style(style).font_size(28.0))
    };
    section(
        "Style",
        [
            item("Normal", FontStyle::Normal), // Default
            item("Italic", FontStyle::Italic),
            item("Oblique", FontStyle::Oblique),
        ],
    )
}

fn item_family(s: &'static str, family: impl IntoModFamily) -> Entity {
    text(s).modify(base_m().font_family(family).font_size(24.0))
}

fn family_section() -> Entity {
    section(
        "Family",
        [
            item_family("Segoe UI", "Segoe UI"),
            item_family("Arial", FontFamily::name("Arial")), // -> FontFamily::Name(Cow<'static, str>)
            item_family("SansSerif", FontFamily::SansSerif), // Default
            item_family("Serif", FontFamily::Serif),
            item_family("Cursive", FontFamily::Cursive),
            item_family("Monospace", FontFamily::Monospace),
            item_family("Fantasy", FontFamily::Fantasy),
        ],
    )
}

fn item_weight(s: &'static str, w: impl IntoModWeight) -> Entity {
    text(s).modify(base_m().font_weight(w).font_size(24.0))
}

fn weight_section() -> Entity {
    section(
        "Weight",
        [
            item_weight("THIN", FontWeight::THIN),
            item_weight("LIGHT", FontWeight::LIGHT),
            item_weight("400", 400),                   // i32
            item_weight("NORMAL", FontWeight::NORMAL), // Default (400)
            item_weight("MEDIUM", FontWeight::MEDIUM),
            item_weight("SEMIBOLD", FontWeight::SEMIBOLD),
            item_weight("BOLD", FontWeight::BOLD),
            item_weight("BLACK", FontWeight::BLACK),
        ],
    )
}

fn line_height_section() -> Entity {
    let item =
        |s: &'static str, h: LineHeight| text(s).modify(base_m().line_height(h).font_size(24.0));
    section(
        "Line Height",
        [
            item("Factor(1.4)", LineHeight::Factor(1.4)), // Default
            item("Factor(1.6)", LineHeight::Factor(1.6)),
            item("Factor(1.8)", LineHeight::Factor(1.8)),
            item("Fixed(44.0)", LineHeight::Fixed(44.0)),
            item("Fixed(48.0)", LineHeight::Fixed(48.0)),
        ],
    )
}

fn item_spacing(s: &'static str, sp: impl IntoModSpacing) -> Entity {
    text(s).modify(base_m().letter_spacing(sp).font_size(21.0))
}

fn spacing_section() -> Entity {
    section(
        "Letter Spacing",
        [
            item_spacing("Spacing(0.0)", 0.0), // Default
            item_spacing("Spacing(0.5)", 0.5),
            item_spacing("Spacing(1.0)", LetterSpacing(1.0)),
        ],
    )
}

fn font_file_section() -> Entity {
    // https://fonts.google.com/icons?icon.size=24&icon.color=%23e3e3e3
    // https://github.com/google/material-design-icons/blob/master/variablefont/MaterialSymbolsOutlined[FILL%2CGRAD%2Copsz%2Cwght].ttf

    let path = concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/sample_collection/assets/MaterialSymbolsOutlined.ttf"
    );

    let family = "Material Symbols Outlined";

    section(
        "Family",
        [
            item_family("search", family),
            item_family("home", family),
            item_family("settings", family),
            item_family("check_box", family),
            item_family("favorite", family),
            item_family("star", family),
        ],
    )
    .load_font_file(path)
}
