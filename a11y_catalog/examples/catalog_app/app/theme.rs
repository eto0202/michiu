#![expect(unused)]

use michiu::ui::Color;
use std::borrow::Cow;

#[derive(Debug, Clone, PartialEq)]
pub struct Theme {
    pub is_dark: bool,

    // Source OKLCH (L: 0.0..1.0, C: 0.0..0.4, H: 0.0..360.0)
    pub accent_oklch: (f32, f32, f32),
    pub background_oklch: (f32, f32, f32),

    // Accent Colors (Step 9 & 10)
    pub accent: Color,
    pub accent_hover: Color,
    pub text_on_accent: Color,

    // Layout Colors (Header > Sidebar / Surface > Background)
    pub header: Color,
    pub header_hover: Color,
    pub sidebar: Color,
    pub sidebar_hover: Color,
    pub background: Color,
    pub background_hover: Color,
    pub surface: Color,
    pub surface_hover: Color,

    // Borders (Step 7 & 8)
    pub border: Color,
    pub border_hover: Color,
    pub focus_ring: Color,

    // Text (Step 11 & 12)
    pub text: Color,
    pub text_muted: Color,

    // Typography
    pub font_family: Cow<'static, str>,
    pub font_size_sm: f32,
    pub font_size_base: f32,
    pub font_size_lg: f32,
}

impl Theme {
    pub fn from_oklch(
        is_dark: bool,
        accent_oklch: (f32, f32, f32),
        background_oklch: (f32, f32, f32),
    ) -> Self {
        let (la, ca, ha) = accent_oklch;
        let (_lb, cb, hb) = background_oklch;

        // UI 背景・枠線の微小なティント
        let bg_c = cb.min(0.012);

        let (l_bg, l_side, l_head, l_hover) = if is_dark {
            (0.1785, 0.2143, 0.2559, 0.2856)
        } else {
            (0.9521, 0.9401, 0.9850, 0.9097)
        };

        // Main Background
        let background = Color::oklch(l_bg, bg_c, hb);
        let background_hover = Color::oklch(l_hover, bg_c, hb);

        // Sidebar & Surface
        let sidebar = Color::oklch(l_side, bg_c, hb);
        let sidebar_hover = Color::oklch(l_hover, bg_c, hb);
        let surface = sidebar;
        let surface_hover = sidebar_hover;

        // Header
        let header = Color::oklch(l_head, bg_c * 0.8, hb);
        let header_hover = if is_dark {
            Color::oklch(l_head + 0.03, bg_c * 0.8, hb)
        } else {
            Color::oklch(l_head - 0.03, bg_c * 0.8, hb)
        };

        let (l_border, l_border_hover) = if is_dark {
            (0.4024, 0.4800)
        } else {
            (0.7952, 0.7000)
        };
        let border = Color::oklch(l_border, bg_c * 1.5, hb);
        let border_hover = Color::oklch(l_border_hover, bg_c * 1.5, hb);

        // Focus Ring
        let l_focus = if is_dark { la.max(0.48) } else { la.min(0.62) };
        let focus_ring = Color::oklch(l_focus, ca, ha);

        let (l_text, l_text_muted) = if is_dark {
            (0.9497, 0.7690)
        } else {
            (0.2435, 0.4459)
        };
        let text = Color::oklch(l_text, bg_c * 0.5, hb);
        let text_muted = Color::oklch(l_text_muted, bg_c * 0.8, hb);

        let accent = Color::oklch(la, ca, ha);
        let accent_hover = if is_dark {
            Color::oklch((la + 0.05).min(1.0), ca, ha)
        } else {
            Color::oklch((la - 0.05).max(0.0), ca, ha)
        };
        let text_on_accent = if la >= 0.65 {
            Color::oklch(0.20, 0.01, hb)
        } else {
            Color::WHITE
        };

        Self {
            is_dark,
            accent_oklch,
            background_oklch,
            accent,
            accent_hover,
            text_on_accent,
            header,
            header_hover,
            sidebar,
            sidebar_hover,
            background,
            background_hover,
            surface,
            surface_hover,
            border,
            border_hover,
            focus_ring,
            text,
            text_muted,
            font_family: "Segoe UI".into(),
            font_size_sm: 12.0,
            font_size_base: 14.0,
            font_size_lg: 18.0,
        }
    }

    /// Default Light Theme (`SimpleTheme1` 準拠)
    pub fn light() -> Self {
        Self::from_oklch(false, (0.65, 0.24, 328.0), (0.9521, 0.0041, 271.37))
    }

    /// Default Dark Theme (`SimpleTheme1` 準拠)
    pub fn dark() -> Self {
        Self::from_oklch(true, (0.68, 0.24, 328.0), (0.1785, 0.0041, 285.98))
    }

    #[must_use]
    pub fn with_accent_oklch(self, l: f32, c: f32, h: f32) -> Self {
        Self::from_oklch(self.is_dark, (l, c, h), self.background_oklch)
    }

    #[must_use]
    pub fn with_background_oklch(self, l: f32, c: f32, h: f32) -> Self {
        Self::from_oklch(self.is_dark, self.accent_oklch, (l, c, h))
    }
}
