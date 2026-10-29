use crate::Color;

/// Raw gradient data for GPU buffer transfers.
#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, bytemuck::Pod, bytemuck::Zeroable)]
pub struct LinearGradientInner {
    /// Colors to use for the gradient (max 4 colors)
    pub colors: [Color; 4],
    /// Position of each color (offset from 0.0 to 1.0)
    pub stops: [f32; 4],
    /// Gradient Angle (Radians)
    pub angle: f32,
    /// Number of valid color stops (2–4)
    pub stop_count: u32,
    /// Rotation and blend center of the gradient (`[0.5, 0.5]` is the center of the rectangle)
    pub origin: [f32; 2],
}

impl Default for LinearGradientInner {
    fn default() -> Self {
        Self {
            colors: [Color::TRANSPARENT; 4],
            stops: [0.0, 1.0, 0.0, 0.0],
            angle: 0.0,
            stop_count: 0,
            origin: [0.5, 0.5],
        }
    }
}

/// A struct that represents linear gradients and four-corner gradients.
///
/// - Linear gradient (`GradientType::Linear`): Complies with the CSS `linear-gradient` specification (0° = top, clockwise).
/// - Four-corner gradient (`GradientType::FourCorners`): Bilinear interpolation with colors placed at the four corners.
///
/// # Angle Specifications (clockwise)
/// - `0°`: From bottom to top
/// - `90°`: From left to right
/// - `180°`: From top to bottom
/// - `270°`: From right to left
///
/// # Examples
/// ```
/// # use michiu_ui::{prelude::*, LinearGradient, GradientCorner};
/// // Two-color linear gradient (from left to right)
/// let grad = LinearGradient::two_stops(Color::WHITE, Color::BLACK, 90.0);
///
/// // Custom Stops in 3 Colors (45 Degrees)
/// let multi = LinearGradient::new(
///     &[
///         (Color::RED, 0.0),
///         (Color::ORANGE, 0.5),
///         (Color::YELLOW, 1.0),
///     ],
///     45.0,
/// );
///
/// // The diagonal of a rectangle (from the top-left to the bottom-right)
/// let corner_grad = LinearGradient::to_corner(
///     &[(Color::WHITE, 0.0), (Color::BLACK, 1.0)],
///     GradientCorner::BottomRight,
///     200.0,
///     100.0,
/// );
///
/// // Corner Gradient (Centered Toward the Top Left)
/// let mesh = LinearGradient::four_corners(
///     Color::RED,
///     Color::BLUE,
///     Color::YELLOW,
///     Color::GREEN,
/// ).with_origin([0.25, 0.25]);
/// ```
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct LinearGradient {
    pub(crate) inner: LinearGradientInner,
    pub(crate) grad_type: GradientType,
}

#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub(crate) enum GradientType {
    #[default]
    Linear,
    FourCorners,
}

/// The four corner directions supported for corner specification.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GradientCorner {
    /// Bottom left -> Top right
    TopRight,
    /// Top left -> Bottom right
    BottomRight,
    /// Top right -> Bottom left
    BottomLeft,
    /// Bottom right -> Top left
    TopLeft,
}

impl LinearGradient {
    /// Creates a linear gradient with two colors (start point 0.0, end point 1.0).
    ///
    /// # Arguments
    /// * `c0` - Start color (offset 0.0)
    /// * `c1` - End color (offset 1.0)
    /// * `angle_degrees` - Gradient angle (degrees 0.0..=360.0)
    #[inline]
    #[must_use]
    pub fn two_stops(c0: Color, c1: Color, angle_degrees: f32) -> Self {
        Self::new(&[(c0, 0.0), (c1, 1.0)], angle_degrees)
    }

    /// Creates a linear gradient from a color stop slice.
    ///
    /// The number of supported stops ranges from 2 to 4 colors.
    ///
    /// # Arguments
    /// - `stops_slice` - A slice of tuples of the form `(color, position 0.0–1.0)`
    ///    - Empty slice: Returns `Default::default()`.
    ///    - Only 1 color: To prevent division by zero, this results in a solid-color gradient using that single color.
    ///    - 2–4 colors: Interpolated according to the specified stop positions.
    ///    - 5 or more colors: Only the first 4 colors are used; the 5th and subsequent colors are ignored.
    /// - `angle_degrees` - The gradient angle (degrees 0.0..=360.0)
    #[inline]
    #[must_use]
    #[expect(clippy::cast_possible_truncation)]
    pub fn new(stops_slice: &[(Color, f32)], angle_degrees: f32) -> Self {
        let mut grad = LinearGradient::default();
        let count = stops_slice.len().clamp(2, 4);

        if stops_slice.is_empty() {
            return Self::default();
        } else if stops_slice.len() == 1 {
            let (c, _) = stops_slice[0];
            grad.inner.colors[0] = c;
            grad.inner.colors[1] = c;
            grad.inner.stops[0] = 0.0;
            grad.inner.stops[1] = 1.0;
        } else {
            for (i, &(c, s)) in stops_slice.iter().take(4).enumerate() {
                grad.inner.colors[i] = c;
                grad.inner.stops[i] = s;
            }
        }

        grad.inner.angle = angle_degrees.to_radians();
        grad.inner.stop_count = count as u32;
        grad
    }

    /// Changes the angle of the gradient (for linear gradients).
    ///
    /// # Arguments
    /// * `angle_degrees` - New angle (degrees 0.0..=360.0)
    #[inline]
    #[must_use]
    pub fn with_angle(mut self, angle_degrees: f32) -> Self {
        self.inner.angle = angle_degrees.to_radians();
        self
    }

    /// Changes the reference position for the gradient.
    ///
    /// - Linear gradient: Shifts the origin for rotation and symmetry.
    /// - Four-corner gradient: Moves the intersection point (where the four colors blend evenly).
    ///
    /// # Arguments
    /// * `origin` - Normalized coordinates relative to the rectangle (e.g., top-left `[0.0, 0.0]`, center `[0.5, 0.5]`)
    #[inline]
    #[must_use]
    pub fn with_origin(mut self, origin: [f32; 2]) -> Self {
        self.inner.origin = origin;
        self
    }

    /// Creates a linear gradient extending toward the four corners, based on the rectangle's dimensions (aspect ratio).
    ///
    /// # Arguments
    /// * `stops_slice` - A slice of tuples of the form `(color, position 0.0..=1.0)`
    /// * `corner` - The direction of the corner toward which the gradient points
    /// * `width` - The width of the rectangle
    /// * `height` - The height of the rectangle
    #[inline]
    #[must_use]
    pub fn to_corner(
        stops_slice: &[(Color, f32)],
        corner: GradientCorner,
        width: f32,
        height: f32,
    ) -> Self {
        let angle_rad = match corner {
            GradientCorner::TopRight => (width).atan2(height),
            GradientCorner::BottomRight => (width).atan2(-height),
            GradientCorner::BottomLeft => (-width).atan2(-height),
            GradientCorner::TopLeft => (-width).atan2(height),
        };

        let angle_rad = if angle_rad < 0.0 {
            angle_rad + std::f32::consts::TAU
        } else {
            angle_rad
        };

        // 0.0 で new してから計算したラジアンを直接セット（度数法の往復誤差を防止）
        let mut grad = Self::new(stops_slice, 0.0);
        grad.inner.angle = angle_rad;
        grad
    }

    /// Creates a gradient with distinct colors at each corner.
    ///
    /// The default meeting point is at the center `[0.5, 0.5]`, but you can freely move the intersection point using [`with_origin`](Self::with_origin).
    ///
    /// # Arguments
    /// * `top_left` - Top-left color
    /// * `top_right` - Top-right color
    /// * `bottom_left` - Bottom-left color
    /// * `bottom_right` - Bottom-right color
    #[inline]
    #[must_use]
    pub fn four_corners(
        top_left: Color,
        top_right: Color,
        bottom_left: Color,
        bottom_right: Color,
    ) -> Self {
        Self {
            inner: LinearGradientInner {
                colors: [top_left, top_right, bottom_left, bottom_right],
                stops: [0.0; 4],
                angle: 0.0,
                stop_count: 4,
                origin: [0.5, 0.5],
            },
            grad_type: GradientType::FourCorners,
        }
    }
}

#[cfg(test)]
#[allow(clippy::float_cmp)]
mod tests {
    use super::*;
    use std::f32::consts::PI;

    // 浮動小数点数の許容誤差比較ヘルパー
    fn approx_eq(a: f32, b: f32) -> bool {
        (a - b).abs() < 1e-4
    }

    #[test]
    fn test_memory_layout() {
        // GPUの Storage Buffer と整合性を保つため、厳密に 96 バイトであることを保証
        assert_eq!(std::mem::size_of::<LinearGradientInner>(), 96);
    }

    #[test]
    fn test_default() {
        let grad = LinearGradient::default();
        assert_eq!(grad.grad_type, GradientType::Linear);
        assert_eq!(grad.inner.stop_count, 0);
        assert_eq!(grad.inner.origin, [0.5, 0.5]);
        assert_eq!(grad.inner.colors[0], Color::TRANSPARENT);
    }

    #[test]
    fn test_two_stops() {
        let grad = LinearGradient::two_stops(Color::WHITE, Color::BLACK, 90.0);
        assert_eq!(grad.grad_type, GradientType::Linear);
        assert_eq!(grad.inner.stop_count, 2);
        assert_eq!(grad.inner.colors[0], Color::WHITE);
        assert_eq!(grad.inner.colors[1], Color::BLACK);
        assert_eq!(grad.inner.stops[0], 0.0);
        assert_eq!(grad.inner.stops[1], 1.0);
        assert!(approx_eq(grad.inner.angle, PI * 0.5)); // 90° = π/2
    }

    #[test]
    fn test_new_slice_variations() {
        // 空スライスのフォールバック
        let empty = LinearGradient::new(&[], 0.0);
        assert_eq!(empty, LinearGradient::default());

        // 1色のみのフォールバック（ゼロ除算防止で単色塗りつぶし）
        let single = LinearGradient::new(&[(Color::RED, 0.5)], 0.0);
        assert_eq!(single.inner.stop_count, 2);
        assert_eq!(single.inner.colors[0], Color::RED);
        assert_eq!(single.inner.colors[1], Color::RED);
        assert_eq!(single.inner.stops[0], 0.0);
        assert_eq!(single.inner.stops[1], 1.0);

        // 3色カスタムストップ
        let three = LinearGradient::new(
            &[(Color::RED, 0.0), (Color::GREEN, 0.3), (Color::BLUE, 1.0)],
            180.0,
        );
        assert_eq!(three.inner.stop_count, 3);
        assert_eq!(three.inner.colors[2], Color::BLUE);
        assert_eq!(three.inner.stops[1], 0.3);

        // 5色以上渡された場合のクランプ（先頭4色のみ採用）
        let five = LinearGradient::new(
            &[
                (Color::RED, 0.0),
                (Color::GREEN, 0.25),
                (Color::BLUE, 0.5),
                (Color::YELLOW, 0.75),
                (Color::WHITE, 1.0), // 切り捨てられる
            ],
            0.0,
        );
        assert_eq!(five.inner.stop_count, 4);
        assert_eq!(five.inner.colors[3], Color::YELLOW);
    }

    #[test]
    fn test_with_angle_and_origin() {
        let grad = LinearGradient::two_stops(Color::WHITE, Color::BLACK, 0.0)
            .with_angle(180.0)
            .with_origin([0.2, 0.8]);

        assert!(approx_eq(grad.inner.angle, PI));
        assert_eq!(grad.inner.origin, [0.2, 0.8]);
    }

    #[test]
    fn test_four_corners() {
        let grad =
            LinearGradient::four_corners(Color::RED, Color::GREEN, Color::BLUE, Color::YELLOW);

        assert_eq!(grad.grad_type, GradientType::FourCorners);
        assert_eq!(grad.inner.stop_count, 4);
        assert_eq!(grad.inner.colors[0], Color::RED); // 左上
        assert_eq!(grad.inner.colors[1], Color::GREEN); // 右上
        assert_eq!(grad.inner.colors[2], Color::BLUE); // 左下
        assert_eq!(grad.inner.colors[3], Color::YELLOW); // 右下
        assert_eq!(grad.inner.origin, [0.5, 0.5]);
    }

    #[test]
    fn test_to_corner() {
        let stops = [(Color::WHITE, 0.0), (Color::BLACK, 1.0)];

        // 正方形 (100x100) における各角への角度検証
        let tr = LinearGradient::to_corner(&stops, GradientCorner::TopRight, 100.0, 100.0);
        let br = LinearGradient::to_corner(&stops, GradientCorner::BottomRight, 100.0, 100.0);
        let bl = LinearGradient::to_corner(&stops, GradientCorner::BottomLeft, 100.0, 100.0);
        let tl = LinearGradient::to_corner(&stops, GradientCorner::TopLeft, 100.0, 100.0);

        assert!(approx_eq(tr.inner.angle.to_degrees(), 45.0));
        assert!(approx_eq(br.inner.angle.to_degrees(), 135.0));
        assert!(approx_eq(bl.inner.angle.to_degrees(), 225.0));
        assert!(approx_eq(tl.inner.angle.to_degrees(), 315.0));
    }
}
