use michiu::prelude::*;

/// Common trait for accessible components registered in the catalog.
pub trait MichiuWidget {
    /// Display name of the component.
    fn name(&self) -> &'static str;

    /// Summary of satisfied WAI-ARIA pattern names and/or WCAG success criteria.
    fn a11y_spec(&self) -> &'static str;

    /// Builds an `Entity` given the current theme (Light/Dark).
    fn build(&self, theme: &dyn MichiuTheme) -> Entity;
}

/// Common theme trait providing a WCAG 2.2 AA-compliant color palette.
/// Based on the 12-step scale from [Radix Colors](https://www.radix-ui.com/colors).
pub trait MichiuTheme: Send + Sync {
    /// Display name of the theme.
    fn name(&self) -> &'static str;

    /// Returns whether the theme is dark mode.
    fn is_dark(&self) -> bool;

    /// App/window base background (Step 1).
    fn bg_app(&self) -> Color;
    /// Subtle background for sub-containers such as cards and sidebars (Step 2).
    fn bg_subtle(&self) -> Color;

    /// UI element background in normal state (Step 3).
    fn bg_ui(&self) -> Color;
    /// UI element background on hover (Step 4).
    fn bg_ui_hover(&self) -> Color;
    /// UI element background when pressed/active (Step 5).
    fn bg_ui_active(&self) -> Color;

    /// Subtle borders for decorative separators and dividers (Step 6).
    fn border_subtle(&self) -> Color;
    /// Border for UI controls (Step 7).
    fn border(&self) -> Color;
    /// Keyboard focus ring (Step 8: guarantees a contrast ratio of at least 3.0:1).
    fn focus_ring(&self) -> Color;

    /// Solid background for primary buttons, etc. (Step 9).
    fn solid(&self) -> Color;
    /// Hover state for solid background (Step 10).
    fn solid_hover(&self) -> Color;

    /// Low-contrast text for secondary text and links (Step 11: guarantees at least 4.5:1).
    fn text_low(&self) -> Color;
    /// High-contrast text for body text and headings (Step 12).
    fn text_high(&self) -> Color;

    /// Text color on top of a solid background (Step 9) (defaults to white; override with black for yellow hues, etc.).
    fn text_on_solid(&self) -> Color {
        Color::WHITE
    }

    /// Appropriate offset for the focus ring (whether to place it outside or inside).
    fn focus_ring_offset(&self, is_solid_bg: bool) -> f32 {
        if is_solid_bg { 2.0 } else { 0.0 }
    }
}
