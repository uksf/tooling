use crate::{odol::Model, op_tests_mesh::rigged, uv_ops::offset_uv};
use serde_json::json;

fn uv(bytes: &[u8]) -> Vec<[f32; 2]> {
    Model::parse(bytes).unwrap().lods[0].uv[0].coords.clone()
}

#[test]
fn offset_uv_moves_only_the_vertices_weighted_to_the_bone() {
    let base = rigged(1);
    let before = uv(&base);
    let op = json!({"op":"offset-uv","lod":"0","bone":"b","offset":[0.25,-0.125]});
    let after = uv(&offset_uv(&base, &op, "t").unwrap());
    for v in 0..20 {
        let expected = if v >= 16 {
            [before[v][0] + 0.25, before[v][1] - 0.125]
        } else {
            before[v]
        };
        for k in 0..2 {
            assert!(
                (after[v][k] - expected[k]).abs() < 1e-3,
                "vertex {v}: {:?} vs {expected:?}",
                after[v]
            );
        }
    }
}

#[test]
fn offset_uv_rejects_a_bone_without_vertices() {
    let op = json!({"op":"offset-uv","lod":"0","bone":"c","offset":[0.1,0.1]});
    let error = offset_uv(&rigged(1), &op, "t").unwrap_err();
    assert!(error.contains("does not use bone 'c'"), "{error}");
}

#[test]
fn offset_uv_rejects_faces_that_cross_the_bone_boundary() {
    let mut m = Model::parse(&rigged(1)).unwrap();
    let lod = &mut m.lods[0];
    let face = lod
        .faces
        .iter()
        .position(|f| f.iter().all(|&v| v < 16))
        .unwrap();
    lod.faces[face][0] = 16;
    let op = json!({"op":"offset-uv","lod":"0","bone":"b","offset":[0.1,0.1]});
    let error = offset_uv(&m.write().unwrap(), &op, "t").unwrap_err();
    assert!(error.contains("faces join vertices of 'b'"), "{error}");
}
