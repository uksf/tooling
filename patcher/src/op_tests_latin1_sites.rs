use crate::{
    add_points_ops::add_points,
    file_ops::text_replace,
    label_quads::add_label_quads,
    model_ops::set_skeleton_name,
    odol::{Model, lod_edit},
    point_ops::add_selection_points,
    selection_lod_ops::add_selection_lod,
    selection_ops::{move_sections, retexture, set_section_texture, split_selection},
};
use serde_json::{Value, json};

type Op = fn(&[u8], &Value, &str) -> Result<Vec<u8>, String>;

const RHICC: &[u8] = include_bytes!("../tests/data/rhicc-memory.p3d");
const SCOPE: &[u8] = include_bytes!("../tests/data/scope.p3d");
const PLUG: &[u8] = include_bytes!("../tests/data/plug.p3d");
const TEXTURE: &str = "#(argb,8,8,3)color(0,0,0,1)";
const SCOPE_TEXTURE: &str = "X\\TBD_M119\\addons\\m119\\tbd_m119\\data\\M119SCOPE.paa";

fn apply(op: Op, bytes: &[u8], args: Value) -> Model {
    Model::parse(&op(bytes, &args, "t").unwrap()).unwrap()
}

fn has_selection(m: &Model, name: &[u8]) -> bool {
    m.lods
        .iter()
        .flat_map(|l| &l.selections)
        .any(|s| s.name == name)
}

#[test]
fn add_points_writes_the_best_fit_name() {
    let op = json!({"lod":"hitpoints","name":"Hit\u{0100}","points":1,"positions":[[1,2,3]]});
    assert!(has_selection(&apply(add_points, RHICC, op), b"HitA"));
}

#[test]
fn add_selection_points_writes_the_best_fit_name() {
    let op = json!({"lod":"memory","name":"fuel\u{1F600}","points":[[1,2,3]]});
    assert!(has_selection(
        &apply(add_selection_points, RHICC, op),
        b"fuel??"
    ));
}

#[test]
fn split_selection_writes_the_best_fit_name() {
    let op = json!({"lod":"memory","selection":"pos_driver","into":"pos_\u{0100}","axis":0,"keepBelow":0});
    assert!(has_selection(&apply(split_selection, RHICC, op), b"pos_A"));
}

#[test]
fn add_selection_lod_writes_the_best_fit_quad_name() {
    let op = json!({"lod":"roadway","fromLod":"geometry","selections":["component01"],"faces":12,
        "quads":[{"name":"floor\u{0100}","points":[[1,2,4],[1,2,-4],[-1,2,-4],[-1,2,4]]}]});
    assert!(has_selection(
        &apply(add_selection_lod, PLUG, op),
        b"floorA"
    ));
}

#[test]
fn move_sections_writes_the_best_fit_name() {
    let op = json!({"lod":"0","texture":TEXTURE,"newSelection":"camo_\u{0100}","from":[]});
    assert!(has_selection(&apply(move_sections, SCOPE, op), b"camo_A"));
}

#[test]
fn retexture_writes_best_fit_and_counts_a_string_change() {
    let m = apply(
        retexture,
        SCOPE,
        json!({"from":SCOPE_TEXTURE,"to":"new\u{0100}.paa"}),
    );
    assert_eq!(m.lods[0].textures[0], b"newA.paa");
    let again = json!({"from":"newA.paa","to":"new\u{0100}.paa"});
    let out = retexture(&m.write().unwrap(), &again, "t").unwrap();
    assert_eq!(out, m.write().unwrap());
}

#[test]
fn set_section_texture_appends_best_fit_and_never_matches_it() {
    let op = json!({"lod":"0","section":2,"texture":"x\u{0100}.paa"});
    let m = apply(set_section_texture, SCOPE, op.clone());
    assert_eq!(m.lods[0].textures.last().unwrap(), b"xA.paa");
    let n = m.lods[0].textures.len();
    let again = apply(set_section_texture, &m.write().unwrap(), op);
    assert_eq!(again.lods[0].textures.len(), n + 1);
}

#[test]
fn add_label_quads_shares_one_best_fit_texture_between_labels() {
    let quad = json!({"normal":[-1,0,0],
        "corners":[[-0.478,0.83,-3.95],[-0.478,0.83,-4.35],[-0.478,0.78,-4.35],[-0.478,0.78,-3.95]]});
    let op = json!({"lods":["1"],"texture":"t\u{0100}","templateSection":0,
        "labels":[{"selection":"a\u{0100}","quads":[quad]},{"selection":"b","quads":[quad]}]});
    let before = Model::parse(PLUG).unwrap().lods[0].textures.len();
    let m = apply(add_label_quads, PLUG, op);
    let lod = &m.lods[0];
    assert_eq!(lod.textures.len(), before + 1);
    assert_eq!(lod.textures.last().unwrap(), b"tA");
    let last = lod.sections.len() - 1;
    assert_eq!(lod.sections[last].texture, lod.sections[last - 1].texture);
    assert!(has_selection(&m, b"aA"));
}

#[test]
fn set_skeleton_name_writes_the_best_fit_name() {
    let from = String::from_utf8(Model::parse(RHICC).unwrap().info.skeleton.name).unwrap();
    let out = set_skeleton_name(RHICC, &json!({"from":from,"to":"s\u{0100}"}), "t").unwrap();
    assert_eq!(Model::parse(&out).unwrap().info.skeleton.name, b"sA");
}

#[test]
fn remap_proxies_writes_best_fit_model_and_selection() {
    let mut model = Model::parse(SCOPE).unwrap();
    let lod = &mut model.lods[0];
    let s = lod_edit::new_selection(b"proxy:old.001".to_vec(), false, &[], &[], &[]).unwrap();
    lod.selections.push(s);
    lod.proxies = vec![crate::odol::Proxy {
        model: b"old".to_vec(),
        transform: [0.0; 12],
        sequence: 1,
        selection: lod.selections.len() as i32 - 1,
        bone: -1,
        section: 0,
    }];
    let rule = json!({"model":"OLD","id":1,"toModel":"n\u{0100}","toId":2});
    let out =
        crate::remap_ops::remap_proxies(&model.write().unwrap(), &json!({"rules":[rule]}), "t")
            .unwrap();
    let lod = &Model::parse(&out).unwrap().lods[0];
    assert_eq!(lod.proxies[0].model, b"na");
    assert_eq!(
        lod.selections[lod.proxies[0].selection as usize].name,
        b"proxy:na.002"
    );
}

#[test]
fn text_replace_writes_best_fit_and_two_units_for_non_bmp() {
    assert_eq!(
        text_replace(b"x", "x", "\u{0100}\u{1F600}", 1).unwrap(),
        b"A??"
    );
}
