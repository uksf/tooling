use super::Lod;

fn within(values: &[i32], len: usize, what: &str, target: &str) -> Result<(), String> {
    match values
        .iter()
        .find(|&&x| usize::try_from(x).map_or(true, |x| x >= len))
    {
        Some(x) => Err(format!(
            "{what} references {target} {x}, outside the LOD's {len}"
        )),
        None => Ok(()),
    }
}

fn per_vertex(len: usize, vertices: usize, what: &str) -> Result<(), String> {
    if len == vertices {
        return Ok(());
    }
    Err(format!("{what} has {len} entries for {vertices} vertices"))
}

pub fn check(lod: &Lod) -> Result<(), String> {
    let vertices = lod.vertices.len();
    for (f, face) in lod.faces.iter().enumerate() {
        within(face, vertices, &format!("face {f}"), "vertex")?;
    }
    for (i, set) in lod.uv.iter().enumerate() {
        per_vertex(set.coords.len(), vertices, &format!("UV set {i}"))?;
    }
    per_vertex(lod.normals.len(), vertices, "the normal array")?;
    for s in &lod.selections {
        let what = format!("selection '{}'", String::from_utf8_lossy(&s.name));
        within(&s.faces, lod.faces.len(), &what, "face")?;
        within(&s.sections, lod.sections.len(), &what, "section")?;
        within(&s.vertices, vertices, &what, "vertex")?;
    }
    for (i, p) in lod.proxies.iter().enumerate() {
        let what = format!("proxy {i}");
        within(&[p.selection], lod.selections.len(), &what, "selection")?;
        within(&[p.section], lod.sections.len(), &what, "section")?;
    }
    Ok(())
}
