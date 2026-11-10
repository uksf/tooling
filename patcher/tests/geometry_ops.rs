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
