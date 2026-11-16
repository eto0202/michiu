use crate::app::theme::Theme;
use iu_ui::*;
use michiu::prelude::*;

pub(crate) fn container(theme: &SimpleBlueTheme) -> Entity {
    wrapper([
        section(
            "Header",theme,
            [
                SimpleText::new("Header1 123 見出し").header1().build(theme),
                SimpleText::new("Header2 123 見出し").header2().build(theme),
                SimpleText::new("Header3 123 見出し").header3().build(theme),
                SimpleText::new("Header4 123 見出し").header4().build(theme),
                SimpleText::new("Header5 123 見出し").header5().build(theme),
            ],
        ),
        section(
            "Body and Caption",theme,
            [
                SimpleText::new("Large 123 ラベル")
                    .body_large()
                    .build(theme),
                SimpleText::new("Body 123 ラベル").body().build(theme),
                SimpleText::new("Caption 123 ラベル").caption().build(theme),
            ],
        ),
        section(
            "Bold",theme,
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
            ],
        ),
        section(
            "OnSolid",theme,
            [
                SimpleText::new("Variant: OnSolid").solid().header1().build(theme),
                SimpleText::new("Variant: OnSolid").solid().header2().build(theme),
                SimpleText::new("Variant: OnSolid").solid().header3().build(theme),
                SimpleText::new("Variant: OnSolid").solid().header4().build(theme),
                SimpleText::new("Variant: OnSolid").solid().header5().build(theme),
                SimpleText::new("Variant: OnSolid").solid()
                    .body_large()
                    .build(theme),
                SimpleText::new("Variant: OnSolid").solid().body().build(theme),
                SimpleText::new("Variant: OnSolid").solid().caption().build(theme),
            ],
        ),
        section(
            "OnBackground",theme,
            [
                SimpleText::new("Variant: OnBackground").background().header1().build(theme),
                SimpleText::new("Variant: OnBackground").background().header2().build(theme),
                SimpleText::new("Variant: OnBackground").background().header3().build(theme),
                SimpleText::new("Variant: OnBackground").background().header4().build(theme),
                SimpleText::new("Variant: OnBackground").background().header5().build(theme),
                SimpleText::new("Variant: OnBackground").background()
                    .body_large()
                    .build(theme),
                SimpleText::new("Variant: OnBackground").background().body().build(theme),
                SimpleText::new("Variant: OnBackground").background().caption().build(theme),
            ],
        ),
        section(
            "Accent",theme,
            [
                SimpleText::new("Variant: Accent").accent().header1().build(theme),
                SimpleText::new("Variant: Accent").accent().header2().build(theme),
                SimpleText::new("Variant: Accent").accent().header3().build(theme),
                SimpleText::new("Variant: Accent").accent().header4().build(theme),
                SimpleText::new("Variant: Accent").accent().header5().build(theme),
                SimpleText::new("Variant: Accent").accent()
                    .body_large()
                    .build(theme),
                SimpleText::new("Variant: Accent").accent().body().build(theme),
                SimpleText::new("Variant: Accent").accent().caption().build(theme),
            ],
        ),
        section(
            "Gray",theme,
            [
                SimpleText::new("Variant: Gray").gray().header1().build(theme),
                SimpleText::new("Variant: Gray").gray().header2().build(theme),
                SimpleText::new("Variant: Gray").gray().header3().build(theme),
                SimpleText::new("Variant: Gray").gray().header4().build(theme),
                SimpleText::new("Variant: Gray").gray().header5().build(theme),
                SimpleText::new("Variant: Gray").gray()
                    .body_large()
                    .build(theme),
                SimpleText::new("Variant: Gray").gray().body().build(theme),
                SimpleText::new("Variant: Gray").gray().caption().build(theme),
            ],
        ),
        section(
            "Destructive",theme,
            [
                SimpleText::new("Intent: Destructive").destructive().header1().build(theme),
                SimpleText::new("Intent: Destructive").destructive().header2().build(theme),
                SimpleText::new("Intent: Destructive").destructive().header3().build(theme),
                SimpleText::new("Intent: Destructive").destructive().header4().build(theme),
                SimpleText::new("Intent: Destructive").destructive().header5().build(theme),
                SimpleText::new("Intent: Destructive").destructive()
                    .body_large()
                    .build(theme),
                SimpleText::new("Intent: Destructive").destructive().body().build(theme),
                SimpleText::new("Intent: Destructive").destructive().caption().build(theme),
            ],
        ),
        section(
            "Success",theme,
            [
                SimpleText::new("Intent: Success").success().header1().build(theme),
                SimpleText::new("Intent: Success").success().header2().build(theme),
                SimpleText::new("Intent: Success").success().header3().build(theme),
                SimpleText::new("Intent: Success").success().header4().build(theme),
                SimpleText::new("Intent: Success").success().header5().build(theme),
                SimpleText::new("Intent: Success").success()
                    .body_large()
                    .build(theme),
                SimpleText::new("Intent: Success").success().body().build(theme),
                SimpleText::new("Intent: Success").success().caption().build(theme),
            ],
        ),
        section(
            "Warning",theme,
            [
                SimpleText::new("Intent: Warning").warning().header1().build(theme),
                SimpleText::new("Intent: Warning").warning().header2().build(theme),
                SimpleText::new("Intent: Warning").warning().header3().build(theme),
                SimpleText::new("Intent: Warning").warning().header4().build(theme),
                SimpleText::new("Intent: Warning").warning().header5().build(theme),
                SimpleText::new("Intent: Warning").warning()
                    .body_large()
                    .build(theme),
                SimpleText::new("Intent: Warning").warning().body().build(theme),
                SimpleText::new("Intent: Warning").warning().caption().build(theme),
            ],
        ),
        section(
            "Disable",theme,
            [
                SimpleText::new("Actived").actived().header3().build(theme),
                SimpleText::new("Disabled")
                    .disabled()
                    .header3()
                    .build(theme),
            ],
        ),
        section(
            "RTL (Right to Left)",theme,
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
            "Selectable",theme,
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
            "Auto Wrap",theme,
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
            "Align",theme,
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
                SimpleText::new("☆ البداية")
                    .align_start()
                    .rtl()
                    .header3()
                    .build(theme),
                SimpleText::new("☆ االوسط")
                    .align_center()
                    .rtl()
                    .header3()
                    .build(theme),
                SimpleText::new("☆ االنهاية")
                    .align_end()
                    .rtl()
                    .header3()
                    .build(theme),
            ],
        ),
        section(
            "Monospace",theme,
            [
                SimpleText::new("SanSerif 1234567890").sans_serif().header3().build(theme),
                SimpleText::new("Monospace 1234567890")
                    .monospace()
                    .header3()
                    .build(theme),
            ],
        ),
        section(
            "Overflow",theme,
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
            "Description",theme,
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

fn section<I>(s: &'static str, theme: &SimpleBlueTheme, e: I) -> Entity
where
    I: IntoIterator<Item = Entity>,
{
    v_flex(ii().gap(20.0)).children([
        SimpleText::new(s).header3().background().build(theme),
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
