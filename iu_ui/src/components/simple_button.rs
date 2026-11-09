use super::core::{MichiuTheme, MichiuWidget};
use michiu::{accessibility::*, prelude::*};

/// Visual appearance variants for [`SimpleButton`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ButtonVariant {
    /// Filled background using `theme.solid()`.
    #[default]
    Solid,
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
    /// Compact square size: 28x28px (exceeds WCAG 2.5.8 minimum of 24x24px).
    Sm,
    /// Standard size: 96x36px.
    #[default]
    Md,
    /// Large size: 120x44px (meets WCAG AAA target size of 44x44px vertically).
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
///   - **Target Size**: Minimum 28x28px across all variants (WCAG 2.5.8 Level AA compliant).
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
pub struct SimpleButton {
    /// Accessible button label text.
    pub label: String,
    /// Visual style variant (Solid, Outline, Ghost).
    pub variant: ButtonVariant,
    /// Button dimensions.
    pub size: ButtonSize,
    /// Whether the button is disabled.
    pub disabled: bool,
}

impl SimpleButton {
    /// Creates a new accessible button with the specified label and default options.
    pub fn new(label: impl Into<String>) -> Self {
        Self {
            label: label.into(),
            variant: ButtonVariant::default(),
            size: ButtonSize::default(),
            disabled: false,
        }
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
}

impl MichiuWidget for SimpleButton {
    fn name(&self) -> &'static str {
        "Button"
    }

    fn a11y_spec(&self) -> &'static str {
        "WAI-ARIA Button Pattern / WCAG 2.2 AA (Target Size >= 24px, Focus Ring >= 3:1)"
    }

    /// Builds and returns the underlying accessible UI entity.
    ///
    /// Additional styling or event handlers (such as `.on_click(...)`) can be chained
    /// onto the returned `Entity` to overwrite defaults.
    fn build(&self, theme: &dyn MichiuTheme) -> Entity {
        // cargo-iu_ui テスト用
        let _ = include_bytes!("../assets/close.svg");
        let size = match self.size {
            ButtonSize::Sm => (28.0, 28.0),
            ButtonSize::Md => (96.0, 36.0),
            ButtonSize::Lg => (120.0, 44.0),
        };

        let (bg, hover, active, border, text_color) = match self.variant {
            ButtonVariant::Solid => (
                theme.accent_solid(),
                theme.accent_solid_hover(),
                theme.accent_8(),
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
                ButtonVariant::Solid => (theme.gray_7(), Color::TRANSPARENT, theme.gray_text_low()),
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

        let modifier = ii()
            .justify_center()
            .items_center()
            .size(size)
            .r(3.0)
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

        flex(modifier)
            .tag::<a11y::AButton>()
            .disable(self.disabled)
            .label(
                self.label.clone(),
                ii().font_size(14.0).text_color(text_color),
            )
    }
}
