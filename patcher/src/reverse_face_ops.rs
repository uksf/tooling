use crate::{
    axis_common::lod_index,
    face_ops::ranges,
    odol::edit_arrays,
    op_json::{edit, int, int32, string},
    section_faces::weight,
};
use serde_json::Value;

fn same_cycle(a: &[i32], b: &[i32]) -> bool {
    a.len() == b.len()
        && (0..a.len()).any(|start| (0..a.len()).all(|i| a[(start + i) % a.len()] == b[i]))
}

pub fn add_reverse_face(data: &[u8], op: &Value, label: &str) -> Result<Vec<u8>, String> {
    edit(data, label, true, |model| {
        let li = lod_index(model, string(op, "lod")?)?;
        let face = int(op, "face", -1)?;
        let expected = op
            .get("vertices")
            .and_then(Value::as_array)
            .and_then(|a| a.iter().map(int32).collect::<Option<Vec<_>>>())
            .ok_or_else(|| format!("op needs \"vertices\" as an array of integers: {op}"))?;
        let lod = &mut model.lods[li];
        let index = usize::try_from(face)
            .ok()
            .filter(|&f| f < lod.faces.len() && lod.faces[f] == expected && expected.len() >= 3)
            .ok_or_else(|| format!("face {face} does not match the expected vertices"))?;
        let reversed: Vec<i32> = expected.iter().rev().copied().collect();
        if lod.faces.iter().any(|f| same_cycle(f, &reversed)) {
            return Err(format!("face {face} already has a reverse face"));
        }
        let old = ranges(lod)?;
        let section = old
            .iter()
            .position(|&(a, b)| index >= a && index < b)
            .ok_or_else(|| format!("face {face} has no section"))?;
        let at = index + 1;
        lod.faces.insert(at, reversed);
        for s in &mut lod.selections {
            let mut selected: Vec<i32> = s
                .faces
                .iter()
                .map(|&f| if f >= at as i32 { f + 1 } else { f })
                .collect();
            if s.faces.contains(&face) {
                selected.push(at as i32);
            }
            edit_arrays::set_faces(s, selected)?;
        }
        let mut offsets = vec![0i32];
        for f in &lod.faces {
            offsets.push(offsets[offsets.len() - 1] + weight(f));
        }
        for (s, (i, &(a, b))) in lod.sections.iter_mut().zip(old.iter().enumerate()) {
            let (a, b) = if i < section {
                (a, b)
            } else if i == section {
                (a, b + 1)
            } else {
                (a + 1, b + 1)
            };
            s.face_lower = offsets[a];
            s.face_upper = offsets[b];
        }
        lod.face_bytes = offsets[offsets.len() - 1] as u32;
        Ok(())
    })
}
