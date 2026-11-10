use serde_json::json;
use uksf_patcher::{
    bone_ops::set_bone_parent,
    mlod_ops::remove_degenerate_faces,
    model_ops::set_skeleton_name,
    odol::{Bone, Model},
};

fn model() -> Vec<u8> {
    let mut model = Model::parse(include_bytes!("data/cable.p3d")).unwrap();
    model.info.skeleton.name = b"M249skel".to_vec();
    model.info.skeleton.bones = vec![
        Bone {
            name: b"root".to_vec(),
            parent: vec![],
        },
        Bone {
            name: b"turret".to_vec(),
            parent: b"root".to_vec(),
        },
    ];
    model.write().unwrap()
}
#[test]
fn renames_skeleton_and_rejects_wrong_name_and_empty_target() {
    let bytes = model();
    let result =
        set_skeleton_name(&bytes, &json!({"from":"M249skel","to":"L110skel"}), "test").unwrap();
    assert_eq!(
        Model::parse(&result).unwrap().info.skeleton.name,
        b"L110skel"
    );
    assert_eq!(bytes.len(), result.len());
    assert_eq!(bytes.iter().zip(&result).filter(|(a, b)| a != b).count(), 4);
    assert!(
        set_skeleton_name(&bytes, &json!({"from":"wrong","to":"L110skel"}), "test")
            .unwrap_err()
            .contains("the patch expects 'wrong'")
    );
    assert!(
        set_skeleton_name(&bytes, &json!({"from":"M249skel","to":""}), "test")
            .unwrap_err()
            .contains("is empty")
    );
}
#[test]
fn detaches_bone_and_refuses_cycle() {
    let bytes = model();
    let result = set_bone_parent(
        &bytes,
        &json!({"bone":"turret","from":"root","to":""}),
        "test",
    )
    .unwrap();
    assert_eq!(
        Model::parse(&result).unwrap().info.skeleton.bones[1].parent,
        b""
    );
    assert_eq!(result.len(), bytes.len() - 4);
    assert!(
        set_bone_parent(
            &bytes,
            &json!({"bone":"root","from":"","to":"turret"}),
            "test"
        )
        .unwrap_err()
        .contains("cycle")
    );
}
fn u32put(out: &mut Vec<u8>, n: u32) {
    out.extend_from_slice(&n.to_le_bytes());
}
fn face(out: &mut Vec<u8>, indexes: [u32; 3]) {
    u32put(out, 3);
    for index in indexes.into_iter().chain([0]) {
        u32put(out, index);
        out.extend_from_slice(&[0; 12]);
    }
    u32put(out, 0);
    out.extend_from_slice(b"\0\0");
}
fn fixture() -> Vec<u8> {
    let mut out = b"MLOD".to_vec();
    u32put(&mut out, 257);
    u32put(&mut out, 1);
    out.extend_from_slice(b"P3DM");
    for n in [28, 256, 3, 0, 2, 0] {
        u32put(&mut out, n);
    }
    for p in [[0f32, 0., 0.], [1., 0., 0.], [0., 1., 0.]] {
        for n in p {
            out.extend_from_slice(&n.to_le_bytes());
        }
        u32put(&mut out, 0);
    }
    face(&mut out, [0, 1, 2]);
    face(&mut out, [0, 0, 2]);
    out.extend_from_slice(b"TAGG\x01selection\0");
    u32put(&mut out, 5);
    out.extend_from_slice(&[1, 1, 1, 7, 9]);
    out.extend_from_slice(b"\x01#EndOfFile#\0");
    u32put(&mut out, 0);
    out.extend_from_slice(&0f32.to_le_bytes());
    out
}
#[test]
fn removes_degenerate_face_and_selection_weight() {
    let bytes = fixture();
    let result = remove_degenerate_faces(&bytes, &json!({"expectedRemoved":1}), "test").unwrap();
    let mut expected = bytes.clone();
    expected[32..36].copy_from_slice(&1u32.to_le_bytes());
    let first_face = 40 + 48;
    expected.drain(first_face + 74..first_face + 148);
    let size_at = first_face + 74 + 4 + 1 + 10;
    expected[size_at..size_at + 4].copy_from_slice(&4u32.to_le_bytes());
    expected.remove(size_at + 4 + 4);
    assert_eq!(result, expected);
    assert!(
        remove_degenerate_faces(&bytes, &json!({"expectedRemoved":2}), "test")
            .unwrap_err()
            .contains("found 1")
    );
    assert!(
        remove_degenerate_faces(b"ODOL", &json!({}), "test")
            .unwrap_err()
            .contains("unbinarised")
    );
}
