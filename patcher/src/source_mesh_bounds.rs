use super::{Mesh, load, read};
use crate::axis_common::Frame;
use serde_json::{Value, json};

#[test]
fn mlod_rejects_huge_point_count() {
    let mut data = Vec::new();
    data.extend_from_slice(b"MLOD");
    data.extend_from_slice(&257u32.to_le_bytes());
    data.extend_from_slice(&1u32.to_le_bytes());
    data.extend_from_slice(b"P3DM");
    data.extend_from_slice(&28u32.to_le_bytes());
    data.extend_from_slice(&256u32.to_le_bytes());
    data.extend_from_slice(&0xFFFF_FFF0u32.to_le_bytes());
    data.extend_from_slice(&0u32.to_le_bytes());
    data.extend_from_slice(&0u32.to_le_bytes());
    data.extend_from_slice(&0u32.to_le_bytes());
    data.extend_from_slice(&[0; 8]);
    assert_eq!(
        read(&data).unwrap_err(),
        "count 4294967280 of 16-byte elements exceeds the remaining 8 bytes"
    );
}

const POINTS: [[f32; 3]; 4] = [[1., 0., 0.], [0., 1., 0.], [0., 0., 1.], [1., 1., 1.]];

fn faces(count: i32) -> Vec<u8> {
    let mut d = Vec::new();
    for v in [
        b"MLOD".as_slice(),
        &257u32.to_le_bytes(),
        &1u32.to_le_bytes(),
        b"P3DM",
    ] {
        d.extend_from_slice(v);
    }
    for n in [28, 256, 4, 0, 1, 0] {
        d.extend_from_slice(&(n as u32).to_le_bytes());
    }
    for p in POINTS {
        p.iter().for_each(|x| d.extend_from_slice(&x.to_le_bytes()));
        d.extend_from_slice(&[0; 4]);
    }
    d.extend_from_slice(&count.to_le_bytes());
    for k in 0..4u32 {
        d.extend_from_slice(&k.to_le_bytes());
        d.extend_from_slice(&[0; 12]);
    }
    d.extend_from_slice(b"\0\0\0\0tex\0mat\0");
    d
}

fn tag(name: &[u8], data: &[u8]) -> Vec<u8> {
    [&[1], name, &[0], &(data.len() as u32).to_le_bytes(), data].concat()
}

fn source(count: i32, tags: &[Vec<u8>]) -> Vec<u8> {
    let eof = tag(b"#EndOfFile#", &[]);
    [
        faces(count),
        b"TAGG".to_vec(),
        tags.concat(),
        eof,
        1f32.to_le_bytes().to_vec(),
    ]
    .concat()
}

fn corners(data: &[u8]) -> Vec<usize> {
    read(data).unwrap().1.concat()
}

#[test]
fn mlod_suffix_is_read_through_the_resolution() {
    let valid = source(4, &[]);
    assert_eq!(corners(&valid), [0, 1, 2, 3]);
    let tagg = faces(4).len();
    let mut marker = valid.clone();
    marker[tagg + 3] = b'X';
    assert_eq!(read(&faces(4)).unwrap_err(), "truncated MLOD");
    assert_eq!(
        read(&marker).unwrap_err(),
        "TAGG expected in the add-mesh source"
    );
    assert_eq!(
        read(&valid[..tagg + 9]).unwrap_err(),
        "unterminated MLOD string"
    );
    assert_eq!(
        read(&valid[..valid.len() - 4]).unwrap_err(),
        "truncated MLOD"
    );
    assert_eq!(
        read(&valid[..valid.len() - 2]).unwrap_err(),
        "truncated MLOD"
    );
    let mut off = valid.clone();
    off[tagg + 4] = 0;
    assert_eq!(
        read(&off).unwrap_err(),
        "deactivated tagg in the add-mesh source"
    );
}

#[test]
fn mlod_tags_are_skipped_by_their_library_sizes() {
    let uv = |count: usize| tag(b"#UVSet#", &vec![0; 4 + 8 * count]);
    let tags = [
        tag(b"sel", &[1; 5]),
        tag(b"#Property#", &[0; 128]),
        tag(b"#Mass#", &[0; 16]),
        tag(b"#SharpEdges#", &[0; 8]),
        tag(b"#Animation#", &[0; 16]),
        uv(5),
    ];
    assert_eq!(corners(&source(5, &tags)), [0, 1, 2, 3]);
    assert_eq!(read(&source(5, &[uv(4)])).unwrap_err(), "truncated MLOD");
    assert_eq!(
        read(&source(-1, &[uv(0)])).unwrap_err(),
        "negative face vertex count"
    );
}

const ON_LOOPS: &str =
    r#"{"sourceAxis":[0,0,0,0,0,1],"axis":[0,0,0,0,0,1],"scale":1,"attachAlong":0,"weldLoops":[]}"#;

fn mesh(data: &[u8], loops: Option<Value>) -> Result<Mesh, String> {
    let mut op: Value = serde_json::from_str(ON_LOOPS).unwrap();
    if let Some(l) = loops {
        op["loopRadii"] = l;
    }
    load(data, &op, &Frame::from(&op).unwrap())
}

#[test]
fn declared_face_counts_take_up_to_four_stored_corners() {
    assert_eq!(corners(&source(5, &[])), [0, 1, 2, 3]);
    assert_eq!(corners(&source(i32::MAX, &[])), [0, 1, 2, 3]);
    assert_eq!(corners(&source(3, &[])), [0, 1, 2]);
    assert_eq!(mesh(&source(i32::MAX, &[]), None).unwrap().faces.len(), 2);
    for count in [0, 1, 2, -1, i32::MIN] {
        assert_eq!(
            mesh(&source(count, &[]), None).err().unwrap(),
            "add-mesh source has a face that is not a triangle or quad"
        );
    }
}

#[test]
fn malformed_loop_radii_are_rejected() {
    let data = source(4, &[]);
    for loops in [
        json!(null),
        json!(3),
        json!({}),
        json!([5]),
        json!([[0]]),
        json!([["x", 1, 2]]),
        json!([[0, "x", 2]]),
    ] {
        let err = mesh(&data, Some(loops.clone())).err();
        assert!(err.is_some(), "{loops} was accepted");
    }
    assert_eq!(
        mesh(&data, Some(json!([[0]]))).err().unwrap(),
        "invalid add-mesh loop [0]"
    );
}

#[test]
fn loop_radii_reshape_matching_points() {
    let data = source(4, &[]);
    let plain = mesh(&data, None).unwrap().points;
    assert_eq!(plain[0].array(), [1., 0., 0.]);
    let shaped = mesh(&data, Some(json!([[0, 1, 2], [5, 1, 9]])))
        .unwrap()
        .points;
    let shaped: Vec<_> = shaped.iter().map(|p| p.array()).collect();
    assert_eq!(
        shaped,
        [[2., 0., 0.], [0., 2., 0.], [0., 0., 1.], [1., 1., 1.]]
    );
}

#[test]
fn add_mesh_with_no_lods_returns_the_model_unchanged() {
    let dir = std::env::temp_dir().join(format!("uksf-add-mesh-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(dir.join("s.p3d"), source(4, &[])).unwrap();
    let mut op: Value = serde_json::from_str(ON_LOOPS).unwrap();
    op["lods"] = json!([]);
    op["source"] = json!("s.p3d");
    op["uvFrom"] = json!({});
    let scope = include_bytes!("../tests/data/scope.p3d");
    let out = crate::mesh_ops::add(scope, &op, "t", &dir).unwrap();
    assert_eq!(out, scope);
}

#[test]
fn loop_coordinates_accept_overflow_as_nonmatching() {
    let data = source(4, &[]);
    for loops in ["[[1e39,1,2]]", "[[0,-1e39,2]]"] {
        let loops = serde_json::from_str(loops).unwrap();
        let result = mesh(&data, Some(loops)).unwrap();
        assert_eq!(result.points[0].array(), [1., 0., 0.]);
    }
}

#[test]
fn loop_radius_accepts_overflow() {
    let data = source(4, &[]);
    let loops = serde_json::from_str("[[0,1,1e39]]").unwrap();
    let result = mesh(&data, Some(loops)).unwrap();
    assert!(result.points[0].array().iter().any(|x| !x.is_finite()));
}
