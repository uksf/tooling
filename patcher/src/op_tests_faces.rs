use crate::{
    face_ops::ranges, hull_ops::convex_hull_component, label_quads::add_label_quads, odol::Model,
    reverse_face_ops::add_reverse_face, selection_lod_ops::add_selection_lod,
};
use serde_json::{Value, json};

const PLUG: &[u8] = include_bytes!("../tests/data/plug.p3d");
const GEOMETRY: usize = 4;

type Op = fn(&[u8], &Value, &str) -> Result<Vec<u8>, String>;

fn apply(op: Op, args: Value) -> Model {
    Model::parse(&op(PLUG, &args, "t").unwrap()).unwrap()
}

#[test]
fn add_selection_lod_copies_component_faces_and_a_two_sided_quad() {
    let op = json!({"lod":"roadway","fromLod":"geometry","selections":["component01"],"faces":12,
        "quads":[{"name":"floor","points":[[1,2,4],[1,2,-4],[-1,2,-4],[-1,2,4]]}]});
    let m = apply(add_selection_lod, op);
    let (at, road) = (GEOMETRY + 1, &m.lods[GEOMETRY + 1]);
    assert_eq!((road.resolution, m.info.lod_index[9]), (3e15, at as u8));
    assert_eq!(
        (road.faces.len(), road.vertices.len(), road.sections.len()),
        (14, 12, 1)
    );
    assert_eq!(road.faces[13], vec![11, 10, 9, 8]);
    assert_eq!(road.faces[12], vec![8, 9, 10, 11]);
    assert_eq!(road.vertices[11], [-1.0, 2.0, 4.0]);
    let floor = road.selections.iter().find(|s| s.name == b"floor").unwrap();
    assert_eq!(
        (floor.faces.clone(), floor.vertices.clone()),
        (vec![12, 13], vec![8, 9, 10, 11])
    );
    assert_eq!(road.selections[0].faces, (0..12).collect::<Vec<_>>());
    let wrong =
        json!({"lod":"roadway","fromLod":"geometry","selections":["component01"],"faces":3});
    assert_eq!(
        add_selection_lod(PLUG, &wrong, "t").unwrap_err(),
        "t: selections component01 have 12 faces, the patch expects 3"
    );
}

#[test]
fn convex_hull_component_replaces_the_box_with_its_hull() {
    let m = apply(
        convex_hull_component,
        json!({"lods":["geometry"],"component":"component01"}),
    );
    let lod = &m.lods[GEOMETRY];
    assert_eq!(lod.faces.len(), 12);
    assert!(lod.faces.iter().all(|f| f.len() == 3));
    assert_eq!(lod.selections[0].faces, (0..12).collect::<Vec<_>>());
    assert_eq!(ranges(lod).unwrap(), vec![(0, 12)]);
    assert_eq!(lod.faces[0], vec![3, 1, 0]);
}

#[test]
fn convex_hull_component_refuses_a_shear_on_a_symmetric_cooked_record() {
    let op = json!({"lods":["geometry"],"component":"component01","shear":0.05});
    assert_eq!(
        convex_hull_component(PLUG, &op, "t").unwrap_err(),
        "convex cache orientation is ambiguous"
    );
    let flat = json!({"lods":["geometry"],"component":"missing"});
    assert_eq!(
        convex_hull_component(PLUG, &flat, "t").unwrap_err(),
        "t:geometry:missing: no such selection"
    );
    let steep = json!({"lods":["geometry"],"component":"component01","shear":0.3});
    assert_eq!(
        convex_hull_component(PLUG, &steep, "t").unwrap_err(),
        "t: shear must be a finite number of at most 0.2"
    );
}

#[test]
fn add_reverse_face_adds_the_opposite_winding_in_the_same_section() {
    let before = Model::parse(PLUG).unwrap();
    let face = before.lods[GEOMETRY].faces[0].clone();
    let op = json!({"lod":"geometry","face":0,"vertices":face});
    let out = add_reverse_face(PLUG, &op, "t").unwrap();
    let lod = &Model::parse(&out).unwrap().lods[GEOMETRY];
    let reversed: Vec<i32> = face.iter().rev().copied().collect();
    assert_eq!((lod.faces.len(), &lod.faces[1]), (13, &reversed));
    assert_eq!(lod.faces[2], before.lods[GEOMETRY].faces[1]);
    assert_eq!(lod.selections[0].faces, (0..13).collect::<Vec<_>>());
    assert_eq!(ranges(lod).unwrap(), vec![(0, 13)]);
    assert_eq!(
        add_reverse_face(&out, &op, "t").unwrap_err(),
        "t: face 0 already has a reverse face"
    );
    let wrong = json!({"lod":"geometry","face":0,"vertices":[face[0], face[1], 99]});
    assert_eq!(
        add_reverse_face(PLUG, &wrong, "t").unwrap_err(),
        "t: face 0 does not match the expected vertices"
    );
}

fn label(uv: Option<Value>) -> Value {
    let mut quad = json!({"normal":[-1,0,0],
        "corners":[[-0.478,0.83,-3.95],[-0.478,0.83,-4.35],[-0.478,0.78,-4.35],[-0.478,0.78,-3.95]]});
    if let Some(uv) = uv {
        quad["uv"] = uv;
    }
    json!({"lods":["1"],"texture":"#(argb,8,8,1)color(0,0,0,0)","templateSection":0,
        "labels":[{"selection":"name","quads":[quad]}]})
}

#[test]
fn add_label_quads_writes_the_custom_uv_onto_a_new_section() {
    let m = apply(
        add_label_quads,
        label(Some(json!([[0, 0], [1, 0], [1, 0.222222], [0, 0.222222]]))),
    );
    let lod = &m.lods[0];
    assert_eq!(
        (lod.vertices.len(), lod.faces.len(), lod.sections.len()),
        (964, 611, 2)
    );
    assert_eq!(lod.textures.last().unwrap(), b"#(argb,8,8,1)color(0,0,0,0)");
    let uv: Vec<[u32; 2]> = lod.uv[0].coords[960..]
        .iter()
        .map(|c| c.map(f32::to_bits))
        .collect();
    assert_eq!(
        uv,
        vec![
            [0, 0],
            [0x3f80_0000, 0],
            [0x3f80_0000, 0x3e63_8e7f],
            [0, 0x3e63_8e7f]
        ]
    );
    assert_eq!(
        m.loadable[0].as_ref().map(|l| (l.faces, l.vertices)),
        Some((611, 964))
    );
}

#[test]
fn add_label_quads_fills_the_texture_when_uv_is_omitted_and_refuses_a_short_list() {
    let m = apply(add_label_quads, label(None));
    let uv = &m.lods[0].uv[0].coords;
    assert_eq!((uv[962], uv[963]), ([1.0, 1.0], [0.0, 1.0]));
    let error = add_label_quads(PLUG, &label(Some(json!([[0, 0], [1, 0]]))), "t").unwrap_err();
    assert_eq!(error, "t: label quad uv needs 4 finite [u, v] pairs");
}
