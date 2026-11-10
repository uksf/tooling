use crate::{
    axis_common::{self, Frame},
    mesh_math::V,
};
use serde_json::Value;
use std::collections::{HashMap, HashSet};

pub struct Corner {
    pub point: usize,
    pub normal: V,
}
pub struct Mesh {
    pub points: Vec<V>,
    pub faces: Vec<Vec<Corner>>,
    pub weld: Vec<bool>,
}
struct Reader<'a> {
    data: &'a [u8],
    pos: usize,
}
impl<'a> Reader<'a> {
    fn take(&mut self, n: usize) -> Result<&'a [u8], String> {
        let end = self.pos.checked_add(n).ok_or("MLOD overflow")?;
        let b = self.data.get(self.pos..end).ok_or("truncated MLOD")?;
        self.pos = end;
        Ok(b)
    }
    fn u32(&mut self) -> Result<usize, String> {
        Ok(u32::from_le_bytes(self.take(4)?.try_into().unwrap()) as usize)
    }
    fn f32(&mut self) -> Result<f32, String> {
        Ok(f32::from_le_bytes(self.take(4)?.try_into().unwrap()))
    }
    fn string(&mut self) -> Result<(), String> {
        let n = self.data[self.pos..]
            .iter()
            .position(|&x| x == 0)
            .ok_or("unterminated MLOD string")?;
        self.take(n + 1)?;
        Ok(())
    }
}
fn read(data: &[u8]) -> Result<(Vec<V>, Vec<Vec<usize>>), String> {
    let mut r = Reader { data, pos: 0 };
    if r.take(4)? != b"MLOD"
        || r.u32()? != 257
        || r.u32()? != 1
        || r.take(4)? != b"P3DM"
        || r.u32()? != 28
        || r.u32()? != 256
    {
        return Err("add-mesh needs a single MLOD LOD".into());
    }
    let np = r.u32()?;
    let nn = r.u32()?;
    let nf = r.u32()?;
    r.u32()?;
    let mut points = Vec::with_capacity(np);
    for _ in 0..np {
        let p = V(r.f32()?, r.f32()?, r.f32()?);
        r.u32()?;
        points.push(p);
    }
    r.take(nn.checked_mul(12).ok_or("MLOD overflow")?)?;
    let mut faces = Vec::with_capacity(nf);
    for _ in 0..nf {
        let n = r.u32()?;
        if n != 3 && n != 4 {
            return Err("add-mesh source has a face that is not a triangle or quad".into());
        }
        let mut f = Vec::new();
        for k in 0..4 {
            let point = r.u32()?;
            r.take(12)?;
            if k < n {
                if point >= np {
                    return Err("invalid point index".into());
                }
                f.push(point);
            }
        }
        r.u32()?;
        r.string()?;
        r.string()?;
        faces.push(f);
    }
    Ok((points, faces))
}
fn numbers(op: &Value, name: &str, n: usize) -> Result<Vec<f32>, String> {
    let a = op
        .get(name)
        .and_then(Value::as_array)
        .filter(|a| a.len() == n)
        .ok_or_else(|| format!("invalid {name}"))?;
    a.iter()
        .map(|v| {
            v.as_f64()
                .map(|x| x as f32)
                .filter(|x| x.is_finite())
                .ok_or_else(|| format!("invalid {name}"))
        })
        .collect()
}
fn key(p: V) -> (i32, i32, i32) {
    (
        (p.0 * 1e4).round_ties_even() as i32,
        (p.1 * 1e4).round_ties_even() as i32,
        (p.2 * 1e4).round_ties_even() as i32,
    )
}
fn smooth(tris: Vec<[usize; 3]>, points: &[V]) -> Vec<Vec<Corner>> {
    let area: Vec<_> = tris
        .iter()
        .map(|f| (points[f[1]] - points[f[0]]).cross(points[f[2]] - points[f[0]]))
        .collect();
    let facing: Vec<_> = area.iter().map(|&v| v.unit()).collect();
    let mut around: HashMap<_, Vec<usize>> = HashMap::new();
    for (i, f) in tris.iter().enumerate() {
        for &p in f {
            around.entry(key(points[p])).or_default().push(i);
        }
    }
    let hard = (std::f32::consts::PI * 40.0 / 180.0).cos();
    tris.iter()
        .enumerate()
        .map(|(i, f)| {
            f.iter()
                .map(|&p| {
                    let mut sum = V::default();
                    let mut seen = HashSet::new();
                    for &h in &around[&key(points[p])] {
                        if seen.insert(h) && facing[h].dot(facing[i]) > hard {
                            sum = sum + area[h];
                        }
                    }
                    Corner {
                        point: p,
                        normal: sum.unit(),
                    }
                })
                .collect()
        })
        .collect()
}
pub fn load(data: &[u8], op: &Value, frame: &Frame) -> Result<Mesh, String> {
    let (src, faces) = read(data)?;
    let a = numbers(op, "sourceAxis", 6)?;
    let origin = V(a[0], a[1], a[2]);
    let fwd = V(a[3], a[4], a[5]).unit();
    let dir = V::from(frame.direction);
    let target_origin = V::from(frame.origin);
    let y = V(0., 1., 0.);
    let up = (y - fwd * y.dot(fwd)).unit();
    let right = up.cross(fwd);
    let target_up = (y - dir * y.dot(dir)).unit();
    let target_right = target_up.cross(dir);
    let rotate = |v: V| {
        let x = v.dot(right);
        let y = v.dot(up);
        let z = v.dot(fwd);
        V(
            z.mul_add(dir.0, y.mul_add(target_up.0, x * target_right.0)),
            z.mul_add(dir.1, y.mul_add(target_up.1, x * target_right.1)),
            z.mul_add(dir.2, y.mul_add(target_up.2, x * target_right.2)),
        )
    };
    let scale = axis_common::number(op, "scale")?;
    let attach = target_origin + dir * axis_common::number(op, "attachAlong")?;
    let loops = op
        .get("loopRadii")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();
    let weld = op
        .get("weldLoops")
        .and_then(Value::as_array)
        .ok_or("missing weldLoops")?;
    let on_loop = |p: V, l: &Value| -> bool {
        let Some(a) = l.as_array() else { return false };
        if a.len() < 2 {
            return false;
        }
        let Some(x) = a[0].as_f64() else { return false };
        let Some(r) = a[1].as_f64() else { return false };
        let rel = p - origin;
        let along = rel.dot(fwd);
        (along - (x as f32 - origin.dot(fwd))).abs() < 5e-4
            && ((rel - fwd * along).len() - r as f32).abs() < 5e-4
    };
    if weld.iter().any(|l| !src.iter().any(|&p| on_loop(p, l))) {
        return Err("an add-mesh weld loop matches no source point".into());
    }
    let welded = src
        .iter()
        .map(|&p| weld.iter().any(|l| on_loop(p, l)))
        .collect();
    let mut points = Vec::new();
    for &p in &src {
        let rel = p - origin;
        let axial = fwd * rel.dot(fwd);
        let mut radial = (rel - axial) * scale;
        for l in loops.iter().filter(|l| on_loop(p, l)) {
            let radius = l
                .get(2)
                .and_then(Value::as_f64)
                .ok_or("invalid loop radius")? as f32;
            radial = radial.unit() * radius;
        }
        points.push(attach + rotate(axial * scale + radial));
    }
    let mut tris = Vec::new();
    for f in faces {
        tris.push([f[0], f[1], f[2]]);
        if f.len() == 4 {
            tris.push([f[0], f[2], f[3]]);
        }
    }
    if tris.is_empty() {
        return Err("add-mesh source has no faces".into());
    }
    let centroid = |f: &[usize; 3]| (points[f[0]] + points[f[1]] + points[f[2]]) / 3.0;
    let rim = tris
        .iter()
        .map(|f| frame.radius(centroid(f).array()))
        .fold(0f32, f32::max)
        * 0.9;
    let outward: i32 = tris
        .iter()
        .filter(|f| frame.radius(centroid(f).array()) > rim)
        .map(|f| {
            let c = centroid(f);
            let v = (points[f[1]] - points[f[0]])
                .cross(points[f[2]] - points[f[0]])
                .dot(c - target_origin - dir * frame.along(c.array()));
            if v > 0.0 {
                1
            } else if v < 0.0 {
                -1
            } else {
                0
            }
        })
        .sum();
    if outward < 0 {
        for t in &mut tris {
            t.swap(1, 2);
        }
    }
    Ok(Mesh {
        faces: smooth(tris, &points),
        points,
        weld: welded,
    })
}
