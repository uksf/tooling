use crate::{bind_ops, odol::Model, op_tests_mesh::rigged};
use serde_json::{Value, json};

const EMPTY_BOX: [i32; 6] = [90, 91, 90, 91, 90, 91];
const ALL_BOX: [i32; 6] = [-9, 9, -9, 9, -9, 9];

fn mapped(subs: usize, b: i32, c: Option<i32>) -> Vec<u8> {
    let mut m = Model::parse(&rigged(1)).unwrap();
    let lod = &mut m.lods[0];
    lod.sub_to_skeleton.resize(subs, 0);
    lod.skeleton_to_sub[1] = vec![b];
    lod.skeleton_to_sub[2] = c.into_iter().collect();
    m.write().unwrap()
}

fn rebind(
    bytes: &[u8],
    from: &str,
    to: &str,
    bounds: [i32; 6],
    vertices: i32,
) -> Result<Model, String> {
    let op = json!({"lod":"0","section":1,"from":from,"to":to,"box":bounds,"vertices":vertices});
    bind_ops::rebind_vertices(bytes, &op, "t").map(|out| Model::parse(&out).unwrap())
}

#[test]
fn rebind_vertices_accepts_an_existing_from_mapping_above_a_byte() {
    let bytes = mapped(257, 256, Some(2));
    let m = rebind(&bytes, "b", "c", EMPTY_BOX, 0).unwrap();
    assert_eq!(m.lods[0].skeleton_to_sub[1], vec![256]);
    assert_eq!(m.lods[0].bones[19].data[0], 1);
}

#[test]
fn rebind_vertices_accepts_an_existing_to_mapping_above_a_byte() {
    let bytes = mapped(257, 1, Some(256));
    let m = rebind(&bytes, "b", "c", EMPTY_BOX, 0).unwrap();
    assert_eq!(m.lods[0].bones[19].data[0], 1);
    assert_eq!(m.lods[0].sub_to_skeleton.len(), 257);
}

#[test]
fn rebind_vertices_truncates_an_existing_to_mapping_like_a_byte_cast() {
    let bytes = mapped(257, 1, Some(256));
    let m = rebind(&bytes, "b", "c", ALL_BOX, 4).unwrap();
    assert_eq!(m.lods[0].bones[19].data[0], 0);
    assert_eq!(m.lods[0].bones[16].data[0], 0);
}

#[test]
fn rebind_vertices_rejects_a_from_mapping_above_a_byte_by_weight() {
    let bytes = mapped(257, 256, Some(2));
    let err = rebind(&bytes, "b", "c", ALL_BOX, 4).err().unwrap();
    assert_eq!(err, "t: vertex 18 is not weighted to 'b' alone");
}

#[test]
fn rebind_vertices_byte_boundary_controls() {
    let m = rebind(&mapped(257, 1, Some(255)), "b", "c", ALL_BOX, 4).unwrap();
    assert_eq!(m.lods[0].bones[19].data[0], 255);
    let err = rebind(&mapped(256, 1, None), "b", "c", ALL_BOX, 4)
        .err()
        .unwrap();
    assert_eq!(
        err,
        "t: bone 'c' would get sub-skeleton index 256, which a vertex weight cannot hold"
    );
}

fn with_faces(faces: &[[i32; 4]]) -> Vec<u8> {
    let mut m = Model::parse(&rigged(1)).unwrap();
    let lod = &mut m.lods[0];
    for (f, face) in faces.iter().enumerate() {
        lod.faces[f] = face.to_vec();
    }
    m.write().unwrap()
}

fn translate_error(faces: &[[i32; 4]]) -> String {
    let op: Value = json!({"lod":"0","selection":"#1","offset":[0,0,1]});
    bind_ops::translate_selection(&with_faces(faces), &op, "t").unwrap_err()
}

#[test]
fn translate_selection_counts_every_outside_occurrence() {
    let msg = |n: usize| format!("t: {n} vertices of '#1' are shared with other faces");
    assert_eq!(translate_error(&[[0, 16, 2, 3], [0, 1, 16, 3]]), msg(2));
    assert_eq!(translate_error(&[[16, 16, 2, 3]]), msg(2));
    assert_eq!(translate_error(&[[0, 16, 2, 3]]), msg(1));
}

#[test]
fn rebind_vertices_keeps_counting_distinct_shared_vertices() {
    let bytes = with_faces(&[[0, 16, 2, 3], [16, 16, 2, 3]]);
    let err = rebind(&bytes, "b", "c", ALL_BOX, 4).err().unwrap();
    assert_eq!(
        err,
        "t: 1 of the vertices are shared with faces outside section 1"
    );
}
