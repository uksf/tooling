use crate::{
    bind_ops, face_ops,
    odol::{Lod, Model, lod_edit},
    op_tests_mesh::rigged,
    shadow_ops,
};
use serde_json::{Value, json};

const SCOPE: &[u8] = include_bytes!("../tests/data/scope.p3d");
const CABLE: &[u8] = include_bytes!("../tests/data/cable.p3d");
const ROUND: &[u8] = include_bytes!("../tests/data/round.p3d");

fn corrupt(bytes: &[u8], lod: usize, change: impl FnOnce(&mut Lod), needle: &str) -> Vec<u8> {
    let mut model = Model::parse(bytes).unwrap();
    change(&mut model.lods[lod]);
    let bad = model.write().unwrap();
    let error = Model::parse(&bad).err().unwrap();
    assert!(error.contains(needle), "{error}");
    bad
}

fn rejects(result: Result<Vec<u8>, String>, needle: &str) {
    let error = result.unwrap_err();
    assert!(error.contains(needle), "{error}");
}

#[test]
fn face_vertex_out_of_range_is_rejected() {
    let n = Model::parse(CABLE).unwrap().lods[0].vertices.len() as i32;
    let needle = format!("face 0 references vertex {n}, outside the LOD's {n}");
    let bad = corrupt(CABLE, 0, |lod| lod.faces[0][0] = n, &needle);
    let op = json!({"lod":"1","axis":[0,0,0,0,0,1],"allBeyond":-100,"anyBeyond":-100,"faces":90});
    rejects(face_ops::remove_faces(&bad, &op, "t"), &needle);
}

#[test]
fn normal_count_must_match_vertices() {
    let model = Model::parse(ROUND).unwrap();
    let li = model
        .lods
        .iter()
        .position(|l| l.resolution == 11002.0)
        .unwrap();
    let n = model.lods[li].vertices.len();
    let needle = format!("the normal array has {} entries for {n} vertices", n - 1);
    let bad = corrupt(
        ROUND,
        li,
        |lod| {
            let raw = lod_edit::normals(lod).unwrap();
            lod_edit::set_normals(lod, &raw[..raw.len() - 4]).unwrap();
        },
        &needle,
    );
    let op = json!({"lod":"10002","from":"11002"});
    rejects(shadow_ops::rebuild(&bad, &op, "t"), &needle);
}

#[test]
fn uv_count_must_match_vertices() {
    corrupt(
        SCOPE,
        0,
        |lod| {
            let coords = lod.uv[0].coords.clone();
            lod_edit::write_uv(&mut lod.uv[0], &coords[1..]).unwrap();
            lod.uv[0].vertices -= 1;
        },
        "UV set 0 has",
    );
}

#[test]
fn selection_references_out_of_range_are_rejected() {
    let base = Model::parse(SCOPE).unwrap();
    let (faces, vertices) = (
        base.lods[0].faces.len() as i32,
        base.lods[0].vertices.len() as i32,
    );
    for (f, v, needle) in [
        (faces, 0, format!("selection 'bad' references face {faces}")),
        (
            0,
            vertices,
            format!("selection 'bad' references vertex {vertices}"),
        ),
        (-1, 0, "selection 'bad' references face -1".to_owned()),
    ] {
        corrupt(
            SCOPE,
            0,
            |lod| {
                let s = lod_edit::new_selection(b"bad".to_vec(), false, &[], &[f], &[v]).unwrap();
                lod.selections.push(s);
            },
            &needle,
        );
    }
}

#[test]
fn selection_section_out_of_range_is_rejected_for_bind_and_translate() {
    let sections = Model::parse(&rigged(-1)).unwrap().lods[0].sections.len() as i32;
    for index in [-1, sections] {
        let needle = format!("selection 'bad' references section {index}");
        let bad = corrupt(
            &rigged(-1),
            0,
            |lod| {
                let s = lod_edit::new_selection(b"bad".to_vec(), true, &[index], &[], &[]).unwrap();
                lod.selections.push(s);
            },
            &needle,
        );
        let bind = json!({"lod":"0","selection":"bad","bone":"b"});
        rejects(bind_ops::bind_selection(&bad, &bind, "t"), &needle);
        let translate = json!({"lod":"0","selection":"bad","offset":[0,0,1]});
        rejects(
            bind_ops::translate_selection(&bad, &translate, "t"),
            &needle,
        );
    }
    let mut model = Model::parse(&rigged(-1)).unwrap();
    let lod = &mut model.lods[0];
    let last = sections - 1;
    lod.selections
        .push(lod_edit::new_selection(b"last".to_vec(), true, &[last], &[], &[]).unwrap());
    let bytes = model.write().unwrap();
    let lod = &Model::parse(&bytes).unwrap().lods[0];
    assert_eq!(
        crate::section_faces::sections(lod, "last").unwrap(),
        vec![last as usize]
    );
}

#[test]
fn proxy_references_out_of_range_are_rejected() {
    let base = Model::parse(SCOPE).unwrap();
    let lod = &base.lods[0];
    let (selections, sections) = (lod.selections.len() as i32 + 1, lod.sections.len() as i32);
    let proxy = || crate::odol::Proxy {
        model: b"\\p".to_vec(),
        transform: [0.0; 12],
        sequence: 1,
        selection: selections - 1,
        bone: -1,
        section: 0,
    };
    for (selection, section, needle) in [
        (
            selections,
            0,
            format!("proxy 0 references selection {selections}"),
        ),
        (
            0,
            sections,
            format!("proxy 0 references section {sections}"),
        ),
    ] {
        corrupt(
            SCOPE,
            0,
            |lod| {
                let s = lod_edit::new_selection(b"p".to_vec(), false, &[], &[], &[]).unwrap();
                lod.selections.push(s);
                lod.proxies = vec![crate::odol::Proxy {
                    selection,
                    section,
                    ..proxy()
                }];
            },
            &needle,
        );
    }
}

#[test]
fn remap_proxies_names_selections_like_dotnet_000() {
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
    let base = model.write().unwrap();
    for (to_id, name) in [
        (json!(-1), "-001"),
        (json!(-99), "-099"),
        (json!(-100), "-100"),
        (json!(0), "000"),
        (json!(1), "001"),
        (json!(999), "999"),
        (json!(1000), "1000"),
        (Value::Null, "-001"),
    ] {
        let mut rule = json!({"model":"OLD","id":1,"toModel":"new"});
        if !to_id.is_null() {
            rule["toId"] = to_id;
        }
        let out = crate::remap_ops::remap_proxies(&base, &json!({"rules":[rule]}), "t").unwrap();
        let lod = &Model::parse(&out).unwrap().lods[0];
        let selection = &lod.selections[lod.proxies[0].selection as usize];
        assert_eq!(selection.name, format!("proxy:new.{name}").as_bytes());
    }
}

#[test]
fn rebuild_shadow_volume_from_itself_matches_csharp() {
    for (from, lod, sha) in [
        (
            "11002",
            "11002",
            "96d7f9e42128311d522bdac06d62228348e9c49edaf04145c9a2052a0e5d74a9",
        ),
        (
            "11002",
            "1.1002e4",
            "96d7f9e42128311d522bdac06d62228348e9c49edaf04145c9a2052a0e5d74a9",
        ),
        (
            "11002",
            "10002",
            "5adf415c14050b6ac5b3a50e61a0d2d2a8efb9b9b164959b8a3d9374fbc83fe8",
        ),
    ] {
        let out = shadow_ops::rebuild(ROUND, &json!({"from":from,"lod":lod}), "t").unwrap();
        assert_eq!(crate::state::sha256(&out), sha, "{from} {lod}");
    }
}
