use crate::{
    odol::{Model, Proxy, lod_edit},
    proxy_ops, selection_ops,
};
use serde_json::json;

const ROUND: &[u8] = include_bytes!("../tests/data/round.p3d");
const SCOPE: &[u8] = include_bytes!("../tests/data/scope.p3d");

fn proxy(model: &[u8], sequence: i32, section: i32) -> Proxy {
    Proxy {
        model: model.to_vec(),
        transform: [0.0; 12],
        sequence,
        selection: 0,
        bone: -1,
        section,
    }
}

fn two_scope_lods(second: &[u8], edit: impl FnOnce(&mut Model)) -> Vec<u8> {
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
            ..proxy(b"p", 1, 0)
        },
        Proxy {
            selection: at,
            ..proxy(second, 2, 2)
        },
    ];
    let second_name = [&b"proxy:"[..], second, b".002"].concat();
    let dest = &mut m.lods[1];
    for name in [b"proxy:p.001".to_vec(), second_name] {
        dest.selections
            .push(lod_edit::new_selection(name, false, &[], &[], &[]).unwrap());
    }
    edit(&mut m);
    m.write().unwrap()
}

fn placed(bytes: &[u8]) -> Vec<(i32, i32)> {
    let op = json!({"from":"1","lods":["2"],"filter":"p"});
    let out = proxy_ops::restore_lod_proxies(bytes, &op, "t").unwrap();
    let dest = &Model::parse(&out).unwrap().lods[1];
    dest.proxies
        .iter()
        .map(|p| (p.sequence, p.section))
        .collect()
}

#[test]
fn restore_lod_proxies_takes_a_sibling_added_earlier_in_the_same_lod() {
    assert_eq!(placed(&two_scope_lods(b"p", |_| {})), [(1, 0), (2, 0)]);
}

#[test]
fn restore_lod_proxies_skips_the_texture_search_for_a_sibling_added_earlier() {
    let bytes = two_scope_lods(b"p", |m| {
        let from = &mut m.lods[0];
        from.textures.push(b"nowhere.paa".to_vec());
        from.sections[2].texture = from.textures.len() as i16 - 1;
    });
    assert_eq!(placed(&bytes), [(1, 0), (2, 0)]);
}

#[test]
fn restore_lod_proxies_matches_a_sibling_model_ignoring_case() {
    assert_eq!(placed(&two_scope_lods(b"P", |_| {})), [(1, 0), (2, 0)]);
}

#[test]
fn restore_lod_proxies_takes_an_initially_present_sibling() {
    let bytes = two_scope_lods(b"p", |m| m.lods[1].proxies = vec![proxy(b"p", 3, 1)]);
    assert_eq!(placed(&bytes), [(3, 1), (1, 1), (2, 1)]);
}

#[test]
fn restore_lod_proxies_rejects_a_texture_index_past_the_end() {
    let bytes = two_scope_lods(b"pq", |m| m.lods[1].sections[1].texture = 9);
    let op = json!({"from":"1","lods":["2"],"filter":"p"});
    let err = proxy_ops::restore_lod_proxies(&bytes, &op, "t").unwrap_err();
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
