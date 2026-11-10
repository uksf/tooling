use crate::{axis_common::Frame, mesh_math::V, odol::Lod};
use std::collections::HashSet;

pub struct Cylinder<'a> {
    pub frame: &'a Frame,
    pub up: V,
    pub right: V,
    pub seam: f32,
    pub coefficients: [[f32; 2]; 3],
    pub radius: f32,
}
impl<'a> Cylinder<'a> {
    pub fn new(frame: &'a Frame) -> Self {
        let dir = V::from(frame.direction);
        let y = V(0., 1., 0.);
        let up = (y - dir * y.dot(dir)).unit();
        Self {
            frame,
            up,
            right: up.cross(dir),
            seam: 0.,
            coefficients: [[0.; 2]; 3],
            radius: 0.,
        }
    }
    pub fn angle(&self, p: V) -> f32 {
        let rel = p - V::from(self.frame.origin);
        rel.dot(self.up).atan2(rel.dot(self.right))
    }
    pub fn around(&self, p: V) -> V {
        let t = self.angle(p);
        (self.up * t.cos() - self.right * t.sin()) * self.frame.radius(p.array()).max(1e-3)
    }
    pub fn unwrap(&self, pts: &[V]) -> Vec<f32> {
        let turn = 2. * std::f32::consts::PI;
        let mut t: Vec<_> = pts
            .iter()
            .map(|&p| (((self.angle(p) - self.seam) % turn) + turn) % turn)
            .collect();
        let min = t.iter().copied().fold(f32::INFINITY, f32::min);
        let max = t.iter().copied().fold(f32::NEG_INFINITY, f32::max);
        if max - min > std::f32::consts::PI {
            for x in &mut t {
                if *x < std::f32::consts::PI {
                    *x += turn;
                }
            }
        }
        t
    }
    pub fn map(&self, t: f32, l: f32) -> [f32; 2] {
        let a = self.coefficients;
        [
            a[0][0] + a[1][0] * t + a[2][0] * l,
            a[0][1] + a[1][1] * t + a[2][1] * l,
        ]
    }
    pub fn tangents(&self, dt: V, dl: V, n: V) -> (V, V) {
        let a = self.coefficients;
        let det = a[1][0] * a[2][1] - a[2][0] * a[1][1];
        let du = dt * (a[2][1] / det) + dl * (-a[1][1] / det);
        let dv = dt * (-a[2][0] / det) + dl * (a[1][0] / det);
        let orth = |v: V| {
            let o = v - n * v.dot(n);
            if o.len() > 1e-6 { o.unit() } else { v.unit() }
        };
        (orth(du), orth(dv))
    }
    pub fn fit(
        frame: &'a Frame,
        lod: &Lod,
        uv: &[[f32; 2]],
        faces: &[usize],
    ) -> Result<Self, String> {
        let mut fit = Self::new(frame);
        let mut seen = HashSet::new();
        let verts: Vec<_> = faces
            .iter()
            .flat_map(|&f| lod.faces[f].iter().copied())
            .filter(|v| seen.insert(*v))
            .map(|v| v as usize)
            .collect();
        'outer: for &v in &verts {
            for &w in &verts {
                let du = uv[v][0] - uv[w][0];
                let dv = uv[v][1] - uv[w][1];
                if v != w
                    && (V::from(lod.vertices[v]) - V::from(lod.vertices[w])).len() < 1e-4
                    && (du * du + dv * dv).sqrt() > 0.05
                {
                    fit.seam = fit.angle(V::from(lod.vertices[v]));
                    break 'outer;
                }
            }
        }
        let mut m = [[0f64; 3]; 3];
        let mut bu = [0f64; 3];
        let mut bv = [0f64; 3];
        let mut rows = Vec::new();
        let mut total = 0f64;
        let mut count = 0;
        for &f in faces {
            let face = &lod.faces[f];
            let pts: Vec<_> = face
                .iter()
                .map(|&v| V::from(lod.vertices[v as usize]))
                .collect();
            let t = fit.unwrap(&pts);
            for (k, &v) in face.iter().enumerate() {
                let v = v as usize;
                let l = frame.along(lod.vertices[v]);
                let x = [1., t[k] as f64, l as f64];
                for i in 0..3 {
                    for j in 0..3 {
                        m[i][j] += x[i] * x[j];
                    }
                    bu[i] += x[i] * uv[v][0] as f64;
                    bv[i] += x[i] * uv[v][1] as f64;
                }
                rows.push((t[k], l, uv[v]));
                total += frame.radius(lod.vertices[v]) as f64;
                count += 1;
            }
        }
        if rows.len() < 3 {
            return Err("UV fit needs barrel faces to sample".into());
        }
        let u = gauss(m, bu);
        let v = gauss(m, bv);
        for i in 0..3 {
            fit.coefficients[i] = [u[i] as f32, v[i] as f32];
        }
        fit.radius = (total / count as f64) as f32;
        for (t, l, uv) in rows {
            let p = fit.map(t, l);
            let d = [p[0] - uv[0], p[1] - uv[1]];
            if (d[0] * d[0] + d[1] * d[1]).sqrt() > 0.02 {
                return Err("the barrel UVs are not a cylindrical unwrap".into());
            }
        }
        Ok(fit)
    }
}
fn gauss(mut m: [[f64; 3]; 3], mut b: [f64; 3]) -> [f64; 3] {
    for c in 0..3 {
        let mut p = c;
        for r in c + 1..3 {
            if m[r][c].abs() > m[p][c].abs() {
                p = r;
            }
        }
        m.swap(c, p);
        b.swap(c, p);
        for r in c + 1..3 {
            let k = m[r][c] / m[c][c];
            for j in c..3 {
                m[r][j] -= k * m[c][j];
            }
            b[r] -= k * b[c];
        }
    }
    let mut x = [0.; 3];
    for r in (0..3).rev() {
        let s: f64 = (r + 1..3).map(|j| m[r][j] * x[j]).sum();
        x[r] = (b[r] - s) / m[r][r];
    }
    x
}
