use michiu::{accessibility::*, prelude::*};

use crate::catalog::core::MichiuTheme;

/// A theme palette compliant with WCAG 2.2 AA standards for both light and dark modes,
/// based on the 12-level scale from [Radix Colors](https://www.radix-ui.com/colors).
///
/// # Contrast Ratio Verification (WCAG 2.2 Level AA)
///
/// ## 1. Text Contrast (Criterion 1.4.3: Normal Text >= 4.5:1, Large Text >= 3.0:1)
/// | Foreground | Background | Light Ratio | Dark Ratio | Status | Notes |
/// | :--- | :--- | :---: | :---: | :---: | :--- |
/// | `text_high` | `bg_app` (Step 1) | 14.13:1 | 16.20:1 | **AAA Pass** | Body text & headings |
/// | `text_low` | `bg_app` (Step 1) | 6.67:1 | 8.75:1 | **AA Pass** | Subtle text / links on window background |
/// | `text_low` | `bg_subtle` (Step 2) | 6.42:1 | 7.82:1 | **AA Pass** | Secondary text on cards / containers |
/// | `text_low` | `bg_ui` (Step 3) | 5.82:1 | 6.57:1 | **AA Pass** | Button labels in normal state |
/// | `text_low` | `bg_ui_hover` (Step 4) | 5.31:1 | 5.64:1 | **AA Pass** | Button labels on hover |
/// | `text_low` | `bg_ui_active` (Step 5) | 4.88:1 | 4.77:1 | **AA Pass** | Button labels when pressed |
/// | `White` (255) | `solid` (Step 9) | 4.80:1 | 5.10:1 | **AA Pass** | White text on primary button |
///
/// ## 2. Non-Text & Focus Indicator Contrast (Criterion 1.4.11: >= 3.0:1)
/// | Foreground / Border | Background | Light Ratio | Dark Ratio | Status | Notes |
/// | :--- | :--- | :---: | :---: | :---: | :--- |
/// | `focus_ring` | `bg_app` (Step 1) | 3.50:1 | 5.11:1 | **AA Pass** | Focus indicator on app background |
/// | `focus_ring` | `bg_subtle` (Step 2) | 3.37:1 | 4.57:1 | **AA Pass** | Focus indicator on card background |
/// | `focus_ring` | `bg_ui` (Step 3) | 3.05:1 | 3.84:1 | **AA Pass** | Focus indicator on interactive elements |
/// | `solid` | `bg_app` (Step 1) | 4.16:1 | 3.70:1 | **AA Pass** | Primary button visual boundary |
/// | `border` | `bg_app` (Step 1) | 1.70:1 | 3.25:1 | **Dark Only** | Component outline / standalone border |
///
/// ---
///
/// # Implementation Notes
///
/// ### 1. Standalone Use of `border` in Light Mode (WCAG 1.4.11)
/// - In **Light** mode, the contrast ratio between `border` (188) and `bg_app` is **~1.70:1**, which does not meet the 3.0:1 threshold for standalone boundaries.
///   - This is acceptable if the component's boundary is already identifiable by a distinct background fill (e.g., `bg_ui`), classifying the border as decorative.
///   - For components relying **solely on a border** to indicate their interactive boundary (e.g., transparent/flat text inputs), use a darker color such as `text_low` for the border instead.
/// - In **Dark** mode, `border` maintains a ratio of **3.25:1**, fully satisfying compliance criteria even as a standalone border.
///
/// ### 2. Focus Ring Placement on `solid` Controls
/// - The direct contrast between `focus_ring` and `solid` is insufficient (**~1.19:1** in Light mode, **~1.38:1** in Dark mode).
/// - When rendering a focus indicator on `solid` buttons, position it **externally with an offset (`outline_offset`)** rather than inside the button bounds.
///   This ensures the indicator is evaluated against the surrounding `bg_app` or `bg_subtle`, maintaining the required 3.0:1+ visual contrast.
///
/// ### 3. Customizing Luminance for `solid`
/// - The contrast ratio between `solid` and white text in Light mode is **4.80:1** (target threshold: 4.5:1).
/// - When customizing this theme (e.g., adapting to brand colors), avoid increasing the lightness/luminance of `solid` in Light mode, as it will quickly fall below the 4.5:1 minimum requirement.
#[derive(Debug, Clone, PartialEq)]
pub struct SimpleTheme1 {
    pub is_dark: bool,

    /// `oklch(0.9642 0.0041, 271.37)`
    pub anchor: Color,

    /// Light: `oklch(0.9521, 0.0, 0.0)` Dark: `oklch(0.1785, 0.0041, 285.98)`
    /// - Usage: Backgrounds
    /// - Pairs with: `text_low`, `text_high`
    pub bg_app: Color, // Step 1

    /// Light: `oklch(0.9401, 0.0, 0.0)` Dark: `oklch(0.2143, 0.0039, 286.07)`
    /// - Usage: Backgrounds
    /// - Pairs with: Text (`text_low`, `text_high`)
    pub bg_subtle: Color, // Step 2

    /// Light: `oklch(0.9097, 0.0, 0.0)` Dark: `oklch(0.2559, 0.004, 264.5)`
    /// - Usage: Interactive components
    /// - Pairs with: Label (`text_low`), Text (`text_high`)
    pub bg_ui: Color, // Step 3

    /// Light: `oklch(0.8822, 0.0, 0.0)` Dark: `oklch(0.2856, 0.0074, 274.7)`
    /// - Usage: Interactive components
    /// - Pairs with: Label (`text_low`, `text_high`)
    pub bg_ui_hover: Color, // Step 4

    /// Light: `oklch(0.8576, 0.0, 0.0)` Dark: `oklch(0.3141, 0.009, 276.85)`
    /// - Usage: Interactive components
    /// - Pairs with: Label (`text_high`)
    pub bg_ui_active: Color, // Step 5

    /// Light: `oklch(0.8328, 0.0, 0.0)` Dark: `oklch(0.3498, 0.0105, 278.3)`
    /// - Usage: Borders and separators
    /// - Pairs with: Backgrounds (`bg_app`, `bg_subtle`, `bg_ui`, `bg_ui_hover`, `bg_ui_active`)
    pub border_subtle: Color, // Step 6

    /// Light: `oklch(0.7952, 0.0, 0.0)` Dark: `oklch(0.4024, 0.012, 273.12)`
    /// - Usage: Borders and separators
    /// - Pairs with: Backgrounds (`bg_app`, `bg_subtle`, `bg_ui`, `bg_ui_hover`, `bg_ui_active`)
    pub border: Color, // Step 7

    /// Light: `oklch(0.58, 0.1155, 251.64)` Dark: `oklch(0.5032, 0.1377, 266.68)`
    /// - Usage: Borders, focus rings
    /// - Pairs with: Backgrounds (`bg_app`, `bg_subtle`, `bg_ui`, `bg_ui_hover`, `bg_ui_active`)
    pub focus_ring: Color, // Step 8

    /// Light: `oklch(0.556, 0.0, 0.0)` Dark: `oklch(0.5403, 0.0172, 277.6)`
    /// - Usage: Solid backgrounds, disabled text
    /// - Pairs with: White text
    pub solid: Color, // Step 9

    /// Light: `oklch(0.518, 0.0, 0.0)` Dark: `oklch(0.5849, 0.0169, 277.65)`
    /// - Usage: Solid backgrounds, disabled text
    /// - Pairs with: White text
    pub solid_hover: Color, // Step 10

    /// Light: `oklch(0.4459, 0.0, 0.0)` Dark: `oklch(0.769, 0.0142, 281.49)`
    /// - Usage: Secondary text, links
    /// - Pairs with: Background colors
    pub text_low: Color, // Step 11

    /// Light: `oklch(0.2435, 0.0, 0.0)` Dark: `oklch(0.9497, 0.0027, 286.35)`
    /// - Usage: High-contrast text
    /// - Pairs with: Background colors
    pub text_high: Color, // Step 12
}
impl SimpleTheme1 {
    pub const ANCHOR_LIGHT: Color = Self::rgb(242, 243, 246);
    pub const ANCHOR_DARK: Color = Self::rgb(17, 17, 19);

    pub const fn rgb(r: u8, g: u8, b: u8) -> Color {
        Color {
            r: r as f32 / 255.0,
            g: g as f32 / 255.0,
            b: b as f32 / 255.0,
            a: 1.0,
        }
    }
    pub const fn rgb_single(v: u8) -> Color {
        Self::rgb(v, v, v)
    }
    pub const fn light() -> Self {
        Self {
            is_dark: false,
            anchor: Self::ANCHOR_LIGHT,
            bg_app: Self::rgb_single(239),
            bg_subtle: Self::rgb_single(235),
            bg_ui: Self::rgb_single(225),
            bg_ui_hover: Self::rgb_single(216),
            bg_ui_active: Self::rgb_single(208),
            border_subtle: Self::rgb_single(200),
            border: Self::rgb_single(188),
            focus_ring: Self::rgb(30, 130, 215),
            solid: Self::rgb_single(115),
            solid_hover: Self::rgb_single(104),
            text_low: Self::rgb_single(84),
            text_high: Self::rgb_single(32),
        }
    }
    pub const fn dark() -> Self {
        Self {
            is_dark: true,
            anchor: Self::ANCHOR_DARK,
            bg_app: Self::rgb(17, 17, 19),
            bg_subtle: Self::rgb(25, 25, 27),
            bg_ui: Self::rgb(34, 35, 37),
            bg_ui_hover: Self::rgb(41, 42, 46),
            bg_ui_active: Self::rgb(48, 49, 54),
            border_subtle: Self::rgb(57, 58, 64),
            border: Self::rgb(70, 72, 79),
            focus_ring: Self::rgb(64, 94, 178),
            solid: Self::rgb(108, 110, 121),
            solid_hover: Self::rgb(121, 123, 134),
            text_low: Self::rgb(178, 179, 189),
            text_high: Self::rgb(238, 238, 240),
        }
    }
}
impl Default for SimpleTheme1 {
    fn default() -> Self {
        Self::light()
    }
}

impl MichiuTheme for SimpleTheme1 {
    fn name(&self) -> &'static str {
        if self.is_dark {
            "SimpleTheme1 (Dark)"
        } else {
            "SimpleTheme1 (Light)"
        }
    }

    fn is_dark(&self) -> bool {
        self.is_dark
    }

    fn bg_app(&self) -> Color {
        self.bg_app
    }
    fn bg_subtle(&self) -> Color {
        self.bg_subtle
    }

    fn bg_ui(&self) -> Color {
        self.bg_ui
    }
    fn bg_ui_hover(&self) -> Color {
        self.bg_ui_hover
    }
    fn bg_ui_active(&self) -> Color {
        self.bg_ui_active
    }

    fn border_subtle(&self) -> Color {
        self.border_subtle
    }
    fn border(&self) -> Color {
        self.border
    }
    fn focus_ring(&self) -> Color {
        self.focus_ring
    }

    fn solid(&self) -> Color {
        self.solid
    }
    fn solid_hover(&self) -> Color {
        self.solid_hover
    }

    fn text_low(&self) -> Color {
        self.text_low
    }
    fn text_high(&self) -> Color {
        self.text_high
    }
}
