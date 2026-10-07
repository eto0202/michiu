use crate::{BoxShadow, Color, CornerRadius, LayoutPoint, PropertyList};
use std::{
    f32::consts::PI,
    time::{Duration, Instant},
};

/// Values that can be transitioned
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum TransitionValue {
    Color(Color),
    Opacity(f32),
    Transform([[f32; 4]; 4]),
    CornerRadius(CornerRadius),
    Width(f32),
    Height(f32),
    BoxShadow(BoxShadow),
}

const TWO_PI: f32 = PI * 2.0;

impl TransitionValue {
    /// 進行度 t (0.0 ～ 1.0) に基づいて、自己と目標値を線形補間する
    #[inline]
    pub(crate) fn lerp(&self, other: &Self, t: f32) -> Self {
        match (self, other) {
            (TransitionValue::Color(s), TransitionValue::Color(e)) => {
                TransitionValue::Color(Color {
                    r: s.r + (e.r - s.r) * t,
                    g: s.g + (e.g - s.g) * t,
                    b: s.b + (e.b - s.b) * t,
                    a: s.a + (e.a - s.a) * t,
                })
            }
            (TransitionValue::Opacity(s), TransitionValue::Opacity(e)) => {
                TransitionValue::Opacity(s + (e - s) * t)
            }
            (TransitionValue::Transform(s), TransitionValue::Transform(e)) => {
                let d_start = decompose_2d(s);
                let d_end = decompose_2d(e);

                // 各変形要素の線形補間 (Lerp)
                let tx =
                    d_start.translation[0] + (d_end.translation[0] - d_start.translation[0]) * t;
                let ty =
                    d_start.translation[1] + (d_end.translation[1] - d_start.translation[1]) * t;

                let sx = d_start.scale[0] + (d_end.scale[0] - d_start.scale[0]) * t;
                let sy = d_start.scale[1] + (d_end.scale[1] - d_start.scale[1]) * t;

                // 境界 (-PI ～ PI) での逆逆回転を防ぐ最短軌道での角度差分補間
                let mut diff = d_end.rotation - d_start.rotation;

                diff = (diff + PI).rem_euclid(TWO_PI) - PI; // 最短角度差にクランプ

                let rotation = d_start.rotation + diff * t;

                let d_mid = Decomposed2D {
                    translation: [tx, ty],
                    scale: [sx, sy],
                    rotation,
                };

                // 正確に再合成して 4x4 行列として返却
                TransitionValue::Transform(recompose_2d(&d_mid))
            }
            (TransitionValue::CornerRadius(s), TransitionValue::CornerRadius(e)) => {
                TransitionValue::CornerRadius(CornerRadius {
                    top_left: s.top_left + (e.top_left - s.top_left) * t,
                    top_right: s.top_right + (e.top_right - s.top_right) * t,
                    bottom_right: s.bottom_right + (e.bottom_right - s.bottom_right) * t,
                    bottom_left: s.bottom_left + (e.bottom_left - s.bottom_left) * t,
                })
            }
            (TransitionValue::Width(s), TransitionValue::Width(e)) => {
                TransitionValue::Width(s + (e - s) * t)
            }
            (TransitionValue::Height(s), TransitionValue::Height(e)) => {
                TransitionValue::Height(s + (e - s) * t)
            }
            (TransitionValue::BoxShadow(s), TransitionValue::BoxShadow(e)) => {
                TransitionValue::BoxShadow(BoxShadow {
                    offset: LayoutPoint {
                        x: s.offset.x + (e.offset.x - s.offset.x) * t,
                        y: s.offset.y + (e.offset.y - s.offset.y) * t,
                    },
                    blur: s.blur + (e.blur - s.blur) * t,
                    spread: s.spread + (e.spread - s.spread) * t,
                    color: Color {
                        r: s.color.r + (e.color.r - s.color.r) * t,
                        g: s.color.g + (e.color.g - s.color.g) * t,
                        b: s.color.b + (e.color.b - s.color.b) * t,
                        a: s.color.a + (e.color.a - s.color.a) * t,
                    },
                })
            }
            _ => *self, // 型不一致の場合は遷移せずに自己を返す
        }
    }
}

/// Active transitions currently in progress
#[derive(Debug, Clone, Copy)]
pub struct ActiveTransition {
    pub property_list: PropertyList,
    pub start_time: Option<Instant>,
    pub duration: Duration,
    pub curve: AnimationCurve,
    pub start_value: TransitionValue,
    pub end_value: TransitionValue,
}

#[derive(Debug, Clone, Copy)]
struct Decomposed2D {
    translation: [f32; 2],
    scale: [f32; 2],
    rotation: f32, // ラジアン単位のZ軸回転
}

/// 4x4 Column-Major 行列から 2D 平行移動・スケール・回転を分解抽出する
fn decompose_2d(m: &[[f32; 4]; 4]) -> Decomposed2D {
    let tx = m[3][0];
    let ty = m[3][1];

    // X軸の基底ベクトル [m[0][0], m[0][1]] から Xスケールを算出
    let m00 = m[0][0];
    let m01 = m[0][1];
    let sx = (m00 * m00 + m01 * m01).sqrt();

    // Y軸の基底ベクトル [m[1][0], m[1][1]] から Yスケールを算出
    let m10 = m[1][0];
    let m11 = m[1][1];
    let sy = (m10 * m10 + m11 * m11).sqrt();

    // atan2 で回転角度を正確に抽出する (sx が極小の場合は 0.0)
    let rotation = if sx > 1e-6 { m01.atan2(m00) } else { 0.0 };

    Decomposed2D {
        translation: [tx, ty],
        scale: [sx, sy],
        rotation,
    }
}

/// 補間された2Dパラメータから 4x4 アフィン行列へ再ビルドする
fn recompose_2d(d: &Decomposed2D) -> [[f32; 4]; 4] {
    let cos = d.rotation.cos();
    let sin = d.rotation.sin();

    let mut m = [
        [1.0, 0.0, 0.0, 0.0],
        [0.0, 1.0, 0.0, 0.0],
        [0.0, 0.0, 1.0, 0.0],
        [0.0, 0.0, 0.0, 1.0],
    ];

    // スケールと回転の合成適用
    m[0][0] = d.scale[0] * cos;
    m[0][1] = d.scale[0] * sin;
    m[1][0] = d.scale[1] * -sin;
    m[1][1] = d.scale[1] * cos;

    // 平行移動
    m[3][0] = d.translation[0];
    m[3][1] = d.translation[1];

    m
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Transition {
    pub property_list: PropertyList,
    pub duration: Duration,
    pub curve: AnimationCurve,
}

impl Transition {
    #[must_use]
    pub fn new(property_list: PropertyList, duration: Duration, curve: AnimationCurve) -> Self {
        Self {
            property_list,
            duration,
            curve,
        }
    }

    #[must_use]
    pub fn property_list(mut self, property_list: PropertyList) -> Self {
        self.property_list = property_list;
        self
    }

    #[must_use]
    pub fn duration(mut self, duration: Duration) -> Self {
        self.duration = duration;
        self
    }

    #[must_use]
    pub fn curve(mut self, curve: AnimationCurve) -> Self {
        self.curve = curve;
        self
    }
}

/// Define the easing curve for the animation.
// TODO: バネ物理シミュレーション Spring Physics
// 摩擦（Damping）とバネの強さ（Stiffness）のパラメータから毎フレーム物理演算
#[derive(Debug, Clone, Copy)]
pub enum AnimationCurve {
    /// No easing (linear)
    Linear,
    /// Standard quadratic easing for acceleration and deceleration
    EaseInOutQuad,
    /// Accelerate
    EaseInQuad,
    /// Deceleration
    EaseOutQuad,
    /// An escape hatch capable of performing custom easing calculations
    /// (accepts values between 0.0 and 1.0 and returns values between 0.0 and 1.0)
    Custom(fn(f32) -> f32),
}

impl AnimationCurve {
    /// Evaluate the smoothed value based on the progress ratio t (0.0 <= t <= 1.0).
    #[must_use]
    pub fn evaluate(&self, t: f32) -> f32 {
        let t = t.clamp(0.0, 1.0);
        match *self {
            AnimationCurve::Linear => t,
            AnimationCurve::EaseInQuad => t * t,
            AnimationCurve::EaseOutQuad => t * (2.0 - t),
            AnimationCurve::EaseInOutQuad => {
                if t < 0.5 {
                    2.0 * t * t
                } else {
                    -1.0 + (4.0 - 2.0 * t) * t
                }
            }
            AnimationCurve::Custom(f) => f(t),
        }
    }
}

impl PartialEq for AnimationCurve {
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            (Self::Linear, Self::Linear)
            | (Self::EaseInOutQuad, Self::EaseInOutQuad)
            | (Self::EaseInQuad, Self::EaseInQuad)
            | (Self::EaseOutQuad, Self::EaseOutQuad) => true,
            (Self::Custom(f1), Self::Custom(f2)) => std::ptr::fn_addr_eq(*f1, *f2),
            _ => false,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PlaybackCount {
    Infinite,
    Count(u32),
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct KeyframeAnimation {
    /// What to Animate
    pub property: PropertyList,
    /// Time per lap (loop)
    pub duration: Duration,
    /// Number of loops
    pub iteration_count: PlaybackCount,
    /// Easing Curve
    pub curve: AnimationCurve,
}

impl KeyframeAnimation {
    #[inline]
    #[must_use]
    pub fn new(
        property: PropertyList,
        duration: Duration,
        iteration_count: PlaybackCount,
        curve: AnimationCurve,
    ) -> Self {
        Self {
            property,
            duration,
            iteration_count,
            curve,
        }
    }
}
