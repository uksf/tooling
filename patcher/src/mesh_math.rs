use std::ops::{Add, Div, Mul, Neg, Sub};
#[derive(Clone, Copy, Debug, Default)]
pub struct V(pub f32, pub f32, pub f32);
impl V {
    pub fn dot(self, b: Self) -> f32 {
        self.0 * b.0 + self.1 * b.1 + self.2 * b.2
    }
    pub fn cross(self, b: Self) -> Self {
        Self(
            self.1 * b.2 - self.2 * b.1,
            self.2 * b.0 - self.0 * b.2,
            self.0 * b.1 - self.1 * b.0,
        )
    }
    pub fn len(self) -> f32 {
        self.dot(self).sqrt()
    }
    pub fn unit(self) -> Self {
        self / self.len()
    }
    pub fn array(self) -> [f32; 3] {
        [self.0, self.1, self.2]
    }
    pub fn transform(self, rows: [Self; 3]) -> Self {
        let [a, b, c] = rows;
        Self(
            self.2.mul_add(c.0, self.1.mul_add(b.0, self.0 * a.0)),
            self.2.mul_add(c.1, self.1.mul_add(b.1, self.0 * a.1)),
            self.2.mul_add(c.2, self.1.mul_add(b.2, self.0 * a.2)),
        )
    }
}
impl From<[f32; 3]> for V {
    fn from(v: [f32; 3]) -> Self {
        Self(v[0], v[1], v[2])
    }
}
impl Add for V {
    type Output = Self;
    fn add(self, b: Self) -> Self {
        Self(self.0 + b.0, self.1 + b.1, self.2 + b.2)
    }
}
impl Sub for V {
    type Output = Self;
    fn sub(self, b: Self) -> Self {
        Self(self.0 - b.0, self.1 - b.1, self.2 - b.2)
    }
}
impl Mul<f32> for V {
    type Output = Self;
    fn mul(self, b: f32) -> Self {
        Self(self.0 * b, self.1 * b, self.2 * b)
    }
}
impl Div<f32> for V {
    type Output = Self;
    fn div(self, b: f32) -> Self {
        Self(self.0 / b, self.1 / b, self.2 / b)
    }
}
impl Neg for V {
    type Output = Self;
    fn neg(self) -> Self {
        Self(-self.0, -self.1, -self.2)
    }
}

#[cfg(test)]
mod tests {
    use super::V;
    #[test]
    fn matches_dotnet_fused_vector_transform() {
        let p = V(0.9864257, -4.23813, 9.183212).transform([
            V(0.74321, 0.912398, -0.10893),
            V(-0.820379, 0.012348, 0.278320),
            V(0.374219, -0.428494, 0.482738),
        ]);
        assert_eq!(
            p.array().map(f32::to_bits),
            [0x40f4b059, 0xc04595d9, 0x40495956]
        );
    }
}
