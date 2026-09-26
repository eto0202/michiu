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
    #[allow(clippy::needless_range_loop)]
    fn mul(&self, other: &Self) -> Self {
        let mut out = [[0.0; 4]; 4];
        for i in 0..4 {
            for j in 0..4 {
                out[i][j] = self.matrix[i][0] * other.matrix[0][j]
                    + self.matrix[i][1] * other.matrix[1][j]
                    + self.matrix[i][2] * other.matrix[2][j]
                    + self.matrix[i][3] * other.matrix[3][j];
            }
        }
        Self { matrix: out }
    }
}
