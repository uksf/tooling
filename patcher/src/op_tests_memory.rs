use crate::{
    anim_ops, memory_edit_ops,
    odol::{Model, Proxy},
    selection_ops,
};
use serde_json::{Value, json};

type Op = fn(&[u8], &Value, &str) -> Result<Vec<u8>, String>;

const RHICC: &[u8] = include_bytes!("../tests/data/rhicc-memory.p3d");

fn apply(op: Op, bytes: &[u8], args: Value) -> Model {
    Model::parse(&op(bytes, &args, "t").unwrap()).unwrap()
}

fn points(model: &Model, name: &str) -> Vec<[f32; 3]> {
    let lod = &model.lods[0];
    let s = lod
        .selections
        .iter()
        .find(|s| s.name == name.as_bytes())
        .unwrap();
    s.vertices
        .iter()
        .map(|&v| lod.vertices[v as usize])
        .collect()
}

#[test]
fn set_animation_source_rewrites_source_and_address() {
    let m = apply(
        anim_ops::set_animation_source,
        RHICC,
        json!({"animation":"DAMAGEHIDEVEZ","to":"hook","address":"loop"}),
    );
    let a = &m.animations.unwrap().classes[0];
    assert_eq!((a.source.as_slice(), a.source_address), (&b"hook"[..], 2));
}

#[test]
fn add_translation_appends_a_class_bound_to_the_bone() {
    let before = Model::parse(RHICC).unwrap();
    let bone = String::from_utf8(before.info.skeleton.bones[3].name.clone()).unwrap();
    let n = before.animations.as_ref().unwrap().classes.len();
    let m = apply(
        anim_ops::add_translation,
        RHICC,
        json!({"animation":"stretch","bone":bone,"animationSource":"s","minValue":0,"maxValue":1,
               "offset0":1.6,"offset1":4.2,"axis":[0,0,1]}),
    );
    let an = m.animations.unwrap();
    let c = &an.classes[n];
    assert_eq!(
        (c.kind, c.offset0, c.offset1, c.max_phase),
        (4, 1.6, 4.2, 1.0)
    );
    assert_eq!(an.anims_to_bones[0][n], 3);
    assert_eq!(an.axes[0][n], Some([[0.0; 3], [0.0, 0.0, 1.0]]));
    assert_eq!(an.bones_to_anims[0][3].last(), Some(&(n as u32)));
}

#[test]
fn add_memory_point_appends_an_unbound_point() {
    let m = apply(
        memory_edit_ops::add_memory_point,
        RHICC,
        json!({"name":"wheel_1_contact","position":[0.25,-0.5,1.0]}),
    );
    assert_eq!(points(&m, "wheel_1_contact"), [[0.25, -0.5, 1.0]]);
    let lod = &m.lods[0];
    assert_eq!(
        (lod.vertex_count, lod.clip[152], lod.bones[152].count),
        (153, 63, 0)
    );
}

#[test]
fn set_memory_point_moves_only_that_point() {
    let m = apply(
        memory_edit_ops::set_memory_point,
        RHICC,
        json!({"name":"watereffectl","position":[1,2,3]}),
    );
    assert_eq!(points(&m, "watereffectl"), [[1.0, 2.0, 3.0]]);
    assert_eq!(m.lods[0].vertices.len(), 152);
}

#[test]
fn split_selection_moves_upper_points_to_a_new_selection() {
    let m = apply(
        selection_ops::split_selection,
        RHICC,
        json!({"lod":"memory","selection":"pos_driver","into":"pos_driver_2","axis":0,"keepBelow":0}),
    );
    assert_eq!(points(&m, "pos_driver").len(), 2);
    assert!(points(&m, "pos_driver_2").iter().all(|p| p[0] >= 0.0));
}

#[test]
fn copy_selection_points_adds_centre_and_shrunk_points() {
    let m = apply(
        selection_ops::copy_selection_points,
        RHICC,
        json!({"lod":"memory","name":"hit_pos","fromLod":"memory","from":"osavelitele","shrink":0.5,"points":3}),
    );
    let p = points(&m, "hit_pos");
    assert_eq!(p[0][1], (1.3249755f32 + 1.3871789) / 2.0);
    assert_eq!(p[1][1], p[0][1] + (1.3249755 - p[0][1]) * 0.5);
}

#[test]
fn add_memory_proxies_builds_proxy_triangles() {
    let mut model = Model::parse(RHICC).unwrap();
    let mut visual = model.lods[0].duplicate(0.0).unwrap();
    visual.proxies.push(Proxy {
        model: b"\\a3\\pylonpod_x".to_vec(),
        transform: [1., 0., 0., 0., 1., 0., 0., 0., 1., 5., 6., 7.],
        sequence: 2,
        selection: 0,
        bone: -1,
        section: 0,
    });
    model.insert_lod(0, visual, 1, None);
    model.info.lod_index[0] = 1;
    for list in [
        &mut model.info.preferred_shadow_volume,
        &mut model.info.preferred_shadow_buffer,
        &mut model.info.preferred_shadow_buffer_vis,
    ] {
        list.insert(0, -1);
    }
    let m = apply(
        memory_edit_ops::add_memory_proxies,
        &model.write().unwrap(),
        json!({"filter":"PYLONPOD"}),
    );
    let mem = &m.lods[1];
    assert_eq!(mem.faces, [[153, 152, 154]]);
    assert_eq!(
        &mem.vertices[152..],
        [[5., 6., 7.], [5., 7., 7.], [5., 6., 7.5]]
    );
    assert_eq!(
        mem.selections.last().unwrap().name,
        b"proxy:\\a3\\pylonpod_x.002"
    );
    assert_eq!(
        (mem.sections[0].face_upper, mem.proxies[0].selection),
        (16, 144)
    );
}
