use serde_json::Value;

pub struct Island {
    pub on_b: bool,
    pub s: f32,
    pub bounds: Option<[f32; 2]>,
    pub pos: Option<[f32; 2]>,
    pub verts: Vec<usize>,
    pub uv: Option<Vec<Vec<f32>>>,
    pub st: Option<Vec<Vec<f32>>>,
}

fn row(e: &Value, width: usize) -> Option<Vec<f32>> {
    let a = e.as_array().filter(|a| a.len() == width)?;
    a.iter().map(crate::op_json::single).collect()
}

fn rows(
    e: &Value,
    name: &str,
    count: usize,
    width: usize,
) -> Result<Option<Vec<Vec<f32>>>, String> {
    let Some(p) = e.get(name) else {
        return Ok(None);
    };
    p.as_array()
        .filter(|a| a.len() == count)
        .and_then(|a| a.iter().map(|r| row(r, width)).collect::<Option<Vec<_>>>())
        .map(Some)
        .ok_or_else(|| format!("needs \"{name}\" as {count} rows of {width} finite numbers"))
}

fn pair(e: &Value, name: &str) -> Result<Option<[f32; 2]>, String> {
    e.get(name)
        .map(|p| {
            row(p, 2)
                .map(|r| [r[0], r[1]])
                .ok_or_else(|| format!("needs \"{name}\" as 2 finite numbers"))
        })
        .transpose()
}

fn island(e: &Value, vertex_count: usize) -> Result<Island, String> {
    let on_b = match e.get("sheet").and_then(Value::as_str) {
        Some("A") => false,
        Some("B") => true,
        _ => return Err("needs \"sheet\" A or B".into()),
    };
    let verts = e
        .get("verts")
        .and_then(Value::as_array)
        .ok_or("needs a \"verts\" array")?
        .iter()
        .map(|v| v.as_i64().ok_or("has a vertex that is not an integer"))
        .collect::<Result<Vec<_>, _>>()?;
    if verts.is_empty() || verts.iter().any(|&v| v < 0 || v >= vertex_count as i64) {
        return Err(format!("needs vertices inside the LOD's {vertex_count}"));
    }
    let verts: Vec<usize> = verts.into_iter().map(|v| v as usize).collect();
    let uv = rows(e, "uv", verts.len(), 2)?;
    let st = rows(e, "st", verts.len(), 6)?;
    if uv.is_some() {
        return Ok(Island {
            on_b,
            s: 1.0,
            bounds: None,
            pos: None,
            verts,
            uv,
            st,
        });
    }
    let s = e
        .get("s")
        .map_or(Some(1.0), crate::op_json::single)
        .ok_or("has a non-numeric \"s\"")?;
    let bounds = pair(e, "box")?;
    let pos = pair(e, "pos")?;
    if !s.is_finite() || s <= 0.0 || bounds.is_none() || pos.is_none() {
        return Err(
            "needs explicit \"uv\" rows or a positive \"s\" with \"box\" and \"pos\"".into(),
        );
    }
    Ok(Island {
        on_b,
        s,
        bounds,
        pos,
        verts,
        uv: None,
        st,
    })
}

pub fn read(plan: &Value, vertex_count: usize) -> Result<Vec<Island>, String> {
    plan.get("islands")
        .and_then(Value::as_array)
        .filter(|a| !a.is_empty())
        .ok_or("uv-split plan needs a non-empty \"islands\" array")?
        .iter()
        .enumerate()
        .map(|(n, e)| island(e, vertex_count).map_err(|m| format!("uv-split island {n} {m}")))
        .collect()
}

pub fn pack(v: &[f32]) -> [u8; 4] {
    let c = |x: f32| (f64::from(-x * 511.).round_ties_even().clamp(-511., 511.) as i32) & 0x3ff;
    (c(v[0]) | c(v[1]) << 10 | c(v[2]) << 20).to_le_bytes()
}

pub fn area(uv: &[[f32; 2]], f: &[i32]) -> f32 {
    let mut a = 0f32;
    let o = uv[f[0] as usize];
    for i in 1..f.len().saturating_sub(1) {
        let p = uv[f[i] as usize];
        let q = uv[f[i + 1] as usize];
        let e1 = [p[0] - o[0], p[1] - o[1]];
        let e2 = [q[0] - o[0], q[1] - o[1]];
        a += (e1[0] * e2[1] - e1[1] * e2[0]).abs() / 2.;
    }
    a
}

pub fn sum(values: impl Iterator<Item = f32>) -> f32 {
    values.map(f64::from).sum::<f64>() as f32
}
