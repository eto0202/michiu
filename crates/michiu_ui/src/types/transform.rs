#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Transform {
    pub(crate) matrix: [[f32; 4]; 4],
}

impl Default for Transform {
    fn default() -> Self {
        Self::new()
    }
}

impl Transform {
    #[inline]
    #[must_use]
    pub fn new() -> Self {
        Self {
            matrix: [
                [1.0, 0.0, 0.0, 0.0],
                [0.0, 1.0, 0.0, 0.0],
                [0.0, 0.0, 1.0, 0.0],
                [0.0, 0.0, 0.0, 1.0],
            ],
        }
    }

    #[inline]
    #[must_use]
    pub fn translate(self, x: f32, y: f32) -> Self {
        let mut t = Self::new();
        t.matrix[3][0] = x;
        t.matrix[3][1] = y;
        self.mul(&t)
    }

    #[inline]
    #[must_use]
    pub fn scale(self, x: f32, y: f32) -> Self {
        let mut s = Self::new();
        s.matrix[0][0] = x;
        s.matrix[1][1] = y;
        self.mul(&s)
    }

    #[inline]
    #[must_use]
    pub fn rotate(self, radians: f32) -> Self {
        let mut r = Self::new();
        let cos = radians.cos();
        let sin = radians.sin();
        r.matrix[0][0] = cos;
        r.matrix[0][1] = sin;
        r.matrix[1][0] = -sin;
        r.matrix[1][1] = cos;
        self.mul(&r)
    }

    /// 4x4 行列の乗算処理（列優先 / Column-Major 対応）
    fn mul(&self, other: &Self) -> Self {
        let mut out = [[0.0; 4]; 4];
        (0..4).for_each(|i| {
            (0..4).for_each(|j| {
                out[i][j] = self.matrix[i][0] * other.matrix[0][j]
                    + self.matrix[i][1] * other.matrix[1][j]
                    + self.matrix[i][2] * other.matrix[2][j]
                    + self.matrix[i][3] * other.matrix[3][j];
            });
        });
        Self { matrix: out }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::f32::consts::PI;

    // 浮動小数点数の 4x4 行列を誤差許容で比較するヘルパー
    fn assert_matrix_approx_eq(a: &Transform, b: &Transform) {
        const EPSILON: f32 = 1e-5;
        for i in 0..4 {
            for j in 0..4 {
                let diff = (a.matrix[i][j] - b.matrix[i][j]).abs();
                assert!(
                    diff < EPSILON,
                    "Matrix mismatch at [{i}][{j}]: expected {}, got {} (diff: {diff})",
                    b.matrix[i][j],
                    a.matrix[i][j]
                );
            }
        }
    }

    // 列ベクトル v = [x, y, z, 1.0] に Transform を適用する簡易関数
    // (WGSL / 列優先: out_row = sum_col (matrix[col][row] * v[col]))
    fn transform_point(t: &Transform, p: [f32; 2]) -> [f32; 2] {
        let v = [p[0], p[1], 0.0, 1.0];
        let mut out = [0.0; 4];
        (0..4).for_each(|row| {
            (0..4).for_each(|col| {
                out[row] += t.matrix[col][row] * v[col];
            });
        });
        [out[0], out[1]]
    }

    #[test]
    fn test_identity() {
        let t = Transform::new();
        let expected = Transform {
            matrix: [
                [1.0, 0.0, 0.0, 0.0],
                [0.0, 1.0, 0.0, 0.0],
                [0.0, 0.0, 1.0, 0.0],
                [0.0, 0.0, 0.0, 1.0],
            ],
        };
        assert_matrix_approx_eq(&t, &expected);

        // 単位行列をかけても変わらないこと (I * M == M)
        let translated = Transform::new().translate(10.0, 20.0);
        assert_matrix_approx_eq(&translated.mul(&Transform::new()), &translated);
        assert_matrix_approx_eq(&Transform::new().mul(&translated), &translated);
    }

    #[test]
    fn test_translate() {
        let t = Transform::new().translate(10.0, -5.0);
        let pt = transform_point(&t, [0.0, 0.0]);
        assert!((pt[0] - 10.0).abs() < 1e-5);
        assert!((pt[1] - (-5.0)).abs() < 1e-5);
    }

    #[test]
    fn test_scale() {
        let t = Transform::new().scale(2.0, 3.0);
        let pt = transform_point(&t, [4.0, 5.0]);
        assert!((pt[0] - 8.0).abs() < 1e-5);
        assert!((pt[1] - 15.0).abs() < 1e-5);
    }

    #[test]
    fn test_rotate() {
        // 90度回転（反時計回り）
        // (1, 0) -> (0, 1)
        let t = Transform::new().rotate(PI / 2.0);
        let pt = transform_point(&t, [1.0, 0.0]);
        assert!((pt[0] - 0.0).abs() < 1e-5);
        assert!((pt[1] - 1.0).abs() < 1e-5);
    }

    #[test]
    fn test_combined_transforms_order() {
        // 拡大してから平行移動
        // (1, 0) -> scale(2, 2) -> (2, 0) -> translate(10, 0) -> (12, 0)
        let t = Transform::new().scale(2.0, 2.0).translate(10.0, 0.0);
        let pt = transform_point(&t, [1.0, 0.0]);

        assert!(
            (pt[0] - 12.0).abs() < 1e-5,
            "expected x=12.0, got {}",
            pt[0]
        );
        assert!((pt[1] - 0.0).abs() < 1e-5, "expected y=0.0, got {}", pt[1]);
    }
}
