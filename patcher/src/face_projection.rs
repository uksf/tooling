use crate::{cylinder_uv::Cylinder, mesh_math::V};
pub struct Projected {
    pub uv: [f32; 2],
    pub dt: V,
    pub dl: V,
}
pub fn project(fit: &Cylinder<'_>, pts: &[V], fold: f32) -> Vec<Projected> {
    let frame = fit.frame;
    let dir = V::from(frame.direction);
    let origin = V::from(frame.origin);
    let radial = |p: V| {
        let rel = p - origin;
        let off = rel - dir * rel.dot(dir);
        if off.len() > 1e-6 {
            off.unit()
        } else {
            V(0., 1., 0.)
        }
    };
    let normal = (pts[1] - pts[0]).cross(pts[2] - pts[0]).unit();
    let centroid = pts.iter().copied().fold(V::default(), |a, b| a + b) / (pts.len() as f32);
    let facing_axis = normal.dot(dir);
    let facing_out = normal.dot(radial(centroid));
    let facing_around = normal.dot(fit.around(centroid).unit());
    let unwrapped = fit.unwrap(pts);
    let mean = (unwrapped.iter().map(|&t| t as f64).sum::<f64>() / unwrapped.len() as f64) as f32;
    let sign = |x: f32| if x >= 0. { 1. } else { -1. };
    let mut corners = Vec::new();
    for (k, &p) in pts.iter().enumerate() {
        let r = frame.radius(p.array());
        let along = frame.along(p.array());
        let outward = radial(p);
        let (t, l, dt, dl) = if facing_around.abs() >= facing_axis.abs()
            && facing_around.abs() >= facing_out.abs()
        {
            let s = sign(facing_around);
            (
                mean + s * (r - fit.radius) / fit.radius,
                along,
                outward * (s * fit.radius),
                dir,
            )
        } else if facing_axis.abs() >= facing_out.abs() {
            let s = sign(facing_axis);
            (
                unwrapped[k],
                along + s * (fit.radius - r),
                fit.around(p),
                outward * (-s),
            )
        } else {
            (unwrapped[k], along, fit.around(p), dir)
        };
        corners.push(if l > fold {
            (t, 2. * fold - l, dt, -dl)
        } else {
            (t, l, dt, dl)
        });
    }
    let min = corners.iter().map(|c| c.0).fold(f32::INFINITY, f32::min);
    let max = corners
        .iter()
        .map(|c| c.0)
        .fold(f32::NEG_INFINITY, f32::max);
    let shift = 0f32.max(-min) - 0f32.max(max - 2. * std::f32::consts::PI);
    corners
        .into_iter()
        .map(|(t, l, dt, dl)| Projected {
            uv: fit.map(t + shift, l),
            dt,
            dl,
        })
        .collect()
}
