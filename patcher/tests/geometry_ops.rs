use serde_json::json;
use uksf_patcher::{face_ops::remove_faces, odol::Model, stretch_ops::stretch};

#[test]
fn removes_faces_and_updates_section_ranges() {
    let data = include_bytes!("data/cable.p3d");
    let op = json!({"lod":"1","axis":[0,0,0,0,0,1],"allBeyond":-100,"anyBeyond":-100,"faces":90});
    let result = remove_faces(data, &op, "fixture").unwrap();
    let model = Model::parse(&result).unwrap();
    assert_eq!(model.lods[0].faces.len(), 0);
    assert_eq!(model.lods[0].face_bytes, 0);
    assert_eq!(model.lods[0].sections[0].face_lower, 0);
    assert_eq!(model.lods[0].sections[0].face_upper, 0);
    let bad = json!({"lod":"1","axis":[0,0,0,0,0,1],"allBeyond":-100,"anyBeyond":-100,"faces":89});
    assert!(
        remove_faces(data, &bad, "fixture")
            .unwrap_err()
            .contains("matches 90 faces")
    );
}

#[test]
fn stretches_vertices_and_checks_expected_count() {
    let data = include_bytes!("data/cable.p3d");
    let op = json!({"lods":["1"],"axis":[0,0,0,0,1,0],"cut":-100,"length":1,"vertices":82});
    let result = stretch(data, &op, "fixture").unwrap();
    let model = Model::parse(&result).unwrap();
    assert_eq!(model.lods[0].vertices[0], [0.0022961006, 1.0, -0.030543277]);
    let bad = json!({"lods":["1"],"axis":[0,0,0,0,1,0],"cut":-100,"length":1,"vertices":81});
    assert!(
        stretch(data, &bad, "fixture")
            .unwrap_err()
            .contains("moved 82 vertices")
    );
}

#[test]
fn removes_casing_without_removing_overlapping_shell() {
    let data = include_bytes!("data/round.p3d");
    let op = json!({"lod":"1","axis":[0,0,0,0,-1,0],"wholeParts":true,"allBeyond":-0.04,"anyBeyond":-0.04,"faces":124});
    let result = remove_faces(data, &op, "round").unwrap();
    let model = Model::parse(&result).unwrap();
    let lod = model.lods.iter().find(|l| l.resolution == 1.0).unwrap();
    assert_eq!(lod.faces.len(), 540);
    let y: Vec<_> = lod
        .faces
        .iter()
        .flatten()
        .map(|&v| lod.vertices[v as usize][1])
        .collect();
    assert!((y.iter().copied().fold(f32::INFINITY, f32::min) - -0.043).abs() < 0.001);
    assert!((y.iter().copied().fold(f32::NEG_INFINITY, f32::max) - 0.457).abs() < 0.001);
}

#[test]
fn rebuilds_unskinned_shadow_and_refuses_textured_source() {
    let data = include_bytes!("data/round.p3d");
    let op = json!({"lod":"10002","from":"11002"});
    let result = uksf_patcher::shadow_ops::rebuild(data, &op, "round").unwrap();
    let model = Model::parse(&result).unwrap();
    let original = Model::parse(data).unwrap();
    let lod = model.lods.iter().find(|l| l.resolution == 10002.0).unwrap();
    let old = original
        .lods
        .iter()
        .find(|l| l.resolution == 10002.0)
        .unwrap();
    let faces = |lod: &uksf_patcher::odol::Lod| {
        let mut faces: Vec<_> = lod
            .faces
            .iter()
            .map(|face| {
                face.iter()
                    .map(|&v| {
                        let v = v as usize;
                        (lod.vertices[v].map(f32::to_bits), lod.clip[v])
                    })
                    .collect::<Vec<_>>()
            })
            .collect();
        faces.sort();
        faces
    };
    assert_eq!(lod.faces.len(), 2656);
    assert_eq!(faces(lod), faces(old));
    assert!(lod.bones.is_empty());
    let bad = json!({"lod":"10002","from":"1"});
    assert!(
        uksf_patcher::shadow_ops::rebuild(data, &bad, "round")
            .unwrap_err()
            .contains("a LOD has tangents")
    );
}

#[test]
fn splits_shared_memory_point_and_keeps_donor_binding() {
    let data = include_bytes!("data/rhicc-memory.p3d");
    let op = json!({"name":"muzzle_3","position":[0.860060871,0.0414807796,-2.22018695],"boneFrom":"chamber_3"});
    let before = Model::parse(data).unwrap();
    let after = uksf_patcher::memory_ops::split_memory_point(data, &op, "rhicc").unwrap();
    let parsed = Model::parse(&after).unwrap();
    let old = &before.lods[0];
    let lod = &parsed.lods[0];
    let point = |name: &[u8]| {
        lod.selections
            .iter()
            .find(|s| s.name == name)
            .unwrap()
            .vertices[0] as usize
    };
    let muzzle = point(b"muzzle_3");
    let chamber = point(b"chamber_3");
    assert_eq!(muzzle, old.vertices.len());
    assert_eq!(
        lod.vertices[muzzle],
        [0.860_060_9, 0.041_480_78, -2.220_187]
    );
    assert_eq!(lod.clip[muzzle], lod.clip[chamber]);
    assert_eq!(lod.bones[muzzle].count, lod.bones[chamber].count);
    assert_eq!(lod.bones[muzzle].data, lod.bones[chamber].data);
    assert!(
        uksf_patcher::memory_ops::split_memory_point(&after, &op, "rhicc")
            .unwrap_err()
            .contains("use set-memory-point")
    );
}

#[test]
fn moves_muzzle_memory_point_with_the_brake() {
    let data = include_bytes!("data/m119-memory.p3d");
    let op = json!({"lods":["memory"],"axis":[0,0.885,1.636,0,0.0487,-0.9988],"cut":3.5,"maxRadius":0.3,"length":0.74,"vertices":4});
    let before = Model::parse(data).unwrap();
    let after = Model::parse(&stretch(data, &op, "m119").unwrap()).unwrap();
    let muzzle = before.lods[0]
        .selections
        .iter()
        .find(|s| s.name == b"usti_hlavne")
        .unwrap()
        .vertices[0] as usize;
    let old = before.lods[0].vertices[muzzle];
    let new = after.lods[0].vertices[muzzle];
    assert!((new[1] - old[1] - 0.036_038).abs() < 0.0001);
    assert!((new[2] - old[2] + 0.739_122).abs() < 0.0001);
}
