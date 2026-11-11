use crate::mesh_math::V;
use std::cmp::Ordering;

pub const MAX_POINTS: usize = 96;

#[derive(Clone, Copy)]
struct D(f64, f64, f64);

impl D {
    fn of(v: V) -> Self {
        Self(f64::from(v.0), f64::from(v.1), f64::from(v.2))
    }
    fn sub(self, b: Self) -> Self {
        Self(self.0 - b.0, self.1 - b.1, self.2 - b.2)
    }
    fn neg(self) -> Self {
        Self(-self.0, -self.1, -self.2)
    }
    fn div(self, s: f64) -> Self {
        Self(self.0 / s, self.1 / s, self.2 / s)
    }
    fn dot(self, b: Self) -> f64 {
        self.0 * b.0 + self.1 * b.1 + self.2 * b.2
    }
    fn cross(self, b: Self) -> Self {
        Self(
            self.1 * b.2 - self.2 * b.1,
            self.2 * b.0 - self.0 * b.2,
            self.0 * b.1 - self.1 * b.0,
        )
    }
    fn len(self) -> f64 {
        self.dot(self).sqrt()
    }
}

pub fn fmin(a: f32, b: f32) -> f32 {
    if a.is_nan() || b.is_nan() {
        f32::NAN
    } else if a < b || (a == b && a.is_sign_negative()) {
        a
    } else {
        b
    }
}

pub fn fmax(a: f32, b: f32) -> f32 {
    if a.is_nan() || b.is_nan() {
        f32::NAN
    } else if a > b || (a == b && a.is_sign_positive()) {
        a
    } else {
        b
    }
}

pub fn bounds(points: impl IntoIterator<Item = V>) -> (V, V) {
    let mut min = V(f32::MAX, f32::MAX, f32::MAX);
    let mut max = V(f32::MIN, f32::MIN, f32::MIN);
    for p in points {
        min = V(fmin(min.0, p.0), fmin(min.1, p.1), fmin(min.2, p.2));
        max = V(fmax(max.0, p.0), fmax(max.1, p.1), fmax(max.2, p.2));
    }
    (min, max)
}

pub fn tolerance(points: &[V]) -> f64 {
    let (min, max) = bounds(points.iter().copied());
    let t = 1e-5 * f64::from((max - min).len());
    if t.is_nan() { t } else { t.max(1e-6) }
}

fn dedupe(ids: &[i32], points: &[V]) -> Vec<(i32, V)> {
    let eps = tolerance(points) as f32;
    let mut result: Vec<(i32, V)> = Vec::new();
    for (&id, &p) in ids.iter().zip(points) {
        if !result.iter().any(|r| (r.1 - p).len() <= eps) {
            result.push((id, p));
        }
    }
    result
}

fn compare(a: f64, b: f64) -> Ordering {
    a.partial_cmp(&b)
        .unwrap_or_else(|| b.is_nan().cmp(&a.is_nan()))
}

fn turn(o: (usize, f64, f64), a: (usize, f64, f64), b: (usize, f64, f64)) -> f64 {
    (a.1 - o.1) * (b.2 - o.2) - (a.2 - o.2) * (b.1 - o.1)
}

fn polygon_on_plane(indices: &[usize], p: &[D], normal: D) -> Vec<usize> {
    let axis = if normal.0.abs() < 0.9 {
        D(1.0, 0.0, 0.0)
    } else {
        D(0.0, 1.0, 0.0)
    };
    let mut u = normal.cross(axis);
    u = u.div(u.len());
    let v = normal.cross(u);
    let mut flat: Vec<(usize, f64, f64)> = indices
        .iter()
        .map(|&i| (i, p[i].dot(u), p[i].dot(v)))
        .collect();
    flat.sort_by(|a, b| compare(a.1, b.1).then_with(|| compare(a.2, b.2)));
    let chain = |items: &mut dyn Iterator<Item = (usize, f64, f64)>| {
        let mut hull: Vec<(usize, f64, f64)> = Vec::new();
        for q in items {
            while hull.len() >= 2 && turn(hull[hull.len() - 2], hull[hull.len() - 1], q) <= 1e-12 {
                hull.pop();
            }
            hull.push(q);
        }
        hull.pop();
        hull
    };
    let lower = chain(&mut flat.iter().copied());
    let upper = chain(&mut flat.iter().rev().copied());
    lower.into_iter().chain(upper).map(|q| q.0).collect()
}

pub fn build(ids: &[i32], points: &[V]) -> Result<Vec<[i32; 3]>, String> {
    let unique = dedupe(ids, points);
    if unique.len() > MAX_POINTS {
        return Err(format!(
            "{} distinct points; the hull builder handles up to {MAX_POINTS}",
            unique.len()
        ));
    }
    let p: Vec<D> = unique.iter().map(|u| D::of(u.1)).collect();
    let eps = tolerance(points);
    let mut planes: Vec<(D, f64)> = Vec::new();
    for i in 0..p.len() {
        for j in i + 1..p.len() {
            for k in j + 1..p.len() {
                let mut n = p[j].sub(p[i]).cross(p[k].sub(p[i]));
                let length = n.len();
                if length < eps * eps {
                    continue;
                }
                n = n.div(length);
                let d = n.dot(p[i]);
                let front = p.iter().filter(|x| n.dot(**x) - d > eps).count();
                let back = p.iter().filter(|x| n.dot(**x) - d < -eps).count();
                if front > 0 && back > 0 {
                    continue;
                }
                let outward = if front == 0 { n } else { n.neg() };
                let od = outward.dot(p[i]);
                if !planes
                    .iter()
                    .any(|q| q.0.dot(outward) > 1.0 - 1e-9 && (q.1 - od).abs() <= eps)
                {
                    planes.push((outward, od));
                }
            }
        }
    }
    if planes.len() < 4 {
        return Err(
            "the points are coplanar or collinear, so their convex hull has no volume".into(),
        );
    }
    let mut triangles = Vec::new();
    for (normal, d) in planes {
        let on: Vec<usize> = (0..p.len())
            .filter(|&i| (normal.dot(p[i]) - d).abs() <= eps)
            .collect();
        let ring = polygon_on_plane(&on, &p, normal);
        for t in 1..ring.len().saturating_sub(1) {
            triangles.push([unique[ring[0]].0, unique[ring[t]].0, unique[ring[t + 1]].0]);
        }
    }
    Ok(triangles)
}

pub fn max_outside(t: [V; 3], points: &[V]) -> f64 {
    let a = D::of(t[0]);
    let mut n = D::of(t[1]).sub(a).cross(D::of(t[2]).sub(a));
    n = n.div(n.len());
    let mut value = f64::NAN;
    for x in points {
        let d = n.dot(D::of(*x).sub(a));
        if value.is_nan() || d > value {
            value = d;
        }
    }
    value
}
