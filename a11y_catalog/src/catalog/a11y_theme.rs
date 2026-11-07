mod simple_theme;

pub use simple_theme::*;

#[cfg(test)]
mod tests {
    use crate::catalog::a11y_theme::*;
    use michiu::prelude::Color;

    /// Calculates relative luminance according to WCAG 2.2 specifications.
    fn relative_luminance(color: Color) -> f32 {
        fn channel_luminance(c: f32) -> f32 {
            if c <= 0.04045 {
                c / 12.92
            } else {
                ((c + 0.055) / 1.055).powf(2.4)
            }
        }

        let r = channel_luminance(color.r);
        let g = channel_luminance(color.g);
        let b = channel_luminance(color.b);

        0.2126 * r + 0.7152 * g + 0.0722 * b
    }

    /// Calculates the contrast ratio between two colors: (L1 + 0.05) / (L2 + 0.05).
    fn contrast_ratio(c1: Color, c2: Color) -> f32 {
        let l1 = relative_luminance(c1);
        let l2 = relative_luminance(c2);
        let (lighter, darker) = if l1 > l2 { (l1, l2) } else { (l2, l1) };
        (lighter + 0.05) / (darker + 0.05)
    }

    struct PairCheck<'a> {
        fg_name: &'a str,
        bg_name: &'a str,
        light_fg: Color,
        light_bg: Color,
        dark_fg: Color,
        dark_bg: Color,
        target_ratio: f32,
        notes: &'a str,
    }

    #[test]
    fn generate_wcag_docs_markdown() {
        // =========================================
        let light = simple_theme::SimpleTheme1::light();
        let dark = simple_theme::SimpleTheme1::dark();
        let white = simple_theme::SimpleTheme1::rgb_single(255);
        // =========================================

        let text_pairs = vec![
            PairCheck {
                fg_name: "`text_high`",
                bg_name: "`bg_app` (Step 1)",
                light_fg: light.text_high,
                light_bg: light.bg_app,
                dark_fg: dark.text_high,
                dark_bg: dark.bg_app,
                target_ratio: 4.5,
                notes: "Body text & headings",
            },
            PairCheck {
                fg_name: "`text_low`",
                bg_name: "`bg_app` (Step 1)",
                light_fg: light.text_low,
                light_bg: light.bg_app,
                dark_fg: dark.text_low,
                dark_bg: dark.bg_app,
                target_ratio: 4.5,
                notes: "Subtle text / links on window background",
            },
            PairCheck {
                fg_name: "`text_low`",
                bg_name: "`bg_subtle` (Step 2)",
                light_fg: light.text_low,
                light_bg: light.bg_subtle,
                dark_fg: dark.text_low,
                dark_bg: dark.bg_subtle,
                target_ratio: 4.5,
                notes: "Secondary text on cards / containers",
            },
            PairCheck {
                fg_name: "`text_low`",
                bg_name: "`bg_ui` (Step 3)",
                light_fg: light.text_low,
                light_bg: light.bg_ui,
                dark_fg: dark.text_low,
                dark_bg: dark.bg_ui,
                target_ratio: 4.5,
                notes: "Button labels in normal state",
            },
            PairCheck {
                fg_name: "`text_low`",
                bg_name: "`bg_ui_hover` (Step 4)",
                light_fg: light.text_low,
                light_bg: light.bg_ui_hover,
                dark_fg: dark.text_low,
                dark_bg: dark.bg_ui_hover,
                target_ratio: 4.5,
                notes: "Button labels on hover",
            },
            PairCheck {
                fg_name: "`text_low`",
                bg_name: "`bg_ui_active` (Step 5)",
                light_fg: light.text_low,
                light_bg: light.bg_ui_active,
                dark_fg: dark.text_low,
                dark_bg: dark.bg_ui_active,
                target_ratio: 4.5,
                notes: "Button labels when pressed",
            },
            PairCheck {
                fg_name: "`White` (255)",
                bg_name: "`solid` (Step 9)",
                light_fg: white,
                light_bg: light.solid,
                dark_fg: white,
                dark_bg: dark.solid,
                target_ratio: 4.5,
                notes: "White text on primary button",
            },
        ];

        let non_text_pairs = vec![
            PairCheck {
                fg_name: "`focus_ring`",
                bg_name: "`bg_app` (Step 1)",
                light_fg: light.focus_ring,
                light_bg: light.bg_app,
                dark_fg: dark.focus_ring,
                dark_bg: dark.bg_app,
                target_ratio: 3.0,
                notes: "Focus indicator on app background",
            },
            PairCheck {
                fg_name: "`focus_ring`",
                bg_name: "`bg_subtle` (Step 2)",
                light_fg: light.focus_ring,
                light_bg: light.bg_subtle,
                dark_fg: dark.focus_ring,
                dark_bg: dark.bg_subtle,
                target_ratio: 3.0,
                notes: "Focus indicator on card background",
            },
            PairCheck {
                fg_name: "`focus_ring`",
                bg_name: "`bg_ui` (Step 3)",
                light_fg: light.focus_ring,
                light_bg: light.bg_ui,
                dark_fg: dark.focus_ring,
                dark_bg: dark.bg_ui,
                target_ratio: 3.0,
                notes: "Focus indicator on interactive elements",
            },
            PairCheck {
                fg_name: "`solid`",
                bg_name: "`bg_app` (Step 1)",
                light_fg: light.solid,
                light_bg: light.bg_app,
                dark_fg: dark.solid,
                dark_bg: dark.bg_app,
                target_ratio: 3.0,
                notes: "Primary button visual boundary",
            },
            PairCheck {
                fg_name: "`border`",
                bg_name: "`bg_app` (Step 1)",
                light_fg: light.border,
                light_bg: light.bg_app,
                dark_fg: dark.border,
                dark_bg: dark.bg_app,
                target_ratio: 3.0,
                notes: "Component outline / standalone border",
            },
        ];

        println!("\n=== COPY BELOW INTO RUSTDOC COMMENTS ===\n");
        println!("/// # Contrast Ratio Verification (WCAG 2.2 Level AA)");
        println!("///");
        println!(
            "/// ## 1. Text Contrast (Criterion 1.4.3: Normal Text >= 4.5:1, Large Text >= 3.0:1)"
        );
        println!("/// | Foreground | Background | Light Ratio | Dark Ratio | Status | Notes |");
        println!("/// | :--- | :--- | :---: | :---: | :---: | :--- |");

        for p in text_pairs {
            let r_light = contrast_ratio(p.light_fg, p.light_bg);
            let r_dark = contrast_ratio(p.dark_fg, p.dark_bg);
            let status = if r_light >= 7.0 && r_dark >= 7.0 {
                "**AAA Pass**"
            } else if r_light >= p.target_ratio && r_dark >= p.target_ratio {
                "**AA Pass**"
            } else {
                "**Fail / Warning**"
            };
            println!(
                "/// | {} | {} | {:.2}:1 | {:.2}:1 | {} | {} |",
                p.fg_name, p.bg_name, r_light, r_dark, status, p.notes
            );
        }

        println!("///");
        println!("/// ## 2. Non-Text & Focus Indicator Contrast (Criterion 1.4.11: >= 3.0:1)");
        println!(
            "/// | Foreground / Border | Background | Light Ratio | Dark Ratio | Status | Notes |"
        );
        println!("/// | :--- | :--- | :---: | :---: | :---: | :--- |");

        for p in non_text_pairs {
            let r_light = contrast_ratio(p.light_fg, p.light_bg);
            let r_dark = contrast_ratio(p.dark_fg, p.dark_bg);
            let status = if r_light >= p.target_ratio && r_dark >= p.target_ratio {
                "**AA Pass**"
            } else if r_dark >= p.target_ratio {
                "**Dark Only**"
            } else {
                "**Warning**"
            };
            println!(
                "/// | {} | {} | {:.2}:1 | {:.2}:1 | {} | {} |",
                p.fg_name, p.bg_name, r_light, r_dark, status, p.notes
            );
        }
        println!("\n=== END OF RUSTDOC COMMENTS ===\n");
    }
}
