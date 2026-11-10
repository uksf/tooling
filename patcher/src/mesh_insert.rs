use crate::{
    face_ops,
    odol::{self, Lod},
};
pub fn insert(
    lod: &mut Lod,
    new_faces: Vec<Vec<i32>>,
    band: &[usize],
    ring: &[usize],
    old_vertices: usize,
    added: usize,
) -> Result<(), String> {
    let ranges = face_ops::ranges(lod)?;
    let section = (0..ranges.len())
        .max_by_key(|&s| {
            (
                band.iter()
                    .filter(|&&f| f >= ranges[s].0 && f < ranges[s].1)
                    .count(),
                std::cmp::Reverse(s),
            )
        })
        .ok_or("no sections")?;
    let at = ranges[section].1;
    let n = new_faces.len();
    let mut faces = Vec::with_capacity(lod.faces.len() + n);
    faces.extend_from_slice(&lod.faces[..at]);
    faces.extend(new_faces);
    faces.extend_from_slice(&lod.faces[at..]);
    for selection in &mut lod.selections {
        let mut sel: Vec<_> = selection
            .faces
            .iter()
            .map(|&f| if f as usize >= at { f + n as i32 } else { f })
            .collect();
        if !sel.is_empty() && band.iter().all(|&f| selection.faces.contains(&(f as i32))) {
            sel.extend((at..at + n).map(|v| v as i32));
        }
        if !sel.is_empty() {
            odol::edit_arrays::set_faces(selection, sel)?;
        }
        if !selection.vertices.is_empty()
            && ring
                .iter()
                .all(|&v| selection.vertices.contains(&(v as i32)))
        {
            let donor = selection
                .vertices
                .iter()
                .position(|&v| v == ring[0] as i32)
                .ok_or("invalid ring")?;
            let new: Vec<_> = (old_vertices..old_vertices + added)
                .map(|v| v as i32)
                .collect();
            odol::mesh_arrays::selection_vertices(selection, &new, donor)?;
        }
    }
    let mut offsets = vec![0i32];
    for f in &faces {
        offsets.push(offsets.last().unwrap() + 4 + 4 * f.len() as i32);
    }
    for (i, (s, (a, b))) in lod.sections.iter_mut().zip(ranges).enumerate() {
        let (a, b) = if i < section {
            (a, b)
        } else if i == section {
            (a, b + n)
        } else {
            (a + n, b + n)
        };
        s.face_lower = offsets[a];
        s.face_upper = offsets[b];
    }
    lod.face_bytes = *offsets.last().unwrap() as u32;
    lod.faces = faces;
    Ok(())
}
