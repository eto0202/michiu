use crate::{Oklch, core::MichiuTheme};
use michiu::prelude::*;

/// An accessible blue-accented theme compliant with WCAG 2.2 AA standards.
///
/// Implements [`MichiuTheme`] to automatically derive a full 24-color dual palette
/// (12 Accent + 12 Gray) using the specified [`ACCENT`](Self::ACCENT) and background anchors.
///
/// # Example
///
/// ```rust
/// // 1. Use the default light theme
/// let theme = SimpleBlueTheme::light();
/// let button = SimpleButton::new("Save").build(&theme);
///
/// // 2. Toggle to dark theme
/// let dark_theme = SimpleBlueTheme::dark();
/// let dark_button = SimpleButton::new("Save").build(&dark_theme);
/// ```
#[derive(Debug, Clone, PartialEq)]
pub struct SimpleBlueTheme {
    /// Whether dark mode is currently active.
    pub is_dark: bool,
    /// The canvas anchor background in OKLCH.
    pub anchor: Oklch,
    /// The primary accent color in OKLCH.
    pub accent: Oklch,
}

impl SimpleBlueTheme {
    /// Calibrated Light mode canvas anchor background (`oklch(0.9642, 0.0041, 271.37)`).
    pub const ANCHOR_LIGHT: Oklch = Oklch(0.9642, 0.0041, 271.37);

    /// Calibrated Dark mode canvas anchor background (`oklch(0.1785, 0.0041, 285.98)`).
    pub const ANCHOR_DARK: Oklch = Oklch(0.1785, 0.0041, 285.98);

    /// Vivid, accessible primary blue accent color (`oklch(0.5433, 0.1913, 266.76)`).
    pub const ACCENT: Oklch = Oklch(0.5433, 0.1913, 266.76);

    /// Converts 8-bit RGB components (0–255) into an sRGB [`Color`].
    pub const fn rgb(r: u8, g: u8, b: u8) -> Color {
        Color {
            r: r as f32 / 255.0,
            g: g as f32 / 255.0,
            b: b as f32 / 255.0,
            a: 1.0,
        }
    }

    /// Converts a single 8-bit grayscale value (0–255) into an sRGB [`Color`].
    pub const fn rgb_single(v: u8) -> Color {
        Self::rgb(v, v, v)
    }

    /// Constructs the default Light variant of the blue theme.
    pub const fn light() -> Self {
        Self {
            is_dark: false,
            anchor: Self::ANCHOR_LIGHT,
            accent: Self::ACCENT,
        }
    }

    /// Constructs the default Dark variant of the blue theme.
    pub const fn dark() -> Self {
        Self {
            is_dark: true,
            anchor: Self::ANCHOR_DARK,
            accent: Self::ACCENT,
        }
    }
}

impl Default for SimpleBlueTheme {
    /// Returns the default Light theme variant.
    fn default() -> Self {
        Self::light()
    }
}

impl MichiuTheme for SimpleBlueTheme {
    fn name(&self) -> &'static str {
        if self.is_dark {
            "SimpleBlueTheme (Dark)"
        } else {
            "SimpleBlueTheme (Light)"
        }
    }

    fn is_dark(&self) -> bool {
        self.is_dark
    }

    fn accent(&self) -> Oklch {
        self.accent
    }

    fn background(&self) -> Oklch {
        if self.is_dark {
            Self::ANCHOR_DARK
        } else {
            Self::ANCHOR_LIGHT
        }
    }
}
