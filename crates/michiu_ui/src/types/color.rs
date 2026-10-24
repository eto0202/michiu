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

    /// Generates a [`Color`] from the HSL (Hue: 0..360, Saturation: 0..100%, Lightness: 0..100%).
    #[inline]
    #[must_use]
    pub fn hsl(h: f32, s: f32, l: f32) -> Self {
        Self::hsla(h, s, l, 1.0)
    }

    /// Generate a [`Color`] by applying an alpha value (0.0..1.0) to HSL
    #[must_use]
    #[expect(clippy::many_single_char_names)]
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
            r: linear_to_srgb(r_linear),
            g: linear_to_srgb(g_linear),
            b: linear_to_srgb(b_linear),
            a: alpha,
        }
    }

    /// Generate from Oklch (L: 0.0..1.0, Chroma: 0.0..0.4+, Hue: 0..360°)
    #[inline]
    #[must_use]
    pub fn oklch(l: f32, c: f32, h: f32) -> Self {
        Self::oklcha(l, c, h, 1.0)
    }

    /// Generate it by adding an alpha value to Oklch
    #[must_use]
    #[expect(clippy::many_single_char_names)]
    pub fn oklcha(l: f32, c: f32, h: f32, alpha: f32) -> Self {
        let h_rad = h.to_radians();
        let a = c * h_rad.cos();
        let b = c * h_rad.sin();
        Self::oklaba(l, a, b, alpha)
    }

    /// Converts sRGB to Oklch (Lightness: 0.0..=1.0, Chroma: 0.0.., Hue: 0.0..=360.0)
    #[must_use]
    #[expect(clippy::many_single_char_names)]
    pub fn to_oklch(self) -> (f32, f32, f32) {
        // sRGB -> Linear sRGB
        let r_lin = srgb_to_linear(self.r);
        let g_lin = srgb_to_linear(self.g);
        let b_lin = srgb_to_linear(self.b);

        // Linear sRGB -> LMS
        let l = 0.412_221_46 * r_lin + 0.536_332_55 * g_lin + 0.051_445_993 * b_lin;
        let m = 0.211_903_5 * r_lin + 0.680_699_5 * g_lin + 0.107_396_96 * b_lin;
        let s = 0.088_302_46 * r_lin + 0.281_718_84 * g_lin + 0.629_978_7 * b_lin;

        let l_ = l.cbrt();
        let m_ = m.cbrt();
        let s_ = s.cbrt();

        // LMS -> Oklab
        let l_star = 0.210_454_26 * l_ + 0.793_617_8 * m_ - 0.004_072_047 * s_;
        let a_star = 1.977_998_5 * l_ - 2.428_592_2 * m_ + 0.450_593_7 * s_;
        let b_star = 0.025_904_037 * l_ + 0.782_771_77 * m_ - 0.808_675_77 * s_;

        // Oklab -> Oklch (極座標変換)
        let c = (a_star * a_star + b_star * b_star).sqrt();
        let mut h = b_star.atan2(a_star).to_degrees();
        if h < 0.0 {
            h += 360.0;
        }

        (l_star, c, h)
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

// Linear sRGB を通常 sRGB に変換
#[inline]
fn linear_to_srgb(x: f32) -> f32 {
    let clamped = x.clamp(0.0, 1.0);
    if clamped <= 0.003_130_8 {
        12.92 * clamped
    } else {
        1.055 * clamped.powf(1.0 / 2.4) - 0.055
    }
}

// sRGB のガンマを除去して Linear sRGB に変換
#[inline]
fn srgb_to_linear(x: f32) -> f32 {
    let clamped = x.clamp(0.0, 1.0);
    if clamped <= 0.04045 {
        clamped / 12.92
    } else {
        ((clamped + 0.055) / 1.055).powf(2.4)
    }
}

#[cfg(test)]
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
}
