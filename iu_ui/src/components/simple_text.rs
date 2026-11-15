use super::core::{MichiuTheme, MichiuWidget, Oklch};
use michiu::{
    accessibility::*,
    prelude::*,
    ui::{Direction, FontDate, FontFamily, FontWeight, UserSelect},
};
use std::borrow::Cow;

// コントラスト比。背景色が何かで調整。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum TextVariant {
    OnSolid, // 基本白
    Primary,
    Secondary,
    #[default]
    OnBackground,
}

// カラーリング。Optionで包む。
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum TextIntent {
    Destructive, // 赤
    Success,     // 緑
    Warning,     // 黄色
}

// ベース値を基準とした倍率で目標値近くになるよう計算。最大値最小値は前後のサイズとしてクランプ。行間と余白は自動調整。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum TextSize {
    #[default]
    Body, // 14px
    BodyLarge, // 18px
    Caption,   // 12px
    Header1,   // 36px
    Header2,   // 26px
    Header3,   // 22px
    // 以降はコントラスト比と太さを強制する。
    Header4, // 18px,
    Header5, // 16px,
}

#[derive(Debug, Clone, PartialEq)]
pub struct SimpleText {
    /// Display text.
    pub text: Cow<'static, str>,
    /// Visual style variant.
    pub variant: TextVariant,
    /// Set the color palette to be applied to the text.
    pub intent: Option<TextIntent>,
    pub default_size: f32,
    /// Text dimensions.
    pub size: TextSize,
    /// Text weight.
    pub weight: FontWeight,
    /// Whether the text is disable.
    pub disable: bool,
    pub wrap: bool,
    pub overflow: Overflow,
    pub selectable: bool,
    pub align: TextAlign,
    pub decorative: bool,
    pub tabular_nums: bool, // SanSerifとMonospaceのみ
    /// Whether to use right-to-left (RTL) layout direction.
    pub rtl: bool,
    /// Supplementary information and detailed descriptions intended for assistive technologies such as screen readers.
    pub description: Option<Cow<'static, str>>,
}
impl SimpleText {
    #[inline]
    pub fn new(text: impl Into<Cow<'static, str>>) -> Self {
        Self {
            text: text.into(),
            variant: TextVariant::default(),
            intent: None,
            default_size: FontDate::FONT_SIZE,
            size: TextSize::default(),
            weight: FontWeight::default(),
            disable: false,
            wrap: false,
            overflow: Overflow::default(),
            selectable: true,
            align: TextAlign::default(),
            decorative: false,
            tabular_nums: false,
            rtl: false,
            description: None,
        }
    }

    #[inline]
    pub fn text(mut self, t: impl Into<Cow<'static, str>>) -> Self {
        self.text = t.into();
        self
    }

    #[inline]
    pub fn desc(mut self, desc: impl Into<Cow<'static, str>>) -> Self {
        self.description = Some(desc.into());
        self
    }

    #[inline]
    pub fn variant(mut self, variant: TextVariant) -> Self {
        self.variant = variant;
        self
    }
    #[inline]
    pub fn solid(self) -> Self {
        self.variant(TextVariant::OnSolid)
    }
    #[inline]
    pub fn accent(self) -> Self {
        self.variant(TextVariant::Primary)
    }
    #[inline]
    pub fn gray(self) -> Self {
        self.variant(TextVariant::Secondary)
    }
    #[inline]
    pub fn background(self) -> Self {
        self.variant(TextVariant::OnBackground)
    }

    #[inline]
    pub fn intent(mut self, intent: TextIntent) -> Self {
        self.intent = Some(intent);
        self
    }
    #[inline]
    pub fn destructive(self) -> Self {
        self.intent(TextIntent::Destructive)
    }
    #[inline]
    pub fn success(self) -> Self {
        self.intent(TextIntent::Success)
    }
    #[inline]
    pub fn warning(self) -> Self {
        self.intent(TextIntent::Warning)
    }

    #[inline]
    pub fn default_size(mut self, s: f32) -> Self {
        self.default_size = s;
        self
    }

    #[inline]
    pub fn size(mut self, s: TextSize) -> Self {
        self.size = s;
        self
    }
    #[inline]
    pub fn body(self) -> Self {
        self.size(TextSize::Body)
    }
    #[inline]
    pub fn body_large(self) -> Self {
        self.size(TextSize::BodyLarge)
    }
    #[inline]
    pub fn caption(self) -> Self {
        self.size(TextSize::Caption)
    }
    #[inline]
    pub fn header1(self) -> Self {
        self.size(TextSize::Header1)
    }
    #[inline]
    pub fn header2(self) -> Self {
        self.size(TextSize::Header2)
    }
    #[inline]
    pub fn header3(self) -> Self {
        self.size(TextSize::Header3)
    }
    #[inline]
    pub fn header4(self) -> Self {
        self.size(TextSize::Header4)
    }
    #[inline]
    pub fn header5(self) -> Self {
        self.size(TextSize::Header5)
    }

    #[inline]
    pub fn weight(mut self, w: FontWeight) -> Self {
        self.weight = w;
        self
    }
    #[inline]
    pub fn normal(self) -> Self {
        self.weight(FontWeight::NORMAL)
    }
    #[inline]
    pub fn light(self) -> Self {
        self.weight(FontWeight::LIGHT)
    }
    #[inline]
    pub fn semibold(self) -> Self {
        self.weight(FontWeight::SEMIBOLD)
    }
    #[inline]
    pub fn bold(self) -> Self {
        self.weight(FontWeight::BOLD)
    }

    #[inline]
    pub fn disable(mut self, enabled: bool) -> Self {
        self.disable = enabled;
        self
    }
    #[inline]
    pub fn disabled(self) -> Self {
        self.disable(true)
    }
    #[inline]
    pub fn actived(self) -> Self {
        self.disable(false)
    }

    #[inline]
    pub fn wrap(mut self, enabled: bool) -> Self {
        self.wrap = enabled;
        self
    }
    #[inline]
    pub fn wrap_allow(self) -> Self {
        self.wrap(true)
    }
    #[inline]
    pub fn wrap_deny(self) -> Self {
        self.wrap(false)
    }

    #[inline]
    pub fn overflow(mut self, flow: Overflow) -> Self {
        self.overflow = flow;
        self
    }
    #[inline]
    pub fn overflow_visible(self) -> Self {
        self.overflow(Overflow::Visible)
    }
    #[inline]
    pub fn overflow_hidden(self) -> Self {
        self.overflow(Overflow::Hidden)
    }
    #[inline]
    pub fn overflow_scroll(self) -> Self {
        self.overflow(Overflow::Scroll)
    }

    #[inline]
    pub fn with_selectable(mut self, enabled: bool) -> Self {
        self.selectable = enabled;
        self
    }
    #[inline]
    pub fn selectable(self) -> Self {
        self.with_selectable(true)
    }
    #[inline]
    pub fn unselectable(self) -> Self {
        self.with_selectable(false)
    }

    #[inline]
    pub fn align(mut self, a: TextAlign) -> Self {
        self.align = a;
        self
    }
    #[inline]
    pub fn align_start(self) -> Self {
        self.align(TextAlign::Left)
    }
    #[inline]
    pub fn align_end(self) -> Self {
        self.align(TextAlign::Right)
    }
    #[inline]
    pub fn align_center(self) -> Self {
        self.align(TextAlign::Center)
    }

    #[inline]
    pub fn decorative(mut self, enabled: bool) -> Self {
        self.decorative = enabled;
        self
    }
    #[inline]
    pub fn decoratived(self) -> Self {
        self.decorative(true)
    }
    #[inline]
    pub fn undecoratived(self) -> Self {
        self.decorative(false)
    }

    #[inline]
    pub fn tabular_nums(mut self, enabled: bool) -> Self {
        self.tabular_nums = enabled;
        self
    }
    #[inline]
    pub fn san_serif(self) -> Self {
        self.tabular_nums(false)
    }
    #[inline]
    pub fn monospace(self) -> Self {
        self.tabular_nums(true)
    }

    #[inline]
    pub fn with_rtl(mut self, enabled: bool) -> Self {
        self.rtl = enabled;
        self
    }
    #[inline]
    pub fn rtl(self) -> Self {
        self.with_rtl(true)
    }
    #[inline]
    pub fn ltr(self) -> Self {
        self.with_rtl(false)
    }

    #[inline]
    fn calc_size(&self, scale: f32, min: f32, max: f32, line: f32) -> (f32, f32) {
        let font = (self.default_size * scale).clamp(min, max).round();
        (font, line)
    }

    #[inline]
    fn metrics(&self) -> (f32, f32) {
        match self.size {
            TextSize::Caption => self.calc_size(0.75, 12.0, 14.0, 1.50), // 16px時: (12, 18, 3)
            TextSize::Body => self.calc_size(0.875, 12.0, 16.0, 1.50),   // 16px時: (14, 21, 6)
            TextSize::Header5 => self.calc_size(1.00, 14.0, 18.0, 1.40), // 16px時: (16, 22, 8)
            TextSize::BodyLarge => self.calc_size(1.125, 14.0, 22.0, 1.50), // 16px時: (18, 27, 8)
            TextSize::Header4 => self.calc_size(1.125, 16.0, 22.0, 1.40), // 16px時: (18, 25, 10)
            TextSize::Header3 => self.calc_size(1.375, 18.0, 26.0, 1.35), // 16px時: (22, 30, 12)
            TextSize::Header2 => self.calc_size(1.625, 22.0, 36.0, 1.30), // 16px時: (26, 34, 16)
            TextSize::Header1 => self.calc_size(2.25, 26.0, 48.0, 1.25), // 16px時: (36, 45, 24)
        }
    }
}

impl MichiuWidget for SimpleText {
    fn name(&self) -> &'static str {
        "Simple Text"
    }

    fn a11y_spec(&self) -> &'static str {
        "WAI-ARIA Text Pattern / WCAG 2.2 AA (後で)"
    }

    fn build(&self, theme: &dyn MichiuTheme) -> Entity {
        let (font_size, line) = self.metrics();

        let text_color = if let Some(intent) = self.intent {
            match intent {
                TextIntent::Destructive => theme.destructive(11),
                TextIntent::Success => theme.success(11),
                TextIntent::Warning => theme.warning(11),
            }
        } else {
            match self.variant {
                TextVariant::OnSolid => theme.text_on_solid(),
                TextVariant::Primary => theme.accent_11(),
                TextVariant::Secondary => theme.gray_11(),
                TextVariant::OnBackground => theme.text_high(),
            }
        };

        let node = if self.decorative {
            let mut n = Node::new(Role::GenericContainer);
            n.set_hidden();
            n
        } else {
            let level = match self.size {
                TextSize::Header1 => Some(1),
                TextSize::Header2 => Some(2),
                TextSize::Header3 => Some(3),
                TextSize::Header4 => Some(4),
                TextSize::Header5 => Some(5),
                _ => None,
            };

            let mut n = if let Some(lvl) = level {
                let mut h = Node::new(Role::Heading);
                h.set_level(lvl);
                h.set_label(self.text.to_string());
                h
            } else {
                let mut l = Node::new(Role::Label);
                l.set_value(self.text.to_string());
                l
            };

            if let Some(desc) = &self.description {
                n.set_description(desc.to_string());
            }
            n
        };

        let selectable = if self.selectable {
            UserSelect::Text
        } else {
            UserSelect::None
        };

        let direction = if self.rtl {
            Direction::Rtl
        } else {
            Direction::Ltr
        };

        let family = if self.tabular_nums {
            FontFamily::Monospace
        } else {
            FontFamily::SansSerif
        };

        let mut m = ii()
            .flex_row()
            .font_size(font_size)
            .font_weight(self.weight)
            .font_family(family)
            .text_color(text_color)
            .line_height(LineHeight::Factor(line))
            .user_select(selectable)
            .text_align(self.align)
            .direction(direction)
            .auto_wrap(self.wrap)
            .disabled(ii().text_color(theme.gray_8()));

        if self.align == TextAlign::Auto {
            m = m.w_fit();
        } else {
            m = m.w_auto();
        }

        match self.overflow {
            Overflow::Visible => m = m.overflow_auto(),
            Overflow::Hidden => m = m.w_fill().overflow_hidden(),
            Overflow::Scroll => m = m.w_fill().overflow_scroll(),
            Overflow::Clip => m = m.w_fill().overflow_clip(),
        }

        if self.selectable {
            m = m.cursor_text()
        }

        text(self.text.clone())
            .a11y(node)
            .disable(self.disable)
            .modify(m)
    }
}
