use super::core::{MichiuTheme, MichiuWidget};
use michiu::{accessibility::*, prelude::*};

/// Visual appearance variants for [`SimpleButton`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ButtonVariant {
    /// Fully filled high-contrast background.
    #[default]
    Solid,
    /// Subtle tinted background with no border.
    Soft,
    /// Transparent background with a distinct colored border.
    Outline,
    /// Transparent background that reveals a subtle background only when hovered.
    Ghost,
}

/// Defines the semantic purpose and color palette applied to a button.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ButtonIntent {
    /// Primary brand accent color.
    #[default]
    Primary,
    /// Neutral gray palette.
    Secondary,
    /// Critical danger palette (Red).
    Destructive,
    /// Positive success palette (Green).
    Success,
    /// Cautionary palette (Amber/Yellow).
    Warning,
}

/// Sizing options for [`SimpleButton`].
///
/// All sizes strictly satisfy the WCAG 2.5.8 minimum target size requirement (>= 24x24px).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ButtonSize {
    /// Extra small size
    Xs,
    /// Small size.
    Sm,
    /// Standard size.
    #[default]
    Md,
    /// Large size.
    Lg,
}

/// Accessible button widget compliant with the WAI-ARIA Button Pattern and WCAG 2.2 AA.
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
/// - **Keyboard**:
///   - `Tab` / `Shift + Tab`: Moves focus into and out of the button.
///   - `Space` / `Enter`: Activates the button.
/// - **Screen Reader**:
///   - Role: `Role::Button` (automatically inferred via `tag::<a11y::AButton>()`).
///   - Accessible Name: Inferred from `self.label`.
/// - **Visual / WCAG**:
///   - **Target Size**: Minimum 24x24px across all variants (WCAG 2.5.8 Level AA compliant).
///   - **Focus Indicator**: 2px focus ring displayed only during keyboard navigation (`focused_visible`),
///     maintaining >= 3.0:1 contrast ratio against the background (WCAG 1.4.11, 2.4.7, 2.4.11).
///   - **Offset**: Automatically applies an external 2px offset for `Solid` buttons to guarantee
///     visual separation and contrast against surrounding containers.
/// - **Customization**:
///   - To attach event listeners (e.g., `.on_click(...)`) or override layout styles,
///     chain modifiers **directly after calling `.build(&theme)`**.
///   - Any modifiers appended after `build()` will take precedence over defaults.
///
/// # Example
///
/// ```rust
/// // 1. Basic button
/// let button = SimpleButton::new("Save").build(&theme);
///
/// // 2. Customizing variant, events, and styles (overwritten after build)
/// let delete_button = SimpleButton::new("Delete")
///     .variant(ButtonVariant::Outline)
///     .size(ButtonSize::Md)
///     .build(&theme)
///     .modify(ii().bg_color(Color::WHITE)) // Overwrites default border
///     .on_click(|| println!("Delete clicked!"));
/// ```
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SimpleButton {
    /// Accessible button label text.
    pub label: Option<String>,
    /// An explicit accessible name for assistive technologies like screen readers.
    ///
    /// By default, the visible `label` is used as the accessible name.
    /// Set this field when creating an icon-only button (where there is no visible text)
    /// or when you want to override the visible label with a more descriptive name.
    pub accessible_name: Option<String>,
    /// Visual style variant (Solid, Outline, Ghost).
    pub variant: ButtonVariant,
    /// Set the color palette to be applied to the button.
    pub intent: ButtonIntent,
    /// Button dimensions.
    pub size: ButtonSize,
    /// Whether the button is disabled.
    pub disabled: bool,
    /// Whether to round the corners (`Modifier::r_full`).
    pub rounded: bool,
    /// Optional child entity placed at the start side (left in LTR, right in RTL, e.g., an icon).
    pub start_child: Option<Entity>,
    /// Optional child entity placed at the end side (right in LTR, left in RTL, e.g., a dropdown arrow).
    pub end_child: Option<Entity>,
    /// Whether to use right-to-left (RTL) layout direction.
    pub rtl: bool,
    /// Whether to match the **text color** of child elements to their own background color.
    ///  (If `true`, the text color is matched to that of the label.)
    pub tint_child: bool,
}
impl Default for SimpleButton {
    fn default() -> Self {
        Self::empty()
    }
}

impl SimpleButton {
    /// Creates a new accessible button with the specified label.
    #[inline]
    pub fn new(label: impl Into<String>) -> Self {
        Self {
            label: Some(label.into()),
            ..Default::default()
        }
    }

    /// Creates a new accessible button.
    #[inline]
    pub fn empty() -> Self {
        Self {
            label: None,
            accessible_name: None,
            variant: ButtonVariant::default(),
            size: ButtonSize::default(),
            disabled: false,
            rounded: false,
            start_child: None,
            end_child: None,
            rtl: false,
            intent: ButtonIntent::default(),
            tint_child: true,
        }
    }

    /// Set the label.
    #[inline]
    pub fn with_label(mut self, label: impl Into<String>) -> Self {
        self.label = Some(label.into());
        self
    }

    /// Set accessible name.
    ///
    /// Overwrite the displayed label with a more specific accessibility name.
    #[inline]
    pub fn a11y_name(mut self, name: impl Into<String>) -> Self {
        self.accessible_name = Some(name.into());
        self
    }

    /// Sets the visual style variant.
    #[inline]
    pub fn variant(mut self, variant: ButtonVariant) -> Self {
        self.variant = variant;
        self
    }
    /// Set variant [`ButtonVariant::Solid`]
    #[inline]
    pub fn solid(self) -> Self {
        self.variant(ButtonVariant::Solid)
    }
    /// Set variant [`ButtonVariant::Soft`]
    #[inline]
    pub fn soft(self) -> Self {
        self.variant(ButtonVariant::Soft)
    }
    /// Set variant [`ButtonVariant::Outline`]
    #[inline]
    pub fn outline(self) -> Self {
        self.variant(ButtonVariant::Outline)
    }
    /// Set variant [`ButtonVariant::Ghost`]
    #[inline]
    pub fn ghost(self) -> Self {
        self.variant(ButtonVariant::Ghost)
    }

    /// Set the semantic purpose and color palette applied to a button.
    #[inline]
    pub fn intent(mut self, intent: ButtonIntent) -> Self {
        self.intent = intent;
        self
    }
    /// Set intent [`ButtonIntent::Primary`]
    #[inline]
    pub fn primary(self) -> Self {
        self.intent(ButtonIntent::Primary)
    }
    /// Set intent [`ButtonIntent::Secondary`]
    #[inline]
    pub fn secondary(self) -> Self {
        self.intent(ButtonIntent::Secondary)
    }
    /// Set intent [`ButtonIntent::Destructive`]
    #[inline]
    pub fn destructive(self) -> Self {
        self.intent(ButtonIntent::Destructive)
    }
    /// Set intent [`ButtonIntent::Success`]
    #[inline]
    pub fn success(self) -> Self {
        self.intent(ButtonIntent::Success)
    }
    /// Set intent [`ButtonIntent::Warning`]
    #[inline]
    pub fn warning(self) -> Self {
        self.intent(ButtonIntent::Warning)
    }

    /// Sets the dimensions of the button.
    #[inline]
    pub fn size(mut self, size: ButtonSize) -> Self {
        self.size = size;
        self
    }
    /// Set size [`ButtonSize::Xs`]
    #[inline]
    pub fn xs(self) -> Self {
        self.size(ButtonSize::Xs)
    }
    /// Set size [`ButtonSize::Sm`]
    #[inline]
    pub fn sm(self) -> Self {
        self.size(ButtonSize::Sm)
    }
    /// Set size [`ButtonSize::Md`]
    #[inline]
    pub fn md(self) -> Self {
        self.size(ButtonSize::Md)
    }
    /// Set size [`ButtonSize::Lg`]
    #[inline]
    pub fn lg(self) -> Self {
        self.size(ButtonSize::Lg)
    }

    /// Sets whether the button is disabled.
    /// All Chroma values will be set to 0.0, and the Lightness of the border will be adjusted.
    #[inline]
    pub fn with_disabled(mut self, disabled: bool) -> Self {
        self.disabled = disabled;
        self
    }
    /// Set disabled `true`
    #[inline]
    pub fn disabled(self) -> Self {
        self.with_disabled(true)
    }
    /// Set disabled `false`
    #[inline]
    pub fn enabled(self) -> Self {
        self.with_disabled(false)
    }

    /// Set whether to round the corners (`Modifier::r_full`)
    #[inline]
    pub fn with_rounded(mut self, enabled: bool) -> Self {
        self.rounded = enabled;
        self
    }
    /// Set rounded `true`
    #[inline]
    pub fn r_full(self) -> Self {
        self.with_rounded(true)
    }
    /// Set rounded `false`
    #[inline]
    pub fn r_default(self) -> Self {
        self.with_rounded(false)
    }

    /// Add any `Entity`. If it has a label,
    /// it will be added to the left (or to the right, if the text runs from RTL).
    #[inline]
    pub fn with_start(mut self, entity: Entity) -> Self {
        self.start_child = Some(entity);
        self
    }

    /// Add any `Entity`. If it has a label,
    /// it will be added to the right (or to the left, if the text runs from RTL).
    #[inline]
    pub fn with_end(mut self, entity: Entity) -> Self {
        self.end_child = Some(entity);
        self
    }

    /// Set whether to use right-to-left (RTL) layout direction.
    #[inline]
    pub fn with_rtl(mut self, enabled: bool) -> Self {
        self.rtl = enabled;
        self
    }
    /// Set right-to-left (RTL) `true`
    #[inline]
    pub fn rtl(self) -> Self {
        self.with_rtl(true)
    }
    /// Set right-to-left (RTL) `false`
    #[inline]
    pub fn ltr(self) -> Self {
        self.with_rtl(false)
    }

    /// Set whether to match the **text color** of child elements to their own background color.
    /// (If `true`, the text color is matched to that of the label.)
    #[inline]
    pub fn tint_child(mut self, enabled: bool) -> Self {
        self.tint_child = enabled;
        self
    }
    /// Set tint_child `true`
    #[inline]
    pub fn tint_allow(self) -> Self {
        self.tint_child(true)
    }
    /// Set tint_child `false`
    #[inline]
    pub fn tint_deny(self) -> Self {
        self.tint_child(false)
    }
}

impl MichiuWidget for SimpleButton {
    #[inline]
    fn name(&self) -> &'static str {
        "Simple Button"
    }

    #[inline]
    fn a11y_spec(&self) -> &'static str {
        "WAI-ARIA Button Pattern / WCAG 2.2 AA (Target Size >= 24px, Focus Ring >= 3:1)"
    }

    /// Builds and returns the underlying accessible UI entity.
    ///
    /// Additional styling or event handlers (such as `.on_click(...)`) can be chained
    /// onto the returned `Entity` to overwrite defaults.
    fn build(&self, theme: &dyn MichiuTheme) -> Entity {
        let (font_size, padding) = match self.size {
            ButtonSize::Xs => (14.0, 4.0),
            ButtonSize::Sm => (14.0, 6.0),
            ButtonSize::Md => (16.0, 8.0),
            ButtonSize::Lg => (16.0, 10.0),
        };

        let (s3, s4, s5, s7, _s8, s9, s10, s11, _text_color) = match self.intent {
            ButtonIntent::Primary => (
                theme.accent_3(),
                theme.accent_4(),
                theme.accent_5(),
                theme.accent_7(),
                theme.accent_8(),
                theme.accent_9(),
                theme.accent_10(),
                theme.accent_11(),
                theme.text_on_solid(),
            ),
            ButtonIntent::Secondary => (
                theme.gray_3(),
                theme.gray_4(),
                theme.gray_5(),
                theme.gray_7(),
                theme.gray_8(),
                theme.gray_9(),
                theme.gray_10(),
                theme.gray_11(),
                theme.text_high(),
            ),
            ButtonIntent::Destructive => (
                theme.destructive(3),
                theme.destructive(4),
                theme.destructive(5),
                theme.destructive(7),
                theme.destructive(8),
                theme.destructive(9),
                theme.destructive(10),
                theme.destructive(11),
                theme.text_high(),
            ),
            ButtonIntent::Success => (
                theme.success(3),
                theme.success(4),
                theme.success(5),
                theme.success(7),
                theme.success(8),
                theme.success(9),
                theme.success(10),
                theme.success(11),
                theme.text_high(),
            ),
            ButtonIntent::Warning => (
                theme.warning(3),
                theme.warning(4),
                theme.warning(5),
                theme.warning(7),
                theme.warning(8),
                theme.warning(9),
                theme.warning(10),
                theme.warning(11),
                theme.text_on_warning_solid(),
            ),
        };

        let (bg, hover, active, border, text_color) = match self.variant {
            ButtonVariant::Solid => (s9, s10, s11, Color::TRANSPARENT, theme.text_on_solid()),
            ButtonVariant::Soft => (s3, s4, s5, s7, s11),
            ButtonVariant::Outline => (Color::TRANSPARENT, s3, s4, s7, s11),
            ButtonVariant::Ghost => (Color::TRANSPARENT, s3, s4, Color::TRANSPARENT, s11),
        };

        let (disabled_bg, disabled_border, text_color) = if self.disabled {
            match self.variant {
                ButtonVariant::Solid => (theme.gray_3(), Color::TRANSPARENT, theme.gray_8()),
                ButtonVariant::Soft => (theme.gray_2(), Color::TRANSPARENT, theme.gray_8()),
                ButtonVariant::Outline => (Color::TRANSPARENT, theme.gray_6(), theme.gray_8()),
                ButtonVariant::Ghost => (Color::TRANSPARENT, Color::TRANSPARENT, theme.gray_8()),
            }
        } else {
            (bg, border, text_color)
        };

        let offset = theme.focus_ring_offset(self.variant == ButtonVariant::Solid);

        let r = if self.rounded { 9999.0 } else { 5.0 };

        let modifier = ii()
            .justify_center()
            .items_center()
            .r(r)
            .p(padding)
            .gap(6.0)
            .min_size(24.0)
            .size_fit()
            .bg_color(bg)
            .border_solid(1.0)
            .border_color(border)
            .cursor_pointer()
            .hovered(ii().bg_color(hover))
            .pressed(ii().bg_color(active))
            .disabled(
                ii().bg_color(disabled_bg)
                    .border_color(disabled_border)
                    .cursor_default()
                    .focusable_none(),
            )
            .focusable_self_both()
            .focused_visible(
                ii().border_color(Color::TRANSPARENT)
                    .outline_solid(2.0)
                    .outline_offset(offset)
                    .outline_color(theme.focus_ring()),
            );

        let mut entity = flex(modifier).tag::<a11y::AButton>().disable(self.disabled);
        let mut label_style = ii().font_size(font_size).text_color(text_color);
        let child_style = ii()
            .hovered_parent(ii().text_color(text_color))
            .pressed_parent(ii().text_color(text_color))
            .text_color(text_color);

        if self.rtl {
            entity = entity.modify(ii().flex_row_reverse());
            label_style = label_style.rtl();
        }

        if let Some(mut left) = self.start_child {
            if self.tint_child {
                left = left.modify(&child_style);
            }
            entity = entity.child(left);
        }
        if let Some(center) = self.label.clone() {
            entity = entity.label(center, label_style);
        }
        if let Some(mut right) = self.end_child {
            if self.tint_child {
                right = right.modify(&child_style);
            }
            entity = entity.child(right);
        }

        if let Some(name) = self.accessible_name.clone() {
            entity = entity.a11y_label(Role::Button, name);
        }

        entity
    }
}
