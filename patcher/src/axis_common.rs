use crate::{
    lod_names::resolution,
    odol::{Lod, Model},
};
use serde_json::Value;

pub fn string<'a>(v: &'a Value, key: &str) -> Result<&'a str, String> {
    v.get(key)
        .and_then(Value::as_str)
        .ok_or_else(|| format!("missing string {key}"))
}
pub fn number(v: &Value, key: &str) -> Result<f32, String> {
    let n = v
        .get(key)
        .and_then(Value::as_f64)
        .ok_or_else(|| format!("missing number {key}"))? as f32;
    if !n.is_finite() {
        return Err(format!("invalid number {key}"));
    }
    Ok(n)
}
pub fn lod_index(model: &Model, name: &str) -> Result<usize, String> {
    let n = resolution(name)?;
    let v: Vec<_> = model
        .lods
        .iter()
        .enumerate()
        .filter(|(_, l)| l.resolution == n || (n > 1000.0 && (l.resolution - n).abs() <= n * 1e-6))
        .map(|(i, _)| i)
        .collect();
    if v.len() != 1 {
        return Err(format!("no {name} LOD"));
    }
    Ok(v[0])
}
pub struct Frame {
    pub origin: [f32; 3],
    pub direction: [f32; 3],
}
impl Frame {
    pub fn from(op: &Value) -> Result<Self, String> {
        let a = op
            .get("axis")
            .and_then(Value::as_array)
            .filter(|a| a.len() == 6)
            .ok_or("invalid axis")?;
        let mut v = [0.; 6];
        for (i, x) in a.iter().enumerate() {
            v[i] = x.as_f64().ok_or("invalid axis")? as f32;
            if !v[i].is_finite() {
                return Err("invalid axis".into());
            }
        }
        let length = (v[3] * v[3] + v[4] * v[4] + v[5] * v[5]).sqrt();
        if length < 1e-6 {
            return Err("axis direction is zero".into());
        }
        Ok(Self {
            origin: [v[0], v[1], v[2]],
            direction: [v[3] / length, v[4] / length, v[5] / length],
        })
    }
    pub fn along(&self, p: [f32; 3]) -> f32 {
        (p[0] - self.origin[0]) * self.direction[0]
            + (p[1] - self.origin[1]) * self.direction[1]
            + (p[2] - self.origin[2]) * self.direction[2]
    }
}
pub fn bound(model: &Model, lod: &Lod, op: &Value) -> Result<Vec<bool>, String> {
    let Some(bones) = op.get("bones") else {
        return Ok(vec![true; lod.vertices.len()]);
    };
    let bones = bones
        .as_array()
        .ok_or("invalid bones")?
        .iter()
        .map(|v| v.as_str().ok_or("invalid bone"))
        .collect::<Result<Vec<_>, _>>()?;
    for name in &bones {
        if !model
            .info
            .skeleton
            .bones
            .iter()
            .any(|b| b.name.eq_ignore_ascii_case(name.as_bytes()))
        {
            return Err(format!("skeleton has no bone '{name}'"));
        }
    }
    let wanted = lod
        .sub_to_skeleton
        .iter()
        .map(|&i| {
            model.info.skeleton.bones.get(i as usize).is_some_and(|b| {
                bones
                    .iter()
                    .any(|n| b.name.eq_ignore_ascii_case(n.as_bytes()))
            })
        })
        .collect::<Vec<_>>();
    if lod.bones.len() != lod.vertices.len() {
        return Ok(vec![false; lod.vertices.len()]);
    }
    Ok(lod
        .bones
        .iter()
        .map(|b| {
            b.count > 0
                && (0..b.count as usize).all(|i| {
                    b.data
                        .get(2 * i)
                        .and_then(|&n| wanted.get(n as usize))
                        .copied()
                        .unwrap_or(false)
                })
        })
        .collect())
}
