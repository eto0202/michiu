#![expect(unused)]

use michiu::ui::Color;
use std::borrow::Cow;

#[derive(Debug, Clone, PartialEq)]
pub struct Theme {
    pub is_dark: bool,

    // Source HSL
    pub accent_hsl: (f32, f32, f32),
    pub background_hsl: (f32, f32, f32),

    // Accent Colors
    pub accent: Color,
    pub accent_hover: Color,
    pub text_on_accent: Color,

    // Layout Colors (Header > Sidebar > Main)
    pub header: Color,
    pub header_hover: Color,
    pub sidebar: Color,
    pub sidebar_hover: Color,
    pub background: Color,
    pub background_hover: Color,
    pub surface: Color,
    pub surface_hover: Color,

    pub border: Color,
    pub border_hover: Color,
    pub text: Color,
    pub text_muted: Color,

    pub font_family: Cow<'static, str>,
    pub font_size_sm: f32,
    pub font_size_base: f32,
    pub font_size_lg: f32,
}

impl Theme {
    pub fn from_hsl(
        is_dark: bool,
        accent_hsl: (f32, f32, f32),
        background_hsl: (f32, f32, f32),
    ) -> Self {
        let (ha, sa, la) = accent_hsl;
        let (hb, sb, lb) = background_hsl;

        // Accent
        let adjusted_la = if is_dark {
            la
        } else {
            (la - 5.0).clamp(0.0, 100.0)
        };
        let accent = Color::hsl(ha, sa, adjusted_la);
        let accent_hover = if is_dark {
            Color::hsl(ha, sa, (adjusted_la + 7.0).min(100.0))
        } else {
            Color::hsl(ha, sa, (adjusted_la - 7.0).max(0.0))
        };
        let text_on_accent = Color::WHITE;

        // Layout
        // Header > Sidebar / Surface > Background
        let (l_bg, l_side, l_head) = if is_dark {
            let bg = lb;
            let side = (lb + 4.5).min(100.0);
            let head = (lb + 9.0).min(100.0);
            (bg, side, head)
        } else {
            let bg = lb;
            let side = (lb + 5.0).min(90.0);
            let head = 98.0;
            (bg, side, head)
        };

        // Main Background
        let background = Color::hsl(hb, sb, l_bg);
        let background_hover = if is_dark {
            Color::hsl(hb, sb * 1.1, (l_bg + 2.0).min(100.0))
        } else {
            Color::hsl(hb, sb, (l_bg - 6.0).max(0.0))
        };

        // Sidebar
        let sidebar = Color::hsl(hb, sb * 0.95, l_side);
        let sidebar_hover = if is_dark {
            Color::hsl(hb, sb * 0.9, (l_side + 3.0).min(100.0))
        } else {
            Color::hsl(hb, sb * 0.95, (l_side - 5.0).max(0.0))
        };

        // Surface = Sidebar or Background
        let surface = sidebar;
        let surface_hover = sidebar_hover;

        // Header
        let header = Color::hsl(hb, sb * 0.9, l_head);
        let header_hover = if is_dark {
            Color::hsl(hb, sb * 0.9, (l_head + 3.0).min(100.0))
        } else {
            Color::hsl(hb, sb * 0.9, (l_head - 5.0).max(0.0))
        };

        // Border & Border Hover
        let border = if is_dark {
            Color::hsl(hb, sb * 0.7, (l_head + 6.0).min(100.0))
        } else {
            Color::hsl(hb, sb * 0.5, (l_bg - 18.0).max(0.0))
        };
        let border_hover = if is_dark {
            Color::hsl(hb, sb * 0.7, (l_head + 16.0).min(100.0))
        } else {
            Color::hsl(hb, sb * 0.5, (l_bg - 22.0).max(0.0))
        };

        // Text & Text Muted
        let text = if is_dark {
            Color::hsl(hb, sb * 0.15, 96.0)
        } else {
            Color::hsl(hb, sb * 0.25, 10.0)
        };
        let text_muted = if is_dark {
            Color::hsl(hb, sb * 0.2, 65.0)
        } else {
            Color::hsl(hb, sb * 0.25, 40.0)
        };

        Self {
            is_dark,
            accent_hsl,
            background_hsl,
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
            text,
            text_muted,
            font_family: "Segoe UI".into(),
            font_size_sm: 12.0,
            font_size_base: 14.0,
            font_size_lg: 18.0,
        }
    }

    /// Default Dark Theme
    pub fn dark() -> Self {
        Self::from_hsl(true, (318.0, 56.0, 59.0), (215.0, 6.0, 10.0))
    }

    /// Default Light Theme
    pub fn light() -> Self {
        Self::from_hsl(false, (318.0, 56.0, 59.0), (213.0, 6.0, 85.0))
    }

    #[must_use]
    pub fn with_accent_hsl(self, h: f32, s: f32, l: f32) -> Self {
        Self::from_hsl(self.is_dark, (h, s, l), self.background_hsl)
    }

    #[must_use]
    pub fn with_background_hsl(self, h: f32, s: f32, l: f32) -> Self {
        Self::from_hsl(self.is_dark, self.accent_hsl, (h, s, l))
    }
}
