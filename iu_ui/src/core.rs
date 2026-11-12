use michiu::{prelude::*, ui::IntoModValue};

/// Common trait for accessible components registered in the catalog.
pub trait MichiuWidget {
    /// Display name of the component.
    fn name(&self) -> &'static str;

    /// Summary of satisfied WAI-ARIA pattern names and/or WCAG success criteria.
    fn a11y_spec(&self) -> &'static str;

    /// Builds an `Entity` given the current theme (Light/Dark).
    fn build(&self, theme: &dyn MichiuTheme) -> Entity;
}

/// A dual-scale accessible theme trait providing 24 calibrated colors (12 Accent + 12 Gray)
/// based on the [Radix Colors](https://www.radix-ui.com/colors) 12-step architecture.
///
/// # Color Architecture & Derivation
///
/// - **Accent Scale (`accent_1` .. `accent_12`)**:
///   Derived mathematically from the user-defined [`accent`](Self::accent) OKLCH parameters.
///   Maintains WCAG AA/AAA-compliant luminance curves across all hues.
/// - **Gray Scale (`gray_1` .. `gray_12`)**:
///   Derived from [`gray`](Self::gray), which defaults to an achromatic ($C = 0.0$) version
///   of [`accent`](Self::accent). Can be optionally tinted by specifying a custom chroma.
///
/// # 12-Step Scale Specification
///
/// | Step | Category | Primary Purpose | WCAG Compliance Target |
/// | :---: | :--- | :--- | :--- |
/// | **1** | Backgrounds | App / window background | Base canvas |
/// | **2** | Backgrounds | Card / sidebar container background | Subtle elevation |
/// | **3** | Interactive | Normal UI component background | Visible against Steps 1–2 |
/// | **4** | Interactive | Hover state UI component background | Interactive feedback |
/// | **5** | Interactive | Active / pressed UI component background | Tactile feedback |
/// | **6** | Borders | Subtle borders, dividers, table rules | Decorative boundaries |
/// | **7** | Borders | Component outlines & input borders | Recognizable boundaries |
/// | **8** | Borders & Rings | Focus rings, strong active borders | Non-text contrast >= 3.0:1 |
/// | **9** | Solid Colors | Primary solid button / badge backgrounds | High-saturation brand anchor |
/// | **10** | Solid Colors | Hover state for solid backgrounds | Darker/lighter interaction |
/// | **11** | Accessible Text | Secondary text, links, subdued icons | Contrast >= 4.5:1 (AA) |
/// | **12** | Accessible Text | Body text, titles, headings | Contrast >= 7.0:1 (AAA) |
pub trait MichiuTheme: Send + Sync {
    /// Human-readable display name of the theme (used in catalog previews, UI pickers, etc.).
    fn name(&self) -> &'static str {
        if self.is_dark() {
            "Custom Theme (Dark)"
        } else {
            "Custom Theme (Light)"
        }
    }

    /// Returns `true` if this theme is currently active in dark mode.
    fn is_dark(&self) -> bool;

    /// **[Required]** The base accent color parameters in OKLCH: `(Lightness, Chroma, Hue)`.
    ///
    /// - `Lightness`: `0.0..=1.0`
    /// - `Chroma`: `0.0..=0.4`
    /// - `Hue`: `0.0..=360.0` (in degrees)
    fn accent(&self) -> Oklch;

    /// The base background canvas color parameters in OKLCH.
    ///
    /// Defaults to calibrated canvas values (`oklch(0.9642, 0.0041, 271.37)` in Light mode,
    /// `oklch(0.1785, 0.0041, 285.98)` in Dark mode).
    fn background(&self) -> Oklch {
        if self.is_dark() {
            Oklch(0.1785, 0.0041, 285.98) // Dark Anchor
        } else {
            Oklch(0.9642, 0.0041, 271.37) // Light Anchor
        }
    }

    /// The base neutral gray parameters in OKLCH.
    ///
    /// Defaults to an **achromatic copy ($C = 0.0$)** of [`accent`](Self::accent),
    /// producing pure, neutral grays with zero unwanted hue tinting.
    fn gray(&self) -> Oklch {
        let (l, _, h) = self.accent().tuple();
        Oklch(l, 0.0, h)
    }

    // =========================================================================
    // Gray Scale (Steps 1–12)
    // =========================================================================

    /// **Gray Step 1**: Primary application / window background.
    fn gray_1(&self) -> Color {
        let (_, c, h) = self.gray().tuple();
        Color::oklch(if self.is_dark() { 0.1785 } else { 0.9521 }, c, h)
    }

    /// **Gray Step 2**: Subtle container background (cards, sidebars, modals).
    fn gray_2(&self) -> Color {
        let (_, c, h) = self.gray().tuple();
        Color::oklch(if self.is_dark() { 0.2143 } else { 0.9401 }, c, h)
    }

    /// **Gray Step 3**: Default background for interactive components (ghost buttons, inputs).
    fn gray_3(&self) -> Color {
        let (_, c, h) = self.gray().tuple();
        Color::oklch(if self.is_dark() { 0.2559 } else { 0.9097 }, c, h)
    }

    /// **Gray Step 4**: Hovered background for interactive components.
    fn gray_4(&self) -> Color {
        let (_, c, h) = self.gray().tuple();
        Color::oklch(if self.is_dark() { 0.2856 } else { 0.8822 }, c, h)
    }

    /// **Gray Step 5**: Active / pressed background for interactive components.
    fn gray_5(&self) -> Color {
        let (_, c, h) = self.gray().tuple();
        Color::oklch(if self.is_dark() { 0.3141 } else { 0.8576 }, c, h)
    }

    /// **Gray Step 6**: Subtle borders, structural dividers, and decorative rules.
    fn gray_6(&self) -> Color {
        let (_, c, h) = self.gray().tuple();
        Color::oklch(if self.is_dark() { 0.3498 } else { 0.8328 }, c, h)
    }

    /// **Gray Step 7**: Component borders and input outlines.
    fn gray_7(&self) -> Color {
        let (_, c, h) = self.gray().tuple();
        Color::oklch(if self.is_dark() { 0.4024 } else { 0.7952 }, c, h)
    }

    /// **Gray Step 8**: Strong borders, focus indicators, and high-visibility outlines (>= 3.0:1 contrast).
    fn gray_8(&self) -> Color {
        let (_, c, h) = self.gray().tuple();
        Color::oklch(if self.is_dark() { 0.5032 } else { 0.5800 }, c, h)
    }

    /// **Gray Step 9**: Solid neutral backgrounds (secondary filled buttons, neutral chips).
    fn gray_9(&self) -> Color {
        let (_, c, h) = self.gray().tuple();
        Color::oklch(if self.is_dark() { 0.5403 } else { 0.5560 }, c, h)
    }

    /// **Gray Step 10**: Hovered state for solid neutral backgrounds.
    fn gray_10(&self) -> Color {
        let (_, c, h) = self.gray().tuple();
        Color::oklch(if self.is_dark() { 0.5849 } else { 0.5180 }, c, h)
    }

    /// **Gray Step 11**: Subdued / secondary text, captions, and links (>= 4.5:1 contrast against Step 1).
    fn gray_11(&self) -> Color {
        let (_, c, h) = self.gray().tuple();
        Color::oklch(if self.is_dark() { 0.7690 } else { 0.4459 }, c, h)
    }

    /// **Gray Step 12**: High-contrast body text, headings, and critical labels (>= 7.0:1 AAA contrast).
    fn gray_12(&self) -> Color {
        let (_, c, h) = self.gray().tuple();
        Color::oklch(if self.is_dark() { 0.9497 } else { 0.2435 }, c, h)
    }

    // =========================================================================
    // Accent Scale (Steps 1–12)
    // =========================================================================

    /// **Accent Step 1**: Extremely subtle tinted application background.
    fn accent_1(&self) -> Color {
        let (_, c, h) = self.accent().tuple();
        Color::oklch(if self.is_dark() { 0.1785 } else { 0.9521 }, c * 0.15, h)
    }

    /// **Accent Step 2**: Subtle tinted container background (selected card, highlight container).
    fn accent_2(&self) -> Color {
        let (_, c, h) = self.accent().tuple();
        Color::oklch(if self.is_dark() { 0.2143 } else { 0.9401 }, c * 0.25, h)
    }

    /// **Accent Step 3**: Default background for tinted interactive components.
    fn accent_3(&self) -> Color {
        let (_, c, h) = self.accent().tuple();
        Color::oklch(if self.is_dark() { 0.2559 } else { 0.9097 }, c * 0.40, h)
    }

    /// **Accent Step 4**: Hovered background for tinted interactive components.
    fn accent_4(&self) -> Color {
        let (_, c, h) = self.accent().tuple();
        Color::oklch(if self.is_dark() { 0.2856 } else { 0.8822 }, c * 0.55, h)
    }

    /// **Accent Step 5**: Active / pressed background for tinted interactive components.
    fn accent_5(&self) -> Color {
        let (_, c, h) = self.accent().tuple();
        Color::oklch(if self.is_dark() { 0.3141 } else { 0.8576 }, c * 0.70, h)
    }

    /// **Accent Step 6**: Subtle tinted borders and selected item dividers.
    fn accent_6(&self) -> Color {
        let (_, c, h) = self.accent().tuple();
        Color::oklch(if self.is_dark() { 0.3498 } else { 0.8328 }, c * 0.80, h)
    }

    /// **Accent Step 7**: Tinted component borders (selected tab underline, active inputs).
    fn accent_7(&self) -> Color {
        let (_, c, h) = self.accent().tuple();
        Color::oklch(if self.is_dark() { 0.4024 } else { 0.7952 }, c * 0.90, h)
    }

    /// **Accent Step 8**: High-visibility focus indicator ring (>= 3.0:1 contrast against Step 1).
    fn accent_8(&self) -> Color {
        let (_, c, h) = self.accent().tuple();
        Color::oklch(if self.is_dark() { 0.5032 } else { 0.5800 }, c, h)
    }

    /// **Accent Step 9**: Primary solid background (call-to-action buttons, active badges).
    fn accent_9(&self) -> Color {
        let (_, c, h) = self.accent().tuple();
        Color::oklch(if self.is_dark() { 0.5403 } else { 0.5560 }, c, h)
    }

    /// **Accent Step 10**: Hovered state for primary solid background.
    fn accent_10(&self) -> Color {
        let (_, c, h) = self.accent().tuple();
        Color::oklch(if self.is_dark() { 0.5849 } else { 0.5180 }, c, h)
    }

    /// **Accent Step 11**: Accessible accent text, interactive links, and colored icons (>= 4.5:1 contrast).
    fn accent_11(&self) -> Color {
        let (_, c, h) = self.accent().tuple();
        Color::oklch(if self.is_dark() { 0.7690 } else { 0.4459 }, c * 0.90, h)
    }

    /// **Accent Step 12**: High-contrast accent headings and bold titles.
    fn accent_12(&self) -> Color {
        let (_, c, h) = self.accent().tuple();
        Color::oklch(if self.is_dark() { 0.9497 } else { 0.2435 }, c * 0.40, h)
    }

    // =========================================================================
    // 3. Semantic Aliases for Gray Scale
    // =========================================================================

    /// Semantic alias for [`gray_1`](Self::gray_1) (**Step 1**: App Background).
    fn gray_bg_app(&self) -> Color {
        self.gray_1()
    }

    /// Semantic alias for [`gray_2`](Self::gray_2) (**Step 2**: Card / Sub-container Background).
    fn gray_bg_subtle(&self) -> Color {
        self.gray_2()
    }

    /// Semantic alias for [`gray_3`](Self::gray_3) (**Step 3**: Interactive Element Background).
    fn gray_bg_ui(&self) -> Color {
        self.gray_3()
    }

    /// Semantic alias for [`gray_4`](Self::gray_4) (**Step 4**: Hovered Element Background).
    fn gray_bg_ui_hover(&self) -> Color {
        self.gray_4()
    }

    /// Semantic alias for [`gray_5`](Self::gray_5) (**Step 5**: Active / Pressed Element Background).
    fn gray_bg_ui_active(&self) -> Color {
        self.gray_5()
    }

    /// Semantic alias for [`gray_6`](Self::gray_6) (**Step 6**: Subtle Separators & Table Rules).
    fn gray_border_subtle(&self) -> Color {
        self.gray_6()
    }

    /// Semantic alias for [`gray_7`](Self::gray_7) (**Step 7**: UI Outlines & Input Borders).
    fn gray_border(&self) -> Color {
        self.gray_7()
    }

    /// Semantic alias for [`gray_8`](Self::gray_8) (**Step 8**: Strong Border).
    fn gray_border_strong(&self) -> Color {
        self.gray_8()
    }

    /// Semantic alias for [`gray_9`](Self::gray_9) (**Step 9**: Solid Neutral Button Background).
    fn gray_solid(&self) -> Color {
        self.gray_9()
    }

    /// Semantic alias for [`gray_10`](Self::gray_10) (**Step 10**: Hovered Solid Neutral Background).
    fn gray_solid_hover(&self) -> Color {
        self.gray_10()
    }

    /// Semantic alias for [`gray_11`](Self::gray_11) (**Step 11**: Subdued / Secondary Text, >= 4.5:1).
    fn gray_text_low(&self) -> Color {
        self.gray_11()
    }

    /// Semantic alias for [`gray_12`](Self::gray_12) (**Step 12**: High-Contrast Body Text, >= 7.0:1).
    fn gray_text_high(&self) -> Color {
        self.gray_12()
    }

    // =========================================================================
    // Semantic Aliases for Accent Scale
    // =========================================================================

    /// Semantic alias for [`accent_1`](Self::accent_1) (**Step 1**: Tinted Canvas Background).
    fn accent_bg_app(&self) -> Color {
        self.accent_1()
    }

    /// Semantic alias for [`accent_2`](Self::accent_2) (**Step 2**: Tinted Container Background).
    fn accent_bg_subtle(&self) -> Color {
        self.accent_2()
    }

    /// Semantic alias for [`accent_3`](Self::accent_3) (**Step 3**: Tinted Interactive Component Background).
    fn accent_bg_ui(&self) -> Color {
        self.accent_3()
    }

    /// Semantic alias for [`accent_4`](Self::accent_4) (**Step 4**: Hovered Tinted Component Background).
    fn accent_bg_ui_hover(&self) -> Color {
        self.accent_4()
    }

    /// Semantic alias for [`accent_5`](Self::accent_5) (**Step 5**: Active Tinted Component Background).
    fn accent_bg_ui_active(&self) -> Color {
        self.accent_5()
    }

    /// Semantic alias for [`accent_6`](Self::accent_6) (**Step 6**: Subtle Tinted Separators).
    fn accent_border_subtle(&self) -> Color {
        self.accent_6()
    }

    /// Semantic alias for [`accent_7`](Self::accent_7) (**Step 7**: Accent Component Borders).
    fn accent_border(&self) -> Color {
        self.accent_7()
    }

    /// Semantic alias for [`accent_8`](Self::accent_8) (**Step 8**: Accent Focus Indicator).
    fn accent_focus_ring(&self) -> Color {
        self.accent_8()
    }

    /// Semantic alias for [`accent_9`](Self::accent_9) (**Step 9**: Primary Solid Button Background).
    fn accent_solid(&self) -> Color {
        self.accent_9()
    }

    /// Semantic alias for [`accent_10`](Self::accent_10) (**Step 10**: Hovered Primary Solid Background).
    fn accent_solid_hover(&self) -> Color {
        self.accent_10()
    }

    /// Semantic alias for [`accent_11`](Self::accent_11) (**Step 11**: Accessible Accent Links & Text, >= 4.5:1).
    fn accent_text_low(&self) -> Color {
        self.accent_11()
    }

    /// Semantic alias for [`accent_12`](Self::accent_12) (**Step 12**: High-Contrast Accent Text).
    fn accent_text_high(&self) -> Color {
        self.accent_12()
    }

    // =========================================================================
    // Global Widget Convenience Shortcuts & Safe Fallbacks
    // =========================================================================

    /// Default app background, delegating to [`gray_1`](Self::gray_1) (**Step 1**).
    fn bg_app(&self) -> Color {
        self.gray_1()
    }

    /// Default card / container background, delegating to [`gray_2`](Self::gray_2) (**Step 2**).
    fn bg_subtle(&self) -> Color {
        self.gray_2()
    }

    /// Default component background, delegating to [`gray_3`](Self::gray_3) (**Step 3**).
    fn bg_ui(&self) -> Color {
        self.gray_3()
    }

    /// Default hovered component background, delegating to [`gray_4`](Self::gray_4) (**Step 4**).
    fn bg_ui_hover(&self) -> Color {
        self.gray_4()
    }

    /// Default active component background, delegating to [`gray_5`](Self::gray_5) (**Step 5**).
    fn bg_ui_active(&self) -> Color {
        self.gray_5()
    }

    /// Default subtle divider color, delegating to [`gray_6`](Self::gray_6) (**Step 6**).
    fn border_subtle(&self) -> Color {
        self.gray_6()
    }

    /// Default component outline border color, delegating to [`gray_7`](Self::gray_7) (**Step 7**).
    fn border(&self) -> Color {
        self.gray_7()
    }

    /// Default high-contrast focus indicator, delegating to [`accent_8`](Self::accent_8) (**Step 8**).
    fn focus_ring(&self) -> Color {
        self.accent_8()
    }

    /// Default primary solid button color, delegating to [`accent_9`](Self::accent_9) (**Step 9**).
    fn solid(&self) -> Color {
        self.accent_9()
    }

    /// Default primary solid button hover color, delegating to [`accent_10`](Self::accent_10) (**Step 10**).
    fn solid_hover(&self) -> Color {
        self.accent_10()
    }

    /// Default secondary text color, delegating to [`gray_11`](Self::gray_11) (**Step 11**).
    fn text_low(&self) -> Color {
        self.gray_11()
    }

    /// Default high-contrast body text color, delegating to [`gray_12`](Self::gray_12) (**Step 12**).
    fn text_high(&self) -> Color {
        self.gray_12()
    }

    /// Foreground text color displayed on top of solid controls ([`solid`](Self::solid)).
    ///
    /// Defaults to white (`Color::WHITE`), but can be overridden (e.g., to dark text for yellow/amber accents).
    fn text_on_solid(&self) -> Color {
        Color::WHITE
    }

    /// External outline offset distance in logical pixels.
    ///
    /// Defaults to `2.0` for solid backgrounds (ensuring >= 3.0:1 contrast against surrounding canvas)
    /// and `0.0` for outline/transparent controls.
    fn focus_ring_offset(&self, is_solid: bool) -> f32 {
        if is_solid { 2.0 } else { 0.0 }
    }

    // =========================================================================
    // Semantic Base Color Anchors
    // =========================================================================

    /// Destructive (Red) base color: Hue 27.0°
    fn destructive_base(&self) -> Oklch {
        let (l, c, _) = self.accent().tuple();
        // 極端に彩度が低いモノトーン設定でも警告色として機能するよう最低彩度を担保
        let chroma = c.clamp(0.18, 0.26);
        Oklch(l, chroma, 27.0)
    }

    /// Success (Green) base color: Hue 148.0°
    fn success_base(&self) -> Oklch {
        let (l, c, _) = self.accent().tuple();
        // 緑は明るく見えやすいため彩度はやや控えめに
        let chroma = (c * 0.85).clamp(0.14, 0.22);
        Oklch(l, chroma, 148.0)
    }

    /// Warning (Amber) base color: Hue 72.0°
    fn warning_base(&self) -> Oklch {
        let (l, c, _) = self.accent().tuple();
        // アンバーは十分な鮮やかさが必要
        let chroma = c.clamp(0.16, 0.24);
        Oklch(l, chroma, 72.0)
    }

    /// Default Warning text color, delegating to [`gray_12`](Self::gray_12) (**Step 12**).
    fn text_on_warning_solid(&self) -> Color {
        // 白ではなく、濃いグレー（Step 12 など）または黒にする
        self.gray_12()
    }

    /// 任意のベースカラーから 12 ステップ色を生成
    fn scale_step(&self, base: Oklch, step: u8) -> Color {
        let is_dark = self.is_dark();
        let (_, c, h) = base.tuple();

        let (lightness, chroma_ratio) = match step {
            1 => (if is_dark { 0.1785 } else { 0.9521 }, 0.15),
            2 => (if is_dark { 0.2143 } else { 0.9401 }, 0.25),
            3 => (if is_dark { 0.2559 } else { 0.9097 }, 0.40),
            4 => (if is_dark { 0.2856 } else { 0.8822 }, 0.55),
            5 => (if is_dark { 0.3141 } else { 0.8576 }, 0.70),
            6 => (if is_dark { 0.3498 } else { 0.8328 }, 0.80),
            7 => (if is_dark { 0.4024 } else { 0.7952 }, 0.90),
            8 => (if is_dark { 0.5032 } else { 0.5800 }, 1.00),
            9 => (if is_dark { 0.5403 } else { 0.5560 }, 1.00),
            10 => (if is_dark { 0.5849 } else { 0.5180 }, 1.00),
            11 => (if is_dark { 0.7690 } else { 0.4459 }, 0.90),
            12 => (if is_dark { 0.9497 } else { 0.2435 }, 0.40),
            _ => (0.5, 0.0),
        };

        Color::oklch(lightness, c * chroma_ratio, h)
    }

    /// Destructive の各ステップ色
    fn destructive(&self, step: u8) -> Color {
        self.scale_step(self.destructive_base(), step)
    }

    /// Success の各ステップ色
    fn success(&self, step: u8) -> Color {
        self.scale_step(self.success_base(), step)
    }

    /// Warning の各ステップ色
    fn warning(&self, step: u8) -> Color {
        self.scale_step(self.warning_base(), step)
    }
}

/// A lightweight representation of a color in the OKLCH color space.
///
/// Contains `(Lightness, Chroma, Hue)`:
/// - `0`: Lightness (`0.0..=1.0`)
/// - `1`: Chroma (`0.0..=0.4`)
/// - `2`: Hue (`0.0..=360.0` in degrees)
#[derive(Debug, Clone, Copy, PartialEq, PartialOrd, Default)]
pub struct Oklch(pub f32, pub f32, pub f32);
impl Oklch {
    /// Returns the color components as a raw tuple `(L, C, H)`.
    pub const fn tuple(&self) -> (f32, f32, f32) {
        (self.0, self.1, self.2)
    }
    pub fn into_color(self) -> Color {
        self.into()
    }
}
impl From<Color> for Oklch {
    fn from(value: Color) -> Self {
        let (l, c, h) = value.to_oklch();
        Self(l, c, h)
    }
}
impl From<Oklch> for Color {
    fn from(value: Oklch) -> Self {
        oklch(value.0, value.1, value.2)
    }
}
