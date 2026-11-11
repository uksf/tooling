use crate::{
    bind_ops, cut_seams,
    odol::{Bone, BoneWeight, Model, lod_edit},
    physx_ops, selection_ops, uv_split,
};
use serde_json::{Value, json};
use std::path::PathBuf;

type Op = fn(&[u8], &Value, &str) -> Result<Vec<u8>, String>;

const SCOPE: &[u8] = include_bytes!("../tests/data/scope.p3d");
const TEXTURE: &str = "#(argb,8,8,3)color(0,0,0,1)";

fn apply(op: Op, bytes: &[u8], args: Value) -> Model {
    Model::parse(&op(bytes, &args, "t").unwrap()).unwrap()
}

fn rigged(section_one_bone: i32) -> Vec<u8> {
    let mut m = Model::parse(SCOPE).unwrap();
    m.info.skeleton.name = b"skel".to_vec();
    m.info.skeleton.bones = ["a", "b", "c"]
        .map(|n| Bone {
            name: n.as_bytes().to_vec(),
            parent: Vec::new(),
        })
        .into();
    let lod = &mut m.lods[0];
    lod.sub_to_skeleton = vec![0, 1];
    lod.skeleton_to_sub = vec![vec![0], vec![1], vec![]];
    let bones = (0..20)
        .map(|v| match (v, section_one_bone) {
            (16.., -1) => BoneWeight {
                count: 0,
                data: [0; 8],
            },
            (16.., b) => BoneWeight {
                count: 1,
                data: [b as u8, 255, 0, 0, 0, 0, 0, 0],
            },
            _ => BoneWeight {
                count: 1,
                data: [0, 255, 0, 0, 0, 0, 0, 0],
            },
        })
        .collect();
    lod_edit::set_bones(lod, bones).unwrap();
    lod.sections[1].min_bone = 1;
    lod.sections[1].bones = 1;
    lod_edit::refresh_rest(lod).unwrap();
    m.write().unwrap()
}

fn plan(name: &str, value: Value) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("uksf-op-tests-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(dir.join(name), value.to_string()).unwrap();
    dir
}

#[test]
fn retexture_renames_matching_textures_case_insensitively() {
    let m = apply(
        selection_ops::retexture,
        SCOPE,
        json!({"from":"X\\TBD_M119\\addons\\m119\\tbd_m119\\data\\M119SCOPE.paa","to":"new.paa"}),
    );
    assert_eq!(m.lods[0].textures[0], b"new.paa");
}

#[test]
fn set_section_texture_reuses_or_appends_a_texture() {
    let m = apply(
        selection_ops::set_section_texture,
        SCOPE,
        json!({"lod":"0","section":2,"texture":TEXTURE}),
    );
    assert_eq!(m.lods[0].sections[2].texture, 1);
    let m = apply(
        selection_ops::set_section_texture,
        SCOPE,
        json!({"lod":"0","section":2,"texture":"extra.paa"}),
    );
    assert_eq!(
        (m.lods[0].sections[2].texture, m.lods[0].textures.len()),
        (3, 4)
    );
}

#[test]
fn move_sections_collects_textured_sections_into_a_selection() {
    let m = apply(
        selection_ops::move_sections,
        SCOPE,
        json!({"lod":"0","texture":TEXTURE,"newSelection":"camo_pilot","from":[]}),
    );
    let s = m.lods[0].selections.last().unwrap();
    assert_eq!(
        (s.name.as_slice(), s.sections.as_slice(), s.is_sectional),
        (&b"camo_pilot"[..], &[0][..], 1)
    );
}

#[test]
fn translate_selection_moves_only_the_section_vertices() {
    let before = Model::parse(SCOPE).unwrap();
    let m = apply(
        bind_ops::translate_selection,
        SCOPE,
        json!({"lod":"0","selection":"#1","offset":[0,0,1.5]}),
    );
    assert_eq!(
        m.lods[0].vertices[16][2],
        before.lods[0].vertices[16][2] + 1.5
    );
    assert_eq!(m.lods[0].vertices[0], before.lods[0].vertices[0]);
}

#[test]
fn bind_selection_weights_unbound_vertices_and_fits_the_window() {
    let m = apply(
        bind_ops::bind_selection,
        &rigged(-1),
        json!({"lod":"0","selection":"#1","bone":"B"}),
    );
    let lod = &m.lods[0];
    assert_eq!(
        (lod.bones[17].count, lod.bones[17].data[..2].to_vec()),
        (1, vec![1, 255])
    );
    assert_eq!((lod.sections[1].min_bone, lod.sections[1].bones), (1, 1));
}

#[test]
fn rebind_vertices_adds_a_sub_bone_and_moves_the_window() {
    let m = apply(
        bind_ops::rebind_vertices,
        &rigged(1),
        json!({"lod":"0","section":1,"from":"b","to":"c","box":[-9,9,-9,9,-9,9],"vertices":4}),
    );
    let lod = &m.lods[0];
    assert_eq!(
        (lod.sub_to_skeleton.clone(), lod.skeleton_to_sub[2].clone()),
        (vec![0, 1, 2], vec![2])
    );
    assert_eq!(lod.bones[19].data[0], 2);
    assert_eq!((lod.sections[1].min_bone, lod.sections[1].bones), (2, 1));
}

#[test]
fn cut_seams_copies_a_vertex_and_repoints_the_face() {
    let dir = plan(
        "cuts.json",
        json!({"copies":[8],"faces":[{"verts":[0,8,10,1],"map":{"8":20}}]}),
    );
    let mut model = Model::parse(SCOPE).unwrap();
    let lod = &mut model.lods[0];
    let st: Vec<u8> = (0..160u8).collect();
    lod_edit::set_st(lod, &st).unwrap();
    lod_edit::refresh_rest(lod).unwrap();
    let bytes = model.write().unwrap();
    let m = Model::parse(
        &cut_seams::cut_seams(&bytes, &json!({"lod":"0","source":"cuts.json"}), "t", &dir).unwrap(),
    )
    .unwrap();
    let lod = &m.lods[0];
    assert_eq!((lod.vertex_count, lod.vertices[20]), (21, lod.vertices[8]));
    assert_eq!(lod.faces[3], [0, 20, 10, 1]);
    assert_eq!(lod.uv[0].vertices, 21);
    let st = crate::odol::mesh_arrays::st(lod).unwrap();
    assert_eq!(st[160..], st[64..72]);
}

#[test]
fn uv_split_moves_a_whole_section_to_sheet_b() {
    let mut model = Model::parse(SCOPE).unwrap();
    let camo = lod_edit::new_selection(b"camo".to_vec(), true, &[0], &[], &[]).unwrap();
    model.lods[0].selections.push(camo);
    let island = json!({"islands":[{"sheet":"B","verts":[0,1,2,3,8,10,13,15],"s":0.5,"box":[0,0],"pos":[0,0]}]});
    let dir = plan("camo.json", island);
    let op = json!({"lod":"0","source":"camo.json","selection":"camo","newSelection":"camo_lower","texture":"lower.paa"});
    let out = uv_split::uv_split(&model.write().unwrap(), &op, "t", &dir).unwrap();
    let lod = &Model::parse(&out).unwrap().lods[0];
    assert_eq!(
        (lod.sections[0].texture, lod.textures[3].as_slice()),
        (3, &b"lower.paa"[..])
    );
    let ratio = lod.sections[0].area_over_tex[0] / (84.466156 * 4.0);
    assert!((ratio - 1.0).abs() < 1e-4, "{ratio}");
    let names: Vec<_> = lod
        .selections
        .iter()
        .map(|s| (s.name.clone(), s.sections.clone()))
        .collect();
    assert_eq!(
        names,
        [
            (b"camo".to_vec(), vec![]),
            (b"camo_lower".to_vec(), vec![0])
        ]
    );
}

#[test]
fn add_physx_clones_geometry_after_it_and_sets_mass() {
    let before = Model::parse(include_bytes!("../tests/data/round.p3d")).unwrap();
    let m = apply(
        physx_ops::add_physx,
        include_bytes!("../tests/data/round.p3d"),
        json!({"mass":11000}),
    );
    assert_eq!((m.lods.len(), m.lods[5].resolution), (8, 4e13));
    assert_eq!(m.lods[5].faces, before.lods[4].faces);
    assert_eq!((m.info.lod_index[1], m.info.total_mass), (5, 11000.0));
    assert_eq!(m.info.inv_mass, 1.0 / 11000.0);
    assert_eq!(m.info.preferred_shadow_volume[5], -1);
}
