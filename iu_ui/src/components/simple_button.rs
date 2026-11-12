use super::core::{MichiuTheme, MichiuWidget};
use michiu::{accessibility::*, prelude::*};

/// Visual appearance variants for [`SimpleButton`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ButtonVariant {
    /// Filled background using `theme.accent_solid()`.
    #[default]
    Solid,
    /// Filled background using `theme.gray_solid()`.
    Secondary,
    /// Transparent background with an outline border using `theme.border()`.
    Outline,
    /// Fully transparent background that subtly highlights on hover.
    Ghost,
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
}
impl Default for SimpleButton {
    fn default() -> Self {
        Self::empty()
    }
}

impl SimpleButton {
    /// Creates a new accessible button with the specified label.
    pub fn new(label: impl Into<String>) -> Self {
        Self {
            label: Some(label.into()),
            ..Default::default()
        }
    }

    /// Creates a new accessible button.
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
        }
    }

    /// Set the label.
    pub fn with_label(mut self, label: impl Into<String>) -> Self {
        self.label = Some(label.into());
        self
    }

    /// Set accessible name.
    ///
    /// Overwrite the displayed label with a more specific accessibility name.
    pub fn accessible_name(mut self, name: impl Into<String>) -> Self {
        self.accessible_name = Some(name.into());
        self
    }

    /// Sets the visual style variant.
    pub fn variant(mut self, variant: ButtonVariant) -> Self {
        self.variant = variant;
        self
    }

    /// Sets the dimensions of the button.
    pub fn size(mut self, size: ButtonSize) -> Self {
        self.size = size;
        self
    }

    /// Sets whether the button is disabled.
    /// All Chroma values will be set to 0.0, and the Lightness of the border will be adjusted.
    pub fn disabled(mut self, disabled: bool) -> Self {
        self.disabled = disabled;
        self
    }

    /// Set whether to round the corners (`Modifier::r_full`)
    pub fn rounded(mut self, enabled: bool) -> Self {
        self.rounded = enabled;
        self
    }

    /// Add any `Entity`. If it has a label,
    /// it will be added to the left (or to the right, if the text runs from RTL).
    pub fn with_start(mut self, entity: Entity) -> Self {
        self.start_child = Some(entity);
        self
    }

    /// Add any `Entity`. If it has a label,
    /// it will be added to the right (or to the left, if the text runs from RTL).
    pub fn with_end(mut self, entity: Entity) -> Self {
        self.end_child = Some(entity);
        self
    }

    /// Set whether to use right-to-left (RTL) layout direction.
    pub fn rtl(mut self, enabled: bool) -> Self {
        self.rtl = enabled;
        self
    }
}

impl MichiuWidget for SimpleButton {
    fn name(&self) -> &'static str {
        "Simple Button"
    }

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
            ButtonSize::Md => (15.0, 8.0),
            ButtonSize::Lg => (15.0, 10.0),
        };

        let (bg, hover, active, border, text_color) = match self.variant {
            ButtonVariant::Solid => (
                theme.accent_solid(),
                theme.accent_solid_hover(),
                theme.accent_8(),
                Color::TRANSPARENT,
                theme.text_on_solid(),
            ),
            ButtonVariant::Secondary => (
                theme.gray_solid(),
                theme.gray_solid_hover(),
                theme.gray_8(),
                Color::TRANSPARENT,
                theme.text_on_solid(),
            ),
            ButtonVariant::Outline => (
                Color::TRANSPARENT,
                theme.gray_bg_ui_hover(),
                theme.gray_border(),
                theme.gray_bg_ui_active(),
                theme.gray_text_high(),
            ),
            ButtonVariant::Ghost => (
                Color::TRANSPARENT,
                theme.gray_bg_ui_hover(),
                theme.gray_bg_ui_active(),
                Color::TRANSPARENT,
                theme.gray_text_high(),
            ),
        };
        let (disabled_bg, disabled_border, text_color) = if self.disabled {
            match self.variant {
                ButtonVariant::Solid | ButtonVariant::Secondary => {
                    (theme.gray_7(), Color::TRANSPARENT, theme.gray_text_low())
                }
                ButtonVariant::Outline => {
                    (Color::TRANSPARENT, theme.gray_4(), theme.gray_text_low())
                }
                ButtonVariant::Ghost => (
                    Color::TRANSPARENT,
                    Color::TRANSPARENT,
                    theme.gray_text_low(),
                ),
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

        if self.rtl {
            entity = entity.modify(ii().flex_row_reverse());
            label_style = label_style.rtl();
        }

        if let Some(left) = self.start_child {
            entity = entity.child(left);
        }
        if let Some(center) = self.label.clone() {
            entity = entity.label(center, label_style);
        }
        if let Some(right) = self.end_child {
            entity = entity.child(right);
        }

        if let Some(name) = self.accessible_name.clone() {
            entity = entity.a11y_label(Role::Button, name);
        }

        entity
    }
}
