use crate::app::theme::Theme;
use iu_ui::*;
use michiu::{accessibility::*, prelude::*};

pub(crate) fn container(theme: &SimpleBlueTheme) -> Entity {
    wrapper([
        section(
            "Header",
            [
                SimpleText::new("Header1 123 見出し").header1().build(theme),
                SimpleText::new("Header2 123 見出し").header2().build(theme),
                SimpleText::new("Header3 123 見出し").header3().build(theme),
                SimpleText::new("Header4 123 見出し").header4().build(theme),
                SimpleText::new("Header5 123 見出し").header5().build(theme),
            ],
        ),
        section(
            "Body and Caption",
            [
                SimpleText::new("Large 123 ラベル")
                    .body_large()
                    .build(theme),
                SimpleText::new("Body 123 ラベル").body().build(theme),
                SimpleText::new("Caption 123 ラベル").caption().build(theme),
            ],
        ),
        section(
            "Bold",
            [
                SimpleText::new("Bold (700)").bold().header3().build(theme),
                SimpleText::new("Semibold (600)")
                    .semibold()
                    .header3()
                    .build(theme),
                SimpleText::new("Normal (400)")
                    .normal()
                    .header3()
                    .build(theme),
                SimpleText::new("Light (300)")
                    .light()
                    .header3()
                    .build(theme),
            ],
        ),
        section(
            "Variant",
            [
                SimpleText::new("On Solid").solid().header3().build(theme),
                SimpleText::new("On Background")
                    .background()
                    .header3()
                    .build(theme),
                SimpleText::new("Accent").accent().header3().build(theme),
                SimpleText::new("Gray").gray().header3().build(theme),
            ],
        ),
        section(
            "Intent",
            [
                SimpleText::new("Destructive")
                    .destructive()
                    .header3()
                    .build(theme),
                SimpleText::new("Success").success().header3().build(theme),
                SimpleText::new("Warning").warning().header3().build(theme),
            ],
        ),
        section(
            "Disable",
            [
                SimpleText::new("Actived").actived().header3().build(theme),
                SimpleText::new("Disabled")
                    .disabled()
                    .header3()
                    .build(theme),
            ],
        ),
        section(
            "RTL (Right to Left)",
            [
                SimpleText::new("☆ من اليمين إلى اليسار")
                    .rtl()
                    .header3()
                    .build(theme),
                SimpleText::new("☆ מימין לשמאל")
                    .rtl()
                    .header3()
                    .build(theme),
                SimpleText::new("☆ از راست به چپ")
                    .rtl()
                    .header3()
                    .build(theme),
            ],
        ),
        section(
            "Selectable",
            [
                SimpleText::new("Selectable")
                    .selectable()
                    .header3()
                    .build(theme),
                SimpleText::new("Unselectable")
                    .unselectable()
                    .header3()
                    .build(theme),
            ],
        ),
        section(
            "Auto Wrap",
            [
                SimpleText::new("ABCDEFGHIJKLMNOPQRSTUVWXYZABCDEFGHIJKLMNOPQRSTUVWXYZABCDEFGHIJKLMNOPQRSTUVWXYZ")
                    .wrap_deny()
                    .header3()
                    .build(theme),
                SimpleText::new("absdefghijklmnopqrstuvwxyzabsdefghijklmnopqrstuvwxyzabsdefghijklmnopqrstuvwxyz")
                    .wrap_allow()
                    .header3()
                    .build(theme),
            ],
        ),
        section(
            "Align",
            [
                SimpleText::new("Start")
                    .align_start()
                    .header3()
                    .build(theme),
                SimpleText::new("Center")
                    .align_center()
                    .header3()
                    .build(theme),
                SimpleText::new("End")
                    .align_end()
                    .header3()
                    .build(theme),
            ],
        ),
        section(
            "Tabular nums",
            [
                SimpleText::new("SanSerif 1234567890").san_serif().header3().build(theme),
                SimpleText::new("Monospace 1234567890")
                    .monospace()
                    .header3()
                    .build(theme),
            ],
        ),
        section(
            "Overflow",
            [
                SimpleText::new("VisibleVisibleVisibleVisibleVisibleVisibleVisibleVisible")
                    .overflow_visible()
                    .header3()
                    .build(theme),
                SimpleText::new("CenterCenterCenterCenterCenterCenterCenterCenterCenter")
                    .overflow_hidden()
                    .header3()
                    .build(theme),
            ],
        ),
        section(
            "Description",
            [
                SimpleText::new("Description")
                    .desc("This is Description sample")
                    .header3()
                    .build(theme),
            ],
        ),
    ])
}

fn wrapper<I>(e: I) -> Entity
where
    I: IntoIterator<Item = Entity>,
{
    v_flex(ii().gap(40.0).p(20.0).r(3.0).size_max()).children(e)
}

fn section<I>(s: &'static str, e: I) -> Entity
where
    I: IntoIterator<Item = Entity>,
{
    v_flex(ii().gap(20.0)).children([
        section_header(s),
        v_flex(
            ii().r(3.0)
                .w(500.0)
                .p(20.0)
                .border_solid(1.0)
                .border_color(dynamic(|t: &Theme| t.border)),
        )
        .children(e),
    ])
}

fn section_header(s: &'static str) -> Entity {
    text(s)
        .tag::<a11y::AHeading2>()
        .modify(ii().text_color(dynamic(|t: &Theme| t.text)).font_size(18.0))
}
