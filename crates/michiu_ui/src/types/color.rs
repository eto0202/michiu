#![expect(clippy::many_single_char_names)]

use bytemuck::{Pod, Zeroable};

#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Pod, Zeroable)]
pub struct Color {
    pub r: f32,
    pub g: f32,
    pub b: f32,
    pub a: f32,
}

impl Default for Color {
    fn default() -> Self {
        Color::TRANSPARENT
    }
}

impl Color {
    pub const TRANSPARENT: Self = Self {
        r: 0.0,
        g: 0.0,
        b: 0.0,
        a: 0.0,
    };
    pub const WHITE: Self = Self {
        r: 1.0,
        g: 1.0,
        b: 1.0,
        a: 1.0,
    };
    pub const BLACK: Self = Self {
        r: 0.0,
        g: 0.0,
        b: 0.0,
        a: 1.0,
    };

    pub const RED: Self = Self {
        r: 1.0,
        g: 0.0,
        b: 0.0,
        a: 1.0,
    };
    pub const GREEN: Self = Self {
        r: 0.0,
        g: 1.0,
        b: 0.0,
        a: 1.0,
    };
    pub const BLUE: Self = Self {
        r: 0.0,
        g: 0.0,
        b: 1.0,
        a: 1.0,
    };
    pub const YELLOW: Self = Self {
        r: 1.0,
        g: 1.0,
        b: 0.0,
        a: 1.0,
    };
    pub const ORANGE: Self = Self {
        r: 1.0,
        g: 0.5,
        b: 0.0,
        a: 1.0,
    };
    pub const PURPLE: Self = Self {
        r: 0.5,
        g: 0.0,
        b: 0.5,
        a: 1.0,
    };
    pub const CYAN: Self = Self {
        r: 0.0,
        g: 1.0,
        b: 1.0,
        a: 1.0,
    };
    pub const MAGENTA: Self = Self {
        r: 1.0,
        g: 0.0,
        b: 1.0,
        a: 1.0,
    };
    pub const GRAY: Self = Self {
        r: 0.5,
        g: 0.5,
        b: 0.5,
        a: 1.0,
    };
    pub const LIGHT_GRAY: Self = Self {
        r: 0.75,
        g: 0.75,
        b: 0.75,
        a: 1.0,
    };
    pub const DARK_GRAY: Self = Self {
        r: 0.25,
        g: 0.25,
        b: 0.25,
        a: 1.0,
    };

    /// Generates directly from 0.0–1.0 (f32) values for GPUs and shaders
    #[inline]
    #[must_use]
    pub const fn rgb_f32(r: f32, g: f32, b: f32) -> Self {
        Self { r, g, b, a: 1.0 }
    }

    /// Generates directly from 0.0–1.0 (f32) values for GPUs and shaders
    #[inline]
    #[must_use]
    pub const fn rgba_f32(r: f32, g: f32, b: f32, a: f32) -> Self {
        Self { r, g, b, a }
    }

    /// Change the opacity (0.0..=1.0) (values outside this range are automatically clamped)
    #[inline]
    #[must_use]
    pub const fn with_alpha(self, a: f32) -> Self {
        let a = if a < 0.0 {
            0.0
        } else if a > 1.0 {
            1.0
        } else {
            a
        };
        Self { a, ..self }
    }

    /// Multiply the current alpha value by the coefficient.
    #[inline]
    #[must_use]
    pub const fn multiply_alpha(self, factor: f32) -> Self {
        self.with_alpha(self.a * factor)
    }

    /// Change only the red
    #[inline]
    #[must_use]
    pub const fn with_r(self, r: f32) -> Self {
        let r = if r < 0.0 {
            0.0
        } else if r > 1.0 {
            1.0
        } else {
            r
        };
        Self { r, ..self }
    }

    /// Change only the green
    #[inline]
    #[must_use]
    pub const fn with_g(self, g: f32) -> Self {
        let g = if g < 0.0 {
            0.0
        } else if g > 1.0 {
            1.0
        } else {
            g
        };
        Self { g, ..self }
    }

    /// Change only the blue
    #[inline]
    #[must_use]
    pub const fn with_b(self, b: f32) -> Self {
        let b = if b < 0.0 {
            0.0
        } else if b > 1.0 {
            1.0
        } else {
            b
        };
        Self { b, ..self }
    }

    #[inline]
    #[must_use]
    #[expect(clippy::cast_sign_loss, clippy::cast_possible_truncation)]
    pub fn to_rgba8(&self) -> (u8, u8, u8, f32) {
        let f = |v: f32| (v.clamp(0.0, 1.0) * 255.0).round() as u8;
        (f(self.r), f(self.g), f(self.b), self.a)
    }

    #[inline]
    #[must_use]
    #[expect(clippy::cast_lossless)]
    pub fn to_hex(&self) -> u32 {
        let (r, g, b, _) = self.to_rgba8();
        ((r as u32) << 16) | ((g as u32) << 8) | (b as u32)
    }

    #[inline]
    #[must_use]
    #[expect(clippy::cast_sign_loss, clippy::cast_possible_truncation)]
    pub fn to_hex_string(&self) -> String {
        let (r, g, b, a) = self.to_rgba8();
        // ここでのみ一時的に f32 -> u8 変換を行う
        let a = (a.clamp(0.0, 1.0) * 255.0).round() as u8;
        format!("{r:02x}{g:02x}{b:02x}{a:02x}") // 例: "808080ff"
    }

    /// Generates a [`Color`] from the HSL (Hue: 0..360, Saturation: 0..100%, Lightness: 0..100%).
    #[inline]
    #[must_use]
    pub fn hsl(h: f32, s: f32, l: f32) -> Self {
        Self::hsla(h, s, l, 1.0)
    }

    /// Generate a [`Color`] by applying an alpha value (0.0..1.0) to HSL
    #[must_use]
    pub fn hsla(h: f32, s: f32, l: f32, a: f32) -> Self {
        // 色相（h）を 0..360 の範囲に正規化
        let h_mod = (h % 360.0 + 360.0) % 360.0;

        // s, l を 0.0..100.0 (%) から 0.0..1.0 の比率へ安全変換
        let s = (s / 100.0).clamp(0.0, 1.0);
        let l = (l / 100.0).clamp(0.0, 1.0);

        let c = (1.0 - (2.0 * l - 1.0).abs()) * s;
        let x = c * (1.0 - ((h_mod / 60.0) % 2.0 - 1.0).abs());
        let m = l - c / 2.0;

        let (r, g, b) = if h_mod < 60.0 {
            (c, x, 0.0)
        } else if h_mod < 120.0 {
            (x, c, 0.0)
        } else if h_mod < 180.0 {
            (0.0, c, x)
        } else if h_mod < 240.0 {
            (0.0, x, c)
        } else if h_mod < 300.0 {
            (x, 0.0, c)
        } else {
            (c, 0.0, x)
        };

        Self {
            r: r + m,
            g: g + m,
            b: b + m,
            a,
        }
    }

    /// HSL (Hue: 0..360, Saturation: 0..100%, Lightness: 0..100%, alpha: 0.0..1.0)
    #[inline]
    #[must_use]
    #[expect(clippy::float_cmp)]
    pub fn to_hsla(self) -> (f32, f32, f32, f32) {
        let r = self.r.clamp(0.0, 1.0);
        let g = self.g.clamp(0.0, 1.0);
        let b = self.b.clamp(0.0, 1.0);

        let max = r.max(g).max(b);
        let min = r.min(g).min(b);
        let delta = max - min;

        // Lightness (0.0 ~ 1.0)
        let l = f32::midpoint(max, min);

        // グレースケールの場合
        if delta == 0.0 {
            // 色相・彩度は 0
            return (0.0, 0.0, l * 100.0, self.a);
        }

        // Saturation (0.0 ~ 1.0)
        let s = if l > 0.5 {
            delta / (2.0 - max - min)
        } else {
            delta / (max + min)
        };

        // Hue (0.0 ~ 360.0)
        let mut h = if max == r {
            let mut segment = (g - b) / delta;
            if g < b {
                segment += 6.0;
            }
            segment
        } else if max == g {
            ((b - r) / delta) + 2.0
        } else {
            ((r - g) / delta) + 4.0
        };

        h *= 60.0; // 0.0 ~ 360.0 にスケール

        // (h: 0..360, s: 0..100%, l: 0..100%, a: そのまま)
        (h, s * 100.0, l * 100.0, self.a)
    }

    /// HSL (Hue: 0..360, Saturation: 0..100%, Lightness: 0..100%)
    #[inline]
    #[must_use]
    pub fn to_hsl(self) -> (f32, f32, f32) {
        let (h, s, l, _) = self.to_hsla();
        (h, s, l)
    }

    /// Generate from Oklab (L: 0.0..1.0, a: -0.4..0.4, b: -0.4..0.4)
    #[inline]
    #[must_use]
    pub fn oklab(l: f32, a: f32, b: f32) -> Self {
        Self::oklaba(l, a, b, 1.0)
    }

    /// Generate it by adding an alpha value to Oklab
    #[must_use]
    pub fn oklaba(l: f32, a: f32, b: f32, alpha: f32) -> Self {
        // Oklab -> LMS
        let l_ = l + 0.396_337_78 * a + 0.215_803_76 * b;
        let m_ = l - 0.105_561_346 * a - 0.063_854_17 * b;
        let s_ = l - 0.089_484_18 * a - 1.291_485_5 * b;

        let l_cubed = l_ * l_ * l_;
        let m_cubed = m_ * m_ * m_;
        let s_cubed = s_ * s_ * s_;

        // LMS -> Linear sRGB
        let r_linear = 4.076_743_4 * l_cubed - 3.307_711_6 * m_cubed + 0.230_969_93 * s_cubed;
        let g_linear = -1.268_438 * l_cubed + 2.609_757_4 * m_cubed - 0.341_319_4 * s_cubed;
        let b_linear = -0.004_196_086_3 * l_cubed - 0.703_418_6 * m_cubed + 1.707_614_7 * s_cubed;

        // Linear sRGB -> sRGB (ガンマ補正)
        Self {
            r: Self::linear_to_srgb(r_linear),
            g: Self::linear_to_srgb(g_linear),
            b: Self::linear_to_srgb(b_linear),
            a: alpha,
        }
    }

    /// Oklaba (L: 0.0..1.0, a: -0.4..0.4, b: -0.4..0.4, alpha: f32)
    #[inline]
    #[must_use]
    pub fn to_oklaba(self) -> (f32, f32, f32, f32) {
        // ガンマ解除
        let r_lin = Self::srgb_to_linear(self.r);
        let g_lin = Self::srgb_to_linear(self.g);
        let b_lin = Self::srgb_to_linear(self.b);

        // Linear sRGB -> LMS
        let l = 0.412_221_46 * r_lin + 0.536_332_55 * g_lin + 0.051_445_993 * b_lin;
        let m = 0.211_903_5 * r_lin + 0.680_699_5 * g_lin + 0.107_396_96 * b_lin;
        let s = 0.088_302_46 * r_lin + 0.281_718_84 * g_lin + 0.629_978_7 * b_lin;

        let l_ = l.cbrt();
        let m_ = m.cbrt();
        let s_ = s.cbrt();

        // LMS -> Oklab
        let ok_l = 0.210_454_26 * l_ + 0.793_617_8 * m_ - 0.004_072_047 * s_;
        let ok_a = 1.977_998_5 * l_ - 2.428_592_2 * m_ + 0.450_593_7 * s_;
        let ok_b = 0.025_904_037 * l_ + 0.782_771_77 * m_ - 0.808_675_77 * s_;

        (ok_l, ok_a, ok_b, self.a)
    }

    /// Oklab (L: 0.0..1.0, a: -0.4..0.4, b: -0.4..0.4)
    #[inline]
    #[must_use]
    pub fn to_oklab(self) -> (f32, f32, f32) {
        let (l, a, b, _) = self.to_oklaba();
        (l, a, b)
    }

    /// Generate from Oklch (L: 0.0..1.0, Chroma: 0.0..0.4+, Hue: 0..360°)
    #[inline]
    #[must_use]
    pub fn oklch(l: f32, c: f32, h: f32) -> Self {
        Self::oklcha(l, c, h, 1.0)
    }

    /// Generate it by adding an alpha value to Oklch
    #[inline]
    #[must_use]
    pub fn oklcha(l: f32, c: f32, h: f32, alpha: f32) -> Self {
        let h_rad = h.to_radians();
        let a = c * h_rad.cos();
        let b = c * h_rad.sin();
        Self::oklaba(l, a, b, alpha)
    }

    /// Converts sRGB to Oklcha (Lightness: 0.0..=1.0, Chroma: 0.0.., Hue: 0.0..=360.0, alpha: 0.0..=1.0)
    #[inline]
    #[must_use]
    pub fn to_oklcha(self) -> (f32, f32, f32, f32) {
        // 先に Oklab を取得
        let (l, a, b, alpha) = self.to_oklaba();

        // Chroma (彩度): a と b のユークリッド距離
        let c = a.hypot(b);

        // Hue (色相): atan2(b, a) から角度を計算
        // 微小な浮動小数点誤差がある場合は、彩度・色相ともに完全な 0.0 に倒す
        let (c, h) = if c < 1e-6 {
            (0.0, 0.0)
        } else {
            let mut h = b.atan2(a).to_degrees();
            if h < 0.0 {
                // 角度がマイナスの場合は 0..360 の範囲に補正
                h += 360.0;
            }
            (c, h)
        };

        (l, c, h, alpha)
    }

    /// Converts sRGB to Oklch (Lightness: 0.0..=1.0, Chroma: 0.0.., Hue: 0.0..=360.0)
    #[inline]
    #[must_use]
    pub fn to_oklch(self) -> (f32, f32, f32) {
        let (l, c, h, _) = self.to_oklcha();
        (l, c, h)
    }

    /// Replace the Oklch brightness value (0.0..=1.0)
    #[inline]
    #[must_use]
    pub fn with_lightness(self, lightness: f32) -> Self {
        if lightness >= 1.0 {
            return Self::WHITE.with_alpha(self.a);
        }
        if lightness <= 0.0 {
            return Self::BLACK.with_alpha(self.a);
        }

        let (_, c, h) = self.to_oklch();
        Self::oklcha(lightness, c, h, self.a)
    }

    /// Replace the chroma value (Chroma: 0.0..) for Oklch
    #[inline]
    #[must_use]
    pub fn with_chroma(self, chroma: f32) -> Self {
        let (l, _, h) = self.to_oklch();
        Self::oklcha(l, chroma.max(0.0), h, self.a)
    }

    /// Replace the hue (Hue: 0.0..=360.0) of Oklch
    #[inline]
    #[must_use]
    pub fn with_hue(self, hue: f32) -> Self {
        let (l, c, _) = self.to_oklch();
        Self::oklcha(l, c, hue, self.a)
    }

    /// Rotates the Oklch hue by the specified angle (in degrees).
    #[inline]
    #[must_use]
    pub fn rotate_hue(self, degrees: f32) -> Self {
        let (l, c, h) = self.to_oklch();
        Self::oklcha(l, c, h + degrees, self.a)
    }

    /// Lighten using Oklch as the base
    #[inline]
    #[must_use]
    pub fn lighten(self, amount: f32) -> Self {
        let (l, c, h) = self.to_oklch();
        let new_l = l + amount;
        if new_l >= 1.0 {
            Self::WHITE.with_alpha(self.a)
        } else if new_l <= 0.0 {
            Self::BLACK.with_alpha(self.a)
        } else {
            Self::oklcha(new_l, c, h, self.a)
        }
    }

    /// Darken using Oklch as the base
    #[inline]
    #[must_use]
    pub fn darken(self, amount: f32) -> Self {
        self.lighten(-amount)
    }

    /// Increase the saturation using the Oklch base
    #[inline]
    #[must_use]
    pub fn saturate(self, amount: f32) -> Self {
        let (l, c, h) = self.to_oklch();
        Self::oklcha(l, (c + amount).max(0.0), h, self.a)
    }

    /// Use the Oklch base to reduce saturation (the closer to 0.0, the more monochrome it becomes)
    #[inline]
    #[must_use]
    pub fn desaturate(self, amount: f32) -> Self {
        self.saturate(-amount)
    }

    /// Naturally interpolate between two colors in Oklab space (t: 0.0..=1.0)
    #[inline]
    #[must_use]
    pub fn lerp(self, other: Self, t: f32) -> Self {
        let t = t.clamp(0.0, 1.0);
        let (l1, a1, b1, alpha1) = self.to_oklaba();
        let (l2, a2, b2, alpha2) = other.to_oklaba();

        Self::oklaba(
            l1 + (l2 - l1) * t,
            a1 + (a2 - a1) * t,
            b1 + (b2 - b1) * t,
            alpha1 + (alpha2 - alpha1) * t,
        )
    }

    /// Pass `self` to the closure to perform custom color changes inline.
    ///
    /// # Example
    /// ```
    /// # use michiu_ui::Color;
    /// # struct Theme { color: Color }
    /// # let theme = Theme { color: Color::RED };
    /// let custom = theme.color.map(|c| {
    ///     let (l, chroma, h) = c.to_oklch();
    ///     Color::oklcha(l * 0.9, chroma + 0.05, h + 15.0, c.a)
    /// });
    /// ```
    #[inline]
    #[must_use]
    pub fn map<F>(self, f: F) -> Self
    where
        F: FnOnce(Self) -> Self,
    {
        f(self)
    }

    /// Convert to any type (such as tuples, Vec4, or types from other crates)
    ///
    /// # Example
    /// ```
    /// # use michiu_ui::Color;
    /// let col = Color::rgba_f32(1.0, 0.5, 0.25, 1.0);
    /// let array: [f32; 4] = col.map_into(|c| [c.r, c.g, c.b, c.a]);
    /// let expected = [1.0, 0.5, 0.25, 1.0];
    ///
    /// assert_eq!(array.len(), expected.len());
    /// ```
    #[inline]
    #[must_use]
    pub fn map_into<F, R>(self, f: F) -> R
    where
        F: FnOnce(Self) -> R,
    {
        f(self)
    }

    /// Convert Linear sRGB to Standard sRGB
    #[inline]
    #[must_use]
    pub fn linear_to_srgb(x: f32) -> f32 {
        let clamped = x.clamp(0.0, 1.0);
        if clamped <= 0.003_130_8 {
            12.92 * clamped
        } else {
            1.055 * clamped.powf(1.0 / 2.4) - 0.055
        }
    }

    /// Remove the sRGB gamma and convert to Linear sRGB
    #[inline]
    #[must_use]
    pub fn srgb_to_linear(x: f32) -> f32 {
        let clamped = x.clamp(0.0, 1.0);
        if clamped <= 0.04045 {
            clamped / 12.92
        } else {
            ((clamped + 0.055) / 1.055).powf(2.4)
        }
    }
}

pub trait IntoHexColor {
    fn into_hex_color(self) -> Color;
}

impl IntoHexColor for &str {
    #[inline]
    fn into_hex_color(self) -> Color {
        let s = self.trim_start_matches('#').trim_start_matches("0x");
        if let Ok(num) = u32::from_str_radix(s, 16) {
            parse_u32_to_color(num, s.len() > 6)
        } else {
            Color::TRANSPARENT
        }
    }
}

impl IntoHexColor for String {
    #[inline]
    fn into_hex_color(self) -> Color {
        self.as_str().into_hex_color()
    }
}

impl IntoHexColor for u32 {
    #[inline]
    fn into_hex_color(self) -> Color {
        parse_u32_to_color(self, self > 0xFF_FFFF)
    }
}

#[inline]
fn parse_u32_to_color(num: u32, is_8digit: bool) -> Color {
    if is_8digit {
        // 0xRRGGBBAA を [R, G, B, A] の [u8; 4] に分解
        let [r, g, b, a] = num.to_be_bytes();
        Color {
            r: f32::from(r) / 255.0,
            g: f32::from(g) / 255.0,
            b: f32::from(b) / 255.0,
            a: f32::from(a) / 255.0,
        }
    } else {
        // 先頭はダミーの 0
        let [_, r, g, b] = num.to_be_bytes();
        Color {
            r: f32::from(r) / 255.0,
            g: f32::from(g) / 255.0,
            b: f32::from(b) / 255.0,
            a: 1.0,
        }
    }
}

#[cfg(test)]
#[expect(clippy::float_cmp)]
mod tests {
    use super::*;

    const EPS: f32 = 1e-3; // Oklab変換の浮動小数点計算用に少しだけ余裕を持たせる

    // 浮動小数点誤差を許容して Color を比較するヘルパー
    fn assert_color_approx_eq(actual: Color, expected: Color) {
        assert!(
            (actual.r - expected.r).abs() < EPS
                && (actual.g - expected.g).abs() < EPS
                && (actual.b - expected.b).abs() < EPS
                && (actual.a - expected.a).abs() < EPS,
            "Color mismatch!\n  actual:   {actual:?}\n  expected: {expected:?}"
        );
    }

    // 基本的な関数のテスト
    #[test]
    fn test_rgb_rgba_with_alpha() {
        let col = Color::rgb_f32(0.2, 0.4, 0.6);
        assert_color_approx_eq(
            col,
            Color {
                r: 0.2,
                g: 0.4,
                b: 0.6,
                a: 1.0,
            },
        );

        let col_alpha = Color::rgba_f32(0.2, 0.4, 0.6, 0.8);
        assert_color_approx_eq(
            col_alpha,
            Color {
                r: 0.2,
                g: 0.4,
                b: 0.6,
                a: 0.8,
            },
        );

        let modified = col.with_alpha(0.5);
        assert_color_approx_eq(
            modified,
            Color {
                r: 0.2,
                g: 0.4,
                b: 0.6,
                a: 0.5,
            },
        );
    }

    // HSL / HSLA 変換のテスト
    #[test]
    fn test_hsl_basic_colors() {
        // 赤: 0°, 100%, 50%
        assert_color_approx_eq(Color::hsl(0.0, 100.0, 50.0), Color::RED);
        // 緑: 120°, 100%, 50%
        assert_color_approx_eq(Color::hsl(120.0, 100.0, 50.0), Color::GREEN);
        // 青: 240°, 100%, 50%
        assert_color_approx_eq(Color::hsl(240.0, 100.0, 50.0), Color::BLUE);
        // 白: 任意°, 0%, 100%
        assert_color_approx_eq(Color::hsl(0.0, 0.0, 100.0), Color::WHITE);
        // 黒: 任意°, 0%, 0%
        assert_color_approx_eq(Color::hsl(0.0, 0.0, 0.0), Color::BLACK);
    }

    #[test]
    fn test_hsl_edge_cases() {
        // 色相の 360° 周回 (360° -> 0°, -120° -> 240°)
        assert_color_approx_eq(Color::hsl(360.0, 100.0, 50.0), Color::RED);
        assert_color_approx_eq(Color::hsl(720.0, 100.0, 50.0), Color::RED);
        assert_color_approx_eq(Color::hsl(-120.0, 100.0, 50.0), Color::BLUE);

        // 彩度・輝度の clamp (120% や -10% の安全処理)
        assert_color_approx_eq(Color::hsl(0.0, 150.0, 50.0), Color::RED);
        assert_color_approx_eq(Color::hsl(0.0, 100.0, -10.0), Color::BLACK);

        // アルファ値つき HSLA
        let hsla = Color::hsla(0.0, 100.0, 50.0, 0.5);
        assert_color_approx_eq(hsla, Color::RED.with_alpha(0.5));
    }

    // &str, String からの変換テスト
    #[test]
    fn test_hex_str_parsing() {
        // # 付き 6桁
        assert_color_approx_eq("#ff0000".into_hex_color(), Color::RED);
        assert_color_approx_eq("#00ff00".into_hex_color(), Color::GREEN);
        assert_color_approx_eq("#0000ff".into_hex_color(), Color::BLUE);

        // 0x プレフィックス
        assert_color_approx_eq("0xff0000".into_hex_color(), Color::RED);

        // プレフィックスなし
        assert_color_approx_eq("ffffff".into_hex_color(), Color::WHITE);

        // 8桁 (RGBA)
        let half_red = "#ff000080".into_hex_color();
        assert_color_approx_eq(
            half_red,
            Color {
                r: 1.0,
                g: 0.0,
                b: 0.0,
                a: 128.0 / 255.0,
            },
        );

        // String 型からの変換
        let s = String::from("#00ffff");
        assert_color_approx_eq(s.into_hex_color(), Color::CYAN);

        // パース失敗時は TRANSPARENT
        assert_color_approx_eq("invalid_color".into_hex_color(), Color::TRANSPARENT);
    }

    // u32 値からの変換テスト
    #[test]
    fn test_hex_u32_parsing() {
        // 6桁相当 (0xRRGGBB)
        assert_color_approx_eq(0x00FF_0000.into_hex_color(), Color::RED);
        assert_color_approx_eq(0x0000_FF00.into_hex_color(), Color::GREEN);
        assert_color_approx_eq(0x0000_00FF.into_hex_color(), Color::BLUE);

        // 8桁相当 (0xRRGGBBAA) -> 0xFF_FFFF より大きい値
        let red_with_alpha = 0xFF00_0080_u32.into_hex_color();
        assert_color_approx_eq(
            red_with_alpha,
            Color {
                r: 1.0,
                g: 0.0,
                b: 0.0,
                a: 128.0 / 255.0,
            },
        );
    }

    #[test]
    fn test_oklab_black_and_white() {
        // L=0 は黒
        assert_color_approx_eq(Color::oklab(0.0, 0.0, 0.0), Color::BLACK);
        // L=1 は白
        assert_color_approx_eq(Color::oklab(1.0, 0.0, 0.0), Color::WHITE);
    }

    #[test]
    fn test_oklab_primary_colors() {
        // sRGB の代表色の Oklab 近似値による逆変換テスト
        // 赤: sRGB(1, 0, 0) -> Oklab(0.627955, 0.224863, 0.125846)
        assert_color_approx_eq(Color::oklab(0.627_955, 0.224_863, 0.125_846), Color::RED);
        // 緑: sRGB(0, 1, 0) -> Oklab(0.866440, -0.233887, 0.179498)
        assert_color_approx_eq(Color::oklab(0.866_440, -0.233_887, 0.179_498), Color::GREEN);
        // 青: sRGB(0, 0, 1) -> Oklab(0.452014, -0.032457, -0.311528)
        assert_color_approx_eq(Color::oklab(0.452_014, -0.032_457, -0.311_528), Color::BLUE);
    }

    #[test]
    fn test_oklch_creation() {
        // 無彩色 (Chroma=0) は白黒グレーになる
        assert_color_approx_eq(Color::oklch(0.0, 0.0, 0.0), Color::BLACK);
        assert_color_approx_eq(Color::oklch(1.0, 0.0, 0.0), Color::WHITE);

        // 赤 (Chroma ≈ 0.25768, Hue ≈ 29.23°)
        assert_color_approx_eq(Color::oklch(0.627_955, 0.257_683, 29.233_885), Color::RED);

        // アルファ付き Oklcha
        let half_red = Color::oklcha(0.627_955, 0.257_683, 29.233_885, 0.5);
        assert_color_approx_eq(half_red, Color::RED.with_alpha(0.5));
    }

    #[test]
    fn test_with_alpha_clamping() {
        let col = Color::WHITE;

        // 通常の変更
        assert!((0.5 - col.with_alpha(0.5).a).abs() < EPS);

        // マイナス値は 0.0 にクランプ
        assert!((0.0 - col.with_alpha(-0.5).a).abs() < EPS);

        // 1.0 超過は 1.0 にクランプ
        assert!((1.0 - col.with_alpha(1.5).a).abs() < EPS);
    }

    #[test]
    fn test_multiply_alpha() {
        let col = Color::WHITE.with_alpha(0.8);
        let semi = col.multiply_alpha(0.5);
        assert!((semi.a - 0.4).abs() < 1e-6);
    }

    #[test]
    fn test_with_rgb_components() {
        let base = Color::rgb_f32(0.1, 0.2, 0.3);

        let modified_r = base.with_r(0.9);
        assert_eq!(modified_r, Color::rgba_f32(0.9, 0.2, 0.3, 1.0));

        let modified_g = base.with_g(0.8);
        assert_eq!(modified_g, Color::rgba_f32(0.1, 0.8, 0.3, 1.0));

        let modified_b = base.with_b(0.7);
        assert_eq!(modified_b, Color::rgba_f32(0.1, 0.2, 0.7, 1.0));
    }

    // 往復変換（Round-trip）テスト：変換しても元の色に戻るか
    #[test]
    fn test_oklch_roundtrip() {
        let test_colors = [
            Color::WHITE,
            Color::BLACK,
            Color::RED,
            Color::GREEN,
            Color::BLUE,
            Color::YELLOW,
            Color::CYAN,
            Color::MAGENTA,
        ];

        for col in test_colors {
            let (l, c, h) = col.to_oklch();
            let roundtripped = Color::oklcha(l, c, h, col.a);
            assert_color_approx_eq(roundtripped, col);
        }
    }

    // lighten / darken のテスト
    #[test]
    fn test_lighten_darken() {
        let base = Color::RED;
        let (original_l, _, _) = base.to_oklch();

        // lighten で明度が増える
        let brighter = base.lighten(0.1);
        let (new_l, _, _) = brighter.to_oklch();
        assert!(new_l > original_l);

        // darken で明度が下がる
        let darker = base.darken(0.1);
        let (dark_l, _, _) = darker.to_oklch();
        assert!(dark_l < original_l);

        // 1.0 を超えても安全
        let max_bright = base.lighten(2.0);
        assert_color_approx_eq(max_bright, Color::WHITE);
    }

    // desaturate のテスト（彩度をゼロにすると完全な白黒グレーになる）
    #[test]
    fn test_desaturate_to_grayscale() {
        let red = Color::RED;
        let gray = red.desaturate(1.0); // 完全に脱色

        // グレースケールは R ≈ G ≈ B になる
        assert!((gray.r - gray.g).abs() < EPS);
        assert!((gray.g - gray.b).abs() < EPS);
    }

    // 色相回転 (rotate_hue) のテスト
    #[test]
    fn test_rotate_hue() {
        let col = Color::RED;
        // 360度回したら元の色に戻る
        let rotated_full = col.rotate_hue(360.0);
        assert_color_approx_eq(rotated_full, col);

        // 720度回しても同じ
        let rotated_double = col.rotate_hue(720.0);
        assert_color_approx_eq(rotated_double, col);
    }

    #[test]
    fn test_map_inline_modification() {
        let base = Color::RED;

        // 基本的なインライン変更
        let modified = base.map(|c| {
            // ライブラリに無い独自の計算（例: Gチャンネルを少し混ぜる等）
            Color { g: 0.5, ..c }
        });

        assert_color_approx_eq(
            modified,
            Color {
                r: 1.0,
                g: 0.5,
                b: 0.0,
                a: 1.0,
            },
        );
    }

    #[test]
    fn test_map_capturing_environment() {
        let theme_accent = Color::BLUE;
        let opacity_factor = 0.5;

        // 変数をキャプチャして計算
        let result = theme_accent.map(|c| c.with_alpha(c.a * opacity_factor));

        assert_color_approx_eq(result, Color::BLUE.with_alpha(0.5));
    }

    #[test]
    fn test_map_method_chaining() {
        // ビルダーチェーンの中でスムーズに連結できるか
        let final_color = Color::BLACK
            .map(|c| c.with_r(0.2))
            .lighten(0.1)
            .map(|c| c.with_b(0.8))
            .with_alpha(0.9);

        assert!((0.9 - final_color.a).abs() < 1e-5);
        assert!(final_color.b > 0.7);
    }

    #[test]
    fn test_transform_to_other_types() {
        // Color から別な型（GPU用配列やタプル等）へその場で変換できるか
        let col = Color::rgba_f32(1.0, 0.5, 0.25, 1.0);

        let array: [f32; 4] = col.map_into(|c| [c.r, c.g, c.b, c.a]);
        let expected = [1.0, 0.5, 0.25, 1.0];

        // 長さが一致しているか確認
        assert_eq!(array.len(), expected.len());

        // 各要素をループして誤差の範囲内かチェック
        for (idx, (actual, exp)) in array.iter().zip(expected.iter()).enumerate() {
            assert!(
                (actual - exp).abs() < EPS,
                "Mismatch!: idx: {idx}: actual: {actual}, expected: {exp}"
            );
        }

        let is_bright = col.map_into(|c| c.r > 0.5);
        assert!(is_bright);
    }

    #[test]
    fn test_to_rgba8() {
        // 代表色の変換確認
        assert_eq!(Color::RED.to_rgba8(), (255, 0, 0, 1.0));
        assert_eq!(Color::GREEN.to_rgba8(), (0, 255, 0, 1.0));
        assert_eq!(Color::BLUE.to_rgba8(), (0, 0, 255, 1.0));
        assert_eq!(Color::BLACK.to_rgba8(), (0, 0, 0, 1.0));
        assert_eq!(Color::WHITE.to_rgba8(), (255, 255, 255, 1.0));

        // 0.5 は四捨五入で 128
        assert_eq!(Color::GRAY.to_rgba8(), (128, 128, 128, 1.0));

        // a (f32) がそのまま維持されることの確認
        let semi_red = Color::RED.with_alpha(0.42);
        assert_eq!(semi_red.to_rgba8(), (255, 0, 0, 0.42));

        // 0.0 未満および 1.0 超過の値が安全に clamp されるか
        let out_of_range = Color {
            r: -0.5,
            g: 1.5,
            b: 0.5,
            a: 2.0,
        };
        assert_eq!(out_of_range.to_rgba8(), (0, 255, 128, 2.0));
    }

    #[test]
    fn test_to_hex() {
        // 0xRRGGBB の数値 (u32) への変換
        assert_eq!(Color::RED.to_hex(), 0x00FF_0000);
        assert_eq!(Color::GREEN.to_hex(), 0x0000_FF00);
        assert_eq!(Color::BLUE.to_hex(), 0x0000_00FF);
        assert_eq!(Color::WHITE.to_hex(), 0x00FF_FFFF);
        assert_eq!(Color::BLACK.to_hex(), 0x0000_0000);
        assert_eq!(Color::GRAY.to_hex(), 0x0080_8080);
        assert_eq!(Color::YELLOW.to_hex(), 0x00FF_FF00);
        assert_eq!(Color::CYAN.to_hex(), 0x0000_FFFF);
        assert_eq!(Color::MAGENTA.to_hex(), 0x00FF_00FF);
    }

    #[test]
    fn test_to_hex_string() {
        // "rrggbbaa" (8桁小文字) 文字列への変換
        assert_eq!(Color::RED.to_hex_string(), "ff0000ff");
        assert_eq!(Color::GREEN.to_hex_string(), "00ff00ff");
        assert_eq!(Color::BLUE.to_hex_string(), "0000ffff");
        assert_eq!(Color::WHITE.to_hex_string(), "ffffffff");
        assert_eq!(Color::BLACK.to_hex_string(), "000000ff");
        assert_eq!(Color::TRANSPARENT.to_hex_string(), "00000000");

        // 半透明 (a: 0.5 -> "80")
        let semi_gray = Color::GRAY.with_alpha(0.5);
        assert_eq!(semi_gray.to_hex_string(), "80808080");
    }

    #[test]
    fn test_to_hsla_and_hsl() {
        // 1. 基本色の確認
        let (h, s, l, a) = Color::RED.to_hsla();
        assert!((h - 0.0).abs() < EPS);
        assert!((s - 100.0).abs() < EPS);
        assert!((l - 50.0).abs() < EPS);
        assert_eq!(a, 1.0);

        let (h, s, l, _) = Color::GREEN.to_hsla();
        assert!((h - 120.0).abs() < EPS);
        assert!((s - 100.0).abs() < EPS);
        assert!((l - 50.0).abs() < EPS);

        let (h, s, l, _) = Color::BLUE.to_hsla();
        assert!((h - 240.0).abs() < EPS);
        assert!((s - 100.0).abs() < EPS);
        assert!((l - 50.0).abs() < EPS);

        // 2. 無彩色（delta == 0）時の挙動確認
        // GRAY は H=0, S=0, L=50%
        let (h, s, l, _) = Color::GRAY.to_hsla();
        assert_eq!(h, 0.0);
        assert_eq!(s, 0.0);
        assert!((l - 50.0).abs() < EPS);

        // WHITE は H=0, S=0, L=100%
        let (h, s, l, _) = Color::WHITE.to_hsla();
        assert_eq!(h, 0.0);
        assert_eq!(s, 0.0);
        assert!((l - 100.0).abs() < EPS);

        // BLACK は H=0, S=0, L=0%
        let (h, s, l, _) = Color::BLACK.to_hsla();
        assert_eq!(h, 0.0);
        assert_eq!(s, 0.0);
        assert_eq!(l, 0.0);

        // 3. to_hsl とアルファの保持確認
        let (h, s, l) = Color::RED.with_alpha(0.3).to_hsl();
        assert!((h - 0.0).abs() < EPS);
        assert!((s - 100.0).abs() < EPS);
        assert!((l - 50.0).abs() < EPS);

        let (_, _, _, a) = Color::RED.with_alpha(0.3).to_hsla();
        assert_eq!(a, 0.3);
    }

    #[test]
    fn test_to_oklaba_and_oklab() {
        // 黒 (L=0, a=0, b=0)
        let (l, a, b, alpha) = Color::BLACK.to_oklaba();
        assert!(l.abs() < EPS);
        assert!(a.abs() < EPS);
        assert!(b.abs() < EPS);
        assert_eq!(alpha, 1.0);

        // 白 (L=1, a=0, b=0)
        let (l, a, b, _) = Color::WHITE.to_oklaba();
        assert!((l - 1.0).abs() < EPS);
        assert!(a.abs() < EPS);
        assert!(b.abs() < EPS);

        // 赤 (代表的な Oklab 値との一致確認)
        let (l, a, b, _) = Color::RED.to_oklaba();
        assert!((l - 0.627_955).abs() < EPS);
        assert!((a - 0.224_863).abs() < EPS);
        assert!((b - 0.125_846).abs() < EPS);

        // to_oklab (alphaなし) のタプル要素数・値の整合性
        let (l2, a2, b2) = Color::RED.to_oklab();
        assert_eq!(l, l2);
        assert_eq!(a, a2);
        assert_eq!(b, b2);

        // 往復変換（Color -> to_oklaba -> oklaba -> Color）の整合性
        let original = Color::rgba_f32(0.3, 0.6, 0.9, 0.7);
        let (l, a, b, alpha) = original.to_oklaba();
        let restored = Color::oklaba(l, a, b, alpha);
        assert_color_approx_eq(restored, original);
    }

    #[test]
    fn test_to_oklcha_and_oklch() {
        // 無彩色: Chroma ≈ 0, Hue = 0
        let (l, c, h, alpha) = Color::GRAY.to_oklcha();
        assert!(l > 0.0);
        assert_eq!(c, 0.0);
        assert_eq!(h, 0.0); // 無彩色の色相不定ガード
        assert_eq!(alpha, 1.0);

        // 赤: L ≈ 0.628, Chroma ≈ 0.258, Hue ≈ 29.23°
        let (l, c, h, alpha) = Color::RED.with_alpha(0.65).to_oklcha();
        assert!((l - 0.627_955).abs() < EPS);
        assert!((c - 0.257_683).abs() < EPS);
        assert!((h - 29.233_885).abs() < 1e-2);
        assert_eq!(alpha, 0.65);

        // to_oklch (alphaなし) の整合性
        let (l2, c2, h2) = Color::RED.to_oklch();
        assert_eq!(l, l2);
        assert_eq!(c, c2);
        assert_eq!(h, h2);

        // 色相（h）が常に 0.0..=360.0 の範囲に収まるか
        let test_colors = [
            Color::RED,
            Color::GREEN,
            Color::BLUE,
            Color::YELLOW,
            Color::CYAN,
            Color::MAGENTA,
        ];
        for col in test_colors {
            let (_, _, h) = col.to_oklch();
            assert!((0.0..=360.0).contains(&h), "Hue out of range: {h}");
        }
    }
}
