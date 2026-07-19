use michiu_ui::prelude::*;
use std::borrow::Cow;

#[derive(Debug, Clone, PartialEq)]
pub struct Theme {
    pub is_dark: bool,

    // 生成基準となったHSL情報
    pub primary_hsl: (f32, f32, f32),
    pub background_hsl: (f32, f32, f32),

    // 動的算出されるカラーパレット
    pub primary: Color,
    pub primary_hover: Color,
    pub secondary: Color,
    pub secondary_hover: Color,
    pub background: Color,
    pub background_hover: Color,
    pub surface: Color,
    pub border: Color,
    pub border_hover: Color,
    pub text: Color,
    pub text_muted: Color,

    // フォント
    pub font_family: Cow<'static, str>,
    pub font_size_sm: f32,
    pub font_size_base: f32,
    pub font_size_lg: f32,
}

// Theme 実装箇所の修正

impl Theme {
    /// HSL の基本パラメータ（Hue: 0..360, Saturation: 0..100%, Lightness: 0..100%）を元に、
    /// モダンなトーン調和アルゴリズムに沿ってパレットを一括算出します。
    pub fn from_hsl(
        is_dark: bool,
        primary_hsl: (f32, f32, f32),
        background_hsl: (f32, f32, f32),
    ) -> Self {
        let (hp, sp, lp) = primary_hsl;
        let (hb, sb, lb) = background_hsl;

        // 1. Primary & Primary Hover
        let primary = hsl(hp, sp, lp);
        let primary_hover = if is_dark {
            hsl(hp, sp, (lp + 8.0).min(100.0)) // +0.08 ➔ +8.0%
        } else {
            hsl(hp, sp, (lp - 8.0).max(0.0)) // -0.08 ➔ -8.0%
        };

        // 2. Secondary (プライマリの色相を30度シフト、彩度を落として補助トーンを構成)
        let hs = (hp + 30.0) % 360.0;
        let ss = (sp * 0.65).clamp(0.0, 100.0);
        let ls = if is_dark {
            (lp * 0.9).clamp(40.0, 70.0) // 0.4..0.7 ➔ 40.0..70.0%
        } else {
            (lp * 1.1).clamp(30.0, 60.0) // 0.3..0.6 ➔ 30.0..60.0%
        };
        let secondary = hsl(hs, ss, ls);
        let secondary_hover = if is_dark {
            hsl(hs, ss, (ls + 8.0).min(100.0))
        } else {
            hsl(hs, ss, (ls - 8.0).max(0.0))
        };

        // 3. Background & Background Hover
        let background = hsl(hb, sb, lb);
        let background_hover = if is_dark {
            hsl(hb, sb, (lb + 4.0).min(100.0)) // +0.04 ➔ +4.0%
        } else {
            hsl(hb, sb, (lb - 5.0).max(0.0)) // -0.05 ➔ -5.0%
        };

        // 4. Surface (背景よりも手前に浮かび上がって見えるように明度を制御)
        let surface = if is_dark {
            hsl(hb, sb * 0.9, (lb + 5.0).min(100.0))
        } else {
            hsl(hb, sb * 0.5, (lb + 1.0).min(98.0)) // +0.01 ➔ +1.0%, 0.98 ➔ 98.0%
        };

        // 5. Border & Border Hover
        let border = if is_dark {
            hsl(hb, sb * 0.8, (lb + 15.0).min(100.0)) // 0.15 ➔ 15.0%
        } else {
            hsl(hb, sb * 0.8, (lb - 16.0).max(0.0)) // 0.16 ➔ 16.0%
        };
        let border_hover = if is_dark {
            hsl(hb, sb * 0.8, (lb + 25.0).min(100.0)) // 0.25 ➔ 25.0%
        } else {
            hsl(hb, sb * 0.8, (lb - 26.0).max(0.0)) // 0.26 ➔ 26.0%
        };

        // 6. Text (背景の環境色をわずかに帯びた快適な明暗コントラスト)
        let text = if is_dark {
            hsl(hb, sb * 0.2, 95.0) // 0.95 ➔ 95.0% (輝度95%のソフトな白)
        } else {
            hsl(hb, sb * 0.3, 12.0) // 0.12 ➔ 12.0% (輝度12%の引き締まったダーク炭色)
        };
        let text_muted = if is_dark {
            hsl(hb, sb * 0.25, 65.0) // 0.65 ➔ 65.0%
        } else {
            hsl(hb, sb * 0.25, 50.0) // 0.50 ➔ 50.0%
        };

        let font_family = "Segoe UI".into();
        let font_size_sm = 12.0;
        let font_size_base = 14.0;
        let font_size_lg = 18.0;

        Self {
            is_dark,
            primary_hsl,
            background_hsl,
            primary,
            primary_hover,
            secondary,
            secondary_hover,
            background,
            background_hover,
            surface,
            border,
            border_hover,
            text,
            text_muted,
            font_family,
            font_size_sm,
            font_size_base,
            font_size_lg,
        }
    }

    /// デフォルトのダークテーマ（S, L を % 単位で直接定義）
    pub fn dark() -> Self {
        Self::from_hsl(true, (318.0, 56.0, 59.0), (224.0, 15.0, 10.0))
    }

    /// デフォルトのライトテーマ（S, L を % 単位で直接定義）
    pub fn light() -> Self {
        Self::from_hsl(false, (318.0, 56.0, 59.0), (220.0, 10.0, 92.0))
    }

    /// プライマリの HSL 値を変更し、依存するカラーパレット全体を再算出します。
    #[allow(unused)]
    pub fn with_primary_hsl(self, h: f32, s: f32, l: f32) -> Self {
        Self::from_hsl(self.is_dark, (h, s, l), self.background_hsl)
    }

    /// バックグラウンドの HSL 値を変更し、依存するカラーパレット全体を再算出します。
    #[allow(unused)]
    pub fn with_background_hsl(self, h: f32, s: f32, l: f32) -> Self {
        Self::from_hsl(self.is_dark, self.primary_hsl, (h, s, l))
    }
}
