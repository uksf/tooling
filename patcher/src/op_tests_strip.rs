use crate::odol::{Model, strip::strip_animations};
use serde_json::json;

const MODEL_SOURCES: &str = "codriver1gun, codriver1muzzle_reload, codriver1muzzle_reloadmagazine, codriver1muzzle_rot, codriver1turret, codrivergun, codrivermuzzle_reload, codrivermuzzle_reloadmagazine, codrivermuzzle_rot, codriverturret, damage, drivingwheel, gatling_1, gatling_2, hide_wlight, lightmain, maingun, mainmuzzle_reload, mainmuzzle_rot, mainturret, minigunl_dir, minigunl_elev, minigunr_dir, minigunr_elev, minilgun, minimuzzle_reload, minirgun, minirmuzzle_reload, obsgun, obsturret";

fn animated(sources: Option<&[&str]>) -> Vec<u8> {
    let mut m = Model::parse(include_bytes!("../tests/data/rhicc-memory.p3d")).unwrap();
    let a = m.animations.as_mut().unwrap();
    a.bones_to_anims.truncate(1);
    a.anims_to_bones.truncate(1);
    a.axes.truncate(1);
    if let Some(sources) = sources {
        for (class, source) in a.classes.iter_mut().zip(sources.iter().cycle()) {
            class.source = source.as_bytes().to_vec();
        }
    }
    m.write().unwrap()
}

fn mismatch(bytes: &[u8], sources: &[&str]) -> String {
    strip_animations(bytes, &json!({ "sources": sources }), "t").unwrap_err()
}

fn message(model: &str, expected: &str) -> String {
    format!("t: the model's animation sources are [{model}], not [{expected}]")
}

#[test]
fn strip_animations_lists_a_single_mismatch_like_csharp() {
    assert_eq!(
        mismatch(&animated(Some(&["actual"])), &["z"]),
        message("actual", "z")
    );
}

#[test]
fn strip_animations_sorts_and_dedupes_both_sides_like_csharp() {
    assert_eq!(
        mismatch(&animated(None), &["z", "a", "z", "A", "B", "b"]),
        message(MODEL_SOURCES, "a, B, z")
    );
    assert_eq!(
        mismatch(&animated(Some(&["Zeta", "alpha", "ZETA", "Beta"])), &["x"]),
        message("alpha, Beta, Zeta", "x")
    );
}

#[test]
fn strip_animations_shows_quotes_and_backslashes_raw() {
    assert_eq!(
        mismatch(&animated(Some(&["a\"b"])), &["c\\d"]),
        message("a\"b", "c\\d")
    );
}

#[test]
fn strip_animations_accepts_reordered_and_duplicated_matching_sources() {
    let bytes = animated(Some(&["Reload", "damage"]));
    let op = json!({ "sources": ["DAMAGE", "reload", "damage"] });
    let out = strip_animations(&bytes, &op, "t").unwrap();
    assert!(Model::parse(&out).unwrap().animations.is_none());
}
