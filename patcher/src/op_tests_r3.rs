use crate::{
    odol::{Model, Proxy, lod_edit},
    proxy_ops, selection_ops,
};
use serde_json::json;

const ROUND: &[u8] = include_bytes!("../tests/data/round.p3d");
const SCOPE: &[u8] = include_bytes!("../tests/data/scope.p3d");

fn proxy(sequence: i32, section: i32) -> Proxy {
    Proxy {
        model: b"p".to_vec(),
        transform: [0.0; 12],
        sequence,
        selection: 0,
        bone: -1,
        section,
    }
}

fn two_scope_lods(dest_texture: Option<(usize, i16)>) -> Vec<u8> {
    let scope = Model::parse(SCOPE).unwrap();
    let mut m = Model::parse(ROUND).unwrap();
    m.lods[0] = scope.lods[0].duplicate(1.0).unwrap();
    m.lods[1] = scope.lods[0].duplicate(2.0).unwrap();
    let from = &mut m.lods[0];
    from.selections
        .push(lod_edit::new_selection(b"proxy:p.001".to_vec(), false, &[], &[], &[]).unwrap());
    let at = from.selections.len() as i32 - 1;
    from.proxies = vec![
        Proxy {
            selection: at,
            ..proxy(1, 0)
        },
        Proxy {
            selection: at,
            ..proxy(2, 2)
        },
    ];
    let dest = &mut m.lods[1];
    for name in ["proxy:p.001", "proxy:p.002"] {
        let s = lod_edit::new_selection(name.as_bytes().to_vec(), false, &[], &[], &[]).unwrap();
        dest.selections.push(s);
    }
    if let Some((section, texture)) = dest_texture {
        dest.sections[section].texture = texture;
    }
    m.write().unwrap()
}

fn restore(bytes: &[u8]) -> Result<Vec<u8>, String> {
    let op = json!({"from":"1","lods":["2"],"filter":"p"});
    proxy_ops::restore_lod_proxies(bytes, &op, "t")
}

#[test]
fn restore_lod_proxies_ignores_proxies_added_in_the_same_lod() {
    let out = restore(&two_scope_lods(None)).unwrap();
    let dest = &Model::parse(&out).unwrap().lods[1];
    let placed: Vec<_> = dest
        .proxies
        .iter()
        .map(|p| (p.sequence, p.section))
        .collect();
    assert_eq!(placed, [(1, 0), (2, 2)]);
}

#[test]
fn restore_lod_proxies_rejects_a_texture_index_past_the_end() {
    let err = restore(&two_scope_lods(Some((1, 9)))).unwrap_err();
    assert_eq!(err, "t: invalid texture");
}

#[test]
fn move_sections_rejects_a_texture_index_past_the_end() {
    let mut m = Model::parse(SCOPE).unwrap();
    m.lods[0].sections[1].texture = 9;
    let op =
        json!({"lod":"0","texture":"#(argb,8,8,3)color(0,0,0,1)","newSelection":"n","from":[]});
    let err = selection_ops::move_sections(&m.write().unwrap(), &op, "t")
        .err()
        .unwrap();
    assert_eq!(err, "t: section 1 references texture 9, outside the LOD");
}
