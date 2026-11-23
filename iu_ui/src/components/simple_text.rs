use super::core::{MichiuTheme, MichiuWidget, Oklch};
use michiu::{
    accessibility::*,
    prelude::*,
    ui::{Direction, FontDate, FontFamily, FontWeight, UserSelect},
};
use std::borrow::Cow;

/// Text color variants for [`SimpleText`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum TextVariant {
    OnSolid,
    Accent,
    Gray,
    #[default]
    OnBackground,
}

/// Semantic color intent for [SimpleText].
///
/// Takes precedence over [`TextVariant`].
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum TextIntent {
    Destructive,
    Success,
    Warning,
}

/// Font size and line-height (`font_size * multiplier`) variants scaled relative to `default_size` (default: `16px`).
///
/// # Accessibility
/// - `Body`, `BodyLarge`, and `Caption` are exposed as the accessibility node `value`;
///   `Header1`–`Header5` are exposed as the node `label`.
/// - `Caption`, `Header4`, and `Header5` may not meet accessibility guidelines depending on
///   layout or contrast. Consider applying bold weight, higher contrast, or extra padding.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum TextSize {
    /// Standard body text: `14px` at 16px base (min: `12px`, max: `16px`, line: `1.5x`).
    #[default]
    Body,
    /// Emphasized body text: `18px` at 16px base (min: `14px`, max: `22px`, line: `1.5x`).
    BodyLarge,
    /// Secondary/caption text: `12px` at 16px base (min: `12px`, max: `14px`, line: `1.5x`).
    Caption,
    /// Level 1 heading: `36px` at 16px base (min: `26px`, max: `48px`, line: `1.25x`).
    Header1,
    /// Level 2 heading: `26px` at 16px base (min: `22px`, max: `36px`, line: `1.3x`).
    Header2,
    /// Level 3 heading: `22px` at 16px base (min: `18px`, max: `26px`, line: `1.35x`).
    Header3,
    /// Level 4 heading: `18px` at 16px base (min: `16px`, max: `22px`, line: `1.4x`).
    Header4,
    /// Level 5 heading: `16px` at 16px base (min: `14px`, max: `18px`, line: `1.4x`).
    Header5,
}

/// Accessible text and heading widget compliant with WAI-ARIA semantics and WCAG 2.2 AA.
///
/// # Scope
/// - **Stand-alone**: Can be used by copying only this file (or struct and its `impl` blocks),
///   provided `core.rs` (`MichiuWidget`, `MichiuTheme`, `Oklch`) is present in your project.
///
/// # Required Assets
/// - None
///
/// # Event Loop Requirements
/// - None (Self-contained within the widget tree).
///
/// # Feature Details
/// - **Selection & Cursor**:
///   - Text selection is enabled by default (`selectable: true`) with a text cursor (`cursor_text`),
///     allowing users to highlight and copy text.
/// - **Screen Reader**:
///   - Role: `Role::Label` for body/caption variants (`Body`, `BodyLarge`, `Caption`),
///     or `Role::Heading` with `level(1..=5)` for `Header1`–`Header5`.
///   - Accessible Text: Exposed as `value` for `Role::Label`, or `label` for `Role::Heading`.
///   - Description: Inferred from `self.description` (`desc(...)`).
///   - Decorative Mode: Setting `decorative: true` switches to `Role::GenericContainer` with
///     `set_hidden()`, hiding the node from assistive technologies.
/// - **Visual / WCAG**:
///   - **Scaling & Line Height**: Scales font sizes relative to `default_size` (default `16px`)
///     within accessible bounds (`>= 12px`) and applies readable line-height factors (`1.25x`–`1.5x`).
///   - **Weight Enforcement**: Automatically promotes `Header4`–`Header5` to at least
///     `FontWeight::SEMIBOLD`, and `Caption`/`Body` to at least `FontWeight::NORMAL` to maintain legibility.
///   - **Color & Contrast**: Uses high-contrast theme colors (step 11 / `text_high`).
///     `TextIntent` takes precedence over `TextVariant`.
///   - **RTL Support**: Setting `rtl: true` applies `Direction::Rtl` and mirrors
///     `TextAlign::Left` / `TextAlign::Right` automatically.
/// - **Customization**:
///   - To attach event listeners or override layout styles,
///     chain modifiers **directly after calling `.build(&theme)`**.
///   - Any modifiers appended after `build()` will take precedence over defaults.
///
/// # Example
///
/// ```rust
/// // 1. Basic body text
/// let body = SimpleText::new("Hello, world!").build(&theme);
///
/// // 2. Heading with wrapping and style overrides (overwritten after build)
/// let heading = SimpleText::new("Section Title")
///     .header2()
///     .accent()
///     .wrap_allow()
///     .build(&theme)
///     .modify(ii().m_t(16.0));
/// ```
#[derive(Debug, Clone, PartialEq)]
pub struct SimpleText {
    /// Text to display.
    pub text: Cow<'static, str>,
    /// Visual style variant.
    pub variant: TextVariant,
    /// Semantic color palette applied to the text.
    pub intent: Option<TextIntent>,
    /// Base font size used to calculate [`TextSize`].
    pub default_size: f32,
    /// Text size variant.
    pub size: TextSize,
    /// Font weight.
    pub weight: FontWeight,
    /// Whether the text is disabled.
    pub disable: bool,
    /// Whether the text wraps automatically.
    pub wrap: bool,
    /// How text overflow is handled when exceeding the container bounds.
    pub overflow: Overflow,
    /// Whether the text can be selected.
    pub selectable: bool,
    /// Text alignment.
    pub align: TextAlign,
    /// Whether the text is decorative. If `true`, it is excluded from the accessibility tree.
    pub decorative: bool,
    /// Whether to use a monospace font.
    pub monospace: bool,
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
            monospace: false,
            rtl: false,
            description: None,
        }
    }

    /// Set the display text.
    #[inline]
    pub fn text(mut self, t: impl Into<Cow<'static, str>>) -> Self {
        self.text = t.into();
        self
    }

    /// Configure supplementary information and detailed descriptions provided to assistive technologies such as screen readers.
    #[inline]
    pub fn desc(mut self, desc: impl Into<Cow<'static, str>>) -> Self {
        self.description = Some(desc.into());
        self
    }

    /// Set the Text Color.
    ///
    /// Default: [`TextVariant::OnBackground`]
    #[inline]
    pub fn variant(mut self, variant: TextVariant) -> Self {
        self.variant = variant;
        self
    }
    /// Set the text color to [`TextVariant::OnSolid`] (`theme.text_on_solid()`).
    #[inline]
    pub fn solid(self) -> Self {
        self.variant(TextVariant::OnSolid)
    }
    /// Set the text color to [`TextVariant::Accent`] (`theme.accent_11()`).
    #[inline]
    pub fn accent(self) -> Self {
        self.variant(TextVariant::Accent)
    }
    /// Set the text color to [`TextVariant::Gray`] (`theme.gray_11()`).
    #[inline]
    pub fn gray(self) -> Self {
        self.variant(TextVariant::Gray)
    }
    /// Set the text color to [`TextVariant::OnBackground`] (`theme.text_high()`).
    #[inline]
    pub fn background(self) -> Self {
        self.variant(TextVariant::OnBackground)
    }

    /// Set the Text Intent.
    #[inline]
    pub fn intent(mut self, intent: TextIntent) -> Self {
        self.intent = Some(intent);
        self
    }
    /// Set intent [`TextIntent::Destructive`].
    #[inline]
    pub fn destructive(self) -> Self {
        self.intent(TextIntent::Destructive)
    }
    /// Set intent [`TextIntent::Success`].
    #[inline]
    pub fn success(self) -> Self {
        self.intent(TextIntent::Success)
    }
    /// Set intent [`TextIntent::Warning`].
    #[inline]
    pub fn warning(self) -> Self {
        self.intent(TextIntent::Warning)
    }

    /// Set base font size.
    ///
    /// Default: `16px`
    #[inline]
    pub fn default_size(mut self, s: f32) -> Self {
        self.default_size = s;
        self
    }

    /// Set text size.
    ///
    /// Default: [`TextSize::Body`]
    #[inline]
    pub fn size(mut self, s: TextSize) -> Self {
        self.size = s;
        self
    }
    /// Set text size [`TextSize::Body`].
    ///
    /// Standard body text: `14px` at 16px base (min: `12px`, max: `16px`, line: `1.5x`).
    #[inline]
    pub fn body(self) -> Self {
        self.size(TextSize::Body)
    }
    /// Set text size [`TextSize::BodyLarge`].
    ///
    /// Emphasized body text: `18px` at 16px base (min: `14px`, max: `22px`, line: `1.5x`).
    #[inline]
    pub fn body_large(self) -> Self {
        self.size(TextSize::BodyLarge)
    }
    /// Set text size [`TextSize::Caption`].
    ///
    /// Secondary/caption text: `12px` at 16px base (min: `12px`, max: `14px`, line: `1.5x`).
    #[inline]
    pub fn caption(self) -> Self {
        self.size(TextSize::Caption)
    }
    /// Set text size [`TextSize::Header1`].
    ///
    /// Level 1 heading: `36px` at 16px base (min: `26px`, max: `48px`, line: `1.25x`).
    #[inline]
    pub fn header1(self) -> Self {
        self.size(TextSize::Header1)
    }
    /// Set text size [`TextSize::Header2`].
    ///
    /// Level 2 heading: `26px` at 16px base (min: `22px`, max: `36px`, line: `1.3x`).
    #[inline]
    pub fn header2(self) -> Self {
        self.size(TextSize::Header2)
    }
    /// Set text size [`TextSize::Header3`].
    ///
    /// Level 3 heading: `22px` at 16px base (min: `18px`, max: `26px`, line: `1.35x`).
    #[inline]
    pub fn header3(self) -> Self {
        self.size(TextSize::Header3)
    }
    /// Set text size [`TextSize::Header4`].
    ///
    /// Level 4 heading: `18px` at 16px base (min: `16px`, max: `22px`, line: `1.4x`).
    #[inline]
    pub fn header4(self) -> Self {
        self.size(TextSize::Header4)
    }
    /// Set text size [`TextSize::Header5`].
    ///
    /// Level 5 heading: `16px` at 16px base (min: `14px`, max: `18px`, line: `1.4x`).
    #[inline]
    pub fn header5(self) -> Self {
        self.size(TextSize::Header5)
    }

    /// Set font weight.
    ///
    /// Default: [`FontWeight::NORMAL`]
    #[inline]
    pub fn weight(mut self, w: FontWeight) -> Self {
        self.weight = w;
        self
    }
    /// Set font weight [`FontWeight::NORMAL`]
    #[inline]
    pub fn normal(self) -> Self {
        self.weight(FontWeight::NORMAL)
    }
    /// Set font weight [`FontWeight::SEMIBOLD`]
    #[inline]
    pub fn semibold(self) -> Self {
        self.weight(FontWeight::SEMIBOLD)
    }
    /// Set font weight [`FontWeight::BOLD`]
    #[inline]
    pub fn bold(self) -> Self {
        self.weight(FontWeight::BOLD)
    }

    /// Sets whether the text is disabled.
    ///
    /// All Chroma values will be set to 0.0, and the Lightness of the text color will be adjusted.
    ///
    /// Default: `false`
    #[inline]
    pub fn disable(mut self, enabled: bool) -> Self {
        self.disable = enabled;
        self
    }
    /// Set disabled `true`
    #[inline]
    pub fn disabled(self) -> Self {
        self.disable(true)
    }
    /// Set disabled `false`
    #[inline]
    pub fn actived(self) -> Self {
        self.disable(false)
    }

    /// Set whether the text wraps automatically.
    ///
    /// Default: `false`
    #[inline]
    pub fn wrap(mut self, enabled: bool) -> Self {
        self.wrap = enabled;
        self
    }
    /// Set wrap `true`.
    #[inline]
    pub fn wrap_allow(self) -> Self {
        self.wrap(true)
    }
    /// Set wrap `false`.
    #[inline]
    pub fn wrap_deny(self) -> Self {
        self.wrap(false)
    }

    /// Configure how to handle text that extends beyond the container's boundaries.
    ///
    /// Default: [`Overflow::Visible`]
    #[inline]
    pub fn overflow(mut self, flow: Overflow) -> Self {
        self.overflow = flow;
        self
    }
    /// Set overflow [`Overflow::Visible`].
    ///
    /// Text is displayed even if it extends beyond the container's boundaries.
    #[inline]
    pub fn overflow_visible(self) -> Self {
        self.overflow(Overflow::Visible)
    }
    /// Set overflow [`Overflow::Hidden`].
    ///
    /// If the text extends beyond the container's boundaries, the portion that extends beyond is hidden.
    #[inline]
    pub fn overflow_hidden(self) -> Self {
        self.overflow(Overflow::Hidden)
    }
    /// Set overflow [`Overflow::Scroll`].
    ///
    /// If scroll bars are enabled, they will appear when the text extends beyond the container's boundaries.
    #[inline]
    pub fn overflow_scroll(self) -> Self {
        self.overflow(Overflow::Scroll)
    }
    /// Set overflow [`Overflow::Clip`].
    ///
    /// If the text extends beyond the container's boundaries, the portion that extends beyond is clip.
    #[inline]
    pub fn overflow_clip(self) -> Self {
        self.overflow(Overflow::Clip)
    }

    /// Set whether text can be selected.
    ///
    /// Unless there is a specific reason not to, text should be selectable.
    #[inline]
    pub fn with_selectable(mut self, enabled: bool) -> Self {
        self.selectable = enabled;
        self
    }
    /// Set selectable `true`
    #[inline]
    pub fn selectable(self) -> Self {
        self.with_selectable(true)
    }
    /// Set selectable `false`
    #[inline]
    pub fn unselectable(self) -> Self {
        self.with_selectable(false)
    }

    /// Set the text alignment.
    ///
    /// In the case of RTL, [`TextAlign::Left`] and [`TextAlign::Right`] are automatically reversed.
    #[inline]
    pub fn align(mut self, a: TextAlign) -> Self {
        self.align = a;
        self
    }
    /// Set alignment [`TextAlign::Left`].
    ///
    /// For RTL, [`TextAlign::Right`] is set.
    #[inline]
    pub fn align_start(self) -> Self {
        self.align(TextAlign::Left)
    }
    /// Set alignment [`TextAlign::Right`].
    ///
    /// For RTL, [`TextAlign::Left`] is set.
    #[inline]
    pub fn align_end(self) -> Self {
        self.align(TextAlign::Right)
    }
    /// Set alignment [`TextAlign::Center`].
    #[inline]
    pub fn align_center(self) -> Self {
        self.align(TextAlign::Center)
    }

    /// Set whether the text is for decorative purposes.
    ///
    /// If it is for decorative purposes, it will be ignored by assistive technologies such as screen readers.
    ///
    /// Default: `false`
    #[inline]
    pub fn decorative(mut self, enabled: bool) -> Self {
        self.decorative = enabled;
        self
    }
    /// The text is intended for decorative purposes.
    #[inline]
    pub fn decoratived(self) -> Self {
        self.decorative(true)
    }
    /// The text is not intended for decorative purposes.
    #[inline]
    pub fn undecoratived(self) -> Self {
        self.decorative(false)
    }

    /// Set whether to display text in a monospaced font.
    ///
    /// Default: `false`
    #[inline]
    pub fn with_monospace(mut self, enabled: bool) -> Self {
        self.monospace = enabled;
        self
    }
    /// Set font family [`FontFamily::SansSerif`].
    #[inline]
    pub fn sans_serif(self) -> Self {
        self.with_monospace(false)
    }
    /// Set font family [`FontFamily::Monospace`].
    #[inline]
    pub fn monospace(self) -> Self {
        self.with_monospace(true)
    }

    /// Set whether to use right-to-left (RTL) layout direction.
    ///
    /// Default: `false`
    #[inline]
    pub fn with_rtl(mut self, enabled: bool) -> Self {
        self.rtl = enabled;
        self
    }
    /// Set right-to-left (RTL).
    #[inline]
    pub fn rtl(self) -> Self {
        self.with_rtl(true)
    }
    /// Set left-to-right (LTR).
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
            TextSize::Caption => self.calc_size(0.75, 12.0, 14.0, 1.50),
            TextSize::Body => self.calc_size(0.875, 12.0, 16.0, 1.50),
            TextSize::Header5 => self.calc_size(1.00, 14.0, 18.0, 1.40),
            TextSize::BodyLarge => self.calc_size(1.125, 14.0, 22.0, 1.50),
            TextSize::Header4 => self.calc_size(1.125, 16.0, 22.0, 1.40),
            TextSize::Header3 => self.calc_size(1.375, 18.0, 26.0, 1.35),
            TextSize::Header2 => self.calc_size(1.625, 22.0, 36.0, 1.30),
            TextSize::Header1 => self.calc_size(2.25, 26.0, 48.0, 1.25),
        }
    }
}

impl MichiuWidget for SimpleText {
    fn name(&self) -> &'static str {
        "Simple Text"
    }

    fn a11y_spec(&self) -> &'static str {
        "WAI-ARIA Heading & Static Text / WCAG 2.2 AA (Min Size >= 12px, Line Height >= 1.25x)"
    }

    /// Builds and returns the underlying accessible UI entity.
    ///
    /// Additional styling or event handlers (such as `.on_click(...)`) can be chained
    /// onto the returned `Entity` to overwrite defaults.
    fn build(&self, theme: impl MichiuTheme) -> Entity {
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
                TextVariant::Accent => theme.accent_11(),
                TextVariant::Gray => theme.gray_11(),
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

        let family = if self.monospace {
            FontFamily::Monospace
        } else {
            FontFamily::SansSerif
        };

        let mut weight = self.weight;
        match self.size {
            TextSize::Header4 | TextSize::Header5 if self.weight < FontWeight::SEMIBOLD => {
                weight = FontWeight::SEMIBOLD;
            }
            TextSize::Caption | TextSize::Body if self.weight < FontWeight::NORMAL => {
                weight = FontWeight::NORMAL
            }
            _ => {}
        };

        let align = if self.rtl {
            match self.align {
                TextAlign::Auto => TextAlign::Auto,
                TextAlign::Left => TextAlign::Right,
                TextAlign::Right => TextAlign::Left,
                TextAlign::Center => TextAlign::Center,
            }
        } else {
            self.align
        };

        let mut m = ii()
            .flex_row()
            .font_size(font_size)
            .font_weight(weight)
            .font_family(family)
            .text_color(text_color)
            .line_height(LineHeight::Factor(line))
            .user_select(selectable)
            .text_align(align)
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
